// Permission is hereby granted, free of charge, to any person obtaining a
// copy of this software and associated documentation files (the "Software"),
// to deal in the Software without restriction, including without limitation
// the rights to use, copy, modify, merge, publish, distribute, sublicense,
// and/or sell copies of the Software, and to permit persons to whom the
// Software is furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
// OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
// FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
// DEALINGS IN THE SOFTWARE.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use tokio::net::TcpListener;
use tokio::sync::Semaphore;
use tokio_rustls::TlsAcceptor;
use tracing;

use super::auth::{self, TokenService};
use super::control_plane::{ControlPlane, FakeControlPlane};
use super::errors::I2pControlError;
use super::rpc::{
    self, AuthenticateParams, AuthenticateResult, JsonRpcErrorResponse, JsonRpcRequest,
    JsonRpcSuccess, RequestId,
};
use super::tls::TlsConfig;

const LOG_TARGET: &str = "emissary::i2pcontrol::server";

/// Maximum request body size (1 MiB).
const MAX_BODY_SIZE: usize = 1024 * 1024;

/// Maximum concurrent in-flight requests.
const MAX_CONCURRENT_REQUESTS: usize = 64;

/// I2PControl server configuration.
#[derive(Debug, Clone)]
pub struct I2pControlConfig {
    /// Whether I2PControl is enabled.
    pub enabled: bool,
    /// Bind address.
    pub bind: SocketAddr,
    /// Password for authentication.
    pub password: String,
    /// TLS configuration.
    pub tls: TlsConfig,
}

impl I2pControlConfig {
    /// Validate the configuration.
    pub fn validate(&self) -> Result<(), I2pControlError> {
        if self.enabled && self.password.is_empty() {
            return Err(I2pControlError::Config(
                "I2PControl enabled but no password configured".into(),
            ));
        }

        // Warn on non-loopback binding
        if self.enabled && !self.bind.ip().is_loopback() {
            tracing::warn!(
                target: LOG_TARGET,
                bind = %self.bind,
                "I2PControl bound to non-loopback address; ensure this is intentional",
            );
        }

        Ok(())
    }
}

/// Shared application state for the I2PControl server.
struct I2pControlState {
    token_service: TokenService,
    password: String,
    control_plane: Box<dyn ControlPlane>,
    semaphore: Semaphore,
}

/// Start the I2PControl HTTPS server.
///
/// This function:
/// 1. Validates configuration
/// 2. Builds TLS material
/// 3. Binds the listener
/// 4. Runs the server under structured cancellation
/// 5. Performs bounded graceful shutdown
pub async fn run_server(
    config: I2pControlConfig,
    base_path: std::path::PathBuf,
    mut shutdown_rx: tokio::sync::broadcast::Receiver<()>,
) -> Result<(), I2pControlError> {
    if !config.enabled {
        tracing::debug!(
            target: LOG_TARGET,
            "I2PControl is disabled, not starting listener",
        );
        return Ok(());
    }

    config.validate()?;

    // Build TLS config
    let tls_config = super::tls::build_tls_config(&config.tls, &base_path)?;
    let _tls_acceptor = TlsAcceptor::from(tls_config);

    // Build shared state
    let state = Arc::new(I2pControlState {
        token_service: TokenService::new(),
        password: config.password.clone(),
        control_plane: Box::new(FakeControlPlane::new()),
        semaphore: Semaphore::new(MAX_CONCURRENT_REQUESTS),
    });

    // Build Axum router
    let app = Router::new().route("/", post(handle_jsonrpc)).with_state(state.clone());

    let app = app.into_make_service();

    // Bind listener
    let listener = TcpListener::bind(config.bind)
        .await
        .map_err(|e| I2pControlError::Bind(format!("Failed to bind to {}: {e}", config.bind)))?;

    tracing::info!(
        target: LOG_TARGET,
        bind = %config.bind,
        "I2PControl HTTPS listener started",
    );

    // Run server with graceful shutdown
    let result = tokio::select! {
        result = axum::serve(listener, app) => {
            match result {
                Ok(()) => {
                    tracing::info!(
                        target: LOG_TARGET,
                        "I2PControl server exited normally",
                    );
                    Ok(())
                }
                Err(e) => {
                    tracing::error!(
                        target: LOG_TARGET,
                        ?e,
                        "I2PControl server failed",
                    );
                    Err(I2pControlError::Internal(format!("Server error: {e}")))
                }
            }
        }
        _ = shutdown_rx.recv() => {
            tracing::info!(
                target: LOG_TARGET,
                "I2PControl server received shutdown signal",
            );
            Ok(())
        }
    };

    // Clear tokens on shutdown
    state.token_service.clear();

    result
}

/// Resolve an optional request ID, defaulting to Null.
fn resolve_id(id: &Option<RequestId>) -> RequestId {
    id.clone().unwrap_or(RequestId::Null)
}

/// Handle a JSON-RPC request.
async fn handle_jsonrpc(
    State(state): State<Arc<I2pControlState>>,
    headers: HeaderMap,
    body: String,
) -> Response {
    // Acquire concurrency permit
    let _permit =
        match tokio::time::timeout(Duration::from_secs(5), state.semaphore.acquire()).await {
            Ok(Ok(permit)) => permit,
            _ => {
                return (
                    StatusCode::SERVICE_UNAVAILABLE,
                    Json(serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": null,
                        "error": {
                            "code": -32603,
                            "message": "Server is busy"
                        }
                    })),
                )
                    .into_response();
            }
        };

    // Check body size
    if body.len() > MAX_BODY_SIZE {
        return (
            StatusCode::PAYLOAD_TOO_LARGE,
            Json(serde_json::json!({
                "jsonrpc": "2.0",
                "id": null,
                "error": {
                    "code": -32700,
                    "message": "Request body too large"
                }
            })),
        )
            .into_response();
    }

    // Parse JSON-RPC request
    let request = match rpc::parse_request(&body) {
        Ok(req) => req,
        Err(err) => return Json(serde_json::to_value(&err).unwrap()).into_response(),
    };

    // Handle notification (null ID) — no response
    if request.id.is_none() || request.id == Some(RequestId::Null) {
        return StatusCode::NO_CONTENT.into_response();
    }

    // Dispatch the method
    let response = match request.method.as_str() {
        rpc::methods::AUTHENTICATE => handle_authenticate(&state, &request).await,
        _ => {
            // Unknown method — check token first for protected methods
            let token = extract_token(&headers);
            if !state.token_service.validate(token) {
                serde_json::to_value(&JsonRpcErrorResponse::new(
                    resolve_id(&request.id),
                    rpc::error_codes::APP_ERROR,
                    "Authentication required",
                ))
                .unwrap()
            } else {
                serde_json::to_value(&JsonRpcErrorResponse::new(
                    resolve_id(&request.id),
                    rpc::error_codes::METHOD_NOT_FOUND,
                    format!("Method '{}' not found", request.method),
                ))
                .unwrap()
            }
        }
    };

    Json(response).into_response()
}

/// Handle the Authenticate method.
///
/// Returns a `serde_json::Value` that is either a success or error JSON-RPC response.
async fn handle_authenticate(
    state: &I2pControlState,
    request: &JsonRpcRequest,
) -> serde_json::Value {
    let id = resolve_id(&request.id);

    // Parse authenticate params
    let params = match &request.params {
        Some(params) => {
            match serde_json::from_value::<AuthenticateParams>(serde_json::Value::Object(
                params.clone(),
            )) {
                Ok(p) => p,
                Err(_) => {
                    return serde_json::to_value(&JsonRpcErrorResponse::new(
                        id,
                        rpc::error_codes::INVALID_PARAMS,
                        "Invalid Authenticate parameters",
                    ))
                    .unwrap();
                }
            }
        }
        None => {
            return serde_json::to_value(&JsonRpcErrorResponse::new(
                id,
                rpc::error_codes::INVALID_PARAMS,
                "Missing parameters",
            ))
            .unwrap();
        }
    };

    // Validate API version
    let api_version = match params.api {
        Some(v) if auth::validate_api_version(v) => v,
        _ => {
            return serde_json::to_value(&JsonRpcErrorResponse::new(
                id,
                rpc::error_codes::INVALID_PARAMS,
                "Invalid or missing API version (must be 1 or 2)",
            ))
            .unwrap();
        }
    };

    // Validate username
    match params.username.as_deref() {
        Some("i2pcontrol") => {}
        _ => {
            return serde_json::to_value(&JsonRpcErrorResponse::new(
                id,
                rpc::error_codes::APP_ERROR,
                "Invalid username or password",
            ))
            .unwrap();
        }
    }

    // Validate password
    let password = match params.password.as_deref() {
        Some(p) => p,
        None => {
            return serde_json::to_value(&JsonRpcErrorResponse::new(
                id,
                rpc::error_codes::APP_ERROR,
                "Invalid username or password",
            ))
            .unwrap();
        }
    };

    if !auth::compare_passwords(password, &state.password) {
        return serde_json::to_value(&JsonRpcErrorResponse::new(
            id,
            rpc::error_codes::APP_ERROR,
            "Invalid username or password",
        ))
        .unwrap();
    }

    // Issue token
    let token = state.token_service.issue();

    tracing::debug!(
        target: LOG_TARGET,
        "Authenticate successful",
    );

    serde_json::to_value(&JsonRpcSuccess::new(
        id,
        serde_json::to_value(&AuthenticateResult {
            Token: token,
            API: api_version.to_string(),
        })
        .unwrap(),
    ))
    .unwrap()
}

/// Extract the token from the request headers.
///
/// I2PControl uses the `X-I2PControl-Token` header for authentication.
fn extract_token(headers: &HeaderMap) -> &str {
    headers.get("X-I2PControl-Token").and_then(|v| v.to_str().ok()).unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_validation_empty_password() {
        let config = I2pControlConfig {
            enabled: true,
            bind: "127.0.0.1:7650".parse().unwrap(),
            password: String::new(),
            tls: TlsConfig {
                certificate: None,
                private_key: None,
            },
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn config_validation_disabled_no_password() {
        let config = I2pControlConfig {
            enabled: false,
            bind: "127.0.0.1:7650".parse().unwrap(),
            password: String::new(),
            tls: TlsConfig {
                certificate: None,
                private_key: None,
            },
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn extract_token_from_headers() {
        let mut headers = HeaderMap::new();
        let name: axum::http::HeaderName = "x-i2pcontrol-token".parse().unwrap();
        headers.insert(name, "test-token-123".parse().unwrap());
        assert_eq!(extract_token(&headers), "test-token-123");
    }

    #[test]
    fn extract_token_missing() {
        let headers = HeaderMap::new();
        assert_eq!(extract_token(&headers), "");
    }

    #[test]
    fn resolve_id_returns_id() {
        assert_eq!(
            resolve_id(&Some(RequestId::Number(1))),
            RequestId::Number(1)
        );
    }

    #[test]
    fn resolve_id_defaults_to_null() {
        assert_eq!(resolve_id(&None), RequestId::Null);
    }
}
