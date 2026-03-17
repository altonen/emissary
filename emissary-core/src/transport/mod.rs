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

use crate::{
    crypto::base64_encode,
    error::QueryError,
    events::EventHandle,
    netdb::NetDbHandle,
    primitives::{Date, RouterAddress, RouterId, RouterInfo, Str, TransportKind},
    router::context::RouterContext,
    runtime::{Counter, Gauge, JoinSet, MetricType, MetricsHandle, Runtime},
    subsystem::SubsystemEvent,
    transport::{metrics::*, ntcp2::Ntcp2Context, ssu2::Ssu2Context},
    Ntcp2Config, Ssu2Config,
};

use bytes::Bytes;
use futures::{FutureExt, Stream, StreamExt};
use hashbrown::{HashMap, HashSet};
use thingbuf::mpsc::{errors::TrySendError, Receiver, Sender};

use alloc::{
    boxed::Box,
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};
use core::{
    future::Future,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

mod metrics;
mod ntcp2;
mod ssu2;

pub use ntcp2::Ntcp2Transport;
pub use ssu2::Ssu2Transport;

#[cfg(feature = "fuzz")]
pub use {
    ntcp2::MessageBlock,
    ssu2::{Block, HeaderReader},
};

/// Logging target for the file.
const LOG_TARGET: &str = "emissary::transport-manager";

/// Local [`RouterInfo`] republish interval.
///
/// Local router info gets republished to `NetDb` every 15 minutes.
const ROUTER_INFO_REPUBLISH_INTERVAL: Duration = Duration::from_secs(15 * 60);

/// Introducer expiration.
const INTRODUCER_EXPIRATION: Duration = Duration::from_secs(80 * 60);

/// Termination reason.
#[derive(Debug, Default, PartialEq, Eq, Clone, Copy)]
pub enum TerminationReason {
    /// Unspecified or normal termination.
    #[default]
    Unspecified,

    /// Termination block was received.
    TerminationReceived,

    /// Idle timeout.
    IdleTimeout,

    /// Socket was closed (NTCP2 only).
    IoError,

    /// Router is shutting down.
    RouterShutdown,

    /// AEAD failure.
    AeadFailure,

    /// Incompatible options,
    IncompatibleOptions,

    /// Unsupported signature kind.
    IncompatibleSignatureKind,

    /// Clock skew.
    ClockSkew,

    /// Padding violation.
    PaddinViolation,

    /// Payload format error.
    PayloadFormatError,

    /// AEAD framing error.
    AeadFramingError,

    /// NTCP2 handshake error.
    Ntcp2HandshakeError(u8),

    /// SSU2 handshake error.
    Ssu2HandshakeError(u8),

    /// Intra frame timeout.
    IntraFrameReadTimeout,

    /// Invalid router info.
    InvalidRouterInfo,

    /// Router has been banned.
    Banned,

    /// Timeout (SSU2 only)
    Timeout,

    /// Bad token (SSU2 only).
    BadToken,

    /// Connection limit reached (SSU2 only)
    ConnectionLimits,

    /// Incompatible version (SSU2 only)
    IncompatibleVersion,

    /// Wrong network ID (SSU2 only)
    WrongNetId,

    /// Replaced by new session (SSU2 only)
    ReplacedByNewSession,
}

impl TerminationReason {
    /// Get [`TerminationReason`] from an NTCP2 termination reason.
    pub fn ntcp2(value: u8) -> Self {
        match value {
            0 => TerminationReason::Unspecified,
            1 => TerminationReason::TerminationReceived,
            2 => TerminationReason::IdleTimeout,
            3 => TerminationReason::RouterShutdown,
            4 => TerminationReason::AeadFailure,
            5 => TerminationReason::IncompatibleOptions,
            6 => TerminationReason::IncompatibleSignatureKind,
            7 => TerminationReason::ClockSkew,
            8 => TerminationReason::PaddinViolation,
            9 => TerminationReason::AeadFramingError,
            10 => TerminationReason::PayloadFormatError,
            11 => TerminationReason::Ntcp2HandshakeError(1),
            12 => TerminationReason::Ntcp2HandshakeError(2),
            13 => TerminationReason::Ntcp2HandshakeError(3),
            14 => TerminationReason::IntraFrameReadTimeout,
            15 => TerminationReason::InvalidRouterInfo,
            16 => TerminationReason::InvalidRouterInfo,
            17 => TerminationReason::Banned,
            _ => TerminationReason::Unspecified,
        }
    }

    /// Get [`TerminationReason`] from an SSU2 termination reason.
    pub fn ssu2(value: u8) -> Self {
        match value {
            0 => TerminationReason::Unspecified,
            1 => TerminationReason::TerminationReceived,
            2 => TerminationReason::IdleTimeout,
            3 => TerminationReason::RouterShutdown,
            4 => TerminationReason::AeadFailure,
            5 => TerminationReason::IncompatibleOptions,
            6 => TerminationReason::IncompatibleSignatureKind,
            7 => TerminationReason::ClockSkew,
            8 => TerminationReason::PaddinViolation,
            9 => TerminationReason::AeadFramingError,
            10 => TerminationReason::PayloadFormatError,
            11 => TerminationReason::Ssu2HandshakeError(1),
            12 => TerminationReason::Ssu2HandshakeError(2),
            13 => TerminationReason::Ssu2HandshakeError(3),
            14 => TerminationReason::IntraFrameReadTimeout,
            15 => TerminationReason::InvalidRouterInfo,
            16 => TerminationReason::InvalidRouterInfo,
            17 => TerminationReason::Banned,
            18 => TerminationReason::BadToken,
            19 => TerminationReason::ConnectionLimits,
            20 => TerminationReason::IncompatibleVersion,
            21 => TerminationReason::WrongNetId,
            22 => TerminationReason::ReplacedByNewSession,
            _ => TerminationReason::Unspecified,
        }
    }

    /// Convert NTCP2 termination reason into `u8`.
    pub fn from_ntcp2(self) -> u8 {
        match self {
            TerminationReason::Unspecified => 0,
            TerminationReason::TerminationReceived => 1,
            TerminationReason::IdleTimeout => 2,
            TerminationReason::RouterShutdown => 3,
            TerminationReason::AeadFailure => 4,
            TerminationReason::IncompatibleOptions => 5,
            TerminationReason::IncompatibleSignatureKind => 6,
            TerminationReason::ClockSkew => 7,
            TerminationReason::PaddinViolation => 8,
            TerminationReason::AeadFramingError => 9,
            TerminationReason::PayloadFormatError => 10,
            TerminationReason::Ntcp2HandshakeError(1) => 11,
            TerminationReason::Ntcp2HandshakeError(2) => 12,
            TerminationReason::Ntcp2HandshakeError(3) => 13,
            TerminationReason::IntraFrameReadTimeout => 14,
            TerminationReason::InvalidRouterInfo => 15,
            TerminationReason::Banned => 17,
            _ => 0,
        }
    }

    /// Convert SSU2 termination reason into `u8`.
    pub fn from_ssu2(self) -> u8 {
        match self {
            TerminationReason::Unspecified => 0,
            TerminationReason::TerminationReceived => 1,
            TerminationReason::IdleTimeout => 2,
            TerminationReason::RouterShutdown => 3,
            TerminationReason::AeadFailure => 4,
            TerminationReason::IncompatibleOptions => 5,
            TerminationReason::IncompatibleSignatureKind => 6,
            TerminationReason::ClockSkew => 7,
            TerminationReason::PaddinViolation => 8,
            TerminationReason::AeadFramingError => 9,
            TerminationReason::PayloadFormatError => 10,
            TerminationReason::Ssu2HandshakeError(1) => 11,
            TerminationReason::Ssu2HandshakeError(2) => 12,
            TerminationReason::Ssu2HandshakeError(3) => 13,
            TerminationReason::IntraFrameReadTimeout => 14,
            TerminationReason::InvalidRouterInfo => 15,
            TerminationReason::Banned => 17,
            TerminationReason::BadToken => 18,
            TerminationReason::ConnectionLimits => 19,
            TerminationReason::IncompatibleVersion => 20,
            TerminationReason::WrongNetId => 21,
            TerminationReason::ReplacedByNewSession => 22,
            _ => 255,
        }
    }
}

/// Firewall status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirewallStatus {
    /// Router's firewall status is unknown.
    Unknown,

    /// SSU2 has detected that the router is firewalled.
    Firewalled,

    /// Firewall is open.
    Ok,

    /// Router is behind a symmetric NAT.
    SymmetricNat,
}

/// Direction of the connection.
#[derive(Debug, Clone, Copy)]
pub enum Direction {
    /// Inbound connection.
    Inbound,

    /// Outbound connection.
    Outbound,
}

/// Transport event.
#[derive(Debug)]
pub enum TransportEvent {
    /// Connection successfully established to router.
    ConnectionEstablished {
        /// Address of remote router.
        address: SocketAddr,

        /// Is this an outbound or an inbound connection.
        direction: Direction,

        /// ID of the connected router.
        router_id: RouterId,
    },

    /// Connection closed to router.
    ConnectionClosed {
        /// ID of the disconnected router.
        router_id: RouterId,

        /// Reason for the termination.
        reason: TerminationReason,
    },

    /// Failed to dial peer.
    ///
    /// The connection is considered failed if we failed to reach the router
    /// or if there was an error during handshaking.
    ConnectionFailure {
        /// ID of the remote router.
        router_id: RouterId,
    },

    /// SSU2 has learned something about the router's firewall status.
    FirewallStatus {
        /// Firewall status.
        status: FirewallStatus,
    },

    /// External address discovered.
    ExternalAddress {
        /// Our external address.
        address: SocketAddr,
    },

    /// New introducer
    IntroducerAdded {
        /// Relay tag.
        relay_tag: u32,

        /// Router ID of Bob.
        router_id: RouterId,

        /// When does the introducer expire.
        expires: Duration,
    },

    /// Introducer removed.
    IntroducerRemoved {
        /// Router ID of Bob.
        router_id: RouterId,
    },
}

/// Transport interface.
pub trait Transport: Stream + Unpin + Send {
    /// Connect to `router`.
    fn connect(&mut self, router: RouterInfo);

    /// Accept connection and start its event loop.
    fn accept(&mut self, router: &RouterId);

    /// Reject connection.
    fn reject(&mut self, router: &RouterId);
}

/// Builder for [`TransportManager`].
pub struct TransportManagerBuilder<R: Runtime> {
    /// Allow local addresses.
    allow_local: bool,

    /// Router capability override.
    caps: Option<Str>,

    /// RX channel for receiving dial requests.
    dial_rx: Receiver<RouterId>,

    /// Local router info.
    local_router_info: RouterInfo,

    /// Handle to [`NetDb`].
    netdb_handle: Option<NetDbHandle>,

    /// NTCP2 config.
    ntcp2_config: Option<Ntcp2Config>,

    /// Router context.
    router_ctx: RouterContext<R>,

    /// SSU2 config.
    ssu2_config: Option<Ssu2Config>,

    /// Supported transports.
    supported_transports: HashSet<TransportKind>,

    /// Are transit tunnels disabled.
    transit_tunnels_disabled: bool,

    /// TX channel given to connections which they use to send inbound
    /// messages to `SubsystemManager` for processing.
    transport_tx: Sender<SubsystemEvent>,

    /// Enabled transports.
    transports: Vec<Box<dyn Transport<Item = TransportEvent>>>,
}

impl<R: Runtime> TransportManagerBuilder<R> {
    /// Create new [`TransportManagerBuilder`].
    pub fn new(
        router_ctx: RouterContext<R>,
        local_router_info: RouterInfo,
        allow_local: bool,
        dial_rx: Receiver<RouterId>,
        transport_tx: Sender<SubsystemEvent>,
    ) -> Self {
        Self {
            allow_local,
            caps: None,
            dial_rx,
            local_router_info,
            netdb_handle: None,
            ntcp2_config: None,
            router_ctx,
            ssu2_config: None,
            supported_transports: HashSet::new(),
            transit_tunnels_disabled: false,
            transports: Vec::with_capacity(2),
            transport_tx,
        }
    }

    /// Register NTCP2 as an active transport.
    pub fn register_ntcp2(&mut self, context: Ntcp2Context<R>) {
        self.supported_transports.extend(context.classify());
        self.ntcp2_config = Some(context.config());
        self.transports.push(Box::new(Ntcp2Transport::new(
            context,
            self.allow_local,
            self.router_ctx.clone(),
            self.transport_tx.clone(),
        )))
    }

    /// Register SSU2 as an active transport.
    pub fn register_ssu2(&mut self, context: Ssu2Context<R>) {
        self.supported_transports.insert(context.classify());
        self.ssu2_config = Some(context.config());
        self.transports.push(Box::new(Ssu2Transport::new(
            context,
            self.allow_local,
            self.router_ctx.clone(),
            self.transport_tx.clone(),
        )))
    }

    /// Register [`NetDbHandle`].
    pub fn register_netdb_handle(&mut self, netdb_handle: NetDbHandle) {
        self.netdb_handle = Some(netdb_handle);
    }

    /// Specify whether transit tunnels are disabled or not.
    pub fn with_transit_tunnels_disabled(&mut self, transit_tunnels_disabled: bool) -> &mut Self {
        self.transit_tunnels_disabled = transit_tunnels_disabled;
        self
    }

    /// Specify router capability override.
    pub fn with_capabilities(&mut self, caps: String) -> &mut Self {
        self.caps = Some(Str::from(caps));
        self
    }

    /// Build into [`TransportManager`].
    pub fn build(self) -> TransportManager<R> {
        TransportManager {
            caps: self.caps,
            dial_rx: self.dial_rx,
            event_handle: self.router_ctx.event_handle().clone(),
            external_address: None,
            firewall_status: FirewallStatus::Unknown,
            introducers: Vec::new(),
            local_router_info: self.local_router_info,
            netdb_handle: self.netdb_handle.expect("to exist"),
            ntcp2_config: self.ntcp2_config,
            pending_connections: HashMap::new(),
            pending_introducers: HashMap::new(),
            pending_queries: HashSet::new(),
            pending_query_futures: R::join_set(),
            poll_index: 0usize,
            router_ctx: self.router_ctx,
            // publish the router info 10 seconds after booting, otherwise republish it periodically
            // in intervals of `ROUTER_INFO_REPUBLISH_INTERVAL`
            router_info_republish_timer: R::timer(Duration::from_secs(10)),
            routers: HashMap::new(),
            ssu2_config: self.ssu2_config,
            supported_transports: self.supported_transports,
            transit_tunnels_disabled: self.transit_tunnels_disabled,
            transports: self.transports,
            transport_tx: self.transport_tx.clone(),
        }
    }
}

/// Object representing the state of a pending connection that requires relay.
///
/// Each unreachable SSU2 connection that requires and has published introducers has an
/// `IntroducerConnection` which tracks the connection state while it's pending. If an introducer
/// for the router is already connected, the router is dialed immediately, without creating context.
///
/// `IntroducerConnection` tracks the overall state of all intrducers and it is destroyed when:
///
/// a) connection was established the one of the introducers, allowing the router to be dialed
/// b) local router failed to dial all of the introducers, making it impossible to ial the router
struct IntroducerConnection {
    /// `RouterInfo` of the router that needs relay.
    router_info: RouterInfo,

    /// Pending connections to introducers.
    ///
    /// All introducers are dialed in parallel and whichever of the connections succeeds
    /// first is selected as the relay.
    ///
    /// If all connections fail, the router cannot be dialed and a dial failure is reported.
    pending_connections: HashSet<RouterId>,

    /// Pending router info queries.
    ///
    /// If an introducer's router info is not known, it must queried from NetDb.
    ///
    /// If all introducer router info queries fail, dialing the router fails.
    pending_queries: HashSet<RouterId>,
}

/// Transport manager.
///
/// Transport manager is responsible for connecting the higher-level subsystems
/// together with enabled, lower-level transports and polling for polling those
/// transports so that they can make progress.
pub struct TransportManager<R: Runtime> {
    /// Router capability override.
    caps: Option<Str>,

    /// RX channel for receiving dial requests.
    dial_rx: Receiver<RouterId>,

    /// Event handle.
    event_handle: EventHandle<R>,

    /// External address, if any.
    external_address: Option<Ipv4Addr>,

    /// Firewall status.
    firewall_status: FirewallStatus,

    /// Introducers.
    ///
    /// Linear scans are OK since there are only 1-3 introducers.
    introducers: Vec<(RouterId, u32, Duration)>,

    /// Local router info.
    local_router_info: RouterInfo,

    /// Handle to [`NetDb`].
    netdb_handle: NetDbHandle,

    /// NTCP2 config.
    ntcp2_config: Option<Ntcp2Config>,

    /// Pending outbound connections.
    pending_connections: HashMap<RouterId, Vec<RouterId>>,

    /// Pending introducer connections.
    ///
    /// Indexed by the ID of the router that needs relay.
    pending_introducers: HashMap<RouterId, IntroducerConnection>,

    /// Pending queries.
    pending_queries: HashSet<RouterId>,

    /// Pending router info queries.
    pending_query_futures: R::JoinSet<(RouterId, Result<(), QueryError>)>,

    /// Poll index for transports.
    poll_index: usize,

    /// Router context.
    router_ctx: RouterContext<R>,

    /// Router info republish timer.
    router_info_republish_timer: R::Timer,

    /// Connected routers.
    ///
    /// The value indicates if the router is connected over IPv4.
    routers: HashMap<RouterId, bool>,

    /// SSU2 config.
    ssu2_config: Option<Ssu2Config>,

    /// Supported transports.
    supported_transports: HashSet<TransportKind>,

    /// Are transit tunnels disabled.
    transit_tunnels_disabled: bool,

    /// TX channel for sending transport-related events to `SubsystemManager`.
    transport_tx: Sender<SubsystemEvent>,

    /// Enabled transports.
    transports: Vec<Box<dyn Transport<Item = TransportEvent>>>,
}

impl<R: Runtime> TransportManager<R> {
    /// Collect `TransportManager`-related metric counters, gauges and histograms.
    pub fn metrics(metrics: Vec<MetricType>) -> Vec<MetricType> {
        let metrics = register_metrics(metrics);
        let metrics = Ntcp2Transport::<R>::metrics(metrics);

        Ssu2Transport::<R>::metrics(metrics)
    }

    /// Add external address for the router.
    pub fn add_external_address(&mut self, address: Ipv4Addr) {
        tracing::info!(
            target: LOG_TARGET,
            ?address,
            "external address discovered",
        );

        match (self.external_address, address) {
            (None, address) => {
                tracing::info!(
                    target: LOG_TARGET,
                    ?address,
                    "external address discovered, publishing new router info",
                );

                self.external_address = Some(address);
            }
            (Some(old_address), new_address) if old_address != new_address => {
                tracing::info!(
                    target: LOG_TARGET,
                    ?old_address,
                    ?new_address,
                    "new external address discovered, publishing new router info",
                );

                self.external_address = Some(address);
            }
            _ => return,
        };

        match &self.ntcp2_config {
            Some(Ntcp2Config {
                port,
                ipv4_host,
                publish: true,
                iv,
                ..
            }) => match (ipv4_host, address) {
                (None, address) => match self.local_router_info.ntcp2_ipv4_mut() {
                    Some(ntcp2) => ntcp2.into_reachable_ntcp2(*iv, *port, IpAddr::V4(address)),
                    None => tracing::warn!(
                        target: LOG_TARGET,
                        "unable to make ntcp2 address reachable, transport not enabled",
                    ),
                },
                (Some(published), address) if published == &address => {}
                (Some(published), address) => tracing::warn!(
                    target: LOG_TARGET,
                    ?published,
                    ?address,
                    "external address doesn't match published address, router address not updated",
                ),
            },
            _ => tracing::trace!(
                target: LOG_TARGET,
                "ntcp2 not active or unpublished, router address not updated",
            ),
        }

        match &self.ssu2_config {
            Some(Ssu2Config {
                port,
                host,
                publish: true,
                static_key,
                intro_key,
            }) => match (host, address) {
                (None, address) => {
                    self.local_router_info.addresses.push(RouterAddress::new_published_ssu2(
                        *static_key,
                        *intro_key,
                        *port,
                        address,
                    ));
                }
                (Some(published), address) if published == &address => {}
                (Some(published), address) => tracing::warn!(
                    target: LOG_TARGET,
                    ?published,
                    ?address,
                    "external address doesn't match published ssu2 address, router address not updated",
                ),
            },
            _ => tracing::trace!(
                target: LOG_TARGET,
                "ssu2 not active or unpublished, router address not updated",
            ),
        }
    }

    /// Report dial failure to `SubsystemManager`.
    ///
    /// Attempts to send the event in a non-blocking way and if the channel is clogged, informs
    /// `SubsystemManager` in the background.
    fn report_dial_failure(&self, router_id: RouterId) {
        match self.transport_tx.try_send(SubsystemEvent::ConnectionFailure { router_id }) {
            Ok(()) => {}
            Err(TrySendError::Full(event)) => {
                let transport_tx = self.transport_tx.clone();

                R::spawn(async move {
                    let _ = transport_tx.send(event).await;
                });
            }
            Err(error) => tracing::error!(
                target: LOG_TARGET,
                ?error,
                "failed to inform subsystem manager of a closed connection",
            ),
        }
    }

    /// Send `RouterInfo` query to NetDb for `router_id`.
    ///
    /// `clients` contain the list routers that are interested in the result of the query.
    fn send_router_info_query(&mut self, router_id: RouterId, clients: Vec<RouterId>) {
        match self.netdb_handle.try_query_router_info(router_id.clone()) {
            Err(error) => tracing::warn!(
                target: LOG_TARGET,
                %router_id,
                ?error,
                "failed to send router info query",
            ),
            Ok(rx) => {
                self.pending_connections.insert(router_id.clone(), clients);
                self.pending_queries.insert(router_id.clone());
                self.pending_query_futures.push(async move {
                    match rx.await {
                        // `Err(_)` indicates that `NetDb` didn't finish the query and
                        // instead dropped the channel which shouldn't happen unless there
                        // is a bug in router info query logic
                        Err(error) => {
                            tracing::debug!(
                                target: LOG_TARGET,
                                %router_id,
                                ?error,
                                "netdb didn't properly finish the router info lookup",
                            );

                            (router_id, Err(QueryError::Timeout))
                        }
                        Ok(Err(error)) => (router_id, Err(error)),
                        Ok(Ok(lease_set)) => (router_id, Ok(lease_set)),
                    }
                });
            }
        }
    }

    /// Attempt to dial `router_id`.
    ///
    /// If `router_id` is not found in local storage, send [`RouterInfo`] query for `router_id` to
    /// [`NetDb`] and if the [`RouterInfo`] is found, attempt to dial it.
    ///
    /// `clients` are IDs of the routers that are interested in knowing about the result of this
    /// connection attempt.
    fn on_dial_router(&mut self, router_id: RouterId, clients: Vec<RouterId>) {
        if &router_id == self.router_ctx.router_id() {
            tracing::error!(target: LOG_TARGET, "tried to dial self");
            debug_assert!(false);
            return;
        }

        // `SubsystemManager` might send an outbound connection request just before an inbound
        // connection has been accepted by the `TransportManager`
        //
        // ignore these requests
        if self.routers.contains_key(&router_id) {
            tracing::debug!(
                target: LOG_TARGET,
                %router_id,
                "router is already connected, ignoring dial request",
            );
            return;
        }

        match self.router_ctx.profile_storage().get(&router_id) {
            Some(router_info) => {
                // even though `TransportService` prevents dialing the same router from the same
                // subsystem twice, the notion of a "pending router", i.e., it being dialed, is not
                // shared between the subsystems
                //
                // this means that it's possible for `TransportManager` to receive two dial requests
                // for the same router, with the second request arriving while the first one is
                // still pending
                //
                // ensure that the router is not being dialed before dialing them
                if self.pending_connections.contains_key(&router_id) {
                    tracing::debug!(
                        target: LOG_TARGET,
                        %router_id,
                        "router is already being dialed",
                    );
                    return;
                }

                let Some(transport) = router_info.select_transport(&self.supported_transports)
                else {
                    tracing::debug!(
                        target: LOG_TARGET,
                        %router_id,
                        caps = %router_info.capabilities,
                        "cannot dial router, no reachable transport",
                    );

                    self.router_ctx.metrics_handle().counter(NUM_DIAL_FAILURES).increment(1);
                    return self.report_dial_failure(router_id);
                };

                tracing::trace!(
                    target: LOG_TARGET,
                    %router_id,
                    %transport,
                    "start dialing router",
                );

                // if not ssu2, dial the router over ntcp2
                let RouterAddress::Ssu2 {
                    introducers,
                    socket_address,
                    ..
                } = transport
                else {
                    self.router_ctx.metrics_handle().counter(NUM_INITIATED).increment(1);
                    self.pending_connections.insert(router_id.clone(), clients);
                    self.transports[0].connect(router_info);
                    return;
                };

                // ssu2 is always the last transport
                //
                // if only ssu2 is active, index is 0
                // if both ntcp2 and ssu2 are active, index is 1
                let ssu2_index = self.transports.len() - 1;

                // if a socket address has been published, dial the router directly
                if socket_address.is_some() {
                    self.router_ctx.metrics_handle().counter(NUM_INITIATED).increment(1);
                    self.pending_connections.insert(router_id.clone(), clients);
                    self.transports[ssu2_index].connect(router_info);
                    return;
                }

                // if we're connected to one of the introducers, we can dial right away
                if let Some((introducer, relay_tag)) =
                    introducers.iter().find(|(router_id, _)| self.routers.contains_key(router_id))
                {
                    tracing::trace!(
                        target: LOG_TARGET,
                        %router_id,
                        %introducer,
                        ?relay_tag,
                        "already connected to an introducer, starting relayed connection",
                    );

                    self.router_ctx.metrics_handle().counter(NUM_INITIATED).increment(1);
                    self.pending_connections.insert(router_id.clone(), clients);
                    self.transports[ssu2_index].connect(router_info);
                    return;
                }

                // start dialing introducers in parallel
                let mut pending_connections = HashSet::new();
                let mut pending_queries = HashSet::new();

                for (introducer, relay_tag) in introducers {
                    // check if a connection to one of the introducers is already pending
                    //
                    // if so, store `router_id` into the pending context and when the dial
                    // resolves, either report dial failure (cannot connect to introducer) or
                    // start "dialing" the target router by contacting the introducer
                    if self.pending_connections.contains_key(introducer) {
                        tracing::trace!(
                            target: LOG_TARGET,
                            %router_id,
                            %introducer,
                            ?relay_tag,
                            "connection to introducer pending",
                        );

                        // entry must exist since it was just checked above
                        self.pending_connections
                            .get_mut(introducer)
                            .expect("to exist")
                            .push(router_id.clone());
                        pending_connections.insert(introducer.clone());

                        continue;
                    }

                    // check if the introducer's router info is available and if so, start dialing
                    {
                        let reader = self.router_ctx.profile_storage().reader();
                        match reader.router_info(introducer) {
                            None => {
                                // introducer router info not found, start nedb query
                            }
                            Some(introducer_router_info) => {
                                if !introducer_router_info.is_reachable_ssu2() {
                                    tracing::warn!(
                                        target: LOG_TARGET,
                                        %router_id,
                                        %introducer,
                                        ?relay_tag,
                                        "introducer is not reachable over ssu2",
                                    );
                                    continue;
                                }

                                tracing::trace!(
                                    target: LOG_TARGET,
                                    %router_id,
                                    %introducer,
                                    ?relay_tag,
                                    "dialing introducer",
                                );

                                // dial introducer directly
                                //
                                // at this point it has already been verified that there is neither
                                // active or pending connection to the
                                // introducer and that they're reachable over
                                // ssu2
                                pending_connections.insert(introducer.clone());
                                self.router_ctx
                                    .metrics_handle()
                                    .counter(NUM_INITIATED)
                                    .increment(1);
                                self.pending_connections
                                    .insert(introducer.clone(), vec![router_id.clone()]);
                                self.transports[ssu2_index].connect(introducer_router_info.clone());

                                continue;
                            }
                        }
                    }

                    // the introducer is not connected, doens't have a pending connection and
                    // doesn't exist in router storage
                    //
                    // start a netdb lookup for their router info
                    if self.pending_queries.contains(introducer) {
                        tracing::trace!(
                            target: LOG_TARGET,
                            %router_id,
                            %introducer,
                            ?relay_tag,
                            "router info query pending for introducer",
                        );

                        pending_queries.insert(introducer.clone());
                    } else {
                        tracing::trace!(
                            target: LOG_TARGET,
                            %router_id,
                            %introducer,
                            ?relay_tag,
                            "start router info query for introducer",
                        );

                        pending_queries.insert(introducer.clone());
                        self.send_router_info_query(introducer.clone(), vec![router_id.clone()]);
                    }
                }

                self.pending_connections.insert(router_id.clone(), clients);
                self.pending_introducers.insert(
                    router_id.clone(),
                    IntroducerConnection {
                        router_info,
                        pending_connections,
                        pending_queries,
                    },
                );
            }
            None => {
                tracing::debug!(
                    target: LOG_TARGET,
                    %router_id,
                    "router info not found, send router info query to netdb",
                );

                if self.pending_queries.contains(&router_id) {
                    tracing::debug!(
                        target: LOG_TARGET,
                        %router_id,
                        "router info is already being queried",
                    );
                    return;
                }

                self.send_router_info_query(router_id, Vec::new());
            }
        }
    }

    /// Handle firewall status update received from SSU2.
    fn on_firewall_status(&mut self, status: FirewallStatus) {
        tracing::debug!(
            target: LOG_TARGET,
            ?status,
            "firewall status update",
        );

        self.firewall_status = status;
    }

    /// Handle discovered external address.
    ///
    /// This address is discovered through SSU2 and our router addresses should be converted
    /// to reachable only if peer testing has confirmed us to be reachable.
    fn on_external_address(&mut self, address: SocketAddr) {
        match address.ip() {
            IpAddr::V4(address)
                if Some(address) != self.external_address
                    && self.firewall_status == FirewallStatus::Ok =>
                self.add_external_address(address),
            _ => tracing::trace!(
                target: LOG_TARGET,
                ?address,
                "ignoring discovered external address",
            ),
        }
    }

    /// Handle new introducer.
    fn on_introducer_added(&mut self, router_id: RouterId, relay_tag: u32, expires: Duration) {
        tracing::debug!(
            target: LOG_TARGET,
            %router_id,
            ?relay_tag,
            ?expires,
            "add new introducer",
        );

        self.introducers.push((router_id, relay_tag, expires));
    }

    /// Handle removed introducer.
    fn on_introducer_removed(&mut self, router_id: &RouterId) {
        tracing::debug!(
            target: LOG_TARGET,
            %router_id,
            "remove introducer",
        );

        self.introducers.retain(|(r, _, _)| r != router_id);
    }

    /// Attempt to publish our router info.
    fn publish_router_info(&mut self) {
        // current router capabilties
        let mut caps = Str::from("L");

        // reset publish time and serialize our new router info
        self.local_router_info.published = Date::new(R::time_since_epoch().as_millis() as u64);

        // publish `G`, i.e., rejecting all tunnels, if transit tunnels have been disabled
        if self.transit_tunnels_disabled {
            tracing::info!(
                target: LOG_TARGET,
                "transit tunnels disabled, publishing G",
            );

            caps += "G";
        }

        // update ssu2's address if we have any active introducers
        if let Some(RouterAddress::Ssu2 { options, .. }) = self.local_router_info.ssu2_ipv4_mut() {
            // remove old introducers
            for i in 0..3 {
                options.remove(&Str::from(format!("iexp{i}")));
                options.remove(&Str::from(format!("ih{i}")));
                options.remove(&Str::from(format!("itag{i}")));
            }

            // add introducers to our router info, skipping any expired introducers
            let now = R::time_since_epoch();

            for (i, (router_id, relay_tag, expires)) in self.introducers.iter().enumerate() {
                if expires > &(now + INTRODUCER_EXPIRATION) {
                    tracing::debug!(
                        target: LOG_TARGET,
                        %router_id,
                        ?relay_tag,
                        expired = ?(*expires - now - INTRODUCER_EXPIRATION),
                        "skip expired introducer"
                    );
                    continue;
                }

                options.insert(
                    Str::from(format!("iexp{i}")),
                    Str::from(expires.as_secs().to_string()),
                );
                options.insert(
                    Str::from(format!("ih{i}")),
                    Str::from(base64_encode(router_id.to_vec())),
                );
                options.insert(
                    Str::from(format!("itag{i}")),
                    Str::from(relay_tag.to_string()),
                );
            }
        }

        match self.firewall_status {
            FirewallStatus::Unknown => tracing::info!(
                target: LOG_TARGET,
                "firewall status unknown, not publishing reachability caps",
            ),
            FirewallStatus::Ok => {
                tracing::info!(
                    target: LOG_TARGET,
                    "router is not firewalled, publishing R",
                );
                caps += "R";
            }
            status => {
                tracing::info!(
                    target: LOG_TARGET,
                    ?status,
                    "router is firewalled, publishing U",
                );
                caps += "U";
            }
        }

        // use user-provided caps if they exist, otherwise use the derived acps
        self.local_router_info.options.insert(
            Str::from("caps"),
            self.caps.as_ref().map_or(caps, |caps| caps.clone()),
        );

        let serialized =
            Bytes::from(self.local_router_info.serialize(self.router_ctx.signing_key()));

        // reset router info in router context so all subsystems are using the latest version of
        // it and publish it to netdb
        self.router_ctx.set_router_info(serialized.clone());
        self.netdb_handle
            .publish_router_info(self.router_ctx.router_id().clone(), serialized);
    }
}

impl<R: Runtime> Future for TransportManager<R> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = Pin::into_inner(self);
        let len = this.transports.len();
        let start_index = this.poll_index;

        loop {
            let index = this.poll_index % len;
            this.poll_index += 1;

            loop {
                match this.transports[index].poll_next_unpin(cx) {
                    Poll::Pending => break,
                    Poll::Ready(None) => return Poll::Ready(()),
                    Poll::Ready(Some(TransportEvent::ConnectionEstablished {
                        address,
                        direction,
                        router_id,
                    })) => match direction {
                        Direction::Inbound if this.pending_connections.contains_key(&router_id) => {
                            tracing::debug!(
                                target: LOG_TARGET,
                                %router_id,
                                "outbound connection pending, rejecting inbound connection",
                            );

                            this.router_ctx.metrics_handle().counter(NUM_REJECTED).increment(1);
                            this.transports[index].reject(&router_id);
                        }
                        Direction::Outbound
                            if !this.pending_connections.contains_key(&router_id) =>
                        {
                            tracing::debug!(
                                target: LOG_TARGET,
                                %router_id,
                                "pending connection doesn't exist for router, rejecting connection",
                            );

                            this.router_ctx.metrics_handle().counter(NUM_REJECTED).increment(1);
                            this.transports[index].reject(&router_id);
                        }
                        direction => {
                            if this.routers.contains_key(&router_id) {
                                tracing::debug!(
                                    target: LOG_TARGET,
                                    %router_id,
                                    "router already connected, rejecting",
                                );

                                this.router_ctx.metrics_handle().counter(NUM_REJECTED).increment(1);
                                this.transports[index].reject(&router_id);
                                continue;
                            }

                            tracing::trace!(
                                target: LOG_TARGET,
                                %router_id,
                                ?direction,
                                "connection established",
                            );

                            this.transports[index].accept(&router_id);
                            this.routers.insert(router_id.clone(), address.is_ipv4());

                            // if this was a successful connection to an introducer with
                            // active client(s), start
                            // dialing each of the clients
                            //
                            // the routers can be dialed directly (without calling
                            // `on_dial_request()`) as they've already been validated and
                            // were only awaiting for an
                            // introducer connection
                            //
                            // `IntroducerConnection` may not exist if some other introducer
                            // connected first and thus removed the connection
                            if let Some(routers) = this.pending_connections.remove(&router_id) {
                                let ssu2_index = this.transports.len() - 1;

                                for client_router_id in routers {
                                    let Some(IntroducerConnection { router_info, .. }) =
                                        this.pending_introducers.remove(&client_router_id)
                                    else {
                                        tracing::trace!(
                                            target: LOG_TARGET,
                                            router_id = %client_router_id,
                                            introducer = %router_id,
                                            "context for client doesn't exist"
                                        );
                                        continue;
                                    };

                                    tracing::trace!(
                                        target: LOG_TARGET,
                                        router_id = %client_router_id,
                                        introducer = %router_id,
                                        "introducer connected, dialing router",
                                    );

                                    this.router_ctx
                                        .metrics_handle()
                                        .counter(NUM_INITIATED)
                                        .increment(1);
                                    this.transports[ssu2_index].connect(router_info);
                                }
                            }

                            this.router_ctx.profile_storage().dial_succeeded(&router_id);
                            this.router_ctx.metrics_handle().gauge(NUM_CONNECTIONS).increment(1);
                            this.router_ctx.metrics_handle().counter(NUM_ACCEPTED).increment(1);
                            this.router_ctx
                                .metrics_handle()
                                .gauge(if address.is_ipv4() {
                                    NUM_IPV4
                                } else {
                                    NUM_IPV6
                                })
                                .increment(1);
                        }
                    },
                    Poll::Ready(Some(TransportEvent::ConnectionClosed { router_id, reason })) => {
                        match reason {
                            TerminationReason::Banned => tracing::warn!(
                                target: LOG_TARGET,
                                %router_id,
                                ?reason,
                                "remote router banned us",
                            ),
                            TerminationReason::IdleTimeout => tracing::trace!(
                                target: LOG_TARGET,
                                %router_id,
                                ?reason,
                                "connection closed",
                            ),
                            reason => tracing::debug!(
                                target: LOG_TARGET,
                                %router_id,
                                ?reason,
                                "connection closed",
                            ),
                        }

                        match this.routers.remove(&router_id) {
                            None => {
                                tracing::warn!(
                                    target: LOG_TARGET,
                                    %router_id,
                                    "connection closed to unknown router",
                                );
                                debug_assert!(false);
                            }
                            Some(true) =>
                                this.router_ctx.metrics_handle().gauge(NUM_IPV4).decrement(1),
                            Some(false) =>
                                this.router_ctx.metrics_handle().gauge(NUM_IPV4).decrement(1),
                        }
                        this.router_ctx.metrics_handle().gauge(NUM_CONNECTIONS).decrement(1);
                    }
                    Poll::Ready(Some(TransportEvent::ConnectionFailure { router_id })) => {
                        tracing::trace!(
                            target: LOG_TARGET,
                            %router_id,
                            "failed to dial router",
                        );

                        this.router_ctx.metrics_handle().counter(NUM_DIAL_FAILURES).increment(1);
                        this.router_ctx.profile_storage().dial_failed(&router_id);

                        // if the router for which the dial failed was also a pending introducer,
                        // remove the introducer from each client's `IntroducerConnection` and
                        // if this was the last introducer (all others failed), send dial failure
                        // for the client router
                        if let Some(routers) = this.pending_connections.remove(&router_id) {
                            for client_router_id in routers {
                                let Some(IntroducerConnection {
                                    pending_connections,
                                    pending_queries,
                                    ..
                                }) = this.pending_introducers.get_mut(&client_router_id)
                                else {
                                    tracing::trace!(
                                        target: LOG_TARGET,
                                        router_id = %client_router_id,
                                        introducer = %router_id,
                                        "context for client doesn't exist"
                                    );
                                    continue;
                                };

                                tracing::trace!(
                                    target: LOG_TARGET,
                                    router_id = %client_router_id,
                                    introducer = %router_id,
                                    "failed to dial introducer",
                                );
                                pending_connections.remove(&router_id);

                                if pending_connections.is_empty() && pending_queries.is_empty() {
                                    tracing::debug!(
                                        target: LOG_TARGET,
                                        router_id = %client_router_id,
                                        introducer = %router_id,
                                        "failed to dial all introducers, unable to dial router",
                                    );

                                    this.router_ctx
                                        .metrics_handle()
                                        .counter(NUM_DIAL_FAILURES)
                                        .increment(1);
                                    this.router_ctx
                                        .metrics_handle()
                                        .counter(NUM_INTRODUCER_DIAL_FAILURES)
                                        .increment(1);
                                    this.pending_connections.remove(&client_router_id);
                                    this.pending_introducers.remove(&client_router_id);
                                    this.report_dial_failure(client_router_id);
                                }
                            }
                        }
                    }
                    Poll::Ready(Some(TransportEvent::FirewallStatus { status })) =>
                        this.on_firewall_status(status),
                    Poll::Ready(Some(TransportEvent::ExternalAddress { address })) =>
                        this.on_external_address(address),
                    Poll::Ready(Some(TransportEvent::IntroducerAdded {
                        relay_tag,
                        router_id,
                        expires,
                    })) => this.on_introducer_added(router_id, relay_tag, expires),
                    Poll::Ready(Some(TransportEvent::IntroducerRemoved { router_id })) =>
                        this.on_introducer_removed(&router_id),
                }
            }

            if this.poll_index == start_index + len {
                break;
            }
        }

        loop {
            match this.pending_query_futures.poll_next_unpin(cx) {
                Poll::Pending => break,
                Poll::Ready(None) => return Poll::Ready(()),
                Poll::Ready(Some((router_id, Ok(())))) => {
                    tracing::debug!(
                        target: LOG_TARGET,
                        %router_id,
                        "router info query succeeded, dial pending router",
                    );
                    this.pending_queries.remove(&router_id);

                    let clients = match this.pending_connections.remove(&router_id) {
                        None => {
                            tracing::error!(
                                target: LOG_TARGET,
                                %router_id,
                                "router does not have dial context",
                            );
                            debug_assert!(false);
                            Vec::new()
                        }
                        Some(clients) => clients,
                    };

                    // remove pending query from all client connections
                    for client_router_id in &clients {
                        if let Some(IntroducerConnection {
                            pending_queries, ..
                        }) = this.pending_introducers.get_mut(client_router_id)
                        {
                            pending_queries.remove(&router_id);
                        };
                    }

                    this.router_ctx
                        .metrics_handle()
                        .counter(NUM_NETDB_QUERY_SUCCESSES)
                        .increment(1);

                    this.on_dial_router(router_id, clients);
                }
                Poll::Ready(Some((router_id, Err(error)))) => {
                    tracing::debug!(
                        target: LOG_TARGET,
                        %router_id,
                        ?error,
                        "router info query failed",
                    );

                    this.router_ctx.metrics_handle().gauge(NUM_DIAL_FAILURES).increment(1);
                    this.router_ctx.metrics_handle().counter(NUM_NETDB_QUERY_FAILURES).increment(1);
                    this.pending_queries.remove(&router_id);

                    // remove the pending query for the introducer from all client connections
                    //
                    // if this was the last pending query, report dial failures for the client
                    if let Some(routers) = this.pending_connections.remove(&router_id) {
                        for client_router_id in routers {
                            let Some(IntroducerConnection {
                                pending_connections,
                                pending_queries,
                                ..
                            }) = this.pending_introducers.get_mut(&client_router_id)
                            else {
                                tracing::trace!(
                                    target: LOG_TARGET,
                                    router_id = %client_router_id,
                                    introducer = %router_id,
                                    "context for client doesn't exist"
                                );
                                continue;
                            };

                            tracing::trace!(
                                target: LOG_TARGET,
                                router_id = %client_router_id,
                                introducer = %router_id,
                                "router info query failed for introducer",
                            );
                            pending_queries.remove(&router_id);

                            if pending_connections.is_empty() && pending_queries.is_empty() {
                                tracing::debug!(
                                    target: LOG_TARGET,
                                    router_id = %client_router_id,
                                    introducer = %router_id,
                                    "failed to dial all introducers, unable to dial router",
                                );

                                this.router_ctx
                                    .metrics_handle()
                                    .counter(NUM_DIAL_FAILURES)
                                    .increment(1);
                                this.router_ctx
                                    .metrics_handle()
                                    .counter(NUM_INTRODUCER_DIAL_FAILURES)
                                    .increment(1);
                                this.pending_connections.remove(&client_router_id);
                                this.pending_introducers.remove(&client_router_id);
                                this.report_dial_failure(client_router_id);
                            }
                        }
                    }

                    this.report_dial_failure(router_id);
                }
            }
        }

        loop {
            match this.dial_rx.poll_recv(cx) {
                Poll::Pending => break,
                Poll::Ready(None) => return Poll::Ready(()),
                Poll::Ready(Some(router_id)) => this.on_dial_router(router_id, Vec::new()),
            }
        }

        if this.router_info_republish_timer.poll_unpin(cx).is_ready() {
            // reset timer and register it into the executor
            this.router_info_republish_timer = R::timer(ROUTER_INFO_REPUBLISH_INTERVAL);
            let _ = this.router_info_republish_timer.poll_unpin(cx);

            this.publish_router_info();
        }

        if this.event_handle.poll_unpin(cx).is_ready() {
            this.event_handle.num_connected_routers(this.routers.len());
        }

        Poll::Pending
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        events::EventManager,
        i2np::{Message, MessageType, I2NP_MESSAGE_EXPIRATION},
        netdb::{NetDbAction, NetDbActionRecycle},
        primitives::{Capabilities, RouterInfoBuilder, Str},
        profile::ProfileStorage,
        router::context::builder::RouterContextBuilder,
        runtime::mock::MockRuntime,
        subsystem::OutboundMessage,
        timeout,
    };
    use std::collections::VecDeque;
    use thingbuf::mpsc::channel;
    use tokio::sync::mpsc;

    fn make_transport_manager(
        ntcp2: Option<Ntcp2Config>,
        ssu2: Option<Ssu2Config>,
    ) -> (
        TransportManagerBuilder<MockRuntime>,
        Sender<RouterId>,
        Receiver<SubsystemEvent>,
        Receiver<NetDbAction, NetDbActionRecycle>,
    ) {
        let (router_info, static_key, signing_key) = {
            let mut builder = RouterInfoBuilder::default();

            if let Some(ntcp2) = ntcp2 {
                builder = builder.with_ntcp2(ntcp2);
            }

            if let Some(ssu2) = ssu2 {
                builder = builder.with_ssu2(ssu2);
            }

            builder.build()
        };
        let serialized = Bytes::from(router_info.serialize(&signing_key));
        let (dial_tx, dial_rx) = channel(100);
        let (transport_tx, transport_rx) = channel(100);
        let (handle, netdb_rx) = NetDbHandle::create();
        let (_event_mgr, _event_subscriber, event_handle) =
            EventManager::new(None, MockRuntime::register_metrics(vec![], None));
        let ctx = RouterContext::new(
            MockRuntime::register_metrics(vec![], None),
            ProfileStorage::<MockRuntime>::new(&[], &[]),
            router_info.identity.id(),
            serialized.clone(),
            static_key,
            signing_key,
            2u8,
            event_handle.clone(),
        );

        let mut builder = TransportManagerBuilder::<MockRuntime>::new(
            ctx,
            router_info,
            true,
            dial_rx,
            transport_tx,
        );
        builder.register_netdb_handle(handle);

        (builder, dial_tx, transport_rx, netdb_rx)
    }

    #[tokio::test]
    async fn external_address_discovered_ntcp2() {
        let context = Ntcp2Transport::<MockRuntime>::initialize(Some(Ntcp2Config {
            port: 0,
            ipv4_host: Some("192.168.0.1".parse().unwrap()),
            ipv6_host: None,
            publish: true,
            key: [0u8; 32],
            iv: [0u8; 16],
            ipv4: true,
            ipv6: false,
        }))
        .await
        .unwrap()
        .0
        .unwrap();
        let (mut builder, _dial_tx, _transport_rx, _netdb_rx) =
            make_transport_manager(Some(context.config()), None);
        builder.register_ntcp2(context);
        let mut manager = builder.build();

        // ensure ntcp2 is published
        assert!(manager.ntcp2_config.is_some());
        assert!(
            manager.local_router_info.addresses().any(|address| match address {
                RouterAddress::Ntcp2 { options, .. } => {
                    options.get(&Str::from("host")) == Some(&Str::from("192.168.0.1"))
                        && options.get(&Str::from("s")).is_some()
                }
                _ => false,
            })
        );

        manager.add_external_address("192.168.0.1".parse().unwrap());

        // verify that the address is still published and that host is the same
        assert!(
            manager.local_router_info.addresses().any(|address| match address {
                RouterAddress::Ntcp2 { options, .. } => {
                    options.get(&Str::from("host")) == Some(&Str::from("192.168.0.1"))
                        && options.get(&Str::from("s")).is_some()
                }
                _ => false,
            })
        );
    }

    #[tokio::test]
    async fn external_address_discovered_ntcp2_unpublished() {
        let context = Ntcp2Transport::<MockRuntime>::initialize(Some(Ntcp2Config {
            port: 0,
            ipv4_host: None,
            ipv6_host: None,
            publish: false,
            key: [0u8; 32],
            iv: [0u8; 16],
            ipv4: true,
            ipv6: false,
        }))
        .await
        .unwrap()
        .0
        .unwrap();

        let (mut builder, _dial_tx, _transport_rx, _netdb_rx) =
            make_transport_manager(Some(context.config()), None);
        builder.register_ntcp2(context);
        let mut manager = builder.build();

        // ensure ntcp2 is unpublished
        assert!(manager.ntcp2_config.is_some());
        assert!(
            manager.local_router_info.addresses().any(|address| match address {
                RouterAddress::Ntcp2 { options, .. } => {
                    options.get(&Str::from("host")).is_none()
                        && options.get(&Str::from("i")).is_none()
                }
                _ => false,
            })
        );

        manager.add_external_address("192.168.0.1".parse().unwrap());

        // verify that the address is still unpublished
        assert!(
            manager.local_router_info.addresses().any(|address| match address {
                RouterAddress::Ntcp2 { options, .. } => {
                    options.get(&Str::from("host")).is_none()
                        && options.get(&Str::from("i")).is_none()
                }
                _ => false,
            })
        );
    }

    #[tokio::test]
    async fn external_address_discovered_ssu2() {
        let ssu2 = Ssu2Config {
            port: 0,
            host: Some("192.168.0.1".parse().unwrap()),
            publish: true,
            static_key: [0u8; 32],
            intro_key: [1u8; 32],
        };
        let context =
            Ssu2Transport::<MockRuntime>::initialize(Some(ssu2)).await.unwrap().0.unwrap();
        let (mut builder, _dial_tx, _transport_rx, _netdb_rx) =
            make_transport_manager(None, Some(context.config()));
        builder.register_ssu2(context);
        let mut manager = builder.build();

        // ensure ssu2 is published
        assert!(manager.ssu2_config.is_some());
        assert!(
            manager.local_router_info.addresses().any(|address| match address {
                RouterAddress::Ssu2 { options, .. } => {
                    options.get(&Str::from("host")) == Some(&Str::from("192.168.0.1"))
                }
                _ => false,
            })
        );

        manager.add_external_address("192.168.0.1".parse().unwrap());

        // verify that the address is still published and that host is the same
        assert!(
            manager.local_router_info.addresses().any(|address| match address {
                RouterAddress::Ssu2 { options, .. } => {
                    options.get(&Str::from("host")) == Some(&Str::from("192.168.0.1"))
                }
                _ => false,
            })
        );
    }

    #[tokio::test]
    async fn external_address_discovered_ssu2_unpublished() {
        let context = Ssu2Transport::<MockRuntime>::initialize(Some(Ssu2Config {
            port: 0,
            host: None,
            publish: false,
            static_key: [0u8; 32],
            intro_key: [1u8; 32],
        }))
        .await
        .unwrap()
        .0
        .unwrap();

        let (mut builder, _dial_tx, _transport_rx, _netdb_rx) =
            make_transport_manager(None, Some(context.config()));
        builder.register_ssu2(context);
        let mut manager = builder.build();

        // ensure ssu2 is unpublished
        assert!(manager.ssu2_config.is_some());
        assert!(
            manager.local_router_info.addresses().any(|address| match address {
                RouterAddress::Ssu2 { options, .. } => {
                    options.get(&Str::from("host")).is_none()
                }
                _ => false,
            })
        );

        manager.add_external_address("192.168.0.1".parse().unwrap());

        // verify that the address is still unpublished
        assert!(
            manager.local_router_info.addresses().any(|address| match address {
                RouterAddress::Ssu2 { options, .. } => {
                    options.get(&Str::from("host")).is_none()
                }
                _ => false,
            })
        );
    }

    #[tokio::test]
    async fn new_external_address_discovered() {
        let ssu2_context = Ssu2Transport::<MockRuntime>::initialize(Some(Ssu2Config {
            port: 0,
            host: None,
            publish: true,
            static_key: [0u8; 32],
            intro_key: [1u8; 32],
        }))
        .await
        .unwrap()
        .0
        .unwrap();
        let ntcp2_context = Ntcp2Transport::<MockRuntime>::initialize(Some(Ntcp2Config {
            port: 0,
            ipv4_host: None,
            ipv6_host: None,
            publish: true,
            key: [0u8; 32],
            iv: [0u8; 16],
            ipv4: true,
            ipv6: false,
        }))
        .await
        .unwrap()
        .0
        .unwrap();

        let (mut builder, _dial_tx, _transport_rx, _netdb_rx) =
            make_transport_manager(Some(ntcp2_context.config()), Some(ssu2_context.config()));
        builder.register_ssu2(ssu2_context);
        builder.register_ntcp2(ntcp2_context);
        let mut manager = builder.build();

        // ensure ssu2 and ntcp2 is unpublished since no host was provided
        assert!(manager.ssu2_config.is_some());
        assert!(manager.ntcp2_config.is_some());
        assert!(
            manager.local_router_info.addresses().any(|address| match address {
                RouterAddress::Ssu2 { options, .. } => {
                    options.get(&Str::from("host")).is_none()
                }
                _ => false,
            })
        );
        assert!(
            manager.local_router_info.addresses().any(|address| match address {
                RouterAddress::Ntcp2 { options, .. } => {
                    options.get(&Str::from("host")).is_none()
                        && options.get(&Str::from("i")).is_none()
                }
                _ => false,
            })
        );

        manager.add_external_address("192.168.0.1".parse().unwrap());

        // verify the addresses have been published
        assert!(
            manager.local_router_info.addresses().any(|address| match address {
                RouterAddress::Ssu2 { options, .. } => {
                    options.get(&Str::from("host")) == Some(&Str::from("192.168.0.1"))
                }
                _ => false,
            })
        );
        assert!(
            manager.local_router_info.addresses().any(|address| match address {
                RouterAddress::Ntcp2 { options, .. } => {
                    options.get(&Str::from("host")) == Some(&Str::from("192.168.0.1"))
                        && options.get(&Str::from("i")).is_some()
                }
                _ => false,
            })
        );
    }

    #[tokio::test]
    async fn discovered_address_doesnt_match_published_address_ntcp2() {
        let context = Ntcp2Transport::<MockRuntime>::initialize(Some(Ntcp2Config {
            port: 0,
            ipv4_host: Some("192.168.0.1".parse().unwrap()),
            ipv6_host: None,
            publish: true,
            key: [0u8; 32],
            iv: [0u8; 16],
            ipv4: true,
            ipv6: false,
        }))
        .await
        .unwrap()
        .0
        .unwrap();
        let (mut builder, _dial_tx, _transport_rx, _netdb_rx) =
            make_transport_manager(Some(context.config()), None);
        builder.register_ntcp2(context);
        let mut manager = builder.build();

        // ensure ntcp2 is published
        assert!(manager.ntcp2_config.is_some());
        assert!(
            manager.local_router_info.addresses().any(|address| match address {
                RouterAddress::Ntcp2 { options, .. } => {
                    options.get(&Str::from("host")) == Some(&Str::from("192.168.0.1"))
                        && options.get(&Str::from("i")).is_some()
                }
                _ => false,
            })
        );

        manager.add_external_address("192.168.1.1".parse().unwrap());

        // verify that the address is still published and that host is the same
        assert!(
            manager.local_router_info.addresses().any(|address| match address {
                RouterAddress::Ntcp2 { options, .. } => {
                    options.get(&Str::from("host")) == Some(&Str::from("192.168.0.1"))
                        && options.get(&Str::from("i")).is_some()
                }
                _ => false,
            })
        );
    }

    #[tokio::test]
    async fn discovered_address_doesnt_match_published_address_ssu2() {
        let context = Ssu2Transport::<MockRuntime>::initialize(Some(Ssu2Config {
            port: 0,
            host: Some("192.168.1.1".parse().unwrap()),
            publish: true,
            static_key: [0u8; 32],
            intro_key: [1u8; 32],
        }))
        .await
        .unwrap()
        .0
        .unwrap();

        let (mut builder, _dial_tx, _transport_rx, _netdb_rx) =
            make_transport_manager(None, Some(context.config()));
        builder.register_ssu2(context);
        let mut manager = builder.build();

        // ensure ssu2 is unpublished
        assert!(manager.ssu2_config.is_some());
        assert!(
            manager.local_router_info.addresses().any(|address| match address {
                RouterAddress::Ssu2 { options, .. } => {
                    options.get(&Str::from("host")) == Some(&Str::from("192.168.1.1"))
                }
                _ => false,
            })
        );

        manager.add_external_address("192.168.0.1".parse().unwrap());

        // verify that the address is still unpublished
        assert!(
            manager.local_router_info.addresses().any(|address| match address {
                RouterAddress::Ssu2 { options, .. } => {
                    options.get(&Str::from("host")) == Some(&Str::from("192.168.1.1"))
                }
                _ => false,
            })
        );
    }

    #[tokio::test]
    async fn inbound_connection_rejected_connection_already_exists() {
        let (router_info, static_key, signing_key) = RouterInfoBuilder::default().build();
        let serialized = Bytes::from(router_info.serialize(&signing_key));
        let (handle, _) = NetDbHandle::create();
        let (_event_mgr, _event_subscriber, event_handle) =
            EventManager::new(None, MockRuntime::register_metrics(vec![], None));
        let ctx = RouterContext::new(
            MockRuntime::register_metrics(vec![], None),
            ProfileStorage::<MockRuntime>::new(&[], &[]),
            router_info.identity.id(),
            serialized.clone(),
            static_key,
            signing_key,
            2u8,
            event_handle.clone(),
        );

        let (_dial_tx, dial_rx) = channel(100);
        let (transport_tx, _transport_rx) = channel(100);
        let mut builder = TransportManagerBuilder::<MockRuntime>::new(
            ctx,
            router_info,
            true,
            dial_rx,
            transport_tx,
        );
        builder.register_netdb_handle(handle);
        let mut manager = builder.build();

        #[derive(Clone, Default)]
        enum Command {
            Accept(RouterId),
            Reject(RouterId),
            #[default]
            Dummy,
        }

        pub struct MockTransport {
            events: VecDeque<TransportEvent>,
            tx: Sender<Command>,
        }

        impl Transport for MockTransport {
            fn connect(&mut self, _: RouterInfo) {
                todo!();
            }

            fn accept(&mut self, router_id: &RouterId) {
                self.tx.try_send(Command::Accept(router_id.clone())).unwrap();
            }

            fn reject(&mut self, router_id: &RouterId) {
                self.tx.try_send(Command::Reject(router_id.clone())).unwrap();
            }
        }

        impl Stream for MockTransport {
            type Item = TransportEvent;

            fn poll_next(
                mut self: Pin<&mut Self>,
                cx: &mut Context<'_>,
            ) -> Poll<Option<Self::Item>> {
                match self.events.pop_front() {
                    Some(event) => {
                        cx.waker().wake_by_ref();
                        return Poll::Ready(Some(event));
                    }
                    None => {
                        cx.waker().wake_by_ref();
                        Poll::Pending
                    }
                }
            }
        }

        let (tx, rx) = channel(64);
        let router_id1 = RouterId::random();

        manager.transports.push(Box::new(MockTransport {
            events: VecDeque::from_iter([
                TransportEvent::ConnectionEstablished {
                    address: "127.0.0.1:8888".parse().unwrap(),
                    router_id: router_id1.clone(),
                    direction: Direction::Inbound,
                },
                TransportEvent::ConnectionEstablished {
                    address: "127.0.0.1:8888".parse().unwrap(),
                    router_id: router_id1.clone(),
                    direction: Direction::Inbound,
                },
            ]),
            tx,
        }));
        tokio::spawn(manager);

        match tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .expect("no timeout")
            .expect("to succeed")
        {
            Command::Accept(router_id) => {
                assert_eq!(router_id, router_id1);
            }
            _ => panic!("invalid command"),
        }

        match tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .expect("no timeout")
            .expect("to succeed")
        {
            Command::Reject(router_id) => {
                assert_eq!(router_id, router_id1);
            }
            _ => panic!("invalid command"),
        }
    }

    #[tokio::test]
    async fn inbound_connection_rejected_outbound_pending() {
        let (router_info, static_key, signing_key) = RouterInfoBuilder::default().build();
        let serialized = Bytes::from(router_info.serialize(&signing_key));
        let storage = ProfileStorage::<MockRuntime>::new(&[], &[]);
        let (_event_mgr, _event_subscriber, event_handle) =
            EventManager::new(None, MockRuntime::register_metrics(vec![], None));
        let (handle, _) = NetDbHandle::create();
        let ctx = RouterContext::new(
            MockRuntime::register_metrics(vec![], None),
            storage.clone(),
            router_info.identity.id(),
            serialized.clone(),
            static_key,
            signing_key,
            2u8,
            event_handle.clone(),
        );

        let router = RouterInfoBuilder::default().build().0;
        let router_id = router.identity.id();
        storage.add_router(router);

        let (dial_tx, dial_rx) = channel(100);
        let (transport_tx, _transport_rx) = channel(100);
        let mut builder = TransportManagerBuilder::<MockRuntime>::new(
            ctx,
            router_info,
            true,
            dial_rx,
            transport_tx,
        );
        builder.register_netdb_handle(handle);
        let mut manager = builder.build();

        #[derive(Clone, Default)]
        enum Command {
            Connect(RouterInfo),
            Accept(RouterId),
            Reject(RouterId),
            #[default]
            Dummy,
        }

        pub struct MockTransport {
            tx: mpsc::Sender<Command>,
            event_rx: mpsc::Receiver<TransportEvent>,
        }

        impl Transport for MockTransport {
            fn connect(&mut self, info: RouterInfo) {
                self.tx.try_send(Command::Connect(info)).unwrap();
            }

            fn accept(&mut self, router_id: &RouterId) {
                self.tx.try_send(Command::Accept(router_id.clone())).unwrap();
            }

            fn reject(&mut self, router_id: &RouterId) {
                self.tx.try_send(Command::Reject(router_id.clone())).unwrap();
            }
        }

        impl Stream for MockTransport {
            type Item = TransportEvent;

            fn poll_next(
                mut self: Pin<&mut Self>,
                cx: &mut Context<'_>,
            ) -> Poll<Option<Self::Item>> {
                self.event_rx.poll_recv(cx)
            }
        }

        let (tx, mut rx) = mpsc::channel(64);
        let (event_tx, event_rx) = mpsc::channel(64);

        manager.transports.push(Box::new(MockTransport { event_rx, tx }));
        manager.supported_transports.insert(TransportKind::Ntcp2V4);
        tokio::spawn(manager);

        dial_tx.send(router_id.clone()).await.unwrap();

        match tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .expect("no timeout")
            .expect("to succeed")
        {
            Command::Connect(router_info) => {
                assert_eq!(router_info.identity.id(), router_id);
            }
            _ => panic!("invalid command"),
        }

        event_tx
            .send(TransportEvent::ConnectionEstablished {
                address: "127.0.0.1:8888".parse().unwrap(),
                router_id: router_id.clone(),
                direction: Direction::Inbound,
            })
            .await
            .unwrap();

        match tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .expect("no timeout")
            .expect("to succeed")
        {
            Command::Reject(rid) => {
                assert_eq!(rid, router_id);
            }
            _ => panic!("invalid command"),
        }

        event_tx
            .send(TransportEvent::ConnectionFailure {
                router_id: router_id.clone(),
            })
            .await
            .unwrap();

        event_tx
            .send(TransportEvent::ConnectionEstablished {
                address: "127.0.0.1:8888".parse().unwrap(),
                router_id: router_id.clone(),
                direction: Direction::Inbound,
            })
            .await
            .unwrap();

        match tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .expect("no timeout")
            .expect("to succeed")
        {
            Command::Accept(rid) => {
                assert_eq!(rid, router_id);
            }
            _ => panic!("invalid command"),
        }
    }

    #[tokio::test(start_paused = true)]
    async fn inbound_connection_rejected_while_netdb_lookup_pending() {
        let (router_info, static_key, signing_key) = RouterInfoBuilder::default().build();
        let serialized = Bytes::from(router_info.serialize(&signing_key));
        let storage = ProfileStorage::<MockRuntime>::new(&[], &[]);
        let (_event_mgr, _event_subscriber, event_handle) =
            EventManager::new(None, MockRuntime::register_metrics(vec![], None));
        let (handle, netdb_rx) = NetDbHandle::create();
        let ctx = RouterContext::new(
            MockRuntime::register_metrics(vec![], None),
            storage.clone(),
            router_info.identity.id(),
            serialized.clone(),
            static_key,
            signing_key,
            2u8,
            event_handle.clone(),
        );
        let router_id = RouterId::random();

        let (dial_tx, dial_rx) = channel(100);
        let (transport_tx, _transport_rx) = channel(100);
        let mut builder = TransportManagerBuilder::<MockRuntime>::new(
            ctx,
            router_info,
            true,
            dial_rx,
            transport_tx,
        );
        builder.register_netdb_handle(handle);
        let mut manager = builder.build();

        #[derive(Clone, Default)]
        enum Command {
            Accept(RouterId),
            Reject(RouterId),
            #[default]
            Dummy,
        }

        pub struct MockTransport {
            tx: mpsc::Sender<Command>,
            event_rx: mpsc::Receiver<TransportEvent>,
        }

        impl Transport for MockTransport {
            fn connect(&mut self, _: RouterInfo) {
                todo!();
            }

            fn accept(&mut self, router_id: &RouterId) {
                self.tx.try_send(Command::Accept(router_id.clone())).unwrap();
            }

            fn reject(&mut self, router_id: &RouterId) {
                self.tx.try_send(Command::Reject(router_id.clone())).unwrap();
            }
        }

        impl Stream for MockTransport {
            type Item = TransportEvent;

            fn poll_next(
                mut self: Pin<&mut Self>,
                cx: &mut Context<'_>,
            ) -> Poll<Option<Self::Item>> {
                self.event_rx.poll_recv(cx)
            }
        }

        let (tx, mut rx) = mpsc::channel(64);
        let (event_tx, event_rx) = mpsc::channel(64);

        manager.transports.push(Box::new(MockTransport { event_rx, tx }));
        tokio::spawn(manager);

        dial_tx.send(router_id.clone()).await.unwrap();

        // since RI for `router_id` doesn't exist, it's looked up from netdb
        let query_result_tx = match tokio::time::timeout(Duration::from_secs(5), netdb_rx.recv())
            .await
            .expect("no timeout")
            .expect("to succeed")
        {
            NetDbAction::QueryRouterInfo { router_id: rid, tx } => {
                assert_eq!(rid, router_id);
                tx
            }
            _ => panic!("invalid command"),
        };

        // try to add new inbound connection from the node that's being queried
        event_tx
            .send(TransportEvent::ConnectionEstablished {
                address: "127.0.0.1:8888".parse().unwrap(),
                router_id: router_id.clone(),
                direction: Direction::Inbound,
            })
            .await
            .unwrap();

        match tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .expect("no timeout")
            .expect("to succeed")
        {
            Command::Reject(rid) => {
                assert_eq!(rid, router_id);
            }
            _ => panic!("invalid command"),
        }

        // RI not found
        query_result_tx.send(Err(QueryError::ValueNotFound)).unwrap();

        tokio::time::sleep(Duration::from_secs(5)).await;

        // add new inbound connection from the same node that was previously rejected
        event_tx
            .send(TransportEvent::ConnectionEstablished {
                address: "127.0.0.1:8888".parse().unwrap(),
                router_id: router_id.clone(),
                direction: Direction::Inbound,
            })
            .await
            .unwrap();

        match tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .expect("no timeout")
            .expect("to succeed")
        {
            Command::Accept(rid) => {
                assert_eq!(rid, router_id);
            }
            _ => panic!("invalid command"),
        }
    }

    #[tokio::test]
    async fn transit_tunnels_disabled() {
        let (router_info, static_key, signing_key) = RouterInfoBuilder::default().build();
        let serialized = Bytes::from(router_info.serialize(&signing_key));
        let storage = ProfileStorage::<MockRuntime>::new(&[], &[]);
        let (handle, _netdb_rx) = NetDbHandle::create();
        let (_event_mgr, _event_subscriber, event_handle) =
            EventManager::new(None, MockRuntime::register_metrics(vec![], None));
        let ctx = RouterContext::new(
            MockRuntime::register_metrics(vec![], None),
            storage.clone(),
            router_info.identity.id(),
            serialized.clone(),
            static_key,
            signing_key,
            2u8,
            event_handle.clone(),
        );
        let context = Ntcp2Transport::<MockRuntime>::initialize(Some(Ntcp2Config {
            port: 0,
            ipv4_host: Some("192.168.0.1".parse().unwrap()),
            ipv6_host: None,
            publish: true,
            key: [0u8; 32],
            iv: [0u8; 16],
            ipv4: true,
            ipv6: false,
        }))
        .await
        .unwrap()
        .0
        .unwrap();

        let (_dial_tx, dial_rx) = channel(100);
        let (transport_tx, _transport_rx) = channel(100);
        let mut builder = TransportManagerBuilder::<MockRuntime>::new(
            ctx,
            router_info,
            true,
            dial_rx,
            transport_tx,
        );
        builder.register_netdb_handle(handle);
        builder.register_ntcp2(context);
        builder.with_transit_tunnels_disabled(true);

        let mut manager = builder.build();
        manager.router_info_republish_timer = MockRuntime::timer(Duration::from_secs(1));

        assert!(tokio::time::timeout(Duration::from_secs(3), &mut manager).await.is_err());

        // verify that the local router is no longer considered usable due to the `G` flag
        assert!(!Capabilities::parse(
            &manager.local_router_info.options.get(&Str::from("caps")).unwrap()
        )
        .unwrap()
        .is_usable());
    }

    #[tokio::test(start_paused = true)]
    async fn router_info_query_fails() {
        let (router_info, static_key, signing_key) = RouterInfoBuilder::default().build();
        let serialized = Bytes::from(router_info.serialize(&signing_key));
        let storage = ProfileStorage::<MockRuntime>::new(&[], &[]);
        let (_event_mgr, _event_subscriber, event_handle) =
            EventManager::new(None, MockRuntime::register_metrics(vec![], None));
        let (handle, netdb_rx) = NetDbHandle::create();
        let ctx = RouterContext::new(
            MockRuntime::register_metrics(vec![], None),
            storage.clone(),
            router_info.identity.id(),
            serialized.clone(),
            static_key,
            signing_key,
            2u8,
            event_handle.clone(),
        );
        let router_id = RouterId::random();

        let (dial_tx, dial_rx) = channel(100);
        let (transport_tx, transport_rx) = channel(100);
        let mut builder = TransportManagerBuilder::<MockRuntime>::new(
            ctx,
            router_info,
            true,
            dial_rx,
            transport_tx,
        );
        builder.register_netdb_handle(handle);
        let mut manager = builder.build();

        #[derive(Clone, Default)]
        enum Command {
            #[allow(unused)]
            Accept(RouterId),
            #[allow(unused)]
            Reject(RouterId),
            #[default]
            Dummy,
        }

        pub struct MockTransport {
            tx: mpsc::Sender<Command>,
            event_rx: mpsc::Receiver<TransportEvent>,
        }

        impl Transport for MockTransport {
            fn connect(&mut self, _: RouterInfo) {
                todo!();
            }

            fn accept(&mut self, router_id: &RouterId) {
                self.tx.try_send(Command::Accept(router_id.clone())).unwrap();
            }

            fn reject(&mut self, router_id: &RouterId) {
                self.tx.try_send(Command::Reject(router_id.clone())).unwrap();
            }
        }

        impl Stream for MockTransport {
            type Item = TransportEvent;

            fn poll_next(
                mut self: Pin<&mut Self>,
                cx: &mut Context<'_>,
            ) -> Poll<Option<Self::Item>> {
                self.event_rx.poll_recv(cx)
            }
        }

        let (tx, _rx) = mpsc::channel(64);
        let (_event_tx, event_rx) = mpsc::channel(64);

        manager.transports.push(Box::new(MockTransport { event_rx, tx }));
        tokio::spawn(manager);

        dial_tx.send(router_id.clone()).await.unwrap();

        // since RI for `router_id` doesn't exist, it's looked up from netdb
        let query_result_tx = match tokio::time::timeout(Duration::from_secs(5), netdb_rx.recv())
            .await
            .expect("no timeout")
            .expect("to succeed")
        {
            NetDbAction::QueryRouterInfo { router_id: rid, tx } => {
                assert_eq!(rid, router_id);
                tx
            }
            _ => panic!("invalid command"),
        };

        // RI not found, verify that nothing is reported
        query_result_tx.send(Err(QueryError::ValueNotFound)).unwrap();

        tokio::time::sleep(Duration::from_secs(2)).await;

        match tokio::time::timeout(Duration::from_secs(5), transport_rx.recv())
            .await
            .expect("no timeout")
            .expect("to succeed")
        {
            SubsystemEvent::ConnectionFailure { router_id: router } => {
                assert_eq!(router, router_id)
            }
            _ => panic!("invalid event"),
        }
    }

    #[tokio::test]
    async fn router_without_ntcp2_support_dialed() {
        let (router_info, static_key, signing_key) = RouterInfoBuilder::default().build();
        let serialized = Bytes::from(router_info.serialize(&signing_key));
        let storage = ProfileStorage::<MockRuntime>::new(&[], &[]);
        let (remote_router_info, _, _) = RouterInfoBuilder::default()
            .with_ssu2(Ssu2Config {
                port: 888u16,
                host: Some("127.0.0.1".parse().unwrap()),
                publish: true,
                static_key: [1u8; 32],
                intro_key: [2u8; 32],
            })
            .build();
        let remote_router_id = remote_router_info.identity.id();
        storage.add_router(remote_router_info);

        let (_event_mgr, _event_subscriber, event_handle) =
            EventManager::new(None, MockRuntime::register_metrics(vec![], None));
        let (handle, _netdb_rx) = NetDbHandle::create();
        let ctx = RouterContext::new(
            MockRuntime::register_metrics(vec![], None),
            storage.clone(),
            router_info.identity.id(),
            serialized.clone(),
            static_key,
            signing_key,
            2u8,
            event_handle.clone(),
        );

        let (dial_tx, dial_rx) = channel(100);
        let (transport_tx, transport_rx) = channel(100);
        let mut builder = TransportManagerBuilder::<MockRuntime>::new(
            ctx,
            router_info,
            true,
            dial_rx,
            transport_tx,
        );
        builder.register_netdb_handle(handle);
        let context = Ntcp2Transport::<MockRuntime>::initialize(Some(Ntcp2Config {
            port: 0,
            ipv4_host: Some("192.168.0.1".parse().unwrap()),
            ipv6_host: None,
            publish: true,
            key: [0u8; 32],
            iv: [0u8; 16],
            ipv4: true,
            ipv6: false,
        }))
        .await
        .unwrap()
        .0
        .unwrap();
        builder.register_ntcp2(context);
        let mut manager = builder.build();

        #[derive(Clone, Default)]
        enum Command {
            #[default]
            Dummy,
        }

        pub struct MockTransport {
            _tx: mpsc::Sender<Command>,
            event_rx: mpsc::Receiver<TransportEvent>,
        }

        impl Transport for MockTransport {
            fn connect(&mut self, _: RouterInfo) {}
            fn accept(&mut self, _: &RouterId) {}
            fn reject(&mut self, _: &RouterId) {}
        }

        impl Stream for MockTransport {
            type Item = TransportEvent;

            fn poll_next(
                mut self: Pin<&mut Self>,
                cx: &mut Context<'_>,
            ) -> Poll<Option<Self::Item>> {
                self.event_rx.poll_recv(cx)
            }
        }

        let (_tx, _rx) = mpsc::channel(64);
        let (_event_tx, event_rx) = mpsc::channel(64);

        manager.transports.push(Box::new(MockTransport { event_rx, _tx }));
        tokio::spawn(manager);

        // attempt to connect to remote router but since they don't have an ntcp2 address,
        // the dial fails
        dial_tx.send(remote_router_id.clone()).await.unwrap();

        match tokio::time::timeout(Duration::from_secs(5), transport_rx.recv())
            .await
            .expect("no timeout")
            .expect("to succeed")
        {
            SubsystemEvent::ConnectionFailure { router_id: router } => {
                assert_eq!(router, remote_router_id)
            }
            _ => panic!("invalid event"),
        }

        // attempt to connect again and verify that a dial failure is reported
        //
        // this is a fix for a regression where the outbound connection was left in a pending state
        // and all subsequent dials to the remote peer timed out in in `TransitTunnelManager`
        // because the dial attempt didn't proceed as it was erroneously in the pending state
        dial_tx.send(remote_router_id.clone()).await.unwrap();

        match tokio::time::timeout(Duration::from_secs(5), transport_rx.recv())
            .await
            .expect("no timeout")
            .expect("to succeed")
        {
            SubsystemEvent::ConnectionFailure { router_id: router } => {
                assert_eq!(router, remote_router_id)
            }
            _ => panic!("invalid event"),
        }
    }

    #[tokio::test]
    async fn simultaneous_outbound_ssu2_connections() {
        let config1 = Ssu2Config {
            port: 0,
            host: Some("127.0.0.1".parse().unwrap()),
            publish: true,
            static_key: [0xaa; 32],
            intro_key: [0xbb; 32],
        };
        let (context1, address1) =
            Ssu2Transport::<MockRuntime>::initialize(Some(config1)).await.unwrap();
        let (router_info1, static_key1, signing_key1) = RouterInfoBuilder::default()
            .with_ssu2(Ssu2Config {
                port: address1.unwrap().ssu2_ipv4_address().port(),
                host: Some("127.0.0.1".parse().unwrap()),
                publish: true,
                static_key: [0xaa; 32],
                intro_key: [0xbb; 32],
            })
            .build();
        let serialized1 = Bytes::from(router_info1.serialize(&signing_key1));
        let storage1 = ProfileStorage::<MockRuntime>::new(&[], &[]);
        let router_id1 = router_info1.identity.id();

        let config2 = Ssu2Config {
            port: 0,
            host: Some("127.0.0.1".parse().unwrap()),
            publish: true,
            static_key: [0xcc; 32],
            intro_key: [0xdd; 32],
        };
        let (context2, address2) =
            Ssu2Transport::<MockRuntime>::initialize(Some(config2)).await.unwrap();

        let (router_info2, static_key2, signing_key2) = RouterInfoBuilder::default()
            .with_ssu2(Ssu2Config {
                port: address2.unwrap().ssu2_ipv4_address().port(),
                host: Some("127.0.0.1".parse().unwrap()),
                publish: true,
                static_key: [0xcc; 32],
                intro_key: [0xdd; 32],
            })
            .build();
        let serialized2 = Bytes::from(router_info2.serialize(&signing_key2));
        let storage2 = ProfileStorage::<MockRuntime>::new(&[], &[]);
        let router_id2 = router_info2.identity.id();

        storage1.add_router(router_info2.clone());
        storage2.add_router(router_info1.clone());

        let (_event_mgr1, _event_subscriber1, event_handle1) =
            EventManager::new(None, MockRuntime::register_metrics(vec![], None));
        let (_event_mgr2, _event_subscriber2, event_handle2) =
            EventManager::new(None, MockRuntime::register_metrics(vec![], None));
        let (handle1, _netdb_rx1) = NetDbHandle::create();
        let (handle2, _netdb_rx2) = NetDbHandle::create();

        let ctx1 = RouterContext::new(
            MockRuntime::register_metrics(vec![], None),
            storage1.clone(),
            router_info1.identity.id(),
            serialized1.clone(),
            static_key1,
            signing_key1,
            2u8,
            event_handle1.clone(),
        );
        let ctx2 = RouterContext::new(
            MockRuntime::register_metrics(vec![], None),
            storage2.clone(),
            router_info2.identity.id(),
            serialized2.clone(),
            static_key2,
            signing_key2,
            2u8,
            event_handle2.clone(),
        );

        let (dial_tx1, dial_rx1) = channel(100);
        let (transport_tx1, transport_rx1) = channel(100);
        let mut builder1 = TransportManagerBuilder::<MockRuntime>::new(
            ctx1,
            router_info1,
            true,
            dial_rx1,
            transport_tx1,
        );
        builder1.register_ssu2(context1.unwrap());
        builder1.register_netdb_handle(handle1);
        let transport1 = builder1.build();

        let (dial_tx2, dial_rx2) = channel(100);
        let (transport_tx2, transport_rx2) = channel(100);
        let mut builder2 = TransportManagerBuilder::<MockRuntime>::new(
            ctx2,
            router_info2,
            true,
            dial_rx2,
            transport_tx2,
        );
        builder2.register_ssu2(context2.unwrap());
        builder2.register_netdb_handle(handle2);
        let transport2 = builder2.build();

        tokio::spawn(transport1);
        tokio::spawn(transport2);

        dial_tx1.try_send(router_id2.clone()).unwrap();
        dial_tx2.try_send(router_id1.clone()).unwrap();

        match tokio::time::timeout(Duration::from_secs(5), transport_rx1.recv())
            .await
            .expect("no timeout")
            .unwrap()
        {
            SubsystemEvent::ConnectionFailure { router_id } => {
                assert_eq!(router_id, router_id2);
            }
            _ => panic!("invalid event"),
        }

        match tokio::time::timeout(Duration::from_secs(5), transport_rx2.recv())
            .await
            .expect("no timeout")
            .unwrap()
        {
            SubsystemEvent::ConnectionFailure { router_id } => {
                assert_eq!(router_id, router_id1);
            }
            _ => panic!("invalid event"),
        }

        dial_tx2.try_send(router_id1.clone()).unwrap();

        let conn_tx1 = match tokio::time::timeout(Duration::from_secs(5), transport_rx1.recv())
            .await
            .expect("no timeout")
            .unwrap()
        {
            SubsystemEvent::ConnectionEstablished { router_id, tx } => {
                assert_eq!(router_id, router_id2);
                tx
            }
            _ => panic!("invalid event"),
        };

        let conn_tx2 = match tokio::time::timeout(Duration::from_secs(5), transport_rx2.recv())
            .await
            .expect("no timeout")
            .unwrap()
        {
            SubsystemEvent::ConnectionEstablished { router_id, tx } => {
                assert_eq!(router_id, router_id1);
                tx
            }
            _ => panic!("invalid event"),
        };

        conn_tx2
            .try_send(OutboundMessage::Message(Message {
                message_type: MessageType::DatabaseStore,
                message_id: 1337u32,
                expiration: MockRuntime::time_since_epoch() + I2NP_MESSAGE_EXPIRATION,
                payload: vec![1, 1, 1, 1],
            }))
            .unwrap();

        conn_tx1
            .try_send(OutboundMessage::Message(Message {
                message_type: MessageType::DatabaseStore,
                message_id: 1338u32,
                expiration: MockRuntime::time_since_epoch() + I2NP_MESSAGE_EXPIRATION,
                payload: vec![2, 2, 2, 2],
            }))
            .unwrap();

        match tokio::time::timeout(Duration::from_secs(5), transport_rx1.recv())
            .await
            .expect("no timeout")
            .unwrap()
        {
            SubsystemEvent::Message { messages } => {
                assert_eq!(messages.len(), 1);
                assert_eq!(messages[0].0, router_id2);
                assert_eq!(messages[0].1.message_type, MessageType::DatabaseStore);
                assert_eq!(messages[0].1.message_id, 1337u32);
                assert_eq!(messages[0].1.payload, vec![1, 1, 1, 1]);
            }
            _ => panic!("invalid event"),
        }

        match tokio::time::timeout(Duration::from_secs(5), transport_rx2.recv())
            .await
            .expect("no timeout")
            .unwrap()
        {
            SubsystemEvent::Message { messages } => {
                assert_eq!(messages.len(), 1);
                assert_eq!(messages[0].0, router_id1);
                assert_eq!(messages[0].1.message_type, MessageType::DatabaseStore);
                assert_eq!(messages[0].1.message_id, 1338u32);
                assert_eq!(messages[0].1.payload, vec![2, 2, 2, 2]);
            }
            _ => panic!("invalid event"),
        }
    }

    #[tokio::test]
    async fn dial_connected_router() {
        let (router_info, static_key, signing_key) = RouterInfoBuilder::default().build();
        let serialized = Bytes::from(router_info.serialize(&signing_key));
        let (handle, netdb_rx) = NetDbHandle::create();
        let (_event_mgr, _event_subscriber, event_handle) =
            EventManager::new(None, MockRuntime::register_metrics(vec![], None));
        let (storage, remote_router_id) = {
            let (router_info, _static_key, _signing_key) = RouterInfoBuilder::default().build();
            let router_id = router_info.identity.id();
            let storage = ProfileStorage::<MockRuntime>::new(&[], &[]);
            storage.add_router(router_info);

            (storage, router_id)
        };
        let ctx = RouterContext::new(
            MockRuntime::register_metrics(vec![], None),
            storage,
            router_info.identity.id(),
            serialized.clone(),
            static_key,
            signing_key,
            2u8,
            event_handle.clone(),
        );

        let (dial_tx, dial_rx) = channel(100);
        let (transport_tx, _transport_rx) = channel(100);
        let mut builder = TransportManagerBuilder::<MockRuntime>::new(
            ctx,
            router_info,
            true,
            dial_rx,
            transport_tx,
        );
        builder.register_netdb_handle(handle);
        let mut manager = builder.build();

        #[derive(Clone, Default)]
        enum Command {
            Connect(RouterId),
            Accept(RouterId),
            #[default]
            Dummy,
        }

        pub struct MockTransport {
            events: VecDeque<TransportEvent>,
            tx: Sender<Command>,
        }

        impl Transport for MockTransport {
            fn connect(&mut self, info: RouterInfo) {
                self.tx.try_send(Command::Connect(info.identity.id())).unwrap();
            }

            fn accept(&mut self, router_id: &RouterId) {
                self.tx.try_send(Command::Accept(router_id.clone())).unwrap();
            }

            fn reject(&mut self, _: &RouterId) {
                unreachable!()
            }
        }

        impl Stream for MockTransport {
            type Item = TransportEvent;

            fn poll_next(
                mut self: Pin<&mut Self>,
                cx: &mut Context<'_>,
            ) -> Poll<Option<Self::Item>> {
                match self.events.pop_front() {
                    Some(event) => {
                        cx.waker().wake_by_ref();
                        return Poll::Ready(Some(event));
                    }
                    None => {
                        cx.waker().wake_by_ref();
                        Poll::Pending
                    }
                }
            }
        }

        // start mock transport with a single inbound event and send a dial request for that same
        // router over `dial_rx` to `TransportManager`
        let (tx, rx) = channel(64);

        manager.transports.push(Box::new(MockTransport {
            events: VecDeque::from_iter([TransportEvent::ConnectionEstablished {
                address: "127.0.0.1:8888".parse().unwrap(),
                router_id: remote_router_id.clone(),
                direction: Direction::Inbound,
            }]),
            tx,
        }));
        dial_tx.try_send(remote_router_id.clone()).unwrap();

        // verify that router doesn't exist in either connected or pending
        assert!(!manager.routers.contains_key(&remote_router_id));
        assert!(!manager.pending_connections.contains_key(&remote_router_id));

        futures::future::poll_fn(|cx| {
            let _ = manager.poll_unpin(cx);
            Poll::Ready(())
        })
        .await;

        // verify that `TransportManager` now has the router as a connected peer
        assert!(manager.routers.contains_key(&remote_router_id));
        assert!(!manager.pending_connections.contains_key(&remote_router_id));
        assert!(netdb_rx.try_recv().is_err());

        // verify the inbound connection is accepted
        match rx.try_recv().unwrap() {
            Command::Accept(router_id) => assert_eq!(router_id, remote_router_id),
            _ => panic!("invalid command"),
        }

        // verify that the dial request is ignored
        match rx.try_recv() {
            Err(_) => {}
            Ok(Command::Connect(router_id)) if router_id == remote_router_id =>
                panic!("connected router dialed"),
            _ => panic!("unexpected event"),
        }
    }

    #[tokio::test]
    async fn introducers_published() {
        let (router_info, static_key, signing_key) = RouterInfoBuilder::default()
            .with_ssu2(Ssu2Config {
                port: 888,
                host: None,
                publish: false,
                static_key: [0x33; 32],
                intro_key: [0x34; 32],
            })
            .build();

        let (handle, netdb_rx) = NetDbHandle::create();
        let ctx = RouterContextBuilder::default()
            .with_router_info(router_info.clone(), static_key, signing_key)
            .build();

        let (_dial_tx, dial_rx) = channel(100);
        let (transport_tx, _transport_rx) = channel(100);
        let mut builder = TransportManagerBuilder::<MockRuntime>::new(
            ctx,
            router_info,
            true,
            dial_rx,
            transport_tx,
        );
        builder.register_netdb_handle(handle);
        let mut manager = builder.build();

        pub struct MockTransport {
            events: tokio::sync::mpsc::Receiver<TransportEvent>,
        }

        impl Transport for MockTransport {
            fn connect(&mut self, _: RouterInfo) {}
            fn accept(&mut self, _: &RouterId) {}
            fn reject(&mut self, _: &RouterId) {}
        }

        impl Stream for MockTransport {
            type Item = TransportEvent;

            fn poll_next(
                mut self: Pin<&mut Self>,
                cx: &mut Context<'_>,
            ) -> Poll<Option<Self::Item>> {
                Poll::Ready(futures::ready!(self.events.poll_recv(cx)))
            }
        }

        let (t_event_tx, t_event_rx) = tokio::sync::mpsc::channel(16);
        manager.transports.push(Box::new(MockTransport { events: t_event_rx }));

        // set lower timeout for publishing router info
        assert!(manager.introducers.is_empty());
        manager.router_info_republish_timer =
            <MockRuntime as Runtime>::timer(Duration::from_secs(0));

        let router_info = tokio::select! {
            _ = &mut manager => panic!("manager exited"),
            event = netdb_rx.recv() => match event.unwrap() {
                NetDbAction::PublishRouterInfo { router_info, .. } => router_info,
                _ => panic!("invalid event"),
            },
            _ = tokio::time::sleep(Duration::from_secs(5)) => panic!("timeout"),
        };

        let parsed = RouterInfo::parse::<MockRuntime>(&router_info).unwrap();
        assert_eq!(parsed.addresses.len(), 1);

        match &parsed.addresses[0] {
            RouterAddress::Ssu2 { options, .. } => {
                // verify address is not published
                assert!(options.get(&Str::from("port")).is_none());
                assert!(options.get(&Str::from("host")).is_none());

                // verify there are no introducers
                for i in 0..3 {
                    assert!(options.get(&Str::from(format!("iexp{i}"))).is_none());
                    assert!(options.get(&Str::from(format!("ih{i}"))).is_none());
                    assert!(options.get(&Str::from(format!("itag{i}"))).is_none());
                }
            }
            _ => panic!("no ssu2 address available"),
        }

        // register introducers
        let introducer1 = RouterId::random();
        let introducer2 = RouterId::random();
        let introducer3 = RouterId::random();

        t_event_tx
            .send(TransportEvent::IntroducerAdded {
                relay_tag: 1337,
                router_id: introducer1.clone(),
                expires: MockRuntime::time_since_epoch() + Duration::from_secs(80 * 60),
            })
            .await
            .unwrap();
        t_event_tx
            .send(TransportEvent::IntroducerAdded {
                relay_tag: 1338,
                router_id: introducer2.clone(),
                expires: MockRuntime::time_since_epoch() + Duration::from_secs(80 * 60),
            })
            .await
            .unwrap();

        // last introducer has expired
        t_event_tx
            .send(TransportEvent::IntroducerAdded {
                relay_tag: 1339,
                router_id: introducer3.clone(),
                expires: MockRuntime::time_since_epoch() - Duration::from_secs(1),
            })
            .await
            .unwrap();

        // reset publish timer
        manager.router_info_republish_timer =
            <MockRuntime as Runtime>::timer(Duration::from_secs(0));

        let router_info = tokio::select! {
            _ = &mut manager => panic!("manager exited"),
            event = netdb_rx.recv() => match event.unwrap() {
                NetDbAction::PublishRouterInfo { router_info, .. } => router_info,
                _ => panic!("invalid event"),
            },
            _ = tokio::time::sleep(Duration::from_secs(5)) => panic!("timeout"),
        };

        let parsed = RouterInfo::parse::<MockRuntime>(&router_info).unwrap();
        assert_eq!(parsed.addresses.len(), 1);

        match &parsed.addresses[0] {
            RouterAddress::Ssu2 {
                options,
                introducers,
                ..
            } => {
                // verify address is not published
                assert!(options.get(&Str::from("port")).is_none());
                assert!(options.get(&Str::from("host")).is_none());

                assert_eq!(introducers.len(), 2);
                assert_eq!(introducers[0].0, introducer1);
                assert_eq!(introducers[0].1, 1337);
                assert_eq!(introducers[1].0, introducer2);
                assert_eq!(introducers[1].1, 1338);

                // first introducer
                {
                    assert_eq!(
                        options.get(&Str::from(format!("itag0"))),
                        Some(&Str::from("1337")),
                    );
                    assert_eq!(
                        options.get(&Str::from(format!("ih0"))),
                        Some(&Str::from(base64_encode(introducer1.to_vec()))),
                    );
                }

                // second introducer
                {
                    assert_eq!(
                        options.get(&Str::from(format!("itag1"))),
                        Some(&Str::from("1338")),
                    );
                    assert_eq!(
                        options.get(&Str::from(format!("ih1"))),
                        Some(&Str::from(base64_encode(introducer2.to_vec()))),
                    );
                }
            }
            _ => panic!("no ssu2 address available"),
        }
    }

    #[derive(Default)]
    struct TestContextBuilder {
        ssu2: Option<Box<dyn Transport<Item = TransportEvent>>>,
        ntcp2: Option<Box<dyn Transport<Item = TransportEvent>>>,
        routers: Vec<RouterInfo>,
        transit_tunnels_disabled: bool,
        caps: Option<String>,
    }

    impl TestContextBuilder {
        fn with_ssu2(mut self, ssu2: Box<dyn Transport<Item = TransportEvent>>) -> Self {
            self.ssu2 = Some(ssu2);
            self
        }

        fn with_ntcp2(mut self, ntcp2: Box<dyn Transport<Item = TransportEvent>>) -> Self {
            self.ntcp2 = Some(ntcp2);
            self
        }

        fn with_router(mut self, router_info: RouterInfo) -> Self {
            self.routers.push(router_info);
            self
        }

        fn with_transit_tunnels_disabled(mut self) -> Self {
            self.transit_tunnels_disabled = true;
            self
        }

        fn with_caps(mut self, caps: String) -> Self {
            self.caps = Some(caps);
            self
        }

        fn build(
            self,
        ) -> (
            TransportManager<MockRuntime>,
            Receiver<NetDbAction, NetDbActionRecycle>,
            Sender<RouterId>,
            Receiver<SubsystemEvent>,
        ) {
            let mut builder = RouterInfoBuilder::default();

            if self.ssu2.is_some() {
                builder = builder.with_ssu2(Ssu2Config {
                    port: 8888,
                    host: None,
                    publish: false,
                    static_key: [0x33; 32],
                    intro_key: [0x34; 32],
                });
            }

            if self.ntcp2.is_some() {
                builder = builder.with_ntcp2(Ntcp2Config {
                    port: 8889,
                    ipv4_host: None,
                    ipv6_host: None,
                    publish: false,
                    key: [0xaa; 32],
                    iv: [0xbb; 16],
                    ipv4: true,
                    ipv6: false,
                });
            }
            let (router_info, static_key, signing_key) = builder.build();

            let profile_storage = if self.routers.is_empty() {
                ProfileStorage::new(&[], &[])
            } else {
                let storage = ProfileStorage::new(&[], &[]);

                for router in self.routers {
                    storage.add_router(router);
                }

                storage
            };

            let (handle, netdb_rx) = NetDbHandle::create();
            let ctx = RouterContextBuilder::default()
                .with_profile_storage(profile_storage)
                .with_router_info(router_info.clone(), static_key, signing_key)
                .build();

            let (dial_tx, dial_rx) = channel(100);
            let (transport_tx, transport_rx) = channel(100);
            let mut builder = TransportManagerBuilder::<MockRuntime>::new(
                ctx,
                router_info,
                true,
                dial_rx,
                transport_tx,
            );
            builder.register_netdb_handle(handle);

            if self.transit_tunnels_disabled {
                builder.with_transit_tunnels_disabled(true);
            }

            if let Some(ref caps) = self.caps {
                builder.with_capabilities(caps.clone());
            }

            let mut manager = builder.build();

            if let Some(ntcp2) = self.ntcp2 {
                manager.transports.push(ntcp2);
                manager.supported_transports.insert(TransportKind::Ntcp2V4);
            }

            if let Some(ssu2) = self.ssu2 {
                manager.transports.push(ssu2);
                manager.supported_transports.insert(TransportKind::Ssu2V4);
            }

            (manager, netdb_rx, dial_tx, transport_rx)
        }
    }

    #[tokio::test]
    async fn dial_ntcp2() {
        pub struct MockTransport {
            tx: tokio::sync::mpsc::Sender<RouterId>,
        }

        impl Transport for MockTransport {
            fn connect(&mut self, router_info: RouterInfo) {
                self.tx.try_send(router_info.identity.id()).unwrap();
            }
            fn accept(&mut self, _: &RouterId) {}
            fn reject(&mut self, _: &RouterId) {}
        }

        impl Stream for MockTransport {
            type Item = TransportEvent;

            fn poll_next(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Option<Self::Item>> {
                Poll::Pending
            }
        }

        let (router_info, ..) = RouterInfoBuilder::default()
            .with_ntcp2(Ntcp2Config {
                port: 9999,
                ipv4_host: Some("127.0.0.1".parse().unwrap()),
                ipv6_host: None,
                publish: true,
                key: [0x11; 32],
                iv: [0x22; 16],
                ipv4: true,
                ipv6: false,
            })
            .build();
        let router_id = router_info.identity.id();

        let (tx, mut rx) = tokio::sync::mpsc::channel(16);
        let (manager, _netdb_rx, dial_tx, _subsys_rx) = TestContextBuilder::default()
            .with_router(router_info)
            .with_ntcp2(Box::new(MockTransport { tx }))
            .build();
        tokio::spawn(manager);

        dial_tx.send(router_id.clone()).await.unwrap();

        assert_eq!(timeout!(rx.recv()).await.unwrap().unwrap(), router_id);
    }

    // attempt to dial router over ntcp2 but transport not enabled
    #[tokio::test]
    async fn dial_ntcp2_not_available() {
        pub struct MockTransport {
            tx: tokio::sync::mpsc::Sender<RouterId>,
        }

        impl Transport for MockTransport {
            fn connect(&mut self, router_info: RouterInfo) {
                self.tx.try_send(router_info.identity.id()).unwrap();
            }
            fn accept(&mut self, _: &RouterId) {}
            fn reject(&mut self, _: &RouterId) {}
        }

        impl Stream for MockTransport {
            type Item = TransportEvent;

            fn poll_next(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Option<Self::Item>> {
                Poll::Pending
            }
        }

        let (router_info, ..) = RouterInfoBuilder::default()
            .with_ntcp2(Ntcp2Config {
                port: 9999,
                ipv4_host: Some("127.0.0.1".parse().unwrap()),
                ipv6_host: None,
                publish: true,
                key: [0x11; 32],
                iv: [0x22; 16],
                ipv4: true,
                ipv6: false,
            })
            .build();
        let router_id = router_info.identity.id();

        let (tx, mut rx) = tokio::sync::mpsc::channel(16);
        let (manager, _netdb_rx, dial_tx, subsys_rx) = TestContextBuilder::default()
            .with_router(router_info)
            .with_ssu2(Box::new(MockTransport { tx }))
            .build();
        tokio::spawn(manager);

        dial_tx.send(router_id.clone()).await.unwrap();

        match timeout!(subsys_rx.recv()).await.unwrap().unwrap() {
            SubsystemEvent::ConnectionFailure { router_id: remote } =>
                assert_eq!(remote, router_id),
            _ => panic!("invalid event"),
        }

        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn dial_ssu2() {
        pub struct MockTransport {
            tx: tokio::sync::mpsc::Sender<RouterId>,
        }

        impl Transport for MockTransport {
            fn connect(&mut self, router_info: RouterInfo) {
                self.tx.try_send(router_info.identity.id()).unwrap();
            }
            fn accept(&mut self, _: &RouterId) {}
            fn reject(&mut self, _: &RouterId) {}
        }

        impl Stream for MockTransport {
            type Item = TransportEvent;

            fn poll_next(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Option<Self::Item>> {
                Poll::Pending
            }
        }

        let (router_info, ..) = RouterInfoBuilder::default()
            .with_ssu2(Ssu2Config {
                port: 9999,
                host: Some("127.0.0.1".parse().unwrap()),
                publish: true,
                static_key: [0x11; 32],
                intro_key: [0x22; 32],
            })
            .build();
        let router_id = router_info.identity.id();

        let (tx, mut rx) = tokio::sync::mpsc::channel(16);
        let (manager, _netdb_rx, dial_tx, _subsys_rx) = TestContextBuilder::default()
            .with_router(router_info)
            .with_ssu2(Box::new(MockTransport { tx }))
            .build();
        tokio::spawn(manager);

        dial_tx.send(router_id.clone()).await.unwrap();

        assert_eq!(timeout!(rx.recv()).await.unwrap().unwrap(), router_id);
    }

    // attempt to dial router over ssu2 but transport not enabled
    #[tokio::test]
    async fn dial_ssu2_not_available() {
        pub struct MockTransport {
            tx: tokio::sync::mpsc::Sender<RouterId>,
        }

        impl Transport for MockTransport {
            fn connect(&mut self, router_info: RouterInfo) {
                self.tx.try_send(router_info.identity.id()).unwrap();
            }
            fn accept(&mut self, _: &RouterId) {}
            fn reject(&mut self, _: &RouterId) {}
        }

        impl Stream for MockTransport {
            type Item = TransportEvent;

            fn poll_next(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Option<Self::Item>> {
                Poll::Pending
            }
        }

        let (router_info, ..) = RouterInfoBuilder::default()
            .with_ssu2(Ssu2Config {
                port: 9999,
                host: Some("127.0.0.1".parse().unwrap()),
                publish: true,
                static_key: [0x11; 32],
                intro_key: [0x22; 32],
            })
            .build();
        let router_id = router_info.identity.id();

        let (tx, mut rx) = tokio::sync::mpsc::channel(16);
        let (manager, _netdb_rx, dial_tx, subsys_rx) = TestContextBuilder::default()
            .with_router(router_info)
            .with_ntcp2(Box::new(MockTransport { tx }))
            .build();
        tokio::spawn(manager);

        dial_tx.send(router_id.clone()).await.unwrap();

        match timeout!(subsys_rx.recv()).await.unwrap().unwrap() {
            SubsystemEvent::ConnectionFailure { router_id: remote } =>
                assert_eq!(remote, router_id),
            _ => panic!("invalid event"),
        }

        assert!(rx.try_recv().is_err());
    }

    // attempt to dial router which doens't have a published address
    //
    // verify that since the router's introducer is already connected,
    // the router is also dialed immediately
    #[tokio::test]
    async fn dial_ssu2_connected_introducer() {
        pub struct MockTransport {
            tx: tokio::sync::mpsc::Sender<RouterId>,
        }

        impl Transport for MockTransport {
            fn connect(&mut self, router_info: RouterInfo) {
                self.tx.try_send(router_info.identity.id()).unwrap();
            }
            fn accept(&mut self, _: &RouterId) {}
            fn reject(&mut self, _: &RouterId) {}
        }

        impl Stream for MockTransport {
            type Item = TransportEvent;

            fn poll_next(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Option<Self::Item>> {
                Poll::Pending
            }
        }

        let (introducer, introducer_router_id) = {
            let (introducer, _, sigkey) = RouterInfoBuilder::default()
                .with_ssu2(Ssu2Config {
                    port: 9999,
                    host: Some("127.0.0.1".parse().unwrap()),
                    publish: true,
                    static_key: [0x11; 32],
                    intro_key: [0x22; 32],
                })
                .build();

            (
                RouterInfo::parse::<MockRuntime>(introducer.serialize(&sigkey)).unwrap(),
                introducer.identity.id(),
            )
        };

        let (mut router_info, router_id) = {
            let (router_info, _, sigkey) = RouterInfoBuilder::default()
                .with_ssu2(Ssu2Config {
                    port: 10000,
                    host: None,
                    publish: false,
                    static_key: [0x33; 32],
                    intro_key: [0x44; 32],
                })
                .build();

            (
                RouterInfo::parse::<MockRuntime>(router_info.serialize(&sigkey)).unwrap(),
                router_info.identity.id(),
            )
        };

        // add introducer for `router_info`
        match router_info.addresses.get_mut(0).unwrap() {
            RouterAddress::Ssu2 { introducers, .. } => {
                introducers.push((introducer_router_id.clone(), 1337));
            }
            _ => panic!("expected ssu2"),
        }

        let (tx, mut rx) = tokio::sync::mpsc::channel(16);
        let (mut manager, _netdb_rx, dial_tx, _subsys_rx) = TestContextBuilder::default()
            .with_router(router_info)
            .with_router(introducer)
            .with_ssu2(Box::new(MockTransport { tx }))
            .build();

        // mark introducer as connected router
        manager.routers.insert(introducer_router_id, true);
        tokio::spawn(manager);

        dial_tx.send(router_id.clone()).await.unwrap();

        assert_eq!(timeout!(rx.recv()).await.unwrap().unwrap(), router_id);
    }

    // introducer is already being dialed and then the dial fails
    //
    // report that a dial failure is also reported for the router that needed relay
    #[tokio::test]
    async fn dial_ssu2_introducer_pending_then_fails() {
        pub struct MockTransport {
            rx: tokio::sync::mpsc::Receiver<RouterId>,
        }

        impl Transport for MockTransport {
            fn connect(&mut self, _: RouterInfo) {}
            fn accept(&mut self, _: &RouterId) {}
            fn reject(&mut self, _: &RouterId) {}
        }

        impl Stream for MockTransport {
            type Item = TransportEvent;

            fn poll_next(
                mut self: Pin<&mut Self>,
                cx: &mut Context<'_>,
            ) -> Poll<Option<Self::Item>> {
                let router_id = futures::ready!(self.rx.poll_recv(cx)).unwrap();

                Poll::Ready(Some(TransportEvent::ConnectionFailure { router_id }))
            }
        }

        let (introducer, introducer_router_id) = {
            let (introducer, _, sigkey) = RouterInfoBuilder::default()
                .with_ssu2(Ssu2Config {
                    port: 9999,
                    host: Some("127.0.0.1".parse().unwrap()),
                    publish: true,
                    static_key: [0x11; 32],
                    intro_key: [0x22; 32],
                })
                .build();

            (
                RouterInfo::parse::<MockRuntime>(introducer.serialize(&sigkey)).unwrap(),
                introducer.identity.id(),
            )
        };

        let (mut router_info, router_id) = {
            let (router_info, _, sigkey) = RouterInfoBuilder::default()
                .with_ssu2(Ssu2Config {
                    port: 10000,
                    host: None,
                    publish: false,
                    static_key: [0x33; 32],
                    intro_key: [0x44; 32],
                })
                .build();

            (
                RouterInfo::parse::<MockRuntime>(router_info.serialize(&sigkey)).unwrap(),
                router_info.identity.id(),
            )
        };

        // add introducer for `router_info`
        match router_info.addresses.get_mut(0).unwrap() {
            RouterAddress::Ssu2 { introducers, .. } => {
                introducers.push((introducer_router_id.clone(), 1337));
            }
            _ => panic!("expected ssu2"),
        }

        let (tx, rx) = tokio::sync::mpsc::channel(16);
        let (mut manager, _netdb_rx, dial_tx, subsys_rx) = TestContextBuilder::default()
            .with_router(router_info)
            .with_router(introducer)
            .with_ssu2(Box::new(MockTransport { rx }))
            .build();

        // mark introducer as pending
        manager.pending_connections.insert(introducer_router_id.clone(), Vec::new());

        // send dial request for router that needs relay
        dial_tx.send(router_id.clone()).await.unwrap();

        futures::future::poll_fn(|cx| match manager.poll_unpin(cx) {
            Poll::Pending => Poll::Ready(()),
            Poll::Ready(_) => panic!("manager returned"),
        })
        .await;

        // verify that router is tracked in introducer's pending context
        //
        // also verify the router has a pending connection
        assert!(manager
            .pending_connections
            .get(&introducer_router_id)
            .unwrap()
            .iter()
            .any(|client| client == &router_id));
        assert!(manager.pending_connections.contains_key(&router_id));

        // send dial failure for introducer
        tx.send(introducer_router_id.clone()).await.unwrap();

        futures::future::poll_fn(|cx| match manager.poll_unpin(cx) {
            Poll::Pending => Poll::Ready(()),
            Poll::Ready(_) => panic!("manager returned"),
        })
        .await;

        // verify that both the introducer and the router are no longer considered pending
        assert!(!manager.pending_connections.contains_key(&introducer_router_id));
        assert!(!manager.pending_connections.contains_key(&router_id));
        assert!(!manager.routers.contains_key(&introducer_router_id));
        assert!(!manager.routers.contains_key(&router_id));

        // verify that dial failure is reported to subsystem manager for `router_id`
        //
        // dial failure for the introducer is reported by the transport (omitted for
        // `MockTransport`) so the channel only contains an event for the client
        match timeout!(subsys_rx.recv()).await.unwrap().unwrap() {
            SubsystemEvent::ConnectionFailure { router_id: remote } =>
                assert_eq!(remote, router_id),
            _ => panic!("invalid event"),
        }
        assert!(subsys_rx.try_recv().is_err());
    }

    // introducer is already being dialed and then the dial succeeds
    //
    // verify that a dial is started for the router who needed relay
    #[tokio::test]
    async fn dial_ssu2_introducer_pending_then_succeeds() {
        pub struct MockTransport {
            event_rx: tokio::sync::mpsc::Receiver<RouterId>,
            conn_tx: tokio::sync::mpsc::Sender<RouterId>,
        }

        impl Transport for MockTransport {
            fn connect(&mut self, router_info: RouterInfo) {
                self.conn_tx.try_send(router_info.identity.id()).unwrap();
            }
            fn accept(&mut self, _: &RouterId) {}
            fn reject(&mut self, _: &RouterId) {}
        }

        impl Stream for MockTransport {
            type Item = TransportEvent;

            fn poll_next(
                mut self: Pin<&mut Self>,
                cx: &mut Context<'_>,
            ) -> Poll<Option<Self::Item>> {
                let router_id = futures::ready!(self.event_rx.poll_recv(cx)).unwrap();

                Poll::Ready(Some(TransportEvent::ConnectionEstablished {
                    address: "127.0.0.1:8888".parse().unwrap(),
                    direction: Direction::Outbound,
                    router_id,
                }))
            }
        }

        let (introducer, introducer_router_id) = {
            let (introducer, _, sigkey) = RouterInfoBuilder::default()
                .with_ssu2(Ssu2Config {
                    port: 9999,
                    host: Some("127.0.0.1".parse().unwrap()),
                    publish: true,
                    static_key: [0x11; 32],
                    intro_key: [0x22; 32],
                })
                .build();

            (
                RouterInfo::parse::<MockRuntime>(introducer.serialize(&sigkey)).unwrap(),
                introducer.identity.id(),
            )
        };

        let (mut router_info, router_id) = {
            let (router_info, _, sigkey) = RouterInfoBuilder::default()
                .with_ssu2(Ssu2Config {
                    port: 10000,
                    host: None,
                    publish: false,
                    static_key: [0x33; 32],
                    intro_key: [0x44; 32],
                })
                .build();

            (
                RouterInfo::parse::<MockRuntime>(router_info.serialize(&sigkey)).unwrap(),
                router_info.identity.id(),
            )
        };

        // add introducer for `router_info`
        match router_info.addresses.get_mut(0).unwrap() {
            RouterAddress::Ssu2 { introducers, .. } => {
                introducers.push((introducer_router_id.clone(), 1337));
            }
            _ => panic!("expected ssu2"),
        }

        let (event_tx, event_rx) = tokio::sync::mpsc::channel(16);
        let (conn_tx, mut conn_rx) = tokio::sync::mpsc::channel(16);

        let (mut manager, _netdb_rx, dial_tx, _subsys_rx) = TestContextBuilder::default()
            .with_router(router_info)
            .with_router(introducer)
            .with_ssu2(Box::new(MockTransport { event_rx, conn_tx }))
            .build();

        // mark introducer as pending
        manager.pending_connections.insert(introducer_router_id.clone(), Vec::new());

        // send dial request for router that needs relay
        dial_tx.send(router_id.clone()).await.unwrap();

        futures::future::poll_fn(|cx| match manager.poll_unpin(cx) {
            Poll::Pending => Poll::Ready(()),
            Poll::Ready(_) => panic!("manager returned"),
        })
        .await;

        // verify that router is tracked in introducer's pending context
        //
        // also verify the router has a pending connection
        assert!(manager
            .pending_connections
            .get(&introducer_router_id)
            .unwrap()
            .iter()
            .any(|client| client == &router_id));
        assert!(manager.pending_connections.contains_key(&router_id));

        // send dial success for introducer
        event_tx.send(introducer_router_id.clone()).await.unwrap();

        futures::future::poll_fn(|cx| match manager.poll_unpin(cx) {
            Poll::Pending => Poll::Ready(()),
            Poll::Ready(_) => panic!("manager returned"),
        })
        .await;

        // verify that the introducer is considered connected
        assert!(!manager.pending_connections.contains_key(&introducer_router_id));
        assert!(manager.routers.contains_key(&introducer_router_id));

        // and that the router is considered pending
        assert!(manager.pending_connections.contains_key(&router_id));
        assert!(!manager.routers.contains_key(&router_id));

        // verify that transport manager started to dial the router
        assert_eq!(timeout!(conn_rx.recv()).await.unwrap().unwrap(), router_id);
    }

    // introducer found in storage
    //
    // start dialing the router and once the connection fails,
    // verify that tha dial failure is reported for the router
    #[tokio::test]
    async fn dial_ssu2_introducer_found_in_storage_dial_fails() {
        pub struct MockTransport {
            event_rx: tokio::sync::mpsc::Receiver<RouterId>,
            conn_tx: tokio::sync::mpsc::Sender<RouterId>,
        }

        impl Transport for MockTransport {
            fn connect(&mut self, router_info: RouterInfo) {
                self.conn_tx.try_send(router_info.identity.id()).unwrap();
            }
            fn accept(&mut self, _: &RouterId) {}
            fn reject(&mut self, _: &RouterId) {}
        }

        impl Stream for MockTransport {
            type Item = TransportEvent;

            fn poll_next(
                mut self: Pin<&mut Self>,
                cx: &mut Context<'_>,
            ) -> Poll<Option<Self::Item>> {
                let router_id = futures::ready!(self.event_rx.poll_recv(cx)).unwrap();

                Poll::Ready(Some(TransportEvent::ConnectionFailure { router_id }))
            }
        }

        let (introducer, introducer_router_id) = {
            let (introducer, _, sigkey) = RouterInfoBuilder::default()
                .with_ssu2(Ssu2Config {
                    port: 9999,
                    host: Some("127.0.0.1".parse().unwrap()),
                    publish: true,
                    static_key: [0x11; 32],
                    intro_key: [0x22; 32],
                })
                .build();

            (
                RouterInfo::parse::<MockRuntime>(introducer.serialize(&sigkey)).unwrap(),
                introducer.identity.id(),
            )
        };

        let (mut router_info, router_id) = {
            let (router_info, _, sigkey) = RouterInfoBuilder::default()
                .with_ssu2(Ssu2Config {
                    port: 10000,
                    host: None,
                    publish: false,
                    static_key: [0x33; 32],
                    intro_key: [0x44; 32],
                })
                .build();

            (
                RouterInfo::parse::<MockRuntime>(router_info.serialize(&sigkey)).unwrap(),
                router_info.identity.id(),
            )
        };

        // add introducer for `router_info`
        match router_info.addresses.get_mut(0).unwrap() {
            RouterAddress::Ssu2 { introducers, .. } => {
                introducers.push((introducer_router_id.clone(), 1337));
            }
            _ => panic!("expected ssu2"),
        }

        let (event_tx, event_rx) = tokio::sync::mpsc::channel(16);
        let (conn_tx, mut conn_rx) = tokio::sync::mpsc::channel(16);

        let (mut manager, _netdb_rx, dial_tx, subsys_rx) = TestContextBuilder::default()
            .with_router(router_info)
            .with_router(introducer)
            .with_ssu2(Box::new(MockTransport { event_rx, conn_tx }))
            .build();

        // send dial request for router that needs relay
        dial_tx.send(router_id.clone()).await.unwrap();

        futures::future::poll_fn(|cx| match manager.poll_unpin(cx) {
            Poll::Pending => Poll::Ready(()),
            Poll::Ready(_) => panic!("manager returned"),
        })
        .await;

        // verify that router is tracked in introducer's pending context
        //
        // also verify the router has a pending connection
        assert!(manager
            .pending_connections
            .get(&introducer_router_id)
            .unwrap()
            .iter()
            .any(|client| client == &router_id));
        assert!(manager.pending_connections.contains_key(&router_id));

        // verify that the introducer is dialed
        assert_eq!(
            timeout!(conn_rx.recv()).await.unwrap().unwrap(),
            introducer_router_id
        );

        // send dial failure for introducer
        event_tx.send(introducer_router_id.clone()).await.unwrap();

        futures::future::poll_fn(|cx| match manager.poll_unpin(cx) {
            Poll::Pending => Poll::Ready(()),
            Poll::Ready(_) => panic!("manager returned"),
        })
        .await;

        // verify that the introducer is no longer pending nor connected
        assert!(!manager.pending_connections.contains_key(&introducer_router_id));
        assert!(!manager.routers.contains_key(&introducer_router_id));

        // and that the router is no longer pending
        assert!(!manager.pending_connections.contains_key(&router_id));
        assert!(!manager.routers.contains_key(&router_id));

        // verify that subsystem manager is notified of the dial failure
        match timeout!(subsys_rx.recv()).await.unwrap().unwrap() {
            SubsystemEvent::ConnectionFailure { router_id: remote } =>
                assert_eq!(remote, router_id),
            _ => panic!("invalid event"),
        }
        assert!(subsys_rx.try_recv().is_err());
    }

    // introducer found in storage
    //
    // start dialing the introducer and once the connection succeeds,
    // verify that tha dial failure is reported for the router
    #[tokio::test]
    async fn dial_ssu2_introducer_found_in_storage_dial_succeeds() {
        pub struct MockTransport {
            event_rx: tokio::sync::mpsc::Receiver<RouterId>,
            conn_tx: tokio::sync::mpsc::Sender<RouterId>,
        }

        impl Transport for MockTransport {
            fn connect(&mut self, router_info: RouterInfo) {
                self.conn_tx.try_send(router_info.identity.id()).unwrap();
            }
            fn accept(&mut self, _: &RouterId) {}
            fn reject(&mut self, _: &RouterId) {}
        }

        impl Stream for MockTransport {
            type Item = TransportEvent;

            fn poll_next(
                mut self: Pin<&mut Self>,
                cx: &mut Context<'_>,
            ) -> Poll<Option<Self::Item>> {
                let router_id = futures::ready!(self.event_rx.poll_recv(cx)).unwrap();

                Poll::Ready(Some(TransportEvent::ConnectionEstablished {
                    address: "127.0.0.1:8888".parse().unwrap(),
                    direction: Direction::Outbound,
                    router_id,
                }))
            }
        }

        let (introducer, introducer_router_id) = {
            let (introducer, _, sigkey) = RouterInfoBuilder::default()
                .with_ssu2(Ssu2Config {
                    port: 9999,
                    host: Some("127.0.0.1".parse().unwrap()),
                    publish: true,
                    static_key: [0x11; 32],
                    intro_key: [0x22; 32],
                })
                .build();

            (
                RouterInfo::parse::<MockRuntime>(introducer.serialize(&sigkey)).unwrap(),
                introducer.identity.id(),
            )
        };

        let (mut router_info, router_id) = {
            let (router_info, _, sigkey) = RouterInfoBuilder::default()
                .with_ssu2(Ssu2Config {
                    port: 10000,
                    host: None,
                    publish: false,
                    static_key: [0x33; 32],
                    intro_key: [0x44; 32],
                })
                .build();

            (
                RouterInfo::parse::<MockRuntime>(router_info.serialize(&sigkey)).unwrap(),
                router_info.identity.id(),
            )
        };

        // add introducer for `router_info`
        match router_info.addresses.get_mut(0).unwrap() {
            RouterAddress::Ssu2 { introducers, .. } => {
                introducers.push((introducer_router_id.clone(), 1337));
            }
            _ => panic!("expected ssu2"),
        }

        let (event_tx, event_rx) = tokio::sync::mpsc::channel(16);
        let (conn_tx, mut conn_rx) = tokio::sync::mpsc::channel(16);

        let (mut manager, _netdb_rx, dial_tx, _subsys_rx) = TestContextBuilder::default()
            .with_router(router_info)
            .with_router(introducer)
            .with_ssu2(Box::new(MockTransport { event_rx, conn_tx }))
            .build();

        // send dial request for router that needs relay
        dial_tx.send(router_id.clone()).await.unwrap();

        futures::future::poll_fn(|cx| match manager.poll_unpin(cx) {
            Poll::Pending => Poll::Ready(()),
            Poll::Ready(_) => panic!("manager returned"),
        })
        .await;

        // verify that router is tracked in introducer's pending context
        //
        // also verify the router has a pending connection
        assert!(manager
            .pending_connections
            .get(&introducer_router_id)
            .unwrap()
            .iter()
            .any(|client| client == &router_id));
        assert!(manager.pending_connections.contains_key(&router_id));

        // verify that the introducer is dialed
        assert_eq!(
            timeout!(conn_rx.recv()).await.unwrap().unwrap(),
            introducer_router_id
        );

        // send dial success for introducer
        event_tx.send(introducer_router_id.clone()).await.unwrap();

        futures::future::poll_fn(|cx| match manager.poll_unpin(cx) {
            Poll::Pending => Poll::Ready(()),
            Poll::Ready(_) => panic!("manager returned"),
        })
        .await;

        // verify that the introducer is considered connected
        assert!(!manager.pending_connections.contains_key(&introducer_router_id));
        assert!(manager.routers.contains_key(&introducer_router_id));

        // and that the router is considered pending
        assert!(manager.pending_connections.contains_key(&router_id));
        assert!(!manager.routers.contains_key(&router_id));

        // verify that transport manager started to dial the router
        assert_eq!(timeout!(conn_rx.recv()).await.unwrap().unwrap(), router_id);
    }

    // introducer not found in storage and netdb query is started
    //
    // the query fails and since the router had only a single introducer,
    // the dial fails
    #[tokio::test]
    async fn dial_ssu2_introducer_router_info_query_fails() {
        pub struct MockTransport {
            event_rx: tokio::sync::mpsc::Receiver<RouterId>,
            conn_tx: tokio::sync::mpsc::Sender<RouterId>,
        }

        impl Transport for MockTransport {
            fn connect(&mut self, router_info: RouterInfo) {
                self.conn_tx.try_send(router_info.identity.id()).unwrap();
            }
            fn accept(&mut self, _: &RouterId) {}
            fn reject(&mut self, _: &RouterId) {}
        }

        impl Stream for MockTransport {
            type Item = TransportEvent;

            fn poll_next(
                mut self: Pin<&mut Self>,
                cx: &mut Context<'_>,
            ) -> Poll<Option<Self::Item>> {
                let router_id = futures::ready!(self.event_rx.poll_recv(cx)).unwrap();

                Poll::Ready(Some(TransportEvent::ConnectionEstablished {
                    address: "127.0.0.1:8888".parse().unwrap(),
                    direction: Direction::Outbound,
                    router_id,
                }))
            }
        }

        let (_introducer, introducer_router_id) = {
            let (introducer, _, sigkey) = RouterInfoBuilder::default()
                .with_ssu2(Ssu2Config {
                    port: 9999,
                    host: Some("127.0.0.1".parse().unwrap()),
                    publish: true,
                    static_key: [0x11; 32],
                    intro_key: [0x22; 32],
                })
                .build();

            (
                RouterInfo::parse::<MockRuntime>(introducer.serialize(&sigkey)).unwrap(),
                introducer.identity.id(),
            )
        };

        let (mut router_info, router_id) = {
            let (router_info, _, sigkey) = RouterInfoBuilder::default()
                .with_ssu2(Ssu2Config {
                    port: 10000,
                    host: None,
                    publish: false,
                    static_key: [0x33; 32],
                    intro_key: [0x44; 32],
                })
                .build();

            (
                RouterInfo::parse::<MockRuntime>(router_info.serialize(&sigkey)).unwrap(),
                router_info.identity.id(),
            )
        };

        // add introducer for `router_info`
        match router_info.addresses.get_mut(0).unwrap() {
            RouterAddress::Ssu2 { introducers, .. } => {
                introducers.push((introducer_router_id.clone(), 1337));
            }
            _ => panic!("expected ssu2"),
        }

        let (_event_tx, event_rx) = tokio::sync::mpsc::channel(16);
        let (conn_tx, _conn_rx) = tokio::sync::mpsc::channel(16);

        let (mut manager, netdb_rx, dial_tx, subsys_rx) = TestContextBuilder::default()
            .with_router(router_info)
            .with_ssu2(Box::new(MockTransport { event_rx, conn_tx }))
            .build();

        // send dial request for router that needs relay
        dial_tx.send(router_id.clone()).await.unwrap();

        futures::future::poll_fn(|cx| match manager.poll_unpin(cx) {
            Poll::Pending => Poll::Ready(()),
            Poll::Ready(_) => panic!("manager returned"),
        })
        .await;

        // verify that router is tracked in introducer's pending context
        //
        // also verify the router has a pending connection
        assert!(manager
            .pending_connections
            .get(&introducer_router_id)
            .unwrap()
            .iter()
            .any(|client| client == &router_id));
        assert!(manager.pending_queries.contains(&introducer_router_id));
        assert!(manager.pending_connections.contains_key(&router_id));

        // send query failure back
        match timeout!(netdb_rx.recv()).await.unwrap().unwrap() {
            NetDbAction::QueryRouterInfo {
                router_id: target,
                tx,
            } => {
                assert_eq!(target, introducer_router_id);
                tx.send(Err(QueryError::Timeout)).unwrap();
            }
            _ => panic!("unexpected netdb action"),
        }

        let future = {
            futures::future::poll_fn(|cx| loop {
                match manager.poll_unpin(cx) {
                    Poll::Pending => {
                        if !manager.pending_connections.contains_key(&introducer_router_id)
                            && !manager.pending_queries.contains(&introducer_router_id)
                        {
                            return Poll::Ready(());
                        } else {
                            return Poll::Pending;
                        }
                    }
                    Poll::Ready(_) => panic!("manager returned"),
                }
            })
        };
        let _: () = timeout!(future).await.unwrap();

        // verify that the introducer is no longer considered pending
        assert!(!manager.pending_connections.contains_key(&introducer_router_id));
        assert!(!manager.pending_queries.contains(&introducer_router_id));

        // and that the router is not considered pending
        assert!(!manager.pending_connections.contains_key(&router_id));
        assert!(!manager.routers.contains_key(&router_id));

        // verify that subsystem manager is notified of the client dial failure
        match timeout!(subsys_rx.recv()).await.unwrap().unwrap() {
            SubsystemEvent::ConnectionFailure { router_id: remote } =>
                assert_eq!(remote, router_id),
            _ => panic!("invalid event"),
        }

        // since the dial failure originated from transport and not from transport, subsystem
        // manager is notified of the introducer dial failure directly
        match timeout!(subsys_rx.recv()).await.unwrap().unwrap() {
            SubsystemEvent::ConnectionFailure { router_id: remote } =>
                assert_eq!(remote, introducer_router_id),
            _ => panic!("invalid event"),
        }
    }

    // introducer not found in storage and netdb query is started
    //
    // router info query succeeds but the subsequent dial of the introducer fails,
    // causing the dial for the original router to fail
    #[tokio::test]
    async fn dial_ssu2_introducer_router_info_query_succeeds_dial_fails() {
        pub struct MockTransport {
            event_rx: tokio::sync::mpsc::Receiver<RouterId>,
            conn_tx: tokio::sync::mpsc::Sender<RouterId>,
        }

        impl Transport for MockTransport {
            fn connect(&mut self, router_info: RouterInfo) {
                self.conn_tx.try_send(router_info.identity.id()).unwrap();
            }
            fn accept(&mut self, _: &RouterId) {}
            fn reject(&mut self, _: &RouterId) {}
        }

        impl Stream for MockTransport {
            type Item = TransportEvent;

            fn poll_next(
                mut self: Pin<&mut Self>,
                cx: &mut Context<'_>,
            ) -> Poll<Option<Self::Item>> {
                let router_id = futures::ready!(self.event_rx.poll_recv(cx)).unwrap();

                Poll::Ready(Some(TransportEvent::ConnectionFailure { router_id }))
            }
        }

        let (introducer, introducer_router_id) = {
            let (introducer, _, sigkey) = RouterInfoBuilder::default()
                .with_ssu2(Ssu2Config {
                    port: 9999,
                    host: Some("127.0.0.1".parse().unwrap()),
                    publish: true,
                    static_key: [0x11; 32],
                    intro_key: [0x22; 32],
                })
                .build();

            (
                RouterInfo::parse::<MockRuntime>(introducer.serialize(&sigkey)).unwrap(),
                introducer.identity.id(),
            )
        };

        let (mut router_info, router_id) = {
            let (router_info, _, sigkey) = RouterInfoBuilder::default()
                .with_ssu2(Ssu2Config {
                    port: 10000,
                    host: None,
                    publish: false,
                    static_key: [0x33; 32],
                    intro_key: [0x44; 32],
                })
                .build();

            (
                RouterInfo::parse::<MockRuntime>(router_info.serialize(&sigkey)).unwrap(),
                router_info.identity.id(),
            )
        };

        // add introducer for `router_info`
        match router_info.addresses.get_mut(0).unwrap() {
            RouterAddress::Ssu2 { introducers, .. } => {
                introducers.push((introducer_router_id.clone(), 1337));
            }
            _ => panic!("expected ssu2"),
        }

        let (event_tx, event_rx) = tokio::sync::mpsc::channel(16);
        let (conn_tx, mut conn_rx) = tokio::sync::mpsc::channel(16);

        let (mut manager, netdb_rx, dial_tx, subsys_rx) = TestContextBuilder::default()
            .with_router(router_info)
            .with_ssu2(Box::new(MockTransport { event_rx, conn_tx }))
            .build();

        let profile_storage = manager.router_ctx.profile_storage().clone();

        // send dial request for router that needs relay
        dial_tx.send(router_id.clone()).await.unwrap();

        futures::future::poll_fn(|cx| match manager.poll_unpin(cx) {
            Poll::Pending => Poll::Ready(()),
            Poll::Ready(_) => panic!("manager returned"),
        })
        .await;

        // verify that router is tracked in introducer's pending context
        //
        // also verify the router has a pending connection
        assert!(manager
            .pending_connections
            .get(&introducer_router_id)
            .unwrap()
            .iter()
            .any(|client| client == &router_id));
        assert!(manager.pending_queries.contains(&introducer_router_id));
        assert!(manager.pending_connections.contains_key(&router_id));

        match timeout!(netdb_rx.recv()).await.unwrap().unwrap() {
            NetDbAction::QueryRouterInfo {
                router_id: target,
                tx,
            } => {
                assert_eq!(target, introducer_router_id);
                profile_storage.add_router(introducer);
                tx.send(Ok(())).unwrap();
            }
            _ => panic!("unexpected netdb action"),
        }

        let future = {
            futures::future::poll_fn(|cx| loop {
                match manager.poll_unpin(cx) {
                    Poll::Pending =>
                        if !manager.pending_queries.contains(&introducer_router_id) {
                            return Poll::Ready(());
                        } else {
                            return Poll::Pending;
                        },
                    Poll::Ready(_) => panic!("manager returned"),
                }
            })
        };
        let _: () = timeout!(future).await.unwrap();

        // verify that the introducer is still pending but no longer has a pending query
        assert!(manager.pending_connections.contains_key(&introducer_router_id));
        assert!(!manager.pending_queries.contains(&introducer_router_id));
        assert!(!manager.routers.contains_key(&introducer_router_id));

        // and that the original router is still pending
        assert!(manager.pending_connections.contains_key(&router_id));
        assert!(!manager.routers.contains_key(&router_id));

        // verify that the introducer is dialed
        assert_eq!(
            timeout!(conn_rx.recv()).await.unwrap().unwrap(),
            introducer_router_id
        );

        // send dial failure for introducer
        event_tx.send(introducer_router_id.clone()).await.unwrap();

        futures::future::poll_fn(|cx| match manager.poll_unpin(cx) {
            Poll::Pending => Poll::Ready(()),
            Poll::Ready(_) => panic!("manager returned"),
        })
        .await;

        // verify that subsystem manager is notified of the client dial failure
        match timeout!(subsys_rx.recv()).await.unwrap().unwrap() {
            SubsystemEvent::ConnectionFailure { router_id: remote } =>
                assert_eq!(remote, router_id),
            _ => panic!("invalid event"),
        }
    }

    // introducer not found in storage and netdb query is started
    //
    // router info query succeeds and the subsequent dial of the introducer succeeds,
    // causing the client router to be dialed
    #[tokio::test]
    async fn dial_ssu2_introducer_router_info_query_succeeds_dial_succeeds() {
        pub struct MockTransport {
            event_rx: tokio::sync::mpsc::Receiver<RouterId>,
            conn_tx: tokio::sync::mpsc::Sender<RouterId>,
        }

        impl Transport for MockTransport {
            fn connect(&mut self, router_info: RouterInfo) {
                self.conn_tx.try_send(router_info.identity.id()).unwrap();
            }
            fn accept(&mut self, _: &RouterId) {}
            fn reject(&mut self, _: &RouterId) {}
        }

        impl Stream for MockTransport {
            type Item = TransportEvent;

            fn poll_next(
                mut self: Pin<&mut Self>,
                cx: &mut Context<'_>,
            ) -> Poll<Option<Self::Item>> {
                let router_id = futures::ready!(self.event_rx.poll_recv(cx)).unwrap();

                Poll::Ready(Some(TransportEvent::ConnectionEstablished {
                    address: "127.0.0.1:8888".parse().unwrap(),
                    direction: Direction::Outbound,
                    router_id,
                }))
            }
        }

        let (introducer, introducer_router_id) = {
            let (introducer, _, sigkey) = RouterInfoBuilder::default()
                .with_ssu2(Ssu2Config {
                    port: 9999,
                    host: Some("127.0.0.1".parse().unwrap()),
                    publish: true,
                    static_key: [0x11; 32],
                    intro_key: [0x22; 32],
                })
                .build();

            (
                RouterInfo::parse::<MockRuntime>(introducer.serialize(&sigkey)).unwrap(),
                introducer.identity.id(),
            )
        };

        let (mut router_info, router_id) = {
            let (router_info, _, sigkey) = RouterInfoBuilder::default()
                .with_ssu2(Ssu2Config {
                    port: 10000,
                    host: None,
                    publish: false,
                    static_key: [0x33; 32],
                    intro_key: [0x44; 32],
                })
                .build();

            (
                RouterInfo::parse::<MockRuntime>(router_info.serialize(&sigkey)).unwrap(),
                router_info.identity.id(),
            )
        };

        // add introducer for `router_info`
        match router_info.addresses.get_mut(0).unwrap() {
            RouterAddress::Ssu2 { introducers, .. } => {
                introducers.push((introducer_router_id.clone(), 1337));
            }
            _ => panic!("expected ssu2"),
        }

        let (event_tx, event_rx) = tokio::sync::mpsc::channel(16);
        let (conn_tx, mut conn_rx) = tokio::sync::mpsc::channel(16);

        let (mut manager, netdb_rx, dial_tx, _subsys_rx) = TestContextBuilder::default()
            .with_router(router_info)
            .with_ssu2(Box::new(MockTransport { event_rx, conn_tx }))
            .build();

        let profile_storage = manager.router_ctx.profile_storage().clone();

        // send dial request for router that needs relay
        dial_tx.send(router_id.clone()).await.unwrap();

        futures::future::poll_fn(|cx| match manager.poll_unpin(cx) {
            Poll::Pending => Poll::Ready(()),
            Poll::Ready(_) => panic!("manager returned"),
        })
        .await;

        // verify that router is tracked in introducer's pending context
        //
        // also verify the router has a pending connection
        assert!(manager
            .pending_connections
            .get(&introducer_router_id)
            .unwrap()
            .iter()
            .any(|client| client == &router_id));
        assert!(manager.pending_queries.contains(&introducer_router_id));
        assert!(manager.pending_connections.contains_key(&router_id));

        match timeout!(netdb_rx.recv()).await.unwrap().unwrap() {
            NetDbAction::QueryRouterInfo {
                router_id: target,
                tx,
            } => {
                assert_eq!(target, introducer_router_id);
                profile_storage.add_router(introducer);
                tx.send(Ok(())).unwrap();
            }
            _ => panic!("unexpected netdb action"),
        }

        let future = {
            futures::future::poll_fn(|cx| loop {
                match manager.poll_unpin(cx) {
                    Poll::Pending =>
                        if !manager.pending_queries.contains(&introducer_router_id) {
                            return Poll::Ready(());
                        } else {
                            return Poll::Pending;
                        },
                    Poll::Ready(_) => panic!("manager returned"),
                }
            })
        };
        let _: () = timeout!(future).await.unwrap();

        // verify that the introducer is still pending but no longer has a pending query
        assert!(manager.pending_connections.contains_key(&introducer_router_id));
        assert!(!manager.pending_queries.contains(&introducer_router_id));
        assert!(!manager.routers.contains_key(&introducer_router_id));

        // and that the original router is still pending
        assert!(manager.pending_connections.contains_key(&router_id));
        assert!(!manager.routers.contains_key(&router_id));

        // verify that the introducer is dialed
        assert_eq!(
            timeout!(conn_rx.recv()).await.unwrap().unwrap(),
            introducer_router_id
        );

        // send dial success for introducer
        event_tx.send(introducer_router_id.clone()).await.unwrap();

        futures::future::poll_fn(|cx| match manager.poll_unpin(cx) {
            Poll::Pending => Poll::Ready(()),
            Poll::Ready(_) => panic!("manager returned"),
        })
        .await;

        // verify that the router is dialed
        assert_eq!(timeout!(conn_rx.recv()).await.unwrap().unwrap(), router_id);
    }

    #[tokio::test]
    async fn dial_ssu2_introducer_all_dials_fail() {
        pub struct MockTransport {
            event_rx: tokio::sync::mpsc::Receiver<Event>,
            conn_tx: tokio::sync::mpsc::Sender<RouterId>,
        }

        enum Event {
            Failure(RouterId),
        }

        impl Transport for MockTransport {
            fn connect(&mut self, router_info: RouterInfo) {
                self.conn_tx.try_send(router_info.identity.id()).unwrap();
            }
            fn accept(&mut self, _: &RouterId) {}
            fn reject(&mut self, _: &RouterId) {}
        }

        impl Stream for MockTransport {
            type Item = TransportEvent;

            fn poll_next(
                mut self: Pin<&mut Self>,
                cx: &mut Context<'_>,
            ) -> Poll<Option<Self::Item>> {
                match futures::ready!(self.event_rx.poll_recv(cx)).unwrap() {
                    Event::Failure(router_id) =>
                        Poll::Ready(Some(TransportEvent::ConnectionFailure { router_id })),
                }
            }
        }

        let introducers = (0..3)
            .map(|i| {
                let (introducer, _, sigkey) = RouterInfoBuilder::default()
                    .with_ssu2(Ssu2Config {
                        port: 9999,
                        host: Some("127.0.0.1".parse().unwrap()),
                        publish: true,
                        static_key: [0x11 + i; 32],
                        intro_key: [0x22 + i; 32],
                    })
                    .build();

                (
                    introducer.identity.id(),
                    RouterInfo::parse::<MockRuntime>(introducer.serialize(&sigkey)).unwrap(),
                )
            })
            .collect::<HashMap<_, _>>();

        let (mut router_info, router_id) = {
            let (router_info, _, sigkey) = RouterInfoBuilder::default()
                .with_ssu2(Ssu2Config {
                    port: 10000,
                    host: None,
                    publish: false,
                    static_key: [0x33; 32],
                    intro_key: [0x44; 32],
                })
                .build();

            (
                RouterInfo::parse::<MockRuntime>(router_info.serialize(&sigkey)).unwrap(),
                router_info.identity.id(),
            )
        };

        // add introducer for `router_info`
        match router_info.addresses.get_mut(0).unwrap() {
            RouterAddress::Ssu2 {
                introducers: router_introducers,
                ..
            } =>
                for (i, (introducer_router_id, _)) in introducers.iter().enumerate() {
                    router_introducers.push((introducer_router_id.clone(), 1337 + i as u32));
                },
            _ => panic!("expected ssu2"),
        }

        let (event_tx, event_rx) = tokio::sync::mpsc::channel(16);
        let (conn_tx, mut conn_rx) = tokio::sync::mpsc::channel(16);

        let mut builder = TestContextBuilder::default()
            .with_router(router_info)
            .with_ssu2(Box::new(MockTransport { event_rx, conn_tx }));

        // add all introducers to profile storage
        for (_, introducer) in &introducers {
            builder = builder.with_router(introducer.clone());
        }

        let (mut manager, _netdb_rx, dial_tx, subsys_rx) = builder.build();

        // send dial request for router that needs relay
        dial_tx.send(router_id.clone()).await.unwrap();

        futures::future::poll_fn(|cx| match manager.poll_unpin(cx) {
            Poll::Pending => Poll::Ready(()),
            Poll::Ready(_) => panic!("manager returned"),
        })
        .await;

        // verify all introducers are dialed
        for _ in 0..3 {
            let introducer_router_id = conn_rx.try_recv().unwrap();
            assert!(introducers.contains_key(&introducer_router_id));
        }

        // verify that all introducers are being dialed and that the router
        // is tracked in their dial context
        assert!(introducers.keys().all(|introducer_id| {
            match manager.pending_connections.get(introducer_id) {
                Some(routers) => routers.iter().any(|client| client == &router_id),
                None => false,
            }
        }));

        // verify that all introducers are in the pending introducer context
        assert!(manager
            .pending_introducers
            .get(&router_id)
            .unwrap()
            .pending_connections
            .iter()
            .all(|key| introducers.contains_key(key)));
        assert!(manager.pending_introducers.get(&router_id).unwrap().pending_queries.is_empty());

        // verify that the client router is pending
        assert!(manager.pending_connections.contains_key(&router_id));

        let mut introducers = introducers.into_iter().collect::<Vec<_>>();

        // send dial failures for the first two introducers
        for _ in 0..2 {
            let (introducer_router_id, _) = introducers.pop().unwrap();
            event_tx.send(Event::Failure(introducer_router_id.clone())).await.unwrap();
            futures::future::poll_fn(|cx| match manager.poll_unpin(cx) {
                Poll::Pending => Poll::Ready(()),
                Poll::Ready(_) => panic!("manager returned"),
            })
            .await;

            // verify that the introducer is no longer pending or part of `pending_introducers`
            assert!(!manager
                .pending_introducers
                .get(&router_id)
                .unwrap()
                .pending_connections
                .contains(&introducer_router_id));
            assert!(!manager.pending_connections.contains_key(&introducer_router_id));
            assert!(manager.pending_connections.contains_key(&router_id));
        }

        // send dial failure for the last introducer
        let (introducer_router_id, _) = introducers.pop().unwrap();
        event_tx.send(Event::Failure(introducer_router_id.clone())).await.unwrap();
        futures::future::poll_fn(|cx| match manager.poll_unpin(cx) {
            Poll::Pending => Poll::Ready(()),
            Poll::Ready(_) => panic!("manager returned"),
        })
        .await;
        assert!(!manager.pending_introducers.contains_key(&router_id));
        assert!(!manager.pending_connections.contains_key(&introducer_router_id));
        assert!(!manager.pending_connections.contains_key(&router_id));

        // verify dial failure is reported for the client router
        match timeout!(subsys_rx.recv()).await.unwrap().unwrap() {
            SubsystemEvent::ConnectionFailure { router_id: remote } =>
                assert_eq!(remote, router_id),
            _ => panic!("invalid event"),
        }
    }

    #[tokio::test]
    async fn dial_ssu2_introducer_all_dials_succeed() {
        pub struct MockTransport {
            event_rx: tokio::sync::mpsc::Receiver<Event>,
            conn_tx: tokio::sync::mpsc::Sender<RouterId>,
        }

        enum Event {
            Success(RouterId),
        }

        impl Transport for MockTransport {
            fn connect(&mut self, router_info: RouterInfo) {
                self.conn_tx.try_send(router_info.identity.id()).unwrap();
            }
            fn accept(&mut self, _: &RouterId) {}
            fn reject(&mut self, _: &RouterId) {}
        }

        impl Stream for MockTransport {
            type Item = TransportEvent;

            fn poll_next(
                mut self: Pin<&mut Self>,
                cx: &mut Context<'_>,
            ) -> Poll<Option<Self::Item>> {
                match futures::ready!(self.event_rx.poll_recv(cx)).unwrap() {
                    Event::Success(router_id) =>
                        Poll::Ready(Some(TransportEvent::ConnectionEstablished {
                            address: "127.0.0.1:8888".parse().unwrap(),
                            direction: Direction::Outbound,
                            router_id,
                        })),
                }
            }
        }

        let introducers = (0..3)
            .map(|i| {
                let (introducer, _, sigkey) = RouterInfoBuilder::default()
                    .with_ssu2(Ssu2Config {
                        port: 9999,
                        host: Some("127.0.0.1".parse().unwrap()),
                        publish: true,
                        static_key: [0x11 + i; 32],
                        intro_key: [0x22 + i; 32],
                    })
                    .build();

                (
                    introducer.identity.id(),
                    RouterInfo::parse::<MockRuntime>(introducer.serialize(&sigkey)).unwrap(),
                )
            })
            .collect::<HashMap<_, _>>();

        let (mut router_info, router_id) = {
            let (router_info, _, sigkey) = RouterInfoBuilder::default()
                .with_ssu2(Ssu2Config {
                    port: 10000,
                    host: None,
                    publish: false,
                    static_key: [0x33; 32],
                    intro_key: [0x44; 32],
                })
                .build();

            (
                RouterInfo::parse::<MockRuntime>(router_info.serialize(&sigkey)).unwrap(),
                router_info.identity.id(),
            )
        };

        // add introducer for `router_info`
        match router_info.addresses.get_mut(0).unwrap() {
            RouterAddress::Ssu2 {
                introducers: router_introducers,
                ..
            } =>
                for (i, (introducer_router_id, _)) in introducers.iter().enumerate() {
                    router_introducers.push((introducer_router_id.clone(), 1337 + i as u32));
                },
            _ => panic!("expected ssu2"),
        }

        let (event_tx, event_rx) = tokio::sync::mpsc::channel(16);
        let (conn_tx, mut conn_rx) = tokio::sync::mpsc::channel(16);

        let mut builder = TestContextBuilder::default()
            .with_router(router_info)
            .with_ssu2(Box::new(MockTransport { event_rx, conn_tx }));

        // add all introducers to profile storage
        for (_, introducer) in &introducers {
            builder = builder.with_router(introducer.clone());
        }

        let (mut manager, _netdb_rx, dial_tx, _subsys_rx) = builder.build();

        // send dial request for router that needs relay
        dial_tx.send(router_id.clone()).await.unwrap();

        futures::future::poll_fn(|cx| match manager.poll_unpin(cx) {
            Poll::Pending => Poll::Ready(()),
            Poll::Ready(_) => panic!("manager returned"),
        })
        .await;

        // verify all introducers are dialed
        for _ in 0..3 {
            let introducer_router_id = conn_rx.try_recv().unwrap();
            assert!(introducers.contains_key(&introducer_router_id));
        }

        // verify that all introducers are being dialed and that the router
        // is tracked in their dial context
        assert!(introducers.keys().all(|introducer_id| {
            match manager.pending_connections.get(introducer_id) {
                Some(routers) => routers.iter().any(|client| client == &router_id),
                None => false,
            }
        }));

        // verify that all introducers are in the pending introducer context
        assert!(manager
            .pending_introducers
            .get(&router_id)
            .unwrap()
            .pending_connections
            .iter()
            .all(|key| introducers.contains_key(key)));
        assert!(manager.pending_introducers.get(&router_id).unwrap().pending_queries.is_empty());

        // verify that the client router is pending
        assert!(manager.pending_connections.contains_key(&router_id));

        let mut introducers = introducers.into_iter().collect::<Vec<_>>();

        // send dial success for the first introducer
        let (introducer_router_id, _) = introducers.pop().unwrap();
        event_tx.send(Event::Success(introducer_router_id.clone())).await.unwrap();
        futures::future::poll_fn(|cx| match manager.poll_unpin(cx) {
            Poll::Pending => Poll::Ready(()),
            Poll::Ready(_) => panic!("manager returned"),
        })
        .await;

        // verify pending introducer context no longer exist since the client router was dialed
        assert!(!manager.pending_introducers.contains_key(&router_id));
        assert!(!manager.pending_connections.contains_key(&introducer_router_id));
        assert!(manager.routers.contains_key(&introducer_router_id));
        assert!(manager.pending_connections.contains_key(&router_id));

        // verify the client router is dialed
        assert_eq!(conn_rx.try_recv().unwrap(), router_id);

        // send dial succeses for the remaining introducers
        for (introducer_router_id, _) in introducers {
            event_tx.send(Event::Success(introducer_router_id.clone())).await.unwrap();
            futures::future::poll_fn(|cx| match manager.poll_unpin(cx) {
                Poll::Pending => Poll::Ready(()),
                Poll::Ready(_) => panic!("manager returned"),
            })
            .await;

            // verify the pending introducer context is still missing
            assert!(!manager.pending_introducers.contains_key(&router_id));

            // and that the introducer is no considered connected
            assert!(!manager.pending_connections.contains_key(&introducer_router_id));
            assert!(manager.routers.contains_key(&introducer_router_id));

            // and connection for the client router is still pending
            assert!(manager.pending_connections.contains_key(&router_id));

            // and that no more dial requests are sent
            assert!(conn_rx.try_recv().is_err());
        }
    }

    #[tokio::test]
    async fn dial_ssu2_introducer_one_dial_succeeds() {
        pub struct MockTransport {
            event_rx: tokio::sync::mpsc::Receiver<Event>,
            conn_tx: tokio::sync::mpsc::Sender<RouterId>,
        }

        enum Event {
            Success(RouterId),
        }

        impl Transport for MockTransport {
            fn connect(&mut self, router_info: RouterInfo) {
                self.conn_tx.try_send(router_info.identity.id()).unwrap();
            }
            fn accept(&mut self, _: &RouterId) {}
            fn reject(&mut self, _: &RouterId) {}
        }

        impl Stream for MockTransport {
            type Item = TransportEvent;

            fn poll_next(
                mut self: Pin<&mut Self>,
                cx: &mut Context<'_>,
            ) -> Poll<Option<Self::Item>> {
                match futures::ready!(self.event_rx.poll_recv(cx)).unwrap() {
                    Event::Success(router_id) =>
                        Poll::Ready(Some(TransportEvent::ConnectionEstablished {
                            address: "127.0.0.1:8888".parse().unwrap(),
                            direction: Direction::Outbound,
                            router_id,
                        })),
                }
            }
        }

        let introducers = (0..3)
            .map(|i| {
                let (introducer, _, sigkey) = RouterInfoBuilder::default()
                    .with_ssu2(Ssu2Config {
                        port: 9999,
                        host: Some("127.0.0.1".parse().unwrap()),
                        publish: true,
                        static_key: [0x11 + i; 32],
                        intro_key: [0x22 + i; 32],
                    })
                    .build();

                (
                    introducer.identity.id(),
                    RouterInfo::parse::<MockRuntime>(introducer.serialize(&sigkey)).unwrap(),
                )
            })
            .collect::<HashMap<_, _>>();

        let (mut router_info, router_id) = {
            let (router_info, _, sigkey) = RouterInfoBuilder::default()
                .with_ssu2(Ssu2Config {
                    port: 10000,
                    host: None,
                    publish: false,
                    static_key: [0x33; 32],
                    intro_key: [0x44; 32],
                })
                .build();

            (
                RouterInfo::parse::<MockRuntime>(router_info.serialize(&sigkey)).unwrap(),
                router_info.identity.id(),
            )
        };

        // add introducer for `router_info`
        match router_info.addresses.get_mut(0).unwrap() {
            RouterAddress::Ssu2 {
                introducers: router_introducers,
                ..
            } =>
                for (i, (introducer_router_id, _)) in introducers.iter().enumerate() {
                    router_introducers.push((introducer_router_id.clone(), 1337 + i as u32));
                },
            _ => panic!("expected ssu2"),
        }

        let (event_tx, event_rx) = tokio::sync::mpsc::channel(16);
        let (conn_tx, mut conn_rx) = tokio::sync::mpsc::channel(16);

        let mut builder = TestContextBuilder::default()
            .with_router(router_info)
            .with_ssu2(Box::new(MockTransport { event_rx, conn_tx }));

        // add all introducers to profile storage
        for (_, introducer) in &introducers {
            builder = builder.with_router(introducer.clone());
        }

        let (mut manager, _netdb_rx, dial_tx, _subsys_rx) = builder.build();

        // send dial request for router that needs relay
        dial_tx.send(router_id.clone()).await.unwrap();

        futures::future::poll_fn(|cx| match manager.poll_unpin(cx) {
            Poll::Pending => Poll::Ready(()),
            Poll::Ready(_) => panic!("manager returned"),
        })
        .await;

        // verify all introducers are dialed
        for _ in 0..3 {
            let introducer_router_id = conn_rx.try_recv().unwrap();
            assert!(introducers.contains_key(&introducer_router_id));
        }

        // verify that all introducers are being dialed and that the router
        // is tracked in their dial context
        assert!(introducers.keys().all(|introducer_id| {
            match manager.pending_connections.get(introducer_id) {
                Some(routers) => routers.iter().any(|client| client == &router_id),
                None => false,
            }
        }));

        // verify that all introducers are in the pending introducer context
        assert!(manager
            .pending_introducers
            .get(&router_id)
            .unwrap()
            .pending_connections
            .iter()
            .all(|key| introducers.contains_key(key)));
        assert!(manager.pending_introducers.get(&router_id).unwrap().pending_queries.is_empty());

        // verify that the client router is pending
        assert!(manager.pending_connections.contains_key(&router_id));

        let mut introducers = introducers.into_iter().collect::<Vec<_>>();

        // send dial success for the first introducer
        let (introducer_router_id, _) = introducers.pop().unwrap();
        event_tx.send(Event::Success(introducer_router_id.clone())).await.unwrap();
        futures::future::poll_fn(|cx| match manager.poll_unpin(cx) {
            Poll::Pending => Poll::Ready(()),
            Poll::Ready(_) => panic!("manager returned"),
        })
        .await;

        // verify pending introducer context no longer exist since the client router was dialed
        assert!(!manager.pending_introducers.contains_key(&router_id));
        assert!(!manager.pending_connections.contains_key(&introducer_router_id));
        assert!(manager.routers.contains_key(&introducer_router_id));
        assert!(manager.pending_connections.contains_key(&router_id));

        // verify the client router is dialed
        assert_eq!(conn_rx.try_recv().unwrap(), router_id);

        // send dial succeses for the remaining introducers
        for (introducer_router_id, _) in introducers {
            event_tx.send(Event::Success(introducer_router_id.clone())).await.unwrap();
            futures::future::poll_fn(|cx| match manager.poll_unpin(cx) {
                Poll::Pending => Poll::Ready(()),
                Poll::Ready(_) => panic!("manager returned"),
            })
            .await;

            // verify the pending introducer context is still missing
            assert!(!manager.pending_introducers.contains_key(&router_id));

            // and that the introducer is no considered connected
            assert!(!manager.pending_connections.contains_key(&introducer_router_id));
            assert!(manager.routers.contains_key(&introducer_router_id));

            // and connection for the client router is still pending
            assert!(manager.pending_connections.contains_key(&router_id));

            // and that no more dial requests are sent
            assert!(conn_rx.try_recv().is_err());
        }
    }

    #[tokio::test]
    async fn dial_ssu2_all_queries_fail() {
        pub struct MockTransport {
            conn_tx: tokio::sync::mpsc::Sender<RouterId>,
        }

        impl Transport for MockTransport {
            fn connect(&mut self, router_info: RouterInfo) {
                self.conn_tx.try_send(router_info.identity.id()).unwrap();
            }
            fn accept(&mut self, _: &RouterId) {}
            fn reject(&mut self, _: &RouterId) {}
        }

        impl Stream for MockTransport {
            type Item = TransportEvent;

            fn poll_next(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Option<Self::Item>> {
                Poll::Pending
            }
        }

        let mut introducers = (0..3)
            .map(|i| {
                let (introducer, _, sigkey) = RouterInfoBuilder::default()
                    .with_ssu2(Ssu2Config {
                        port: 9999,
                        host: Some("127.0.0.1".parse().unwrap()),
                        publish: true,
                        static_key: [0x11 + i; 32],
                        intro_key: [0x22 + i; 32],
                    })
                    .build();

                (
                    introducer.identity.id(),
                    RouterInfo::parse::<MockRuntime>(introducer.serialize(&sigkey)).unwrap(),
                )
            })
            .collect::<HashMap<_, _>>();

        let (mut router_info, router_id) = {
            let (router_info, _, sigkey) = RouterInfoBuilder::default()
                .with_ssu2(Ssu2Config {
                    port: 10000,
                    host: None,
                    publish: false,
                    static_key: [0x33; 32],
                    intro_key: [0x44; 32],
                })
                .build();

            (
                RouterInfo::parse::<MockRuntime>(router_info.serialize(&sigkey)).unwrap(),
                router_info.identity.id(),
            )
        };

        // add introducer for `router_info`
        match router_info.addresses.get_mut(0).unwrap() {
            RouterAddress::Ssu2 {
                introducers: router_introducers,
                ..
            } =>
                for (i, (introducer_router_id, _)) in introducers.iter().enumerate() {
                    router_introducers.push((introducer_router_id.clone(), 1337 + i as u32));
                },
            _ => panic!("expected ssu2"),
        }

        let (conn_tx, _conn_rx) = tokio::sync::mpsc::channel(16);

        let (mut manager, netdb_rx, dial_tx, subsys_rx) = TestContextBuilder::default()
            .with_router(router_info)
            .with_ssu2(Box::new(MockTransport { conn_tx }))
            .build();

        // send dial request for router that needs relay
        dial_tx.send(router_id.clone()).await.unwrap();

        futures::future::poll_fn(|cx| match manager.poll_unpin(cx) {
            Poll::Pending => Poll::Ready(()),
            Poll::Ready(_) => panic!("manager returned"),
        })
        .await;

        // verify all introducers are in the pending query context
        assert!(manager
            .pending_introducers
            .get(&router_id)
            .unwrap()
            .pending_queries
            .iter()
            .all(|key| introducers.contains_key(key)));
        assert!(manager
            .pending_introducers
            .get(&router_id)
            .unwrap()
            .pending_connections
            .is_empty());
        assert!(introducers.keys().all(|key| manager.pending_connections.contains_key(key)));
        assert!(introducers.keys().all(|key| manager.pending_queries.contains(key)));

        // verify that all introducers are being dialed and that the router
        // is tracked in their dial context
        assert!(introducers.keys().all(|introducer_id| {
            match manager.pending_connections.get(introducer_id) {
                Some(routers) => routers.iter().any(|client| client == &router_id),
                None => false,
            }
        }));

        // send router info query failure for the first two introducers
        for _ in 0..2 {
            let introducer = match timeout!(netdb_rx.recv()).await.unwrap().unwrap() {
                NetDbAction::QueryRouterInfo {
                    tx,
                    router_id: target,
                } => {
                    tx.send(Err(QueryError::Timeout)).unwrap();
                    target
                }
                _ => panic!("unexpected netdb action"),
            };

            let future = {
                futures::future::poll_fn(|cx| loop {
                    match manager.poll_unpin(cx) {
                        Poll::Pending =>
                            if !manager.pending_queries.contains(&introducer) {
                                return Poll::Ready(());
                            } else {
                                return Poll::Pending;
                            },
                        Poll::Ready(_) => panic!("manager returned"),
                    }
                })
            };
            let _: () = timeout!(future).await.unwrap();

            // verify the introducer is no longer tracked in any context
            assert!(!manager
                .pending_introducers
                .get(&router_id)
                .unwrap()
                .pending_queries
                .contains(&introducer));
            assert!(!manager
                .pending_introducers
                .get(&router_id)
                .unwrap()
                .pending_connections
                .contains(&introducer));
            assert!(!manager.pending_connections.contains_key(&introducer));
            assert!(!manager.pending_queries.contains(&introducer));

            // verify dial failure is reported to the subsystem manager
            match timeout!(subsys_rx.recv()).await.unwrap().unwrap() {
                SubsystemEvent::ConnectionFailure { router_id: remote } =>
                    assert_eq!(remote, introducer),
                _ => panic!("invalid event"),
            }
            introducers.remove(&introducer);
        }

        // send router info query failure for the last introducer
        let introducer = match timeout!(netdb_rx.recv()).await.unwrap().unwrap() {
            NetDbAction::QueryRouterInfo {
                tx,
                router_id: target,
            } => {
                tx.send(Err(QueryError::Timeout)).unwrap();
                target
            }
            _ => panic!("unexpected netdb action"),
        };

        let future = {
            futures::future::poll_fn(|cx| loop {
                match manager.poll_unpin(cx) {
                    Poll::Pending =>
                        if !manager.pending_queries.contains(&introducer) {
                            return Poll::Ready(());
                        } else {
                            return Poll::Pending;
                        },
                    Poll::Ready(_) => panic!("manager returned"),
                }
            })
        };
        let _: () = timeout!(future).await.unwrap();

        // verify that there is no pending context for the introducer or the router
        assert!(!manager.pending_introducers.contains_key(&router_id));
        assert!(!manager.pending_connections.contains_key(&introducer));
        assert!(!manager.pending_queries.contains(&introducer));
        assert!(!manager.pending_connections.contains_key(&router_id));
        assert!(!manager.pending_queries.contains(&router_id));

        match timeout!(subsys_rx.recv()).await.unwrap().unwrap() {
            SubsystemEvent::ConnectionFailure { router_id: remote } =>
                assert_eq!(remote, router_id),
            _ => panic!("invalid event"),
        }

        match timeout!(subsys_rx.recv()).await.unwrap().unwrap() {
            SubsystemEvent::ConnectionFailure { router_id: remote } =>
                assert!(introducers.remove(&remote).is_some()),
            _ => panic!("invalid event"),
        }
    }

    #[tokio::test]
    async fn dial_ssu2_all_queries_succeed_all_dials_fail() {
        pub struct MockTransport {
            event_rx: tokio::sync::mpsc::Receiver<Event>,
            conn_tx: tokio::sync::mpsc::Sender<RouterId>,
        }

        enum Event {
            Failure(RouterId),
        }

        impl Transport for MockTransport {
            fn connect(&mut self, router_info: RouterInfo) {
                self.conn_tx.try_send(router_info.identity.id()).unwrap();
            }
            fn accept(&mut self, _: &RouterId) {}
            fn reject(&mut self, _: &RouterId) {}
        }

        impl Stream for MockTransport {
            type Item = TransportEvent;

            fn poll_next(
                mut self: Pin<&mut Self>,
                cx: &mut Context<'_>,
            ) -> Poll<Option<Self::Item>> {
                match futures::ready!(self.event_rx.poll_recv(cx)).unwrap() {
                    Event::Failure(router_id) =>
                        Poll::Ready(Some(TransportEvent::ConnectionFailure { router_id })),
                }
            }
        }

        let introducers = (0..3)
            .map(|i| {
                let (introducer, _, sigkey) = RouterInfoBuilder::default()
                    .with_ssu2(Ssu2Config {
                        port: 9999,
                        host: Some("127.0.0.1".parse().unwrap()),
                        publish: true,
                        static_key: [0x11 + i; 32],
                        intro_key: [0x22 + i; 32],
                    })
                    .build();

                (
                    introducer.identity.id(),
                    RouterInfo::parse::<MockRuntime>(introducer.serialize(&sigkey)).unwrap(),
                )
            })
            .collect::<HashMap<_, _>>();

        let (mut router_info, router_id) = {
            let (router_info, _, sigkey) = RouterInfoBuilder::default()
                .with_ssu2(Ssu2Config {
                    port: 10000,
                    host: None,
                    publish: false,
                    static_key: [0x33; 32],
                    intro_key: [0x44; 32],
                })
                .build();

            (
                RouterInfo::parse::<MockRuntime>(router_info.serialize(&sigkey)).unwrap(),
                router_info.identity.id(),
            )
        };

        // add introducer for `router_info`
        match router_info.addresses.get_mut(0).unwrap() {
            RouterAddress::Ssu2 {
                introducers: router_introducers,
                ..
            } =>
                for (i, (introducer_router_id, _)) in introducers.iter().enumerate() {
                    router_introducers.push((introducer_router_id.clone(), 1337 + i as u32));
                },
            _ => panic!("expected ssu2"),
        }

        let (event_tx, event_rx) = tokio::sync::mpsc::channel(16);
        let (conn_tx, mut conn_rx) = tokio::sync::mpsc::channel(16);

        let (mut manager, netdb_rx, dial_tx, subsys_rx) = TestContextBuilder::default()
            .with_router(router_info)
            .with_ssu2(Box::new(MockTransport { event_rx, conn_tx }))
            .build();

        let storage = manager.router_ctx.profile_storage().clone();

        // send dial request for router that needs relay
        dial_tx.send(router_id.clone()).await.unwrap();

        futures::future::poll_fn(|cx| match manager.poll_unpin(cx) {
            Poll::Pending => Poll::Ready(()),
            Poll::Ready(_) => panic!("manager returned"),
        })
        .await;

        // verify all introducers are in the pending query context
        assert!(manager
            .pending_introducers
            .get(&router_id)
            .unwrap()
            .pending_queries
            .iter()
            .all(|key| introducers.contains_key(key)));
        assert!(manager
            .pending_introducers
            .get(&router_id)
            .unwrap()
            .pending_connections
            .is_empty());
        assert!(introducers.keys().all(|key| manager.pending_connections.contains_key(key)));
        assert!(introducers.keys().all(|key| manager.pending_queries.contains(key)));

        // verify that all introducers are being dialed and that the router
        // is tracked in their dial context
        assert!(introducers.keys().all(|introducer_id| {
            match manager.pending_connections.get(introducer_id) {
                Some(routers) => routers.iter().any(|client| client == &router_id),
                None => false,
            }
        }));

        // send query successes for all queries
        for _ in 0..3 {
            match timeout!(netdb_rx.recv()).await.unwrap().unwrap() {
                NetDbAction::QueryRouterInfo {
                    tx,
                    router_id: target,
                } => {
                    tx.send(Ok(())).unwrap();
                    storage.add_router(introducers.get(&target).unwrap().clone());
                }
                _ => panic!("unexpected netdb action"),
            }
        }

        let future = {
            futures::future::poll_fn(|cx| loop {
                match manager.poll_unpin(cx) {
                    Poll::Pending =>
                        if manager.pending_queries.is_empty() {
                            return Poll::Ready(());
                        } else {
                            return Poll::Pending;
                        },
                    Poll::Ready(_) => panic!("manager returned"),
                }
            })
        };
        let _: () = timeout!(future).await.unwrap();

        // verify that all introducers have transitioned to pending
        assert!(manager
            .pending_introducers
            .get(&router_id)
            .unwrap()
            .pending_connections
            .iter()
            .all(|key| introducers.contains_key(key)));
        assert!(manager.pending_introducers.get(&router_id).unwrap().pending_queries.is_empty());

        // send dial failures for each of the introducers
        for _ in 0..3 {
            let introducer = conn_rx.try_recv().unwrap();
            event_tx.send(Event::Failure(introducer)).await.unwrap();
        }

        futures::future::poll_fn(|cx| match manager.poll_unpin(cx) {
            Poll::Pending => Poll::Ready(()),
            Poll::Ready(_) => panic!("manager returned"),
        })
        .await;

        // verify that there is no pending context for the introducer or the router
        assert!(!manager.pending_introducers.contains_key(&router_id));
        assert!(!manager.pending_queries.contains(&router_id));
        assert!(manager.pending_connections.is_empty());

        match timeout!(subsys_rx.recv()).await.unwrap().unwrap() {
            SubsystemEvent::ConnectionFailure { router_id: remote } =>
                assert_eq!(remote, router_id),
            _ => panic!("invalid event"),
        }
    }

    #[tokio::test]
    async fn dial_ssu2_all_queries_succeed_all_dials_succeed() {
        pub struct MockTransport {
            event_rx: tokio::sync::mpsc::Receiver<Event>,
            conn_tx: tokio::sync::mpsc::Sender<RouterId>,
        }

        enum Event {
            Success(RouterId),
        }

        impl Transport for MockTransport {
            fn connect(&mut self, router_info: RouterInfo) {
                self.conn_tx.try_send(router_info.identity.id()).unwrap();
            }
            fn accept(&mut self, _: &RouterId) {}
            fn reject(&mut self, _: &RouterId) {}
        }

        impl Stream for MockTransport {
            type Item = TransportEvent;

            fn poll_next(
                mut self: Pin<&mut Self>,
                cx: &mut Context<'_>,
            ) -> Poll<Option<Self::Item>> {
                match futures::ready!(self.event_rx.poll_recv(cx)).unwrap() {
                    Event::Success(router_id) =>
                        Poll::Ready(Some(TransportEvent::ConnectionEstablished {
                            address: "127.0.0.1:8888".parse().unwrap(),
                            direction: Direction::Outbound,
                            router_id,
                        })),
                }
            }
        }

        let introducers = (0..3)
            .map(|i| {
                let (introducer, _, sigkey) = RouterInfoBuilder::default()
                    .with_ssu2(Ssu2Config {
                        port: 9999,
                        host: Some("127.0.0.1".parse().unwrap()),
                        publish: true,
                        static_key: [0x11 + i; 32],
                        intro_key: [0x22 + i; 32],
                    })
                    .build();

                (
                    introducer.identity.id(),
                    RouterInfo::parse::<MockRuntime>(introducer.serialize(&sigkey)).unwrap(),
                )
            })
            .collect::<HashMap<_, _>>();

        let (mut router_info, router_id) = {
            let (router_info, _, sigkey) = RouterInfoBuilder::default()
                .with_ssu2(Ssu2Config {
                    port: 10000,
                    host: None,
                    publish: false,
                    static_key: [0x33; 32],
                    intro_key: [0x44; 32],
                })
                .build();

            (
                RouterInfo::parse::<MockRuntime>(router_info.serialize(&sigkey)).unwrap(),
                router_info.identity.id(),
            )
        };

        // add introducer for `router_info`
        match router_info.addresses.get_mut(0).unwrap() {
            RouterAddress::Ssu2 {
                introducers: router_introducers,
                ..
            } =>
                for (i, (introducer_router_id, _)) in introducers.iter().enumerate() {
                    router_introducers.push((introducer_router_id.clone(), 1337 + i as u32));
                },
            _ => panic!("expected ssu2"),
        }

        let (event_tx, event_rx) = tokio::sync::mpsc::channel(16);
        let (conn_tx, mut conn_rx) = tokio::sync::mpsc::channel(16);

        let (mut manager, netdb_rx, dial_tx, _subsys_rx) = TestContextBuilder::default()
            .with_router(router_info)
            .with_ssu2(Box::new(MockTransport { event_rx, conn_tx }))
            .build();

        let storage = manager.router_ctx.profile_storage().clone();

        // send dial request for router that needs relay
        dial_tx.send(router_id.clone()).await.unwrap();

        futures::future::poll_fn(|cx| match manager.poll_unpin(cx) {
            Poll::Pending => Poll::Ready(()),
            Poll::Ready(_) => panic!("manager returned"),
        })
        .await;

        // verify all introducers are in the pending query context
        assert!(manager
            .pending_introducers
            .get(&router_id)
            .unwrap()
            .pending_queries
            .iter()
            .all(|key| introducers.contains_key(key)));
        assert!(manager
            .pending_introducers
            .get(&router_id)
            .unwrap()
            .pending_connections
            .is_empty());
        assert!(introducers.keys().all(|key| manager.pending_connections.contains_key(key)));
        assert!(introducers.keys().all(|key| manager.pending_queries.contains(key)));

        // verify that all introducers are being dialed and that the router
        // is tracked in their dial context
        assert!(introducers.keys().all(|introducer_id| {
            match manager.pending_connections.get(introducer_id) {
                Some(routers) => routers.iter().any(|client| client == &router_id),
                None => false,
            }
        }));

        // send query successes for all queries
        for _ in 0..3 {
            match timeout!(netdb_rx.recv()).await.unwrap().unwrap() {
                NetDbAction::QueryRouterInfo {
                    tx,
                    router_id: target,
                } => {
                    tx.send(Ok(())).unwrap();
                    storage.add_router(introducers.get(&target).unwrap().clone());
                }
                _ => panic!("unexpected netdb action"),
            }
        }

        let future = {
            futures::future::poll_fn(|cx| loop {
                match manager.poll_unpin(cx) {
                    Poll::Pending =>
                        if manager.pending_queries.is_empty() {
                            return Poll::Ready(());
                        } else {
                            return Poll::Pending;
                        },
                    Poll::Ready(_) => panic!("manager returned"),
                }
            })
        };
        let _: () = timeout!(future).await.unwrap();

        // verify that all introducers have transitioned to pending
        assert!(manager
            .pending_introducers
            .get(&router_id)
            .unwrap()
            .pending_connections
            .iter()
            .all(|key| introducers.contains_key(key)));
        assert!(manager.pending_introducers.get(&router_id).unwrap().pending_queries.is_empty());

        // send dial success for each of the introducers
        for _ in 0..3 {
            let introducer = conn_rx.try_recv().unwrap();
            event_tx.send(Event::Success(introducer)).await.unwrap();
        }

        futures::future::poll_fn(|cx| match manager.poll_unpin(cx) {
            Poll::Pending => Poll::Ready(()),
            Poll::Ready(_) => panic!("manager returned"),
        })
        .await;

        assert!(introducers.keys().all(|key| manager.routers.contains_key(key)));
        assert_eq!(manager.pending_connections.len(), 1);
        assert!(manager.pending_connections.contains_key(&router_id));
        assert!(manager.pending_introducers.is_empty());
        assert!(manager.pending_queries.is_empty());

        // verify the client router is dialed
        assert_eq!(conn_rx.try_recv().unwrap(), router_id);
    }

    struct NoopTransport {}

    impl Transport for NoopTransport {
        fn connect(&mut self, _: RouterInfo) {}
        fn accept(&mut self, _: &RouterId) {}
        fn reject(&mut self, _: &RouterId) {}
    }

    impl Stream for NoopTransport {
        type Item = TransportEvent;

        fn poll_next(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Option<Self::Item>> {
            Poll::Pending
        }
    }

    #[tokio::test]
    async fn publish_router_info_transit_tunnels_disabled_reachable() {
        let (router_info, ..) = RouterInfoBuilder::default().build();
        let (mut manager, netdb_rx, _dial_tx, _subsys_rx) = TestContextBuilder::default()
            .with_router(router_info)
            .with_ntcp2(Box::new(NoopTransport {}))
            .with_transit_tunnels_disabled()
            .build();

        manager.on_firewall_status(FirewallStatus::Ok);
        manager.publish_router_info();

        match netdb_rx.try_recv().unwrap() {
            NetDbAction::PublishRouterInfo { router_info, .. } => {
                let router_info = RouterInfo::parse::<MockRuntime>(&router_info).unwrap();
                assert_eq!(
                    router_info.options.get(&Str::from("caps")),
                    Some(&Str::from("LGR"))
                );
            }
            _ => panic!("unexpected action"),
        }
    }

    #[tokio::test]
    async fn publish_router_info_transit_tunnels_disabled_unreachable() {
        let (router_info, ..) = RouterInfoBuilder::default().build();
        let (mut manager, netdb_rx, _dial_tx, _subsys_rx) = TestContextBuilder::default()
            .with_router(router_info)
            .with_ntcp2(Box::new(NoopTransport {}))
            .with_transit_tunnels_disabled()
            .build();

        manager.on_firewall_status(FirewallStatus::SymmetricNat);
        manager.publish_router_info();

        match netdb_rx.try_recv().unwrap() {
            NetDbAction::PublishRouterInfo { router_info, .. } => {
                let router_info = RouterInfo::parse::<MockRuntime>(&router_info).unwrap();
                assert_eq!(
                    router_info.options.get(&Str::from("caps")),
                    Some(&Str::from("LGU"))
                );
            }
            _ => panic!("unexpected action"),
        }
    }

    #[tokio::test]
    async fn publish_router_info_transit_tunnels_disabled_unknown_status() {
        let (router_info, ..) = RouterInfoBuilder::default().build();
        let (mut manager, netdb_rx, _dial_tx, _subsys_rx) = TestContextBuilder::default()
            .with_router(router_info)
            .with_ntcp2(Box::new(NoopTransport {}))
            .with_transit_tunnels_disabled()
            .build();

        manager.publish_router_info();

        match netdb_rx.try_recv().unwrap() {
            NetDbAction::PublishRouterInfo { router_info, .. } => {
                let router_info = RouterInfo::parse::<MockRuntime>(&router_info).unwrap();
                assert_eq!(
                    router_info.options.get(&Str::from("caps")),
                    Some(&Str::from("LG"))
                );
            }
            _ => panic!("unexpected action"),
        }
    }

    #[tokio::test]
    async fn publish_router_info_unknown_firewall_status() {
        let (router_info, ..) = RouterInfoBuilder::default().build();
        let (mut manager, netdb_rx, _dial_tx, _subsys_rx) = TestContextBuilder::default()
            .with_router(router_info)
            .with_ntcp2(Box::new(NoopTransport {}))
            .build();

        manager.publish_router_info();

        match netdb_rx.try_recv().unwrap() {
            NetDbAction::PublishRouterInfo { router_info, .. } => {
                let router_info = RouterInfo::parse::<MockRuntime>(&router_info).unwrap();
                assert_eq!(
                    router_info.options.get(&Str::from("caps")),
                    Some(&Str::from("L"))
                );
            }
            _ => panic!("unexpected action"),
        }
    }

    #[tokio::test]
    async fn publish_router_info_symnat() {
        let (router_info, ..) = RouterInfoBuilder::default().build();
        let (mut manager, netdb_rx, _dial_tx, _subsys_rx) = TestContextBuilder::default()
            .with_router(router_info)
            .with_ntcp2(Box::new(NoopTransport {}))
            .build();

        manager.on_firewall_status(FirewallStatus::SymmetricNat);
        manager.publish_router_info();

        match netdb_rx.try_recv().unwrap() {
            NetDbAction::PublishRouterInfo { router_info, .. } => {
                let router_info = RouterInfo::parse::<MockRuntime>(&router_info).unwrap();
                assert_eq!(
                    router_info.options.get(&Str::from("caps")),
                    Some(&Str::from("LU"))
                );
            }
            _ => panic!("unexpected action"),
        }
    }

    #[tokio::test]
    async fn publish_router_info_firewalled() {
        let (router_info, ..) = RouterInfoBuilder::default().build();
        let (mut manager, netdb_rx, _dial_tx, _subsys_rx) = TestContextBuilder::default()
            .with_router(router_info)
            .with_ntcp2(Box::new(NoopTransport {}))
            .build();

        manager.on_firewall_status(FirewallStatus::Firewalled);
        manager.publish_router_info();

        match netdb_rx.try_recv().unwrap() {
            NetDbAction::PublishRouterInfo { router_info, .. } => {
                let router_info = RouterInfo::parse::<MockRuntime>(&router_info).unwrap();
                assert_eq!(
                    router_info.options.get(&Str::from("caps")),
                    Some(&Str::from("LU"))
                );
            }
            _ => panic!("unexpected action"),
        }
    }

    #[tokio::test]
    async fn publish_router_info_not_firewalled() {
        let (router_info, ..) = RouterInfoBuilder::default().build();
        let (mut manager, netdb_rx, _dial_tx, _subsys_rx) = TestContextBuilder::default()
            .with_router(router_info)
            .with_ntcp2(Box::new(NoopTransport {}))
            .build();

        manager.on_firewall_status(FirewallStatus::Ok);
        manager.publish_router_info();

        match netdb_rx.try_recv().unwrap() {
            NetDbAction::PublishRouterInfo { router_info, .. } => {
                let router_info = RouterInfo::parse::<MockRuntime>(&router_info).unwrap();
                assert_eq!(
                    router_info.options.get(&Str::from("caps")),
                    Some(&Str::from("LR"))
                );
            }
            _ => panic!("unexpected action"),
        }
    }

    #[tokio::test]
    async fn publish_router_info_overriden_capabilities() {
        let (router_info, ..) = RouterInfoBuilder::default().build();
        let (mut manager, netdb_rx, _dial_tx, _subsys_rx) = TestContextBuilder::default()
            .with_router(router_info)
            .with_ntcp2(Box::new(NoopTransport {}))
            .with_transit_tunnels_disabled()
            .with_caps("XfR".to_string())
            .build();

        manager.on_firewall_status(FirewallStatus::SymmetricNat);
        manager.publish_router_info();

        match netdb_rx.try_recv().unwrap() {
            NetDbAction::PublishRouterInfo { router_info, .. } => {
                let router_info = RouterInfo::parse::<MockRuntime>(&router_info).unwrap();

                // caps should be `LGU` but they were overriden by user
                assert_eq!(
                    router_info.options.get(&Str::from("caps")),
                    Some(&Str::from("XfR"))
                );
            }
            _ => panic!("unexpected action"),
        }
    }
}
