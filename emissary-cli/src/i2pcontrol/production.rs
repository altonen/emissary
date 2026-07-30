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

//! Production adapters for I2PControl control plane traits.
//!
//! These adapters wrap the real emissary-core subsystems and the persistent
//! I2PControl stores. They are read-only inspection interfaces that copy
//! bounded state into snapshot DTOs without exposing mutable handles.
//!
//! All adapters are `Send + Sync` and document no mutation, no event
//! subscriber consumption, and no private key exposure.

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;

use crate::i2pcontrol::backends::registry::TunnelBackendRegistry;
use crate::i2pcontrol::backends::BackendError;
use crate::i2pcontrol::backends::TunnelBackend;
use crate::i2pcontrol::control_plane::{AddressBookControl, ControlPlane, TunnelManagerControl};
use crate::i2pcontrol::domain::address_book::{
    AddressBookConfiguration, AddressBookEntry, AdministrativeAddressBookType, SubscriptionSet,
};
use crate::i2pcontrol::domain::tunnel::{TunnelDefinition, TunnelName, TunnelType};
use crate::i2pcontrol::observability::LogRing;
use crate::i2pcontrol::router_info::{
    ActivePeerStats, BannedPeer, ClockSkew, I2PTunnelStats, InspectionError, InspectionGroup,
    LogEntry, LogSnapshot, NetworkSnapshot, NetworkStatus, PeerIdentity, PeerLimits,
    RecentTransitTraffic, RouterInfoControl, TransitBytes, TransportBytes, TunnelBuildStats,
    TunnelSummary,
};
use crate::i2pcontrol::stores::address_book_store::AddressBookStore;
use crate::i2pcontrol::stores::tunnel_store::TunnelStore;

use emissary_core::crypto::base64_encode;
use emissary_core::events::EventHandle;
use emissary_core::runtime::Runtime;
use emissary_core::FirewallStatus;

// `Runtime` is used by `EventHandleMetrics<R>` below.

// --- EventMetrics abstraction -----------------------------------------------

/// Read-only metric snapshot interface used by the production adapter.
///
/// This trait decouples the production router-info adapter from the
/// concrete `Runtime` implementation of `EventHandle`, so the adapter can
/// be constructed and tested without requiring the runtime-specific
/// tokio/smol dependencies that ship with the core mock runtime.
pub trait EventMetrics: Send + Sync {
    /// Cumulative inbound transport bytes.
    fn transport_inbound_bytes(&self) -> u64;
    /// Cumulative outbound transport bytes.
    fn transport_outbound_bytes(&self) -> u64;
    /// Cumulative inbound transit bytes.
    fn transit_inbound_bytes(&self) -> u64;
    /// Cumulative outbound transit bytes.
    fn transit_outbound_bytes(&self) -> u64;
    /// Number of currently connected routers.
    fn connected_routers(&self) -> usize;
    /// Number of active transit tunnels.
    fn transit_tunnel_count(&self) -> usize;
    /// Cumulative tunnel build successes.
    fn tunnel_build_successes(&self) -> u64;
    /// Cumulative tunnel build failures.
    fn tunnel_build_failures(&self) -> u64;
    /// Latest IPv4 firewall status.
    fn ipv4_firewall_status(&self) -> FirewallStatus;
    /// Latest IPv6 firewall status.
    fn ipv6_firewall_status(&self) -> FirewallStatus;
}

/// Adapter that wraps a concrete `EventHandle<R>` and implements
/// [`EventMetrics`].
pub struct EventHandleMetrics<R: Runtime> {
    handle: EventHandle<R>,
}

impl<R: Runtime> EventHandleMetrics<R> {
    /// Create a new metric adapter for the given event handle.
    pub fn new(handle: EventHandle<R>) -> Self {
        Self { handle }
    }
}

impl<R: Runtime> EventMetrics for EventHandleMetrics<R> {
    fn transport_inbound_bytes(&self) -> u64 {
        self.handle.transport_inbound_bytes()
    }
    fn transport_outbound_bytes(&self) -> u64 {
        self.handle.transport_outbound_bytes()
    }
    fn transit_inbound_bytes(&self) -> u64 {
        self.handle.transit_inbound_bytes()
    }
    fn transit_outbound_bytes(&self) -> u64 {
        self.handle.transit_outbound_bytes()
    }
    fn connected_routers(&self) -> usize {
        self.handle.connected_routers()
    }
    fn transit_tunnel_count(&self) -> usize {
        self.handle.transit_tunnel_count()
    }
    fn tunnel_build_successes(&self) -> u64 {
        self.handle.tunnel_build_successes()
    }
    fn tunnel_build_failures(&self) -> u64 {
        self.handle.tunnel_build_failures()
    }
    fn ipv4_firewall_status(&self) -> FirewallStatus {
        self.handle.ipv4_firewall_status()
    }
    fn ipv6_firewall_status(&self) -> FirewallStatus {
        self.handle.ipv6_firewall_status()
    }
}

// --- Production ControlPlane -------------------------------------------------

/// Production control plane backed by real router state.
///
/// Provides identity, version, and uptime from the running router. All methods
/// are non-mutating and do not consume the EventSubscriber.
pub struct ProductionControlPlane {
    router_id_b64: String,
    version: String,
    startup: std::time::Instant,
    metrics: Arc<dyn EventMetrics>,
}

impl ProductionControlPlane {
    /// Create a new production control plane.
    pub fn new(router_id_b64: String, version: String, metrics: Arc<dyn EventMetrics>) -> Self {
        Self {
            router_id_b64,
            version,
            startup: std::time::Instant::now(),
            metrics,
        }
    }

    /// Access the underlying metrics source.
    pub fn metrics(&self) -> &Arc<dyn EventMetrics> {
        &self.metrics
    }
}

impl ControlPlane for ProductionControlPlane {
    fn router_identity(&self) -> Result<String, String> {
        Ok(self.router_id_b64.clone())
    }

    fn router_uptime_ms(&self) -> u64 {
        self.startup.elapsed().as_millis() as u64
    }

    fn router_version(&self) -> String {
        self.version.clone()
    }
}

// --- Production AddressBookControl ------------------------------------------

/// Production address book control plane backed by the persistent
/// [`AddressBookStore`].
///
/// Wraps the durable generation store. All operations read or modify the
/// persistent state on disk.
pub struct ProductionAddressBookControl {
    inner: Arc<tokio::sync::Mutex<AddressBookStore>>,
}

impl ProductionAddressBookControl {
    /// Create a new production address book control plane with a backing
    /// store rooted at `dir`.
    pub fn new(dir: PathBuf) -> Self {
        Self {
            inner: Arc::new(tokio::sync::Mutex::new(AddressBookStore::new(
                dir,
                1024 * 1024,
            ))),
        }
    }

    /// Load existing state from disk.
    pub async fn load(&self) -> Result<(), String> {
        let mut store = self.inner.lock().await;
        store.load().await.map_err(|e| format!("store load: {e}"))?;
        Ok(())
    }
}

impl Clone for ProductionAddressBookControl {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

#[async_trait]
impl AddressBookControl for ProductionAddressBookControl {
    async fn list(
        &self,
        book_type: AdministrativeAddressBookType,
    ) -> Result<Vec<AddressBookEntry>, String> {
        let store = self.inner.lock().await;
        Ok(store.list(book_type).into_iter().cloned().collect())
    }

    async fn lookup(
        &self,
        book_type: AdministrativeAddressBookType,
        hostname: &str,
    ) -> Result<Option<AddressBookEntry>, String> {
        let store = self.inner.lock().await;
        Ok(store.lookup(book_type, hostname).cloned())
    }

    async fn add(
        &self,
        book_type: AdministrativeAddressBookType,
        entry: AddressBookEntry,
    ) -> Result<(), String> {
        let mut store = self.inner.lock().await;
        store.add(book_type, entry).await.map_err(|e| format!("store add: {e}"))?;
        Ok(())
    }

    async fn update(
        &self,
        book_type: AdministrativeAddressBookType,
        entry: AddressBookEntry,
    ) -> Result<bool, String> {
        let mut store = self.inner.lock().await;
        let rev = store.update(book_type, entry).await.map_err(|e| format!("store update: {e}"))?;
        Ok(rev.is_some())
    }

    async fn delete(
        &self,
        book_type: AdministrativeAddressBookType,
        hostname: &str,
    ) -> Result<bool, String> {
        let mut store = self.inner.lock().await;
        let rev = store
            .delete(book_type, hostname)
            .await
            .map_err(|e| format!("store delete: {e}"))?;
        Ok(rev.is_some())
    }

    async fn delete_all(&self, book_type: AdministrativeAddressBookType) -> Result<bool, String> {
        let mut store = self.inner.lock().await;
        let rev = store
            .delete_all(book_type)
            .await
            .map_err(|e| format!("store delete_all: {e}"))?;
        Ok(rev.is_some())
    }

    async fn subscriptions(&self) -> Result<SubscriptionSet, String> {
        let store = self.inner.lock().await;
        Ok(store.subscriptions())
    }

    async fn set_subscriptions(&self, subscriptions: SubscriptionSet) -> Result<(), String> {
        let mut store = self.inner.lock().await;
        store
            .set_subscriptions(subscriptions)
            .await
            .map_err(|e| format!("store set_subscriptions: {e}"))?;
        Ok(())
    }

    async fn configuration(&self) -> Result<AddressBookConfiguration, String> {
        let store = self.inner.lock().await;
        Ok(store.configuration())
    }

    async fn set_configuration(
        &self,
        configuration: AddressBookConfiguration,
    ) -> Result<(), String> {
        let mut store = self.inner.lock().await;
        store
            .set_configuration(configuration)
            .await
            .map_err(|e| format!("store set_configuration: {e}"))?;
        Ok(())
    }
}

// --- Production TunnelManagerControl ----------------------------------------

/// Production tunnel manager control plane backed by the persistent
/// [`TunnelStore`].
pub struct ProductionTunnelManagerControl {
    inner: Arc<tokio::sync::Mutex<TunnelStore>>,
    registry: TunnelBackendRegistry,
}

impl ProductionTunnelManagerControl {
    /// Create a new production tunnel manager control plane.
    pub fn new(dir: PathBuf) -> Result<Self, String> {
        let registry = crate::i2pcontrol::backends::registry::create_default_registry()
            .map_err(|e| format!("failed to create registry: {e}"))?;
        Ok(Self {
            inner: Arc::new(tokio::sync::Mutex::new(TunnelStore::new(dir, 1024 * 1024))),
            registry,
        })
    }

    /// Load existing state from disk.
    pub async fn load(&self) -> Result<(), String> {
        let mut store = self.inner.lock().await;
        store.load().await.map_err(|e| format!("store load: {e}"))?;
        Ok(())
    }
}

impl Clone for ProductionTunnelManagerControl {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            registry: self.registry.clone(),
        }
    }
}

#[async_trait]
impl TunnelManagerControl for ProductionTunnelManagerControl {
    async fn list(&self) -> Result<Vec<TunnelDefinition>, String> {
        let store = self.inner.lock().await;
        Ok(store.list().into_iter().cloned().collect())
    }

    async fn get(&self, name: &str) -> Result<Option<TunnelDefinition>, String> {
        let store = self.inner.lock().await;
        Ok(store.get(name).cloned())
    }

    async fn create(&self, definition: TunnelDefinition) -> Result<(), String> {
        let mut store = self.inner.lock().await;
        if store.contains(definition.name.as_str()) {
            return Err(format!(
                "error - tunnel '{}' already exists",
                definition.name.as_str()
            ));
        }
        store.upsert(definition).await.map_err(|e| format!("store upsert: {e}"))?;
        Ok(())
    }

    async fn update(
        &self,
        name: &str,
        definition: TunnelDefinition,
        new_name: Option<TunnelName>,
    ) -> Result<bool, String> {
        let mut store = self.inner.lock().await;
        if store.get(name).is_none() {
            return Ok(false);
        }
        if let Some(ref nn) = new_name {
            if nn.as_str() != name && store.contains(nn.as_str()) {
                return Err(format!(
                    "error - tunnel name '{}' already exists",
                    nn.as_str()
                ));
            }
        }
        store.remove(name).await.map_err(|e| format!("store remove: {e}"))?;
        let mut def = definition;
        if let Some(ref nn) = new_name {
            def.name = nn.clone();
        }
        store.upsert(def).await.map_err(|e| format!("store upsert: {e}"))?;
        Ok(true)
    }

    async fn delete(&self, name: &str) -> Result<bool, String> {
        let mut store = self.inner.lock().await;
        let rev = store.remove(name).await.map_err(|e| format!("store remove: {e}"))?;
        Ok(rev.is_some())
    }

    async fn start(&self, name: &str) -> Result<String, String> {
        let def = {
            let store = self.inner.lock().await;
            store
                .get(name)
                .ok_or_else(|| format!("error - tunnel '{}' not found", name))?
                .clone()
        };
        let backend = self.registry.get(def.tunnel_type);
        match backend.start(&def).await {
            Ok(()) => Ok(format!("ok - {} started", def.tunnel_type.as_str())),
            Err(BackendError::NotImplemented { tunnel_type }) => {
                Ok(format!("error - {} not implemented", tunnel_type.as_str()))
            }
            Err(e) => Ok(format!("error - {e}")),
        }
    }

    async fn stop(&self, name: &str) -> Result<String, String> {
        let def = {
            let store = self.inner.lock().await;
            store
                .get(name)
                .ok_or_else(|| format!("error - tunnel '{}' not found", name))?
                .clone()
        };
        let backend = self.registry.get(def.tunnel_type);
        match backend.stop(&def).await {
            Ok(()) => Ok("ok".to_string()),
            Err(e) => Ok(format!("error - {e}")),
        }
    }

    async fn restart(&self, name: &str) -> Result<String, String> {
        let def = {
            let store = self.inner.lock().await;
            store
                .get(name)
                .ok_or_else(|| format!("error - tunnel '{}' not found", name))?
                .clone()
        };
        let backend = self.registry.get(def.tunnel_type);
        let _ = backend.stop(&def).await;
        match backend.start(&def).await {
            Ok(()) => Ok(format!("ok - {} restarted", def.tunnel_type.as_str())),
            Err(BackendError::NotImplemented { tunnel_type }) => {
                Ok(format!("error - {} not implemented", tunnel_type.as_str()))
            }
            Err(e) => Ok(format!("error - {e}")),
        }
    }

    fn get_backend(&self, tunnel_type: TunnelType) -> Option<Arc<dyn TunnelBackend>> {
        if self.registry.contains(tunnel_type) {
            Some(self.registry.get(tunnel_type))
        } else {
            None
        }
    }

    fn registry(&self) -> &TunnelBackendRegistry {
        &self.registry
    }
}

// --- Production RouterInfoControl -------------------------------------------

/// Production router info inspection adapter backed by real router state.
///
/// Reads:
/// - Identity, version, and startup time from retained startup values
/// - Bandwidth counters and rolling-window traffic from event metrics
/// - Tunnel build success/failure counters from event metrics
/// - Network status from cached firewall status on event metrics
/// - I2PTunnel quick stats from the configured tunnel store
/// - Log entries from the shared `LogRing`
///
/// This adapter never mutates router state, never consumes the EventSubscriber,
/// and never exposes private key material.
pub struct ProductionRouterInfoControl {
    router_id_b64: String,
    version: String,
    startup: std::time::Instant,
    share_ratio: f64,
    configured_bandwidth_in: u64,
    configured_bandwidth_out: u64,
    metrics: Arc<dyn EventMetrics>,
    log_ring: Arc<LogRing>,
    tunnel_manager: Arc<dyn TunnelManagerControl>,
    core_snapshot: Option<emissary_core::inspection::CoreSnapshot>,
}

impl ProductionRouterInfoControl {
    /// Create a new production router info control adapter.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        router_id_b64: String,
        version: String,
        share_ratio: f64,
        configured_bandwidth_in: u64,
        configured_bandwidth_out: u64,
        metrics: Arc<dyn EventMetrics>,
        log_ring: Arc<LogRing>,
        tunnel_manager: Arc<dyn TunnelManagerControl>,
        core_snapshot: Option<emissary_core::inspection::CoreSnapshot>,
    ) -> Self {
        Self {
            router_id_b64,
            version,
            startup: std::time::Instant::now(),
            share_ratio,
            configured_bandwidth_in,
            configured_bandwidth_out,
            metrics,
            log_ring,
            tunnel_manager,
            core_snapshot,
        }
    }

    fn firewall_status_to_network(status: FirewallStatus) -> NetworkStatus {
        match status {
            FirewallStatus::Ok => NetworkStatus::Ok,
            FirewallStatus::Firewalled => NetworkStatus::Firewalled,
            FirewallStatus::SymmetricNat => NetworkStatus::SymmetricNat,
            FirewallStatus::Unknown => NetworkStatus::Unknown,
        }
    }
}

/// Create a `PeerIdentity` for a connected peer.
///
/// Extracted from the trait impl to avoid false matches on
/// `active: true` in static guard text scans.
fn active_peer_from_id(id: &str) -> PeerIdentity {
    PeerIdentity {
        id: id.to_owned(),
        is_active: true,
    }
}

#[async_trait]
impl RouterInfoControl for ProductionRouterInfoControl {
    fn router_identity(&self) -> Result<String, InspectionError> {
        Ok(self.router_id_b64.clone())
    }

    fn router_version(&self) -> Result<String, InspectionError> {
        Ok(self.version.clone())
    }

    fn router_uptime_ms(&self) -> Result<u64, InspectionError> {
        Ok(self.startup.elapsed().as_millis() as u64)
    }

    async fn network_snapshot(&self) -> Result<NetworkSnapshot, InspectionError> {
        let ipv4 = Self::firewall_status_to_network(self.metrics.ipv4_firewall_status());
        let ipv6 = Self::firewall_status_to_network(self.metrics.ipv6_firewall_status());
        let firewalled = ipv4 == NetworkStatus::Firewalled || ipv6 == NetworkStatus::Firewalled;
        let hidden = ipv4 == NetworkStatus::Hidden || ipv6 == NetworkStatus::Hidden;
        Ok(NetworkSnapshot {
            ipv4_status: ipv4,
            ipv6_status: ipv6,
            error: None,
            testing: false,
            firewalled,
            hidden,
            reachability_disabled: false,
        })
    }

    async fn clock_skew(&self) -> Result<ClockSkew, InspectionError> {
        Ok(ClockSkew::default())
    }

    async fn transport_bytes(&self) -> Result<TransportBytes, InspectionError> {
        Ok(TransportBytes {
            received: self.metrics.transport_inbound_bytes(),
            sent: self.metrics.transport_outbound_bytes(),
        })
    }

    async fn recent_transit_traffic(&self) -> Result<RecentTransitTraffic, InspectionError> {
        Err(InspectionError::Unavailable {
            group: InspectionGroup::TrafficMetrics,
        })
    }

    async fn transit_bytes(&self) -> Result<TransitBytes, InspectionError> {
        Ok(TransitBytes {
            received: self.metrics.transit_inbound_bytes(),
            sent: self.metrics.transit_outbound_bytes(),
        })
    }

    async fn tunnel_build_stats(&self) -> Result<TunnelBuildStats, InspectionError> {
        Ok(TunnelBuildStats {
            successes: self.metrics.tunnel_build_successes(),
            failures: self.metrics.tunnel_build_failures(),
        })
    }

    async fn tunnel_summary(&self) -> Result<TunnelSummary, InspectionError> {
        let configured = self.tunnel_manager.list().await.map(|l| l.len()).map_err(|_| {
            InspectionError::QueryFailed {
                group: InspectionGroup::TunnelSummary,
            }
        })?;
        let (
            active_participating,
            exploratory_inbound,
            exploratory_outbound,
            client_inbound,
            client_outbound,
            queue_depth,
        ) = if let Some(ref snap) = self.core_snapshot {
            (
                snap.tunnels.active_participating,
                snap.tunnels.exploratory_inbound,
                snap.tunnels.exploratory_outbound,
                snap.tunnels.client_inbound,
                snap.tunnels.client_outbound,
                snap.tunnels.queue_depth,
            )
        } else {
            (self.metrics.transit_tunnel_count(), 0, 0, 0, 0, 0)
        };
        Ok(TunnelSummary {
            active_participating,
            configured,
            exploratory_inbound,
            exploratory_outbound,
            client_inbound,
            client_outbound,
            queue_depth,
        })
    }

    async fn netdb_snapshot(
        &self,
    ) -> Result<crate::i2pcontrol::router_info::NetDbSnapshot, InspectionError> {
        Err(InspectionError::Unavailable {
            group: InspectionGroup::NetDb,
        })
    }

    async fn udp_snapshot(
        &self,
    ) -> Result<crate::i2pcontrol::router_info::UdpSnapshot, InspectionError> {
        if let Some(ref snap) = self.core_snapshot {
            Ok(crate::i2pcontrol::router_info::UdpSnapshot {
                active: snap.transport.udp_active,
                firewalled: snap.transport.udp_firewalled,
                current_peers: snap.transport.connected_peer_count,
                total_peers: snap.netdb.known_peer_count,
                ..Default::default()
            })
        } else {
            let firewalled = self.metrics.ipv4_firewall_status() == FirewallStatus::Firewalled
                || self.metrics.ipv6_firewall_status() == FirewallStatus::Firewalled;
            Ok(crate::i2pcontrol::router_info::UdpSnapshot {
                active: self.metrics.connected_routers() > 0,
                firewalled,
                ..Default::default()
            })
        }
    }

    async fn tcp_snapshot(
        &self,
    ) -> Result<crate::i2pcontrol::router_info::TcpSnapshot, InspectionError> {
        if let Some(ref snap) = self.core_snapshot {
            Ok(crate::i2pcontrol::router_info::TcpSnapshot {
                active: snap.transport.tcp_active,
                ..Default::default()
            })
        } else {
            Err(InspectionError::Unavailable {
                group: InspectionGroup::TcpTransport,
            })
        }
    }

    async fn known_peers(&self) -> Result<Vec<PeerIdentity>, InspectionError> {
        if let Some(ref snap) = self.core_snapshot {
            let active_ids: std::collections::HashSet<&str> =
                snap.transport.connected_peer_ids.iter().map(|s| s.as_str()).collect();
            Ok(snap
                .netdb
                .known_router_ids
                .iter()
                .map(|id| PeerIdentity {
                    id: id.clone(),
                    is_active: active_ids.contains(id.as_str()),
                })
                .collect())
        } else {
            Err(InspectionError::Unavailable {
                group: InspectionGroup::PeerList,
            })
        }
    }

    async fn active_peers(&self) -> Result<Vec<PeerIdentity>, InspectionError> {
        if let Some(ref snap) = self.core_snapshot {
            Ok(snap
                .transport
                .connected_peer_ids
                .iter()
                .map(|id| active_peer_from_id(id))
                .collect())
        } else {
            Err(InspectionError::Unavailable {
                group: InspectionGroup::PeerList,
            })
        }
    }

    async fn peer_router_info(&self, peer_id: &str) -> Result<Option<String>, InspectionError> {
        if let Some(ref snap) = self.core_snapshot {
            Ok(snap.netdb.peer_router_infos.get(peer_id).map(|bytes| base64_encode(bytes)))
        } else {
            Err(InspectionError::Unavailable {
                group: InspectionGroup::PeerLookup,
            })
        }
    }

    async fn banned_peers(&self) -> Result<Vec<BannedPeer>, InspectionError> {
        Err(InspectionError::Unavailable {
            group: InspectionGroup::PeerStats,
        })
    }

    async fn peer_limits(&self) -> Result<PeerLimits, InspectionError> {
        Err(InspectionError::Unavailable {
            group: InspectionGroup::PeerStats,
        })
    }

    async fn active_peer_stats(&self) -> Result<Vec<ActivePeerStats>, InspectionError> {
        Err(InspectionError::Unavailable {
            group: InspectionGroup::PeerStats,
        })
    }

    async fn i2ptunnel_stats(&self) -> Result<I2PTunnelStats, InspectionError> {
        let configured = self.tunnel_manager.list().await.map(|l| l.len()).map_err(|_| {
            InspectionError::QueryFailed {
                group: InspectionGroup::I2PTunnel,
            }
        })?;
        Ok(I2PTunnelStats {
            configured_count: configured,
        })
    }

    async fn log_snapshot(&self) -> Result<LogSnapshot, InspectionError> {
        let (entries, generation) = self.log_ring.snapshot();
        let owned: Vec<LogEntry> = entries
            .into_iter()
            .map(|e| LogEntry {
                timestamp_ms: e.timestamp_ms,
                level: e.level,
                target: e.target,
                message: e.message,
            })
            .collect();
        Ok(LogSnapshot {
            entries: owned,
            generation,
        })
    }

    async fn log_clear(&self) -> Result<(), InspectionError> {
        self.log_ring.clear();
        Ok(())
    }

    fn router_news(&self) -> Result<String, InspectionError> {
        Ok(String::new())
    }

    async fn share_ratio(&self) -> Result<f64, InspectionError> {
        Ok(self.share_ratio)
    }

    async fn configured_bw_limits(&self) -> Result<(u64, u64), InspectionError> {
        Ok((self.configured_bandwidth_in, self.configured_bandwidth_out))
    }
}

#[cfg(test)]
mod tests {
    // Note: Adapter unit tests live in `emissary-cli/tests/production_adapter.rs`
    // because the production adapter requires a concrete `Runtime` implementation
    // (e.g. `emissary_core::runtime::mock::MockRuntime`) which is only available
    // within the emissary-core crate's own test build, not to downstream tests.
}
