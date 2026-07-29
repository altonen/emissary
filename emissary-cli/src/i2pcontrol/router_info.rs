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

use std::collections::HashMap;

use async_trait::async_trait;

#[allow(dead_code)]
const LOG_TARGET: &str = "emissary::i2pcontrol::router_info";

// --- Bounded snapshot DTOs ---

/// Network status codes per Proposal 170.
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

/// Rolling 15-second transit traffic snapshot.
#[derive(Debug, Clone, Default)]
pub struct RecentTransitTraffic {
    pub inbound_1s: u64,
    pub outbound_1s: u64,
    pub inbound_15s: u64,
    pub outbound_15s: u64,
}

/// Cumulative transit byte counters.
#[derive(Debug, Clone, Default)]
pub struct TransitBytes {
    pub received: u64,
    pub sent: u64,
}

/// Tunnel build success/failure counters.
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
#[derive(Debug, Clone)]
pub struct PeerIdentity {
    pub id: String,
    pub is_active: bool,
}

/// Peer connection limits.
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
    pub active_count: usize,
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
#[derive(Debug, Clone, Default)]
pub struct LogSnapshot {
    pub entries: Vec<LogEntry>,
    pub generation: u64,
}

/// UDP peer stats entry for the peerStats selector.
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
#[async_trait]
pub trait RouterInfoControl: Send + Sync {
    /// Get the local router identity as Base64-encoded serialized RouterInfo.
    fn router_identity(&self) -> Result<String, String>;

    /// Get router version string.
    fn router_version(&self) -> String;

    /// Get router uptime in milliseconds.
    fn router_uptime_ms(&self) -> u64;

    /// Get network reachability snapshot.
    async fn network_snapshot(&self) -> NetworkSnapshot;

    /// Get clock skew estimate.
    async fn clock_skew(&self) -> ClockSkew;

    /// Get cumulative transport bytes.
    async fn transport_bytes(&self) -> TransportBytes;

    /// Get rolling transit traffic snapshot.
    async fn recent_transit_traffic(&self) -> RecentTransitTraffic;

    /// Get cumulative transit bytes.
    async fn transit_bytes(&self) -> TransitBytes;

    /// Get tunnel build success/failure stats.
    async fn tunnel_build_stats(&self) -> TunnelBuildStats;

    /// Get tunnel summary counts.
    async fn tunnel_summary(&self) -> TunnelSummary;

    /// Get NetDB summary.
    async fn netdb_snapshot(&self) -> NetDbSnapshot;

    /// Get UDP transport snapshot.
    async fn udp_snapshot(&self) -> UdpSnapshot;

    /// Get TCP transport snapshot.
    async fn tcp_snapshot(&self) -> TcpSnapshot;

    /// Get known peers (canonical stored peer set).
    async fn known_peers(&self) -> Vec<PeerIdentity>;

    /// Get active peers (live transport sessions).
    async fn active_peers(&self) -> Vec<PeerIdentity>;

    /// Get a serialized RouterInfo for a specific peer.
    async fn peer_router_info(&self, peer_id: &str) -> Result<Option<String>, String>;

    /// Get banned peers.
    async fn banned_peers(&self) -> Vec<BannedPeer>;

    /// Get configured and effective peer/transport limits.
    async fn peer_limits(&self) -> PeerLimits;

    /// Get active peer transport statistics.
    async fn active_peer_stats(&self) -> Vec<ActivePeerStats>;

    /// Get I2PTunnel quick statistics from M004.
    async fn i2ptunnel_stats(&self) -> I2PTunnelStats;

    /// Get log buffer snapshot.
    async fn log_snapshot(&self) -> LogSnapshot;

    /// Clear the I2PControl log buffer.
    async fn log_clear(&self);

    /// Get router news. Emissary has no news subsystem; returns empty string.
    fn router_news(&self) -> String;

    /// Get bandwidth shares/ratios from configuration.
    async fn share_ratio(&self) -> f64;

    /// Get configured bandwidth limits.
    async fn configured_bw_limits(&self) -> (u64, u64);
}

// --- Fake implementation for testing ---

/// Fake implementation of RouterInfoControl for unit tests.
///
/// Returns configurable stub values. No real router state.
pub struct FakeRouterInfoControl {
    inner: std::sync::Mutex<FakeInner>,
}

struct FakeInner {
    identity: String,
    version: String,
    uptime_ms: u64,
    network: NetworkSnapshot,
    clock_skew: ClockSkew,
    transport_bytes: TransportBytes,
    recent_transit: RecentTransitTraffic,
    transit_bytes: TransitBytes,
    build_stats: TunnelBuildStats,
    tunnel_summary: TunnelSummary,
    netdb: NetDbSnapshot,
    udp: UdpSnapshot,
    tcp: TcpSnapshot,
    known_peers: Vec<PeerIdentity>,
    active_peers: Vec<PeerIdentity>,
    peer_ris: HashMap<String, String>,
    banned_peers: Vec<BannedPeer>,
    peer_limits: PeerLimits,
    active_peer_stats: Vec<ActivePeerStats>,
    i2ptunnel_stats: I2PTunnelStats,
    log_entries: Vec<LogEntry>,
    log_generation: u64,
    share_ratio: f64,
    configured_bw: (u64, u64),
    router_news: String,
}

impl FakeRouterInfoControl {
    pub fn new() -> Self {
        Self {
            inner: std::sync::Mutex::new(FakeInner {
                identity: String::new(),
                version: "Emissary 0.4.0".to_string(),
                uptime_ms: 0,
                network: NetworkSnapshot::default(),
                clock_skew: ClockSkew::default(),
                transport_bytes: TransportBytes::default(),
                recent_transit: RecentTransitTraffic::default(),
                transit_bytes: TransitBytes::default(),
                build_stats: TunnelBuildStats::default(),
                tunnel_summary: TunnelSummary::default(),
                netdb: NetDbSnapshot::default(),
                udp: UdpSnapshot::default(),
                tcp: TcpSnapshot::default(),
                known_peers: Vec::new(),
                active_peers: Vec::new(),
                peer_ris: HashMap::new(),
                banned_peers: Vec::new(),
                peer_limits: PeerLimits::default(),
                active_peer_stats: Vec::new(),
                i2ptunnel_stats: I2PTunnelStats::default(),
                log_entries: Vec::new(),
                log_generation: 0,
                share_ratio: 0.5,
                configured_bw: (512, 512),
                router_news: String::new(),
            }),
        }
    }

    /// Set the router identity for tests.
    pub fn set_identity(&self, identity: String) {
        let mut inner = self.inner.lock().unwrap();
        inner.identity = identity;
    }

    /// Set the router version for tests.
    pub fn set_version(&self, version: String) {
        let mut inner = self.inner.lock().unwrap();
        inner.version = version;
    }

    /// Set uptime for tests.
    pub fn set_uptime_ms(&self, ms: u64) {
        let mut inner = self.inner.lock().unwrap();
        inner.uptime_ms = ms;
    }

    /// Set network snapshot for tests.
    pub fn set_network(&self, network: NetworkSnapshot) {
        let mut inner = self.inner.lock().unwrap();
        inner.network = network;
    }

    /// Set transport bytes for tests.
    pub fn set_transport_bytes(&self, bytes: TransportBytes) {
        let mut inner = self.inner.lock().unwrap();
        inner.transport_bytes = bytes;
    }

    /// Set tunnel build stats for tests.
    pub fn set_build_stats(&self, stats: TunnelBuildStats) {
        let mut inner = self.inner.lock().unwrap();
        inner.build_stats = stats;
    }

    /// Set tunnel summary for tests.
    pub fn set_tunnel_summary(&self, summary: TunnelSummary) {
        let mut inner = self.inner.lock().unwrap();
        inner.tunnel_summary = summary;
    }

    /// Set netdb snapshot for tests.
    pub fn set_netdb(&self, netdb: NetDbSnapshot) {
        let mut inner = self.inner.lock().unwrap();
        inner.netdb = netdb;
    }

    /// Set UDP snapshot for tests.
    pub fn set_udp(&self, udp: UdpSnapshot) {
        let mut inner = self.inner.lock().unwrap();
        inner.udp = udp;
    }

    /// Set TCP snapshot for tests.
    pub fn set_tcp(&self, tcp: TcpSnapshot) {
        let mut inner = self.inner.lock().unwrap();
        inner.tcp = tcp;
    }

    /// Set known peers for tests.
    pub fn set_known_peers(&self, peers: Vec<PeerIdentity>) {
        let mut inner = self.inner.lock().unwrap();
        inner.known_peers = peers;
    }

    /// Set active peers for tests.
    pub fn set_active_peers(&self, peers: Vec<PeerIdentity>) {
        let mut inner = self.inner.lock().unwrap();
        inner.active_peers = peers;
    }

    /// Insert a peer RouterInfo for tests.
    pub fn insert_peer_ri(&self, peer_id: String, ri: String) {
        let mut inner = self.inner.lock().unwrap();
        inner.peer_ris.insert(peer_id, ri);
    }

    /// Set banned peers for tests.
    pub fn set_banned_peers(&self, peers: Vec<BannedPeer>) {
        let mut inner = self.inner.lock().unwrap();
        inner.banned_peers = peers;
    }

    /// Set peer limits for tests.
    pub fn set_peer_limits(&self, limits: PeerLimits) {
        let mut inner = self.inner.lock().unwrap();
        inner.peer_limits = limits;
    }

    /// Set active peer stats for tests.
    pub fn set_active_peer_stats(&self, stats: Vec<ActivePeerStats>) {
        let mut inner = self.inner.lock().unwrap();
        inner.active_peer_stats = stats;
    }

    /// Set I2PTunnel stats for tests.
    pub fn set_i2ptunnel_stats(&self, stats: I2PTunnelStats) {
        let mut inner = self.inner.lock().unwrap();
        inner.i2ptunnel_stats = stats;
    }

    /// Add a log entry for tests.
    pub fn add_log_entry(&self, entry: LogEntry) {
        let mut inner = self.inner.lock().unwrap();
        inner.log_entries.push(entry);
    }

    /// Set share ratio for tests.
    pub fn set_share_ratio(&self, ratio: f64) {
        let mut inner = self.inner.lock().unwrap();
        inner.share_ratio = ratio;
    }

    /// Set router news for tests.
    pub fn set_router_news(&self, news: String) {
        let mut inner = self.inner.lock().unwrap();
        inner.router_news = news;
    }
}

impl Default for FakeRouterInfoControl {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl RouterInfoControl for FakeRouterInfoControl {
    fn router_identity(&self) -> Result<String, String> {
        let inner = self.inner.lock().unwrap();
        Ok(inner.identity.clone())
    }

    fn router_version(&self) -> String {
        let inner = self.inner.lock().unwrap();
        inner.version.clone()
    }

    fn router_uptime_ms(&self) -> u64 {
        let inner = self.inner.lock().unwrap();
        inner.uptime_ms
    }

    async fn network_snapshot(&self) -> NetworkSnapshot {
        let inner = self.inner.lock().unwrap();
        inner.network.clone()
    }

    async fn clock_skew(&self) -> ClockSkew {
        let inner = self.inner.lock().unwrap();
        inner.clock_skew.clone()
    }

    async fn transport_bytes(&self) -> TransportBytes {
        let inner = self.inner.lock().unwrap();
        inner.transport_bytes.clone()
    }

    async fn recent_transit_traffic(&self) -> RecentTransitTraffic {
        let inner = self.inner.lock().unwrap();
        inner.recent_transit.clone()
    }

    async fn transit_bytes(&self) -> TransitBytes {
        let inner = self.inner.lock().unwrap();
        inner.transit_bytes.clone()
    }

    async fn tunnel_build_stats(&self) -> TunnelBuildStats {
        let inner = self.inner.lock().unwrap();
        inner.build_stats.clone()
    }

    async fn tunnel_summary(&self) -> TunnelSummary {
        let inner = self.inner.lock().unwrap();
        inner.tunnel_summary.clone()
    }

    async fn netdb_snapshot(&self) -> NetDbSnapshot {
        let inner = self.inner.lock().unwrap();
        inner.netdb.clone()
    }

    async fn udp_snapshot(&self) -> UdpSnapshot {
        let inner = self.inner.lock().unwrap();
        inner.udp.clone()
    }

    async fn tcp_snapshot(&self) -> TcpSnapshot {
        let inner = self.inner.lock().unwrap();
        inner.tcp.clone()
    }

    async fn known_peers(&self) -> Vec<PeerIdentity> {
        let inner = self.inner.lock().unwrap();
        inner.known_peers.clone()
    }

    async fn active_peers(&self) -> Vec<PeerIdentity> {
        let inner = self.inner.lock().unwrap();
        inner.active_peers.clone()
    }

    async fn peer_router_info(&self, peer_id: &str) -> Result<Option<String>, String> {
        let inner = self.inner.lock().unwrap();
        Ok(inner.peer_ris.get(peer_id).cloned())
    }

    async fn banned_peers(&self) -> Vec<BannedPeer> {
        let inner = self.inner.lock().unwrap();
        inner.banned_peers.clone()
    }

    async fn peer_limits(&self) -> PeerLimits {
        let inner = self.inner.lock().unwrap();
        inner.peer_limits.clone()
    }

    async fn active_peer_stats(&self) -> Vec<ActivePeerStats> {
        let inner = self.inner.lock().unwrap();
        inner.active_peer_stats.clone()
    }

    async fn i2ptunnel_stats(&self) -> I2PTunnelStats {
        let inner = self.inner.lock().unwrap();
        inner.i2ptunnel_stats.clone()
    }

    async fn log_snapshot(&self) -> LogSnapshot {
        let inner = self.inner.lock().unwrap();
        LogSnapshot {
            entries: inner.log_entries.clone(),
            generation: inner.log_generation,
        }
    }

    async fn log_clear(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.log_entries.clear();
        inner.log_generation += 1;
    }

    fn router_news(&self) -> String {
        String::new()
    }

    async fn share_ratio(&self) -> f64 {
        let inner = self.inner.lock().unwrap();
        inner.share_ratio
    }

    async fn configured_bw_limits(&self) -> (u64, u64) {
        let inner = self.inner.lock().unwrap();
        inner.configured_bw
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

    #[tokio::test]
    async fn fake_router_info_control_defaults() {
        let fake = FakeRouterInfoControl::new();
        assert_eq!(fake.router_version(), "Emissary 0.4.0");
        assert_eq!(fake.router_uptime_ms(), 0);
        assert!(fake.router_identity().unwrap().is_empty());

        let network = fake.network_snapshot().await;
        assert_eq!(network.ipv4_status, NetworkStatus::Unknown);

        let skew = fake.clock_skew().await;
        assert!(skew.skew_seconds.is_none());

        let tb = fake.transport_bytes().await;
        assert_eq!(tb.received, 0);
        assert_eq!(tb.sent, 0);

        let bs = fake.tunnel_build_stats().await;
        assert_eq!(bs.successes, 0);
        assert_eq!(bs.failures, 0);

        let peers = fake.known_peers().await;
        assert!(peers.is_empty());

        let active = fake.active_peers().await;
        assert!(active.is_empty());
    }

    #[tokio::test]
    async fn fake_router_info_control_setters() {
        let fake = FakeRouterInfoControl::new();
        fake.set_identity("test-identity-b64".to_string());
        fake.set_version("Test 1.0".to_string());
        fake.set_uptime_ms(60000);

        assert_eq!(fake.router_identity().unwrap(), "test-identity-b64");
        assert_eq!(fake.router_version(), "Test 1.0");
        assert_eq!(fake.router_uptime_ms(), 60000);

        fake.set_transport_bytes(TransportBytes {
            received: 1024,
            sent: 2048,
        });
        let tb = fake.transport_bytes().await;
        assert_eq!(tb.received, 1024);
        assert_eq!(tb.sent, 2048);

        fake.set_build_stats(TunnelBuildStats {
            successes: 10,
            failures: 2,
        });
        let bs = fake.tunnel_build_stats().await;
        assert_eq!(bs.successes, 10);
        assert_eq!(bs.failures, 2);
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

        let snap = fake.log_snapshot().await;
        assert_eq!(snap.entries.len(), 1);
        assert_eq!(snap.generation, 0);

        fake.log_clear().await;

        let snap = fake.log_snapshot().await;
        assert!(snap.entries.is_empty());
        assert_eq!(snap.generation, 1);
    }
}
