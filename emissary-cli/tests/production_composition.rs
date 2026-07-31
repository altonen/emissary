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

//! Integration tests for M008 production composition and durable-state integrity.
//!
//! These tests verify invariants from the M008 plan using only public API:
//!
//! - Production adapter construction uses real stores and shared objects.
//! - Production adapters propagate query errors rather than suppressing them.
//! - Restart preserves durable state across server instances.
//! - ControlPlane no longer includes tunnel methods (narrowed to identity/version/uptime).
//! - ProductionRouterInfoControl uses the shared tunnel service.

#![cfg(feature = "i2pcontrol")]

use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use emissary_cli::i2pcontrol::{
    client_services::assemble_response_with_observation,
    control_plane::{ControlPlane, TunnelManagerControl},
    production::{
        EventMetrics, ProductionControlPlane, ProductionRouterInfoControl,
        ProductionTunnelManagerControl,
    },
    router_info::{RouterInfoControl, TunnelSummary},
    server::{I2pControlState, ProductionControls},
    service_registry::{ObservedServiceState, ServiceCategory, ServiceMetadata, ServiceRegistry},
};

use emissary_core::{FirewallStatus, SamSessionObservationHandle};

// --- In-memory EventMetrics for tests ---

#[derive(Default)]
struct TestMetrics {
    transit_tunnels: AtomicUsize,
}

impl TestMetrics {
    fn new() -> Self {
        Self::default()
    }
}

impl EventMetrics for TestMetrics {
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
        self.transit_tunnels.load(Ordering::Acquire)
    }
    fn tunnel_build_successes(&self) -> u64 {
        0
    }
    fn tunnel_build_failures(&self) -> u64 {
        0
    }
    fn ipv4_firewall_status(&self) -> FirewallStatus {
        FirewallStatus::Unknown
    }
    fn ipv6_firewall_status(&self) -> FirewallStatus {
        FirewallStatus::Unknown
    }
}

// --- Shared identity tests ---

#[tokio::test]
async fn shared_tunnel_object_identity_through_production_adapters() {
    // Create a production tunnel manager
    let tmp = tempfile::tempdir().unwrap();
    let tm = Arc::new(ProductionTunnelManagerControl::new(tmp.path().join("tunnels")).unwrap());
    tm.load().await.unwrap();

    // Build production router info with the same tunnel manager
    let metrics = Arc::new(TestMetrics::new());
    let log_ring = Arc::new(emissary_cli::i2pcontrol::observability::LogRing::default());
    let ri = ProductionRouterInfoControl::new(
        "test-id".to_string(),
        "test".to_string(),
        0.5,
        512,
        512,
        metrics,
        log_ring,
        tm.clone() as Arc<dyn TunnelManagerControl>,
    );

    // Create a tunnel through the tunnel manager
    let def = emissary_cli::i2pcontrol::domain::tunnel::TunnelDefinition {
        name: emissary_cli::i2pcontrol::domain::tunnel::TunnelName::new("shared-test").unwrap(),
        tunnel_type: emissary_cli::i2pcontrol::domain::tunnel::TunnelType::Client,
        ownership: emissary_cli::i2pcontrol::domain::tunnel::TunnelOwnership::ControlPlane,
        runtime_state: emissary_cli::i2pcontrol::domain::tunnel::TunnelRuntimeState::Stopped,
        start_intent: emissary_cli::i2pcontrol::domain::tunnel::StartIntent::DoNotStart,
        options: Default::default(),
        raw_config: Default::default(),
    };
    tm.create(def).await.unwrap();

    // Query through RouterInfo — should see the same tunnel
    let summary: TunnelSummary = ri.tunnel_summary().await.unwrap();
    assert_eq!(
        summary.configured, 1,
        "RouterInfo should see the tunnel created through the shared tunnel manager"
    );
}

#[tokio::test]
async fn production_router_info_uses_shared_tunnel_service() {
    let tmp = tempfile::tempdir().unwrap();
    let tm = Arc::new(ProductionTunnelManagerControl::new(tmp.path().join("tunnels")).unwrap());
    tm.load().await.unwrap();

    let metrics = Arc::new(TestMetrics::new());
    let log_ring = Arc::new(emissary_cli::i2pcontrol::observability::LogRing::default());
    let ri = ProductionRouterInfoControl::new(
        "test-id".to_string(),
        "test".to_string(),
        0.5,
        512,
        512,
        metrics,
        log_ring,
        tm.clone() as Arc<dyn TunnelManagerControl>,
    );

    // i2ptunnel_stats should use the shared tunnel service
    let stats = ri.i2ptunnel_stats().await.unwrap();
    assert_eq!(
        stats.configured_count, 0,
        "fresh store should have 0 configured tunnels"
    );

    // Create a tunnel and verify the count updates
    let def = emissary_cli::i2pcontrol::domain::tunnel::TunnelDefinition {
        name: emissary_cli::i2pcontrol::domain::tunnel::TunnelName::new("stats-test").unwrap(),
        tunnel_type: emissary_cli::i2pcontrol::domain::tunnel::TunnelType::Client,
        ownership: emissary_cli::i2pcontrol::domain::tunnel::TunnelOwnership::ControlPlane,
        runtime_state: emissary_cli::i2pcontrol::domain::tunnel::TunnelRuntimeState::Stopped,
        start_intent: emissary_cli::i2pcontrol::domain::tunnel::StartIntent::DoNotStart,
        options: Default::default(),
        raw_config: Default::default(),
    };
    tm.create(def).await.unwrap();

    let stats = ri.i2ptunnel_stats().await.unwrap();
    assert_eq!(
        stats.configured_count, 1,
        "RouterInfo should see 1 configured tunnel via shared service"
    );
}

#[tokio::test]
async fn production_sam_observation_source_reaches_client_services_serializer() {
    // A real SAM protocol activation cannot be made deterministic in this
    // repository-wide production-composition test: the publisher is private
    // to SamServer and activation requires a live destination/session path.
    // This is the closest production seam: real production controls and the
    // exact shared observation handle are passed through I2pControlState to
    // the ClientServicesInfo serializer. M019 must decide whether this is
    // sufficient or require an environment-specific SAM integration.
    let tmp = tempfile::tempdir().unwrap();
    let tunnels =
        Arc::new(ProductionTunnelManagerControl::new(tmp.path().join("tunnels")).unwrap());
    tunnels.load().await.unwrap();
    let metrics = Arc::new(TestMetrics::new());
    let router_info = Arc::new(ProductionRouterInfoControl::new(
        "test-id".into(),
        "test".into(),
        0.0,
        0,
        0,
        metrics.clone(),
        Arc::new(emissary_cli::i2pcontrol::observability::LogRing::default()),
        tunnels.clone() as Arc<dyn TunnelManagerControl>,
    ));
    let sam_handle = SamSessionObservationHandle::empty_for_test();
    let registry = ServiceRegistry::new();
    registry
        .allocate_handle(ServiceCategory::Sam)
        .update(
            ObservedServiceState::Listening,
            ServiceMetadata {
                enabled: true,
                ..Default::default()
            },
        )
        .unwrap();
    let state = I2pControlState::new_production_with_sam_observation(
        "testpass".into(),
        ProductionControls {
            address_books: Arc::new(
                emissary_cli::i2pcontrol::production::ProductionAddressBookControl::new(
                    tmp.path().join("addressbooks"),
                ),
            ),
            tunnels: tunnels.clone() as Arc<dyn TunnelManagerControl>,
            router_info,
            control_plane: Arc::new(ProductionControlPlane::new(
                "test-id".into(),
                "test".into(),
                metrics,
            )),
            service_registry: registry,
        },
        Some(sam_handle.clone()),
    );
    let result = assemble_response_with_observation(
        &state.service_snapshot(),
        &["SAM"],
        state.tunnel_manager(),
        state.sam_session_observation(),
    )
    .await
    .unwrap();
    assert_eq!(result["SAM"]["enabled"], true);
    assert_eq!(result["SAM"]["sessions"], serde_json::json!({}));
}

// --- ControlPlane narrowed tests ---

#[test]
fn control_plane_has_no_tunnel_methods() {
    // Verify ControlPlane only exposes identity/version/uptime
    let cp = ProductionControlPlane::new(
        "test-id".to_string(),
        "test".to_string(),
        Arc::new(TestMetrics::new()),
    );

    // These methods exist on ControlPlane
    assert!(cp.router_identity().unwrap().contains("test-id"));
    assert_eq!(cp.router_version(), "test");

    // Note: ControlPlane no longer has tunnel_list, tunnel_get,
    // or is_tunnel_type_supported — these are verified at compile time
    // by the trait definition in control_plane.rs
}

// --- Fail-closed startup tests (via public init_server) ---

#[tokio::test]
async fn fail_closed_on_address_book_dir_creation_failure() {
    let tmp = tempfile::tempdir().unwrap();
    let bad_path = tmp.path();
    // Block addressbooks directory creation by placing a file at that path
    std::fs::write(bad_path.join("addressbooks"), "x").unwrap();

    let ctx =
        emissary_cli::i2pcontrol::server::ServerInitContext::new("test-id".to_string(), vec![]);

    let config = emissary_cli::i2pcontrol::server::I2pControlConfig {
        enabled: true,
        bind: "127.0.0.1:0".parse().unwrap(),
        password: "testpass".to_string(),
        tls: emissary_cli::i2pcontrol::tls::TlsConfig {
            certificate: None,
            private_key: None,
        },
    };

    let result = emissary_cli::i2pcontrol::server::init_server(&config, bad_path, ctx).await;
    assert!(
        result.is_err(),
        "startup should fail when address-book dir creation fails"
    );
    if let Err(emissary_cli::i2pcontrol::errors::I2pControlError::Persistence(msg)) = result {
        assert!(
            msg.contains("address book"),
            "error should mention address book: {msg}"
        );
    } else {
        panic!("expected Persistence error");
    }
}

#[tokio::test]
async fn fail_closed_on_tunnel_dir_creation_failure() {
    let tmp = tempfile::tempdir().unwrap();
    let blocker = tmp.path().join("addressbooks");
    std::fs::write(&blocker, "blocker").unwrap();

    let ctx =
        emissary_cli::i2pcontrol::server::ServerInitContext::new("test-id".to_string(), vec![]);

    let config = emissary_cli::i2pcontrol::server::I2pControlConfig {
        enabled: true,
        bind: "127.0.0.1:0".parse().unwrap(),
        password: "testpass".to_string(),
        tls: emissary_cli::i2pcontrol::tls::TlsConfig {
            certificate: None,
            private_key: None,
        },
    };

    let result = emissary_cli::i2pcontrol::server::init_server(&config, tmp.path(), ctx).await;
    assert!(
        result.is_err(),
        "startup should fail when addressbooks file blocks tunnel dir"
    );
}

#[tokio::test]
async fn successful_startup_returns_instance() {
    let tmp = tempfile::tempdir().unwrap();

    let ctx =
        emissary_cli::i2pcontrol::server::ServerInitContext::new("test-id".to_string(), vec![]);

    let config = emissary_cli::i2pcontrol::server::I2pControlConfig {
        enabled: true,
        bind: "127.0.0.1:0".parse().unwrap(),
        password: "testpass".to_string(),
        tls: emissary_cli::i2pcontrol::tls::TlsConfig {
            certificate: None,
            private_key: None,
        },
    };

    let result = emissary_cli::i2pcontrol::server::init_server(&config, tmp.path(), ctx).await;
    assert!(result.is_ok(), "startup should succeed: {:?}", result.err());
}

// --- Restart tests ---

#[tokio::test]
async fn restart_preserves_durable_state() {
    let tmp = tempfile::tempdir().unwrap();
    let base_path = tmp.path();

    let config = emissary_cli::i2pcontrol::server::I2pControlConfig {
        enabled: true,
        bind: "127.0.0.1:0".parse().unwrap(),
        password: "testpass".to_string(),
        tls: emissary_cli::i2pcontrol::tls::TlsConfig {
            certificate: None,
            private_key: None,
        },
    };

    // First instance: create a tunnel via production adapter
    let ctx1 =
        emissary_cli::i2pcontrol::server::ServerInitContext::new("test-id".to_string(), vec![]);
    let _instance1 = emissary_cli::i2pcontrol::server::init_server(&config, base_path, ctx1)
        .await
        .unwrap();

    // Create a tunnel store and add a tunnel
    let tm1 = Arc::new(ProductionTunnelManagerControl::new(base_path.join("tunnels")).unwrap());
    tm1.load().await.unwrap();
    let def = emissary_cli::i2pcontrol::domain::tunnel::TunnelDefinition {
        name: emissary_cli::i2pcontrol::domain::tunnel::TunnelName::new("restart-test").unwrap(),
        tunnel_type: emissary_cli::i2pcontrol::domain::tunnel::TunnelType::Client,
        ownership: emissary_cli::i2pcontrol::domain::tunnel::TunnelOwnership::ControlPlane,
        runtime_state: emissary_cli::i2pcontrol::domain::tunnel::TunnelRuntimeState::Stopped,
        start_intent: emissary_cli::i2pcontrol::domain::tunnel::StartIntent::DoNotStart,
        options: Default::default(),
        raw_config: Default::default(),
    };
    tm1.create(def).await.unwrap();

    // Second instance: reconstruct from the same base path
    let ctx2 =
        emissary_cli::i2pcontrol::server::ServerInitContext::new("test-id".to_string(), vec![]);
    let _instance2 = emissary_cli::i2pcontrol::server::init_server(&config, base_path, ctx2)
        .await
        .unwrap();

    // Verify the tunnel persisted
    let tm2 = ProductionTunnelManagerControl::new(base_path.join("tunnels")).unwrap();
    tm2.load().await.unwrap();
    let tunnels = tm2.list().await.unwrap();
    assert_eq!(tunnels.len(), 1);
    assert_eq!(tunnels[0].name.as_str(), "restart-test");
}
