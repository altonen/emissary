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

use serde::{Deserialize, Serialize};

/// JSON-RPC 2.0 request ID.
///
/// Preserves string or numeric request IDs as required by JSON-RPC 2.0.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(untagged)]
pub enum RequestId {
    String(String),
    Number(i64),
    Null,
}

/// JSON-RPC 2.0 request.
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct JsonRpcRequest {
    /// Must be exactly `"2.0"`.
    pub jsonrpc: String,

    /// Method name.
    pub method: String,

    /// Named parameters (object). Positional params are rejected.
    #[serde(default)]
    pub params: Option<serde_json::Map<String, serde_json::Value>>,

    /// Request ID. Null for notifications (no response sent).
    #[serde(default)]
    pub id: Option<RequestId>,
}

/// JSON-RPC 2.0 success response envelope.
#[derive(Debug, Clone, Serialize)]
pub struct JsonRpcSuccess {
    pub jsonrpc: &'static str,
    pub id: RequestId,
    pub result: serde_json::Value,
}

/// JSON-RPC 2.0 error object.
#[derive(Debug, Clone, Serialize)]
pub struct JsonRpcErrorObject {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// JSON-RPC 2.0 error response envelope.
#[derive(Debug, Clone, Serialize)]
pub struct JsonRpcErrorResponse {
    pub jsonrpc: &'static str,
    pub id: RequestId,
    pub error: JsonRpcErrorObject,
}

/// JSON-RPC 2.0 error codes.
pub mod error_codes {
    /// Parse error: invalid JSON.
    pub const PARSE_ERROR: i32 = -32700;

    /// Invalid request: valid JSON but not a valid JSON-RPC request.
    pub const INVALID_REQUEST: i32 = -32600;

    /// Method not found.
    pub const METHOD_NOT_FOUND: i32 = -32601;

    /// Invalid params.
    pub const INVALID_PARAMS: i32 = -32602;

    /// Internal error.
    pub const INTERNAL_ERROR: i32 = -32603;

    /// Application-defined error (e.g., authentication failure).
    pub const APP_ERROR: i32 = -1;
}

impl JsonRpcSuccess {
    /// Create a success response.
    pub fn new(id: RequestId, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            result,
        }
    }
}

impl JsonRpcErrorResponse {
    /// Create an error response.
    pub fn new(id: RequestId, code: i32, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            error: JsonRpcErrorObject {
                code,
                message: message.into(),
                data: None,
            },
        }
    }

    /// Create an error response with data.
    #[allow(dead_code)]
    pub fn with_data(
        id: RequestId,
        code: i32,
        message: impl Into<String>,
        data: serde_json::Value,
    ) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            error: JsonRpcErrorObject {
                code,
                message: message.into(),
                data: Some(data),
            },
        }
    }
}

/// Parse a JSON-RPC request from a raw body string.
///
/// Returns the parsed request or an error response.
pub fn parse_request(body: &str) -> Result<JsonRpcRequest, JsonRpcErrorResponse> {
    let raw: serde_json::Value = serde_json::from_str(body).map_err(|e| {
        JsonRpcErrorResponse::new(
            RequestId::Null,
            error_codes::PARSE_ERROR,
            format!("Parse error: {e}"),
        )
    })?;

    // Must be a JSON object at top level
    let obj = raw.as_object().ok_or_else(|| {
        JsonRpcErrorResponse::new(
            RequestId::Null,
            error_codes::INVALID_REQUEST,
            "Request must be a JSON object",
        )
    })?;

    // Extract jsonrpc version
    let jsonrpc = obj.get("jsonrpc").and_then(|v| v.as_str()).ok_or_else(|| {
        JsonRpcErrorResponse::new(
            RequestId::Null,
            error_codes::INVALID_REQUEST,
            "Missing 'jsonrpc' field",
        )
    })?;

    if jsonrpc != "2.0" {
        return Err(JsonRpcErrorResponse::new(
            RequestId::Null,
            error_codes::INVALID_REQUEST,
            "jsonrpc must be exactly \"2.0\"",
        ));
    }

    // Extract method
    let method = obj.get("method").and_then(|v| v.as_str()).ok_or_else(|| {
        JsonRpcErrorResponse::new(
            RequestId::Null,
            error_codes::INVALID_REQUEST,
            "Missing 'method' field",
        )
    })?;

    if method.is_empty() {
        return Err(JsonRpcErrorResponse::new(
            RequestId::Null,
            error_codes::INVALID_REQUEST,
            "Method must not be empty",
        ));
    }

    // Extract id
    let id = obj.get("id").cloned().map(RequestId::from_json);

    // Extract params — must be an object if present (named params)
    let params = match obj.get("params") {
        Some(serde_json::Value::Object(map)) => Some(map.clone()),
        Some(_) => {
            return Err(JsonRpcErrorResponse::new(
                id.unwrap_or(RequestId::Null),
                error_codes::INVALID_PARAMS,
                "Params must be a JSON object (named parameters)",
            ));
        }
        None => None,
    };

    Ok(JsonRpcRequest {
        jsonrpc: jsonrpc.to_string(),
        method: method.to_string(),
        params,
        id,
    })
}

impl RequestId {
    fn from_json(val: serde_json::Value) -> Self {
        match val {
            serde_json::Value::String(s) => RequestId::String(s),
            serde_json::Value::Number(n) => RequestId::Number(n.as_i64().unwrap_or(0)),
            serde_json::Value::Null => RequestId::Null,
            _ => RequestId::Null,
        }
    }
}

/// I2PControl method names.
#[allow(dead_code)]
pub mod methods {
    /// Authenticate method.
    pub const AUTHENTICATE: &str = "Authenticate";

    /// RouterInfo method.
    pub const ROUTER_INFO: &str = "RouterInfo";

    /// AddressBook method.
    pub const ADDRESS_BOOK: &str = "AddressBook";

    /// TunnelManager method.
    pub const TUNNEL_MANAGER: &str = "TunnelManager";

    /// ClientServicesInfo method.
    pub const CLIENT_SERVICES_INFO: &str = "ClientServicesInfo";

    /// GetKeys method.
    pub const GET_KEYS: &str = "GetKeys";

    /// SetConfig method.
    pub const SET_CONFIG: &str = "SetConfig";

    /// SetSubscriptions method.
    pub const SET_SUBSCRIPTIONS: &str = "SetSubscriptions";
}

/// Proposal 170 tunnel types.
#[allow(dead_code)]
pub mod tunnel_types {
    pub const CLIENT: &str = "client";
    pub const HTTP_CLIENT: &str = "httpclient";
    pub const IRC_CLIENT: &str = "ircclient";
    pub const SOCKS: &str = "socks";
    pub const SOCKS_IRC: &str = "socksirc";
    pub const CONNECT_CLIENT: &str = "connectclient";
    pub const STREAMR_CLIENT: &str = "streamrclient";
    pub const SERVER: &str = "server";
    pub const HTTP_SERVER: &str = "httpserver";
    pub const HTTP_BIDIR_SERVER: &str = "httpbidirserver";
    pub const IRC_SERVER: &str = "ircserver";
    pub const STREAMR_SERVER: &str = "streamrserver";

    /// All valid Proposal 170 tunnel types.
    pub const ALL: &[&str] = &[
        CLIENT,
        HTTP_CLIENT,
        IRC_CLIENT,
        SOCKS,
        SOCKS_IRC,
        CONNECT_CLIENT,
        STREAMR_CLIENT,
        SERVER,
        HTTP_SERVER,
        HTTP_BIDIR_SERVER,
        IRC_SERVER,
        STREAMR_SERVER,
    ];
}

/// TunnelManager actions.
#[allow(dead_code)]
pub mod tunnel_actions {
    /// Emissary compatibility extension; not part of Proposal 170's
    /// canonical action vocabulary.
    pub const LIST: &str = "List";
    pub const CREATE: &str = "Create";
    pub const EDIT: &str = "Edit";
    pub const GET: &str = "Get";
    pub const DELETE: &str = "Delete";
    pub const START: &str = "Start";
    pub const STOP: &str = "Stop";
    pub const RESTART: &str = "Restart";

    /// Canonical Proposal 170 TunnelManager actions.
    pub const CANONICAL: &[&str] = &[
        "create", "edit", "get", "start", "stop", "restart", "delete",
    ];

    /// Already-shipped Emissary action aliases.
    pub const COMPATIBILITY: &[&str] = &[
        "List", "Create", "Edit", "Get", "Start", "Stop", "Restart", "Delete",
    ];
}

/// AddressBook books.
#[allow(dead_code)]
pub mod address_books {
    pub const PRIVATE: &str = "private";
    pub const LOCAL: &str = "local";
    pub const ROUTER: &str = "router";
    pub const PUBLISHED: &str = "published";

    pub const ALL: &[&str] = &[PRIVATE, LOCAL, ROUTER, PUBLISHED];
}

/// AddressBook request modes.
#[allow(dead_code)]
pub mod address_book_requests {
    pub const LIST: &str = "List";
    pub const LOOKUP: &str = "Lookup";
    pub const ADD: &str = "Add";
    pub const UPDATE: &str = "Update";
    pub const DELETE: &str = "Delete";
}

/// Authenticate request parameters.
#[derive(Debug, Clone, Deserialize)]
pub struct AuthenticateParams {
    /// API version. Must be 1 or 2.
    #[serde(rename = "API")]
    pub api: Option<i32>,

    /// Username. Must be "i2pcontrol".
    #[serde(rename = "Username")]
    pub username: Option<String>,

    /// Password.
    #[serde(rename = "Password")]
    pub password: Option<String>,
}

/// Authenticate result.
#[derive(Debug, Clone, Serialize)]
#[allow(non_snake_case)]
pub struct AuthenticateResult {
    /// Authentication token.
    pub Token: String,

    /// Negotiated API version.
    pub API: String,
}

/// Proposal 170 RouterInfo selector keys.
/// https://i2p.net/en/proposals/170-i2pcontrol-expansion/
#[allow(dead_code)]
pub mod router_info_keys {
    // --- UDP transport selectors ---
    pub const UDP_ACTIVE: &str = "i2p.router.udp.active";
    pub const UDP_COOKIE_ACTIVE: &str = "i2p.router.udp.cookie.active";
    pub const UDP_INTEGRATED_PEERS: &str = "i2p.router.udp.integratedPeers";
    pub const UDP_FIREWALLED: &str = "i2p.router.udp.firewalled";
    pub const UDP_HIDDEN: &str = "i2p.router.udp.hidden";
    pub const UDP_COINFICIENT_PEERS: &str = "i2p.router.udp.coinficientPeers";
    pub const UDP_CRITICAL_PEERS: &str = "i2p.router.udp.criticalPeers";
    pub const UDP_FAST_PEERS: &str = "i2p.router.udp.fastPeers";
    pub const UDP_HIGH_CAPACITY_PEERS: &str = "i2p.router.udp.highCapacityPeers";
    pub const UDP_INTERLEAVED_PEERS: &str = "i2p.router.udp.interleavedPeers";
    pub const UDP_LIT_PEERS: &str = "i2p.router.udp.litPeers";
    pub const UDP_LOW_CAPACITY_PEERS: &str = "i2p.router.udp.lowCapacityPeers";
    pub const UDP_ON_DEMAND_PEERS: &str = "i2p.router.udp.onDemandPeers";
    pub const UDP_PEER_STATS: &str = "i2p.router.udp.peerStats";
    pub const UDP_STANDARD_PEERS: &str = "i2p.router.udp.standardPeers";
    pub const UDP_UNREACHABLE_PEERS: &str = "i2p.router.udp.unreachablePeers";
    pub const UDP_TOTAL_PEERS: &str = "i2p.router.udp.totalPeers";
    pub const UDP_CURRENT_PEERS: &str = "i2p.router.udp.currentPeers";

    // --- General router selectors ---
    pub const VERSION: &str = "i2p.router.version";
    pub const UPTIME: &str = "i2p.router.uptime";

    // --- NetDB selectors ---
    pub const NETDB_ACTIVE: &str = "i2p.router.netdb.active";
    pub const NETDB_ACTIVE_PROFILES: &str = "i2p.router.netdb.activeProfiles";
    pub const NETDB_HIGHEST_VERSION: &str = "i2p.router.netdb.highestVersion";
    pub const NETDB_KNOWN_PROFILES: &str = "i2p.router.netdb.knownProfiles";
    pub const NETDB_NEW_PROFILES: &str = "i2p.router.netdb.newProfiles";
    pub const NETDB_ACTIVE_ROUTERS: &str = "i2p.router.netdb.activeRouters";
    pub const NETDB_ALREADY_EXPERIENCED_PEERS: &str = "i2p.router.netdb.alreadyExperiencedPeers";
    pub const NETDB_BANLIST_SIZE: &str = "i2p.router.netdb.banlistSize";
    pub const NETDB_EXPLORATORY_PEERS: &str = "i2p.router.netdb.exploratoryPeers";
    pub const NETDB_FAST_PEERS: &str = "i2p.router.netdb.fastPeers";
    pub const NETDB_HIGH_CAPACITY_PEERS: &str = "i2p.router.netdb.highCapacityPeers";
    pub const NETDB_IS_BACKLOGGED: &str = "i2p.router.netdb.isBacklogged";
    pub const NETDB_KNOWN_ACTIVE: &str = "i2p.router.netdb.knownActive";
    pub const NETDB_KNOWN_IDLE: &str = "i2p.router.netdb.knownIdle";
    pub const NETDB_KNOWN_USED: &str = "i2p.router.netdb.knownUsed";
    pub const NETDB_KNOWN_VANILLA: &str = "i2p.router.netdb.knownVanilla";
    pub const NETDB_KNOWN_VOLATILE: &str = "i2p.router.netdb.knownVolatile";
    pub const NETDB_LAST_EXPLORED: &str = "i2p.router.netdb.lastExplored";
    pub const NETDB_LAST_PROFILE_LOOKUP: &str = "i2p.router.netdb.lastProfileLookup";
    pub const NETDB_LAST_ROUTER_LOOKUP: &str = "i2p.router.netdb.lastRouterLookup";
    pub const NETDB_LAST_UNSAVED: &str = "i2p.router.netdb.lastUnsaved";
    pub const NETDB_LEASE_SETS: &str = "i2p.router.netdb.leaseSets";
    pub const NETDB_NEW_ACTIVE: &str = "i2p.router.netdb.newActive";
    pub const NETDB_NEW_IDLE: &str = "i2p.router.netdb.newIdle";
    pub const NETDB_OLD_ACTIVE: &str = "i2p.router.netdb.oldActive";
    pub const NETDB_OLD_IDLE: &str = "i2p.router.netdb.oldIdle";
    pub const NETDB_PEER_PROFILES: &str = "i2p.router.netdb.peerProfiles";
    pub const NETDB_PLAINTEXT_PEERS: &str = "i2p.router.netdb.plaintextPeers";
    pub const NETDB_RESERVE_ACTIVE: &str = "i2p.router.netdb.reserveActive";
    pub const NETDB_RESERVE_ACTIVE_PEERS: &str = "i2p.router.netdb.reserveActivePeers";
    pub const NETDB_RESERVE_HIGH_CAPACITY: &str = "i2p.router.netdb.reserveHighCapacity";
    pub const NETDB_RESERVE_INTEGRATED: &str = "i2p.router.netdb.reserveIntegrated";
    pub const NETDB_RESERVE_KNOWN: &str = "i2p.router.netdb.reserveKnown";
    pub const NETDB_RESERVE_LOOKUP: &str = "i2p.router.netdb.reserveLookup";
    pub const NETDB_RESERVE_PENDING: &str = "i2p.router.netdb.reservePending";
    pub const NETDB_RESERVE_RESERVED: &str = "i2p.router.netdb.reserveReserved";
    pub const NETDB_RESERVE_STANDARD: &str = "i2p.router.netdb.reserveStandard";
    pub const NETDB_RESERVE_TIER2: &str = "i2p.router.netdb.reserveTier2";
    pub const NETDB_RESERVE_USED: &str = "i2p.router.netdb.reserveUsed";
    pub const NETDB_RESERVE_VOLATILE: &str = "i2p.router.netdb.reserveVolatile";
    pub const NETDB_STANDARD_PEERS: &str = "i2p.router.netdb.standardPeers";
    pub const NETDB_LOW_CAPACITY_PEERS: &str = "i2p.router.netdb.lowCapacityPeers";
    pub const NETDB_TUNNELS: &str = "i2p.router.netdb.tunnels";
    pub const NETDB_USED_PEERS: &str = "i2p.router.netdb.usedPeers";
    pub const NETDB_VOLATILE_PEERS: &str = "i2p.router.netdb.volatilePeers";
    pub const NETDB_ADDRESS_BOOKS: &str = "i2p.router.netdb.addressBooks";
    pub const NETDB_ADDRESS_BOOK_ENTRIES: &str = "i2p.router.netdb.addressBookEntries";
    pub const NETDB_ADDRESS_BOOK_SOURCES: &str = "i2p.router.netdb.addressBookSources";
    pub const NETDB_ADDRESS_BOOK_SUBSCRIPTIONS: &str = "i2p.router.netdb.addressBookSubscriptions";
    pub const NETDB_ADDRESS_BOOK_UPDATES: &str = "i2p.router.netdb.addressBookUpdates";

    // --- Bandwidth selectors ---
    pub const BW_INBOUND_1S: &str = "i2p.router.bw.inbound.1s";
    pub const BW_INBOUND_15S: &str = "i2p.router.bw.inbound.15s";
    pub const BW_INBOUND_1M: &str = "i2p.router.bw.inbound.1m";
    pub const BW_INBOUND_1H: &str = "i2p.router.bw.inbound.1h";
    pub const BW_INBOUND_1D: &str = "i2p.router.bw.inbound.1d";
    pub const BW_INBOUND_TOTAL: &str = "i2p.router.bw.inbound.total";
    pub const BW_OUTBOUND_1S: &str = "i2p.router.bw.outbound.1s";
    pub const BW_OUTBOUND_15S: &str = "i2p.router.bw.outbound.15s";
    pub const BW_OUTBOUND_1M: &str = "i2p.router.bw.outbound.1m";
    pub const BW_OUTBOUND_1H: &str = "i2p.router.bw.outbound.1h";
    pub const BW_OUTBOUND_1D: &str = "i2p.router.bw.outbound.1d";
    pub const BW_OUTBOUND_TOTAL: &str = "i2p.router.bw.outbound.total";

    // --- TCP transport selectors ---
    pub const TCP_ACTIVE: &str = "i2p.router.tcp.active";
    pub const TCP_INTEGRATED_PEERS: &str = "i2p.router.tcp.integratedPeers";
    pub const TCP_FIREWALLED: &str = "i2p.router.tcp.firewalled";
    pub const TCP_HOSTS: &str = "i2p.router.tcp.hosts";
    pub const TCP_STATUS: &str = "i2p.router.tcp.status";
    pub const TCP_VERSION: &str = "i2p.router.tcp.version";

    // --- Identity and network selectors ---
    pub const IDENTITY: &str = "i2p.router.identity";
    pub const NET_BW_INBOUND: &str = "i2p.router.net.bw.inbound";
    pub const NET_BW_OUTBOUND: &str = "i2p.router.net.bw.outbound";

    // --- Router news ---
    pub const ROUTER_NEWS: &str = "i2p.router.news";

    // --- Proposal 170 exact additions ---
    pub const P170_ID: &str = "i2p.router.id";
    pub const P170_CLOCKSKEW: &str = "i2p.router.clockskew";
    pub const P170_INFO: &str = "i2p.router.info";
    pub const P170_LOGS: &str = "i2p.router.logs";
    pub const P170_LOGS_CLEAR: &str = "i2p.router.logs.clear";
    pub const P170_NET_TOTAL_RECEIVED_BYTES: &str = "i2p.router.net.total.received.bytes";
    pub const P170_NET_TOTAL_SENT_BYTES: &str = "i2p.router.net.total.sent.bytes";
    pub const P170_NET_TOTAL_TRANSIT_BYTES: &str = "i2p.router.net.total.transit.bytes";
    pub const P170_NET_BW_TRANSIT_15S: &str = "i2p.router.net.bw.transit.15s";
    pub const P170_NET_TUNNELS_SHARE_RATIO: &str = "i2p.router.net.tunnels.shareratio";
    pub const P170_NET_TUNNELS_PARTICIPATING_INFO: &str =
        "i2p.router.net.tunnels.participating.info";
    pub const P170_NET_TUNNELS_I2PTUNNEL: &str = "i2p.router.net.tunnels.i2ptunnel";
    pub const P170_NET_TUNNELS_EXPLORATORY_INBOUND: &str =
        "i2p.router.net.tunnels.exploratory.inbound";
    pub const P170_NET_TUNNELS_EXPLORATORY_OUTBOUND: &str =
        "i2p.router.net.tunnels.exploratory.outbound";
    pub const P170_NET_TUNNELS_EXPLORATORY_INFO_LIST: &str =
        "i2p.router.net.tunnels.exploratory.info.list";
    pub const P170_NET_TUNNELS_CLIENT_INBOUND: &str = "i2p.router.net.tunnels.client.inbound";
    pub const P170_NET_TUNNELS_CLIENT_OUTBOUND: &str = "i2p.router.net.tunnels.client.outbound";
    pub const P170_NET_TUNNELS_CLIENT_INFO_LIST: &str = "i2p.router.net.tunnels.client.info.list";
    pub const P170_NET_STATUS_V6: &str = "i2p.router.net.status.v6";
    pub const P170_NET_ERROR: &str = "i2p.router.net.error";
    pub const P170_NET_ERROR_V6: &str = "i2p.router.net.error.v6";
    pub const P170_NET_TESTING: &str = "i2p.router.net.testing";
    pub const P170_NET_TESTING_V6: &str = "i2p.router.net.testing.v6";
    pub const P170_NET_TUNNELS_SUCCESS_RATE: &str = "i2p.router.net.tunnels.successrate";
    pub const P170_NET_TUNNELS_TOTAL_SUCCESS_RATE: &str = "i2p.router.net.tunnels.totalsuccessrate";
    pub const P170_NET_TUNNELS_QUEUE: &str = "i2p.router.net.tunnels.queue";
    pub const P170_NET_TUNNELS_TBM_QUEUE: &str = "i2p.router.net.tunnels.tbmqueue";
    pub const P170_NETDB_PEERS: &str = "i2p.router.netdb.peers";
    pub const P170_NETDB_ACTIVE_PEERS_INFO: &str = "i2p.router.netdb.activepeers.info";
    pub const P170_NETDB_NTCP_LIMIT: &str = "i2p.router.netdb.ntcp.limit";
    pub const P170_NETDB_SSU_LIMIT: &str = "i2p.router.netdb.ssu.limit";
    pub const P170_NETDB_BANNED_PEERS: &str = "i2p.router.netdb.bannedpeers";
    pub const P170_NETDB_ACTIVE_PEERS_LIST: &str = "i2p.router.netdb.activepeers.list";
    pub const P170_NETDB_PEERS_LIST: &str = "i2p.router.netdb.peers.list";
    pub const P170_NETDB_PEERS_INFO: &str = "i2p.router.netdb.peers.info";
    pub const P170_NETDB_ACTIVE_PEERS_STATS: &str = "i2p.router.netdb.activepeers.stats";

    // --- Clock skew ---
    pub const CLOCK_SKEW: &str = "i2p.router.clock.skew";

    // --- Share ratio and configured BW ---
    pub const SHARE_RATIO: &str = "i2p.router.shareRatio";
    pub const CONFIGURED_BW_INBOUND: &str = "i2p.router.configuredBwInbound";
    pub const CONFIGURED_BW_OUTBOUND: &str = "i2p.router.configuredBwOutbound";

    // --- Tunnel selectors ---
    pub const TUNNELS_PARTICIPATING: &str = "i2p.router.tunnels.participating";
    pub const TUNNELS_EXPLORATORY_IN: &str = "i2p.router.tunnels.exploratoryIn";
    pub const TUNNELS_EXPLORATORY_OUT: &str = "i2p.router.tunnels.exploratoryOut";
    pub const TUNNELS_CLIENT_IN: &str = "i2p.router.tunnels.clientIn";
    pub const TUNNELS_CLIENT_OUT: &str = "i2p.router.tunnels.clientOut";
    pub const TUNNELS_CONFIGURED: &str = "i2p.router.tunnels.configured";
    pub const TUNNELS_QUEUE: &str = "i2p.router.tunnels.queue";

    // --- Peer selectors ---
    pub const PEERS_KNOWN_COUNT: &str = "i2p.router.peers.knownCount";
    pub const PEERS_KNOWN: &str = "i2p.router.peers.known";
    pub const PEERS_ACTIVE_COUNT: &str = "i2p.router.peers.activeCount";
    pub const PEERS_ACTIVE: &str = "i2p.router.peers.active";
    pub const PEERS_ROUTER_INFO: &str = "i2p.router.peers.routerInfo";
    pub const PEERS_BANNED: &str = "i2p.router.peers.banned";
    pub const PEERS_BANNED_COUNT: &str = "i2p.router.peers.bannedCount";
    pub const PEERS_LIMITS: &str = "i2p.router.peers.limits";
    pub const PEERS_ACTIVE_STATS: &str = "i2p.router.peers.activeStats";

    // --- I2PTunnel selectors ---
    pub const NET_IPTUNNELS: &str = "i2p.router.net.i2ptunnels";

    // --- Log selectors ---
    pub const LOG_SNAPSHOT: &str = "i2p.router.log";
    pub const LOG_CLEAR: &str = "i2p.router.log.clear";

    // --- Address-book selectors (owned by M003) ---
    pub const ADDRESS_BOOK_PRIVATE: &str = "i2p.router.addressbook.private";
    pub const ADDRESS_BOOK_LOCAL: &str = "i2p.router.addressbook.local";
    pub const ADDRESS_BOOK_ROUTER: &str = "i2p.router.addressbook.router";
    pub const ADDRESS_BOOK_PUBLISHED: &str = "i2p.router.addressbook.published";
    pub const ADDRESS_BOOK_SUBSCRIPTIONS: &str = "i2p.router.addressbook.subscriptions";
    pub const ADDRESS_BOOK_CONFIG: &str = "i2p.router.addressbook.config";

    pub const P170_ADDRESS_BOOK_PRIVATE_LIST: &str = "i2p.router.addressbook.private.list";
    pub const P170_ADDRESS_BOOK_LOCAL_LIST: &str = "i2p.router.addressbook.local.list";
    pub const P170_ADDRESS_BOOK_ROUTER_LIST: &str = "i2p.router.addressbook.router.list";
    pub const P170_ADDRESS_BOOK_PUBLISHED_LIST: &str = "i2p.router.addressbook.published.list";
    pub const P170_ADDRESS_BOOK_SUBSCRIPTIONS: &str = "i2p.router.addressbook.subscriptions";
    pub const P170_ADDRESS_BOOK_CONFIG: &str = "i2p.router.addressbook.config";

    /// JSON types used by the pinned Proposal 170 RouterInfo additions.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum JsonType {
        String,
        NullableString,
        NullableInteger,
        Integer,
        Number,
        ArrayOfStrings,
        ArrayOfObjects,
        Object,
    }

    /// Truthful current-source state for a canonical RouterInfo addition.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum SourceState {
        Available,
        Unavailable,
        ProtocolAmbiguity,
    }

    /// Machine-checkable contract inventory for the pinned revision.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ContractField {
        pub key: &'static str,
        pub json_type: JsonType,
        pub source: SourceState,
    }

    /// The exact 43-key Proposal 170 RouterInfo addition set.
    pub const PROPOSAL_170_ADDITIONS: &[&str; 43] = &[
        ROUTER_NEWS,
        P170_ID,
        P170_CLOCKSKEW,
        P170_INFO,
        P170_LOGS,
        P170_LOGS_CLEAR,
        P170_NET_TOTAL_RECEIVED_BYTES,
        P170_NET_TOTAL_SENT_BYTES,
        P170_NET_TOTAL_TRANSIT_BYTES,
        P170_NET_BW_TRANSIT_15S,
        P170_NET_TUNNELS_SHARE_RATIO,
        P170_NET_TUNNELS_PARTICIPATING_INFO,
        P170_NET_TUNNELS_I2PTUNNEL,
        P170_NET_TUNNELS_EXPLORATORY_INBOUND,
        P170_NET_TUNNELS_EXPLORATORY_OUTBOUND,
        P170_NET_TUNNELS_EXPLORATORY_INFO_LIST,
        P170_NET_TUNNELS_CLIENT_INBOUND,
        P170_NET_TUNNELS_CLIENT_OUTBOUND,
        P170_NET_TUNNELS_CLIENT_INFO_LIST,
        P170_NET_STATUS_V6,
        P170_NET_ERROR,
        P170_NET_ERROR_V6,
        P170_NET_TESTING,
        P170_NET_TESTING_V6,
        P170_NET_TUNNELS_SUCCESS_RATE,
        P170_NET_TUNNELS_TOTAL_SUCCESS_RATE,
        P170_NET_TUNNELS_QUEUE,
        P170_NET_TUNNELS_TBM_QUEUE,
        P170_NETDB_PEERS,
        P170_NETDB_ACTIVE_PEERS_INFO,
        P170_NETDB_NTCP_LIMIT,
        P170_NETDB_SSU_LIMIT,
        P170_NETDB_BANNED_PEERS,
        P170_NETDB_ACTIVE_PEERS_LIST,
        P170_NETDB_PEERS_LIST,
        P170_NETDB_PEERS_INFO,
        P170_NETDB_ACTIVE_PEERS_STATS,
        P170_ADDRESS_BOOK_PRIVATE_LIST,
        P170_ADDRESS_BOOK_LOCAL_LIST,
        P170_ADDRESS_BOOK_ROUTER_LIST,
        P170_ADDRESS_BOOK_PUBLISHED_LIST,
        P170_ADDRESS_BOOK_SUBSCRIPTIONS,
        P170_ADDRESS_BOOK_CONFIG,
    ];

    /// Types and source states for every canonical addition.
    pub const PROPOSAL_170_CONTRACT: &[ContractField; 43] = &[
        ContractField {
            key: ROUTER_NEWS,
            json_type: JsonType::String,
            source: SourceState::Available,
        },
        ContractField {
            key: P170_ID,
            json_type: JsonType::NullableString,
            source: SourceState::Available,
        },
        ContractField {
            key: P170_CLOCKSKEW,
            json_type: JsonType::NullableInteger,
            source: SourceState::Available,
        },
        ContractField {
            key: P170_INFO,
            json_type: JsonType::NullableString,
            source: SourceState::Available,
        },
        ContractField {
            key: P170_LOGS,
            json_type: JsonType::ArrayOfStrings,
            source: SourceState::Available,
        },
        ContractField {
            key: P170_LOGS_CLEAR,
            json_type: JsonType::String,
            source: SourceState::Available,
        },
        ContractField {
            key: P170_NET_TOTAL_RECEIVED_BYTES,
            json_type: JsonType::Integer,
            source: SourceState::Available,
        },
        ContractField {
            key: P170_NET_TOTAL_SENT_BYTES,
            json_type: JsonType::Integer,
            source: SourceState::Available,
        },
        ContractField {
            key: P170_NET_TOTAL_TRANSIT_BYTES,
            json_type: JsonType::Integer,
            source: SourceState::Available,
        },
        ContractField {
            key: P170_NET_BW_TRANSIT_15S,
            json_type: JsonType::Integer,
            source: SourceState::Unavailable,
        },
        ContractField {
            key: P170_NET_TUNNELS_SHARE_RATIO,
            json_type: JsonType::Number,
            source: SourceState::Available,
        },
        ContractField {
            key: P170_NET_TUNNELS_PARTICIPATING_INFO,
            json_type: JsonType::ArrayOfObjects,
            source: SourceState::Unavailable,
        },
        ContractField {
            key: P170_NET_TUNNELS_I2PTUNNEL,
            json_type: JsonType::ArrayOfObjects,
            source: SourceState::Available,
        },
        ContractField {
            key: P170_NET_TUNNELS_EXPLORATORY_INBOUND,
            json_type: JsonType::Integer,
            source: SourceState::Unavailable,
        },
        ContractField {
            key: P170_NET_TUNNELS_EXPLORATORY_OUTBOUND,
            json_type: JsonType::Integer,
            source: SourceState::Unavailable,
        },
        ContractField {
            key: P170_NET_TUNNELS_EXPLORATORY_INFO_LIST,
            json_type: JsonType::ArrayOfObjects,
            source: SourceState::Unavailable,
        },
        ContractField {
            key: P170_NET_TUNNELS_CLIENT_INBOUND,
            json_type: JsonType::Integer,
            source: SourceState::Unavailable,
        },
        ContractField {
            key: P170_NET_TUNNELS_CLIENT_OUTBOUND,
            json_type: JsonType::Integer,
            source: SourceState::Unavailable,
        },
        ContractField {
            key: P170_NET_TUNNELS_CLIENT_INFO_LIST,
            json_type: JsonType::ArrayOfObjects,
            source: SourceState::Unavailable,
        },
        ContractField {
            key: P170_NET_STATUS_V6,
            json_type: JsonType::Integer,
            source: SourceState::ProtocolAmbiguity,
        },
        ContractField {
            key: P170_NET_ERROR,
            json_type: JsonType::Integer,
            source: SourceState::Unavailable,
        },
        ContractField {
            key: P170_NET_ERROR_V6,
            json_type: JsonType::Integer,
            source: SourceState::Unavailable,
        },
        ContractField {
            key: P170_NET_TESTING,
            json_type: JsonType::Integer,
            source: SourceState::ProtocolAmbiguity,
        },
        ContractField {
            key: P170_NET_TESTING_V6,
            json_type: JsonType::Integer,
            source: SourceState::ProtocolAmbiguity,
        },
        ContractField {
            key: P170_NET_TUNNELS_SUCCESS_RATE,
            json_type: JsonType::Number,
            source: SourceState::Unavailable,
        },
        ContractField {
            key: P170_NET_TUNNELS_TOTAL_SUCCESS_RATE,
            json_type: JsonType::Number,
            source: SourceState::Available,
        },
        ContractField {
            key: P170_NET_TUNNELS_QUEUE,
            json_type: JsonType::Integer,
            source: SourceState::Unavailable,
        },
        ContractField {
            key: P170_NET_TUNNELS_TBM_QUEUE,
            json_type: JsonType::Integer,
            source: SourceState::Unavailable,
        },
        ContractField {
            key: P170_NETDB_PEERS,
            json_type: JsonType::ArrayOfStrings,
            source: SourceState::Unavailable,
        },
        ContractField {
            key: P170_NETDB_ACTIVE_PEERS_INFO,
            json_type: JsonType::ArrayOfStrings,
            source: SourceState::Unavailable,
        },
        ContractField {
            key: P170_NETDB_NTCP_LIMIT,
            json_type: JsonType::Integer,
            source: SourceState::Unavailable,
        },
        ContractField {
            key: P170_NETDB_SSU_LIMIT,
            json_type: JsonType::Integer,
            source: SourceState::Unavailable,
        },
        ContractField {
            key: P170_NETDB_BANNED_PEERS,
            json_type: JsonType::Object,
            source: SourceState::Unavailable,
        },
        ContractField {
            key: P170_NETDB_ACTIVE_PEERS_LIST,
            json_type: JsonType::ArrayOfStrings,
            source: SourceState::Unavailable,
        },
        ContractField {
            key: P170_NETDB_PEERS_LIST,
            json_type: JsonType::ArrayOfStrings,
            source: SourceState::Unavailable,
        },
        ContractField {
            key: P170_NETDB_PEERS_INFO,
            json_type: JsonType::ArrayOfStrings,
            source: SourceState::Unavailable,
        },
        ContractField {
            key: P170_NETDB_ACTIVE_PEERS_STATS,
            json_type: JsonType::ArrayOfObjects,
            source: SourceState::Unavailable,
        },
        ContractField {
            key: P170_ADDRESS_BOOK_PRIVATE_LIST,
            json_type: JsonType::ArrayOfObjects,
            source: SourceState::Available,
        },
        ContractField {
            key: P170_ADDRESS_BOOK_LOCAL_LIST,
            json_type: JsonType::ArrayOfObjects,
            source: SourceState::Available,
        },
        ContractField {
            key: P170_ADDRESS_BOOK_ROUTER_LIST,
            json_type: JsonType::ArrayOfObjects,
            source: SourceState::Available,
        },
        ContractField {
            key: P170_ADDRESS_BOOK_PUBLISHED_LIST,
            json_type: JsonType::ArrayOfObjects,
            source: SourceState::Available,
        },
        ContractField {
            key: P170_ADDRESS_BOOK_SUBSCRIPTIONS,
            json_type: JsonType::Object,
            source: SourceState::Unavailable,
        },
        ContractField {
            key: P170_ADDRESS_BOOK_CONFIG,
            json_type: JsonType::Object,
            source: SourceState::Unavailable,
        },
    ];

    /// True when a selector belongs to the exact Proposal 170 addition set.
    pub fn is_proposal_170_addition(key: &str) -> bool {
        PROPOSAL_170_ADDITIONS.contains(&key)
    }

    /// All address-book selector keys.
    pub const ADDRESS_BOOK_KEYS: &[&str] = &[
        ADDRESS_BOOK_PRIVATE,
        ADDRESS_BOOK_LOCAL,
        ADDRESS_BOOK_ROUTER,
        ADDRESS_BOOK_PUBLISHED,
        ADDRESS_BOOK_SUBSCRIPTIONS,
        ADDRESS_BOOK_CONFIG,
    ];

    /// All legacy/base and exact Proposal 170 RouterInfo selector keys.
    pub const ALL: &[&str] = &[
        // UDP
        UDP_ACTIVE,
        UDP_COOKIE_ACTIVE,
        UDP_INTEGRATED_PEERS,
        UDP_FIREWALLED,
        UDP_HIDDEN,
        UDP_COINFICIENT_PEERS,
        UDP_CRITICAL_PEERS,
        UDP_FAST_PEERS,
        UDP_HIGH_CAPACITY_PEERS,
        UDP_INTERLEAVED_PEERS,
        UDP_LIT_PEERS,
        UDP_LOW_CAPACITY_PEERS,
        UDP_ON_DEMAND_PEERS,
        UDP_PEER_STATS,
        UDP_STANDARD_PEERS,
        UDP_UNREACHABLE_PEERS,
        UDP_TOTAL_PEERS,
        UDP_CURRENT_PEERS,
        // General
        VERSION,
        UPTIME,
        // NetDB
        NETDB_ACTIVE,
        NETDB_ACTIVE_PROFILES,
        NETDB_HIGHEST_VERSION,
        NETDB_KNOWN_PROFILES,
        NETDB_NEW_PROFILES,
        NETDB_ACTIVE_ROUTERS,
        NETDB_ALREADY_EXPERIENCED_PEERS,
        NETDB_BANLIST_SIZE,
        NETDB_EXPLORATORY_PEERS,
        NETDB_FAST_PEERS,
        NETDB_HIGH_CAPACITY_PEERS,
        NETDB_IS_BACKLOGGED,
        NETDB_KNOWN_ACTIVE,
        NETDB_KNOWN_IDLE,
        NETDB_KNOWN_USED,
        NETDB_KNOWN_VANILLA,
        NETDB_KNOWN_VOLATILE,
        NETDB_LAST_EXPLORED,
        NETDB_LAST_PROFILE_LOOKUP,
        NETDB_LAST_ROUTER_LOOKUP,
        NETDB_LAST_UNSAVED,
        NETDB_LEASE_SETS,
        NETDB_NEW_ACTIVE,
        NETDB_NEW_IDLE,
        NETDB_OLD_ACTIVE,
        NETDB_OLD_IDLE,
        NETDB_PEER_PROFILES,
        NETDB_PLAINTEXT_PEERS,
        NETDB_RESERVE_ACTIVE,
        NETDB_RESERVE_ACTIVE_PEERS,
        NETDB_RESERVE_HIGH_CAPACITY,
        NETDB_RESERVE_INTEGRATED,
        NETDB_RESERVE_KNOWN,
        NETDB_RESERVE_LOOKUP,
        NETDB_RESERVE_PENDING,
        NETDB_RESERVE_RESERVED,
        NETDB_RESERVE_STANDARD,
        NETDB_RESERVE_TIER2,
        NETDB_RESERVE_USED,
        NETDB_RESERVE_VOLATILE,
        NETDB_STANDARD_PEERS,
        NETDB_LOW_CAPACITY_PEERS,
        NETDB_TUNNELS,
        NETDB_USED_PEERS,
        NETDB_VOLATILE_PEERS,
        NETDB_ADDRESS_BOOKS,
        NETDB_ADDRESS_BOOK_ENTRIES,
        NETDB_ADDRESS_BOOK_SOURCES,
        NETDB_ADDRESS_BOOK_SUBSCRIPTIONS,
        NETDB_ADDRESS_BOOK_UPDATES,
        // Bandwidth
        BW_INBOUND_1S,
        BW_INBOUND_15S,
        BW_INBOUND_1M,
        BW_INBOUND_1H,
        BW_INBOUND_1D,
        BW_INBOUND_TOTAL,
        BW_OUTBOUND_1S,
        BW_OUTBOUND_15S,
        BW_OUTBOUND_1M,
        BW_OUTBOUND_1H,
        BW_OUTBOUND_1D,
        BW_OUTBOUND_TOTAL,
        // TCP
        TCP_ACTIVE,
        TCP_INTEGRATED_PEERS,
        TCP_FIREWALLED,
        TCP_HOSTS,
        TCP_STATUS,
        TCP_VERSION,
        // Identity/Network
        IDENTITY,
        NET_BW_INBOUND,
        NET_BW_OUTBOUND,
        // Router news, clock, share ratio, configured BW
        ROUTER_NEWS,
        CLOCK_SKEW,
        SHARE_RATIO,
        CONFIGURED_BW_INBOUND,
        CONFIGURED_BW_OUTBOUND,
        // Tunnels
        TUNNELS_PARTICIPATING,
        TUNNELS_EXPLORATORY_IN,
        TUNNELS_EXPLORATORY_OUT,
        TUNNELS_CLIENT_IN,
        TUNNELS_CLIENT_OUT,
        TUNNELS_CONFIGURED,
        TUNNELS_QUEUE,
        // Peers
        PEERS_KNOWN_COUNT,
        PEERS_KNOWN,
        PEERS_ACTIVE_COUNT,
        PEERS_ACTIVE,
        PEERS_ROUTER_INFO,
        PEERS_BANNED,
        PEERS_BANNED_COUNT,
        PEERS_LIMITS,
        PEERS_ACTIVE_STATS,
        // I2PTunnel
        NET_IPTUNNELS,
        // Logs
        LOG_SNAPSHOT,
        LOG_CLEAR,
        // Address book
        ADDRESS_BOOK_PRIVATE,
        ADDRESS_BOOK_LOCAL,
        ADDRESS_BOOK_ROUTER,
        ADDRESS_BOOK_PUBLISHED,
        ADDRESS_BOOK_SUBSCRIPTIONS,
        ADDRESS_BOOK_CONFIG,
        // Exact Proposal 170 additions not already present above.
        P170_ID,
        P170_CLOCKSKEW,
        P170_INFO,
        P170_LOGS,
        P170_LOGS_CLEAR,
        P170_NET_TOTAL_RECEIVED_BYTES,
        P170_NET_TOTAL_SENT_BYTES,
        P170_NET_TOTAL_TRANSIT_BYTES,
        P170_NET_BW_TRANSIT_15S,
        P170_NET_TUNNELS_SHARE_RATIO,
        P170_NET_TUNNELS_PARTICIPATING_INFO,
        P170_NET_TUNNELS_I2PTUNNEL,
        P170_NET_TUNNELS_EXPLORATORY_INBOUND,
        P170_NET_TUNNELS_EXPLORATORY_OUTBOUND,
        P170_NET_TUNNELS_EXPLORATORY_INFO_LIST,
        P170_NET_TUNNELS_CLIENT_INBOUND,
        P170_NET_TUNNELS_CLIENT_OUTBOUND,
        P170_NET_TUNNELS_CLIENT_INFO_LIST,
        P170_NET_STATUS_V6,
        P170_NET_ERROR,
        P170_NET_ERROR_V6,
        P170_NET_TESTING,
        P170_NET_TESTING_V6,
        P170_NET_TUNNELS_SUCCESS_RATE,
        P170_NET_TUNNELS_TOTAL_SUCCESS_RATE,
        P170_NET_TUNNELS_QUEUE,
        P170_NET_TUNNELS_TBM_QUEUE,
        P170_NETDB_PEERS,
        P170_NETDB_ACTIVE_PEERS_INFO,
        P170_NETDB_NTCP_LIMIT,
        P170_NETDB_SSU_LIMIT,
        P170_NETDB_BANNED_PEERS,
        P170_NETDB_ACTIVE_PEERS_LIST,
        P170_NETDB_PEERS_LIST,
        P170_NETDB_PEERS_INFO,
        P170_NETDB_ACTIVE_PEERS_STATS,
        P170_ADDRESS_BOOK_PRIVATE_LIST,
        P170_ADDRESS_BOOK_LOCAL_LIST,
        P170_ADDRESS_BOOK_ROUTER_LIST,
        P170_ADDRESS_BOOK_PUBLISHED_LIST,
    ];

    /// All non-address-book selector keys (owned by M005).
    pub const CORE_KEYS: &[&str] = &[
        UDP_ACTIVE,
        UDP_COOKIE_ACTIVE,
        UDP_INTEGRATED_PEERS,
        UDP_FIREWALLED,
        UDP_HIDDEN,
        UDP_COINFICIENT_PEERS,
        UDP_CRITICAL_PEERS,
        UDP_FAST_PEERS,
        UDP_HIGH_CAPACITY_PEERS,
        UDP_INTERLEAVED_PEERS,
        UDP_LIT_PEERS,
        UDP_LOW_CAPACITY_PEERS,
        UDP_ON_DEMAND_PEERS,
        UDP_PEER_STATS,
        UDP_STANDARD_PEERS,
        UDP_UNREACHABLE_PEERS,
        UDP_TOTAL_PEERS,
        UDP_CURRENT_PEERS,
        VERSION,
        UPTIME,
        NETDB_ACTIVE,
        NETDB_ACTIVE_PROFILES,
        NETDB_HIGHEST_VERSION,
        NETDB_KNOWN_PROFILES,
        NETDB_NEW_PROFILES,
        NETDB_ACTIVE_ROUTERS,
        NETDB_ALREADY_EXPERIENCED_PEERS,
        NETDB_BANLIST_SIZE,
        NETDB_EXPLORATORY_PEERS,
        NETDB_FAST_PEERS,
        NETDB_HIGH_CAPACITY_PEERS,
        NETDB_IS_BACKLOGGED,
        NETDB_KNOWN_ACTIVE,
        NETDB_KNOWN_IDLE,
        NETDB_KNOWN_USED,
        NETDB_KNOWN_VANILLA,
        NETDB_KNOWN_VOLATILE,
        NETDB_LAST_EXPLORED,
        NETDB_LAST_PROFILE_LOOKUP,
        NETDB_LAST_ROUTER_LOOKUP,
        NETDB_LAST_UNSAVED,
        NETDB_LEASE_SETS,
        NETDB_NEW_ACTIVE,
        NETDB_NEW_IDLE,
        NETDB_OLD_ACTIVE,
        NETDB_OLD_IDLE,
        NETDB_PEER_PROFILES,
        NETDB_PLAINTEXT_PEERS,
        NETDB_RESERVE_ACTIVE,
        NETDB_RESERVE_ACTIVE_PEERS,
        NETDB_RESERVE_HIGH_CAPACITY,
        NETDB_RESERVE_INTEGRATED,
        NETDB_RESERVE_KNOWN,
        NETDB_RESERVE_LOOKUP,
        NETDB_RESERVE_PENDING,
        NETDB_RESERVE_RESERVED,
        NETDB_RESERVE_STANDARD,
        NETDB_RESERVE_TIER2,
        NETDB_RESERVE_USED,
        NETDB_RESERVE_VOLATILE,
        NETDB_STANDARD_PEERS,
        NETDB_LOW_CAPACITY_PEERS,
        NETDB_TUNNELS,
        NETDB_USED_PEERS,
        NETDB_VOLATILE_PEERS,
        NETDB_ADDRESS_BOOKS,
        NETDB_ADDRESS_BOOK_ENTRIES,
        NETDB_ADDRESS_BOOK_SOURCES,
        NETDB_ADDRESS_BOOK_SUBSCRIPTIONS,
        NETDB_ADDRESS_BOOK_UPDATES,
        BW_INBOUND_1S,
        BW_INBOUND_15S,
        BW_INBOUND_1M,
        BW_INBOUND_1H,
        BW_INBOUND_1D,
        BW_INBOUND_TOTAL,
        BW_OUTBOUND_1S,
        BW_OUTBOUND_15S,
        BW_OUTBOUND_1M,
        BW_OUTBOUND_1H,
        BW_OUTBOUND_1D,
        BW_OUTBOUND_TOTAL,
        TCP_ACTIVE,
        TCP_INTEGRATED_PEERS,
        TCP_FIREWALLED,
        TCP_HOSTS,
        TCP_STATUS,
        TCP_VERSION,
        IDENTITY,
        NET_BW_INBOUND,
        NET_BW_OUTBOUND,
        ROUTER_NEWS,
        CLOCK_SKEW,
        SHARE_RATIO,
        CONFIGURED_BW_INBOUND,
        CONFIGURED_BW_OUTBOUND,
        TUNNELS_PARTICIPATING,
        TUNNELS_EXPLORATORY_IN,
        TUNNELS_EXPLORATORY_OUT,
        TUNNELS_CLIENT_IN,
        TUNNELS_CLIENT_OUT,
        TUNNELS_CONFIGURED,
        TUNNELS_QUEUE,
        PEERS_KNOWN_COUNT,
        PEERS_KNOWN,
        PEERS_ACTIVE_COUNT,
        PEERS_ACTIVE,
        PEERS_ROUTER_INFO,
        PEERS_BANNED,
        PEERS_BANNED_COUNT,
        PEERS_LIMITS,
        PEERS_ACTIVE_STATS,
        NET_IPTUNNELS,
        LOG_SNAPSHOT,
        LOG_CLEAR,
    ];
}

/// Test if a string is a valid tunnel type.
#[allow(dead_code)]
pub fn is_valid_tunnel_type(s: &str) -> bool {
    tunnel_types::ALL.contains(&s)
}

/// Test if a string is a valid address book name.
#[allow(dead_code)]
pub fn is_valid_address_book(s: &str) -> bool {
    address_books::ALL.contains(&s)
}

/// Test if a string is a valid Proposal 170 RouterInfo selector key.
#[allow(dead_code)]
pub fn is_valid_router_info_selector(s: &str) -> bool {
    router_info_keys::ALL.contains(&s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_request() {
        let body = r#"{"jsonrpc":"2.0","method":"Authenticate","params":{"API":2,"Username":"i2pcontrol","Password":"secret"},"id":1}"#;
        let req = parse_request(body).unwrap();
        assert_eq!(req.jsonrpc, "2.0");
        assert_eq!(req.method, "Authenticate");
        assert_eq!(req.id, Some(RequestId::Number(1)));
        assert!(req.params.is_some());
    }

    #[test]
    fn parse_request_missing_jsonrpc() {
        let body = r#"{"method":"Authenticate","id":1}"#;
        let err = parse_request(body).unwrap_err();
        assert_eq!(err.error.code, error_codes::INVALID_REQUEST);
    }

    #[test]
    fn parse_request_wrong_version() {
        let body = r#"{"jsonrpc":"1.0","method":"Authenticate","id":1}"#;
        let err = parse_request(body).unwrap_err();
        assert_eq!(err.error.code, error_codes::INVALID_REQUEST);
    }

    #[test]
    fn parse_request_missing_method() {
        let body = r#"{"jsonrpc":"2.0","id":1}"#;
        let err = parse_request(body).unwrap_err();
        assert_eq!(err.error.code, error_codes::INVALID_REQUEST);
    }

    #[test]
    fn parse_request_invalid_json() {
        let body = "not json";
        let err = parse_request(body).unwrap_err();
        assert_eq!(err.error.code, error_codes::PARSE_ERROR);
    }

    #[test]
    fn parse_request_positional_params() {
        let body = r#"{"jsonrpc":"2.0","method":"Authenticate","params":["a","b"],"id":1}"#;
        let err = parse_request(body).unwrap_err();
        assert_eq!(err.error.code, error_codes::INVALID_PARAMS);
    }

    #[test]
    fn parse_request_notification() {
        let body = r#"{"jsonrpc":"2.0","method":"Authenticate"}"#;
        let req = parse_request(body).unwrap();
        assert!(req.id.is_none());
    }

    #[test]
    fn parse_request_string_id() {
        let body = r#"{"jsonrpc":"2.0","method":"Authenticate","id":"abc"}"#;
        let req = parse_request(body).unwrap();
        assert_eq!(req.id, Some(RequestId::String("abc".to_string())));
    }

    #[test]
    fn tunnel_types_complete() {
        assert_eq!(tunnel_types::ALL.len(), 12);
        for tt in tunnel_types::ALL {
            assert!(is_valid_tunnel_type(tt));
        }
        assert!(!is_valid_tunnel_type("unknown"));
    }

    #[test]
    fn address_books_complete() {
        assert_eq!(address_books::ALL.len(), 4);
        for ab in address_books::ALL {
            assert!(is_valid_address_book(ab));
        }
        assert!(!is_valid_address_book("unknown"));
    }

    #[test]
    fn router_info_selectors_complete() {
        assert_eq!(router_info_keys::PROPOSAL_170_ADDITIONS.len(), 43);
        assert_eq!(router_info_keys::PROPOSAL_170_CONTRACT.len(), 43);
        let additions: std::collections::HashSet<&str> =
            router_info_keys::PROPOSAL_170_ADDITIONS.iter().copied().collect();
        assert_eq!(additions.len(), 43);
        assert_eq!(
            additions,
            router_info_keys::PROPOSAL_170_CONTRACT.iter().map(|field| field.key).collect()
        );
        for key in router_info_keys::ALL {
            assert!(is_valid_router_info_selector(key));
        }
        for key in router_info_keys::PROPOSAL_170_ADDITIONS {
            assert!(is_valid_router_info_selector(key));
        }
        assert!(!is_valid_router_info_selector("unknown.selector"));
    }

    #[test]
    fn router_info_core_keys_excludes_address_book() {
        for key in router_info_keys::CORE_KEYS {
            assert!(
                !router_info_keys::ADDRESS_BOOK_KEYS.contains(key),
                "CORE_KEYS should not contain address-book key: {key}"
            );
        }
    }

    #[test]
    fn router_info_all_keys_is_superset_of_core_and_address_book() {
        let all_set: std::collections::HashSet<&str> =
            router_info_keys::ALL.iter().copied().collect();
        for key in router_info_keys::CORE_KEYS {
            assert!(all_set.contains(key), "ALL missing CORE_KEY: {key}");
        }
        for key in router_info_keys::ADDRESS_BOOK_KEYS {
            assert!(all_set.contains(key), "ALL missing ADDRESS_BOOK_KEY: {key}");
        }
    }

    #[test]
    fn serialize_success_response() {
        let resp = JsonRpcSuccess::new(
            RequestId::Number(1),
            serde_json::json!({"Token": "abc", "API": "2"}),
        );
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"jsonrpc\":\"2.0\""));
        assert!(json.contains("\"result\""));
    }

    #[test]
    fn serialize_error_response() {
        let resp = JsonRpcErrorResponse::new(
            RequestId::Number(1),
            error_codes::METHOD_NOT_FOUND,
            "Method not found",
        );
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"error\""));
        assert!(json.contains("-32601"));
    }
}
