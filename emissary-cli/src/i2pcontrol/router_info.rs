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

//! Proposal 170 RouterInfo inspection control plane and selector dispatch.
//!
//! Defines the read-only snapshot queries that selector adapters use to
//! produce exact Proposal 170 responses. All data is returned as bounded
//! immutable snapshots. No mutation, no private keys, no EventSubscriber.

use std::{collections::HashMap, fmt};

use async_trait::async_trait;

#[allow(dead_code)]
const LOG_TARGET: &str = "emissary::i2pcontrol::router_info";

// --- Inspection error vocabulary ---

/// Snapshot group for grouped request dispatch.
///
/// Each group corresponds to one coherent source query per request.
/// The handler queries a group at most once when any selector in the
/// group is requested.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InspectionGroup {
    Retained,
    Network,
    UdpTransport,
    TcpTransport,
    NetDb,
    TrafficMetrics,
    TunnelSummary,
    PeerList,
    PeerLookup,
    PeerStats,
    I2PTunnel,
    Log,
    AddressBook,
}

impl fmt::Display for InspectionGroup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Retained => write!(f, "retained"),
            Self::Network => write!(f, "network"),
            Self::UdpTransport => write!(f, "udp-transport"),
            Self::TcpTransport => write!(f, "tcp-transport"),
            Self::NetDb => write!(f, "netdb"),
            Self::TrafficMetrics => write!(f, "traffic-metrics"),
            Self::TunnelSummary => write!(f, "tunnel-summary"),
            Self::PeerList => write!(f, "peer-list"),
            Self::PeerLookup => write!(f, "peer-lookup"),
            Self::PeerStats => write!(f, "peer-stats"),
            Self::I2PTunnel => write!(f, "i2ptunnel"),
            Self::Log => write!(f, "log"),
            Self::AddressBook => write!(f, "address-book"),
        }
    }
}

/// Typed error for RouterInfo inspection failures.
///
/// Errors map to sanitized JSON-RPC error responses. No private keys,
/// file paths, or internal backtraces are exposed in `Display` output.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum InspectionError {
    /// Source group not wired or not yet implemented.
    Unavailable { group: InspectionGroup },
    /// Source temporarily unavailable (e.g. transient query failure).
    TemporarilyUnavailable { group: InspectionGroup },
    /// Source query failed.
    QueryFailed { group: InspectionGroup },
    /// Result exceeds protocol or resource bounds.
    ResultTooLarge {
        group: InspectionGroup,
        limit: usize,
    },
    /// Invalid peer identifier in lookup request.
    InvalidPeerId,
    /// Internal invariant violation (should never occur).
    InternalInvariant,
}

impl fmt::Display for InspectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable { group } => {
                write!(f, "{group} data unavailable")
            }
            Self::TemporarilyUnavailable { group } => {
                write!(f, "{group} temporarily unavailable")
            }
            Self::QueryFailed { group } => {
                write!(f, "{group} query failed")
            }
            Self::ResultTooLarge { group, limit } => {
                write!(f, "{group} result exceeds bound of {limit} items")
            }
            Self::InvalidPeerId => write!(f, "invalid peer identifier"),
            Self::InternalInvariant => {
                write!(f, "internal inspection invariant violation")
            }
        }
    }
}

impl std::error::Error for InspectionError {}

// --- Bounded snapshot DTOs ---

/// Network status codes per Proposal 170.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkStatus {
    Ok,
    Firewalled,
    Hidden,
    Testing,
    Fail,
    FailTcp,
    FailUdp,
    FailNat,
    SymmetricNat,
    Unknown,
}

impl NetworkStatus {
    /// Wire value for Proposal 170 IPv4/IPv6 status.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "OK",
            Self::Firewalled => "Firewalled",
            Self::Hidden => "Hidden",
            Self::Testing => "Testing",
            Self::Fail => "Fail",
            Self::FailTcp => "Fail TCP",
            Self::FailUdp => "Fail UDP",
            Self::FailNat => "Fail NAT",
            Self::SymmetricNat => "Symmetric NAT",
            Self::Unknown => "Unknown",
        }
    }
}

/// Cumulative transport byte counters.
#[derive(Debug, Clone, Default)]
pub struct TransportBytes {
    pub received: u64,
    pub sent: u64,
}

/// Rolling transit traffic snapshot at multiple intervals.
#[derive(Debug, Clone, Default)]
pub struct RecentTransitTraffic {
    pub inbound_1s: u64,
    pub outbound_1s: u64,
    pub inbound_15s: u64,
    pub outbound_15s: u64,
    pub inbound_1m: u64,
    pub outbound_1m: u64,
    pub inbound_1h: u64,
    pub outbound_1h: u64,
    pub inbound_1d: u64,
    pub outbound_1d: u64,
}

/// Cumulative transit byte counters.
#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct TransitBytes {
    pub received: u64,
    pub sent: u64,
}

/// Tunnel build success/failure counters.
#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct TunnelBuildStats {
    pub successes: u64,
    pub failures: u64,
}

/// Tunnel summary counts.
#[derive(Debug, Clone, Default)]
pub struct TunnelSummary {
    pub active_participating: usize,
    pub configured: usize,
    pub exploratory_inbound: usize,
    pub exploratory_outbound: usize,
    pub client_inbound: usize,
    pub client_outbound: usize,
    pub queue_depth: usize,
}

/// Network reachability snapshot.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct NetworkSnapshot {
    pub ipv4_status: NetworkStatus,
    pub ipv6_status: NetworkStatus,
    pub error: Option<String>,
    pub testing: bool,
    pub firewalled: bool,
    pub hidden: bool,
    pub reachability_disabled: bool,
}

impl Default for NetworkSnapshot {
    fn default() -> Self {
        Self {
            ipv4_status: NetworkStatus::Unknown,
            ipv6_status: NetworkStatus::Unknown,
            error: None,
            testing: false,
            firewalled: false,
            hidden: false,
            reachability_disabled: false,
        }
    }
}

/// Clock skew estimate in seconds (positive = ahead of peers).
#[derive(Debug, Clone, Default)]
pub struct ClockSkew {
    /// None means not yet estimated; Some(0) means no skew detected.
    pub skew_seconds: Option<i64>,
}

/// NetDB summary snapshot.
#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
#[allow(non_snake_case)]
pub struct NetDbSnapshot {
    pub active: bool,
    pub known_profiles: usize,
    pub active_profiles: usize,
    pub highest_version: u32,
    pub new_profiles: usize,
    pub active_routers: usize,
    pub banlist_size: usize,
    pub lease_sets: usize,
    pub exploratory_peers: usize,
    pub fast_peers: usize,
    pub high_capacity_peers: usize,
    pub standard_peers: usize,
    pub low_capacity_peers: usize,
    pub web_rtc_peers: usize,
    pub SSU_peers: usize,
    pub NTCP_peers: usize,
    pub total_peers: usize,
    pub used_peers: usize,
    pub volatile_peers: usize,
    pub fast_reject_profiles: usize,
    pub high_capacity_reject_profiles: usize,
    pub standard_reject_profiles: usize,
    pub low_capacity_reject_profiles: usize,
    pub web_rtc_reject_profiles: usize,
    pub SSU_reject_profiles: usize,
    pub NTCP_reject_profiles: usize,
    pub total_reject_profiles: usize,
    pub active_fast_profiles: usize,
    pub active_high_capacity_profiles: usize,
    pub active_standard_profiles: usize,
    pub active_low_capacity_profiles: usize,
    pub active_web_rtc_profiles: usize,
    pub active_SSU_profiles: usize,
    pub active_NTCP_profiles: usize,
    pub total_active_profiles: usize,
    pub idle_fast_profiles: usize,
    pub idle_high_capacity_profiles: usize,
    pub idle_standard_profiles: usize,
    pub idle_low_capacity_profiles: usize,
    pub idle_web_rtc_profiles: usize,
    pub idle_SSU_profiles: usize,
    pub idle_NTCP_profiles: usize,
    pub total_idle_profiles: usize,
}

/// Peer identity for list responses.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct PeerIdentity {
    pub id: String,
    pub is_active: bool,
}

/// Peer connection limits.
#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct PeerLimits {
    pub configured_inbound: usize,
    pub configured_outbound: usize,
    pub effective_inbound: usize,
    pub effective_outbound: usize,
}

/// Banned peer entry.
#[derive(Debug, Clone)]
pub struct BannedPeer {
    pub id: String,
    pub reason: String,
    pub expires_at: Option<u64>,
}

/// Active peer transport statistics.
#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct ActivePeerStats {
    pub peer_id: String,
    pub direction: String,
    pub state: String,
    pub bytes_received: u64,
    pub bytes_sent: u64,
    pub avg_latency_ms: Option<f64>,
}

/// UDP transport snapshot.
#[derive(Debug, Clone, Default)]
pub struct UdpSnapshot {
    pub active: bool,
    pub cookie_active: bool,
    pub integrated_peers: usize,
    pub firewalled: bool,
    pub hidden: bool,
    pub coinficient_peers: usize,
    pub critical_peers: usize,
    pub fast_peers: usize,
    pub high_capacity_peers: usize,
    pub interleaved_peers: usize,
    pub lit_peers: usize,
    pub low_capacity_peers: usize,
    pub on_demand_peers: usize,
    pub standard_peers: usize,
    pub unreachable_peers: usize,
    pub total_peers: usize,
    pub current_peers: usize,
}

/// TCP transport snapshot.
#[derive(Debug, Clone, Default)]
pub struct TcpSnapshot {
    pub active: bool,
    pub integrated_peers: usize,
    pub firewalled: bool,
    pub hosts: String,
    pub status: String,
    pub version: String,
}

/// I2PTunnel quick statistics (from M004).
#[derive(Debug, Clone, Default)]
pub struct I2PTunnelStats {
    pub configured_count: usize,
}

/// Bounded log entry for I2PControl buffer.
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp_ms: u64,
    pub level: String,
    pub target: String,
    pub message: String,
}

/// I2PControl log buffer snapshot.
#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct LogSnapshot {
    pub entries: Vec<LogEntry>,
    pub generation: u64,
}

/// UDP peer stats entry for the peerStats selector.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize)]
pub struct UdpPeerStatEntry {
    #[serde(rename = "ID")]
    pub id: String,
    #[serde(rename = "Type")]
    pub peer_type: String,
    #[serde(rename = "Updated")]
    pub updated: u64,
    #[serde(rename = "IP")]
    pub ip: String,
    #[serde(rename = "Port")]
    pub port: u16,
    #[serde(rename = "Version")]
    pub version: u32,
    #[serde(rename = "Capability High Capacity")]
    pub high_capacity: bool,
    #[serde(rename = "Capability Fast")]
    pub fast: bool,
    #[serde(rename = "Capability Low")]
    pub low: bool,
    #[serde(rename = "Capability Medium")]
    pub medium: bool,
    #[serde(rename = "Capability Integrating")]
    pub integrating: bool,
    #[serde(rename = "Capability Reachable")]
    pub reachable: bool,
    #[serde(rename = "Capability Unreachable")]
    pub unreachable: bool,
    #[serde(rename = "Profile Share")]
    pub profile_share: f64,
    #[serde(rename = "Speed")]
    pub speed: String,
    #[serde(rename = "Duration")]
    pub duration: String,
}

use serde::Serialize;

/// Read-only inspection boundary for Proposal 170 RouterInfo selectors.
///
/// All methods return immutable snapshots. No method mutates router state,
/// triggers reachability tests, builds tunnels, or consumes EventSubscriber.
///
/// # Invariants
///
/// - Snapshots are bounded and do not expose mutable core handles.
/// - No private keys, tunnel session keys, or authentication tokens.
/// - No direct references into mutable core collections.
/// - Read operations do not block router progress.
/// - Methods returning `Result` distinguish unavailable, failed, and successful-but-empty states.
#[allow(dead_code)]
#[async_trait]
pub trait RouterInfoControl: Send + Sync {
    /// Get the local router identity as Base64-encoded serialized RouterInfo.
    fn router_identity(&self) -> Result<String, InspectionError>;

    /// Get router version string.
    fn router_version(&self) -> Result<String, InspectionError>;

    /// Get router uptime in milliseconds.
    fn router_uptime_ms(&self) -> Result<u64, InspectionError>;

    /// Get network reachability snapshot.
    async fn network_snapshot(&self) -> Result<NetworkSnapshot, InspectionError>;

    /// Get clock skew estimate.
    async fn clock_skew(&self) -> Result<ClockSkew, InspectionError>;

    /// Get cumulative transport bytes.
    async fn transport_bytes(&self) -> Result<TransportBytes, InspectionError>;

    /// Get rolling transit traffic snapshot.
    async fn recent_transit_traffic(&self) -> Result<RecentTransitTraffic, InspectionError>;

    /// Get cumulative transit bytes.
    async fn transit_bytes(&self) -> Result<TransitBytes, InspectionError>;

    /// Get tunnel build success/failure stats.
    async fn tunnel_build_stats(&self) -> Result<TunnelBuildStats, InspectionError>;

    /// Get tunnel summary counts.
    async fn tunnel_summary(&self) -> Result<TunnelSummary, InspectionError>;

    /// Get NetDB summary.
    async fn netdb_snapshot(&self) -> Result<NetDbSnapshot, InspectionError>;

    /// Get UDP transport snapshot.
    async fn udp_snapshot(&self) -> Result<UdpSnapshot, InspectionError>;

    /// Get TCP transport snapshot.
    async fn tcp_snapshot(&self) -> Result<TcpSnapshot, InspectionError>;

    /// Get known peers (canonical stored peer set).
    async fn known_peers(&self) -> Result<Vec<PeerIdentity>, InspectionError>;

    /// Get active peers (live transport sessions).
    async fn active_peers(&self) -> Result<Vec<PeerIdentity>, InspectionError>;

    /// Get a serialized RouterInfo for a specific peer.
    ///
    /// `Ok(None)` means the source was queried successfully and the peer
    /// is not present. It must not mean the source is not wired.
    async fn peer_router_info(&self, peer_id: &str) -> Result<Option<String>, InspectionError>;

    /// Get banned peers.
    async fn banned_peers(&self) -> Result<Vec<BannedPeer>, InspectionError>;

    /// Get configured and effective peer/transport limits.
    async fn peer_limits(&self) -> Result<PeerLimits, InspectionError>;

    /// Get active peer transport statistics.
    async fn active_peer_stats(&self) -> Result<Vec<ActivePeerStats>, InspectionError>;

    /// Get I2PTunnel quick statistics from M004.
    async fn i2ptunnel_stats(&self) -> Result<I2PTunnelStats, InspectionError>;

    /// Get log buffer snapshot.
    async fn log_snapshot(&self) -> Result<LogSnapshot, InspectionError>;

    /// Clear the I2PControl log buffer.
    async fn log_clear(&self) -> Result<(), InspectionError>;

    /// Get router news. Emissary has no news subsystem; returns empty string.
    fn router_news(&self) -> Result<String, InspectionError>;

    /// Get bandwidth shares/ratios from configuration.
    async fn share_ratio(&self) -> Result<f64, InspectionError>;

    /// Get configured bandwidth limits.
    async fn configured_bw_limits(&self) -> Result<(u64, u64), InspectionError>;
}

// --- Fake implementation for testing ---

/// Fake implementation of RouterInfoControl for unit tests.
///
/// Defaults every snapshot group to `Err(InspectionError::Unavailable)`.
/// Tests must explicitly configure each requested snapshot group to prove
/// that returned values are known facts rather than constructor defaults.
#[allow(dead_code)]
pub struct FakeRouterInfoControl {
    inner: std::sync::Mutex<FakeInner>,
}

#[allow(dead_code)]
struct FakeInner {
    identity: Result<String, InspectionError>,
    version: Result<String, InspectionError>,
    uptime_ms: Result<u64, InspectionError>,
    network: Result<NetworkSnapshot, InspectionError>,
    clock_skew: Result<ClockSkew, InspectionError>,
    transport_bytes: Result<TransportBytes, InspectionError>,
    recent_transit: Result<RecentTransitTraffic, InspectionError>,
    transit_bytes: Result<TransitBytes, InspectionError>,
    build_stats: Result<TunnelBuildStats, InspectionError>,
    tunnel_summary: Result<TunnelSummary, InspectionError>,
    netdb: Result<NetDbSnapshot, InspectionError>,
    udp: Result<UdpSnapshot, InspectionError>,
    tcp: Result<TcpSnapshot, InspectionError>,
    known_peers: Result<Vec<PeerIdentity>, InspectionError>,
    active_peers: Result<Vec<PeerIdentity>, InspectionError>,
    peer_ris: HashMap<String, String>,
    banned_peers: Result<Vec<BannedPeer>, InspectionError>,
    peer_limits: Result<PeerLimits, InspectionError>,
    active_peer_stats: Result<Vec<ActivePeerStats>, InspectionError>,
    i2ptunnel_stats: Result<I2PTunnelStats, InspectionError>,
    log_entries: Vec<LogEntry>,
    log_generation: u64,
    share_ratio: Result<f64, InspectionError>,
    configured_bw: Result<(u64, u64), InspectionError>,
    router_news: Result<String, InspectionError>,
}

#[allow(dead_code)]
fn unavailable(group: InspectionGroup) -> InspectionError {
    InspectionError::Unavailable { group }
}

#[allow(dead_code)]
impl FakeRouterInfoControl {
    pub fn new() -> Self {
        Self {
            inner: std::sync::Mutex::new(FakeInner {
                identity: Err(unavailable(InspectionGroup::Retained)),
                version: Err(unavailable(InspectionGroup::Retained)),
                uptime_ms: Err(unavailable(InspectionGroup::Retained)),
                network: Err(unavailable(InspectionGroup::Network)),
                clock_skew: Err(unavailable(InspectionGroup::Network)),
                transport_bytes: Err(unavailable(InspectionGroup::TrafficMetrics)),
                recent_transit: Err(unavailable(InspectionGroup::TrafficMetrics)),
                transit_bytes: Err(unavailable(InspectionGroup::TrafficMetrics)),
                build_stats: Err(unavailable(InspectionGroup::TrafficMetrics)),
                tunnel_summary: Err(unavailable(InspectionGroup::TunnelSummary)),
                netdb: Err(unavailable(InspectionGroup::NetDb)),
                udp: Err(unavailable(InspectionGroup::UdpTransport)),
                tcp: Err(unavailable(InspectionGroup::TcpTransport)),
                known_peers: Err(unavailable(InspectionGroup::PeerList)),
                active_peers: Err(unavailable(InspectionGroup::PeerList)),
                peer_ris: HashMap::new(),
                banned_peers: Err(unavailable(InspectionGroup::PeerStats)),
                peer_limits: Err(unavailable(InspectionGroup::PeerStats)),
                active_peer_stats: Err(unavailable(InspectionGroup::PeerStats)),
                i2ptunnel_stats: Err(unavailable(InspectionGroup::I2PTunnel)),
                log_entries: Vec::new(),
                log_generation: 0,
                share_ratio: Err(unavailable(InspectionGroup::Retained)),
                configured_bw: Err(unavailable(InspectionGroup::Retained)),
                router_news: Err(unavailable(InspectionGroup::Retained)),
            }),
        }
    }

    /// Set the router identity for tests.
    pub fn set_identity(&self, identity: String) {
        let mut inner = self.inner.lock().unwrap();
        inner.identity = Ok(identity);
    }

    /// Set the router version for tests.
    pub fn set_version(&self, version: String) {
        let mut inner = self.inner.lock().unwrap();
        inner.version = Ok(version);
    }

    /// Set uptime for tests.
    pub fn set_uptime_ms(&self, ms: u64) {
        let mut inner = self.inner.lock().unwrap();
        inner.uptime_ms = Ok(ms);
    }

    /// Set network snapshot for tests.
    pub fn set_network(&self, network: NetworkSnapshot) {
        let mut inner = self.inner.lock().unwrap();
        inner.network = Ok(network);
    }

    /// Set clock skew for tests.
    pub fn set_clock_skew(&self, skew: ClockSkew) {
        let mut inner = self.inner.lock().unwrap();
        inner.clock_skew = Ok(skew);
    }

    /// Set transport bytes for tests.
    pub fn set_transport_bytes(&self, bytes: TransportBytes) {
        let mut inner = self.inner.lock().unwrap();
        inner.transport_bytes = Ok(bytes);
    }

    /// Set recent transit traffic for tests.
    pub fn set_recent_transit_traffic(&self, traffic: RecentTransitTraffic) {
        let mut inner = self.inner.lock().unwrap();
        inner.recent_transit = Ok(traffic);
    }

    /// Set tunnel build stats for tests.
    pub fn set_build_stats(&self, stats: TunnelBuildStats) {
        let mut inner = self.inner.lock().unwrap();
        inner.build_stats = Ok(stats);
    }

    /// Set tunnel summary for tests.
    pub fn set_tunnel_summary(&self, summary: TunnelSummary) {
        let mut inner = self.inner.lock().unwrap();
        inner.tunnel_summary = Ok(summary);
    }

    /// Set netdb snapshot for tests.
    pub fn set_netdb(&self, netdb: NetDbSnapshot) {
        let mut inner = self.inner.lock().unwrap();
        inner.netdb = Ok(netdb);
    }

    /// Set UDP snapshot for tests.
    pub fn set_udp(&self, udp: UdpSnapshot) {
        let mut inner = self.inner.lock().unwrap();
        inner.udp = Ok(udp);
    }

    /// Set TCP snapshot for tests.
    pub fn set_tcp(&self, tcp: TcpSnapshot) {
        let mut inner = self.inner.lock().unwrap();
        inner.tcp = Ok(tcp);
    }

    /// Set known peers for tests.
    pub fn set_known_peers(&self, peers: Vec<PeerIdentity>) {
        let mut inner = self.inner.lock().unwrap();
        inner.known_peers = Ok(peers);
    }

    /// Set active peers for tests.
    pub fn set_active_peers(&self, peers: Vec<PeerIdentity>) {
        let mut inner = self.inner.lock().unwrap();
        inner.active_peers = Ok(peers);
    }

    /// Insert a peer RouterInfo for tests.
    pub fn insert_peer_ri(&self, peer_id: String, ri: String) {
        let mut inner = self.inner.lock().unwrap();
        inner.peer_ris.insert(peer_id, ri);
    }

    /// Set banned peers for tests.
    pub fn set_banned_peers(&self, peers: Vec<BannedPeer>) {
        let mut inner = self.inner.lock().unwrap();
        inner.banned_peers = Ok(peers);
    }

    /// Set peer limits for tests.
    pub fn set_peer_limits(&self, limits: PeerLimits) {
        let mut inner = self.inner.lock().unwrap();
        inner.peer_limits = Ok(limits);
    }

    /// Set active peer stats for tests.
    pub fn set_active_peer_stats(&self, stats: Vec<ActivePeerStats>) {
        let mut inner = self.inner.lock().unwrap();
        inner.active_peer_stats = Ok(stats);
    }

    /// Set I2PTunnel stats for tests.
    pub fn set_i2ptunnel_stats(&self, stats: I2PTunnelStats) {
        let mut inner = self.inner.lock().unwrap();
        inner.i2ptunnel_stats = Ok(stats);
    }

    /// Add a log entry for tests.
    pub fn add_log_entry(&self, entry: LogEntry) {
        let mut inner = self.inner.lock().unwrap();
        inner.log_entries.push(entry);
    }

    /// Set share ratio for tests.
    pub fn set_share_ratio(&self, ratio: f64) {
        let mut inner = self.inner.lock().unwrap();
        inner.share_ratio = Ok(ratio);
    }

    /// Set router news for tests.
    pub fn set_router_news(&self, news: String) {
        let mut inner = self.inner.lock().unwrap();
        inner.router_news = Ok(news);
    }
}

impl Default for FakeRouterInfoControl {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl RouterInfoControl for FakeRouterInfoControl {
    fn router_identity(&self) -> Result<String, InspectionError> {
        let inner = self.inner.lock().unwrap();
        inner.identity.clone()
    }

    fn router_version(&self) -> Result<String, InspectionError> {
        let inner = self.inner.lock().unwrap();
        inner.version.clone()
    }

    fn router_uptime_ms(&self) -> Result<u64, InspectionError> {
        let inner = self.inner.lock().unwrap();
        inner.uptime_ms.clone()
    }

    async fn network_snapshot(&self) -> Result<NetworkSnapshot, InspectionError> {
        let inner = self.inner.lock().unwrap();
        inner.network.clone()
    }

    async fn clock_skew(&self) -> Result<ClockSkew, InspectionError> {
        let inner = self.inner.lock().unwrap();
        inner.clock_skew.clone()
    }

    async fn transport_bytes(&self) -> Result<TransportBytes, InspectionError> {
        let inner = self.inner.lock().unwrap();
        inner.transport_bytes.clone()
    }

    async fn recent_transit_traffic(&self) -> Result<RecentTransitTraffic, InspectionError> {
        let inner = self.inner.lock().unwrap();
        inner.recent_transit.clone()
    }

    async fn transit_bytes(&self) -> Result<TransitBytes, InspectionError> {
        let inner = self.inner.lock().unwrap();
        inner.transit_bytes.clone()
    }

    async fn tunnel_build_stats(&self) -> Result<TunnelBuildStats, InspectionError> {
        let inner = self.inner.lock().unwrap();
        inner.build_stats.clone()
    }

    async fn tunnel_summary(&self) -> Result<TunnelSummary, InspectionError> {
        let inner = self.inner.lock().unwrap();
        inner.tunnel_summary.clone()
    }

    async fn netdb_snapshot(&self) -> Result<NetDbSnapshot, InspectionError> {
        let inner = self.inner.lock().unwrap();
        inner.netdb.clone()
    }

    async fn udp_snapshot(&self) -> Result<UdpSnapshot, InspectionError> {
        let inner = self.inner.lock().unwrap();
        inner.udp.clone()
    }

    async fn tcp_snapshot(&self) -> Result<TcpSnapshot, InspectionError> {
        let inner = self.inner.lock().unwrap();
        inner.tcp.clone()
    }

    async fn known_peers(&self) -> Result<Vec<PeerIdentity>, InspectionError> {
        let inner = self.inner.lock().unwrap();
        inner.known_peers.clone()
    }

    async fn active_peers(&self) -> Result<Vec<PeerIdentity>, InspectionError> {
        let inner = self.inner.lock().unwrap();
        inner.active_peers.clone()
    }

    async fn peer_router_info(&self, peer_id: &str) -> Result<Option<String>, InspectionError> {
        let inner = self.inner.lock().unwrap();
        Ok(inner.peer_ris.get(peer_id).cloned())
    }

    async fn banned_peers(&self) -> Result<Vec<BannedPeer>, InspectionError> {
        let inner = self.inner.lock().unwrap();
        inner.banned_peers.clone()
    }

    async fn peer_limits(&self) -> Result<PeerLimits, InspectionError> {
        let inner = self.inner.lock().unwrap();
        inner.peer_limits.clone()
    }

    async fn active_peer_stats(&self) -> Result<Vec<ActivePeerStats>, InspectionError> {
        let inner = self.inner.lock().unwrap();
        inner.active_peer_stats.clone()
    }

    async fn i2ptunnel_stats(&self) -> Result<I2PTunnelStats, InspectionError> {
        let inner = self.inner.lock().unwrap();
        inner.i2ptunnel_stats.clone()
    }

    async fn log_snapshot(&self) -> Result<LogSnapshot, InspectionError> {
        let inner = self.inner.lock().unwrap();
        Ok(LogSnapshot {
            entries: inner.log_entries.clone(),
            generation: inner.log_generation,
        })
    }

    async fn log_clear(&self) -> Result<(), InspectionError> {
        let mut inner = self.inner.lock().unwrap();
        inner.log_entries.clear();
        inner.log_generation += 1;
        Ok(())
    }

    fn router_news(&self) -> Result<String, InspectionError> {
        let inner = self.inner.lock().unwrap();
        inner.router_news.clone()
    }

    async fn share_ratio(&self) -> Result<f64, InspectionError> {
        let inner = self.inner.lock().unwrap();
        inner.share_ratio.clone()
    }

    async fn configured_bw_limits(&self) -> Result<(u64, u64), InspectionError> {
        let inner = self.inner.lock().unwrap();
        inner.configured_bw.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn network_status_as_str() {
        assert_eq!(NetworkStatus::Ok.as_str(), "OK");
        assert_eq!(NetworkStatus::Firewalled.as_str(), "Firewalled");
        assert_eq!(NetworkStatus::Hidden.as_str(), "Hidden");
        assert_eq!(NetworkStatus::Testing.as_str(), "Testing");
        assert_eq!(NetworkStatus::Fail.as_str(), "Fail");
        assert_eq!(NetworkStatus::FailTcp.as_str(), "Fail TCP");
        assert_eq!(NetworkStatus::FailUdp.as_str(), "Fail UDP");
        assert_eq!(NetworkStatus::FailNat.as_str(), "Fail NAT");
        assert_eq!(NetworkStatus::SymmetricNat.as_str(), "Symmetric NAT");
        assert_eq!(NetworkStatus::Unknown.as_str(), "Unknown");
    }

    #[test]
    fn clock_skew_default_unknown() {
        let skew = ClockSkew::default();
        assert!(skew.skew_seconds.is_none());
    }

    #[test]
    fn inspection_group_display() {
        assert_eq!(InspectionGroup::Retained.to_string(), "retained");
        assert_eq!(InspectionGroup::Network.to_string(), "network");
        assert_eq!(InspectionGroup::UdpTransport.to_string(), "udp-transport");
        assert_eq!(InspectionGroup::TcpTransport.to_string(), "tcp-transport");
        assert_eq!(InspectionGroup::NetDb.to_string(), "netdb");
        assert_eq!(
            InspectionGroup::TrafficMetrics.to_string(),
            "traffic-metrics"
        );
        assert_eq!(InspectionGroup::TunnelSummary.to_string(), "tunnel-summary");
        assert_eq!(InspectionGroup::PeerList.to_string(), "peer-list");
        assert_eq!(InspectionGroup::PeerLookup.to_string(), "peer-lookup");
        assert_eq!(InspectionGroup::PeerStats.to_string(), "peer-stats");
        assert_eq!(InspectionGroup::I2PTunnel.to_string(), "i2ptunnel");
        assert_eq!(InspectionGroup::Log.to_string(), "log");
        assert_eq!(InspectionGroup::AddressBook.to_string(), "address-book");
    }

    #[test]
    fn inspection_error_display_no_secrets() {
        let err = InspectionError::Unavailable {
            group: InspectionGroup::NetDb,
        };
        let msg = err.to_string();
        assert!(msg.contains("netdb"));
        assert!(!msg.contains("/"));
        assert!(!msg.contains("key"));

        let err = InspectionError::InvalidPeerId;
        assert!(!err.to_string().contains("/"));
    }

    #[tokio::test]
    async fn fake_defaults_to_unavailable() {
        let fake = FakeRouterInfoControl::new();
        assert!(fake.router_identity().is_err());
        assert!(fake.router_version().is_err());
        assert!(fake.router_uptime_ms().is_err());
        assert!(fake.network_snapshot().await.is_err());
        assert!(fake.clock_skew().await.is_err());
        assert!(fake.transport_bytes().await.is_err());
        assert!(fake.recent_transit_traffic().await.is_err());
        assert!(fake.transit_bytes().await.is_err());
        assert!(fake.tunnel_build_stats().await.is_err());
        assert!(fake.tunnel_summary().await.is_err());
        assert!(fake.netdb_snapshot().await.is_err());
        assert!(fake.udp_snapshot().await.is_err());
        assert!(fake.tcp_snapshot().await.is_err());
        assert!(fake.known_peers().await.is_err());
        assert!(fake.active_peers().await.is_err());
        assert!(fake.banned_peers().await.is_err());
        assert!(fake.peer_limits().await.is_err());
        assert!(fake.active_peer_stats().await.is_err());
        assert!(fake.i2ptunnel_stats().await.is_err());
        assert!(fake.share_ratio().await.is_err());
        assert!(fake.configured_bw_limits().await.is_err());
        assert!(fake.router_news().is_err());
    }

    #[tokio::test]
    async fn fake_setters_prove_known_facts() {
        let fake = FakeRouterInfoControl::new();
        fake.set_identity("test-identity-b64".to_string());
        fake.set_version("Test 1.0".to_string());
        fake.set_uptime_ms(60000);

        assert_eq!(fake.router_identity().unwrap(), "test-identity-b64");
        assert_eq!(fake.router_version().unwrap(), "Test 1.0");
        assert_eq!(fake.router_uptime_ms().unwrap(), 60000);

        fake.set_transport_bytes(TransportBytes {
            received: 1024,
            sent: 2048,
        });
        let tb = fake.transport_bytes().await.unwrap();
        assert_eq!(tb.received, 1024);
        assert_eq!(tb.sent, 2048);

        fake.set_build_stats(TunnelBuildStats {
            successes: 10,
            failures: 2,
        });
        let bs = fake.tunnel_build_stats().await.unwrap();
        assert_eq!(bs.successes, 10);
        assert_eq!(bs.failures, 2);
    }

    #[tokio::test]
    async fn fake_available_zero_distinct_from_unavailable() {
        let fake = FakeRouterInfoControl::new();
        // Unset peers should be unavailable
        assert!(fake.known_peers().await.is_err());
        // Explicitly configured empty peers should succeed
        fake.set_known_peers(Vec::new());
        let peers = fake.known_peers().await.unwrap();
        assert!(peers.is_empty());
    }

    #[tokio::test]
    async fn fake_log_clear_increments_generation() {
        let fake = FakeRouterInfoControl::new();
        fake.add_log_entry(LogEntry {
            timestamp_ms: 1000,
            level: "INFO".to_string(),
            target: "test".to_string(),
            message: "hello".to_string(),
        });

        let snap = fake.log_snapshot().await.unwrap();
        assert_eq!(snap.entries.len(), 1);
        assert_eq!(snap.generation, 0);

        fake.log_clear().await.unwrap();

        let snap = fake.log_snapshot().await.unwrap();
        assert!(snap.entries.is_empty());
        assert_eq!(snap.generation, 1);
    }
}
