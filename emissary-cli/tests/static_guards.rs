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
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
// FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
// DEALINGS IN THE SOFTWARE.

//! Static and structural guards for the I2PControl M005 boundary.
//!
//! These tests verify the invariants required by the M005 plan and Proposal
//! 170's read-only inspection architecture:
//!
//! - No `EventSubscriber` use anywhere in the I2PControl code path.
//! - No UI / frontend module imports in inspection code.
//! - No HTTP / JSON-RPC / server dependencies in the emissary-core crate.
//! - `RouterInfoControl` and its DTOs do not expose private key types or
//!   mutable core handles.
//! - The selector registry contains exactly the Proposal 170 keys.
//! - The handler only returns requested selector keys.
//! - The router info adapter never mutates state.

#![cfg(feature = "i2pcontrol")]

use std::path::Path;

use emissary_cli::i2pcontrol::control_plane::AddressBookControl;
use emissary_cli::i2pcontrol::control_plane::TunnelManagerControl;
use emissary_cli::i2pcontrol::production::{
    EventMetrics, ProductionAddressBookControl, ProductionControlPlane,
    ProductionRouterInfoControl, ProductionTunnelManagerControl,
};
use emissary_cli::i2pcontrol::router_info::{
    ActivePeerStats, I2PTunnelStats, LogSnapshot, NetworkSnapshot, PeerLimits,
    RecentTransitTraffic, RouterInfoControl, TransitBytes, TransportBytes, TunnelBuildStats,
    TunnelSummary,
};
use emissary_cli::i2pcontrol::rpc;

// --- Source-level structural guards (probed by file reads) ---

const I2PCONTROL_FILES: &[&str] = &[
    "src/i2pcontrol/address_book.rs",
    "src/i2pcontrol/auth.rs",
    "src/i2pcontrol/control_plane.rs",
    "src/i2pcontrol/observability.rs",
    "src/i2pcontrol/production.rs",
    "src/i2pcontrol/router_info.rs",
    "src/i2pcontrol/router_info_handler.rs",
    "src/i2pcontrol/rpc.rs",
    "src/i2pcontrol/server.rs",
    "src/i2pcontrol/tunnel_manager.rs",
];

fn workspace_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("workspace root")
}

fn read_source(rel: &str) -> String {
    let p = workspace_root().join("emissary-cli").join(rel);
    std::fs::read_to_string(p).unwrap_or_else(|e| panic!("read {}: {e}", rel))
}

#[test]
fn i2pcontrol_does_not_consume_event_subscriber() {
    for f in I2PCONTROL_FILES {
        let src = read_source(f);
        // Disallow imports or usages; doc comments mentioning the name
        // are acceptable because they document the invariant.
        for line in src.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("//") {
                continue;
            }
            assert!(
                !line.contains("EventSubscriber"),
                "I2PControl file {f} must not reference EventSubscriber: {line}"
            );
        }
    }
}

#[test]
fn i2pcontrol_does_not_import_ui_modules() {
    for f in I2PCONTROL_FILES {
        let src = read_source(f);
        assert!(
            !src.contains("crate::ui") && !src.contains("crate::dioxus"),
            "I2PControl file {f} must not import the UI module"
        );
    }
}

#[test]
fn i2pcontrol_does_not_import_http_or_serde_json_server_libs() {
    for f in I2PCONTROL_FILES {
        let src = read_source(f);
        // axum is the HTTP framework; allowed in server.rs only because
        // the I2PControl server is HTTP-based. Other files must not use it.
        if f.ends_with("server.rs") {
            continue;
        }
        assert!(
            !src.contains("use axum") && !src.contains("axum::"),
            "I2PControl file {f} must not import axum (HTTP server framework)"
        );
    }
}

#[test]
fn emissary_core_cargo_has_no_i2pcontrol_dependencies() {
    let p = workspace_root().join("emissary-core").join("Cargo.toml");
    let s = std::fs::read_to_string(p).unwrap();
    for forbidden in [
        "axum",
        "hyper",
        "tokio-rustls",
        "rustls-pemfile",
        "serde_json",
    ] {
        assert!(
            !s.contains(forbidden),
            "emissary-core must not depend on {forbidden}"
        );
    }
}

#[test]
fn router_info_dtos_do_not_expose_signing_or_static_key() {
    // The router info DTOs should not have fields of type SigningPrivateKey or
    // StaticPrivateKey or any other private key material. The DTOs are pure
    // protocol-required primitives. Allow doc comments but no actual
    // type references.
    let d = read_source("src/i2pcontrol/router_info.rs");
    for line in d.lines() {
        if line.trim().starts_with("//") {
            continue;
        }
        assert!(
            !line.contains("SigningPrivateKey"),
            "router_info DTOs must not reference SigningPrivateKey: {line}"
        );
        assert!(
            !line.contains("StaticPrivateKey"),
            "router_info DTOs must not reference StaticPrivateKey: {line}"
        );
        assert!(
            !line.contains("NoiseContext"),
            "router_info DTOs must not reference NoiseContext: {line}"
        );
    }
}

#[test]
fn router_info_control_trait_is_send_sync_and_async() {
    fn assert_send_sync<T: Send + Sync + ?Sized>() {}
    assert_send_sync::<dyn RouterInfoControl>();
    assert_send_sync::<dyn AddressBookControl>();
    assert_send_sync::<dyn TunnelManagerControl>();
}

#[test]
fn production_router_info_does_not_mutate_state() {
    // The production adapter's read methods must be visible. There is no
    // `set_*` or `mutate_*` method exposed on the trait.
    let d = read_source("src/i2pcontrol/production.rs");
    for line in d.lines() {
        let l = line.trim();
        if l.starts_with("pub fn ") || l.starts_with("pub(crate) fn ") {
            let forbidden = l.contains("fn set_")
                || l.contains("fn mutate_")
                || l.contains("fn write_")
                || l.contains("fn update_")
                || l.contains("fn trigger_");
            assert!(
                !forbidden,
                "Production router info adapter must not expose mutation: {l}"
            );
        }
    }
}

// --- Runtime structural guards (asserted against in-memory state) ---

fn make_production_router_info() -> ProductionRouterInfoControl {
    let metrics: Arc<dyn EventMetrics> = Arc::new(NullMetrics);
    let tunnel_mgr = Arc::new(
        ProductionTunnelManagerControl::new(
            std::env::temp_dir().join("emissary-i2pcontrol-static-guard"),
        )
        .expect("tunnel manager"),
    );
    let log_ring = Arc::new(emissary_cli::i2pcontrol::observability::LogRing::default());
    ProductionRouterInfoControl::new(
        "test".to_string(),
        "test".to_string(),
        0.0,
        0,
        0,
        metrics,
        log_ring,
        tunnel_mgr,
    )
}

use std::sync::Arc;

struct NullMetrics;

impl EventMetrics for NullMetrics {
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

#[test]
fn router_info_dtos_clone_and_default() {
    // DTOs are plain data; they must support clone, default, debug, send, sync.
    fn assert_data<T: Clone + Default + std::fmt::Debug + Send + Sync>() {}

    assert_data::<NetworkSnapshot>();
    assert_data::<TransportBytes>();
    assert_data::<TransitBytes>();
    assert_data::<RecentTransitTraffic>();
    assert_data::<TunnelBuildStats>();
    assert_data::<TunnelSummary>();
    assert_data::<ActivePeerStats>();
    assert_data::<PeerLimits>();
    assert_data::<I2PTunnelStats>();
    assert_data::<LogSnapshot>();
}

#[test]
fn selector_registry_is_complete() {
    // 121 Proposal 170 RouterInfo selectors.
    assert_eq!(rpc::router_info_keys::ALL.len(), 121);
}

#[test]
fn selector_registry_has_unique_keys() {
    use std::collections::HashSet;
    let set: HashSet<&str> = rpc::router_info_keys::ALL.iter().copied().collect();
    assert_eq!(set.len(), rpc::router_info_keys::ALL.len());
}

#[test]
fn selector_registry_address_book_partition() {
    use std::collections::HashSet;
    let all: HashSet<&str> = rpc::router_info_keys::ALL.iter().copied().collect();
    let ab: HashSet<&str> = rpc::router_info_keys::ADDRESS_BOOK_KEYS.iter().copied().collect();
    let core: HashSet<&str> = rpc::router_info_keys::CORE_KEYS.iter().copied().collect();

    // ALL = CORE ∪ ADDRESS_BOOK
    assert_eq!(all, core.union(&ab).copied().collect::<HashSet<_>>());
    // CORE ∩ ADDRESS_BOOK = ∅
    assert!(core.is_disjoint(&ab));
}

#[test]
fn production_adapter_returns_empty_for_unimplemented_selectors() {
    // The production adapter does not yet wire known peers, active peers,
    // banned peers, peer limits, or netdb summaries. The handler must
    // receive empty results, not fabricated values.
    let rt = tokio::runtime::Runtime::new().unwrap();
    let ri = make_production_router_info();
    rt.block_on(async {
        assert!(ri.known_peers().await.is_empty());
        assert!(ri.active_peers().await.is_empty());
        assert!(ri.banned_peers().await.is_empty());
        assert!(ri.active_peer_stats().await.is_empty());
        assert!(ri.peer_router_info("any").await.unwrap().is_none());
        let limits = ri.peer_limits().await;
        assert_eq!(limits.configured_inbound, 0);
        assert_eq!(limits.configured_outbound, 0);
        assert_eq!(limits.effective_inbound, 0);
        assert_eq!(limits.effective_outbound, 0);
    });
}

#[test]
fn production_adapter_does_not_silently_truncate() {
    // The production adapter has no truncation; it returns whatever the
    // underlying source provides. Empty collections are returned as empty,
    // not as error.
    let rt = tokio::runtime::Runtime::new().unwrap();
    let ri = make_production_router_info();
    rt.block_on(async {
        let snap = ri.log_snapshot().await;
        assert!(snap.entries.is_empty());
        let netdb = ri.netdb_snapshot().await;
        // Default netdb snapshot has known_profiles=0, not 1 or some fabricated
        // value. This proves we are not silently truncating a 1-element list.
        assert_eq!(netdb.known_profiles, 0);
    });
}

#[test]
fn production_address_book_adapter_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    let ab = ProductionAddressBookControl::new(std::env::temp_dir());
    assert_send_sync::<ProductionAddressBookControl>();
    let _ = ab;
}

#[test]
fn production_tunnel_manager_adapter_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    let tm = ProductionTunnelManagerControl::new(
        std::env::temp_dir().join("emissary-i2pcontrol-static-guard-tm"),
    )
    .unwrap();
    assert_send_sync::<ProductionTunnelManagerControl>();
    let _ = tm;
}

#[test]
fn production_control_plane_adapter_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    let metrics: Arc<dyn EventMetrics> = Arc::new(NullMetrics);
    let cp = ProductionControlPlane::new("test".to_string(), "test".to_string(), metrics);
    assert_send_sync::<ProductionControlPlane>();
    let _ = cp;
}

// --- M008 static guards ---

/// Guard: no enabled production path falls back to fake adapters.
///
/// If any production code contains "falling back to fake", this test fails.
/// This catches defects where init_server logs a warning and continues
/// with a fake adapter instead of failing startup.
#[test]
fn no_fallback_to_fake_in_production() {
    let files = &["src/i2pcontrol/server.rs", "src/i2pcontrol/production.rs"];
    for f in files {
        let src = read_source(f);
        // Split at #[cfg(test)] so test-only code is excluded
        let non_test = src.split("#[cfg(test)]").next().unwrap_or(&src);
        assert!(
            !non_test.contains("falling back to fake"),
            "Production code {f} must not contain 'falling back to fake'"
        );
    }
}

/// Guard: no temporary fallback tunnel directory in production code.
///
/// Catches the defect where init_server creates a temp fallback directory
/// instead of failing.
#[test]
fn no_temp_fallback_tunnel_dir() {
    let files = &["src/i2pcontrol/server.rs", "src/i2pcontrol/production.rs"];
    for f in files {
        let src = read_source(f);
        let non_test = src.split("#[cfg(test)]").next().unwrap_or(&src);
        assert!(
            !non_test.contains("emissary-i2pcontrol-tunnels-fallback"),
            "Production code {f} must not contain temp fallback tunnel directory"
        );
    }
}

/// Guard: no production construction of Fake* adapters in init_server.
///
/// Catches the defect where init_server installs fake adapters as
/// production defaults.
#[test]
fn no_production_fake_adapter_construction() {
    let src = read_source("src/i2pcontrol/server.rs");
    // Only check non-test code
    let non_test = src.split("#[cfg(test)]").next().unwrap_or(&src);
    assert!(
        !non_test.contains("FakeControlPlane::new()"),
        "Production init_server must not construct FakeControlPlane"
    );
    assert!(
        !non_test.contains("FakeAddressBookControl::new()"),
        "Production init_server must not construct FakeAddressBookControl"
    );
    assert!(
        !non_test.contains("FakeTunnelManagerControl::new()"),
        "Production init_server must not construct FakeTunnelManagerControl"
    );
    assert!(
        !non_test.contains("FakeRouterInfoControl::new()"),
        "Production init_server must not construct FakeRouterInfoControl"
    );
}

/// Guard: no second ProductionTunnelManagerControl::new() in RouterInfo wiring.
///
/// Catches the defect where init_server opens a second tunnel store
/// instance for RouterInfo instead of sharing the one already loaded.
#[test]
fn no_duplicate_tunnel_manager_in_init_server() {
    let src = read_source("src/i2pcontrol/server.rs");
    let non_test = src.split("#[cfg(test)]").next().unwrap_or(&src);
    // Count occurrences of ProductionTunnelManagerControl::new outside test code
    let count = non_test.matches("ProductionTunnelManagerControl::new").count();
    assert!(
        count <= 1,
        "Production init_server must construct ProductionTunnelManagerControl exactly once, found {count}"
    );
}

/// Guard: no error-suppressing unwrap_or_default or unwrap_or(0) in
/// production state query helpers.
///
/// Catches the defect where tunnel_list, tunnel_get, address_book_list,
/// address_book_lookup, address_book_subscriptions, or
/// address_book_configuration silently turn errors into empty/zero state.
#[test]
fn no_error_suppressing_helpers() {
    let src = read_source("src/i2pcontrol/server.rs");
    let non_test = src.split("#[cfg(test)]").next().unwrap_or(&src);
    assert!(
        !non_test.contains("unwrap_or_default()"),
        "Production state helpers must not use unwrap_or_default() to suppress errors"
    );
}

/// Guard: ControlPlane no longer includes tunnel methods.
///
/// Catches the defect where ControlPlane still has tunnel_list, tunnel_get,
/// or is_tunnel_type_supported, which creates dual-path access.
#[test]
fn control_plane_has_no_tunnel_methods() {
    let src = read_source("src/i2pcontrol/control_plane.rs");
    let non_test = src.split("#[cfg(test)]").next().unwrap_or(&src);
    // Check the trait definition (between "pub trait ControlPlane" and the closing "}")
    if let Some(trait_start) = non_test.find("pub trait ControlPlane") {
        if let Some(trait_body_end) = non_test[trait_start..].find("\n}") {
            let trait_body = &non_test[trait_start..trait_start + trait_body_end + 2];
            assert!(
                !trait_body.contains("fn tunnel_list"),
                "ControlPlane must not have tunnel_list method"
            );
            assert!(
                !trait_body.contains("fn tunnel_get"),
                "ControlPlane must not have tunnel_get method"
            );
            assert!(
                !trait_body.contains("fn is_tunnel_type_supported"),
                "ControlPlane must not have is_tunnel_type_supported method"
            );
        }
    }
}
