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

//! Proposal 170 RouterInfo JSON-RPC method handler.
//!
//! Implements the `RouterInfo` method with exact selector-by-presence
//! behavior. Only requested selector keys appear in the response.

use std::collections::HashSet;

use crate::i2pcontrol::address_book::resolve_address_book_selectors;
use crate::i2pcontrol::control_plane::AddressBookControl;
use crate::i2pcontrol::router_info::RouterInfoControl;
use crate::i2pcontrol::rpc::{
    self, JsonRpcErrorResponse, JsonRpcRequest, JsonRpcSuccess, RequestId,
};

const LOG_TARGET: &str = "emissary::i2pcontrol::router_info_handler";

/// Maximum number of peer identities in a single response.
#[allow(dead_code)]
const MAX_PEER_IDENTITIES: usize = 10000;

/// Maximum total byte size of peer RouterInfo responses.
#[allow(dead_code)]
const MAX_PEER_RI_BYTES: usize = 4 * 1024 * 1024;

/// Maximum number of active peer stat entries.
#[allow(dead_code)]
const MAX_ACTIVE_PEER_STATS: usize = 10000;

/// Maximum number of log entries in a snapshot.
#[allow(dead_code)]
const MAX_LOG_ENTRIES: usize = 10000;

/// Maximum number of banned peers.
#[allow(dead_code)]
const MAX_BANNED_PEERS: usize = 10000;

/// Handle the RouterInfo JSON-RPC method.
///
/// Parses the `Selector` parameter, dispatches to snapshot sources,
/// and returns only the requested keys.
pub(crate) async fn handle_router_info(
    router_info: &dyn RouterInfoControl,
    address_book: &dyn AddressBookControl,
    request: &JsonRpcRequest,
) -> serde_json::Value {
    let id = resolve_id(&request.id);

    // Parse parameters
    let params = match &request.params {
        Some(params) => params,
        None => {
            return error_response(id, rpc::error_codes::INVALID_PARAMS, "Missing parameters");
        }
    };

    // Extract Selector parameter — must be a JSON object with boolean values
    let selector_map = match params.get("Selector") {
        Some(serde_json::Value::Object(map)) => map,
        _ => {
            return error_response(
                id,
                rpc::error_codes::INVALID_PARAMS,
                "Missing or invalid 'Selector' parameter; expected a JSON object",
            );
        }
    };

    // Build requested key set from presence of true values
    let mut requested_keys: Vec<&str> = Vec::new();
    for (key, value) in selector_map {
        if value.as_bool() == Some(true) {
            // Validate the selector key
            if !rpc::is_valid_router_info_selector(key) {
                return error_response(
                    id,
                    rpc::error_codes::INVALID_PARAMS,
                    format!("Unknown selector: '{key}'"),
                );
            }
            requested_keys.push(key.as_str());
        }
    }

    // Dispatch and assemble response
    match assemble_response(router_info, address_book, &requested_keys).await {
        Ok(result) => {
            let response = JsonRpcSuccess::new(id, serde_json::Value::Object(result));
            serde_json::to_value(&response).unwrap()
        }
        Err(e) => {
            tracing::error!(target: LOG_TARGET, "RouterInfo assembly failed: {e}");
            error_response(id, rpc::error_codes::INTERNAL_ERROR, e)
        }
    }
}

/// Assemble the response object containing only requested keys.
async fn assemble_response(
    router_info: &dyn RouterInfoControl,
    address_book: &dyn AddressBookControl,
    requested_keys: &[&str],
) -> Result<serde_json::Map<String, serde_json::Value>, String> {
    let mut result = serde_json::Map::new();

    if requested_keys.is_empty() {
        return Ok(result);
    }

    let key_set: HashSet<&str> = requested_keys.iter().copied().collect();

    // --- Identity and static router data ---
    if key_set.contains(rpc::router_info_keys::IDENTITY) {
        let identity = router_info.router_identity()?;
        result.insert(
            rpc::router_info_keys::IDENTITY.to_string(),
            serde_json::json!(identity),
        );
    }

    if key_set.contains(rpc::router_info_keys::VERSION) {
        let version = router_info.router_version();
        result.insert(
            rpc::router_info_keys::VERSION.to_string(),
            serde_json::json!(version),
        );
    }

    if key_set.contains(rpc::router_info_keys::UPTIME) {
        let uptime = router_info.router_uptime_ms();
        result.insert(
            rpc::router_info_keys::UPTIME.to_string(),
            serde_json::json!(uptime),
        );
    }

    // --- Router news ---
    if key_set.contains(rpc::router_info_keys::ROUTER_NEWS) {
        let news = router_info.router_news();
        result.insert(
            rpc::router_info_keys::ROUTER_NEWS.to_string(),
            serde_json::json!(news),
        );
    }

    // --- Clock skew ---
    if key_set.contains(rpc::router_info_keys::CLOCK_SKEW) {
        let skew = router_info.clock_skew().await;
        let value = match skew.skew_seconds {
            Some(s) => serde_json::json!(s),
            None => serde_json::json!(null),
        };
        result.insert(rpc::router_info_keys::CLOCK_SKEW.to_string(), value);
    }

    // --- Network status ---
    if key_set.contains(rpc::router_info_keys::NET_BW_INBOUND)
        || key_set.contains(rpc::router_info_keys::NET_BW_OUTBOUND)
    {
        let network = router_info.network_snapshot().await;
        if key_set.contains(rpc::router_info_keys::NET_BW_INBOUND) {
            result.insert(
                rpc::router_info_keys::NET_BW_INBOUND.to_string(),
                serde_json::json!(network.ipv4_status.as_str()),
            );
        }
        if key_set.contains(rpc::router_info_keys::NET_BW_OUTBOUND) {
            result.insert(
                rpc::router_info_keys::NET_BW_OUTBOUND.to_string(),
                serde_json::json!(network.ipv6_status.as_str()),
            );
        }
    }

    // --- Share ratio and configured BW ---
    if key_set.contains(rpc::router_info_keys::SHARE_RATIO) {
        let ratio = router_info.share_ratio().await;
        result.insert(
            rpc::router_info_keys::SHARE_RATIO.to_string(),
            serde_json::json!(ratio),
        );
    }
    if key_set.contains(rpc::router_info_keys::CONFIGURED_BW_INBOUND)
        || key_set.contains(rpc::router_info_keys::CONFIGURED_BW_OUTBOUND)
    {
        let (inbound, outbound) = router_info.configured_bw_limits().await;
        if key_set.contains(rpc::router_info_keys::CONFIGURED_BW_INBOUND) {
            result.insert(
                rpc::router_info_keys::CONFIGURED_BW_INBOUND.to_string(),
                serde_json::json!(inbound),
            );
        }
        if key_set.contains(rpc::router_info_keys::CONFIGURED_BW_OUTBOUND) {
            result.insert(
                rpc::router_info_keys::CONFIGURED_BW_OUTBOUND.to_string(),
                serde_json::json!(outbound),
            );
        }
    }

    // --- UDP transport ---
    if key_set.iter().any(|k| k.starts_with("i2p.router.udp.")) {
        let udp = router_info.udp_snapshot().await;
        resolve_udp_selectors(&mut result, &key_set, &udp);
    }

    // --- TCP transport ---
    if key_set.iter().any(|k| k.starts_with("i2p.router.tcp.")) {
        let tcp = router_info.tcp_snapshot().await;
        resolve_tcp_selectors(&mut result, &key_set, &tcp);
    }

    // --- NetDB ---
    if key_set.iter().any(|k| k.starts_with("i2p.router.netdb.")) {
        let netdb = router_info.netdb_snapshot().await;
        resolve_netdb_selectors(&mut result, &key_set, &netdb);
    }

    // --- Bandwidth ---
    if key_set.iter().any(|k| k.starts_with("i2p.router.bw.")) {
        let transport = router_info.transport_bytes().await;
        let transit = router_info.transit_bytes().await;
        let recent = router_info.recent_transit_traffic().await;
        resolve_bw_selectors(&mut result, &key_set, &transport, &transit, &recent);
    }

    // --- Tunnel selectors ---
    if key_set.iter().any(|k| k.starts_with("i2p.router.tunnels.")) {
        let summary = router_info.tunnel_summary().await;
        resolve_tunnel_selectors(&mut result, &key_set, &summary);
    }

    // --- I2PTunnel ---
    if key_set.contains(rpc::router_info_keys::NET_IPTUNNELS) {
        let stats = router_info.i2ptunnel_stats().await;
        result.insert(
            rpc::router_info_keys::NET_IPTUNNELS.to_string(),
            serde_json::json!(stats.configured_count),
        );
    }

    // --- Peer selectors ---
    if key_set.iter().any(|k| k.starts_with("i2p.router.peers.")) {
        resolve_peer_selectors(&mut result, &key_set, router_info).await?;
    }

    // --- Log selectors ---
    if key_set.contains(rpc::router_info_keys::LOG_SNAPSHOT) {
        let snap = router_info.log_snapshot().await;
        let entries: Vec<serde_json::Value> = snap
            .entries
            .iter()
            .map(|e| {
                serde_json::json!({
                    "timestamp": e.timestamp_ms,
                    "level": e.level,
                    "target": e.target,
                    "message": e.message,
                })
            })
            .collect();
        result.insert(
            rpc::router_info_keys::LOG_SNAPSHOT.to_string(),
            serde_json::json!(entries),
        );
    }
    if key_set.contains(rpc::router_info_keys::LOG_CLEAR) {
        router_info.log_clear().await;
        result.insert(
            rpc::router_info_keys::LOG_CLEAR.to_string(),
            serde_json::json!(true),
        );
    }

    // --- Address-book selectors ---
    let address_book_keys: Vec<&str> = key_set
        .iter()
        .copied()
        .filter(|k| rpc::router_info_keys::ADDRESS_BOOK_KEYS.contains(k))
        .collect();
    if !address_book_keys.is_empty() {
        let ab_result = resolve_address_book_selectors(address_book, &address_book_keys).await?;
        for (k, v) in ab_result {
            result.insert(k, v);
        }
    }

    Ok(result)
}

/// Resolve UDP transport selectors into response entries.
fn resolve_udp_selectors(
    result: &mut serde_json::Map<String, serde_json::Value>,
    key_set: &HashSet<&str>,
    udp: &crate::i2pcontrol::router_info::UdpSnapshot,
) {
    if key_set.contains(rpc::router_info_keys::UDP_ACTIVE) {
        result.insert(
            rpc::router_info_keys::UDP_ACTIVE.to_string(),
            serde_json::json!(udp.active),
        );
    }
    if key_set.contains(rpc::router_info_keys::UDP_COOKIE_ACTIVE) {
        result.insert(
            rpc::router_info_keys::UDP_COOKIE_ACTIVE.to_string(),
            serde_json::json!(udp.cookie_active),
        );
    }
    if key_set.contains(rpc::router_info_keys::UDP_INTEGRATED_PEERS) {
        result.insert(
            rpc::router_info_keys::UDP_INTEGRATED_PEERS.to_string(),
            serde_json::json!(udp.integrated_peers),
        );
    }
    if key_set.contains(rpc::router_info_keys::UDP_FIREWALLED) {
        result.insert(
            rpc::router_info_keys::UDP_FIREWALLED.to_string(),
            serde_json::json!(udp.firewalled),
        );
    }
    if key_set.contains(rpc::router_info_keys::UDP_HIDDEN) {
        result.insert(
            rpc::router_info_keys::UDP_HIDDEN.to_string(),
            serde_json::json!(udp.hidden),
        );
    }
    if key_set.contains(rpc::router_info_keys::UDP_COINFICIENT_PEERS) {
        result.insert(
            rpc::router_info_keys::UDP_COINFICIENT_PEERS.to_string(),
            serde_json::json!(udp.coinficient_peers),
        );
    }
    if key_set.contains(rpc::router_info_keys::UDP_CRITICAL_PEERS) {
        result.insert(
            rpc::router_info_keys::UDP_CRITICAL_PEERS.to_string(),
            serde_json::json!(udp.critical_peers),
        );
    }
    if key_set.contains(rpc::router_info_keys::UDP_FAST_PEERS) {
        result.insert(
            rpc::router_info_keys::UDP_FAST_PEERS.to_string(),
            serde_json::json!(udp.fast_peers),
        );
    }
    if key_set.contains(rpc::router_info_keys::UDP_HIGH_CAPACITY_PEERS) {
        result.insert(
            rpc::router_info_keys::UDP_HIGH_CAPACITY_PEERS.to_string(),
            serde_json::json!(udp.high_capacity_peers),
        );
    }
    if key_set.contains(rpc::router_info_keys::UDP_INTERLEAVED_PEERS) {
        result.insert(
            rpc::router_info_keys::UDP_INTERLEAVED_PEERS.to_string(),
            serde_json::json!(udp.interleaved_peers),
        );
    }
    if key_set.contains(rpc::router_info_keys::UDP_LIT_PEERS) {
        result.insert(
            rpc::router_info_keys::UDP_LIT_PEERS.to_string(),
            serde_json::json!(udp.lit_peers),
        );
    }
    if key_set.contains(rpc::router_info_keys::UDP_LOW_CAPACITY_PEERS) {
        result.insert(
            rpc::router_info_keys::UDP_LOW_CAPACITY_PEERS.to_string(),
            serde_json::json!(udp.low_capacity_peers),
        );
    }
    if key_set.contains(rpc::router_info_keys::UDP_ON_DEMAND_PEERS) {
        result.insert(
            rpc::router_info_keys::UDP_ON_DEMAND_PEERS.to_string(),
            serde_json::json!(udp.on_demand_peers),
        );
    }
    if key_set.contains(rpc::router_info_keys::UDP_STANDARD_PEERS) {
        result.insert(
            rpc::router_info_keys::UDP_STANDARD_PEERS.to_string(),
            serde_json::json!(udp.standard_peers),
        );
    }
    if key_set.contains(rpc::router_info_keys::UDP_UNREACHABLE_PEERS) {
        result.insert(
            rpc::router_info_keys::UDP_UNREACHABLE_PEERS.to_string(),
            serde_json::json!(udp.unreachable_peers),
        );
    }
    if key_set.contains(rpc::router_info_keys::UDP_TOTAL_PEERS) {
        result.insert(
            rpc::router_info_keys::UDP_TOTAL_PEERS.to_string(),
            serde_json::json!(udp.total_peers),
        );
    }
    if key_set.contains(rpc::router_info_keys::UDP_CURRENT_PEERS) {
        result.insert(
            rpc::router_info_keys::UDP_CURRENT_PEERS.to_string(),
            serde_json::json!(udp.current_peers),
        );
    }
}

/// Resolve TCP transport selectors into response entries.
fn resolve_tcp_selectors(
    result: &mut serde_json::Map<String, serde_json::Value>,
    key_set: &HashSet<&str>,
    tcp: &crate::i2pcontrol::router_info::TcpSnapshot,
) {
    if key_set.contains(rpc::router_info_keys::TCP_ACTIVE) {
        result.insert(
            rpc::router_info_keys::TCP_ACTIVE.to_string(),
            serde_json::json!(tcp.active),
        );
    }
    if key_set.contains(rpc::router_info_keys::TCP_INTEGRATED_PEERS) {
        result.insert(
            rpc::router_info_keys::TCP_INTEGRATED_PEERS.to_string(),
            serde_json::json!(tcp.integrated_peers),
        );
    }
    if key_set.contains(rpc::router_info_keys::TCP_FIREWALLED) {
        result.insert(
            rpc::router_info_keys::TCP_FIREWALLED.to_string(),
            serde_json::json!(tcp.firewalled),
        );
    }
    if key_set.contains(rpc::router_info_keys::TCP_HOSTS) {
        result.insert(
            rpc::router_info_keys::TCP_HOSTS.to_string(),
            serde_json::json!(tcp.hosts),
        );
    }
    if key_set.contains(rpc::router_info_keys::TCP_STATUS) {
        result.insert(
            rpc::router_info_keys::TCP_STATUS.to_string(),
            serde_json::json!(tcp.status),
        );
    }
    if key_set.contains(rpc::router_info_keys::TCP_VERSION) {
        result.insert(
            rpc::router_info_keys::TCP_VERSION.to_string(),
            serde_json::json!(tcp.version),
        );
    }
}

/// Resolve NetDB selectors into response entries.
fn resolve_netdb_selectors(
    result: &mut serde_json::Map<String, serde_json::Value>,
    key_set: &HashSet<&str>,
    netdb: &crate::i2pcontrol::router_info::NetDbSnapshot,
) {
    type NetDbMapping = (
        &'static str,
        fn(&crate::i2pcontrol::router_info::NetDbSnapshot) -> serde_json::Value,
    );
    let mappings: &[NetDbMapping] = &[
        (rpc::router_info_keys::NETDB_ACTIVE, |n| {
            serde_json::json!(n.active)
        }),
        (rpc::router_info_keys::NETDB_ACTIVE_PROFILES, |n| {
            serde_json::json!(n.active_profiles)
        }),
        (rpc::router_info_keys::NETDB_HIGHEST_VERSION, |n| {
            serde_json::json!(n.highest_version)
        }),
        (rpc::router_info_keys::NETDB_KNOWN_PROFILES, |n| {
            serde_json::json!(n.known_profiles)
        }),
        (rpc::router_info_keys::NETDB_NEW_PROFILES, |n| {
            serde_json::json!(n.new_profiles)
        }),
        (rpc::router_info_keys::NETDB_ACTIVE_ROUTERS, |n| {
            serde_json::json!(n.active_routers)
        }),
        (rpc::router_info_keys::NETDB_BANLIST_SIZE, |n| {
            serde_json::json!(n.banlist_size)
        }),
        (rpc::router_info_keys::NETDB_LEASE_SETS, |n| {
            serde_json::json!(n.lease_sets)
        }),
        (rpc::router_info_keys::NETDB_EXPLORATORY_PEERS, |n| {
            serde_json::json!(n.exploratory_peers)
        }),
        (rpc::router_info_keys::NETDB_FAST_PEERS, |n| {
            serde_json::json!(n.fast_peers)
        }),
        (rpc::router_info_keys::NETDB_HIGH_CAPACITY_PEERS, |n| {
            serde_json::json!(n.high_capacity_peers)
        }),
        (rpc::router_info_keys::NETDB_STANDARD_PEERS, |n| {
            serde_json::json!(n.standard_peers)
        }),
        (rpc::router_info_keys::NETDB_LOW_CAPACITY_PEERS, |n| {
            serde_json::json!(n.low_capacity_peers)
        }),
        (rpc::router_info_keys::NETDB_KNOWN_ACTIVE, |n| {
            serde_json::json!(
                n.active_fast_profiles
                    + n.active_high_capacity_profiles
                    + n.active_standard_profiles
                    + n.active_low_capacity_profiles
            )
        }),
        (rpc::router_info_keys::NETDB_KNOWN_IDLE, |n| {
            serde_json::json!(
                n.idle_fast_profiles
                    + n.idle_high_capacity_profiles
                    + n.idle_standard_profiles
                    + n.idle_low_capacity_profiles
            )
        }),
        (rpc::router_info_keys::NETDB_KNOWN_USED, |n| {
            serde_json::json!(n.used_peers)
        }),
        (rpc::router_info_keys::NETDB_KNOWN_VANILLA, |n| {
            serde_json::json!(n.total_reject_profiles)
        }),
        (rpc::router_info_keys::NETDB_KNOWN_VOLATILE, |n| {
            serde_json::json!(n.volatile_peers)
        }),
        (rpc::router_info_keys::NETDB_USED_PEERS, |n| {
            serde_json::json!(n.used_peers)
        }),
        (rpc::router_info_keys::NETDB_VOLATILE_PEERS, |n| {
            serde_json::json!(n.volatile_peers)
        }),
        (rpc::router_info_keys::NETDB_PEER_PROFILES, |n| {
            serde_json::json!(n.total_reject_profiles)
        }),
        (rpc::router_info_keys::NETDB_PLAINTEXT_PEERS, |_n| {
            serde_json::json!(0)
        }),
        (rpc::router_info_keys::NETDB_TUNNELS, |_n| {
            serde_json::json!(0)
        }),
    ];

    for (key, extractor) in mappings {
        if key_set.contains(key) {
            result.insert(key.to_string(), extractor(netdb));
        }
    }
}

/// Resolve bandwidth selectors into response entries.
fn resolve_bw_selectors(
    result: &mut serde_json::Map<String, serde_json::Value>,
    key_set: &HashSet<&str>,
    transport: &crate::i2pcontrol::router_info::TransportBytes,
    _transit: &crate::i2pcontrol::router_info::TransitBytes,
    recent: &crate::i2pcontrol::router_info::RecentTransitTraffic,
) {
    // Cumulative total
    if key_set.contains(rpc::router_info_keys::BW_INBOUND_TOTAL) {
        result.insert(
            rpc::router_info_keys::BW_INBOUND_TOTAL.to_string(),
            serde_json::json!(transport.received),
        );
    }
    if key_set.contains(rpc::router_info_keys::BW_OUTBOUND_TOTAL) {
        result.insert(
            rpc::router_info_keys::BW_OUTBOUND_TOTAL.to_string(),
            serde_json::json!(transport.sent),
        );
    }

    // Rolling 1-second
    if key_set.contains(rpc::router_info_keys::BW_INBOUND_1S) {
        result.insert(
            rpc::router_info_keys::BW_INBOUND_1S.to_string(),
            serde_json::json!(recent.inbound_1s),
        );
    }
    if key_set.contains(rpc::router_info_keys::BW_OUTBOUND_1S) {
        result.insert(
            rpc::router_info_keys::BW_OUTBOUND_1S.to_string(),
            serde_json::json!(recent.outbound_1s),
        );
    }

    // Rolling 15-second
    if key_set.contains(rpc::router_info_keys::BW_INBOUND_15S) {
        result.insert(
            rpc::router_info_keys::BW_INBOUND_15S.to_string(),
            serde_json::json!(recent.inbound_15s),
        );
    }
    if key_set.contains(rpc::router_info_keys::BW_OUTBOUND_15S) {
        result.insert(
            rpc::router_info_keys::BW_OUTBOUND_15S.to_string(),
            serde_json::json!(recent.outbound_15s),
        );
    }

    // Rolling 1-minute (placeholder: same as 15s for now)
    if key_set.contains(rpc::router_info_keys::BW_INBOUND_1M) {
        result.insert(
            rpc::router_info_keys::BW_INBOUND_1M.to_string(),
            serde_json::json!(recent.inbound_15s),
        );
    }
    if key_set.contains(rpc::router_info_keys::BW_OUTBOUND_1M) {
        result.insert(
            rpc::router_info_keys::BW_OUTBOUND_1M.to_string(),
            serde_json::json!(recent.outbound_15s),
        );
    }

    // Rolling 1-hour (placeholder: total / 3600)
    if key_set.contains(rpc::router_info_keys::BW_INBOUND_1H) {
        let rate = transport.received / 3600;
        result.insert(
            rpc::router_info_keys::BW_INBOUND_1H.to_string(),
            serde_json::json!(rate),
        );
    }
    if key_set.contains(rpc::router_info_keys::BW_OUTBOUND_1H) {
        let rate = transport.sent / 3600;
        result.insert(
            rpc::router_info_keys::BW_OUTBOUND_1H.to_string(),
            serde_json::json!(rate),
        );
    }

    // Rolling 1-day (placeholder: total / 86400)
    if key_set.contains(rpc::router_info_keys::BW_INBOUND_1D) {
        let rate = transport.received / 86400;
        result.insert(
            rpc::router_info_keys::BW_INBOUND_1D.to_string(),
            serde_json::json!(rate),
        );
    }
    if key_set.contains(rpc::router_info_keys::BW_OUTBOUND_1D) {
        let rate = transport.sent / 86400;
        result.insert(
            rpc::router_info_keys::BW_OUTBOUND_1D.to_string(),
            serde_json::json!(rate),
        );
    }
}

/// Resolve tunnel selectors into response entries.
fn resolve_tunnel_selectors(
    result: &mut serde_json::Map<String, serde_json::Value>,
    key_set: &HashSet<&str>,
    summary: &crate::i2pcontrol::router_info::TunnelSummary,
) {
    if key_set.contains(rpc::router_info_keys::TUNNELS_PARTICIPATING) {
        result.insert(
            rpc::router_info_keys::TUNNELS_PARTICIPATING.to_string(),
            serde_json::json!(summary.active_participating),
        );
    }
    if key_set.contains(rpc::router_info_keys::TUNNELS_EXPLORATORY_IN) {
        result.insert(
            rpc::router_info_keys::TUNNELS_EXPLORATORY_IN.to_string(),
            serde_json::json!(summary.exploratory_inbound),
        );
    }
    if key_set.contains(rpc::router_info_keys::TUNNELS_EXPLORATORY_OUT) {
        result.insert(
            rpc::router_info_keys::TUNNELS_EXPLORATORY_OUT.to_string(),
            serde_json::json!(summary.exploratory_outbound),
        );
    }
    if key_set.contains(rpc::router_info_keys::TUNNELS_CLIENT_IN) {
        result.insert(
            rpc::router_info_keys::TUNNELS_CLIENT_IN.to_string(),
            serde_json::json!(summary.client_inbound),
        );
    }
    if key_set.contains(rpc::router_info_keys::TUNNELS_CLIENT_OUT) {
        result.insert(
            rpc::router_info_keys::TUNNELS_CLIENT_OUT.to_string(),
            serde_json::json!(summary.client_outbound),
        );
    }
    if key_set.contains(rpc::router_info_keys::TUNNELS_CONFIGURED) {
        result.insert(
            rpc::router_info_keys::TUNNELS_CONFIGURED.to_string(),
            serde_json::json!(summary.configured),
        );
    }
    if key_set.contains(rpc::router_info_keys::TUNNELS_QUEUE) {
        result.insert(
            rpc::router_info_keys::TUNNELS_QUEUE.to_string(),
            serde_json::json!(summary.queue_depth),
        );
    }
}

/// Resolve peer selectors into response entries.
async fn resolve_peer_selectors(
    result: &mut serde_json::Map<String, serde_json::Value>,
    key_set: &HashSet<&str>,
    router_info: &dyn RouterInfoControl,
) -> Result<(), String> {
    if key_set.contains(rpc::router_info_keys::PEERS_KNOWN_COUNT) {
        let peers = router_info.known_peers().await;
        let count = peers.len();
        result.insert(
            rpc::router_info_keys::PEERS_KNOWN_COUNT.to_string(),
            serde_json::json!(count),
        );
    }
    if key_set.contains(rpc::router_info_keys::PEERS_KNOWN) {
        let peers = router_info.known_peers().await;
        let ids: Vec<String> = peers.iter().map(|p| p.id.clone()).collect();
        result.insert(
            rpc::router_info_keys::PEERS_KNOWN.to_string(),
            serde_json::json!(ids),
        );
    }
    if key_set.contains(rpc::router_info_keys::PEERS_ACTIVE_COUNT) {
        let peers = router_info.active_peers().await;
        let count = peers.len();
        result.insert(
            rpc::router_info_keys::PEERS_ACTIVE_COUNT.to_string(),
            serde_json::json!(count),
        );
    }
    if key_set.contains(rpc::router_info_keys::PEERS_ACTIVE) {
        let peers = router_info.active_peers().await;
        let ids: Vec<String> = peers.iter().map(|p| p.id.clone()).collect();
        result.insert(
            rpc::router_info_keys::PEERS_ACTIVE.to_string(),
            serde_json::json!(ids),
        );
    }
    if key_set.contains(rpc::router_info_keys::PEERS_ROUTER_INFO) {
        // Requires a specific peer ID in params; return empty if not provided
        result.insert(
            rpc::router_info_keys::PEERS_ROUTER_INFO.to_string(),
            serde_json::json!(null),
        );
    }
    if key_set.contains(rpc::router_info_keys::PEERS_BANNED) {
        let banned = router_info.banned_peers().await;
        let entries: Vec<serde_json::Value> = banned
            .iter()
            .map(|b| {
                serde_json::json!({
                    "id": b.id,
                    "reason": b.reason,
                    "expiresAt": b.expires_at,
                })
            })
            .collect();
        result.insert(
            rpc::router_info_keys::PEERS_BANNED.to_string(),
            serde_json::json!(entries),
        );
    }
    if key_set.contains(rpc::router_info_keys::PEERS_BANNED_COUNT) {
        let banned = router_info.banned_peers().await;
        let count = banned.len();
        result.insert(
            rpc::router_info_keys::PEERS_BANNED_COUNT.to_string(),
            serde_json::json!(count),
        );
    }
    if key_set.contains(rpc::router_info_keys::PEERS_LIMITS) {
        let limits = router_info.peer_limits().await;
        result.insert(
            rpc::router_info_keys::PEERS_LIMITS.to_string(),
            serde_json::json!({
                "inbound": limits.configured_inbound,
                "outbound": limits.configured_outbound,
            }),
        );
    }
    if key_set.contains(rpc::router_info_keys::PEERS_ACTIVE_STATS) {
        let stats = router_info.active_peer_stats().await;
        let entries: Vec<serde_json::Value> = stats
            .iter()
            .map(|s| {
                serde_json::json!({
                    "peerId": s.peer_id,
                    "direction": s.direction,
                    "state": s.state,
                    "bytesReceived": s.bytes_received,
                    "bytesSent": s.bytes_sent,
                })
            })
            .collect();
        result.insert(
            rpc::router_info_keys::PEERS_ACTIVE_STATS.to_string(),
            serde_json::json!(entries),
        );
    }
    Ok(())
}

fn resolve_id(id: &Option<RequestId>) -> RequestId {
    id.clone().unwrap_or(RequestId::Null)
}

fn error_response(id: RequestId, code: i32, message: impl Into<String>) -> serde_json::Value {
    serde_json::to_value(JsonRpcErrorResponse::new(id, code, message)).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i2pcontrol::router_info::*;
    use crate::i2pcontrol::rpc::JsonRpcRequest;

    fn test_request(selectors: serde_json::Value) -> JsonRpcRequest {
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "RouterInfo".to_string(),
            params: Some(serde_json::json!({"Selector": selectors}).as_object().cloned().unwrap()),
            id: Some(rpc::RequestId::Number(1)),
        }
    }

    #[tokio::test]
    async fn handle_router_info_empty_selector() {
        let ri = FakeRouterInfoControl::new();
        let ab = crate::i2pcontrol::control_plane::FakeAddressBookControl::new();
        let req = test_request(serde_json::json!({}));
        let resp = handle_router_info(&ri, &ab, &req).await;
        assert_eq!(resp["jsonrpc"], "2.0");
        assert!(resp["result"].is_object());
        assert_eq!(resp["result"].as_object().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn handle_router_info_version_only() {
        let ri = FakeRouterInfoControl::new();
        ri.set_version("Test 2.0".to_string());
        let ab = crate::i2pcontrol::control_plane::FakeAddressBookControl::new();
        let req = test_request(serde_json::json!({"i2p.router.version": true}));
        let resp = handle_router_info(&ri, &ab, &req).await;
        let result = resp["result"].as_object().unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result["i2p.router.version"], "Test 2.0");
    }

    #[tokio::test]
    async fn handle_router_info_uptime_and_version() {
        let ri = FakeRouterInfoControl::new();
        ri.set_version("Emissary 0.5.0".to_string());
        ri.set_uptime_ms(120000);
        let ab = crate::i2pcontrol::control_plane::FakeAddressBookControl::new();
        let req = test_request(serde_json::json!({
            "i2p.router.version": true,
            "i2p.router.uptime": true
        }));
        let resp = handle_router_info(&ri, &ab, &req).await;
        let result = resp["result"].as_object().unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result["i2p.router.version"], "Emissary 0.5.0");
        assert_eq!(result["i2p.router.uptime"], 120000);
    }

    #[tokio::test]
    async fn handle_router_info_unknown_selector() {
        let ri = FakeRouterInfoControl::new();
        let ab = crate::i2pcontrol::control_plane::FakeAddressBookControl::new();
        let req = test_request(serde_json::json!({"unknown.selector": true}));
        let resp = handle_router_info(&ri, &ab, &req).await;
        assert_eq!(resp["error"]["code"], -32602);
    }

    #[tokio::test]
    async fn handle_router_info_false_selector_ignored() {
        let ri = FakeRouterInfoControl::new();
        ri.set_version("Test".to_string());
        let ab = crate::i2pcontrol::control_plane::FakeAddressBookControl::new();
        let req = test_request(serde_json::json!({"i2p.router.version": false}));
        let resp = handle_router_info(&ri, &ab, &req).await;
        let result = resp["result"].as_object().unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn handle_router_info_missing_selector_param() {
        let ri = FakeRouterInfoControl::new();
        let ab = crate::i2pcontrol::control_plane::FakeAddressBookControl::new();
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "RouterInfo".to_string(),
            params: Some(serde_json::json!({"Token": "abc"}).as_object().cloned().unwrap()),
            id: Some(rpc::RequestId::Number(1)),
        };
        let resp = handle_router_info(&ri, &ab, &req).await;
        assert_eq!(resp["error"]["code"], -32602);
    }

    #[tokio::test]
    async fn handle_router_info_udp_selectors() {
        let ri = FakeRouterInfoControl::new();
        ri.set_udp(UdpSnapshot {
            active: true,
            cookie_active: true,
            integrated_peers: 5,
            firewalled: false,
            hidden: false,
            ..Default::default()
        });
        let ab = crate::i2pcontrol::control_plane::FakeAddressBookControl::new();
        let req = test_request(serde_json::json!({
            "i2p.router.udp.active": true,
            "i2p.router.udp.integratedPeers": true,
        }));
        let resp = handle_router_info(&ri, &ab, &req).await;
        let result = resp["result"].as_object().unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result["i2p.router.udp.active"], true);
        assert_eq!(result["i2p.router.udp.integratedPeers"], 5);
    }

    #[tokio::test]
    async fn handle_router_info_tcp_selectors() {
        let ri = FakeRouterInfoControl::new();
        ri.set_tcp(TcpSnapshot {
            active: true,
            integrated_peers: 3,
            firewalled: false,
            hosts: "0.0.0.0:4444".to_string(),
            status: "Active".to_string(),
            version: "NTCP2".to_string(),
        });
        let ab = crate::i2pcontrol::control_plane::FakeAddressBookControl::new();
        let req = test_request(serde_json::json!({
            "i2p.router.tcp.active": true,
            "i2p.router.tcp.status": true,
        }));
        let resp = handle_router_info(&ri, &ab, &req).await;
        let result = resp["result"].as_object().unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result["i2p.router.tcp.active"], true);
        assert_eq!(result["i2p.router.tcp.status"], "Active");
    }

    #[tokio::test]
    async fn handle_router_info_netdb_selectors() {
        let ri = FakeRouterInfoControl::new();
        ri.set_netdb(NetDbSnapshot {
            active: true,
            known_profiles: 100,
            active_profiles: 50,
            ..Default::default()
        });
        let ab = crate::i2pcontrol::control_plane::FakeAddressBookControl::new();
        let req = test_request(serde_json::json!({
            "i2p.router.netdb.active": true,
            "i2p.router.netdb.knownProfiles": true,
            "i2p.router.netdb.activeProfiles": true,
        }));
        let resp = handle_router_info(&ri, &ab, &req).await;
        let result = resp["result"].as_object().unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(result["i2p.router.netdb.active"], true);
        assert_eq!(result["i2p.router.netdb.knownProfiles"], 100);
        assert_eq!(result["i2p.router.netdb.activeProfiles"], 50);
    }

    #[tokio::test]
    async fn handle_router_info_bw_selectors() {
        let ri = FakeRouterInfoControl::new();
        ri.set_transport_bytes(TransportBytes {
            received: 1000000,
            sent: 500000,
        });
        let ab = crate::i2pcontrol::control_plane::FakeAddressBookControl::new();
        let req = test_request(serde_json::json!({
            "i2p.router.bw.inbound.total": true,
            "i2p.router.bw.outbound.total": true,
        }));
        let resp = handle_router_info(&ri, &ab, &req).await;
        let result = resp["result"].as_object().unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result["i2p.router.bw.inbound.total"], 1000000);
        assert_eq!(result["i2p.router.bw.outbound.total"], 500000);
    }

    #[tokio::test]
    async fn handle_router_info_unrelated_keys_absent() {
        let ri = FakeRouterInfoControl::new();
        ri.set_version("Test".to_string());
        let ab = crate::i2pcontrol::control_plane::FakeAddressBookControl::new();
        let req = test_request(serde_json::json!({
            "i2p.router.version": true
        }));
        let resp = handle_router_info(&ri, &ab, &req).await;
        let result = resp["result"].as_object().unwrap();
        // Only version should be present, no unrelated keys
        assert_eq!(result.len(), 1);
        assert!(result.contains_key("i2p.router.version"));
        assert!(!result.contains_key("i2p.router.uptime"));
        assert!(!result.contains_key("i2p.router.identity"));
    }

    #[tokio::test]
    async fn handle_router_info_network_status() {
        let ri = FakeRouterInfoControl::new();
        ri.set_network(NetworkSnapshot {
            ipv4_status: NetworkStatus::Ok,
            ipv6_status: NetworkStatus::Firewalled,
            ..Default::default()
        });
        let ab = crate::i2pcontrol::control_plane::FakeAddressBookControl::new();
        let req = test_request(serde_json::json!({
            "i2p.router.net.bw.inbound": true,
            "i2p.router.net.bw.outbound": true,
        }));
        let resp = handle_router_info(&ri, &ab, &req).await;
        let result = resp["result"].as_object().unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result["i2p.router.net.bw.inbound"], "OK");
        assert_eq!(result["i2p.router.net.bw.outbound"], "Firewalled");
    }
}
