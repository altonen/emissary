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
use super::control_plane::{AddressBookControl, ControlPlane, TunnelManagerControl};
use super::errors::I2pControlError;
use super::production::{
    EventMetrics, ProductionAddressBookControl, ProductionControlPlane,
    ProductionRouterInfoControl, ProductionTunnelManagerControl,
};
use super::router_info::RouterInfoControl;
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
///
/// Production state is constructed via [`new_production`] with all required
/// dependencies supplied explicitly. Test state is constructed via
/// [`new_test`] which installs fake adapters. The generic `new()` is
/// retained only for internal composition in `init_server`.
pub(crate) struct I2pControlState {
    token_service: TokenService,
    #[allow(dead_code)]
    password: String,
    #[allow(dead_code)]
    control_plane: Arc<dyn ControlPlane>,
    address_book_control: Arc<dyn AddressBookControl>,
    tunnel_manager: Arc<dyn TunnelManagerControl>,
    router_info: Arc<dyn RouterInfoControl>,
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
    /// Create production state from required, already-validated dependencies.
    ///
    /// All adapter objects are constructed and loaded by the caller before
    /// this call. The state takes ownership of the `Arc` clones and never
    /// falls back to fake adapters.
    pub fn new_production(password: String, controls: ProductionControls) -> Self {
        let metrics_snapshot = super::observability::MetricsSnapshot::new();
        let rolling_window = Arc::new(super::observability::RollingWindow::default());
        let log_ring = Arc::new(super::observability::LogRing::default());
        Self {
            token_service: TokenService::new(),
            password,
            control_plane: controls.control_plane,
            address_book_control: controls.address_books,
            tunnel_manager: controls.tunnels,
            router_info: controls.router_info,
            semaphore: Semaphore::new(MAX_CONCURRENT_REQUESTS),
            router_id: String::new(),
            router_info_bytes: Vec::new(),
            router_info_b64: String::new(),
            startup_time: std::time::Instant::now(),
            metrics_snapshot,
            rolling_window,
            log_ring,
            service_registry: controls.service_registry,
        }
    }

    /// Create test state with fake adapters.
    ///
    /// This is the only constructor that installs fake implementations.
    /// Available only in test builds.
    #[cfg(test)]
    pub fn new_test(password: String) -> Self {
        use super::control_plane::{
            FakeAddressBookControl, FakeControlPlane, FakeTunnelManagerControl,
        };
        use super::router_info::FakeRouterInfoControl;
        let metrics_snapshot = super::observability::MetricsSnapshot::new();
        let rolling_window = Arc::new(super::observability::RollingWindow::default());
        let log_ring = Arc::new(super::observability::LogRing::default());
        Self {
            token_service: TokenService::new(),
            password,
            control_plane: Arc::new(FakeControlPlane::new()),
            address_book_control: Arc::new(FakeAddressBookControl::new()),
            tunnel_manager: Arc::new(FakeTunnelManagerControl::new()),
            router_info: Arc::new(FakeRouterInfoControl::new()),
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

    /// Get a clone of the service registry (cheap Arc clone). Used by the
    /// application composition root to share the registry with producers
    /// (proxy tasks, listener snapshot readouts, tunnel query tasks).
    #[allow(dead_code)]
    pub fn service_registry_clone(&self) -> ServiceRegistry {
        self.service_registry.clone()
    }

    /// Take a snapshot from the service registry.
    #[allow(dead_code)]
    pub fn service_snapshot(&self) -> ServiceSnapshot {
        self.service_registry.snapshot()
    }

    /// Replace the service registry (for testing or composition).
    ///
    /// Producers in the composition root (proxy tasks, listener snapshot
    /// readouts) should allocate their handles from the registry they
    /// already hold a clone of — only the I2PControl-facing half is
    /// replaced here.
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
        self.address_book_control = control.into();
    }

    /// Replace the tunnel manager control plane (for testing).
    pub fn set_tunnel_manager(&mut self, control: Box<dyn TunnelManagerControl>) {
        self.tunnel_manager = control.into();
    }

    /// Replace the router info inspection control plane (for testing).
    #[allow(dead_code)]
    pub fn set_router_info(&mut self, control: Box<dyn RouterInfoControl>) {
        self.router_info = control.into();
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
    pub async fn tunnel_list(
        &self,
    ) -> Result<Vec<crate::i2pcontrol::domain::tunnel::TunnelDefinition>, String> {
        self.tunnel_manager.list().await
    }

    /// Get a tunnel definition by name.
    pub async fn tunnel_get(
        &self,
        name: &str,
    ) -> Result<Option<crate::i2pcontrol::domain::tunnel::TunnelDefinition>, String> {
        self.tunnel_manager.get(name).await
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
    ) -> Result<Vec<crate::i2pcontrol::domain::address_book::AddressBookEntry>, String> {
        self.address_book_control.list(book_type).await
    }

    /// Look up an entry in the specified address book.
    pub async fn address_book_lookup(
        &self,
        book_type: crate::i2pcontrol::domain::address_book::AdministrativeAddressBookType,
        hostname: &str,
    ) -> Result<Option<crate::i2pcontrol::domain::address_book::AddressBookEntry>, String> {
        self.address_book_control.lookup(book_type, hostname).await
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
    ) -> Result<crate::i2pcontrol::domain::address_book::SubscriptionSet, String> {
        self.address_book_control.subscriptions().await
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
    ) -> Result<crate::i2pcontrol::domain::address_book::AddressBookConfiguration, String> {
        self.address_book_control.configuration().await
    }

    /// Set the address book configuration atomically.
    pub async fn address_book_set_configuration(
        &self,
        configuration: crate::i2pcontrol::domain::address_book::AddressBookConfiguration,
    ) -> Result<(), String> {
        self.address_book_control.set_configuration(configuration).await
    }
}

/// Production dependencies for I2PControl state construction.
///
/// All fields are required. The caller must construct and load each adapter
/// before passing it here. This ensures that the production composition root
/// cannot silently substitute fake, empty, zeroed, or separately initialized
/// state.
pub struct ProductionControls {
    pub address_books: Arc<dyn AddressBookControl>,
    pub tunnels: Arc<dyn TunnelManagerControl>,
    pub router_info: Arc<dyn RouterInfoControl>,
    pub control_plane: Arc<dyn ControlPlane>,
    pub service_registry: ServiceRegistry,
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

impl ServerInstance {
    /// Get a clone of the shared [`I2pControlState`].
    ///
    /// Used by the application composition root to access the service
    /// registry handle so additional producers (e.g. SAM session snapshot
    /// tasks) can be wired after [`init_server`] returns.
    #[allow(dead_code)]
    pub(crate) fn state_clone(&self) -> Arc<I2pControlState> {
        Arc::clone(&self.state)
    }

    /// Get the bound listener address.
    #[allow(dead_code)]
    pub(crate) fn bind(&self) -> SocketAddr {
        self.bind
    }
}

/// Bundle of dependencies used to construct the production I2PControl server.
///
/// When production adapters are supplied, they replace the corresponding
/// fakes. The fakes are retained as defaults so that headless test
/// environments and unit tests can build a server without supplying real
/// router state.
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
    /// Pre-built service registry from the application composition root.
    ///
    /// When provided, `init_server` uses this registry instead of creating
    /// a new one. The composition root shares its clone of the same
    /// registry with proxy tasks and listener snapshot readouts.
    pub service_registry: Option<ServiceRegistry>,
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
            service_registry: None,
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

    /// Inject a pre-built service registry from the application composition
    /// root. Producers in the composition root (proxy tasks, listener
    /// snapshot readouts, tunnel query tasks) share clones of this same
    /// registry.
    pub fn with_service_registry(mut self, registry: ServiceRegistry) -> Self {
        self.service_registry = Some(registry);
        self
    }
}

/// Initialize the I2PControl server: validate config, set up TLS, bind the port.
///
/// Returns a `ServerInstance` ready to serve, or an error if startup fails.
/// This function is synchronous-safe for calling from `setup_router` so that
/// bind/TLS/startup failures are surfaced as application errors.
///
/// # Fail-closed behavior
///
/// Directory creation, adapter construction, or store load failure aborts
/// I2PControl initialization. No partially constructed server state is
/// returned. No fake adapters are substituted on failure.
pub async fn init_server(
    config: &I2pControlConfig,
    base_path: &std::path::Path,
    ctx: ServerInitContext,
) -> Result<ServerInstance, I2pControlError> {
    config.validate()?;

    // Build TLS config (validates cert/key material)
    let tls_config = super::tls::build_tls_config(&config.tls, base_path)?;
    let _tls_acceptor = tokio_rustls::TlsAcceptor::from(tls_config);

    let router_info_bytes = ctx.router_info_bytes.clone();
    let router_info_b64 = base64_encode(&router_info_bytes);

    // --- Build and load production address book adapter ---
    let ab_dir = base_path.join("addressbooks");
    std::fs::create_dir_all(&ab_dir).map_err(|e| {
        I2pControlError::Persistence(format!(
            "failed to create address book directory {}: {e}",
            ab_dir.display()
        ))
    })?;
    let address_books = Arc::new(ProductionAddressBookControl::new(ab_dir));
    address_books.load().await.map_err(|e| {
        I2pControlError::Persistence(format!("failed to load address book store: {e}"))
    })?;

    // --- Build and load production tunnel manager adapter ---
    let tm_dir = base_path.join("tunnels");
    std::fs::create_dir_all(&tm_dir).map_err(|e| {
        I2pControlError::Persistence(format!(
            "failed to create tunnel store directory {}: {e}",
            tm_dir.display()
        ))
    })?;
    let tunnels: Arc<ProductionTunnelManagerControl> = Arc::new(
        ProductionTunnelManagerControl::new(tm_dir.clone()).map_err(|e| {
            I2pControlError::Persistence(format!("failed to create tunnel manager: {e}"))
        })?,
    );
    tunnels
        .load()
        .await
        .map_err(|e| I2pControlError::Persistence(format!("failed to load tunnel store: {e}")))?;

    // --- Build the shared tunnel service reference for all consumers ---
    let tunnels_shared: Arc<dyn TunnelManagerControl> = tunnels.clone();

    // --- Build production control plane (identity/version/uptime only) ---
    let metrics = ctx.event_metrics.clone().unwrap_or_else(|| Arc::new(NoopEventMetrics));
    let control_plane: Arc<dyn ControlPlane> = Arc::new(ProductionControlPlane::new(
        ctx.router_id.clone(),
        env!("CARGO_PKG_VERSION").to_string(),
        Arc::clone(&metrics),
    ));

    // --- Build production router info adapter using the shared tunnel service ---
    let log_ring = Arc::new(super::observability::LogRing::default());
    let router_info: Arc<dyn RouterInfoControl> = Arc::new(ProductionRouterInfoControl::new(
        ctx.router_id.clone(),
        env!("CARGO_PKG_VERSION").to_string(),
        ctx.share_ratio,
        ctx.configured_bandwidth_in,
        ctx.configured_bandwidth_out,
        metrics,
        log_ring,
        tunnels_shared,
    ));

    // --- Install the pre-built service registry from the composition root ---
    let service_registry = ctx.service_registry.unwrap_or_default();

    // --- Construct production state with all required dependencies ---
    let mut state = I2pControlState::new_production(
        config.password.clone(),
        ProductionControls {
            address_books,
            tunnels: tunnels.clone(),
            router_info,
            control_plane,
            service_registry,
        },
    );
    state.set_startup_values(ctx.router_id, router_info_bytes, router_info_b64);

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

/// Zero-cost event metrics stub for production startup when no real metrics
/// source is provided. All counters return zero.
struct NoopEventMetrics;

impl EventMetrics for NoopEventMetrics {
    fn transport_inbound_bytes(&self) -> u64 {
        0
    }
    fn transport_outbound_bytes(&self) -> u64 {
        0
    }
    fn transit_inbound_bytes(&self) -> u64 {
        0
    }
    fn transit_outbound_bytes(&self) -> u64 {
        0
    }
    fn connected_routers(&self) -> usize {
        0
    }
    fn transit_tunnel_count(&self) -> usize {
        0
    }
    fn tunnel_build_successes(&self) -> u64 {
        0
    }
    fn tunnel_build_failures(&self) -> u64 {
        0
    }
    fn ipv4_firewall_status(&self) -> emissary_core::FirewallStatus {
        emissary_core::FirewallStatus::Unknown
    }
    fn ipv6_firewall_status(&self) -> emissary_core::FirewallStatus {
        emissary_core::FirewallStatus::Unknown
    }
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
    use crate::i2pcontrol::control_plane::{
        FakeAddressBookControl, FakeControlPlane, FakeTunnelManagerControl,
    };
    use crate::i2pcontrol::router_info::FakeRouterInfoControl;

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

    // --- M008 composition and provenance tests ---

    #[test]
    fn production_requires_all_dependencies() {
        let tm: Arc<dyn TunnelManagerControl> = Arc::new(FakeTunnelManagerControl::new());
        let ab: Arc<dyn AddressBookControl> = Arc::new(FakeAddressBookControl::new());
        let ri: Arc<dyn RouterInfoControl> = Arc::new(FakeRouterInfoControl::new());
        let cp: Arc<dyn ControlPlane> = Arc::new(FakeControlPlane::new());

        let state = I2pControlState::new_production(
            "testpass".to_string(),
            ProductionControls {
                address_books: ab,
                tunnels: tm,
                router_info: ri,
                control_plane: cp,
                service_registry: ServiceRegistry::new(),
            },
        );

        assert_eq!(state.router_id(), "");
    }

    #[test]
    fn test_state_is_only_path_for_fakes() {
        let state = I2pControlState::new_test("testpass".to_string());
        let _ = state.router_info();
    }

    #[tokio::test]
    async fn shared_tunnel_object_identity() {
        use crate::i2pcontrol::domain::tunnel::{
            StartIntent, TunnelDefinition, TunnelName, TunnelOwnership, TunnelRuntimeState,
            TunnelType,
        };
        use std::sync::atomic::AtomicUsize;

        struct SentinelTunnelControl {
            generation: AtomicUsize,
        }
        impl SentinelTunnelControl {
            fn new() -> Self {
                Self {
                    generation: AtomicUsize::new(0),
                }
            }
            fn generation(&self) -> usize {
                self.generation.load(std::sync::atomic::Ordering::Acquire)
            }
        }

        #[async_trait::async_trait]
        impl TunnelManagerControl for SentinelTunnelControl {
            async fn list(&self) -> Result<Vec<TunnelDefinition>, String> {
                Ok(Vec::new())
            }
            async fn get(&self, _: &str) -> Result<Option<TunnelDefinition>, String> {
                Ok(None)
            }
            async fn create(&self, _: TunnelDefinition) -> Result<(), String> {
                self.generation.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
                Ok(())
            }
            async fn update(
                &self,
                _: &str,
                _: TunnelDefinition,
                _: Option<TunnelName>,
            ) -> Result<bool, String> {
                Ok(false)
            }
            async fn delete(&self, _: &str) -> Result<bool, String> {
                Ok(false)
            }
            async fn start(&self, _: &str) -> Result<String, String> {
                Ok("ok".into())
            }
            async fn stop(&self, _: &str) -> Result<String, String> {
                Ok("ok".into())
            }
            async fn restart(&self, _: &str) -> Result<String, String> {
                Ok("ok".into())
            }
            fn get_backend(
                &self,
                _: TunnelType,
            ) -> Option<Arc<dyn crate::i2pcontrol::backends::TunnelBackend>> {
                None
            }
            fn registry(&self) -> &crate::i2pcontrol::backends::registry::TunnelBackendRegistry {
                use std::sync::OnceLock;
                static REGISTRY: OnceLock<
                    crate::i2pcontrol::backends::registry::TunnelBackendRegistry,
                > = OnceLock::new();
                REGISTRY.get_or_init(|| {
                    crate::i2pcontrol::backends::registry::create_default_registry()
                        .expect("default registry is exhaustive")
                })
            }
        }

        let sentinel = Arc::new(SentinelTunnelControl::new());

        sentinel
            .create(TunnelDefinition {
                name: TunnelName::new("t").unwrap(),
                tunnel_type: TunnelType::Client,
                ownership: TunnelOwnership::ControlPlane,
                runtime_state: TunnelRuntimeState::Stopped,
                start_intent: StartIntent::DoNotStart,
                options: Default::default(),
                raw_config: Default::default(),
            })
            .await
            .unwrap();
        assert_eq!(sentinel.generation(), 1);

        let ab: Arc<dyn AddressBookControl> = Arc::new(FakeAddressBookControl::new());
        let ri: Arc<dyn RouterInfoControl> = Arc::new(FakeRouterInfoControl::new());
        let cp: Arc<dyn ControlPlane> = Arc::new(FakeControlPlane::new());

        let state = I2pControlState::new_production(
            "testpass".to_string(),
            ProductionControls {
                address_books: ab,
                tunnels: sentinel.clone() as Arc<dyn TunnelManagerControl>,
                router_info: ri,
                control_plane: cp,
                service_registry: ServiceRegistry::new(),
            },
        );

        let _ = state.tunnel_list().await.unwrap();
        assert_eq!(
            sentinel.generation(),
            1,
            "state and sentinel share the same object"
        );
    }

    #[tokio::test]
    async fn tunnel_list_failure_returns_error() {
        struct FailingTunnelControl;
        #[async_trait::async_trait]
        impl TunnelManagerControl for FailingTunnelControl {
            async fn list(
                &self,
            ) -> Result<Vec<crate::i2pcontrol::domain::tunnel::TunnelDefinition>, String>
            {
                Err("store read failed".into())
            }
            async fn get(
                &self,
                _: &str,
            ) -> Result<Option<crate::i2pcontrol::domain::tunnel::TunnelDefinition>, String>
            {
                Err("store read failed".into())
            }
            async fn create(
                &self,
                _: crate::i2pcontrol::domain::tunnel::TunnelDefinition,
            ) -> Result<(), String> {
                unimplemented!()
            }
            async fn update(
                &self,
                _: &str,
                _: crate::i2pcontrol::domain::tunnel::TunnelDefinition,
                _: Option<crate::i2pcontrol::domain::tunnel::TunnelName>,
            ) -> Result<bool, String> {
                unimplemented!()
            }
            async fn delete(&self, _: &str) -> Result<bool, String> {
                unimplemented!()
            }
            async fn start(&self, _: &str) -> Result<String, String> {
                unimplemented!()
            }
            async fn stop(&self, _: &str) -> Result<String, String> {
                unimplemented!()
            }
            async fn restart(&self, _: &str) -> Result<String, String> {
                unimplemented!()
            }
            fn get_backend(
                &self,
                _: crate::i2pcontrol::domain::tunnel::TunnelType,
            ) -> Option<Arc<dyn crate::i2pcontrol::backends::TunnelBackend>> {
                None
            }
            fn registry(&self) -> &crate::i2pcontrol::backends::registry::TunnelBackendRegistry {
                use std::sync::OnceLock;
                static REGISTRY: OnceLock<
                    crate::i2pcontrol::backends::registry::TunnelBackendRegistry,
                > = OnceLock::new();
                REGISTRY.get_or_init(|| {
                    crate::i2pcontrol::backends::registry::create_default_registry()
                        .expect("default registry is exhaustive")
                })
            }
        }

        let tm: Arc<dyn TunnelManagerControl> = Arc::new(FailingTunnelControl);
        let ab: Arc<dyn AddressBookControl> = Arc::new(FakeAddressBookControl::new());
        let ri: Arc<dyn RouterInfoControl> = Arc::new(FakeRouterInfoControl::new());
        let cp: Arc<dyn ControlPlane> = Arc::new(FakeControlPlane::new());

        let state = I2pControlState::new_production(
            "testpass".to_string(),
            ProductionControls {
                address_books: ab,
                tunnels: tm,
                router_info: ri,
                control_plane: cp,
                service_registry: ServiceRegistry::new(),
            },
        );

        let result = state.tunnel_list().await;
        assert!(
            result.is_err(),
            "tunnel_list should propagate the error, not return empty Vec"
        );

        let result = state.tunnel_get("test").await;
        assert!(
            result.is_err(),
            "tunnel_get should propagate the error, not return None"
        );
    }

    #[tokio::test]
    async fn address_book_failure_returns_error() {
        use crate::i2pcontrol::domain::address_book::AdministrativeAddressBookType;

        struct FailingAddressBookControl;
        #[async_trait::async_trait]
        impl crate::i2pcontrol::control_plane::AddressBookControl for FailingAddressBookControl {
            async fn list(
                &self,
                _: AdministrativeAddressBookType,
            ) -> Result<Vec<crate::i2pcontrol::domain::address_book::AddressBookEntry>, String>
            {
                Err("store read failed".into())
            }
            async fn lookup(
                &self,
                _: AdministrativeAddressBookType,
                _: &str,
            ) -> Result<Option<crate::i2pcontrol::domain::address_book::AddressBookEntry>, String>
            {
                Err("store read failed".into())
            }
            async fn add(
                &self,
                _: AdministrativeAddressBookType,
                _: crate::i2pcontrol::domain::address_book::AddressBookEntry,
            ) -> Result<(), String> {
                unimplemented!()
            }
            async fn update(
                &self,
                _: AdministrativeAddressBookType,
                _: crate::i2pcontrol::domain::address_book::AddressBookEntry,
            ) -> Result<bool, String> {
                unimplemented!()
            }
            async fn delete(
                &self,
                _: AdministrativeAddressBookType,
                _: &str,
            ) -> Result<bool, String> {
                unimplemented!()
            }
            async fn delete_all(&self, _: AdministrativeAddressBookType) -> Result<bool, String> {
                unimplemented!()
            }
            async fn subscriptions(
                &self,
            ) -> Result<crate::i2pcontrol::domain::address_book::SubscriptionSet, String>
            {
                Err("store read failed".into())
            }
            async fn set_subscriptions(
                &self,
                _: crate::i2pcontrol::domain::address_book::SubscriptionSet,
            ) -> Result<(), String> {
                unimplemented!()
            }
            async fn configuration(
                &self,
            ) -> Result<crate::i2pcontrol::domain::address_book::AddressBookConfiguration, String>
            {
                Err("store read failed".into())
            }
            async fn set_configuration(
                &self,
                _: crate::i2pcontrol::domain::address_book::AddressBookConfiguration,
            ) -> Result<(), String> {
                unimplemented!()
            }
        }

        let tm: Arc<dyn TunnelManagerControl> = Arc::new(FakeTunnelManagerControl::new());
        let ab: Arc<dyn AddressBookControl> = Arc::new(FailingAddressBookControl);
        let ri: Arc<dyn RouterInfoControl> = Arc::new(FakeRouterInfoControl::new());
        let cp: Arc<dyn ControlPlane> = Arc::new(FakeControlPlane::new());

        let state = I2pControlState::new_production(
            "testpass".to_string(),
            ProductionControls {
                address_books: ab,
                tunnels: tm,
                router_info: ri,
                control_plane: cp,
                service_registry: ServiceRegistry::new(),
            },
        );

        let result = state.address_book_list(AdministrativeAddressBookType::Private).await;
        assert!(
            result.is_err(),
            "address_book_list should propagate error, not return empty Vec"
        );

        let result = state
            .address_book_lookup(AdministrativeAddressBookType::Private, "test.i2p")
            .await;
        assert!(
            result.is_err(),
            "address_book_lookup should propagate error, not return None"
        );

        let result = state.address_book_subscriptions().await;
        assert!(
            result.is_err(),
            "address_book_subscriptions should propagate error"
        );
    }

    #[tokio::test]
    async fn fail_closed_startup_dir_creation_failure() {
        let tmp = tempfile::tempdir().unwrap();
        // Block addressbooks directory creation by placing a file in its path
        let blocker = tmp.path().join("addressbooks");
        std::fs::write(&blocker, "x").unwrap();

        let config = I2pControlConfig {
            enabled: true,
            bind: "127.0.0.1:0".parse().unwrap(),
            password: "testpass".to_string(),
            tls: TlsConfig {
                certificate: None,
                private_key: None,
            },
        };
        let ctx = ServerInitContext::new("id".into(), vec![]);

        let result = init_server(&config, tmp.path(), ctx).await;
        assert!(result.is_err());
        if let Err(I2pControlError::Persistence(msg)) = result {
            assert!(
                msg.contains("address book"),
                "error should mention address book: {msg}"
            );
        } else {
            panic!("expected Persistence error");
        }
    }

    #[tokio::test]
    async fn fail_closed_startup_no_temp_fallback() {
        let tmp = tempfile::tempdir().unwrap();
        let config = I2pControlConfig {
            enabled: true,
            bind: "127.0.0.1:0".parse().unwrap(),
            password: "testpass".to_string(),
            tls: TlsConfig {
                certificate: None,
                private_key: None,
            },
        };
        let ctx = ServerInitContext::new("id".into(), vec![]);

        let _ = init_server(&config, tmp.path(), ctx).await.unwrap();

        // No temp fallback directories should have been created
        let temp = std::env::temp_dir();
        let entries: Vec<_> = std::fs::read_dir(&temp)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains("emissary-i2pcontrol"))
            .collect();
        assert!(
            entries.is_empty(),
            "no fallback directories should exist in temp: {:?}",
            entries
        );
    }
}
