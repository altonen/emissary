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
use super::production::{
    EventMetrics, ProductionAddressBookControl, ProductionControlPlane,
    ProductionRouterInfoControl, ProductionTunnelManagerControl,
};
use super::router_info::{FakeRouterInfoControl, RouterInfoControl};
use super::rpc::{
    self, AuthenticateParams, AuthenticateResult, JsonRpcErrorResponse, JsonRpcRequest,
    JsonRpcSuccess, RequestId,
};
use super::service_registry::{ServiceRegistry, ServiceSnapshot};
use super::tls::TlsConfig;

use emissary_core::crypto::base64_encode;

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
    /// Local router identity in Base64 (retained at startup, never re-read).
    router_id: String,
    /// Serialized local RouterInfo bytes (retained at startup).
    router_info_bytes: Vec<u8>,
    /// Base64 encoding of serialized RouterInfo.
    router_info_b64: String,
    /// Startup time for uptime calculation.
    startup_time: std::time::Instant,
    /// Cumulative metrics snapshot (fed by transport/tunnel event tracking).
    metrics_snapshot: super::observability::MetricsSnapshot,
    /// Rolling traffic window (fed by transport byte accounting).
    rolling_window: Arc<super::observability::RollingWindow>,
    /// Shared log ring for I2PControl snapshot/clear.
    log_ring: Arc<super::observability::LogRing>,
    /// Passive client-service registry for ClientServicesInfo.
    service_registry: ServiceRegistry,
}

impl I2pControlState {
    /// Create a new state with the given password.
    pub fn new(password: String) -> Self {
        let metrics_snapshot = super::observability::MetricsSnapshot::new();
        let rolling_window = Arc::new(super::observability::RollingWindow::default());
        let log_ring = Arc::new(super::observability::LogRing::default());
        Self {
            token_service: TokenService::new(),
            password,
            control_plane: Box::new(FakeControlPlane::new()),
            address_book_control: Box::new(FakeAddressBookControl::new()),
            tunnel_manager: Box::new(FakeTunnelManagerControl::new()),
            router_info: Box::new(FakeRouterInfoControl::new()),
            semaphore: Semaphore::new(MAX_CONCURRENT_REQUESTS),
            router_id: String::new(),
            router_info_bytes: Vec::new(),
            router_info_b64: String::new(),
            startup_time: std::time::Instant::now(),
            metrics_snapshot,
            rolling_window,
            log_ring,
            service_registry: ServiceRegistry::new(),
        }
    }

    /// Get a clone of the shared log ring (used by the production router
    /// info adapter for I2PControl log snapshot/clear).
    pub fn log_ring_arc(&self) -> Arc<super::observability::LogRing> {
        Arc::clone(&self.log_ring)
    }

    /// Get a reference to the service registry.
    #[allow(dead_code)]
    pub fn service_registry(&self) -> &ServiceRegistry {
        &self.service_registry
    }

    /// Take a snapshot from the service registry.
    #[allow(dead_code)]
    pub fn service_snapshot(&self) -> ServiceSnapshot {
        self.service_registry.snapshot()
    }

    /// Replace the service registry (for testing or composition).
    #[allow(dead_code)]
    pub fn set_service_registry(&mut self, registry: ServiceRegistry) {
        self.service_registry = registry;
    }

    /// Get a reference to the token service.
    pub fn token_service(&self) -> &TokenService {
        &self.token_service
    }

    /// Get a reference to the metrics snapshot for feeding/reading.
    pub fn metrics_snapshot(&self) -> &super::observability::MetricsSnapshot {
        &self.metrics_snapshot
    }

    /// Get a reference to the rolling window for feeding/reading.
    pub fn rolling_window(&self) -> &Arc<super::observability::RollingWindow> {
        &self.rolling_window
    }

    /// Get a reference to the router info control.
    pub fn router_info(&self) -> &dyn RouterInfoControl {
        &*self.router_info
    }

    /// Get a reference to the address book control.
    pub fn address_book_control(&self) -> &dyn AddressBookControl {
        &*self.address_book_control
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

    /// Replace the address book control plane with a production adapter.
    pub fn set_address_book_control_production(&mut self, control: Box<dyn AddressBookControl>) {
        self.address_book_control = control;
    }

    /// Replace the tunnel manager control plane with a production adapter.
    pub fn set_tunnel_manager_production(&mut self, control: Box<dyn TunnelManagerControl>) {
        self.tunnel_manager = control;
    }

    /// Replace the router info control plane with a production adapter.
    pub fn set_router_info_production(&mut self, control: Box<dyn RouterInfoControl>) {
        self.router_info = control;
    }

    /// Replace the control plane with a production adapter.
    pub fn set_control_plane_production(&mut self, control: Box<dyn ControlPlane>) {
        self.control_plane = control;
    }

    /// Set startup-retained values (router identity, serialized RI).
    ///
    /// These are retained once at startup and never re-read from disk.
    pub fn set_startup_values(
        &mut self,
        router_id: String,
        router_info_bytes: Vec<u8>,
        router_info_b64: String,
    ) {
        self.router_id = router_id;
        self.router_info_bytes = router_info_bytes;
        self.router_info_b64 = router_info_b64;
    }

    /// Get the local router identity (Base64).
    #[allow(dead_code)]
    pub fn router_id(&self) -> &str {
        &self.router_id
    }

    /// Get the serialized RouterInfo bytes.
    #[allow(dead_code)]
    pub fn router_info_bytes(&self) -> &[u8] {
        &self.router_info_bytes
    }

    /// Get the Base64-encoded serialized RouterInfo.
    #[allow(dead_code)]
    pub fn router_info_b64(&self) -> &str {
        &self.router_info_b64
    }

    /// Get the uptime since server startup.
    #[allow(dead_code)]
    pub fn uptime(&self) -> std::time::Duration {
        self.startup_time.elapsed()
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

/// Bundle of dependencies used to construct the production I2PControl server.
///
/// All fields are optional. When present, they replace the corresponding
/// fakes with production adapters. The fakes are retained as defaults so
/// that headless test environments and unit tests can build a server
/// without supplying real router state.
pub struct ServerInitContext {
    /// Local router identity in Base64.
    pub router_id: String,
    /// Serialized local RouterInfo bytes.
    pub router_info_bytes: Vec<u8>,
    /// Event metrics source for bandwidth and tunnel build counters.
    ///
    /// When `None`, a default zeroed source is used.
    pub event_metrics: Option<Arc<dyn EventMetrics>>,
    /// Share ratio from the active configuration.
    pub share_ratio: f64,
    /// Configured inbound bandwidth limit in bytes/second.
    pub configured_bandwidth_in: u64,
    /// Configured outbound bandwidth limit in bytes/second.
    pub configured_bandwidth_out: u64,
    /// Whether to use the production address book adapter.
    pub use_production_address_book: bool,
    /// Whether to use the production tunnel manager adapter.
    pub use_production_tunnel_manager: bool,
}

impl ServerInitContext {
    /// Create a new init context with the required startup values and
    /// sensible defaults for optional dependencies.
    pub fn new(router_id: String, router_info_bytes: Vec<u8>) -> Self {
        Self {
            router_id,
            router_info_bytes,
            event_metrics: None,
            share_ratio: 0.0,
            configured_bandwidth_in: 0,
            configured_bandwidth_out: 0,
            use_production_address_book: false,
            use_production_tunnel_manager: false,
        }
    }

    /// Set the event metrics source.
    pub fn with_event_metrics(mut self, metrics: Arc<dyn EventMetrics>) -> Self {
        self.event_metrics = Some(metrics);
        self
    }

    /// Set the share ratio.
    pub fn with_share_ratio(mut self, ratio: f64) -> Self {
        self.share_ratio = ratio;
        self
    }

    /// Set the configured bandwidth limits.
    pub fn with_configured_bandwidth(mut self, inbound: u64, outbound: u64) -> Self {
        self.configured_bandwidth_in = inbound;
        self.configured_bandwidth_out = outbound;
        self
    }

    /// Enable the production address book adapter rooted at the given path.
    pub fn with_production_address_book(mut self) -> Self {
        self.use_production_address_book = true;
        self
    }

    /// Enable the production tunnel manager adapter rooted at the given path.
    pub fn with_production_tunnel_manager(mut self) -> Self {
        self.use_production_tunnel_manager = true;
        self
    }
}

/// Initialize the I2PControl server: validate config, set up TLS, bind the port.
///
/// Returns a `ServerInstance` ready to serve, or an error if startup fails.
/// This function is synchronous-safe for calling from `setup_router` so that
/// bind/TLS/startup failures are surfaced as application errors.
pub async fn init_server(
    config: &I2pControlConfig,
    base_path: &std::path::Path,
    ctx: ServerInitContext,
) -> Result<ServerInstance, I2pControlError> {
    config.validate()?;

    // Build TLS config (validates cert/key material)
    let tls_config = super::tls::build_tls_config(&config.tls, base_path)?;
    let _tls_acceptor = tokio_rustls::TlsAcceptor::from(tls_config);

    // Build shared state with startup-retained values
    let router_info_bytes = ctx.router_info_bytes.clone();
    let router_info_b64 = base64_encode(&router_info_bytes);
    let mut state = I2pControlState::new(config.password.clone());
    state.set_startup_values(ctx.router_id, router_info_bytes, router_info_b64);

    // Wire production adapters as requested. Each branch is independent so
    // a single failure does not affect unrelated adapters.
    if ctx.use_production_address_book {
        let dir = base_path.join("addressbooks");
        if let Err(e) = std::fs::create_dir_all(&dir) {
            tracing::warn!(
                target: LOG_TARGET,
                ?e,
                dir = %dir.display(),
                "failed to create address book dir, falling back to fake",
            );
        } else {
            let ab = ProductionAddressBookControl::new(dir);
            if let Err(e) = ab.load().await {
                tracing::warn!(
                    target: LOG_TARGET,
                    ?e,
                    "failed to load address book store, falling back to fake",
                );
            } else {
                state.set_address_book_control_production(Box::new(ab));
            }
        }
    }

    if ctx.use_production_tunnel_manager {
        let dir = base_path.join("tunnels");
        if let Err(e) = std::fs::create_dir_all(&dir) {
            tracing::warn!(
                target: LOG_TARGET,
                ?e,
                dir = %dir.display(),
                "failed to create tunnel store dir, falling back to fake",
            );
        } else {
            match ProductionTunnelManagerControl::new(dir) {
                Ok(tm) => {
                    if let Err(e) = tm.load().await {
                        tracing::warn!(
                            target: LOG_TARGET,
                            ?e,
                            "failed to load tunnel store, falling back to fake",
                        );
                    } else {
                        state.set_tunnel_manager_production(Box::new(tm));
                    }
                }
                Err(e) => tracing::warn!(
                    target: LOG_TARGET,
                    ?e,
                    "failed to create production tunnel manager, falling back to fake",
                ),
            }
        }
    }

    // Wire the production control plane when we have event metrics. The
    // production router info adapter needs the tunnel manager; if a fake
    // is in use, the configured tunnel count is reported as 0.
    if let Some(metrics) = ctx.event_metrics.clone() {
        let cp = ProductionControlPlane::new(
            state.router_id.clone(),
            env!("CARGO_PKG_VERSION").to_string(),
            Arc::clone(&metrics),
        );
        state.set_control_plane_production(Box::new(cp));

        // Build a router info adapter. If the user enabled the production
        // tunnel manager, point the adapter at the same directory; otherwise
        // build a fresh in-memory adapter.
        let tm_arc: Arc<ProductionTunnelManagerControl> = if ctx.use_production_tunnel_manager {
            let dir = base_path.join("tunnels");
            ProductionTunnelManagerControl::new(dir).ok().map(Arc::new).unwrap_or_else(|| {
                // Use a safe fallback directory.
                let dir = std::env::temp_dir().join("emissary-i2pcontrol-tunnels-fallback");
                let _ = std::fs::create_dir_all(&dir);
                Arc::new(
                    ProductionTunnelManagerControl::new(dir)
                        .expect("tunnel store directory was created"),
                )
            })
        } else {
            let dir = std::env::temp_dir().join("emissary-i2pcontrol-tunnels-fallback");
            let _ = std::fs::create_dir_all(&dir);
            Arc::new(
                ProductionTunnelManagerControl::new(dir)
                    .expect("tunnel store directory was created"),
            )
        };
        let log_ring = state.log_ring_arc();
        let ri = ProductionRouterInfoControl::new(
            state.router_id.clone(),
            env!("CARGO_PKG_VERSION").to_string(),
            ctx.share_ratio,
            ctx.configured_bandwidth_in,
            ctx.configured_bandwidth_out,
            metrics,
            log_ring,
            tm_arc,
        );
        state.set_router_info_production(Box::new(ri));
    }

    let state = Arc::new(state);

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
                super::router_info_handler::handle_router_info(&state, &request).await
            }
        }
        rpc::methods::CLIENT_SERVICES_INFO => {
            let token = extract_token(&headers);
            if !state.token_service.validate(token) {
                serde_json::to_value(JsonRpcErrorResponse::new(
                    resolve_id(&request.id),
                    rpc::error_codes::APP_ERROR,
                    "Authentication required",
                ))
                .unwrap()
            } else {
                super::client_services::handle_client_services_info(&state, &request).await
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
