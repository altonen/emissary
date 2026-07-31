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

//! Integration tests for the production I2PControl adapters.
//!
//! These tests exercise the production code path with in-memory fakes that
//! satisfy the `EventMetrics` trait. The full `EventHandle<R>` is not used
//! here because the core mock runtime is gated by `cfg(test)` and not
//! available to downstream integration tests.

#![cfg(feature = "i2pcontrol")]

use std::sync::{
    atomic::{AtomicU64, AtomicUsize, Ordering},
    Arc,
};

use emissary_cli::i2pcontrol::{
    control_plane::ControlPlane,
    observability::{LogEntry, LogRing},
    production::{
        EventMetrics, ProductionAddressBookControl, ProductionControlPlane,
        ProductionRouterInfoControl, ProductionTunnelManagerControl,
    },
    router_info::{NetworkStatus, RouterInfoControl},
    stores::{address_book_store::AddressBookStore, tunnel_store::TunnelStore},
};

use emissary_core::FirewallStatus;

// --- In-memory EventMetrics for tests ---

#[derive(Default)]
struct InMemoryMetrics {
    transport_inbound: AtomicU64,
    transport_outbound: AtomicU64,
    transit_inbound: AtomicU64,
    transit_outbound: AtomicU64,
    connected_routers: AtomicUsize,
    transit_tunnels: AtomicUsize,
    build_successes: AtomicU64,
    build_failures: AtomicU64,
    ipv4_status: AtomicUsize,
    ipv6_status: AtomicUsize,
}

impl InMemoryMetrics {
    fn new() -> Self {
        Self::default()
    }

    fn set_firewall(&self, ipv4: FirewallStatus, ipv6: FirewallStatus) {
        self.ipv4_status.store(ipv4 as usize, Ordering::Release);
        self.ipv6_status.store(ipv6 as usize, Ordering::Release);
    }

    fn set_transport_bytes(&self, inbound: u64, outbound: u64) {
        self.transport_inbound.store(inbound, Ordering::Release);
        self.transport_outbound.store(outbound, Ordering::Release);
    }
}

impl EventMetrics for InMemoryMetrics {
    fn transport_inbound_bytes(&self) -> u64 {
        self.transport_inbound.load(Ordering::Acquire)
    }
    fn transport_outbound_bytes(&self) -> u64 {
        self.transport_outbound.load(Ordering::Acquire)
    }
    fn transit_inbound_bytes(&self) -> u64 {
        self.transit_inbound.load(Ordering::Acquire)
    }
    fn transit_outbound_bytes(&self) -> u64 {
        self.transit_outbound.load(Ordering::Acquire)
    }
    fn connected_routers(&self) -> usize {
        self.connected_routers.load(Ordering::Acquire)
    }
    fn transit_tunnel_count(&self) -> usize {
        self.transit_tunnels.load(Ordering::Acquire)
    }
    fn tunnel_build_successes(&self) -> u64 {
        self.build_successes.load(Ordering::Acquire)
    }
    fn tunnel_build_failures(&self) -> u64 {
        self.build_failures.load(Ordering::Acquire)
    }
    fn ipv4_firewall_status(&self) -> FirewallStatus {
        match self.ipv4_status.load(Ordering::Acquire) {
            1 => FirewallStatus::Firewalled,
            2 => FirewallStatus::Ok,
            3 => FirewallStatus::SymmetricNat,
            _ => FirewallStatus::Unknown,
        }
    }
    fn ipv6_firewall_status(&self) -> FirewallStatus {
        match self.ipv6_status.load(Ordering::Acquire) {
            1 => FirewallStatus::Firewalled,
            2 => FirewallStatus::Ok,
            3 => FirewallStatus::SymmetricNat,
            _ => FirewallStatus::Unknown,
        }
    }
}

fn make_metrics() -> Arc<InMemoryMetrics> {
    Arc::new(InMemoryMetrics::new())
}

fn make_tunnel_manager() -> Arc<ProductionTunnelManagerControl> {
    let dir = tempfile::tempdir().unwrap();
    let tm = ProductionTunnelManagerControl::new(dir.keep()).unwrap();
    Arc::new(tm)
}

#[tokio::test]
async fn production_router_info_reads_live_event_metrics() {
    let metrics = make_metrics();
    metrics.set_transport_bytes(1234, 5678);
    let tm = make_tunnel_manager();
    tm.load().await.unwrap();
    let ri = ProductionRouterInfoControl::new(
        "test-id".to_string(),
        "test".to_string(),
        0.0,
        0,
        0,
        metrics,
        Arc::new(LogRing::default()),
        tm,
    );

    let bytes = ri.transport_bytes().await.unwrap();
    assert_eq!(bytes.received, 1234);
    assert_eq!(bytes.sent, 5678);
}

// --- ProductionControlPlane ---

#[test]
fn production_control_plane_identity_and_uptime() {
    let metrics = make_metrics();
    let cp = ProductionControlPlane::new(
        "test-router-id-b64".to_string(),
        "Emissary 0.5.0".to_string(),
        metrics,
    );
    assert_eq!(cp.router_identity().unwrap(), "test-router-id-b64");
    assert_eq!(cp.router_version(), "Emissary 0.5.0");
    assert!(cp.router_uptime_ms() < 1000);
}

// --- ProductionAddressBookControl ---

#[tokio::test]
async fn production_address_book_control_crud() {
    let dir = tempfile::tempdir().unwrap();
    let ab = ProductionAddressBookControl::new(dir.keep());
    ab.load().await.unwrap();

    use emissary_cli::i2pcontrol::{
        control_plane::AddressBookControl,
        domain::address_book::{AddressBookEntry, AdministrativeAddressBookType},
    };

    let entries = ab.list(AdministrativeAddressBookType::Private).await.unwrap();
    assert!(entries.is_empty());

    ab.add(
        AdministrativeAddressBookType::Private,
        AddressBookEntry::new("test.i2p", "dest"),
    )
    .await
    .unwrap();

    let entries = ab.list(AdministrativeAddressBookType::Private).await.unwrap();
    assert_eq!(entries.len(), 1);

    let found = ab.lookup(AdministrativeAddressBookType::Private, "test.i2p").await.unwrap();
    assert!(found.is_some());

    let updated = ab
        .update(
            AdministrativeAddressBookType::Private,
            AddressBookEntry::new("test.i2p", "new-dest"),
        )
        .await
        .unwrap();
    assert!(updated);

    let deleted = ab.delete(AdministrativeAddressBookType::Private, "test.i2p").await.unwrap();
    assert!(deleted);
}

#[tokio::test]
async fn production_address_book_persistence_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    {
        let ab = ProductionAddressBookControl::new(dir.path().to_path_buf());
        ab.load().await.unwrap();
        use emissary_cli::i2pcontrol::{
            control_plane::AddressBookControl,
            domain::address_book::{AddressBookEntry, AdministrativeAddressBookType},
        };
        ab.add(
            AdministrativeAddressBookType::Private,
            AddressBookEntry::new("p.i2p", "p-dest"),
        )
        .await
        .unwrap();
        ab.add(
            AdministrativeAddressBookType::Local,
            AddressBookEntry::new("l.i2p", "l-dest"),
        )
        .await
        .unwrap();
    }
    {
        let ab = ProductionAddressBookControl::new(dir.path().to_path_buf());
        ab.load().await.unwrap();
        use emissary_cli::i2pcontrol::{
            control_plane::AddressBookControl, domain::address_book::AdministrativeAddressBookType,
        };
        let total = ab.list(AdministrativeAddressBookType::Private).await.unwrap().len()
            + ab.list(AdministrativeAddressBookType::Local).await.unwrap().len();
        assert_eq!(total, 2);
    }
}

// --- ProductionTunnelManagerControl ---

#[tokio::test]
async fn production_tunnel_manager_crud() {
    let dir = tempfile::tempdir().unwrap();
    let tm = ProductionTunnelManagerControl::new(dir.keep()).unwrap();
    tm.load().await.unwrap();

    use emissary_cli::i2pcontrol::{
        control_plane::TunnelManagerControl,
        domain::tunnel::{
            StartIntent, TunnelDefinition, TunnelName, TunnelOptions, TunnelOwnership,
            TunnelRuntimeState, TunnelType,
        },
    };

    let list = tm.list().await.unwrap();
    assert!(list.is_empty());

    let def = TunnelDefinition {
        name: TunnelName::new("test-tunnel").unwrap(),
        tunnel_type: TunnelType::Socks,
        ownership: TunnelOwnership::ControlPlane,
        runtime_state: TunnelRuntimeState::Stopped,
        start_intent: StartIntent::DoNotStart,
        options: TunnelOptions::default(),
        raw_config: std::collections::BTreeMap::new(),
    };
    tm.create(def.clone()).await.unwrap();
    let list = tm.list().await.unwrap();
    assert_eq!(list.len(), 1);

    let got = tm.get("test-tunnel").await.unwrap();
    assert!(got.is_some());
    let deleted = tm.delete("test-tunnel").await.unwrap();
    assert!(deleted);
}

#[tokio::test]
async fn production_tunnel_manager_duplicate_create_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let tm = ProductionTunnelManagerControl::new(dir.keep()).unwrap();
    tm.load().await.unwrap();
    use emissary_cli::i2pcontrol::{
        control_plane::TunnelManagerControl,
        domain::tunnel::{
            StartIntent, TunnelDefinition, TunnelName, TunnelOptions, TunnelOwnership,
            TunnelRuntimeState, TunnelType,
        },
    };
    let def = TunnelDefinition {
        name: TunnelName::new("dup").unwrap(),
        tunnel_type: TunnelType::Client,
        ownership: TunnelOwnership::ControlPlane,
        runtime_state: TunnelRuntimeState::Stopped,
        start_intent: StartIntent::DoNotStart,
        options: TunnelOptions::default(),
        raw_config: std::collections::BTreeMap::new(),
    };
    tm.create(def.clone()).await.unwrap();
    let err = tm.create(def).await;
    assert!(err.is_err());
}

// --- ProductionRouterInfoControl ---

#[tokio::test]
async fn production_router_info_identity_and_version() {
    let metrics = make_metrics();
    let tunnel_mgr = make_tunnel_manager();
    let log_ring = Arc::new(LogRing::default());
    let ri = ProductionRouterInfoControl::new(
        "test-router-id-b64".to_string(),
        "Emissary 0.5.0".to_string(),
        0.5,
        1024,
        512,
        metrics,
        log_ring,
        tunnel_mgr,
    );
    assert_eq!(ri.router_identity().unwrap(), "test-router-id-b64");
    assert_eq!(ri.router_version().unwrap(), "Emissary 0.5.0");
    assert_eq!(ri.share_ratio().await.unwrap(), 0.5);
    let (in_bw, out_bw) = ri.configured_bw_limits().await.unwrap();
    assert_eq!(in_bw, 1024);
    assert_eq!(out_bw, 512);
}

#[tokio::test]
async fn production_router_info_network_status_unknown_initially() {
    let metrics = make_metrics();
    let tunnel_mgr = make_tunnel_manager();
    let log_ring = Arc::new(LogRing::default());
    let ri = ProductionRouterInfoControl::new(
        String::new(),
        "test".to_string(),
        0.0,
        0,
        0,
        metrics,
        log_ring,
        tunnel_mgr,
    );
    let network = ri.network_snapshot().await.unwrap();
    assert_eq!(network.ipv4_status, NetworkStatus::Unknown);
    assert_eq!(network.ipv6_status, NetworkStatus::Unknown);
    assert!(!network.firewalled);
    assert!(!network.hidden);
}

#[tokio::test]
async fn production_router_info_network_status_after_set() {
    let metrics = make_metrics();
    metrics.set_firewall(FirewallStatus::Ok, FirewallStatus::Firewalled);
    let tunnel_mgr = make_tunnel_manager();
    let log_ring = Arc::new(LogRing::default());
    let ri = ProductionRouterInfoControl::new(
        String::new(),
        "test".to_string(),
        0.0,
        0,
        0,
        metrics,
        log_ring,
        tunnel_mgr,
    );
    let network = ri.network_snapshot().await.unwrap();
    assert_eq!(network.ipv4_status, NetworkStatus::Ok);
    assert_eq!(network.ipv6_status, NetworkStatus::Firewalled);
    assert!(network.firewalled);
}

#[tokio::test]
async fn production_router_info_clock_skew_unknown() {
    let metrics = make_metrics();
    let tunnel_mgr = make_tunnel_manager();
    let log_ring = Arc::new(LogRing::default());
    let ri = ProductionRouterInfoControl::new(
        String::new(),
        "test".to_string(),
        0.0,
        0,
        0,
        metrics,
        log_ring,
        tunnel_mgr,
    );
    let skew = ri.clock_skew().await.unwrap();
    assert!(skew.skew_seconds.is_none());
}

#[tokio::test]
async fn production_router_info_transport_bytes_from_event_handle() {
    let metrics = make_metrics();
    metrics.transport_inbound.fetch_add(1500, Ordering::Release);
    metrics.transport_outbound.fetch_add(2000, Ordering::Release);
    metrics.transit_inbound.fetch_add(500, Ordering::Release);
    metrics.transit_outbound.fetch_add(700, Ordering::Release);
    let tunnel_mgr = make_tunnel_manager();
    let log_ring = Arc::new(LogRing::default());
    let ri = ProductionRouterInfoControl::new(
        String::new(),
        "test".to_string(),
        0.0,
        0,
        0,
        metrics,
        log_ring,
        tunnel_mgr,
    );
    let tb = ri.transport_bytes().await.unwrap();
    assert_eq!(tb.received, 1500);
    assert_eq!(tb.sent, 2000);

    let trans = ri.transit_bytes().await.unwrap();
    assert_eq!(trans.received, 500);
    assert_eq!(trans.sent, 700);
}

#[tokio::test]
async fn production_router_info_tunnel_build_stats() {
    let metrics = make_metrics();
    metrics.build_successes.fetch_add(10, Ordering::Release);
    metrics.build_failures.fetch_add(5, Ordering::Release);
    let tunnel_mgr = make_tunnel_manager();
    let log_ring = Arc::new(LogRing::default());
    let ri = ProductionRouterInfoControl::new(
        String::new(),
        "test".to_string(),
        0.0,
        0,
        0,
        metrics,
        log_ring,
        tunnel_mgr,
    );
    let stats = ri.tunnel_build_stats().await.unwrap();
    assert_eq!(stats.successes, 10);
    assert_eq!(stats.failures, 5);
}

#[tokio::test]
async fn production_router_info_log_round_trip() {
    let metrics = make_metrics();
    let tunnel_mgr = make_tunnel_manager();
    let log_ring = Arc::new(LogRing::default());
    log_ring.push(LogEntry {
        timestamp_ms: 0,
        level: "INFO".to_string(),
        target: "test".to_string(),
        message: "hello".to_string(),
    });
    let ri = ProductionRouterInfoControl::new(
        String::new(),
        "test".to_string(),
        0.0,
        0,
        0,
        metrics,
        Arc::clone(&log_ring),
        tunnel_mgr,
    );
    let snap = ri.log_snapshot().await.unwrap();
    assert_eq!(snap.entries.len(), 1);
    assert_eq!(snap.entries[0].message, "hello");
    ri.log_clear().await.unwrap();
    let snap = ri.log_snapshot().await.unwrap();
    assert!(snap.entries.is_empty());
    assert_eq!(snap.generation, 1);
}

#[tokio::test]
async fn production_router_info_i2ptunnel_stats() {
    let metrics = make_metrics();
    let dir = tempfile::tempdir().unwrap();
    let tm = ProductionTunnelManagerControl::new(dir.keep()).unwrap();
    tm.load().await.unwrap();
    use emissary_cli::i2pcontrol::{
        control_plane::TunnelManagerControl,
        domain::tunnel::{
            StartIntent, TunnelDefinition, TunnelName, TunnelOptions, TunnelOwnership,
            TunnelRuntimeState, TunnelType,
        },
    };
    tm.create(TunnelDefinition {
        name: TunnelName::new("t1").unwrap(),
        tunnel_type: TunnelType::Socks,
        ownership: TunnelOwnership::ControlPlane,
        runtime_state: TunnelRuntimeState::Stopped,
        start_intent: StartIntent::DoNotStart,
        options: TunnelOptions::default(),
        raw_config: std::collections::BTreeMap::new(),
    })
    .await
    .unwrap();
    let tm_arc = Arc::new(tm);
    let log_ring = Arc::new(LogRing::default());
    let ri = ProductionRouterInfoControl::new(
        String::new(),
        "test".to_string(),
        0.0,
        0,
        0,
        metrics,
        log_ring,
        tm_arc,
    );
    let stats = ri.i2ptunnel_stats().await.unwrap();
    assert_eq!(stats.configured_count, 1);
}

#[tokio::test]
async fn production_router_info_udp_snapshot_returns_unavailable() {
    let metrics = make_metrics();
    metrics.set_firewall(FirewallStatus::Firewalled, FirewallStatus::Unknown);
    metrics.connected_routers.fetch_add(1, Ordering::Release);
    let tunnel_mgr = make_tunnel_manager();
    let log_ring = Arc::new(LogRing::default());
    let ri = ProductionRouterInfoControl::new(
        String::new(),
        "test".to_string(),
        0.0,
        0,
        0,
        metrics,
        log_ring,
        tunnel_mgr,
    );
    // UDP-specific active state requires a transport-specific canonical
    // source. No such source exists in EventMetrics, so UDP snapshot
    // returns Unavailable rather than inferring from an aggregate.
    let result = ri.udp_snapshot().await;
    assert!(result.is_err());
}

#[tokio::test]
async fn production_router_info_tcp_snapshot_returns_unavailable() {
    let metrics = make_metrics();
    let tunnel_mgr = make_tunnel_manager();
    let log_ring = Arc::new(LogRing::default());
    let ri = ProductionRouterInfoControl::new(
        String::new(),
        "test".to_string(),
        0.0,
        0,
        0,
        metrics,
        log_ring,
        tunnel_mgr,
    );
    // TCP-specific active state requires a transport-specific canonical
    // source. No such source exists in EventMetrics, so TCP snapshot
    // returns Unavailable rather than fabricating a value.
    let result = ri.tcp_snapshot().await;
    assert!(result.is_err());
}

// --- Static guards ---

#[test]
fn production_router_info_returns_no_router_news() {
    let metrics = make_metrics();
    let tunnel_mgr = make_tunnel_manager();
    let log_ring = Arc::new(LogRing::default());
    let ri = ProductionRouterInfoControl::new(
        String::new(),
        "test".to_string(),
        0.0,
        0,
        0,
        metrics,
        log_ring,
        tunnel_mgr,
    );
    assert_eq!(ri.router_news().unwrap(), "");
}

#[test]
fn production_adapter_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<ProductionAddressBookControl>();
    assert_send_sync::<ProductionTunnelManagerControl>();
    assert_send_sync::<ProductionRouterInfoControl>();
    assert_send_sync::<ProductionControlPlane>();
}

// Direct construction test of the underlying stores
#[test]
fn address_book_store_direct_construction() {
    let _store = AddressBookStore::new(tempfile::tempdir().unwrap().keep(), 1024);
}

#[test]
fn tunnel_store_direct_construction() {
    let _store = TunnelStore::new(tempfile::tempdir().unwrap().keep(), 1024);
}
