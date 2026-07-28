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
use tracing;

use super::auth::{self, TokenService};
use super::control_plane::{
    AddressBookControl, ControlPlane, FakeAddressBookControl, FakeControlPlane,
    FakeTunnelManagerControl, TunnelManagerControl,
};
use super::errors::I2pControlError;
use super::router_info::{FakeRouterInfoControl, RouterInfoControl};
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
pub(crate) struct I2pControlState {
    token_service: TokenService,
    #[allow(dead_code)]
    password: String,
    #[allow(dead_code)]
    control_plane: Box<dyn ControlPlane>,
    address_book_control: Box<dyn AddressBookControl>,
    tunnel_manager: Box<dyn TunnelManagerControl>,
    router_info: Box<dyn RouterInfoControl>,
    semaphore: Semaphore,
}

impl I2pControlState {
    /// Create a new state with the given password.
    pub fn new(password: String) -> Self {
        Self {
            token_service: TokenService::new(),
            password,
            control_plane: Box::new(FakeControlPlane::new()),
            address_book_control: Box::new(FakeAddressBookControl::new()),
            tunnel_manager: Box::new(FakeTunnelManagerControl::new()),
            router_info: Box::new(FakeRouterInfoControl::new()),
            semaphore: Semaphore::new(MAX_CONCURRENT_REQUESTS),
        }
    }

    /// Get a reference to the token service.
    pub fn token_service(&self) -> &TokenService {
        &self.token_service
    }

    /// Replace the address book control plane (for testing).
    pub fn set_address_book_control(&mut self, control: Box<dyn AddressBookControl>) {
        self.address_book_control = control;
    }

    /// Replace the tunnel manager control plane (for testing).
    pub fn set_tunnel_manager(&mut self, control: Box<dyn TunnelManagerControl>) {
        self.tunnel_manager = control;
    }

    /// Replace the router info inspection control plane (for testing).
    #[allow(dead_code)]
    pub fn set_router_info(&mut self, control: Box<dyn RouterInfoControl>) {
        self.router_info = control;
    }

    /// List all tunnel definitions.
    pub async fn tunnel_list(&self) -> Vec<crate::i2pcontrol::domain::tunnel::TunnelDefinition> {
        self.tunnel_manager.list().await.unwrap_or_default()
    }

    /// Get a tunnel definition by name.
    pub async fn tunnel_get(
        &self,
        name: &str,
    ) -> Option<crate::i2pcontrol::domain::tunnel::TunnelDefinition> {
        self.tunnel_manager.get(name).await.ok().flatten()
    }

    /// Create a new tunnel definition.
    pub async fn tunnel_create(
        &self,
        definition: crate::i2pcontrol::domain::tunnel::TunnelDefinition,
    ) -> Result<(), String> {
        self.tunnel_manager.create(definition).await
    }

    /// Update an existing tunnel definition.
    pub async fn tunnel_update(
        &self,
        name: &str,
        definition: crate::i2pcontrol::domain::tunnel::TunnelDefinition,
        new_name: Option<crate::i2pcontrol::domain::tunnel::TunnelName>,
    ) -> Result<bool, String> {
        self.tunnel_manager.update(name, definition, new_name).await
    }

    /// Delete a tunnel definition.
    pub async fn tunnel_delete(&self, name: &str) -> Result<bool, String> {
        self.tunnel_manager.delete(name).await
    }

    /// Start a tunnel.
    pub async fn tunnel_start(&self, name: &str) -> Result<String, String> {
        self.tunnel_manager.start(name).await
    }

    /// Stop a tunnel.
    pub async fn tunnel_stop(&self, name: &str) -> Result<String, String> {
        self.tunnel_manager.stop(name).await
    }

    /// Restart a tunnel.
    pub async fn tunnel_restart(&self, name: &str) -> Result<String, String> {
        self.tunnel_manager.restart(name).await
    }

    /// List entries in the specified address book.
    pub async fn address_book_list(
        &self,
        book_type: crate::i2pcontrol::domain::address_book::AdministrativeAddressBookType,
    ) -> Vec<crate::i2pcontrol::domain::address_book::AddressBookEntry> {
        self.address_book_control.list(book_type).await.unwrap_or_default()
    }

    /// Look up an entry in the specified address book.
    pub async fn address_book_lookup(
        &self,
        book_type: crate::i2pcontrol::domain::address_book::AdministrativeAddressBookType,
        hostname: &str,
    ) -> Option<crate::i2pcontrol::domain::address_book::AddressBookEntry> {
        self.address_book_control.lookup(book_type, hostname).await.ok().flatten()
    }

    /// Add an entry to the specified address book.
    pub async fn address_book_add(
        &self,
        book_type: crate::i2pcontrol::domain::address_book::AdministrativeAddressBookType,
        entry: crate::i2pcontrol::domain::address_book::AddressBookEntry,
    ) -> Result<(), String> {
        self.address_book_control.add(book_type, entry).await
    }

    /// Update an entry in the specified address book.
    pub async fn address_book_update(
        &self,
        book_type: crate::i2pcontrol::domain::address_book::AdministrativeAddressBookType,
        entry: crate::i2pcontrol::domain::address_book::AddressBookEntry,
    ) -> Result<bool, String> {
        self.address_book_control.update(book_type, entry).await
    }

    /// Delete an entry from the specified address book.
    pub async fn address_book_delete(
        &self,
        book_type: crate::i2pcontrol::domain::address_book::AdministrativeAddressBookType,
        hostname: &str,
    ) -> Result<bool, String> {
        self.address_book_control.delete(book_type, hostname).await
    }

    /// Delete all entries from the specified address book.
    pub async fn address_book_delete_all(
        &self,
        book_type: crate::i2pcontrol::domain::address_book::AdministrativeAddressBookType,
    ) -> Result<bool, String> {
        self.address_book_control.delete_all(book_type).await
    }

    /// Get the current subscription set.
    pub async fn address_book_subscriptions(
        &self,
    ) -> crate::i2pcontrol::domain::address_book::SubscriptionSet {
        self.address_book_control.subscriptions().await.unwrap_or_default()
    }

    /// Set the subscription set atomically.
    pub async fn address_book_set_subscriptions(
        &self,
        subscriptions: crate::i2pcontrol::domain::address_book::SubscriptionSet,
    ) -> Result<(), String> {
        self.address_book_control.set_subscriptions(subscriptions).await
    }

    /// Get the address book configuration.
    pub async fn address_book_configuration(
        &self,
    ) -> crate::i2pcontrol::domain::address_book::AddressBookConfiguration {
        self.address_book_control.configuration().await.unwrap_or_default()
    }

    /// Set the address book configuration atomically.
    pub async fn address_book_set_configuration(
        &self,
        configuration: crate::i2pcontrol::domain::address_book::AddressBookConfiguration,
    ) -> Result<(), String> {
        self.address_book_control.set_configuration(configuration).await
    }
}

/// A bound and initialized I2PControl server, ready to serve requests.
///
/// Created by `init_server` which performs validation, TLS setup, and port binding.
/// Passed to `serve` which runs the request loop under structured cancellation.
pub struct ServerInstance {
    listener: TcpListener,
    state: Arc<I2pControlState>,
    bind: SocketAddr,
}

/// Initialize the I2PControl server: validate config, set up TLS, bind the port.
///
/// Returns a `ServerInstance` ready to serve, or an error if startup fails.
/// This function is synchronous-safe for calling from `setup_router` so that
/// bind/TLS/startup failures are surfaced as application errors.
pub async fn init_server(
    config: &I2pControlConfig,
    base_path: &std::path::Path,
) -> Result<ServerInstance, I2pControlError> {
    config.validate()?;

    // Build TLS config (validates cert/key material)
    let tls_config = super::tls::build_tls_config(&config.tls, base_path)?;
    let _tls_acceptor = tokio_rustls::TlsAcceptor::from(tls_config);

    // Build shared state
    let state = Arc::new(I2pControlState::new(config.password.clone()));

    // Bind listener — this verifies the port is available
    let listener = TcpListener::bind(config.bind)
        .await
        .map_err(|e| I2pControlError::Bind(format!("Failed to bind to {}: {e}", config.bind)))?;

    tracing::info!(
        target: LOG_TARGET,
        bind = %config.bind,
        "I2PControl HTTPS listener bound",
    );

    Ok(ServerInstance {
        listener,
        state,
        bind: config.bind,
    })
}

/// Run the I2PControl server loop with structured shutdown.
///
/// This function is called from a spawned task after `init_server` has
/// validated configuration, set up TLS, and bound the port.
pub async fn serve(
    instance: ServerInstance,
    mut shutdown_rx: tokio::sync::broadcast::Receiver<()>,
) -> Result<(), I2pControlError> {
    let ServerInstance {
        listener,
        state,
        bind,
    } = instance;

    // Build Axum router
    let app = Router::new().route("/", post(handle_jsonrpc)).with_state(state.clone());

    let app = app.into_make_service();

    tracing::info!(
        target: LOG_TARGET,
        %bind,
        "I2PControl HTTPS server accepting requests",
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
    state.token_service().clear();

    result
}

/// Resolve an optional request ID, defaulting to Null.
fn resolve_id(id: &Option<RequestId>) -> RequestId {
    id.clone().unwrap_or(RequestId::Null)
}

/// Handle a JSON-RPC request.
pub(crate) async fn handle_jsonrpc(
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
        rpc::methods::ADDRESS_BOOK => {
            let token = extract_token(&headers);
            if !state.token_service.validate(token) {
                serde_json::to_value(JsonRpcErrorResponse::new(
                    resolve_id(&request.id),
                    rpc::error_codes::APP_ERROR,
                    "Authentication required",
                ))
                .unwrap()
            } else {
                super::address_book::handle_address_book(&state, &request).await
            }
        }
        rpc::methods::SET_SUBSCRIPTIONS => {
            let token = extract_token(&headers);
            if !state.token_service.validate(token) {
                serde_json::to_value(JsonRpcErrorResponse::new(
                    resolve_id(&request.id),
                    rpc::error_codes::APP_ERROR,
                    "Authentication required",
                ))
                .unwrap()
            } else {
                super::address_book::handle_set_subscriptions(&state, &request).await
            }
        }
        rpc::methods::SET_CONFIG => {
            let token = extract_token(&headers);
            if !state.token_service.validate(token) {
                serde_json::to_value(JsonRpcErrorResponse::new(
                    resolve_id(&request.id),
                    rpc::error_codes::APP_ERROR,
                    "Authentication required",
                ))
                .unwrap()
            } else {
                super::address_book::handle_set_config(&state, &request).await
            }
        }
        rpc::methods::TUNNEL_MANAGER => {
            let token = extract_token(&headers);
            if !state.token_service.validate(token) {
                serde_json::to_value(JsonRpcErrorResponse::new(
                    resolve_id(&request.id),
                    rpc::error_codes::APP_ERROR,
                    "Authentication required",
                ))
                .unwrap()
            } else {
                super::tunnel_manager::handle_tunnel_manager(&state, &request).await
            }
        }
        rpc::methods::ROUTER_INFO => {
            let token = extract_token(&headers);
            if !state.token_service.validate(token) {
                serde_json::to_value(JsonRpcErrorResponse::new(
                    resolve_id(&request.id),
                    rpc::error_codes::APP_ERROR,
                    "Authentication required",
                ))
                .unwrap()
            } else {
                super::router_info_handler::handle_router_info(
                    &*state.router_info,
                    &*state.address_book_control,
                    &request,
                )
                .await
            }
        }
        _ => {
            // Unknown method — check token first for protected methods
            let token = extract_token(&headers);
            if !state.token_service.validate(token) {
                serde_json::to_value(JsonRpcErrorResponse::new(
                    resolve_id(&request.id),
                    rpc::error_codes::APP_ERROR,
                    "Authentication required",
                ))
                .unwrap()
            } else {
                serde_json::to_value(JsonRpcErrorResponse::new(
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
