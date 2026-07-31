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

//! SAMV3 server implementation.
//!
//! https://geti2p.net/en/docs/api/samv3

use crate::{
    crypto::{base32_decode, base32_encode, base64_encode},
    error::{ChannelError, ConnectionError, Error},
    events::EventHandle,
    netdb::NetDbHandle,
    primitives::{DestinationId, Mapping, Str},
    profile::ProfileStorage,
    runtime::{AddressBook, JoinSet, Runtime, TcpListener, UdpSocket as _},
    sam::{
        parser::{HostKind, SessionKind},
        pending::{
            connection::{ConnectionKind, PendingSamConnection},
            session::{PendingSamSession, SamSessionContext},
        },
        session::SamSession,
        types::{SamSessionCommand, SamSessionCommandRecycle},
    },
    tunnel::{TunnelManagerHandle, TunnelPoolConfig},
    util::udp::{UdpSocket, UdpSocketHandle},
};

use futures::{Stream, StreamExt};
use hashbrown::{HashMap, HashSet};
use thingbuf::mpsc::{channel, with_recycle, Receiver, Sender};

use alloc::{
    boxed::Box,
    collections::BTreeMap,
    format,
    string::{String, ToString},
    sync::Arc,
    vec::Vec,
};
use core::{
    future::Future,
    mem,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    pin::Pin,
    task::{Context, Poll},
};

#[cfg(feature = "std")]
use parking_lot::RwLock;
#[cfg(not(feature = "std"))]
use spin::rwlock::RwLock;

mod parser;
mod pending;
mod protocol;
mod session;
mod socket;
mod types;

#[cfg(not(feature = "fuzz"))]
use parser::Datagram;
#[cfg(feature = "fuzz")]
pub use {
    parser::{Datagram, SamCommand},
    protocol::streaming::Packet,
};

/// Logging target for the file.
const LOG_TARGET: &str = "emissary::sam";

/// SAMv3 command channel size.
const COMMAND_CHANNEL_SIZE: usize = 256;

/// Maximum number of active SAM sessions exposed to I2PControl.
pub const SAM_SESSION_OBSERVATION_LIMIT: usize = 1000;

/// Maximum number of sockets retained for one observed SAM session.
pub const SAM_SOCKET_OBSERVATION_LIMIT: usize = 8;

/// Error returned when the bounded SAM observation state can no longer represent reality.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SamSessionObservationError {
    /// The bounded state overflowed or received incomplete socket metadata.
    Overflow,
}

/// A socket in a SAM session observation snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SamObservedSocket {
    /// i2pd-compatible SAM socket type: session, stream, or acceptor.
    pub socket_type: u8,

    /// Remote TCP peer address.
    pub peer: Arc<str>,
}

/// A SAM session in an observation snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SamObservedSession {
    /// i2pd-compatible destination nickname.
    pub name: Arc<str>,

    /// i2pd-compatible `.b32.i2p` destination address.
    pub address: Arc<str>,

    /// Active SAM sockets belonging to this session.
    pub sockets: Vec<SamObservedSocket>,
}

/// Bounded, read-only SAM observation snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SamSessionObservationSnapshot {
    /// Active sessions keyed by their SAM session identifier.
    pub sessions: BTreeMap<Arc<str>, SamObservedSession>,

    /// Monotonic publication generation.
    pub generation: u64,
}

#[derive(Clone)]
pub struct SamSessionObservationHandle {
    state: Arc<RwLock<SamSessionObservationState>>,
}

#[derive(Clone)]
pub(crate) struct SamSessionObservationPublisher {
    state: Arc<RwLock<SamSessionObservationState>>,
}

struct SamSessionObservationState {
    sessions: BTreeMap<Arc<str>, SamObservedSessionState>,
    generation: u64,
    overflowed: bool,
}

struct SamObservedSessionState {
    name: Arc<str>,
    address: Arc<str>,
    sockets: BTreeMap<u64, SamObservedSocket>,
}

impl SamSessionObservationHandle {
    /// Read a bounded snapshot without holding the lock across any await point.
    pub fn snapshot(&self) -> Result<SamSessionObservationSnapshot, SamSessionObservationError> {
        let state = self.state.read();
        if state.overflowed {
            return Err(SamSessionObservationError::Overflow);
        }

        let sessions = state
            .sessions
            .iter()
            .map(|(session_id, session)| {
                (
                    Arc::clone(session_id),
                    SamObservedSession {
                        name: Arc::clone(&session.name),
                        address: Arc::clone(&session.address),
                        sockets: session.sockets.values().cloned().collect(),
                    },
                )
            })
            .collect();

        Ok(SamSessionObservationSnapshot {
            sessions,
            generation: state.generation,
        })
    }

    /// Construct an empty bounded source for test-only I2PControl state.
    #[doc(hidden)]
    pub fn empty_for_test() -> Self {
        let (_, handle) = SamSessionObservationPublisher::new();
        handle
    }
}

impl SamSessionObservationPublisher {
    fn new() -> (Self, SamSessionObservationHandle) {
        let state = Arc::new(RwLock::new(SamSessionObservationState {
            sessions: BTreeMap::new(),
            generation: 0,
            overflowed: false,
        }));

        (
            Self {
                state: Arc::clone(&state),
            },
            SamSessionObservationHandle { state },
        )
    }

    fn activate_session(
        &self,
        session_id: &Arc<str>,
        destination_id: &DestinationId,
        options: &HashMap<String, String>,
        socket_id: u64,
        peer: Option<SocketAddr>,
    ) -> Result<(), SamSessionObservationError> {
        let Some(peer) = peer else {
            return self.overflow();
        };
        let mut state = self.state.write();
        if state.sessions.len() >= SAM_SESSION_OBSERVATION_LIMIT
            || state.sessions.contains_key(session_id)
        {
            state.overflowed = true;
            return Err(SamSessionObservationError::Overflow);
        }

        let mut sockets = BTreeMap::new();
        sockets.insert(
            socket_id,
            SamObservedSocket {
                socket_type: 1,
                peer: Arc::from(peer.to_string()),
            },
        );
        state.sessions.insert(
            Arc::clone(session_id),
            SamObservedSessionState {
                name: Arc::from(
                    options
                        .get("inbound.nickname")
                        .or_else(|| options.get("outbound.nickname"))
                        .cloned()
                        .unwrap_or_else(|| {
                            base64_encode(destination_id.to_vec()).chars().take(4).collect()
                        }),
                ),
                address: Arc::from(format!(
                    "{}.b32.i2p",
                    base32_encode(destination_id.to_vec())
                )),
                sockets,
            },
        );
        state.generation = state.generation.wrapping_add(1);
        Ok(())
    }

    fn add_socket(
        &self,
        session_id: &Arc<str>,
        socket_id: u64,
        socket_type: u8,
        peer: Option<SocketAddr>,
    ) -> Result<(), SamSessionObservationError> {
        let Some(peer) = peer else {
            return self.overflow();
        };
        let mut state = self.state.write();
        let Some(session) = state.sessions.get_mut(session_id) else {
            state.overflowed = true;
            return Err(SamSessionObservationError::Overflow);
        };
        if session.sockets.len() >= SAM_SOCKET_OBSERVATION_LIMIT {
            state.overflowed = true;
            return Err(SamSessionObservationError::Overflow);
        }
        session.sockets.insert(
            socket_id,
            SamObservedSocket {
                socket_type,
                peer: Arc::from(peer.to_string()),
            },
        );
        state.generation = state.generation.wrapping_add(1);
        Ok(())
    }

    fn remove_socket(&self, session_id: &Arc<str>, socket_id: u64) {
        let mut state = self.state.write();
        if let Some(session) = state.sessions.get_mut(session_id) {
            if session.sockets.remove(&socket_id).is_some() {
                state.generation = state.generation.wrapping_add(1);
            }
        }
    }

    fn remove_session(&self, session_id: &Arc<str>) {
        let mut state = self.state.write();
        if state.sessions.remove(session_id).is_some() {
            state.generation = state.generation.wrapping_add(1);
        }
    }

    fn overflow(&self) -> Result<(), SamSessionObservationError> {
        self.state.write().overflowed = true;
        Err(SamSessionObservationError::Overflow)
    }
}

/// Session context.
///
/// Holds either pending or active sessions.
pub struct SessionContext<R: Runtime, T: 'static + Send + Unpin> {
    /// Sesison futures.
    futures: R::JoinSet<T>,

    /// TX channels for the session.
    senders: HashMap<Arc<str>, Sender<SamSessionCommand<R>, SamSessionCommandRecycle>>,

    /// Active sub-session ID -> primary session ID mappings.
    ///
    /// This mapping must exist in [`SamServer`] because while the sub-session itself is created
    /// using the control socket of the primary session, the protocols (streams and datagrams) use
    /// SAMv3 ports to interact with the router and provide the session ID of the sub-session they
    /// belong to for identification.
    ///
    /// This requires bidirectional traffic between [`SamServer`] and [`SamSession`], allowing
    /// [`SamSession`] to add new sub-sessions for existing primary sessions which in turn allows
    /// [`SamServer`] to associate incoming protocol-related commands with a correct primary
    /// session.
    sub_sessions: HashMap<Arc<str>, Arc<str>>,
}

impl<R: Runtime, T: 'static + Send + Unpin> SessionContext<R, T> {
    /// Create new [`SessionContext`].
    fn new() -> Self {
        Self {
            futures: R::join_set(),
            senders: HashMap::new(),
            sub_sessions: HashMap::new(),
        }
    }

    /// Returns `true` if [`SessionContext`] contains a session identified by `key`.
    fn contains_key(&self, key: &Arc<str>) -> bool {
        self.senders.contains_key(key)
    }

    /// Remove the command channel from [`SessionContext`] for `key` if it exists
    fn remove(
        &mut self,
        key: &Arc<str>,
    ) -> Option<Sender<SamSessionCommand<R>, SamSessionCommandRecycle>> {
        // remove all sub-sessions of the primary session
        self.sub_sessions.retain(|_, primary_session_id| primary_session_id != key);
        self.senders.remove(key)
    }

    /// Insert new session identified by `session_id` in the [`SessionContext`].
    fn insert(
        &mut self,
        session_id: Arc<str>,
        tx: Sender<SamSessionCommand<R>, SamSessionCommandRecycle>,
        future: impl Future<Output = T> + 'static + Send,
    ) {
        self.senders.insert(session_id, tx);
        self.futures.push(future);
    }

    /// Add sub-session ID -> primary session ID mapping.
    fn insert_sub_session(&mut self, session_id: Arc<str>, sub_session_id: Arc<str>) {
        self.sub_sessions.insert(sub_session_id, session_id);
    }

    /// Remove sub-session mapping.
    fn remove_sub_session(&mut self, sub_session_id: &Arc<str>) {
        self.sub_sessions.remove(sub_session_id);
    }
}

impl<R: Runtime> SessionContext<R, Arc<str>> {
    /// Send `command` to an active session identified by `session_id`.
    fn send_command(
        &self,
        session_id: &Arc<str>,
        command: SamSessionCommand<R>,
    ) -> Result<(), ChannelError> {
        if let Some(session) = self.senders.get(session_id) {
            return session.try_send(command).map_err(From::from);
        }

        match self.sub_sessions.get(session_id) {
            None => Err(ChannelError::DoesntExist),
            Some(primary_session_id) => self
                .senders
                .get(primary_session_id)
                .ok_or(ChannelError::DoesntExist)?
                .try_send(command)
                .map_err(From::from),
        }
    }
}

impl<R: Runtime, T: 'static + Send + Unpin> Stream for SessionContext<R, T> {
    type Item = T;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.futures.poll_next_unpin(cx)
    }
}

/// Sub-session command.
#[derive(Default, Clone)]
enum SubSessionCommand {
    /// Associate sub-session with an active primary session.
    Add {
        /// Primary session ID.
        primary_session_id: Arc<str>,

        /// Sub-session ID.
        sub_session_id: Arc<str>,
    },

    /// Remove sub-session.
    #[allow(unused)]
    Remove {
        /// Sub-session ID.
        sub_session_id: Arc<str>,
    },

    /// Dummy event.
    #[default]
    Dummy,
}

/// Datagram write state.
enum DatagramWriterState {
    /// Get next message from the datagram channel.
    GetMessage,

    /// Write current message to socket.
    WriteMessage {
        /// Client address.
        target: SocketAddr,

        /// Datagram.
        datagram: Vec<u8>,
    },
}

/// SAMv3 server.
pub struct SamServer<R: Runtime> {
    /// Active destinations.
    active_destinations: HashSet<DestinationId>,

    /// Active SAMV3 sessions.
    active_sessions: SessionContext<R, Arc<str>>,

    /// Address book.
    address_book: Option<Arc<dyn AddressBook>>,

    /// RX channel for receiving datagrams that should be to clients.
    datagram_rx: Receiver<(u16, Vec<u8>)>,

    /// TX channel given to active sessions they can use to send datagrams to clients.
    datagram_tx: Sender<(u16, Vec<u8>)>,

    /// Datagra writer state.
    datagram_writer_state: DatagramWriterState,

    /// Event handle.
    event_handle: EventHandle<R>,

    /// TCP listener.
    listener: R::TcpListener,

    /// Metrics handle.
    #[allow(unused)]
    metrics: R::MetricsHandle,

    /// Handle to `NetDb`.
    netdb_handle: NetDbHandle,

    /// Pending inbound SAMv3 connections.
    ///
    /// Inbound connections which are in the state of being handshaked and reading a command from
    /// the client. After the command has been read, `SamServer` validates it against the current
    /// state, ensuring, e.g., that it's not a duplicate `SESSION CREATE` request.
    pending_inbound_connections: R::JoinSet<crate::Result<ConnectionKind<R>>>,

    /// Pending SAMv3 sessions that are in the process of building a tunnel pool.
    pending_sessions: SessionContext<R, crate::Result<SamSessionContext<R>>>,

    /// Profile storage.
    profile_storage: ProfileStorage<R>,

    /// Session ID to `DestinationId` mappings.
    session_id_destinations: HashMap<Arc<str>, DestinationId>,

    /// Private publisher for the bounded I2PControl SAM observation handle.
    observation_publisher: SamSessionObservationPublisher,

    /// SAMv3 datagram socket handle.
    socket_handle: UdpSocketHandle,

    /// RX channel for receiving session IDs of subsessions.
    sub_session_rx: Receiver<SubSessionCommand>,

    /// TX channel given to primary sessions, allowing them to send sub-session commands.
    sub_session_tx: Sender<SubSessionCommand>,

    /// Handle to `TunnelManager`.
    tunnel_manager_handle: TunnelManagerHandle,
}

impl<R: Runtime> SamServer<R> {
    /// Create new [`SamServer`]
    pub async fn new(
        tcp_port: u16,
        udp_port: u16,
        host: String,
        netdb_handle: NetDbHandle,
        tunnel_manager_handle: TunnelManagerHandle,
        metrics: R::MetricsHandle,
        address_book: Option<Arc<dyn AddressBook>>,
        event_handle: EventHandle<R>,
        profile_storage: ProfileStorage<R>,
    ) -> crate::Result<Self> {
        let listener = R::TcpListener::bind(SocketAddr::new(
            host.parse::<IpAddr>().expect("valid address"),
            tcp_port,
        ))
        .await
        .ok_or(Error::Connection(ConnectionError::BindFailure))?;

        // create runtime udp socket for the sam server
        //
        // this socket is used to receive datagrams across all sam sessions
        let socket = R::UdpSocket::bind(SocketAddr::new(
            host.parse::<IpAddr>().expect("valid address"),
            udp_port,
        ))
        .await
        .ok_or(Error::Connection(ConnectionError::BindFailure))?;

        // create udp socket object and spawn the even loop in a background task
        let (socket, socket_handle) = UdpSocket::<R>::new(socket);
        R::spawn(socket.run());

        tracing::info!(
            target: LOG_TARGET,
            %host,
            tcp_port = ?listener.local_address().map(|address| address.port()),
            udp_port = ?socket_handle.local_address().map(|address| address.port()),
            "starting sam server",
        );

        let (datagram_tx, datagram_rx) = channel(1024);
        let (sub_session_tx, sub_session_rx) = channel(64);
        let (observation_publisher, _) = SamSessionObservationPublisher::new();

        Ok(Self {
            active_destinations: HashSet::new(),
            active_sessions: SessionContext::new(),
            address_book,
            datagram_rx,
            datagram_tx,
            datagram_writer_state: DatagramWriterState::GetMessage,
            event_handle,
            listener,
            metrics,
            netdb_handle,
            pending_inbound_connections: R::join_set(),
            pending_sessions: SessionContext::new(),
            profile_storage,
            session_id_destinations: HashMap::new(),
            observation_publisher,
            socket_handle,
            sub_session_rx,
            sub_session_tx,
            tunnel_manager_handle,
        })
    }

    /// Get address of the SAMv3 TCP listener.
    pub fn tcp_local_address(&self) -> Option<SocketAddr> {
        self.listener.local_address()
    }

    /// Get address of the SAMv3 UDP socket.
    pub fn udp_local_address(&self) -> Option<SocketAddr> {
        self.socket_handle.local_address()
    }

    /// Clone the read-only SAM observation handle before moving the server into its runtime.
    pub fn observation_handle(&self) -> SamSessionObservationHandle {
        SamSessionObservationHandle {
            state: Arc::clone(&self.observation_publisher.state),
        }
    }
}

impl<R: Runtime> Future for SamServer<R> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = Pin::into_inner(self);

        loop {
            match this.listener.poll_accept(cx) {
                Poll::Pending => break,
                Poll::Ready(None) => return Poll::Ready(()),
                Poll::Ready(Some((stream, peer))) => {
                    this.pending_inbound_connections
                        .push(PendingSamConnection::new_with_peer(stream, peer));
                }
            }
        }

        loop {
            match this.socket_handle.poll_next_unpin(cx) {
                Poll::Pending => break,
                Poll::Ready(None) => return Poll::Ready(()),
                Poll::Ready(Some((datagram, _))) => {
                    let Some(Datagram {
                        session_id,
                        destination,
                        datagram,
                        options,
                        ..
                    }) = Datagram::parse(&datagram)
                    else {
                        tracing::warn!(
                            target: LOG_TARGET,
                            "malformed datagram",
                        );
                        continue;
                    };

                    if let Err(error) = this.active_sessions.send_command(
                        &Arc::clone(&session_id),
                        SamSessionCommand::SendDatagram {
                            destination: Box::new(destination),
                            datagram,
                            session_id: Arc::clone(&session_id),
                            options: (!options.is_empty()).then(|| Mapping::from(options)),
                        },
                    ) {
                        tracing::warn!(
                            target: LOG_TARGET,
                            ?session_id,
                            ?error,
                            "failed to send datagram to active session",
                        );
                    }
                }
            }
        }

        loop {
            match mem::replace(
                &mut this.datagram_writer_state,
                DatagramWriterState::GetMessage,
            ) {
                DatagramWriterState::GetMessage => match this.datagram_rx.poll_recv(cx) {
                    Poll::Pending => break,
                    Poll::Ready(None) => return Poll::Ready(()),
                    Poll::Ready(Some((port, datagram))) => {
                        this.datagram_writer_state = DatagramWriterState::WriteMessage {
                            target: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port),
                            datagram,
                        };
                    }
                },
                DatagramWriterState::WriteMessage { target, datagram } => {
                    match this.socket_handle.try_send_to(datagram, target) {
                        Ok(()) => {
                            this.datagram_writer_state = DatagramWriterState::GetMessage;
                        }
                        Err(ChannelError::Full) => tracing::warn!(
                            target: LOG_TARGET,
                            "datagram channel is full",
                        ),
                        Err(ChannelError::Closed) => {
                            tracing::warn!(
                                target: LOG_TARGET,
                                "datagram channel is closed",
                            );
                            return Poll::Ready(());
                        }
                        Err(ChannelError::DoesntExist) => {}
                    }
                }
            }
        }

        loop {
            match this.pending_inbound_connections.poll_next_unpin(cx) {
                Poll::Pending => break,
                Poll::Ready(None) => return Poll::Ready(()),
                Poll::Ready(Some(Ok(kind))) => match kind {
                    ConnectionKind::Session {
                        mut socket,
                        version,
                        session_id,
                        destination,
                        session_kind,
                        options,
                    } => {
                        // client send a `SESSION CREATE` message with an id that is already
                        // in use by either an active or a pending session
                        //
                        // reject connection by closing the socket
                        if this.active_sessions.contains_key(&session_id)
                            || this.pending_sessions.contains_key(&session_id)
                        {
                            tracing::warn!(
                                target: LOG_TARGET,
                                %session_id,
                                "duplicate session id",
                            );

                            R::spawn(async move {
                                let _ = socket
                                    .send_message_blocking(
                                        b"SESSION STATUS RESULT=DUPLICATE_ID".to_vec(),
                                    )
                                    .await;
                            });
                            continue;
                        }

                        // ensure this is not a duplicate session for the same destination
                        let destination_id = destination.destination.id();

                        if this.active_destinations.contains(&destination_id) {
                            tracing::warn!(
                                target: LOG_TARGET,
                                %destination_id,
                                "duplicate destination",
                            );

                            R::spawn(async move {
                                let _ = socket
                                    .send_message_blocking(
                                        b"SESSION STATUS RESULT=DUPLICATE_DEST".to_vec(),
                                    )
                                    .await;
                            });
                            continue;
                        }

                        tracing::info!(
                            target: LOG_TARGET,
                            ?session_id,
                            %destination_id,
                            ?version,
                            "start constructing new session",
                        );

                        // send request to `TunnelManager` to start creating a tunnel pool and get
                        // back a future which returns a `TunnelPoolHandle` when the tunnel pool has
                        // been constructed
                        //
                        // the constructed pool is not ready for immediate use and must be polled
                        // until the desired amount of inbound/outbound tunnels have been built at
                        // which point an active samv3 session can be constructed
                        let tunnel_pool_future = {
                            let config = TunnelPoolConfig::default();

                            match this.tunnel_manager_handle.create_tunnel_pool(TunnelPoolConfig {
                                name: Str::from(Arc::clone(&session_id)),
                                num_inbound: options
                                    .get("inbound.quantity")
                                    .and_then(|v| v.parse().ok())
                                    .unwrap_or(config.num_inbound),
                                num_inbound_hops: options
                                    .get("inbound.length")
                                    .and_then(|v| v.parse().ok())
                                    .unwrap_or(config.num_inbound_hops),
                                num_outbound: options
                                    .get("outbound.quantity")
                                    .and_then(|v| v.parse().ok())
                                    .unwrap_or(config.num_outbound),
                                num_outbound_hops: options
                                    .get("outbound.length")
                                    .and_then(|v| v.parse().ok())
                                    .unwrap_or(config.num_outbound_hops),
                            }) {
                                Ok(tunnel_pool_future) => tunnel_pool_future,
                                Err(error) => {
                                    tracing::warn!(
                                        target: LOG_TARGET,
                                        %session_id,
                                        ?error,
                                        "failed to create tunnel pool for session",
                                    );
                                    continue;
                                }
                            }
                        };

                        let (tx, rx) =
                            with_recycle(COMMAND_CHANNEL_SIZE, SamSessionCommandRecycle::default());
                        let netdb_handle = this.netdb_handle.clone();

                        this.pending_sessions.insert(
                            Arc::clone(&session_id),
                            tx,
                            PendingSamSession::new(
                                socket,
                                *destination,
                                Arc::clone(&session_id),
                                session_kind,
                                options,
                                rx,
                                this.datagram_tx.clone(),
                                Box::pin(tunnel_pool_future),
                                netdb_handle,
                                this.address_book.clone(),
                                this.event_handle.clone(),
                                this.profile_storage.clone(),
                                core::matches!(session_kind, SessionKind::Primary)
                                    .then(|| this.sub_session_tx.clone()),
                            )
                            .run(),
                        );
                        this.active_destinations.insert(destination_id.clone());
                        this.session_id_destinations.insert(session_id, destination_id);
                    }
                    ConnectionKind::Stream {
                        session_id,
                        mut socket,
                        host,
                        options,
                        ..
                    } => match host {
                        HostKind::Destination { destination } => {
                            if let Err(error) = this.active_sessions.send_command(
                                &Arc::clone(&session_id),
                                SamSessionCommand::Connect {
                                    socket,
                                    destination_id: destination.id(),
                                    options,
                                    session_id: Arc::clone(&session_id),
                                },
                            ) {
                                tracing::warn!(
                                    target: LOG_TARGET,
                                    %session_id,
                                    ?error,
                                    "failed to send `STREAM CONNECT` to active session",
                                )
                            }
                        }
                        HostKind::B32Host { destination_id } => {
                            if let Err(error) = this.active_sessions.send_command(
                                &Arc::clone(&session_id),
                                SamSessionCommand::Connect {
                                    socket,
                                    destination_id,
                                    options,
                                    session_id: Arc::clone(&session_id),
                                },
                            ) {
                                tracing::warn!(
                                    target: LOG_TARGET,
                                    %session_id,
                                    ?error,
                                    "failed to send `STREAM CONNECT` to active session",
                                )
                            }
                        }
                        HostKind::Host { host } => match &this.address_book {
                            None => {
                                tracing::warn!(
                                    target: LOG_TARGET,
                                    %session_id,
                                    %host,
                                    "host lookup requested but address book not specified",
                                );
                                debug_assert!(false);
                            }
                            Some(address_book) => {
                                tracing::trace!(
                                    target: LOG_TARGET,
                                    %session_id,
                                    %host,
                                    "resolve host",
                                );

                                match address_book.resolve_base32(&host) {
                                    Some(destination) => match base32_decode(&destination) {
                                        None => {
                                            tracing::error!(
                                                target: LOG_TARGET,
                                                "failed to base32-decode destination id from a host lookup",
                                            );
                                            debug_assert!(false);
                                        }
                                        Some(destination) => {
                                            let destination_id = DestinationId::from(destination);

                                            tracing::trace!(
                                                target: LOG_TARGET,
                                                %destination_id,
                                                "destination id found from the cache",
                                            );

                                            if let Err(error) = this.active_sessions.send_command(
                                                &Arc::clone(&session_id),
                                                SamSessionCommand::Connect {
                                                    socket,
                                                    destination_id,
                                                    options,
                                                    session_id: Arc::clone(&session_id),
                                                },
                                            ) {
                                                tracing::warn!(
                                                    target: LOG_TARGET,
                                                    %session_id,
                                                    ?error,
                                                    "failed to send `STREAM CONNECT` to active session",
                                                )
                                            }
                                        }
                                    },
                                    None => {
                                        tracing::debug!(
                                            target: LOG_TARGET,
                                            %session_id,
                                            %host,
                                            "failed to resolve host",
                                        );

                                        R::spawn(async move {
                                            let _ = socket
                                                .send_message_blocking(
                                                    "STREAM STATUS RESULT=I2P_ERROR\n"
                                                        .to_string()
                                                        .as_bytes()
                                                        .to_vec(),
                                                )
                                                .await;
                                        });
                                    }
                                }
                            }
                        },
                    },
                    ConnectionKind::Accept {
                        session_id,
                        socket,
                        options,
                        ..
                    } => {
                        if let Err(error) = this.active_sessions.send_command(
                            &Arc::clone(&session_id),
                            SamSessionCommand::Accept {
                                socket,
                                options,
                                session_id: Arc::clone(&session_id),
                            },
                        ) {
                            tracing::warn!(
                                target: LOG_TARGET,
                                %session_id,
                                ?error,
                                "failed to send `STREAM ACCEPT` to active session",
                            )
                        }
                    }
                    ConnectionKind::Forward {
                        session_id,
                        socket,
                        port,
                        options,
                        ..
                    } => {
                        if let Err(error) = this.active_sessions.send_command(
                            &Arc::clone(&session_id),
                            SamSessionCommand::Forward {
                                socket,
                                port,
                                options,
                                session_id: Arc::clone(&session_id),
                            },
                        ) {
                            tracing::warn!(
                                target: LOG_TARGET,
                                %session_id,
                                ?error,
                                "failed to send `STREAM FORWARD` to active session",
                            )
                        }
                    }
                },
                Poll::Ready(Some(Err(error))) => tracing::trace!(
                    target: LOG_TARGET,
                    ?error,
                    "failed to accept samv3 client connection",
                ),
            }
        }

        loop {
            match this.pending_sessions.poll_next_unpin(cx) {
                Poll::Pending => break,
                Poll::Ready(None) => return Poll::Ready(()),
                Poll::Ready(Some(Ok(context))) => {
                    let destination_id = context.destination.destination.id();
                    if let Err(error) = this.observation_publisher.activate_session(
                        &context.session_id,
                        &destination_id,
                        &context.options,
                        context.socket.observation_id(),
                        context.socket.peer_addr(),
                    ) {
                        tracing::warn!(
                            target: LOG_TARGET,
                            session_id = %context.session_id,
                            ?error,
                            "SAM observation source overflowed while activating session",
                        );
                    }

                    match this.pending_sessions.remove(&context.session_id) {
                        Some(tx) => {
                            this.active_sessions.insert(
                                Arc::clone(&context.session_id),
                                tx,
                                SamSession::new_with_observation(
                                    context,
                                    this.observation_publisher.clone(),
                                ),
                            );
                        }
                        None => {
                            tracing::warn!(
                                target: LOG_TARGET,
                                session_id = %context.session_id,
                                "pending session doesn't exist"
                            );
                            debug_assert!(false);

                            this.observation_publisher.remove_session(&context.session_id);

                            if let Some(destination_id) =
                                this.session_id_destinations.remove(&context.session_id)
                            {
                                this.active_destinations.remove(&destination_id);
                            }
                        }
                    }
                }
                Poll::Ready(Some(Err(error))) => tracing::warn!(
                    target: LOG_TARGET,
                    ?error,
                    "failed to create tunnel pool for session",
                ),
            }
        }

        loop {
            match this.active_sessions.poll_next_unpin(cx) {
                Poll::Pending => break,
                Poll::Ready(None) => return Poll::Ready(()),
                Poll::Ready(Some(session_id)) => {
                    tracing::info!(
                        target: LOG_TARGET,
                        %session_id,
                        "session terminated",
                    );
                    this.active_sessions.remove(&session_id);
                    this.observation_publisher.remove_session(&session_id);

                    if let Some(destination_id) = this.session_id_destinations.remove(&session_id) {
                        this.active_destinations.remove(&destination_id);
                    }
                }
            }
        }

        loop {
            match this.sub_session_rx.poll_recv(cx) {
                Poll::Pending => break,
                Poll::Ready(None) => return Poll::Ready(()),
                Poll::Ready(Some(SubSessionCommand::Add {
                    primary_session_id,
                    sub_session_id,
                })) => {
                    tracing::trace!(
                        target: LOG_TARGET,
                        %primary_session_id,
                        %sub_session_id,
                        "adding primary/sub-session id mapping",
                    );
                    this.active_sessions.insert_sub_session(primary_session_id, sub_session_id);
                }
                Poll::Ready(Some(SubSessionCommand::Remove { sub_session_id })) => {
                    tracing::trace!(
                        target: LOG_TARGET,
                        %sub_session_id,
                        "removing primary/sub-session id mapping",
                    );
                    this.active_sessions.remove_sub_session(&sub_session_id);
                }
                Poll::Ready(Some(SubSessionCommand::Dummy)) => unreachable!(),
            }
        }

        Poll::Pending
    }
}

#[cfg(test)]
mod observation_tests {
    use super::*;

    fn session_id(value: &str) -> Arc<str> {
        Arc::from(value)
    }

    fn destination_id(byte: u8) -> DestinationId {
        DestinationId::from([byte; 32])
    }

    fn options() -> HashMap<String, String> {
        HashMap::new()
    }

    fn peer() -> SocketAddr {
        "127.0.0.1:7656".parse().unwrap()
    }

    #[test]
    fn observation_starts_empty() {
        let (_, handle) = SamSessionObservationPublisher::new();
        let snapshot = handle.snapshot().unwrap();
        assert!(snapshot.sessions.is_empty());
        assert_eq!(snapshot.generation, 0);
    }

    #[test]
    fn cloned_handles_read_the_same_state() {
        let (publisher, handle) = SamSessionObservationPublisher::new();
        let clone = handle.clone();
        let id = session_id("session");
        publisher
            .activate_session(&id, &destination_id(1), &options(), 1, Some(peer()))
            .unwrap();
        assert_eq!(handle.snapshot().unwrap(), clone.snapshot().unwrap());
    }

    #[test]
    fn activation_publishes_exact_session_fields() {
        let (publisher, handle) = SamSessionObservationPublisher::new();
        let id = session_id("session");
        publisher
            .activate_session(&id, &destination_id(2), &options(), 1, Some(peer()))
            .unwrap();
        let mut snapshot = handle.snapshot().unwrap();
        let session = snapshot.sessions.remove(&id).unwrap();
        assert_eq!(session.name.as_ref(), &base64_encode([2u8; 32])[..4]);
        assert_eq!(
            session.address,
            format!("{}.b32.i2p", base32_encode([2u8; 32]))
        );
        assert_eq!(session.sockets.len(), 1);
        assert_eq!(session.sockets[0].socket_type, 1);
        assert_eq!(session.sockets[0].peer.as_ref(), "127.0.0.1:7656");
    }

    #[test]
    fn inbound_nickname_has_priority() {
        let (publisher, handle) = SamSessionObservationPublisher::new();
        let id = session_id("session");
        let mut options = options();
        options.insert("inbound.nickname".into(), "inbound".into());
        options.insert("outbound.nickname".into(), "outbound".into());
        publisher
            .activate_session(&id, &destination_id(3), &options, 1, Some(peer()))
            .unwrap();
        assert_eq!(
            handle.snapshot().unwrap().sessions[&id].name.as_ref(),
            "inbound"
        );
    }

    #[test]
    fn outbound_nickname_is_used_when_inbound_is_absent() {
        let (publisher, handle) = SamSessionObservationPublisher::new();
        let id = session_id("session");
        let mut options = options();
        options.insert("outbound.nickname".into(), "outbound".into());
        publisher
            .activate_session(&id, &destination_id(4), &options, 1, Some(peer()))
            .unwrap();
        assert_eq!(
            handle.snapshot().unwrap().sessions[&id].name.as_ref(),
            "outbound"
        );
    }

    #[test]
    fn missing_peer_fails_closed() {
        let (publisher, handle) = SamSessionObservationPublisher::new();
        let result = publisher.activate_session(
            &session_id("session"),
            &destination_id(5),
            &options(),
            1,
            None,
        );
        assert_eq!(result, Err(SamSessionObservationError::Overflow));
        assert_eq!(handle.snapshot(), Err(SamSessionObservationError::Overflow));
    }

    #[test]
    fn socket_add_and_remove_are_visible() {
        let (publisher, handle) = SamSessionObservationPublisher::new();
        let id = session_id("session");
        publisher
            .activate_session(&id, &destination_id(6), &options(), 1, Some(peer()))
            .unwrap();
        publisher.add_socket(&id, 2, 2, Some(peer())).unwrap();
        assert_eq!(handle.snapshot().unwrap().sessions[&id].sockets.len(), 2);
        publisher.remove_socket(&id, 2);
        assert_eq!(handle.snapshot().unwrap().sessions[&id].sockets.len(), 1);
    }

    #[test]
    fn session_removal_is_visible() {
        let (publisher, handle) = SamSessionObservationPublisher::new();
        let id = session_id("session");
        publisher
            .activate_session(&id, &destination_id(7), &options(), 1, Some(peer()))
            .unwrap();
        publisher.remove_session(&id);
        assert!(handle.snapshot().unwrap().sessions.is_empty());
    }

    #[test]
    fn per_session_socket_bound_is_explicit() {
        let (publisher, handle) = SamSessionObservationPublisher::new();
        let id = session_id("session");
        publisher
            .activate_session(&id, &destination_id(8), &options(), 1, Some(peer()))
            .unwrap();
        for socket_id in 2..=SAM_SOCKET_OBSERVATION_LIMIT as u64 + 1 {
            publisher.add_socket(&id, socket_id, 2, Some(peer())).unwrap();
        }
        assert_eq!(
            publisher.add_socket(&id, 99, 2, Some(peer())),
            Err(SamSessionObservationError::Overflow)
        );
        assert_eq!(handle.snapshot(), Err(SamSessionObservationError::Overflow));
    }

    #[test]
    fn session_bound_is_explicit() {
        let (publisher, handle) = SamSessionObservationPublisher::new();
        for session_number in 0..SAM_SESSION_OBSERVATION_LIMIT {
            publisher
                .activate_session(
                    &session_id(&format!("session-{session_number}")),
                    &destination_id(9),
                    &options(),
                    session_number as u64,
                    Some(peer()),
                )
                .unwrap();
        }
        assert_eq!(
            publisher.activate_session(
                &session_id("overflow"),
                &destination_id(9),
                &options(),
                2000,
                Some(peer())
            ),
            Err(SamSessionObservationError::Overflow)
        );
        assert_eq!(handle.snapshot(), Err(SamSessionObservationError::Overflow));
    }

    #[test]
    fn generation_changes_only_on_successful_publications() {
        let (publisher, handle) = SamSessionObservationPublisher::new();
        let id = session_id("session");
        publisher
            .activate_session(&id, &destination_id(10), &options(), 1, Some(peer()))
            .unwrap();
        let generation = handle.snapshot().unwrap().generation;
        publisher.add_socket(&id, 2, 2, Some(peer())).unwrap();
        assert!(handle.snapshot().unwrap().generation > generation);
    }
}
