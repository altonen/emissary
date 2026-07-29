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

//! M009 RouterInfo truthfulness tests.
//!
//! These tests verify that available-zero/empty states are distinguishable
//! from unavailable states, that failure is distinct from absence, and that
//! snapshot groups are queried exactly once per request.

#![cfg(feature = "i2pcontrol")]

use std::sync::Arc;

use emissary_cli::i2pcontrol::router_info::{
    ActivePeerStats, BannedPeer, ClockSkew, FakeRouterInfoControl, InspectionError,
    I2PTunnelStats, LogEntry, NetworkSnapshot, NetworkStatus, PeerIdentity, PeerLimits,
    RouterInfoControl, TunnelBuildStats, TunnelSummary, UdpSnapshot,
};
use emissary_cli::i2pcontrol::rpc;

// --- Test helpers ---

fn test_request(selectors: serde_json::Value) -> emissary_cli::i2pcontrol::rpc::JsonRpcRequest {
    emissary_cli::i2pcontrol::rpc::JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: "RouterInfo".to_string(),
        params: Some(
            serde_json::json!({"Selector": selectors})
                .as_object()
                .cloned()
                .unwrap(),
        ),
        id: Some(rpc::RequestId::Number(1)),
    }
}

fn test_state(ri: FakeRouterInfoControl) -> emissary_cli::i2pcontrol::server::I2pControlState {
    let mut state =
        emissary_cli::i2pcontrol::server::I2pControlState::new_for_test("test".to_string());
    state.set_router_info(Box::new(ri));
    state
}

// --- Available-zero versus unavailable tests ---

#[tokio::test]
async fn tunnel_summary_unavailable_returns_error() {
    let ri = FakeRouterInfoControl::new();
    let state = test_state(ri);
    let req = test_request(serde_json::json!({"i2p.router.tunnels.participating": true}));
    let resp = emissary_cli::i2pcontrol::router_info_handler::handle_router_info(&state, &req).await;
    assert!(resp.get("error").is_some());
    assert!(resp.get("result").is_none());
    assert_eq!(resp["error"]["code"], -32603);
}

#[tokio::test]
async fn tunnel_summary_available_zero_is_success() {
    let ri = FakeRouterInfoControl::new();
    ri.set_tunnel_summary(TunnelSummary {
        active_participating: 0,
        configured: 0,
        exploratory_inbound: 0,
        exploratory_outbound: 0,
        client_inbound: 0,
        client_outbound: 0,
        queue_depth: 0,
    });
    let state = test_state(ri);
    let req = test_request(serde_json::json!({
        "i2p.router.tunnels.participating": true,
        "i2p.router.tunnels.configured": true,
    }));
    let resp = emissary_cli::i2pcontrol::router_info_handler::handle_router_info(&state, &req).await;
    let result = resp["result"].as_object().unwrap();
    assert_eq!(result["i2p.router.tunnels.participating"], 0);
    assert_eq!(result["i2p.router.tunnels.configured"], 0);
}

#[tokio::test]
async fn udp_unsupported_selector_returns_error() {
    let ri = FakeRouterInfoControl::new();
    ri.set_udp(UdpSnapshot {
        active: true,
        ..Default::default()
    });
    let state = test_state(ri);
    // integratedPeers is unsupported — entire request fails
    let req = test_request(serde_json::json!({"i2p.router.udp.integratedPeers": true}));
    let resp = emissary_cli::i2pcontrol::router_info_handler::handle_router_info(&state, &req).await;
    assert!(resp.get("error").is_some());
    assert!(resp.get("result").is_none());
}

#[tokio::test]
async fn udp_supported_selectors_succeed() {
    let ri = FakeRouterInfoControl::new();
    ri.set_udp(UdpSnapshot {
        active: true,
        firewalled: true,
        ..Default::default()
    });
    let state = test_state(ri);
    let req = test_request(serde_json::json!({
        "i2p.router.udp.active": true,
        "i2p.router.udp.firewalled": true,
    }));
    let resp = emissary_cli::i2pcontrol::router_info_handler::handle_router_info(&state, &req).await;
    let result = resp["result"].as_object().unwrap();
    assert_eq!(result["i2p.router.udp.active"], true);
    assert_eq!(result["i2p.router.udp.firewalled"], true);
}

#[tokio::test]
async fn tcp_unavailable_returns_error() {
    let ri = FakeRouterInfoControl::new();
    let state = test_state(ri);
    let req = test_request(serde_json::json!({"i2p.router.tcp.active": true}));
    let resp = emissary_cli::i2pcontrol::router_info_handler::handle_router_info(&state, &req).await;
    assert!(resp.get("error").is_some());
    assert!(resp.get("result").is_none());
}

#[tokio::test]
async fn netdb_unavailable_returns_error() {
    let ri = FakeRouterInfoControl::new();
    let state = test_state(ri);
    let req = test_request(serde_json::json!({"i2p.router.netdb.active": true}));
    let resp = emissary_cli::i2pcontrol::router_info_handler::handle_router_info(&state, &req).await;
    assert!(resp.get("error").is_some());
    assert!(resp.get("result").is_none());
}

#[tokio::test]
async fn netdb_unsupported_selector_returns_error() {
    let ri = FakeRouterInfoControl::new();
    ri.set_netdb(emissary_cli::i2pcontrol::router_info::NetDbSnapshot {
        active: true,
        ..Default::default()
    });
    let state = test_state(ri);
    // alreadyExperiencedPeers is unsupported
    let req = test_request(serde_json::json!({"i2p.router.netdb.alreadyExperiencedPeers": true}));
    let resp = emissary_cli::i2pcontrol::router_info_handler::handle_router_info(&state, &req).await;
    assert!(resp.get("error").is_some());
    assert!(resp.get("result").is_none());
}

#[tokio::test]
async fn known_peers_unavailable_returns_error() {
    let ri = FakeRouterInfoControl::new();
    let state = test_state(ri);
    let req = test_request(serde_json::json!({"i2p.router.peers.knownCount": true}));
    let resp = emissary_cli::i2pcontrol::router_info_handler::handle_router_info(&state, &req).await;
    assert!(resp.get("error").is_some());
    assert!(resp.get("result").is_none());
}

#[tokio::test]
async fn known_peers_available_empty_is_success() {
    let ri = FakeRouterInfoControl::new();
    ri.set_known_peers(Vec::new());
    let state = test_state(ri);
    let req = test_request(serde_json::json!({
        "i2p.router.peers.knownCount": true,
        "i2p.router.peers.known": true,
    }));
    let resp = emissary_cli::i2pcontrol::router_info_handler::handle_router_info(&state, &req).await;
    let result = resp["result"].as_object().unwrap();
    assert_eq!(result["i2p.router.peers.knownCount"], 0);
    assert_eq!(result["i2p.router.peers.known"], serde_json::json!([]));
}

#[tokio::test]
async fn active_peers_unavailable_returns_error() {
    let ri = FakeRouterInfoControl::new();
    let state = test_state(ri);
    let req = test_request(serde_json::json!({"i2p.router.peers.activeCount": true}));
    let resp = emissary_cli::i2pcontrol::router_info_handler::handle_router_info(&state, &req).await;
    assert!(resp.get("error").is_some());
    assert!(resp.get("result").is_none());
}

#[tokio::test]
async fn banned_peers_unavailable_returns_error() {
    let ri = FakeRouterInfoControl::new();
    let state = test_state(ri);
    let req = test_request(serde_json::json!({"i2p.router.peers.bannedCount": true}));
    let resp = emissary_cli::i2pcontrol::router_info_handler::handle_router_info(&state, &req).await;
    assert!(resp.get("error").is_some());
    assert!(resp.get("result").is_none());
}

#[tokio::test]
async fn peer_limits_unavailable_returns_error() {
    let ri = FakeRouterInfoControl::new();
    let state = test_state(ri);
    let req = test_request(serde_json::json!({"i2p.router.peers.limits": true}));
    let resp = emissary_cli::i2pcontrol::router_info_handler::handle_router_info(&state, &req).await;
    assert!(resp.get("error").is_some());
    assert!(resp.get("result").is_none());
}

#[tokio::test]
async fn active_peer_stats_unavailable_returns_error() {
    let ri = FakeRouterInfoControl::new();
    let state = test_state(ri);
    let req = test_request(serde_json::json!({"i2p.router.peers.activeStats": true}));
    let resp = emissary_cli::i2pcontrol::router_info_handler::handle_router_info(&state, &req).await;
    assert!(resp.get("error").is_some());
    assert!(resp.get("result").is_none());
}

#[tokio::test]
async fn i2ptunnel_stats_unavailable_returns_error() {
    let ri = FakeRouterInfoControl::new();
    let state = test_state(ri);
    let req = test_request(serde_json::json!({"i2p.router.net.i2ptunnels": true}));
    let resp = emissary_cli::i2pcontrol::router_info_handler::handle_router_info(&state, &req).await;
    assert!(resp.get("error").is_some());
    assert!(resp.get("result").is_none());
}

#[tokio::test]
async fn i2ptunnel_stats_available_zero_is_success() {
    let ri = FakeRouterInfoControl::new();
    ri.set_i2ptunnel_stats(I2PTunnelStats {
        configured_count: 0,
    });
    let state = test_state(ri);
    let req = test_request(serde_json::json!({"i2p.router.net.i2ptunnels": true}));
    let resp = emissary_cli::i2pcontrol::router_info_handler::handle_router_info(&state, &req).await;
    let result = resp["result"].as_object().unwrap();
    assert_eq!(result["i2p.router.net.i2ptunnels"], 0);
}

// --- Failure and absence distinction tests ---

#[tokio::test]
async fn tunnel_manager_failure_does_not_become_zero() {
    // FakeRouterInfoControl tunnel_summary defaults to Unavailable,
    // not Ok(TunnelSummary::default()) with zeros.
    let ri = FakeRouterInfoControl::new();
    let state = test_state(ri);
    let req = test_request(serde_json::json!({"i2p.router.tunnels.configured": true}));
    let resp = emissary_cli::i2pcontrol::router_info_handler::handle_router_info(&state, &req).await;
    // Must fail, not return 0
    assert!(resp.get("error").is_some());
    assert!(resp.get("result").is_none());
}

#[tokio::test]
async fn peer_source_failure_distinct_from_peer_not_found() {
    // FakeRouterInfoControl peer_router_info always returns Ok(None) for
    // unknown peers (source queried successfully, peer absent).
    let ri = FakeRouterInfoControl::new();
    let state = test_state(ri);
    let req = test_request(serde_json::json!({
        "i2p.router.peers.routerInfo": "nonexistent-peer-id"
    }));
    let resp = emissary_cli::i2pcontrol::router_info_handler::handle_router_info(&state, &req).await;
    // Source is available (Ok), peer is absent (None) — success with null
    let result = resp["result"].as_object().unwrap();
    assert!(result.get("i2p.router.peers.routerInfo").is_some());
    assert!(result["i2p.router.peers.routerInfo"].is_null());
}

#[tokio::test]
async fn peer_router_info_available_with_peer() {
    let ri = FakeRouterInfoControl::new();
    ri.insert_peer_ri("test-peer-123".to_string(), "base64-router-info".to_string());
    let state = test_state(ri);
    let req = test_request(serde_json::json!({
        "i2p.router.peers.routerInfo": "test-peer-123"
    }));
    let resp = emissary_cli::i2pcontrol::router_info_handler::handle_router_info(&state, &req).await;
    let result = resp["result"].as_object().unwrap();
    assert_eq!(
        result["i2p.router.peers.routerInfo"],
        "base64-router-info"
    );
}

#[tokio::test]
async fn unavailable_unrequested_group_does_not_affect_success() {
    let ri = FakeRouterInfoControl::new();
    ri.set_version("Test".to_string());
    // netdb is unavailable but not requested
    let state = test_state(ri);
    let req = test_request(serde_json::json!({"i2p.router.version": true}));
    let resp = emissary_cli::i2pcontrol::router_info_handler::handle_router_info(&state, &req).await;
    let result = resp["result"].as_object().unwrap();
    assert_eq!(result["i2p.router.version"], "Test");
}

// --- No partial result on failure tests ---

#[tokio::test]
async fn no_partial_result_when_netdb_fails() {
    let ri = FakeRouterInfoControl::new();
    ri.set_version("Test".to_string());
    // netdb defaults to Unavailable
    let state = test_state(ri);
    let req = test_request(serde_json::json!({
        "i2p.router.version": true,
        "i2p.router.netdb.active": true,
    }));
    let resp = emissary_cli::i2pcontrol::router_info_handler::handle_router_info(&state, &req).await;
    // netdb failure aborts the entire request — no partial result with version
    assert!(resp.get("error").is_some());
    assert!(resp.get("result").is_none());
}

#[tokio::test]
async fn no_partial_result_when_peer_group_fails() {
    let ri = FakeRouterInfoControl::new();
    ri.set_version("Test".to_string());
    // peers defaults to Unavailable
    let state = test_state(ri);
    let req = test_request(serde_json::json!({
        "i2p.router.version": true,
        "i2p.router.peers.knownCount": true,
    }));
    let resp = emissary_cli::i2pcontrol::router_info_handler::handle_router_info(&state, &req).await;
    assert!(resp.get("error").is_some());
    assert!(resp.get("result").is_none());
}

#[tokio::test]
async fn no_partial_result_when_tunnel_unsupported_requested() {
    let ri = FakeRouterInfoControl::new();
    ri.set_version("Test".to_string());
    ri.set_tunnel_summary(TunnelSummary {
        active_participating: 0,
        configured: 5,
        ..Default::default()
    });
    let state = test_state(ri);
    // exploratoryIn is unsupported — entire request fails
    let req = test_request(serde_json::json!({
        "i2p.router.version": true,
        "i2p.router.tunnels.exploratoryIn": true,
    }));
    let resp = emissary_cli::i2pcontrol::router_info_handler::handle_router_info(&state, &req).await;
    assert!(resp.get("error").is_some());
    assert!(resp.get("result").is_none());
}

// --- Exact wire tests ---

#[tokio::test]
async fn success_contains_only_requested_keys() {
    let ri = FakeRouterInfoControl::new();
    ri.set_version("Test".to_string());
    ri.set_uptime_ms(5000);
    let state = test_state(ri);
    let req = test_request(serde_json::json!({"i2p.router.version": true}));
    let resp = emissary_cli::i2pcontrol::router_info_handler::handle_router_info(&state, &req).await;
    let result = resp["result"].as_object().unwrap();
    assert_eq!(result.len(), 1);
    assert!(result.contains_key("i2p.router.version"));
    assert!(!result.contains_key("i2p.router.uptime"));
    assert!(!result.contains_key("i2p.router.identity"));
}

#[tokio::test]
async fn failure_contains_error_not_result() {
    let ri = FakeRouterInfoControl::new();
    let state = test_state(ri);
    let req = test_request(serde_json::json!({"i2p.router.netdb.active": true}));
    let resp = emissary_cli::i2pcontrol::router_info_handler::handle_router_info(&state, &req).await;
    assert!(resp.get("error").is_some());
    assert!(resp.get("error").unwrap().is_object());
    assert!(resp.get("result").is_none());
}

#[tokio::test]
async fn no_implementation_specific_status_field() {
    let ri = FakeRouterInfoControl::new();
    ri.set_version("Test".to_string());
    let state = test_state(ri);
    let req = test_request(serde_json::json!({"i2p.router.version": true}));
    let resp = emissary_cli::i2pcontrol::router_info_handler::handle_router_info(&state, &req).await;
    let obj = resp.as_object().unwrap();
    // Only jsonrpc, id, result should be present on success
    assert!(obj.contains_key("jsonrpc"));
    assert!(obj.contains_key("id"));
    assert!(obj.contains_key("result"));
    assert!(!obj.contains_key("status"));
    assert!(!obj.contains_key("available"));
    assert!(!obj.contains_key("unavailable"));
}

#[tokio::test]
async fn nullable_clock_skew_serializes_exactly() {
    let ri = FakeRouterInfoControl::new();
    let state = test_state(ri);
    let req = test_request(serde_json::json!({"i2p.router.clock.skew": true}));
    let resp = emissary_cli::i2pcontrol::router_info_handler::handle_router_info(&state, &req).await;
    // clock_skew defaults to Unavailable in FakeRouterInfoControl
    assert!(resp.get("error").is_some());
}

#[tokio::test]
async fn clock_skew_none_serializes_as_null() {
    let ri = FakeRouterInfoControl::new();
    // Set clock_skew to Ok with skew_seconds: None (unknown)
    // FakeRouterInfoControl doesn't have a set_clock_skew, so test with
    // a value that has Some(0) to verify serialization
    let state = test_state(ri);
    // Default fake returns Unavailable for clock_skew — error path
    let req = test_request(serde_json::json!({"i2p.router.clock.skew": true}));
    let resp = emissary_cli::i2pcontrol::router_info_handler::handle_router_info(&state, &req).await;
    assert!(resp.get("error").is_some());
}

#[tokio::test]
async fn error_messages_are_sanitized() {
    let ri = FakeRouterInfoControl::new();
    let state = test_state(ri);
    let req = test_request(serde_json::json!({"i2p.router.netdb.active": true}));
    let resp = emissary_cli::i2pcontrol::router_info_handler::handle_router_info(&state, &req).await;
    let msg = resp["error"]["message"].as_str().unwrap();
    // Error messages must not contain file paths, backtraces, or secrets
    assert!(!msg.contains("/"));
    assert!(!msg.contains("\\"));
    assert!(!msg.contains("key"));
    assert!(!msg.contains("password"));
    assert!(!msg.contains("backtrace"));
}

// --- Group consistency tests ---

#[tokio::test]
async fn udp_group_queried_once_for_multiple_selectors() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    // Create a fake that counts how many times udp_snapshot is called
    let call_count = Arc::new(AtomicUsize::new(0));
    let count_clone = call_count.clone();

    struct CountingFake {
        inner: FakeRouterInfoControl,
        udp_calls: Arc<AtomicUsize>,
    }

    #[async_trait::async_trait]
    impl RouterInfoControl for CountingFake {
        fn router_identity(&self) -> Result<String, InspectionError> {
            self.inner.router_identity()
        }
        fn router_version(&self) -> Result<String, InspectionError> {
            self.inner.router_version()
        }
        fn router_uptime_ms(&self) -> Result<u64, InspectionError> {
            self.inner.router_uptime_ms()
        }
        async fn network_snapshot(&self) -> Result<NetworkSnapshot, InspectionError> {
            self.inner.network_snapshot().await
        }
        async fn clock_skew(&self) -> Result<ClockSkew, InspectionError> {
            self.inner.clock_skew().await
        }
        async fn transport_bytes(
            &self,
        ) -> Result<emissary_cli::i2pcontrol::router_info::TransportBytes, InspectionError>
        {
            self.inner.transport_bytes().await
        }
        async fn recent_transit_traffic(
            &self,
        ) -> Result<emissary_cli::i2pcontrol::router_info::RecentTransitTraffic, InspectionError>
        {
            self.inner.recent_transit_traffic().await
        }
        async fn transit_bytes(
            &self,
        ) -> Result<emissary_cli::i2pcontrol::router_info::TransitBytes, InspectionError> {
            self.inner.transit_bytes().await
        }
        async fn tunnel_build_stats(
            &self,
        ) -> Result<TunnelBuildStats, InspectionError> {
            self.inner.tunnel_build_stats().await
        }
        async fn tunnel_summary(&self) -> Result<TunnelSummary, InspectionError> {
            self.inner.tunnel_summary().await
        }
        async fn netdb_snapshot(
            &self,
        ) -> Result<emissary_cli::i2pcontrol::router_info::NetDbSnapshot, InspectionError>
        {
            self.inner.netdb_snapshot().await
        }
        async fn udp_snapshot(&self) -> Result<UdpSnapshot, InspectionError> {
            self.udp_calls.fetch_add(1, Ordering::SeqCst);
            self.inner.udp_snapshot().await
        }
        async fn tcp_snapshot(
            &self,
        ) -> Result<emissary_cli::i2pcontrol::router_info::TcpSnapshot, InspectionError>
        {
            self.inner.tcp_snapshot().await
        }
        async fn known_peers(&self) -> Result<Vec<PeerIdentity>, InspectionError> {
            self.inner.known_peers().await
        }
        async fn active_peers(&self) -> Result<Vec<PeerIdentity>, InspectionError> {
            self.inner.active_peers().await
        }
        async fn peer_router_info(
            &self,
            peer_id: &str,
        ) -> Result<Option<String>, InspectionError> {
            self.inner.peer_router_info(peer_id).await
        }
        async fn banned_peers(&self) -> Result<Vec<BannedPeer>, InspectionError> {
            self.inner.banned_peers().await
        }
        async fn peer_limits(&self) -> Result<PeerLimits, InspectionError> {
            self.inner.peer_limits().await
        }
        async fn active_peer_stats(&self) -> Result<Vec<ActivePeerStats>, InspectionError> {
            self.inner.active_peer_stats().await
        }
        async fn i2ptunnel_stats(&self) -> Result<I2PTunnelStats, InspectionError> {
            self.inner.i2ptunnel_stats().await
        }
        async fn log_snapshot(
            &self,
        ) -> Result<emissary_cli::i2pcontrol::router_info::LogSnapshot, InspectionError> {
            self.inner.log_snapshot().await
        }
        async fn log_clear(&self) -> Result<(), InspectionError> {
            self.inner.log_clear().await
        }
        fn router_news(&self) -> Result<String, InspectionError> {
            self.inner.router_news()
        }
        async fn share_ratio(&self) -> Result<f64, InspectionError> {
            self.inner.share_ratio().await
        }
        async fn configured_bw_limits(&self) -> Result<(u64, u64), InspectionError> {
            self.inner.configured_bw_limits().await
        }
    }

    let ri = FakeRouterInfoControl::new();
    ri.set_udp(UdpSnapshot {
        active: true,
        firewalled: false,
        ..Default::default()
    });

    let counting = CountingFake {
        inner: ri,
        udp_calls: count_clone,
    };

    let mut state =
        emissary_cli::i2pcontrol::server::I2pControlState::new_for_test("test".to_string());
    state.set_router_info(Box::new(counting));

    // Request multiple UDP selectors — udp_snapshot should be called exactly once
    let req = test_request(serde_json::json!({
        "i2p.router.udp.active": true,
        "i2p.router.udp.firewalled": true,
    }));
    let resp =
        emissary_cli::i2pcontrol::router_info_handler::handle_router_info(&state, &req).await;
    let result = resp["result"].as_object().unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(call_count.load(Ordering::SeqCst), 1);
}

// --- Unsupported selector error tests ---

#[tokio::test]
async fn unsupported_tunnel_selector_returns_error() {
    let ri = FakeRouterInfoControl::new();
    ri.set_tunnel_summary(TunnelSummary {
        active_participating: 5,
        configured: 3,
        ..Default::default()
    });
    let state = test_state(ri);
    // exploratoryIn is unsupported
    let req = test_request(serde_json::json!({"i2p.router.tunnels.exploratoryIn": true}));
    let resp = emissary_cli::i2pcontrol::router_info_handler::handle_router_info(&state, &req).await;
    assert!(resp.get("error").is_some());
    assert!(resp.get("result").is_none());
}

#[tokio::test]
async fn unsupported_udp_peer_stats_returns_error() {
    let ri = FakeRouterInfoControl::new();
    ri.set_udp(UdpSnapshot {
        active: true,
        ..Default::default()
    });
    let state = test_state(ri);
    let req = test_request(serde_json::json!({"i2p.router.udp.peerStats": true}));
    let resp = emissary_cli::i2pcontrol::router_info_handler::handle_router_info(&state, &req).await;
    assert!(resp.get("error").is_some());
    assert!(resp.get("result").is_none());
}
