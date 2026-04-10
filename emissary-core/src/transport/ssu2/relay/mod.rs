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
    crypto::{chachapoly::ChaChaPoly, SigningPublicKey, StaticPublicKey},
    error::{RelayError, Ssu2Error},
    primitives::{MlKemPreference, RouterAddress, RouterId, RouterInfo},
    router::context::RouterContext,
    runtime::{Counter, Instant, MetricsHandle, Runtime, UdpSocket},
    transport::ssu2::{
        message::{Block, HolePunchBuilder, ProtocolVersion},
        metrics::*,
        relay::types::{
            BobRejectionReason, CharlieRejectionReason, RejectionReason, RelayCommand, RelayEvent,
            RelayHandle,
        },
    },
};

use bytes::{BufMut, BytesMut};
use futures::{FutureExt, Stream};
use hashbrown::{HashMap, HashSet};
use rand::Rng;
use thingbuf::mpsc::{channel, Receiver, Sender};

use alloc::{boxed::Box, collections::VecDeque, vec::Vec};
use core::{
    net::{IpAddr, SocketAddr},
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

pub mod types;

/// Logging target for the file.
const LOG_TARGET: &str = "emissary::ssu2::relay";

/// Maintenance interval.
#[cfg(not(test))]
const MAINTENANCE_INTERVAL: Duration = Duration::from_secs(10);

#[cfg(test)]
const MAINTENANCE_INTERVAL: Duration = Duration::from_secs(2);

/// Timeout for a relay process.
///
/// Time after which an outbound relay process is considered failed
const RELAY_TIMEOUT: Duration = Duration::from_secs(10);

/// Introducer expiration.
pub const INTRODUCER_EXPIRATION: Duration = Duration::from_secs(80 * 60);

/// Router hash length.
const ROUTER_HASH_LEN: usize = 32usize;

/// Maximum amount of introducers.
const MAX_INTRODUCERS: usize = 3usize;

/// Events emitted by `RelayManager`.
pub enum RelayManagerEvent {
    // TODO:
    SessionRequestToken {
        /// Token used in an inbound `SessionRequest`.
        token: u64,
    },

    /// Introducer has expired.
    IntroducerExpired {
        /// Router ID of the expired introducer.
        router_id: RouterId,

        /// Was this introducer over IPv4.
        ipv4: bool,
    },

    /// Relay connection succeeded.
    RelaySuccess {
        /// Socket address of Charlie.
        address: SocketAddr,

        /// Router ID of Charlie.
        router_id: RouterId,

        /// Token that should be used in `SessionRequest`.
        token: u64,
    },

    /// Relayed connection failed.
    ///
    /// Failed to connect to Charlie.
    RelayFailure {
        /// Router ID of Charlie.
        router_id: RouterId,
    },
}

/// Context for a relayed connection.
pub struct RelayConnection {
    /// Destination connection ID.
    ///
    /// Derived from random nonce.
    pub dst_id: u64,

    /// Intro key if Charlie.
    pub intro_key: [u8; 32],

    /// MTU of remote router.
    pub mtu: usize,

    /// Source connection ID.
    ///
    /// Derived from random nonce.
    pub src_id: u64,

    /// SSU2 static key of Charlie.
    pub static_key: StaticPublicKey,

    /// Verifying key of Charlie.
    pub verifying_key: SigningPublicKey,

    /// Protocol version.
    pub version: ProtocolVersion,
}

/// Relay client.
struct RelayClient {
    /// TX channel for sending commands to the active session.
    cmd_tx: Sender<RelayCommand>,

    /// ID of remote router.
    router_id: RouterId,
}

/// Relay server.
struct RelayServer<R: Runtime> {
    /// Router ID of Bob.
    router_id: RouterId,

    /// Is the introducer over IPv4.
    ipv4: bool,

    /// When was the server created.
    created: R::Instant,
}

/// Active outbound relay process.
struct RelayProcess<R: Runtime> {
    /// Router ID of Bob.
    bob_router_id: RouterId,

    /// Router ID of Charlie.
    charlie_router_id: RouterId,

    /// Verifying key of Charlie.
    charlie_verifying_key: SigningPublicKey,

    /// When was the relay request sent.
    created: R::Instant,

    /// Relay tag.
    relay_tag: u32,
}

/// Relay manager.
pub struct RelayManager<R: Runtime> {
    /// Active inbound relay processes.
    ///
    /// Indexed by nonce, the senders are used to send relay responses
    /// received from Charlie to Alice.
    active_inbound: HashMap<u32, Sender<RelayCommand>>,

    /// Active outbound relay processes.
    ///
    /// Indexed by source connection ID derived from random nonce.
    active_outbound: HashMap<u64, RelayProcess<R>>,

    /// Active relay clients.
    ///
    /// IOW, context for all Charlies we've agreed to act as a relay for.
    clients: HashMap<u32, RelayClient>,

    /// RX channel for receiving relay events.
    event_rx: Receiver<RelayEvent>,

    /// TX channel given to `RelayHandle`s.
    event_tx: Sender<RelayEvent>,

    /// Our external address.
    ///
    /// `None` if it's unknown.
    external_address: Option<SocketAddr>,

    /// Mappings from router IDs to relay tags.
    id_mappings: HashMap<RouterId, u32>,

    /// Our intro key.
    intro_key: [u8; 32],

    /// IPv4 UDP socket.
    ipv4_socket: Option<R::UdpSocket>,

    /// IPv6 UDP socket.
    ipv6_socket: Option<R::UdpSocket>,

    /// Maintenance timer
    maintenance_timer: R::Timer,

    /// Pending events.
    pending_events: VecDeque<RelayManagerEvent>,

    /// Relay tags currently in use.
    relay_tags: HashSet<u32>,

    /// Router context.
    router_ctx: RouterContext<R>,

    /// Active relay servers.
    ///
    /// IOW, context for all Bob's who've agreed to act as relay for us.
    servers: HashMap<u32, RelayServer<R>>,

    /// Active sessions to routers which support the relay protocol.
    sessions: HashMap<RouterId, (Sender<RelayCommand>, bool)>,

    /// Write buffer.
    write_buffer: VecDeque<(BytesMut, SocketAddr)>,
}

impl<R: Runtime> RelayManager<R> {
    /// Create new `RelayManager`.
    pub fn new(
        intro_key: [u8; 32],
        router_ctx: RouterContext<R>,
        ipv4_socket: Option<R::UdpSocket>,
        ipv6_socket: Option<R::UdpSocket>,
    ) -> Self {
        let (event_tx, event_rx) = channel(128);

        Self {
            active_inbound: HashMap::new(),
            active_outbound: HashMap::new(),
            clients: HashMap::new(),
            event_rx,
            event_tx,
            external_address: None,
            id_mappings: HashMap::new(),
            intro_key,
            maintenance_timer: R::timer(MAINTENANCE_INTERVAL),
            pending_events: VecDeque::new(),
            relay_tags: HashSet::new(),
            router_ctx,
            servers: HashMap::new(),
            sessions: HashMap::new(),
            ipv4_socket,
            ipv6_socket,
            write_buffer: VecDeque::new(),
        }
    }

    /// Does `RelayManager` need introducers.
    pub fn needs_introducers(&self) -> bool {
        self.servers.len() < MAX_INTRODUCERS
    }

    /// Get `RelayHandle` for an active session.
    pub fn handle(&self) -> RelayHandle<R> {
        RelayHandle::new(self.event_tx.clone())
    }

    /// Add external address for `RelayManager`.
    pub fn add_external_address(&mut self, address: SocketAddr) {
        self.external_address = Some(address)
    }

    /// Allocate relay tag.
    pub fn allocate_relay_tag(&mut self) -> u32 {
        loop {
            let tag = R::rng().next_u32();

            if self.relay_tags.insert(tag) {
                return tag;
            }
        }
    }

    /// Deallocate relay tag.
    pub fn deallocate_relay_tag(&mut self, tag: u32) {
        self.relay_tags.remove(&tag);
    }

    /// Register active session with a router that supports the relay protocol
    /// and is capable of acting as an introducer.
    pub fn add_session(&mut self, router_id: &RouterId, sender: Sender<RelayCommand>, ipv4: bool) {
        tracing::trace!(
            target: LOG_TARGET,
            %router_id,
            "register session to router that supports relay",
        );

        self.sessions.insert(router_id.clone(), (sender, ipv4));
    }

    /// Register relay client
    ///
    /// Relay clients are routers we're willing to assist in inbound connections.
    pub fn register_relay_client(
        &mut self,
        router_id: RouterId,
        relay_tag: u32,
        cmd_tx: Sender<RelayCommand>,
    ) {
        tracing::debug!(
            target: LOG_TARGET,
            %router_id,
            ?relay_tag,
            num_clients = self.clients.len(),
            "register relay client",
        );

        self.id_mappings.insert(router_id.clone(), relay_tag);
        self.clients.insert(relay_tag, RelayClient { cmd_tx, router_id });
    }

    /// Register relay server.
    ///
    /// Relay servers are routers who are willing to act as relay for us.
    pub fn register_relay_server(&mut self, router_id: RouterId, relay_tag: u32, ipv4: bool) {
        tracing::debug!(
            target: LOG_TARGET,
            %router_id,
            ?relay_tag,
            num_servers = ?self.servers.len(),
            "register relay server",
        );

        self.id_mappings.insert(router_id.clone(), relay_tag);
        self.servers.insert(
            relay_tag,
            RelayServer {
                router_id,
                ipv4,
                created: R::now(),
            },
        );
    }

    /// Register closed connection to `RelayManager`.
    ///
    /// Returns `true` if an active relay server was removed.
    pub fn register_closed_connection(&mut self, router_id: &RouterId) -> bool {
        if let Some(tag) = self.id_mappings.remove(router_id) {
            self.clients.remove(&tag);

            return self.servers.remove(&tag).is_some();
        }

        false
    }

    /// Send relay request to one of the introducers listed in `router_info`.
    pub fn send_relay_request(
        &mut self,
        router_info: RouterInfo,
        disable_pq: bool,
    ) -> Result<RelayConnection, RelayError> {
        let charlie_router_id = router_info.identity.id();
        let charlie_verifying_key = router_info.identity.signing_key();

        let (introducers, intro_key, static_key, mtu, ml_kem) = match router_info
            .addresses()
            .find(|address| core::matches!(address, RouterAddress::Ssu2 { .. }))
        {
            Some(RouterAddress::Ssu2 {
                cost: _,
                introducers,
                intro_key,
                mtu,
                socket_address: _,
                static_key,
                ml_kem,
                ..
            }) => (
                introducers,
                intro_key,
                static_key,
                *mtu,
                (!disable_pq).then_some(*ml_kem).flatten(),
            ),
            _ => return Err(RelayError::NoAddress),
        };

        // find an introducer with have an active connection with
        let (bob_router_id, relay_tag, sender, ipv4) = introducers
            .iter()
            .find_map(|(router_id, relay_tag)| {
                self.sessions
                    .get(router_id)
                    .map(|(sender, ipv4)| (router_id, relay_tag, sender, ipv4))
            })
            .ok_or(RelayError::NoIntroducer)?;

        // select version for the relay connection
        let version = match ml_kem {
            None => ProtocolVersion::V2,
            Some(MlKemPreference::MlKem512 | MlKemPreference::MlKem512MlKem768) =>
                ProtocolVersion::V3,
            Some(MlKemPreference::MlKem768 | MlKemPreference::MlKem768MlKem512) =>
                ProtocolVersion::V4,
        };

        // create relay request and signature
        let (nonce, message, signature) = {
            let nonce = R::rng().next_u32();
            let mut message = BytesMut::with_capacity(128);
            message.put_slice(b"RelayRequestData");
            message.put_slice(&bob_router_id.to_vec());
            message.put_slice(&charlie_router_id.to_vec());
            message.put_u32(nonce);
            message.put_u32(*relay_tag);
            message.put_u32(R::time_since_epoch().as_secs() as u32);
            message.put_u8(2); // version

            // the sockets must exist since they were checked above
            let local_address = ipv4
                .then(|| self.ipv4_socket.as_ref().expect("ipv4 socket to exist").local_address())
                .or_else(|| {
                    Some(self.ipv6_socket.as_ref().expect("ipv6 socket to exist").local_address())
                });

            match local_address.flatten().expect("address to exist") {
                SocketAddr::V4(address) => {
                    message.put_u8(6); // address size
                    message.put_u16(address.port());
                    message.put_slice(&address.ip().octets());
                }
                SocketAddr::V6(address) => {
                    message.put_u8(18); // address size
                    message.put_u16(address.port());
                    message.put_slice(&address.ip().octets());
                }
            }
            let signature = self.router_ctx.signing_key().sign(&message);

            (
                nonce,
                message.split_off(b"RelayRequestData".len() + 2 * ROUTER_HASH_LEN).to_vec(),
                signature,
            )
        };

        match sender.try_send(RelayCommand::RelayRequest {
            nonce,
            message,
            signature,
        }) {
            Ok(()) => {
                let dst_id = (((nonce as u64) << 32) | (nonce as u64)).to_be();
                let src_id = (!(((nonce as u64) << 32) | (nonce as u64))).to_be();

                tracing::trace!(
                    target: LOG_TARGET,
                    %charlie_router_id,
                    %bob_router_id,
                    ?nonce,
                    ?relay_tag,
                    ?dst_id,
                    ?src_id,
                    "relay request sent to bob",
                );

                self.active_outbound.insert(
                    src_id,
                    RelayProcess {
                        bob_router_id: bob_router_id.clone(),
                        charlie_router_id,
                        charlie_verifying_key: charlie_verifying_key.clone(),
                        created: R::now(),
                        relay_tag: *relay_tag,
                    },
                );

                Ok(RelayConnection {
                    dst_id,
                    intro_key: *intro_key,
                    mtu,
                    src_id,
                    static_key: static_key.clone(),
                    verifying_key: charlie_verifying_key.clone(),
                    version,
                })
            }
            Err(error) => {
                tracing::debug!(
                    target: LOG_TARGET,
                    %charlie_router_id,
                    %bob_router_id,
                    ?nonce,
                    ?relay_tag,
                    ?error,
                    "failed to send relay request to bob",
                );

                Err(RelayError::RelayRequestSendFailure)
            }
        }
    }

    /// Handle `HolePunch` message.
    ///
    /// Decrypt and parse `HolePunch` message, locate `RelayResponse` block
    /// and check if the request was accepted.
    ///
    /// If no, return an error.
    /// If yes, return a (Charlie's router ID, Charlie's address, token) tuple.
    pub fn handle_hole_punch(
        &mut self,
        datagram: Vec<u8>,
        pkt_num: u32,
        src_id: u64,
    ) -> Result<(RouterId, SocketAddr, u64), Ssu2Error> {
        tracing::trace!(
            target: LOG_TARGET,
            ?src_id,
            ?pkt_num,
            "handle out-of-session holepunch"
        );

        if datagram.len() <= 32 {
            return Err(Ssu2Error::Relay(RelayError::InvalidHolePunch));
        }

        let Some(RelayProcess {
            bob_router_id,
            charlie_router_id,
            created,
            charlie_verifying_key,
            relay_tag,
        }) = self.active_outbound.remove(&src_id)
        else {
            tracing::debug!(
                target: LOG_TARGET,
                ?src_id,
                "unrecognized relay process",
            );
            return Err(Ssu2Error::Relay(RelayError::UnknownRelayProcess));
        };

        let ad = datagram[..32].to_vec();
        let mut datagram = datagram[32..].to_vec();

        ChaChaPoly::with_nonce(&self.intro_key, pkt_num as u64)
            .decrypt_with_ad(&ad, &mut datagram)?;

        // locate `RelayResponse` block from `datagram`
        let Some(Block::RelayResponse {
            nonce,
            address,
            token,
            rejection,
            message,
            signature,
        }) = Block::parse::<R>(&datagram)
            .map_err(|_| Ssu2Error::Malformed)?
            .into_iter()
            .find(|block| core::matches!(block, Block::RelayResponse { .. }))
        else {
            return Err(Ssu2Error::Relay(RelayError::NoRelayResponse));
        };

        let (token, address) = match (rejection, token, address) {
            (None, Some(token), Some(address)) => (token, address),
            (Some(rejection), ..) => {
                tracing::debug!(
                    target: LOG_TARGET,
                    ?nonce,
                    ?rejection,
                    "relay request rejected",
                );

                return Err(Ssu2Error::Relay(RelayError::Rejected));
            }
            (_, None, _) | (_, _, None) => {
                tracing::debug!(
                    target: LOG_TARGET,
                    ?nonce,
                    token_exists = ?token.is_some(),
                    address_exists = ?address.is_some(),
                    "unable to handle relay response, token, or address",
                );

                return Err(Ssu2Error::Relay(RelayError::InvalidHolePunch));
            }
        };

        // verify signature of `RelayResponse` included in `HolePunch`
        {
            let mut payload = BytesMut::with_capacity(message.len() + 64);
            payload.put_slice(b"RelayAgreementOK");
            payload.put_slice(&bob_router_id.to_vec());
            payload.put_slice(&message);

            if charlie_verifying_key.verify(&payload, &signature).is_err() {
                tracing::warn!(
                    target: LOG_TARGET,
                    ?nonce,
                    "invalid signature for relay response",
                );

                return Err(Ssu2Error::Relay(RelayError::InvalidSignature));
            }
        }

        tracing::trace!(
            target: LOG_TARGET,
            ?nonce,
            ?relay_tag,
            %charlie_router_id,
            %bob_router_id,
            elapsed = ?created.elapsed(),
            "relay request accepted",
        );

        Ok((charlie_router_id, address, token))
    }

    /// Send relay request/intro rejection.
    fn reject_relay(
        &self,
        nonce: u32,
        reason: RejectionReason,
        router_id: &RouterId,
        tx: Sender<RelayCommand>,
    ) {
        let (message, signature) = {
            let mut message = BytesMut::with_capacity(58);
            message.put_slice(b"RelayAgreementOK");
            message.put_slice(&router_id.to_vec());
            message.put_u32(nonce);
            message.put_u32(R::time_since_epoch().as_secs() as u32);
            message.put_u8(2); // version
            message.put_u8(0u8); // address size

            let signature = self.router_ctx.signing_key().sign(&message);

            (
                message.split_off(b"RelayAgreementOK".len() + ROUTER_HASH_LEN).to_vec(),
                signature,
            )
        };

        if let Err(error) = tx.try_send(RelayCommand::RelayResponse {
            nonce,
            rejection: Some(reason),
            message,
            signature,
            token: None,
        }) {
            tracing::debug!(
                target: LOG_TARGET,
                ?nonce,
                ?error,
                "failed to send relay request rejection to alice",
            );
        }
    }

    /// Handle relay request from Alice.
    fn handle_relay_request(
        &mut self,
        alice_router_id: RouterId,
        nonce: u32,
        relay_tag: u32,
        address: SocketAddr,
        message: Vec<u8>,
        signature: Vec<u8>,
        tx: Sender<RelayCommand>,
    ) {
        tracing::debug!(
            target: LOG_TARGET,
            %alice_router_id,
            ?nonce,
            ?relay_tag,
            ?address,
            "handle relay request",
        );

        let Some(RelayClient {
            router_id: charlie_router_id,
            cmd_tx,
        }) = self.clients.get(&relay_tag)
        else {
            tracing::debug!(
                target: LOG_TARGET,
                %alice_router_id,
                ?nonce,
                ?relay_tag,
                "relay agreement does not exist, rejecting",
            );

            return self.reject_relay(
                nonce,
                RejectionReason::Bob(BobRejectionReason::RelayTagNotFound),
                self.router_ctx.router_id(),
                tx,
            );
        };

        // get alice's router infos
        let (router_info, serialized) = {
            let (router_info, serialized) = {
                let reader = self.router_ctx.profile_storage().reader();
                let router_info = reader.router_info(&alice_router_id).cloned();
                let raw_router_info = reader.raw_router_info(&alice_router_id);

                (router_info, raw_router_info)
            };

            match (router_info, serialized) {
                (Some(router_info), Some(serialized)) => (router_info, serialized),
                (router_info, serialized) => {
                    tracing::warn!(
                        target: LOG_TARGET,
                        %alice_router_id,
                        router_info_found = %router_info.is_some(),
                        serialized_found = %serialized.is_some(),
                        "alice's router info not available, rejecting relay request",
                    );

                    return self.reject_relay(
                        nonce,
                        RejectionReason::Bob(BobRejectionReason::AliceNotFound),
                        self.router_ctx.router_id(),
                        tx,
                    );
                }
            }
        };

        // verify signature of `RelayRequest`
        {
            let mut payload = BytesMut::with_capacity(128);
            payload.put_slice(b"RelayRequestData");
            payload.put_slice(&self.router_ctx.router_id().to_vec());
            payload.put_slice(&charlie_router_id.to_vec());
            payload.put_slice(&message);

            if router_info.identity.signing_key().verify(&payload, &signature).is_err() {
                tracing::warn!(
                    %alice_router_id,
                    ?nonce,
                    ?relay_tag,
                    "failed to verify signature, rejecting relay request",
                );

                return self.reject_relay(
                    nonce,
                    RejectionReason::Bob(BobRejectionReason::SignatureFailure),
                    self.router_ctx.router_id(),
                    tx,
                );
            }
        }

        match cmd_tx.try_send(RelayCommand::RelayIntro {
            router_id: router_info.identity.id().to_vec(),
            router_info: serialized,
            message,
            signature,
        }) {
            Ok(()) => {
                tracing::trace!(
                    target: LOG_TARGET,
                    %alice_router_id,
                    %charlie_router_id,
                    ?nonce,
                    ?relay_tag,
                    "relay intro sent to charlie",
                );

                self.active_inbound.insert(nonce, tx);
            }
            Err(error) => {
                tracing::debug!(
                    target: LOG_TARGET,
                    %alice_router_id,
                    charlie_router_id = %router_info.identity.id(),
                    ?nonce,
                    ?relay_tag,
                    ?error,
                    "failed to send relay into to charlie",
                );
            }
        }
    }

    /// Handle relay intro from Bob.
    fn handle_relay_intro(
        &mut self,
        alice_router_id: RouterId,
        bob_router_id: RouterId,
        alice_router_info: Option<Box<RouterInfo>>,
        nonce: u32,
        relay_tag: u32,
        address: SocketAddr,
        message: Vec<u8>,
        signature: Vec<u8>,
        tx: Sender<RelayCommand>,
    ) {
        tracing::debug!(
            target: LOG_TARGET,
            ?nonce,
            ?relay_tag,
            "handle relay intro",
        );

        if self.servers.get(&relay_tag).is_none() {
            tracing::debug!(
                target: LOG_TARGET,
                ?relay_tag,
                ?nonce,
                "no relay tag found, rejecting relay intro",
            );

            return self.reject_relay(
                nonce,
                RejectionReason::Charlie(CharlieRejectionReason::Unspecified),
                &bob_router_id,
                tx,
            );
        }

        let router_info = match alice_router_info {
            Some(router_info) => *router_info,
            None => match self.router_ctx.profile_storage().get(&alice_router_id) {
                Some(router_info) => router_info,
                None => {
                    tracing::debug!(
                        target: LOG_TARGET,
                        ?relay_tag,
                        ?nonce,
                        "alice not found from local storage, unable to hole punch",
                    );

                    return self.reject_relay(
                        nonce,
                        RejectionReason::Charlie(CharlieRejectionReason::AliceNotFound),
                        &bob_router_id,
                        tx,
                    );
                }
            },
        };

        let (intro_key, alice_address) = match address.is_ipv4() {
            true => match router_info.ssu2_ipv4() {
                Some(RouterAddress::Ssu2 {
                    intro_key,
                    socket_address: Some(socket_address),
                    ..
                }) => (*intro_key, *socket_address),
                None => {
                    tracing::debug!(
                        target: LOG_TARGET,
                        "charlie does not have a dialable ipv4 address",
                    );
                    debug_assert!(false);
                    return;
                }
                _ => unreachable!(),
            },
            false => match router_info.ssu2_ipv6() {
                None => {
                    tracing::warn!(
                        target: LOG_TARGET,
                        "charlie does not have a dialable ipv6 address",
                    );
                    debug_assert!(false);
                    return;
                }
                Some(RouterAddress::Ssu2 {
                    intro_key,
                    socket_address: Some(socket_address),
                    ..
                }) => (*intro_key, *socket_address),
                _ => unreachable!(),
            },
        };

        // verify signature
        {
            let mut payload = BytesMut::with_capacity(128);
            payload.put_slice(b"RelayRequestData");
            payload.put_slice(&bob_router_id.to_vec());
            payload.put_slice(&self.router_ctx.router_id().to_vec());
            payload.put_slice(&message);

            if router_info.identity.signing_key().verify(&payload, &signature).is_err() {
                tracing::warn!(
                    target: LOG_TARGET,
                    ?relay_tag,
                    ?nonce,
                    "failed to verify siganture if relay intro, rejecting",
                );

                return self.reject_relay(
                    nonce,
                    RejectionReason::Charlie(CharlieRejectionReason::SignatureFailure),
                    &bob_router_id,
                    tx,
                );
            }
        }

        let Some(external_address) = self.external_address else {
            tracing::debug!(
                target: LOG_TARGET,
                ?nonce,
                ?relay_tag,
                "no external address, rejecting relay intro",
            );

            return self.reject_relay(
                nonce,
                RejectionReason::Charlie(CharlieRejectionReason::Unspecified),
                &bob_router_id,
                tx,
            );
        };

        let token = R::rng().next_u64();
        let (relay_response, signature) = {
            let mut payload = BytesMut::with_capacity(128);
            payload.put_slice(b"RelayAgreementOK");
            payload.put_slice(&bob_router_id.to_vec());
            payload.put_u32(nonce);
            payload.put_u32(R::time_since_epoch().as_secs() as u32);
            payload.put_u8(2); // version

            match external_address.ip() {
                IpAddr::V4(address) => {
                    payload.put_u8(6);
                    payload.put_u16(external_address.port());
                    payload.put_slice(&address.octets());
                }
                IpAddr::V6(address) => {
                    payload.put_u8(18);
                    payload.put_u16(external_address.port());
                    payload.put_slice(&address.octets());
                }
            }
            let signature = self.router_ctx.signing_key().sign(&payload);

            (
                payload.split_off(b"RelayAgreementOK".len() + ROUTER_HASH_LEN).to_vec(),
                signature,
            )
        };

        let dst_id = (((nonce as u64) << 32) | (nonce as u64)).to_be();
        let src_id = (!(((nonce as u64) << 32) | (nonce as u64))).to_be();

        tracing::trace!(
            target: LOG_TARGET,
            %alice_router_id,
            ?nonce,
            ?relay_tag,
            ?address,
            ?token,
            ?dst_id,
            ?src_id,
            "accept relay intro",
        );

        let pkt = HolePunchBuilder::new(&relay_response, &signature)
            .with_net_id(self.router_ctx.net_id())
            .with_src_id(src_id)
            .with_token(token)
            .with_dst_id(dst_id)
            .with_intro_key(intro_key)
            .with_addres(alice_address)
            .build::<R>();

        self.write_buffer.push_back((pkt, address));
        self.pending_events.push_back(RelayManagerEvent::SessionRequestToken { token });

        if let Err(error) = tx.try_send(RelayCommand::RelayResponse {
            nonce,
            rejection: None,
            message: relay_response,
            signature,
            token: Some(token),
        }) {
            tracing::debug!(
                target: LOG_TARGET,
                ?nonce,
                ?relay_tag,
                ?error,
                "failed to send relay response to bob",
            );
        }
    }

    /// Handle relay response, either from Charlie or Bob.
    fn handle_relay_response(
        &mut self,
        nonce: u32,
        address: Option<SocketAddr>,
        token: Option<u64>,
        rejection: Option<RejectionReason>,
        message: Vec<u8>,
        signature: Vec<u8>,
    ) {
        let src_id = (!(((nonce as u64) << 32) | (nonce as u64))).to_be();

        if let Some(tx) = self.active_inbound.remove(&nonce) {
            tracing::trace!(
                target: LOG_TARGET,
                ?nonce,
                ?address,
                ?rejection,
                ?token,
                "send relay response to alice",
            );

            if let Err(error) = tx.try_send(RelayCommand::RelayResponse {
                nonce,
                rejection,
                message,
                signature,
                token,
            }) {
                tracing::debug!(
                    target: LOG_TARGET,
                    ?nonce,
                    ?error,
                    "failed to send relay response to alice",
                );
            }

            return;
        }

        let Some(RelayProcess {
            bob_router_id,
            charlie_router_id,
            created,
            charlie_verifying_key,
            relay_tag,
        }) = self.active_outbound.remove(&src_id)
        else {
            tracing::debug!(
                target: LOG_TARGET,
                ?nonce,
                "unrecognized relay process",
            );
            return;
        };

        tracing::trace!(
            target: LOG_TARGET,
            ?nonce,
            ?relay_tag,
            ?rejection,
            ?token,
            elapsed = ?created.elapsed(),
            "handle relay response",
        );

        let (token, address) = match (rejection, token, address) {
            (None, Some(token), Some(address)) => (token, address),
            (Some(rejection), ..) => {
                tracing::debug!(
                    target: LOG_TARGET,
                    ?nonce,
                    ?rejection,
                    "relay request rejected",
                );
                self.router_ctx.metrics_handle().counter(RELAY_FAILURE).increment_with_label(
                    1,
                    "reason",
                    rejection.into(),
                );

                return self.pending_events.push_back(RelayManagerEvent::RelayFailure {
                    router_id: charlie_router_id,
                });
            }
            (_, None, _) | (_, _, None) => {
                tracing::debug!(
                    target: LOG_TARGET,
                    ?nonce,
                    token_exists = ?token.is_some(),
                    address_exists = ?address.is_some(),
                    "unable to handle relay response, token, address or signature missing",
                );
                self.router_ctx.metrics_handle().counter(RELAY_FAILURE).increment_with_label(
                    1,
                    "reason",
                    "invalid-msg",
                );

                return self.pending_events.push_back(RelayManagerEvent::RelayFailure {
                    router_id: charlie_router_id,
                });
            }
        };

        // verify signature
        {
            let mut payload = BytesMut::with_capacity(message.len() + 64);
            payload.put_slice(b"RelayAgreementOK");
            payload.put_slice(&bob_router_id.to_vec());
            payload.put_slice(&message);

            if charlie_verifying_key.verify(&payload, &signature).is_err() {
                tracing::warn!(
                    target: LOG_TARGET,
                    ?nonce,
                    "invalid signature for relay response",
                );
                self.router_ctx.metrics_handle().counter(RELAY_FAILURE).increment_with_label(
                    1,
                    "reason",
                    "invalid-signature",
                );

                return self.pending_events.push_back(RelayManagerEvent::RelayFailure {
                    router_id: charlie_router_id,
                });
            }
        }

        self.router_ctx.metrics_handle().counter(RELAY_SUCCESS).increment(1);
        self.pending_events.push_back(RelayManagerEvent::RelaySuccess {
            address,
            router_id: charlie_router_id,
            token,
        });
    }

    /// Performance maintenance on `RelayManager`.
    ///
    /// Remove expired introducers and relay requests.
    fn maintain(&mut self) {
        let expired = self
            .servers
            .iter()
            .filter_map(
                |(
                    tag,
                    RelayServer {
                        router_id,
                        created,
                        ipv4,
                    },
                )| {
                    (created.elapsed() > INTRODUCER_EXPIRATION).then_some((
                        *tag,
                        router_id.clone(),
                        *ipv4,
                    ))
                },
            )
            .collect::<Vec<_>>();

        if !expired.is_empty() {
            tracing::info!(
                target: LOG_TARGET,
                ?expired,
                "one or more introducers have expired",
            );

            expired.into_iter().for_each(|(relay_tag, router_id, ipv4)| {
                self.id_mappings.remove(&router_id);
                self.servers.remove(&relay_tag);
                self.pending_events
                    .push_back(RelayManagerEvent::IntroducerExpired { router_id, ipv4 });
            });
        }

        let expired = self
            .active_outbound
            .iter()
            .filter_map(|(src_id, process)| {
                (process.created.elapsed() > RELAY_TIMEOUT)
                    .then_some((*src_id, process.charlie_router_id.clone()))
            })
            .collect::<Vec<_>>();

        if !expired.is_empty() {
            tracing::debug!(
                target: LOG_TARGET,
                ?expired,
                "one or more relay requests have expired",
            );

            expired.into_iter().for_each(|(src_id, router_id)| {
                self.router_ctx
                    .metrics_handle()
                    .counter(RELAY_FAILURE)
                    .increment_with_label(1, "reason", "expired");
                self.active_outbound.remove(&src_id);
                self.pending_events.push_back(RelayManagerEvent::RelayFailure { router_id });
            });
        }
    }
}

impl<R: Runtime> Stream for RelayManager<R> {
    type Item = RelayManagerEvent;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            match self.event_rx.poll_recv(cx) {
                Poll::Pending => break,
                Poll::Ready(None) => return Poll::Ready(None),
                Poll::Ready(Some(event)) => match event {
                    RelayEvent::RelayRequest {
                        alice_router_id,
                        nonce,
                        relay_tag,
                        address,
                        message,
                        signature,
                        tx,
                    } => self.handle_relay_request(
                        alice_router_id,
                        nonce,
                        relay_tag,
                        address,
                        message,
                        signature,
                        tx,
                    ),
                    RelayEvent::RelayIntro {
                        alice_router_id,
                        bob_router_id,
                        alice_router_info,
                        nonce,
                        relay_tag,
                        address,
                        message,
                        signature,
                        tx,
                    } => self.handle_relay_intro(
                        alice_router_id,
                        bob_router_id,
                        alice_router_info,
                        nonce,
                        relay_tag,
                        address,
                        message,
                        signature,
                        tx,
                    ),
                    RelayEvent::RelayResponse {
                        nonce,
                        address,
                        token,
                        rejection,
                        message,
                        signature,
                    } => self.handle_relay_response(
                        nonce, address, token, rejection, message, signature,
                    ),
                    RelayEvent::Dummy => unreachable!(),
                },
            }
        }

        if self.maintenance_timer.poll_unpin(cx).is_ready() {
            // create new timer and register it into the executor
            {
                self.maintenance_timer = R::timer(MAINTENANCE_INTERVAL);
                let _ = self.maintenance_timer.poll_unpin(cx);
            }

            self.maintain();
        }

        if let Some(event) = self.pending_events.pop_front() {
            return Poll::Ready(Some(event));
        }

        while let Some((pkt, address)) = self.write_buffer.pop_front() {
            let socket = match address.is_ipv4() {
                true => self.ipv4_socket.as_mut().expect("to exist"),
                false => self.ipv6_socket.as_mut().expect("to exist"),
            };

            match Pin::new(socket).poll_send_to(cx, &pkt, address) {
                Poll::Pending => {
                    self.write_buffer.push_front((pkt, address));
                    break;
                }
                Poll::Ready(None) => return Poll::Ready(None),
                Poll::Ready(Some(_)) => {}
            }
        }

        Poll::Pending
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        crypto::{base64_encode, chachapoly::ChaChaPoly, SigningPrivateKey, StaticPrivateKey},
        primitives::{RouterInfoBuilder, Str},
        profile::ProfileStorage,
        router::context::builder::RouterContextBuilder,
        runtime::mock::{MockRuntime, MockUdpSocket},
        timeout,
        transport::ssu2::message::{
            handshake::TokenRequestBuilder, Block, HeaderKind, HeaderReader,
        },
        Ssu2Config,
    };
    use bytes::Bytes;
    use futures::{FutureExt, StreamExt};
    use futures_channel::oneshot;

    #[allow(unused)]
    struct TestRouter {
        router_id: RouterId,
        router_info: RouterInfo,
        static_key: StaticPrivateKey,
        signing_key: SigningPrivateKey,
        serialized: Vec<u8>,
        socket: MockUdpSocket,
        intro_key: [u8; 32],
    }

    impl TestRouter {
        async fn with_intorducer(seed: u8, router_id: RouterId, relay_tag: u32) -> Self {
            let socket = <MockRuntime as Runtime>::UdpSocket::bind("127.0.0.1:0".parse().unwrap())
                .await
                .unwrap();
            let (mut router_info, static_key, signing_key) = RouterInfoBuilder::default()
                .with_ssu2(Ssu2Config {
                    ml_kem: None,
                    disable_pq: false,
                    port: socket.local_address().unwrap().port(),
                    ipv4_host: Some("127.0.0.1".parse().unwrap()),
                    ipv6_host: None,
                    ipv4: true,
                    ipv6: false,
                    publish: false,
                    static_key: [seed; 32],
                    intro_key: [seed + 1; 32],
                    ipv4_mtu: None,
                    ipv6_mtu: None,
                })
                .build();

            let Some(RouterAddress::Ssu2 { options, .. }) = router_info.ssu2_ipv4_mut() else {
                panic!("ssu2 address not found");
            };

            options.insert(
                Str::from(format!("iexp0")),
                Str::from(
                    (MockRuntime::time_since_epoch() + Duration::from_secs(2 * 60))
                        .as_secs()
                        .to_string(),
                ),
            );
            options.insert(
                Str::from(format!("ih0")),
                Str::from(base64_encode(router_id.to_vec())),
            );
            options.insert(
                Str::from(format!("itag0")),
                Str::from(relay_tag.to_string()),
            );

            let serialized = router_info.serialize(&signing_key);
            let router_id = router_info.identity.id();

            Self {
                router_id,
                router_info,
                serialized,
                signing_key,
                intro_key: [seed + 1; 32],
                socket,
                static_key,
            }
        }

        fn parsed(&self) -> RouterInfo {
            RouterInfo::parse::<MockRuntime>(&self.serialized).unwrap()
        }

        async fn new(seed: u8) -> Self {
            let socket = <MockRuntime as Runtime>::UdpSocket::bind("127.0.0.1:0".parse().unwrap())
                .await
                .unwrap();
            let (router_info, static_key, signing_key) = RouterInfoBuilder::default()
                .with_ssu2(Ssu2Config {
                    ml_kem: None,
                    disable_pq: false,
                    port: socket.local_address().unwrap().port(),
                    ipv4_host: Some("127.0.0.1".parse().unwrap()),
                    ipv6_host: None,
                    ipv4: true,
                    ipv6: false,
                    publish: true,
                    static_key: [seed; 32],
                    intro_key: [seed + 1; 32],
                    ipv4_mtu: None,
                    ipv6_mtu: None,
                })
                .build();
            let serialized = router_info.serialize(&signing_key);
            let router_id = router_info.identity.id();

            Self {
                router_id,
                router_info,
                serialized,
                signing_key,
                intro_key: [seed + 1; 32],
                socket,
                static_key,
            }
        }
    }

    #[tokio::test]
    async fn relay_request_accepted() {
        let alice = TestRouter::new(0).await;
        let bob = TestRouter::new(2).await;
        let charlie = TestRouter::new(4).await;

        // we are bob
        let mut relay = RelayManager::<MockRuntime>::new(
            [0xab; 32],
            RouterContextBuilder::default()
                .with_router_info(
                    bob.router_info.clone(),
                    bob.static_key.clone(),
                    bob.signing_key.clone(),
                )
                .with_profile_storage({
                    let storage = ProfileStorage::new(&[], &[]);
                    storage.discover_router(
                        alice.router_info.clone(),
                        Bytes::from(alice.serialized.clone()),
                    );
                    storage
                })
                .build(),
            Some(bob.socket.clone()),
            None,
        );

        // add session for charlie
        let relay_tag = 1337;
        let nonce = 1338;

        let (alice_tx, alice_rx) = channel(16);
        let (charlie_tx, charlie_rx) = channel(16);
        relay.register_relay_client(charlie.router_id.clone(), 1337, charlie_tx);

        // create message + signature
        let (message, signature) = {
            let mut payload = BytesMut::with_capacity(128);
            payload.put_slice(b"RelayRequestData");
            payload.put_slice(&bob.router_id.to_vec());
            payload.put_slice(&charlie.router_id.to_vec());

            payload.put_u32(nonce);
            payload.put_u32(relay_tag);
            payload.put_u32(MockRuntime::time_since_epoch().as_secs() as u32);
            payload.put_u8(2);
            payload.put_u8(6);
            payload.put_u16(alice.socket.local_address().unwrap().port());

            match alice.socket.local_address().unwrap().ip() {
                IpAddr::V4(addr) => payload.put_slice(&addr.octets()),
                IpAddr::V6(addr) => payload.put_slice(&addr.octets()),
            }

            let signature = alice.signing_key.sign(&payload);

            (
                payload.split_off(b"RelayAgreementOK".len() + 2 * ROUTER_HASH_LEN).to_vec(),
                signature,
            )
        };

        // handle relay request from alice
        relay.handle_relay_request(
            alice.router_id.clone(),
            nonce,
            relay_tag,
            alice.socket.local_address().unwrap(),
            message.clone(),
            signature.clone(),
            alice_tx,
        );

        match charlie_rx.try_recv().unwrap() {
            RelayCommand::RelayIntro {
                router_id,
                router_info,
                message: intro_message,
                signature: intro_signature,
            } => {
                assert_eq!(router_id, alice.router_id.to_vec());
                assert_eq!(router_info, alice.serialized);
                assert_eq!(message, intro_message);
                assert_eq!(signature, intro_signature);
            }
            _ => panic!("invalid command"),
        }
        assert!(alice_rx.try_recv().is_err());
        assert!(relay.active_inbound.get(&nonce).is_some());
    }

    #[tokio::test]
    async fn relay_request_session_not_found() {
        let alice = TestRouter::new(0).await;
        let bob = TestRouter::new(2).await;
        let charlie = TestRouter::new(4).await;

        // we are bob
        let mut relay = RelayManager::<MockRuntime>::new(
            [0xab; 32],
            RouterContextBuilder::default()
                .with_router_info(
                    bob.router_info.clone(),
                    bob.static_key.clone(),
                    bob.signing_key.clone(),
                )
                .with_profile_storage({
                    let storage = ProfileStorage::new(&[], &[]);
                    storage.discover_router(
                        alice.router_info.clone(),
                        Bytes::from(alice.serialized.clone()),
                    );
                    storage
                })
                .build(),
            Some(bob.socket.clone()),
            None,
        );

        // do not create session for charlie
        let relay_tag = 1337;
        let nonce = 1338;
        let (alice_tx, alice_rx) = channel(16);

        // create message + signature
        let (message, signature) = {
            let mut payload = BytesMut::with_capacity(128);
            payload.put_slice(b"RelayRequestData");
            payload.put_slice(&bob.router_id.to_vec());
            payload.put_slice(&charlie.router_id.to_vec());

            payload.put_u32(nonce);
            payload.put_u32(relay_tag);
            payload.put_u32(MockRuntime::time_since_epoch().as_secs() as u32);
            payload.put_u8(2);
            payload.put_u8(6);
            payload.put_u16(alice.socket.local_address().unwrap().port());

            match alice.socket.local_address().unwrap().ip() {
                IpAddr::V4(addr) => payload.put_slice(&addr.octets()),
                IpAddr::V6(addr) => payload.put_slice(&addr.octets()),
            }

            let signature = alice.signing_key.sign(&payload);

            (
                payload.split_off(b"RelayAgreementOK".len() + 2 * ROUTER_HASH_LEN).to_vec(),
                signature,
            )
        };

        // handle relay request from alice
        relay.handle_relay_request(
            alice.router_id.clone(),
            nonce,
            relay_tag,
            alice.socket.local_address().unwrap(),
            message.clone(),
            signature.clone(),
            alice_tx,
        );

        match alice_rx.try_recv().unwrap() {
            RelayCommand::RelayResponse {
                nonce,
                rejection,
                message,
                token,
                ..
            } => {
                assert_eq!(nonce, 1338);
                assert_eq!(
                    rejection,
                    Some(RejectionReason::Bob(BobRejectionReason::RelayTagNotFound))
                );
                assert_eq!(token, None);

                let test_message = {
                    let mut message = BytesMut::with_capacity(58);
                    message.put_slice(b"RelayAgreementOK");
                    message.put_slice(&bob.router_id.to_vec());
                    message.put_u32(1338);
                    message.put_u32(MockRuntime::time_since_epoch().as_secs() as u32);
                    message.put_u8(2); // version
                    message.put_u8(0u8); // address size

                    message.split_off(b"RelayAgreementOK".len() + ROUTER_HASH_LEN).to_vec()
                };
                assert_eq!(message, test_message);
            }
            _ => panic!("invalid command"),
        }
        assert!(relay.active_inbound.get(&nonce).is_none());
    }

    #[tokio::test]
    async fn relay_request_alice_not_found() {
        let alice = TestRouter::new(0).await;
        let bob = TestRouter::new(2).await;
        let charlie = TestRouter::new(4).await;

        // we are bob
        //
        // do not add alice to router storage
        let mut relay = RelayManager::<MockRuntime>::new(
            [0xab; 32],
            RouterContextBuilder::default()
                .with_router_info(
                    bob.router_info.clone(),
                    bob.static_key.clone(),
                    bob.signing_key.clone(),
                )
                .build(),
            Some(bob.socket.clone()),
            None,
        );

        // add session for charlie
        let relay_tag = 1337;
        let nonce = 1338;

        let (alice_tx, alice_rx) = channel(16);
        let (charlie_tx, charlie_rx) = channel(16);
        relay.register_relay_client(charlie.router_id.clone(), 1337, charlie_tx);

        // create message + signature
        let (message, signature) = {
            let mut payload = BytesMut::with_capacity(128);
            payload.put_slice(b"RelayRequestData");
            payload.put_slice(&bob.router_id.to_vec());
            payload.put_slice(&charlie.router_id.to_vec());

            payload.put_u32(nonce);
            payload.put_u32(relay_tag);
            payload.put_u32(MockRuntime::time_since_epoch().as_secs() as u32);
            payload.put_u8(2);
            payload.put_u8(6);
            payload.put_u16(alice.socket.local_address().unwrap().port());

            match alice.socket.local_address().unwrap().ip() {
                IpAddr::V4(addr) => payload.put_slice(&addr.octets()),
                IpAddr::V6(addr) => payload.put_slice(&addr.octets()),
            }

            let signature = alice.signing_key.sign(&payload);

            (
                payload.split_off(b"RelayAgreementOK".len() + 2 * ROUTER_HASH_LEN).to_vec(),
                signature,
            )
        };

        // handle relay request from alice
        relay.handle_relay_request(
            alice.router_id.clone(),
            nonce,
            relay_tag,
            alice.socket.local_address().unwrap(),
            message.clone(),
            signature.clone(),
            alice_tx,
        );

        match alice_rx.try_recv().unwrap() {
            RelayCommand::RelayResponse {
                nonce,
                rejection,
                message,
                token,
                ..
            } => {
                assert_eq!(nonce, 1338);
                assert_eq!(
                    rejection,
                    Some(RejectionReason::Bob(BobRejectionReason::AliceNotFound))
                );
                assert_eq!(token, None);

                let test_message = {
                    let mut message = BytesMut::with_capacity(58);
                    message.put_slice(b"RelayAgreementOK");
                    message.put_slice(&bob.router_id.to_vec());
                    message.put_u32(1338);
                    message.put_u32(MockRuntime::time_since_epoch().as_secs() as u32);
                    message.put_u8(2); // version
                    message.put_u8(0u8); // address size

                    message.split_off(b"RelayAgreementOK".len() + ROUTER_HASH_LEN).to_vec()
                };
                assert_eq!(message, test_message);
            }
            _ => panic!("invalid command"),
        }
        assert!(charlie_rx.try_recv().is_err());
        assert!(relay.active_inbound.get(&nonce).is_none());
    }

    #[tokio::test]
    async fn relay_request_invalid_signature() {
        let alice = TestRouter::new(0).await;
        let bob = TestRouter::new(2).await;
        let charlie = TestRouter::new(4).await;

        // we are bob
        let mut relay = RelayManager::<MockRuntime>::new(
            [0xab; 32],
            RouterContextBuilder::default()
                .with_profile_storage({
                    let storage = ProfileStorage::new(&[], &[]);
                    storage.discover_router(
                        alice.router_info.clone(),
                        Bytes::from(alice.serialized.clone()),
                    );
                    storage
                })
                .with_router_info(
                    bob.router_info.clone(),
                    bob.static_key.clone(),
                    bob.signing_key.clone(),
                )
                .build(),
            Some(bob.socket.clone()),
            None,
        );

        // add session for charlie
        let relay_tag = 1337;
        let nonce = 1338;

        let (alice_tx, alice_rx) = channel(16);
        let (charlie_tx, charlie_rx) = channel(16);
        relay.register_relay_client(charlie.router_id.clone(), 1337, charlie_tx);

        // create message + signature
        let (message, signature) = {
            let mut payload = BytesMut::with_capacity(128);
            payload.put_slice(b"RelayRequestData");
            payload.put_slice(&bob.router_id.to_vec());
            payload.put_slice(&charlie.router_id.to_vec());

            payload.put_u32(nonce);
            payload.put_u32(relay_tag);
            payload.put_u32(MockRuntime::time_since_epoch().as_secs() as u32);
            payload.put_u8(2);
            payload.put_u8(6);
            payload.put_u16(alice.socket.local_address().unwrap().port());

            match alice.socket.local_address().unwrap().ip() {
                IpAddr::V4(addr) => payload.put_slice(&addr.octets()),
                IpAddr::V6(addr) => payload.put_slice(&addr.octets()),
            }

            // create invalid signature
            let signature = alice.signing_key.sign(&payload[..15]);

            (
                payload.split_off(b"RelayAgreementOK".len() + 2 * ROUTER_HASH_LEN).to_vec(),
                signature,
            )
        };

        // handle relay request from alice
        relay.handle_relay_request(
            alice.router_id.clone(),
            nonce,
            relay_tag,
            alice.socket.local_address().unwrap(),
            message.clone(),
            signature.clone(),
            alice_tx,
        );

        match alice_rx.try_recv().unwrap() {
            RelayCommand::RelayResponse {
                nonce,
                rejection,
                message,
                token,
                ..
            } => {
                assert_eq!(nonce, 1338);
                assert_eq!(
                    rejection,
                    Some(RejectionReason::Bob(BobRejectionReason::SignatureFailure))
                );
                assert_eq!(token, None);

                let test_message = {
                    let mut message = BytesMut::with_capacity(58);
                    message.put_slice(b"RelayAgreementOK");
                    message.put_slice(&bob.router_id.to_vec());
                    message.put_u32(1338);
                    message.put_u32(MockRuntime::time_since_epoch().as_secs() as u32);
                    message.put_u8(2); // version
                    message.put_u8(0u8); // address size

                    message.split_off(b"RelayAgreementOK".len() + ROUTER_HASH_LEN).to_vec()
                };
                assert_eq!(message, test_message);
            }
            _ => panic!("invalid command"),
        }
        assert!(charlie_rx.try_recv().is_err());
        assert!(relay.active_inbound.get(&nonce).is_none());
    }

    #[tokio::test]
    async fn relay_intro_accepted() {
        let mut alice = TestRouter::new(0).await;
        let bob = TestRouter::new(2).await;
        let charlie = TestRouter::new(4).await;

        // we are charlie
        let mut relay = RelayManager::<MockRuntime>::new(
            [0xab; 32],
            RouterContextBuilder::default()
                .with_router_info(
                    charlie.router_info.clone(),
                    charlie.static_key.clone(),
                    charlie.signing_key.clone(),
                )
                .with_profile_storage({
                    let storage = ProfileStorage::new(&[], &[]);
                    storage.discover_router(
                        alice.router_info.clone(),
                        Bytes::from(alice.serialized.clone()),
                    );
                    storage
                })
                .build(),
            Some(charlie.socket.clone()),
            None,
        );

        // create context for the test
        let relay_tag = 1337;
        let nonce = 1338;
        let (bob_tx, bob_rx) = channel(16);

        // add external address for charlie
        relay.add_external_address(charlie.socket.local_address().unwrap());

        // register bob as relay server
        relay.register_relay_server(bob.router_id.clone(), relay_tag, true);

        // create message + signature
        //
        // created by alice
        let (message, signature) = {
            let mut payload = BytesMut::with_capacity(128);
            payload.put_slice(b"RelayRequestData");
            payload.put_slice(&bob.router_id.to_vec());
            payload.put_slice(&charlie.router_id.to_vec());

            payload.put_u32(nonce);
            payload.put_u32(relay_tag);
            payload.put_u32(MockRuntime::time_since_epoch().as_secs() as u32);
            payload.put_u8(2);
            payload.put_u8(6);
            payload.put_u16(alice.socket.local_address().unwrap().port());

            match alice.socket.local_address().unwrap().ip() {
                IpAddr::V4(addr) => payload.put_slice(&addr.octets()),
                IpAddr::V6(addr) => payload.put_slice(&addr.octets()),
            }

            let signature = alice.signing_key.sign(&payload);

            (
                payload.split_off(b"RelayAgreementOK".len() + 2 * ROUTER_HASH_LEN).to_vec(),
                signature,
            )
        };

        // handle relay intro from bob
        relay.handle_relay_intro(
            alice.router_id.clone(),
            bob.router_id.clone(),
            None,
            nonce,
            relay_tag,
            alice.socket.local_address().unwrap(),
            message,
            signature,
            bob_tx,
        );

        match bob_rx.try_recv().unwrap() {
            RelayCommand::RelayResponse {
                nonce,
                rejection,
                token,
                ..
            } => {
                assert_eq!(nonce, 1338);
                assert_eq!(rejection, None);
                assert_ne!(token, None);
            }
            _ => panic!("invalid command"),
        }

        // verify the relay manager returns a token for `SessionRequest`
        let Some(RelayManagerEvent::SessionRequestToken {
            token: session_request_token,
        }) = relay.next().now_or_never().unwrap()
        else {
            panic!("invalid event")
        };

        // spawn manager in the background so the hole punch message gets sent eventually
        tokio::spawn(async move { while relay.next().await.is_some() {} });

        // read hole punch message from alice's socket
        let mut buf = vec![0u8; 1500];
        let (nread, _from) = timeout!(alice.socket.recv_from(&mut buf)).await.unwrap().unwrap();
        let mut pkt = buf[..nread].to_vec();
        let mut reader = HeaderReader::new(alice.intro_key, &mut pkt).unwrap();
        let _dst_id = reader.dst_id();

        let (pkt_num, src_id) = match reader.parse(alice.intro_key).unwrap() {
            HeaderKind::HolePunch {
                pkt_num, src_id, ..
            } => (pkt_num, src_id),
            _ => panic!("invalid header kind"),
        };
        let ad = pkt[..32].to_vec();
        let mut pkt = pkt[32..].to_vec();

        assert_eq!(src_id, (!(((1338u64) << 32) | (1338u64))).to_be());

        ChaChaPoly::with_nonce(&alice.intro_key, pkt_num as u64)
            .decrypt_with_ad(&ad, &mut pkt)
            .unwrap();

        assert!(
            Block::parse::<MockRuntime>(&pkt).unwrap().iter().any(|block| match block {
                Block::RelayResponse { token, .. } => token == &Some(session_request_token),
                _ => false,
            })
        );
    }

    #[tokio::test]
    async fn relay_intro_with_router_info() {
        let mut alice = TestRouter::new(0).await;
        let bob = TestRouter::new(2).await;
        let charlie = TestRouter::new(4).await;

        // we are charlie
        //
        // alice is not found in router storage but their router info
        // is provided in a router info block in-session
        let mut relay = RelayManager::<MockRuntime>::new(
            [0xab; 32],
            RouterContextBuilder::default()
                .with_router_info(
                    charlie.router_info.clone(),
                    charlie.static_key.clone(),
                    charlie.signing_key.clone(),
                )
                .build(),
            Some(charlie.socket.clone()),
            None,
        );

        // create context for the test
        let relay_tag = 1337;
        let nonce = 1338;
        let (bob_tx, bob_rx) = channel(16);

        // add external address for charlie
        relay.add_external_address(charlie.socket.local_address().unwrap());

        // register bob as relay server
        relay.register_relay_server(bob.router_id.clone(), relay_tag, true);

        // create message + signature
        //
        // created by alice
        let (message, signature) = {
            let mut payload = BytesMut::with_capacity(128);
            payload.put_slice(b"RelayRequestData");
            payload.put_slice(&bob.router_id.to_vec());
            payload.put_slice(&charlie.router_id.to_vec());

            payload.put_u32(nonce);
            payload.put_u32(relay_tag);
            payload.put_u32(MockRuntime::time_since_epoch().as_secs() as u32);
            payload.put_u8(2);
            payload.put_u8(6);
            payload.put_u16(alice.socket.local_address().unwrap().port());

            match alice.socket.local_address().unwrap().ip() {
                IpAddr::V4(addr) => payload.put_slice(&addr.octets()),
                IpAddr::V6(addr) => payload.put_slice(&addr.octets()),
            }

            let signature = alice.signing_key.sign(&payload);

            (
                payload.split_off(b"RelayAgreementOK".len() + 2 * ROUTER_HASH_LEN).to_vec(),
                signature,
            )
        };

        // handle relay intro from bob
        relay.handle_relay_intro(
            alice.router_id.clone(),
            bob.router_id.clone(),
            Some(Box::new(alice.router_info.clone())),
            nonce,
            relay_tag,
            alice.socket.local_address().unwrap(),
            message,
            signature,
            bob_tx,
        );

        match bob_rx.try_recv().unwrap() {
            RelayCommand::RelayResponse {
                nonce,
                rejection,
                token,
                ..
            } => {
                assert_eq!(nonce, 1338);
                assert_eq!(rejection, None);
                assert_ne!(token, None);
            }
            _ => panic!("invalid command"),
        }

        // verify the relay manager returns a token for `SessionRequest`
        let Some(RelayManagerEvent::SessionRequestToken {
            token: session_request_token,
        }) = relay.next().now_or_never().unwrap()
        else {
            panic!("invalid event")
        };

        // spawn manager in the background so the hole punch message gets sent eventually
        tokio::spawn(async move { while relay.next().await.is_some() {} });

        // read hole punch message from alice's socket
        let mut buf = vec![0u8; 1500];
        let (nread, _from) = timeout!(alice.socket.recv_from(&mut buf)).await.unwrap().unwrap();
        let mut pkt = buf[..nread].to_vec();
        let mut reader = HeaderReader::new(alice.intro_key, &mut pkt).unwrap();
        let _dst_id = reader.dst_id();

        let (pkt_num, src_id) = match reader.parse(alice.intro_key).unwrap() {
            HeaderKind::HolePunch {
                pkt_num, src_id, ..
            } => (pkt_num, src_id),
            _ => panic!("invalid header kind"),
        };
        let ad = pkt[..32].to_vec();
        let mut pkt = pkt[32..].to_vec();

        assert_eq!(src_id, (!(((1338u64) << 32) | (1338u64))).to_be());

        ChaChaPoly::with_nonce(&alice.intro_key, pkt_num as u64)
            .decrypt_with_ad(&ad, &mut pkt)
            .unwrap();

        assert!(
            Block::parse::<MockRuntime>(&pkt).unwrap().iter().any(|block| match block {
                Block::RelayResponse { token, .. } => token == &Some(session_request_token),
                _ => false,
            })
        );
    }

    #[tokio::test]
    async fn relay_intro_relay_server_not_found() {
        let alice = TestRouter::new(0).await;
        let bob = TestRouter::new(2).await;
        let charlie = TestRouter::new(4).await;

        // we are charlie
        let mut relay = RelayManager::<MockRuntime>::new(
            [0xab; 32],
            RouterContextBuilder::default()
                .with_router_info(
                    charlie.router_info.clone(),
                    charlie.static_key.clone(),
                    charlie.signing_key.clone(),
                )
                .build(),
            Some(charlie.socket.clone()),
            None,
        );

        // create context for the test
        let relay_tag = 1337;
        let nonce = 1338;
        let (bob_tx, bob_rx) = channel(16);

        // add external address for charlie
        relay.add_external_address(charlie.socket.local_address().unwrap());

        // bob is not registered as relay server so the relay into gets rejected

        // create message + signature
        //
        // created by alice
        let (message, signature) = {
            let mut payload = BytesMut::with_capacity(128);
            payload.put_slice(b"RelayRequestData");
            payload.put_slice(&bob.router_id.to_vec());
            payload.put_slice(&charlie.router_id.to_vec());

            payload.put_u32(nonce);
            payload.put_u32(relay_tag);
            payload.put_u32(MockRuntime::time_since_epoch().as_secs() as u32);
            payload.put_u8(2);
            payload.put_u8(6);
            payload.put_u16(alice.socket.local_address().unwrap().port());

            match alice.socket.local_address().unwrap().ip() {
                IpAddr::V4(addr) => payload.put_slice(&addr.octets()),
                IpAddr::V6(addr) => payload.put_slice(&addr.octets()),
            }

            let signature = alice.signing_key.sign(&payload);

            (
                payload.split_off(b"RelayAgreementOK".len() + 2 * ROUTER_HASH_LEN).to_vec(),
                signature,
            )
        };

        // handle relay intro from bob
        relay.handle_relay_intro(
            alice.router_id.clone(),
            bob.router_id.clone(),
            Some(Box::new(alice.router_info.clone())),
            nonce,
            relay_tag,
            alice.socket.local_address().unwrap(),
            message,
            signature,
            bob_tx,
        );

        match bob_rx.try_recv().unwrap() {
            RelayCommand::RelayResponse {
                nonce,
                rejection,
                token,
                ..
            } => {
                assert_eq!(nonce, 1338);
                assert_eq!(token, None);
                assert_eq!(
                    rejection,
                    Some(RejectionReason::Charlie(
                        CharlieRejectionReason::Unspecified
                    ))
                );
            }
            _ => panic!("invalid command"),
        }
    }

    #[tokio::test]
    async fn relay_intro_alice_not_found() {
        let alice = TestRouter::new(0).await;
        let bob = TestRouter::new(2).await;
        let charlie = TestRouter::new(4).await;

        // we are charlie
        //
        // alice is not found in router storage
        let mut relay = RelayManager::<MockRuntime>::new(
            [0xab; 32],
            RouterContextBuilder::default()
                .with_router_info(
                    charlie.router_info.clone(),
                    charlie.static_key.clone(),
                    charlie.signing_key.clone(),
                )
                .build(),
            Some(charlie.socket.clone()),
            None,
        );

        // create context for the test
        let relay_tag = 1337;
        let nonce = 1338;
        let (bob_tx, bob_rx) = channel(16);

        // add external address for charlie
        relay.add_external_address(charlie.socket.local_address().unwrap());

        // register bob as relay server
        relay.register_relay_server(bob.router_id.clone(), relay_tag, true);

        // create message + signature
        //
        // created by alice
        let (message, signature) = {
            let mut payload = BytesMut::with_capacity(128);
            payload.put_slice(b"RelayRequestData");
            payload.put_slice(&bob.router_id.to_vec());
            payload.put_slice(&charlie.router_id.to_vec());

            payload.put_u32(nonce);
            payload.put_u32(relay_tag);
            payload.put_u32(MockRuntime::time_since_epoch().as_secs() as u32);
            payload.put_u8(2);
            payload.put_u8(6);
            payload.put_u16(alice.socket.local_address().unwrap().port());

            match alice.socket.local_address().unwrap().ip() {
                IpAddr::V4(addr) => payload.put_slice(&addr.octets()),
                IpAddr::V6(addr) => payload.put_slice(&addr.octets()),
            }

            let signature = alice.signing_key.sign(&payload);

            (
                payload.split_off(b"RelayAgreementOK".len() + 2 * ROUTER_HASH_LEN).to_vec(),
                signature,
            )
        };

        // handle relay intro from bob
        //
        // alice's router info was not sent in-session
        // so charlie doesn't know who alice is
        relay.handle_relay_intro(
            alice.router_id.clone(),
            bob.router_id.clone(),
            None,
            nonce,
            relay_tag,
            alice.socket.local_address().unwrap(),
            message,
            signature,
            bob_tx,
        );

        match bob_rx.try_recv().unwrap() {
            RelayCommand::RelayResponse {
                nonce,
                rejection,
                token,
                ..
            } => {
                assert_eq!(nonce, 1338);
                assert_eq!(token, None);
                assert_eq!(
                    rejection,
                    Some(RejectionReason::Charlie(
                        CharlieRejectionReason::AliceNotFound
                    ))
                );
            }
            _ => panic!("invalid command"),
        }
    }

    #[tokio::test]
    async fn relay_intro_invalid_signature() {
        let alice = TestRouter::new(0).await;
        let bob = TestRouter::new(2).await;
        let charlie = TestRouter::new(4).await;

        // we are charlie
        let mut relay = RelayManager::<MockRuntime>::new(
            [0xab; 32],
            RouterContextBuilder::default()
                .with_router_info(
                    charlie.router_info.clone(),
                    charlie.static_key.clone(),
                    charlie.signing_key.clone(),
                )
                .build(),
            Some(charlie.socket.clone()),
            None,
        );

        // create context for the test
        let relay_tag = 1337;
        let nonce = 1338;
        let (bob_tx, bob_rx) = channel(16);

        // add external address for charlie
        relay.add_external_address(charlie.socket.local_address().unwrap());

        // register bob as relay server
        relay.register_relay_server(bob.router_id.clone(), relay_tag, true);

        // create message + signature
        //
        // created by alice
        let (message, signature) = {
            let mut payload = BytesMut::with_capacity(128);
            payload.put_slice(b"RelayRequestData");
            payload.put_slice(&bob.router_id.to_vec());
            payload.put_slice(&charlie.router_id.to_vec());

            payload.put_u32(nonce);
            payload.put_u32(relay_tag);
            payload.put_u32(MockRuntime::time_since_epoch().as_secs() as u32);
            payload.put_u8(2);
            payload.put_u8(6);
            payload.put_u16(alice.socket.local_address().unwrap().port());

            match alice.socket.local_address().unwrap().ip() {
                IpAddr::V4(addr) => payload.put_slice(&addr.octets()),
                IpAddr::V6(addr) => payload.put_slice(&addr.octets()),
            }

            // create invalid signature
            let signature = alice.signing_key.sign(&payload[..16]);

            (
                payload.split_off(b"RelayAgreementOK".len() + 2 * ROUTER_HASH_LEN).to_vec(),
                signature,
            )
        };

        // handle relay intro from bob
        relay.handle_relay_intro(
            alice.router_id.clone(),
            bob.router_id.clone(),
            Some(Box::new(alice.router_info.clone())),
            nonce,
            relay_tag,
            alice.socket.local_address().unwrap(),
            message,
            signature,
            bob_tx,
        );

        match bob_rx.try_recv().unwrap() {
            RelayCommand::RelayResponse {
                nonce,
                rejection,
                token,
                ..
            } => {
                assert_eq!(nonce, 1338);
                assert_eq!(token, None);
                assert_eq!(
                    rejection,
                    Some(RejectionReason::Charlie(
                        CharlieRejectionReason::SignatureFailure
                    ))
                );
            }
            _ => panic!("invalid command"),
        }
    }

    #[tokio::test]
    async fn relay_intro_no_external_address() {
        let alice = TestRouter::new(0).await;
        let bob = TestRouter::new(2).await;
        let charlie = TestRouter::new(4).await;

        // we are charlie
        let mut relay = RelayManager::<MockRuntime>::new(
            [0xab; 32],
            RouterContextBuilder::default()
                .with_router_info(
                    charlie.router_info.clone(),
                    charlie.static_key.clone(),
                    charlie.signing_key.clone(),
                )
                .build(),
            Some(charlie.socket.clone()),
            None,
        );

        // create context for the test
        let relay_tag = 1337;
        let nonce = 1338;
        let (bob_tx, bob_rx) = channel(16);

        // charlie doesn't have external address so the intro gets rejected

        // register bob as relay server
        relay.register_relay_server(bob.router_id.clone(), relay_tag, true);

        // create message + signature
        //
        // created by alice
        let (message, signature) = {
            let mut payload = BytesMut::with_capacity(128);
            payload.put_slice(b"RelayRequestData");
            payload.put_slice(&bob.router_id.to_vec());
            payload.put_slice(&charlie.router_id.to_vec());

            payload.put_u32(nonce);
            payload.put_u32(relay_tag);
            payload.put_u32(MockRuntime::time_since_epoch().as_secs() as u32);
            payload.put_u8(2);
            payload.put_u8(6);
            payload.put_u16(alice.socket.local_address().unwrap().port());

            match alice.socket.local_address().unwrap().ip() {
                IpAddr::V4(addr) => payload.put_slice(&addr.octets()),
                IpAddr::V6(addr) => payload.put_slice(&addr.octets()),
            }

            let signature = alice.signing_key.sign(&payload);

            (
                payload.split_off(b"RelayAgreementOK".len() + 2 * ROUTER_HASH_LEN).to_vec(),
                signature,
            )
        };

        // handle relay intro from bob
        relay.handle_relay_intro(
            alice.router_id.clone(),
            bob.router_id.clone(),
            Some(Box::new(alice.router_info.clone())),
            nonce,
            relay_tag,
            alice.socket.local_address().unwrap(),
            message,
            signature,
            bob_tx,
        );

        match bob_rx.try_recv().unwrap() {
            RelayCommand::RelayResponse {
                nonce,
                rejection,
                token,
                ..
            } => {
                assert_eq!(nonce, 1338);
                assert_eq!(token, None);
                assert_eq!(
                    rejection,
                    Some(RejectionReason::Charlie(
                        CharlieRejectionReason::Unspecified
                    ))
                );
            }
            _ => panic!("invalid command"),
        }
    }

    #[tokio::test]
    async fn relay_response_accepted() {
        let bob = TestRouter::new(2).await;

        // we are bob
        let mut relay = RelayManager::<MockRuntime>::new(
            [0xab; 32],
            RouterContextBuilder::default()
                .with_router_info(
                    bob.router_info.clone(),
                    bob.static_key.clone(),
                    bob.signing_key.clone(),
                )
                .build(),
            Some(bob.socket.clone()),
            None,
        );

        // create context for the test
        let nonce = 1338;
        let (alice_tx, alice_rx) = channel(16);

        // create new active relay manually
        relay.active_inbound.insert(nonce, alice_tx);

        // handle relay response for an active session
        relay.handle_relay_response(
            nonce,
            None,
            None,
            Some(RejectionReason::Charlie(
                CharlieRejectionReason::Unspecified,
            )),
            vec![],
            vec![],
        );

        // verify the message is routed to alice
        match alice_rx.try_recv().unwrap() {
            RelayCommand::RelayResponse {
                nonce,
                rejection,
                message,
                signature,
                token,
            } => {
                assert_eq!(nonce, 1338);
                assert_eq!(
                    rejection,
                    Some(RejectionReason::Charlie(
                        CharlieRejectionReason::Unspecified,
                    ))
                );
                assert_eq!(message, vec![]);
                assert_eq!(signature, vec![]);
                assert_eq!(token, None);
            }
            _ => panic!("invalid command"),
        }
        assert!(!relay.active_inbound.contains_key(&nonce));
    }

    #[tokio::test]
    async fn relay_response_no_session() {
        let bob = TestRouter::new(2).await;

        // we are bob
        let mut relay = RelayManager::<MockRuntime>::new(
            [0xab; 32],
            RouterContextBuilder::default()
                .with_router_info(
                    bob.router_info.clone(),
                    bob.static_key.clone(),
                    bob.signing_key.clone(),
                )
                .build(),
            Some(bob.socket.clone()),
            None,
        );

        // create context for the test
        let nonce = 1338;

        // don't insert active session for `nonce`

        // handle relay response for an unknown session
        relay.handle_relay_response(
            nonce,
            None,
            None,
            Some(RejectionReason::Charlie(
                CharlieRejectionReason::Unspecified,
            )),
            vec![],
            vec![],
        );
        assert!(!relay.active_inbound.contains_key(&nonce));
    }

    #[tokio::test]
    async fn bob_and_charlie_accept() {
        let mut alice = TestRouter::new(0).await;
        let bob = TestRouter::new(2).await;
        let charlie = TestRouter::with_intorducer(4, bob.router_id.clone(), 1337).await;

        // create alice relay manager
        let mut alice_relay = RelayManager::<MockRuntime>::new(
            alice.intro_key,
            RouterContextBuilder::default()
                .with_router_info(
                    alice.router_info.clone(),
                    alice.static_key.clone(),
                    alice.signing_key.clone(),
                )
                .build(),
            Some(alice.socket.clone()),
            None,
        );

        // add bob as a router that supports relay
        let (alice_bob_tx, alice_bob_rx) = channel(16);
        alice_relay.add_session(&bob.router_id, alice_bob_tx, true);

        // create relay manager for bob
        let mut bob_relay = RelayManager::<MockRuntime>::new(
            bob.intro_key,
            RouterContextBuilder::default()
                .with_router_info(
                    bob.router_info.clone(),
                    bob.static_key.clone(),
                    bob.signing_key.clone(),
                )
                .build(),
            Some(bob.socket.clone()),
            None,
        );

        // add alice to bob's profile storage so bob can forward
        // alice's router info to charlie
        bob_relay.router_ctx.profile_storage().discover_router(
            alice.router_info.clone(),
            Bytes::from(alice.serialized.clone()),
        );

        // add charlie as relay client
        let (bob_charlie_tx, bob_charlie_rx) = channel(16);
        bob_relay.register_relay_client(charlie.router_id.clone(), 1337, bob_charlie_tx);

        // create relay manager for charlie
        let mut charlie_relay = RelayManager::<MockRuntime>::new(
            charlie.intro_key,
            RouterContextBuilder::default()
                .with_router_info(
                    charlie.router_info.clone(),
                    charlie.static_key.clone(),
                    charlie.signing_key.clone(),
                )
                .build(),
            Some(charlie.socket.clone()),
            None,
        );

        // register bob as relay server
        charlie_relay.register_relay_server(bob.router_id.clone(), 1337, true);
        charlie_relay.add_external_address(charlie.socket.local_address().unwrap());

        // send relay request to charlie via bob
        let RelayConnection { .. } =
            alice_relay.send_relay_request(charlie.parsed(), false).unwrap();

        let (nonce, message, signature) = match alice_bob_rx.try_recv().unwrap() {
            RelayCommand::RelayRequest {
                nonce,
                message,
                signature,
            } => (nonce, message, signature),
            _ => panic!("unexpected command"),
        };

        // handle relay request as bob
        let (bob_alice_tx, bob_alice_rx) = channel(16);
        bob_relay.handle_relay_request(
            alice.router_id.clone(),
            nonce,
            1337,
            alice.socket.local_address().unwrap(),
            message,
            signature,
            bob_alice_tx,
        );

        // verify charlie is sent relay intro
        let (router_info, message, signature) = match bob_charlie_rx.try_recv().unwrap() {
            RelayCommand::RelayIntro {
                router_info,
                message,
                signature,
                ..
            } => (router_info, message, signature),
            _ => panic!("unexpected command"),
        };

        // handle relay intro as charlie
        let (charlie_bob_tx, charlie_bob_rx) = channel(16);
        charlie_relay.handle_relay_intro(
            alice.router_id.clone(),
            bob.router_id.clone(),
            Some(Box::new(
                RouterInfo::parse::<MockRuntime>(router_info).unwrap(),
            )),
            nonce,
            1337,
            alice.socket.local_address().unwrap(),
            message,
            signature,
            charlie_bob_tx,
        );

        // spawn charlie in the background so the holepunch message gets sent
        //
        // the channel is used to receive the token charlie generated
        let (token_tx, token_rx) = oneshot::channel();
        let mut token_tx = Some(token_tx);

        tokio::spawn(async move {
            while let Some(event) = charlie_relay.next().await {
                match event {
                    RelayManagerEvent::SessionRequestToken { token } => {
                        if let Some(tx) = token_tx.take() {
                            let _ = tx.send(token);
                        }
                    }
                    _ => {}
                }
            }
        });

        // read charlie's response from bob's channel
        let (rejection, message, signature, token) = match charlie_bob_rx.try_recv().unwrap() {
            RelayCommand::RelayResponse {
                rejection,
                message,
                signature,
                token,
                ..
            } => (rejection, message, signature, token),
            _ => panic!("unexpected event"),
        };

        // handle charlie's response as bob
        bob_relay.handle_relay_response(
            nonce,
            Some(charlie.socket.local_address().unwrap()),
            token,
            rejection,
            message,
            signature,
        );

        // read bob's response from alice's channel
        let (rejection, message, signature, token) = match bob_alice_rx.try_recv().unwrap() {
            RelayCommand::RelayResponse {
                rejection,
                message,
                signature,
                token,
                ..
            } => (rejection, message, signature, token),
            _ => panic!("unexpected event"),
        };

        // handle response as alice
        alice_relay.handle_relay_response(
            nonce,
            Some(charlie.socket.local_address().unwrap()),
            token,
            rejection,
            message,
            signature,
        );

        let token = loop {
            match alice_relay.next().await.unwrap() {
                RelayManagerEvent::RelaySuccess { token, .. } => break token,
                _ => {}
            }
        };

        assert_eq!(timeout!(token_rx).await.unwrap().unwrap(), token);

        // read hole punch message from alice's socket
        let mut buf = vec![0u8; 1500];
        let (nread, _from) = timeout!(alice.socket.recv_from(&mut buf)).await.unwrap().unwrap();
        let mut pkt = buf[..nread].to_vec();
        let mut reader = HeaderReader::new(alice.intro_key, &mut pkt).unwrap();
        let _dst_id = reader.dst_id();

        let (pkt_num, src_id) = match reader.parse(alice.intro_key).unwrap() {
            HeaderKind::HolePunch {
                pkt_num, src_id, ..
            } => (pkt_num, src_id),
            _ => panic!("invalid header kind"),
        };
        let ad = pkt[..32].to_vec();
        let mut pkt = pkt[32..].to_vec();

        assert_eq!(src_id, (!(((nonce as u64) << 32) | (nonce as u64))).to_be());

        ChaChaPoly::with_nonce(&alice.intro_key, pkt_num as u64)
            .decrypt_with_ad(&ad, &mut pkt)
            .unwrap();

        assert!(
            Block::parse::<MockRuntime>(&pkt).unwrap().iter().any(|block| match block {
                Block::RelayResponse {
                    token: ses_req_token,
                    ..
                } => ses_req_token == &Some(token),
                _ => false,
            })
        );
    }

    #[tokio::test]
    async fn bob_rejects_relay_tag_not_found() {
        let alice = TestRouter::new(0).await;
        let bob = TestRouter::new(2).await;
        let charlie = TestRouter::with_intorducer(4, bob.router_id.clone(), 1337).await;

        // create alice relay manager
        let mut alice_relay = RelayManager::<MockRuntime>::new(
            alice.intro_key,
            RouterContextBuilder::default()
                .with_router_info(
                    alice.router_info.clone(),
                    alice.static_key.clone(),
                    alice.signing_key.clone(),
                )
                .build(),
            Some(alice.socket.clone()),
            None,
        );

        // add bob as a router that supports relay
        let (alice_bob_tx, alice_bob_rx) = channel(16);
        alice_relay.add_session(&bob.router_id, alice_bob_tx, true);

        // create relay manager for bob
        let mut bob_relay = RelayManager::<MockRuntime>::new(
            bob.intro_key,
            RouterContextBuilder::default()
                .with_router_info(
                    bob.router_info.clone(),
                    bob.static_key.clone(),
                    bob.signing_key.clone(),
                )
                .build(),
            Some(bob.socket.clone()),
            None,
        );

        // add alice to bob's profile storage so bob can forward
        // alice's router info to charlie
        bob_relay.router_ctx.profile_storage().discover_router(
            alice.router_info.clone(),
            Bytes::from(alice.serialized.clone()),
        );

        // add charlie as relay client
        let (bob_charlie_tx, _bob_charlie_rx) = channel(16);
        bob_relay.register_relay_client(charlie.router_id.clone(), 1337, bob_charlie_tx);

        // send relay request to charlie via bob
        let RelayConnection { .. } =
            alice_relay.send_relay_request(charlie.parsed(), false).unwrap();

        let (nonce, message, signature) = match alice_bob_rx.try_recv().unwrap() {
            RelayCommand::RelayRequest {
                nonce,
                message,
                signature,
            } => (nonce, message, signature),
            _ => panic!("unexpected command"),
        };

        // handle relay request as bob
        let (bob_alice_tx, bob_alice_rx) = channel(16);
        bob_relay.handle_relay_request(
            alice.router_id.clone(),
            nonce,
            1338, // unknown relay tag
            alice.socket.local_address().unwrap(),
            message,
            signature,
            bob_alice_tx,
        );

        // verify charlie is sent relay intro
        let (rejection, message, signature, token) = match bob_alice_rx.try_recv().unwrap() {
            RelayCommand::RelayResponse {
                rejection,
                message,
                signature,
                token,
                ..
            } => (rejection, message, signature, token),
            _ => panic!("unexpected command"),
        };
        assert_eq!(
            rejection,
            Some(RejectionReason::Bob(BobRejectionReason::RelayTagNotFound))
        );

        // handle response as alice
        alice_relay.handle_relay_response(nonce, None, token, rejection, message, signature);

        match alice_relay.next().await.unwrap() {
            RelayManagerEvent::RelayFailure { router_id } =>
                assert_eq!(router_id, charlie.router_id),
            _ => {}
        }
    }

    #[tokio::test]
    async fn bob_rejects_alice_not_found() {
        let alice = TestRouter::new(0).await;
        let bob = TestRouter::new(2).await;
        let charlie = TestRouter::with_intorducer(4, bob.router_id.clone(), 1337).await;

        // create alice relay manager
        let mut alice_relay = RelayManager::<MockRuntime>::new(
            alice.intro_key,
            RouterContextBuilder::default()
                .with_router_info(
                    alice.router_info.clone(),
                    alice.static_key.clone(),
                    alice.signing_key.clone(),
                )
                .build(),
            Some(alice.socket.clone()),
            None,
        );

        // add bob as a router that supports relay
        let (alice_bob_tx, alice_bob_rx) = channel(16);
        alice_relay.add_session(&bob.router_id, alice_bob_tx, true);

        // create relay manager for bob
        let mut bob_relay = RelayManager::<MockRuntime>::new(
            bob.intro_key,
            RouterContextBuilder::default()
                .with_router_info(
                    bob.router_info.clone(),
                    bob.static_key.clone(),
                    bob.signing_key.clone(),
                )
                .build(),
            Some(bob.socket.clone()),
            None,
        );

        // alice is not added to bob's profile storage so the request is rejected

        // add charlie as relay client
        let (bob_charlie_tx, _bob_charlie_rx) = channel(16);
        bob_relay.register_relay_client(charlie.router_id.clone(), 1337, bob_charlie_tx);

        // send relay request to charlie via bob
        let RelayConnection { .. } =
            alice_relay.send_relay_request(charlie.parsed(), false).unwrap();

        let (nonce, message, signature) = match alice_bob_rx.try_recv().unwrap() {
            RelayCommand::RelayRequest {
                nonce,
                message,
                signature,
            } => (nonce, message, signature),
            _ => panic!("unexpected command"),
        };

        // handle relay request as bob
        let (bob_alice_tx, bob_alice_rx) = channel(16);
        bob_relay.handle_relay_request(
            alice.router_id.clone(),
            nonce,
            1337,
            alice.socket.local_address().unwrap(),
            message,
            signature,
            bob_alice_tx,
        );

        // verify charlie is sent relay intro
        let (rejection, message, signature, token) = match bob_alice_rx.try_recv().unwrap() {
            RelayCommand::RelayResponse {
                rejection,
                message,
                signature,
                token,
                ..
            } => (rejection, message, signature, token),
            _ => panic!("unexpected command"),
        };
        assert_eq!(
            rejection,
            Some(RejectionReason::Bob(BobRejectionReason::AliceNotFound))
        );

        // handle response as alice
        alice_relay.handle_relay_response(nonce, None, token, rejection, message, signature);

        match alice_relay.next().await.unwrap() {
            RelayManagerEvent::RelayFailure { router_id } =>
                assert_eq!(router_id, charlie.router_id),
            _ => {}
        }
    }

    #[tokio::test]
    async fn bob_rejects_invalid_signature() {
        let alice = TestRouter::new(0).await;
        let bob = TestRouter::new(2).await;
        let charlie = TestRouter::with_intorducer(4, bob.router_id.clone(), 1337).await;

        // create alice relay manager
        let mut alice_relay = RelayManager::<MockRuntime>::new(
            alice.intro_key,
            RouterContextBuilder::default()
                .with_router_info(
                    alice.router_info.clone(),
                    alice.static_key.clone(),
                    alice.signing_key.clone(),
                )
                .build(),
            Some(alice.socket.clone()),
            None,
        );

        // add bob as a router that supports relay
        let (alice_bob_tx, alice_bob_rx) = channel(16);
        alice_relay.add_session(&bob.router_id, alice_bob_tx, true);

        // create relay manager for bob
        let mut bob_relay = RelayManager::<MockRuntime>::new(
            bob.intro_key,
            RouterContextBuilder::default()
                .with_router_info(
                    bob.router_info.clone(),
                    bob.static_key.clone(),
                    bob.signing_key.clone(),
                )
                .build(),
            Some(bob.socket.clone()),
            None,
        );

        // add alice to bob's profile storage so bob can forward
        // alice's router info to charlie
        bob_relay.router_ctx.profile_storage().discover_router(
            alice.router_info.clone(),
            Bytes::from(alice.serialized.clone()),
        );

        // add charlie as relay client
        let (bob_charlie_tx, _bob_charlie_rx) = channel(16);
        bob_relay.register_relay_client(charlie.router_id.clone(), 1337, bob_charlie_tx);

        // send relay request to charlie via bob
        let RelayConnection { .. } =
            alice_relay.send_relay_request(charlie.parsed(), false).unwrap();

        let (nonce, message) = match alice_bob_rx.try_recv().unwrap() {
            RelayCommand::RelayRequest { nonce, message, .. } => (nonce, message),
            _ => panic!("unexpected command"),
        };

        // handle relay request as bob
        let (bob_alice_tx, bob_alice_rx) = channel(16);
        bob_relay.handle_relay_request(
            alice.router_id.clone(),
            nonce,
            1337,
            alice.socket.local_address().unwrap(),
            message,
            vec![0u8; 64], // invalid signature
            bob_alice_tx,
        );

        // verify charlie is sent relay intro
        let (rejection, message, signature, token) = match bob_alice_rx.try_recv().unwrap() {
            RelayCommand::RelayResponse {
                rejection,
                message,
                signature,
                token,
                ..
            } => (rejection, message, signature, token),
            _ => panic!("unexpected command"),
        };
        assert_eq!(
            rejection,
            Some(RejectionReason::Bob(BobRejectionReason::SignatureFailure))
        );

        // handle response as alice
        alice_relay.handle_relay_response(nonce, None, token, rejection, message, signature);

        match alice_relay.next().await.unwrap() {
            RelayManagerEvent::RelayFailure { router_id } =>
                assert_eq!(router_id, charlie.router_id),
            _ => {}
        }
    }

    #[tokio::test]
    async fn charlie_rejects_session_not_found() {
        let alice = TestRouter::new(0).await;
        let bob = TestRouter::new(2).await;
        let charlie = TestRouter::with_intorducer(4, bob.router_id.clone(), 1337).await;

        // create alice relay manager
        let mut alice_relay = RelayManager::<MockRuntime>::new(
            alice.intro_key,
            RouterContextBuilder::default()
                .with_router_info(
                    alice.router_info.clone(),
                    alice.static_key.clone(),
                    alice.signing_key.clone(),
                )
                .build(),
            Some(alice.socket.clone()),
            None,
        );

        // add bob as a router that supports relay
        let (alice_bob_tx, alice_bob_rx) = channel(16);
        alice_relay.add_session(&bob.router_id, alice_bob_tx, true);

        // create relay manager for bob
        let mut bob_relay = RelayManager::<MockRuntime>::new(
            bob.intro_key,
            RouterContextBuilder::default()
                .with_router_info(
                    bob.router_info.clone(),
                    bob.static_key.clone(),
                    bob.signing_key.clone(),
                )
                .build(),
            Some(bob.socket.clone()),
            None,
        );

        // add alice to bob's profile storage so bob can forward
        // alice's router info to charlie
        bob_relay.router_ctx.profile_storage().discover_router(
            alice.router_info.clone(),
            Bytes::from(alice.serialized.clone()),
        );

        // add charlie as relay client
        let (bob_charlie_tx, bob_charlie_rx) = channel(16);
        bob_relay.register_relay_client(charlie.router_id.clone(), 1337, bob_charlie_tx);

        // create relay manager for charlie
        let mut charlie_relay = RelayManager::<MockRuntime>::new(
            charlie.intro_key,
            RouterContextBuilder::default()
                .with_router_info(
                    charlie.router_info.clone(),
                    charlie.static_key.clone(),
                    charlie.signing_key.clone(),
                )
                .build(),
            Some(charlie.socket.clone()),
            None,
        );

        // register bob as relay server
        charlie_relay.register_relay_server(bob.router_id.clone(), 1337, true);
        charlie_relay.add_external_address(charlie.socket.local_address().unwrap());

        // send relay request to charlie via bob
        let RelayConnection { .. } =
            alice_relay.send_relay_request(charlie.parsed(), false).unwrap();

        let (nonce, message, signature) = match alice_bob_rx.try_recv().unwrap() {
            RelayCommand::RelayRequest {
                nonce,
                message,
                signature,
            } => (nonce, message, signature),
            _ => panic!("unexpected command"),
        };

        // handle relay request as bob
        let (bob_alice_tx, bob_alice_rx) = channel(16);
        bob_relay.handle_relay_request(
            alice.router_id.clone(),
            nonce,
            1337,
            alice.socket.local_address().unwrap(),
            message,
            signature,
            bob_alice_tx,
        );

        // verify charlie is sent relay intro
        let (router_info, message, signature) = match bob_charlie_rx.try_recv().unwrap() {
            RelayCommand::RelayIntro {
                router_info,
                message,
                signature,
                ..
            } => (router_info, message, signature),
            _ => panic!("unexpected command"),
        };

        // handle relay intro as charlie
        let (charlie_bob_tx, charlie_bob_rx) = channel(16);
        charlie_relay.handle_relay_intro(
            alice.router_id.clone(),
            bob.router_id.clone(),
            Some(Box::new(
                RouterInfo::parse::<MockRuntime>(router_info).unwrap(),
            )),
            nonce,
            1338, // invalid relay tag
            alice.socket.local_address().unwrap(),
            message,
            signature,
            charlie_bob_tx,
        );

        // read charlie's response from bob's channel
        let (rejection, message, signature, token) = match charlie_bob_rx.try_recv().unwrap() {
            RelayCommand::RelayResponse {
                rejection,
                message,
                signature,
                token,
                ..
            } => (rejection, message, signature, token),
            _ => panic!("unexpected event"),
        };
        assert_eq!(
            rejection,
            Some(RejectionReason::Charlie(
                CharlieRejectionReason::Unspecified
            ))
        );

        // handle charlie's response as bob
        bob_relay.handle_relay_response(nonce, None, token, rejection, message, signature);

        // read bob's response from alice's channel
        let (rejection, message, signature, token) = match bob_alice_rx.try_recv().unwrap() {
            RelayCommand::RelayResponse {
                rejection,
                message,
                signature,
                token,
                ..
            } => (rejection, message, signature, token),
            _ => panic!("unexpected event"),
        };
        assert_eq!(
            rejection,
            Some(RejectionReason::Charlie(
                CharlieRejectionReason::Unspecified
            ))
        );

        // handle response as alice
        alice_relay.handle_relay_response(
            nonce,
            Some(charlie.socket.local_address().unwrap()),
            token,
            rejection,
            message,
            signature,
        );

        match alice_relay.next().await.unwrap() {
            RelayManagerEvent::RelayFailure { router_id } =>
                assert_eq!(router_id, charlie.router_id),
            _ => {}
        }
    }

    #[tokio::test]
    async fn charlie_rejects_alice_not_found() {
        let alice = TestRouter::new(0).await;
        let bob = TestRouter::new(2).await;
        let charlie = TestRouter::with_intorducer(4, bob.router_id.clone(), 1337).await;

        // create alice relay manager
        let mut alice_relay = RelayManager::<MockRuntime>::new(
            alice.intro_key,
            RouterContextBuilder::default()
                .with_router_info(
                    alice.router_info.clone(),
                    alice.static_key.clone(),
                    alice.signing_key.clone(),
                )
                .build(),
            Some(alice.socket.clone()),
            None,
        );

        // add bob as a router that supports relay
        let (alice_bob_tx, alice_bob_rx) = channel(16);
        alice_relay.add_session(&bob.router_id, alice_bob_tx, true);

        // create relay manager for bob
        let mut bob_relay = RelayManager::<MockRuntime>::new(
            bob.intro_key,
            RouterContextBuilder::default()
                .with_router_info(
                    bob.router_info.clone(),
                    bob.static_key.clone(),
                    bob.signing_key.clone(),
                )
                .build(),
            Some(bob.socket.clone()),
            None,
        );

        // add alice to bob's profile storage so bob can forward
        // alice's router info to charlie
        bob_relay.router_ctx.profile_storage().discover_router(
            alice.router_info.clone(),
            Bytes::from(alice.serialized.clone()),
        );

        // add charlie as relay client
        let (bob_charlie_tx, bob_charlie_rx) = channel(16);
        bob_relay.register_relay_client(charlie.router_id.clone(), 1337, bob_charlie_tx);

        // create relay manager for charlie
        let mut charlie_relay = RelayManager::<MockRuntime>::new(
            charlie.intro_key,
            RouterContextBuilder::default()
                .with_router_info(
                    charlie.router_info.clone(),
                    charlie.static_key.clone(),
                    charlie.signing_key.clone(),
                )
                .build(),
            Some(charlie.socket.clone()),
            None,
        );

        // register bob as relay server
        charlie_relay.register_relay_server(bob.router_id.clone(), 1337, true);
        charlie_relay.add_external_address(charlie.socket.local_address().unwrap());

        // send relay request to charlie via bob
        let RelayConnection { .. } =
            alice_relay.send_relay_request(charlie.parsed(), false).unwrap();

        let (nonce, message, signature) = match alice_bob_rx.try_recv().unwrap() {
            RelayCommand::RelayRequest {
                nonce,
                message,
                signature,
            } => (nonce, message, signature),
            _ => panic!("unexpected command"),
        };

        // handle relay request as bob
        let (bob_alice_tx, bob_alice_rx) = channel(16);
        bob_relay.handle_relay_request(
            alice.router_id.clone(),
            nonce,
            1337,
            alice.socket.local_address().unwrap(),
            message,
            signature,
            bob_alice_tx,
        );

        // verify charlie is sent relay intro
        let (message, signature) = match bob_charlie_rx.try_recv().unwrap() {
            RelayCommand::RelayIntro {
                message, signature, ..
            } => (message, signature),
            _ => panic!("unexpected command"),
        };

        // handle relay intro as charlie
        let (charlie_bob_tx, charlie_bob_rx) = channel(16);
        charlie_relay.handle_relay_intro(
            alice.router_id.clone(),
            bob.router_id.clone(),
            None, // alice router info not found from charli
            // Some(Box::new(
            //     RouterInfo::parse::<MockRuntime>(router_info).unwrap(),
            // )),
            nonce,
            1337,
            alice.socket.local_address().unwrap(),
            message,
            signature,
            charlie_bob_tx,
        );

        // read charlie's response from bob's channel
        let (rejection, message, signature, token) = match charlie_bob_rx.try_recv().unwrap() {
            RelayCommand::RelayResponse {
                rejection,
                message,
                signature,
                token,
                ..
            } => (rejection, message, signature, token),
            _ => panic!("unexpected event"),
        };
        assert_eq!(
            rejection,
            Some(RejectionReason::Charlie(
                CharlieRejectionReason::AliceNotFound
            ))
        );

        // handle charlie's response as bob
        bob_relay.handle_relay_response(nonce, None, token, rejection, message, signature);

        // read bob's response from alice's channel
        let (rejection, message, signature, token) = match bob_alice_rx.try_recv().unwrap() {
            RelayCommand::RelayResponse {
                rejection,
                message,
                signature,
                token,
                ..
            } => (rejection, message, signature, token),
            _ => panic!("unexpected event"),
        };
        assert_eq!(
            rejection,
            Some(RejectionReason::Charlie(
                CharlieRejectionReason::AliceNotFound
            ))
        );

        // handle response as alice
        alice_relay.handle_relay_response(
            nonce,
            Some(charlie.socket.local_address().unwrap()),
            token,
            rejection,
            message,
            signature,
        );

        match alice_relay.next().await.unwrap() {
            RelayManagerEvent::RelayFailure { router_id } =>
                assert_eq!(router_id, charlie.router_id),
            _ => {}
        }
    }

    #[tokio::test]
    async fn charlie_rejects_invalid_signature() {
        let alice = TestRouter::new(0).await;
        let bob = TestRouter::new(2).await;
        let charlie = TestRouter::with_intorducer(4, bob.router_id.clone(), 1337).await;

        // create alice relay manager
        let mut alice_relay = RelayManager::<MockRuntime>::new(
            alice.intro_key,
            RouterContextBuilder::default()
                .with_router_info(
                    alice.router_info.clone(),
                    alice.static_key.clone(),
                    alice.signing_key.clone(),
                )
                .build(),
            Some(alice.socket.clone()),
            None,
        );

        // add bob as a router that supports relay
        let (alice_bob_tx, alice_bob_rx) = channel(16);
        alice_relay.add_session(&bob.router_id, alice_bob_tx, true);

        // create relay manager for bob
        let mut bob_relay = RelayManager::<MockRuntime>::new(
            bob.intro_key,
            RouterContextBuilder::default()
                .with_router_info(
                    bob.router_info.clone(),
                    bob.static_key.clone(),
                    bob.signing_key.clone(),
                )
                .build(),
            Some(bob.socket.clone()),
            None,
        );

        // add alice to bob's profile storage so bob can forward
        // alice's router info to charlie
        bob_relay.router_ctx.profile_storage().discover_router(
            alice.router_info.clone(),
            Bytes::from(alice.serialized.clone()),
        );

        // add charlie as relay client
        let (bob_charlie_tx, bob_charlie_rx) = channel(16);
        bob_relay.register_relay_client(charlie.router_id.clone(), 1337, bob_charlie_tx);

        // create relay manager for charlie
        let mut charlie_relay = RelayManager::<MockRuntime>::new(
            charlie.intro_key,
            RouterContextBuilder::default()
                .with_router_info(
                    charlie.router_info.clone(),
                    charlie.static_key.clone(),
                    charlie.signing_key.clone(),
                )
                .build(),
            Some(charlie.socket.clone()),
            None,
        );

        // register bob as relay server
        charlie_relay.register_relay_server(bob.router_id.clone(), 1337, true);
        charlie_relay.add_external_address(charlie.socket.local_address().unwrap());

        // send relay request to charlie via bob
        let RelayConnection { .. } =
            alice_relay.send_relay_request(charlie.parsed(), false).unwrap();

        let (nonce, message, signature) = match alice_bob_rx.try_recv().unwrap() {
            RelayCommand::RelayRequest {
                nonce,
                message,
                signature,
            } => (nonce, message, signature),
            _ => panic!("unexpected command"),
        };

        // handle relay request as bob
        let (bob_alice_tx, bob_alice_rx) = channel(16);
        bob_relay.handle_relay_request(
            alice.router_id.clone(),
            nonce,
            1337,
            alice.socket.local_address().unwrap(),
            message,
            signature,
            bob_alice_tx,
        );

        // verify charlie is sent relay intro
        let (router_info, message) = match bob_charlie_rx.try_recv().unwrap() {
            RelayCommand::RelayIntro {
                router_info,
                message,
                ..
            } => (router_info, message),
            _ => panic!("unexpected command"),
        };

        // handle relay intro as charlie
        let (charlie_bob_tx, charlie_bob_rx) = channel(16);
        charlie_relay.handle_relay_intro(
            alice.router_id.clone(),
            bob.router_id.clone(),
            Some(Box::new(
                RouterInfo::parse::<MockRuntime>(router_info).unwrap(),
            )),
            nonce,
            1337,
            alice.socket.local_address().unwrap(),
            message,
            vec![0u8; 64], // invalid signature
            charlie_bob_tx,
        );

        // read charlie's response from bob's channel
        let (rejection, message, signature, token) = match charlie_bob_rx.try_recv().unwrap() {
            RelayCommand::RelayResponse {
                rejection,
                message,
                signature,
                token,
                ..
            } => (rejection, message, signature, token),
            _ => panic!("unexpected event"),
        };
        assert_eq!(
            rejection,
            Some(RejectionReason::Charlie(
                CharlieRejectionReason::SignatureFailure
            ))
        );

        // handle charlie's response as bob
        bob_relay.handle_relay_response(nonce, None, token, rejection, message, signature);

        // read bob's response from alice's channel
        let (rejection, message, signature, token) = match bob_alice_rx.try_recv().unwrap() {
            RelayCommand::RelayResponse {
                rejection,
                message,
                signature,
                token,
                ..
            } => (rejection, message, signature, token),
            _ => panic!("unexpected event"),
        };
        assert_eq!(
            rejection,
            Some(RejectionReason::Charlie(
                CharlieRejectionReason::SignatureFailure
            ))
        );

        // handle response as alice
        alice_relay.handle_relay_response(
            nonce,
            Some(charlie.socket.local_address().unwrap()),
            token,
            rejection,
            message,
            signature,
        );

        match alice_relay.next().await.unwrap() {
            RelayManagerEvent::RelayFailure { router_id } =>
                assert_eq!(router_id, charlie.router_id),
            _ => {}
        }
    }

    #[tokio::test]
    async fn charlie_rejects_no_external_address() {
        let alice = TestRouter::new(0).await;
        let bob = TestRouter::new(2).await;
        let charlie = TestRouter::with_intorducer(4, bob.router_id.clone(), 1337).await;

        // create alice relay manager
        let mut alice_relay = RelayManager::<MockRuntime>::new(
            alice.intro_key,
            RouterContextBuilder::default()
                .with_router_info(
                    alice.router_info.clone(),
                    alice.static_key.clone(),
                    alice.signing_key.clone(),
                )
                .build(),
            Some(alice.socket.clone()),
            None,
        );

        // add bob as a router that supports relay
        let (alice_bob_tx, alice_bob_rx) = channel(16);
        alice_relay.add_session(&bob.router_id, alice_bob_tx, true);

        // create relay manager for bob
        let mut bob_relay = RelayManager::<MockRuntime>::new(
            bob.intro_key,
            RouterContextBuilder::default()
                .with_router_info(
                    bob.router_info.clone(),
                    bob.static_key.clone(),
                    bob.signing_key.clone(),
                )
                .build(),
            Some(bob.socket.clone()),
            None,
        );

        // add alice to bob's profile storage so bob can forward
        // alice's router info to charlie
        bob_relay.router_ctx.profile_storage().discover_router(
            alice.router_info.clone(),
            Bytes::from(alice.serialized.clone()),
        );

        // add charlie as relay client
        let (bob_charlie_tx, bob_charlie_rx) = channel(16);
        bob_relay.register_relay_client(charlie.router_id.clone(), 1337, bob_charlie_tx);

        // create relay manager for charlie
        let mut charlie_relay = RelayManager::<MockRuntime>::new(
            charlie.intro_key,
            RouterContextBuilder::default()
                .with_router_info(
                    charlie.router_info.clone(),
                    charlie.static_key.clone(),
                    charlie.signing_key.clone(),
                )
                .build(),
            Some(charlie.socket.clone()),
            None,
        );

        // register bob as relay server
        //
        // charlie has no external address so the relay intro is rejected
        charlie_relay.register_relay_server(bob.router_id.clone(), 1337, true);

        // send relay request to charlie via bob
        let RelayConnection { .. } =
            alice_relay.send_relay_request(charlie.parsed(), false).unwrap();

        let (nonce, message, signature) = match alice_bob_rx.try_recv().unwrap() {
            RelayCommand::RelayRequest {
                nonce,
                message,
                signature,
            } => (nonce, message, signature),
            _ => panic!("unexpected command"),
        };

        // handle relay request as bob
        let (bob_alice_tx, bob_alice_rx) = channel(16);
        bob_relay.handle_relay_request(
            alice.router_id.clone(),
            nonce,
            1337,
            alice.socket.local_address().unwrap(),
            message,
            signature,
            bob_alice_tx,
        );

        // verify charlie is sent relay intro
        let (router_info, message, signature) = match bob_charlie_rx.try_recv().unwrap() {
            RelayCommand::RelayIntro {
                router_info,
                message,
                signature,
                ..
            } => (router_info, message, signature),
            _ => panic!("unexpected command"),
        };

        // handle relay intro as charlie
        let (charlie_bob_tx, charlie_bob_rx) = channel(16);
        charlie_relay.handle_relay_intro(
            alice.router_id.clone(),
            bob.router_id.clone(),
            Some(Box::new(
                RouterInfo::parse::<MockRuntime>(router_info).unwrap(),
            )),
            nonce,
            1337,
            alice.socket.local_address().unwrap(),
            message,
            signature,
            charlie_bob_tx,
        );

        // read charlie's response from bob's channel
        let (rejection, message, signature, token) = match charlie_bob_rx.try_recv().unwrap() {
            RelayCommand::RelayResponse {
                rejection,
                message,
                signature,
                token,
                ..
            } => (rejection, message, signature, token),
            _ => panic!("unexpected event"),
        };
        assert_eq!(
            rejection,
            Some(RejectionReason::Charlie(
                CharlieRejectionReason::Unspecified
            ))
        );

        // handle charlie's response as bob
        bob_relay.handle_relay_response(nonce, None, token, rejection, message, signature);

        // read bob's response from alice's channel
        let (rejection, message, signature, token) = match bob_alice_rx.try_recv().unwrap() {
            RelayCommand::RelayResponse {
                rejection,
                message,
                signature,
                token,
                ..
            } => (rejection, message, signature, token),
            _ => panic!("unexpected event"),
        };
        assert_eq!(
            rejection,
            Some(RejectionReason::Charlie(
                CharlieRejectionReason::Unspecified
            ))
        );

        // handle response as alice
        alice_relay.handle_relay_response(
            nonce,
            Some(charlie.socket.local_address().unwrap()),
            token,
            rejection,
            message,
            signature,
        );

        match alice_relay.next().await.unwrap() {
            RelayManagerEvent::RelayFailure { router_id } =>
                assert_eq!(router_id, charlie.router_id),
            _ => {}
        }
    }

    #[tokio::test]
    async fn send_relay_request_no_ssu2_address() {
        let alice = TestRouter::new(0).await;
        let bob = TestRouter::new(2).await;

        // create alice relay manager
        let mut alice_relay = RelayManager::<MockRuntime>::new(
            alice.intro_key,
            RouterContextBuilder::default()
                .with_router_info(
                    alice.router_info.clone(),
                    alice.static_key.clone(),
                    alice.signing_key.clone(),
                )
                .build(),
            Some(alice.socket.clone()),
            None,
        );

        // add bob as a router that supports relay
        let (alice_bob_tx, _alice_bob_rx) = channel(16);
        alice_relay.add_session(&bob.router_id, alice_bob_tx, true);

        // charlie doesn't have an ssu2 address
        match alice_relay.send_relay_request(RouterInfoBuilder::default().build().0, false) {
            Err(RelayError::NoAddress) => {}
            _ => panic!("unexpected result"),
        }
    }

    #[tokio::test]
    async fn send_relay_request_introducer_not_found() {
        let alice = TestRouter::new(0).await;
        let bob = TestRouter::new(2).await;
        let charlie = TestRouter::with_intorducer(4, bob.router_id.clone(), 1337).await;

        // create alice relay manager
        let mut alice_relay = RelayManager::<MockRuntime>::new(
            alice.intro_key,
            RouterContextBuilder::default()
                .with_router_info(
                    alice.router_info.clone(),
                    alice.static_key.clone(),
                    alice.signing_key.clone(),
                )
                .build(),
            Some(alice.socket.clone()),
            None,
        );

        // bob is not added as active introducer so the request will fail

        // charlie doesn't have an ssu2 address
        match alice_relay.send_relay_request(charlie.parsed(), false) {
            Err(RelayError::NoIntroducer) => {}
            _ => panic!("unexpected result"),
        }
    }

    #[tokio::test]
    async fn send_relay_request_send_failure() {
        let alice = TestRouter::new(0).await;
        let bob = TestRouter::new(2).await;
        let charlie = TestRouter::with_intorducer(4, bob.router_id.clone(), 1337).await;

        // create alice relay manager
        let mut alice_relay = RelayManager::<MockRuntime>::new(
            alice.intro_key,
            RouterContextBuilder::default()
                .with_router_info(
                    alice.router_info.clone(),
                    alice.static_key.clone(),
                    alice.signing_key.clone(),
                )
                .build(),
            Some(alice.socket.clone()),
            None,
        );

        // add bob as a router that supports relay
        //
        // drop bob's channel so sending the request fails
        let (alice_bob_tx, alice_bob_rx) = channel(16);
        alice_relay.add_session(&bob.router_id, alice_bob_tx, true);
        drop(alice_bob_rx);

        // charlie doesn't have an ssu2 address
        match alice_relay.send_relay_request(charlie.parsed(), false) {
            Err(RelayError::RelayRequestSendFailure) => {}
            _ => panic!("unexpected result"),
        }
    }

    #[tokio::test]
    async fn handle_relay_response_success() {
        let alice = TestRouter::new(0).await;

        // create alice relay manager
        let mut alice_relay = RelayManager::<MockRuntime>::new(
            alice.intro_key,
            RouterContextBuilder::default()
                .with_router_info(
                    alice.router_info.clone(),
                    alice.static_key.clone(),
                    alice.signing_key.clone(),
                )
                .build(),
            Some(alice.socket.clone()),
            None,
        );

        let bob_router_id = RouterId::random();
        let charlie_router_id = RouterId::random();
        let relay_tag = 1337;
        let nonce = 1338;
        let token = 13371338;
        let src_id = (!(((nonce as u64) << 32) | (nonce as u64))).to_be();
        let charlie_signing_key = SigningPrivateKey::random(&mut MockRuntime::rng());

        alice_relay.active_outbound.insert(
            src_id,
            RelayProcess {
                bob_router_id: bob_router_id.clone(),
                charlie_router_id: charlie_router_id.clone(),
                created: MockRuntime::now(),
                charlie_verifying_key: charlie_signing_key.public(),
                relay_tag,
            },
        );

        let (message, signature) = {
            let mut message = BytesMut::with_capacity(128);
            message.put_slice(b"RelayAgreementOK");
            message.put_slice(&bob_router_id.to_vec());
            message.put_slice(&b"hello, world".to_vec());

            let signature = charlie_signing_key.sign(&message);

            (
                message.split_off(b"RelayAgreementOK".len() + ROUTER_HASH_LEN).to_vec(),
                signature,
            )
        };

        alice_relay.handle_relay_response(
            nonce,
            Some("127.0.0.1:8888".parse().unwrap()),
            Some(token),
            None,
            message,
            signature,
        );

        match timeout!(alice_relay.next()).await.unwrap().unwrap() {
            RelayManagerEvent::RelaySuccess {
                router_id,
                token: session_request_token,
                ..
            } => {
                assert_eq!(router_id, charlie_router_id);
                assert_eq!(session_request_token, token);
            }
            _ => panic!("unexpected event"),
        }
    }

    #[tokio::test]
    async fn handle_relay_response_rejected() {
        let alice = TestRouter::new(0).await;

        // create alice relay manager
        let mut alice_relay = RelayManager::<MockRuntime>::new(
            alice.intro_key,
            RouterContextBuilder::default()
                .with_router_info(
                    alice.router_info.clone(),
                    alice.static_key.clone(),
                    alice.signing_key.clone(),
                )
                .build(),
            Some(alice.socket.clone()),
            None,
        );

        let bob_router_id = RouterId::random();
        let charlie_router_id = RouterId::random();
        let relay_tag = 1337;
        let nonce = 1338;
        let token = 13371338;
        let src_id = (!(((nonce as u64) << 32) | (nonce as u64))).to_be();
        let charlie_signing_key = SigningPrivateKey::random(&mut MockRuntime::rng());

        alice_relay.active_outbound.insert(
            src_id,
            RelayProcess {
                bob_router_id: bob_router_id.clone(),
                charlie_router_id: charlie_router_id.clone(),
                created: MockRuntime::now(),
                charlie_verifying_key: charlie_signing_key.public(),
                relay_tag,
            },
        );

        let (message, signature) = {
            let mut message = BytesMut::with_capacity(128);
            message.put_slice(b"RelayAgreementOK");
            message.put_slice(&bob_router_id.to_vec());
            message.put_slice(&b"hello, world".to_vec());

            let signature = charlie_signing_key.sign(&message);

            (
                message.split_off(b"RelayAgreementOK".len() + ROUTER_HASH_LEN).to_vec(),
                signature,
            )
        };

        alice_relay.handle_relay_response(
            nonce,
            Some("127.0.0.1:8888".parse().unwrap()),
            Some(token),
            Some(RejectionReason::Unspecified),
            message,
            signature,
        );

        match timeout!(alice_relay.next()).await.unwrap().unwrap() {
            RelayManagerEvent::RelayFailure { router_id } =>
                assert_eq!(router_id, charlie_router_id),
            _ => panic!("unexpected event"),
        }
    }

    #[tokio::test]
    async fn handle_relay_response_no_address() {
        let alice = TestRouter::new(0).await;

        // create alice relay manager
        let mut alice_relay = RelayManager::<MockRuntime>::new(
            alice.intro_key,
            RouterContextBuilder::default()
                .with_router_info(
                    alice.router_info.clone(),
                    alice.static_key.clone(),
                    alice.signing_key.clone(),
                )
                .build(),
            Some(alice.socket.clone()),
            None,
        );

        let bob_router_id = RouterId::random();
        let charlie_router_id = RouterId::random();
        let relay_tag = 1337;
        let nonce = 1338;
        let token = 13371338;
        let src_id = (!(((nonce as u64) << 32) | (nonce as u64))).to_be();
        let charlie_signing_key = SigningPrivateKey::random(&mut MockRuntime::rng());

        alice_relay.active_outbound.insert(
            src_id,
            RelayProcess {
                bob_router_id: bob_router_id.clone(),
                charlie_router_id: charlie_router_id.clone(),
                created: MockRuntime::now(),
                charlie_verifying_key: charlie_signing_key.public(),
                relay_tag,
            },
        );

        let (message, signature) = {
            let mut message = BytesMut::with_capacity(128);
            message.put_slice(b"RelayAgreementOK");
            message.put_slice(&bob_router_id.to_vec());
            message.put_slice(&b"hello, world".to_vec());

            let signature = charlie_signing_key.sign(&message);

            (
                message.split_off(b"RelayAgreementOK".len() + ROUTER_HASH_LEN).to_vec(),
                signature,
            )
        };

        alice_relay.handle_relay_response(
            nonce,
            None, // charlie's address omitted
            Some(token),
            None,
            message,
            signature,
        );

        match timeout!(alice_relay.next()).await.unwrap().unwrap() {
            RelayManagerEvent::RelayFailure { router_id } =>
                assert_eq!(router_id, charlie_router_id),
            _ => panic!("unexpected event"),
        }
    }

    #[tokio::test]
    async fn handle_relay_response_no_token() {
        let alice = TestRouter::new(0).await;

        // create alice relay manager
        let mut alice_relay = RelayManager::<MockRuntime>::new(
            alice.intro_key,
            RouterContextBuilder::default()
                .with_router_info(
                    alice.router_info.clone(),
                    alice.static_key.clone(),
                    alice.signing_key.clone(),
                )
                .build(),
            Some(alice.socket.clone()),
            None,
        );

        let bob_router_id = RouterId::random();
        let charlie_router_id = RouterId::random();
        let relay_tag = 1337;
        let nonce = 1338;
        let src_id = (!(((nonce as u64) << 32) | (nonce as u64))).to_be();
        let charlie_signing_key = SigningPrivateKey::random(&mut MockRuntime::rng());

        alice_relay.active_outbound.insert(
            src_id,
            RelayProcess {
                bob_router_id: bob_router_id.clone(),
                charlie_router_id: charlie_router_id.clone(),
                created: MockRuntime::now(),
                charlie_verifying_key: charlie_signing_key.public(),
                relay_tag,
            },
        );

        let (message, signature) = {
            let mut message = BytesMut::with_capacity(128);
            message.put_slice(b"RelayAgreementOK");
            message.put_slice(&bob_router_id.to_vec());
            message.put_slice(&b"hello, world".to_vec());

            let signature = charlie_signing_key.sign(&message);

            (
                message.split_off(b"RelayAgreementOK".len() + ROUTER_HASH_LEN).to_vec(),
                signature,
            )
        };

        alice_relay.handle_relay_response(
            nonce,
            Some("127.0.0.1:8888".parse().unwrap()),
            None, // token omitted
            None,
            message,
            signature,
        );

        match timeout!(alice_relay.next()).await.unwrap().unwrap() {
            RelayManagerEvent::RelayFailure { router_id } =>
                assert_eq!(router_id, charlie_router_id),
            _ => panic!("unexpected event"),
        }
    }

    #[tokio::test]
    async fn handle_relay_response_invalid_signature() {
        let alice = TestRouter::new(0).await;

        // create alice relay manager
        let mut alice_relay = RelayManager::<MockRuntime>::new(
            alice.intro_key,
            RouterContextBuilder::default()
                .with_router_info(
                    alice.router_info.clone(),
                    alice.static_key.clone(),
                    alice.signing_key.clone(),
                )
                .build(),
            Some(alice.socket.clone()),
            None,
        );

        let bob_router_id = RouterId::random();
        let charlie_router_id = RouterId::random();
        let relay_tag = 1337;
        let nonce = 1338;
        let token = 13371338;
        let src_id = (!(((nonce as u64) << 32) | (nonce as u64))).to_be();
        let charlie_signing_key = SigningPrivateKey::random(&mut MockRuntime::rng());

        alice_relay.active_outbound.insert(
            src_id,
            RelayProcess {
                bob_router_id: bob_router_id.clone(),
                charlie_router_id: charlie_router_id.clone(),
                created: MockRuntime::now(),
                charlie_verifying_key: charlie_signing_key.public(),
                relay_tag,
            },
        );

        let message = b"hello, world".to_vec();

        alice_relay.handle_relay_response(
            nonce,
            Some("127.0.0.1:8888".parse().unwrap()),
            Some(token),
            None,
            message,
            vec![0u8; 64], // invalid signature
        );

        match timeout!(alice_relay.next()).await.unwrap().unwrap() {
            RelayManagerEvent::RelayFailure { router_id } =>
                assert_eq!(router_id, charlie_router_id),
            _ => panic!("unexpected event"),
        }
    }

    #[tokio::test]
    async fn handle_hole_punch_success() {
        let alice = TestRouter::new(0).await;

        // create alice relay manager
        let mut alice_relay = RelayManager::<MockRuntime>::new(
            alice.intro_key,
            RouterContextBuilder::default()
                .with_router_info(
                    alice.router_info.clone(),
                    alice.static_key.clone(),
                    alice.signing_key.clone(),
                )
                .build(),
            Some(alice.socket.clone()),
            None,
        );

        let bob_router_id = RouterId::random();
        let charlie_router_id = RouterId::random();
        let relay_tag = 1337;
        let nonce = 1338;
        let token = 13371338;
        let src_id = (!(((nonce as u64) << 32) | (nonce as u64))).to_be();
        let dst_id = (((nonce as u64) << 32) | (nonce as u64)).to_be();
        let charlie_signing_key = SigningPrivateKey::random(&mut MockRuntime::rng());

        alice_relay.active_outbound.insert(
            src_id,
            RelayProcess {
                bob_router_id: bob_router_id.clone(),
                charlie_router_id: charlie_router_id.clone(),
                created: MockRuntime::now(),
                charlie_verifying_key: charlie_signing_key.public(),
                relay_tag,
            },
        );

        let (message, signature) = {
            let mut payload = BytesMut::with_capacity(128);
            payload.put_slice(b"RelayAgreementOK");
            payload.put_slice(&bob_router_id.to_vec());
            payload.put_u32(nonce);
            payload.put_u32(MockRuntime::time_since_epoch().as_secs() as u32);
            payload.put_u8(2);
            payload.put_u8(6);
            payload.put_u16(8888);
            payload.put_slice(&[127, 0, 0, 1]);

            let signature = charlie_signing_key.sign(&payload);

            (
                payload.split_off(b"RelayAgreementOK".len() + ROUTER_HASH_LEN).to_vec(),
                signature,
            )
        };
        let mut pkt = HolePunchBuilder::new(&message, &signature)
            .with_src_id(src_id)
            .with_token(token)
            .with_dst_id(dst_id)
            .with_intro_key(alice.intro_key)
            .with_addres("127.0.0.1:8888".parse().unwrap())
            .build::<MockRuntime>()
            .to_vec();

        let mut reader = HeaderReader::new(alice.intro_key, &mut pkt).unwrap();
        let _dst_id = reader.dst_id();

        let (pkt_num, src_id) = match reader.parse(alice.intro_key).unwrap() {
            HeaderKind::HolePunch {
                pkt_num, src_id, ..
            } => (pkt_num, src_id),
            _ => panic!("unexpected packet"),
        };

        match alice_relay.handle_hole_punch(pkt, pkt_num, src_id) {
            Ok((router_id, _, recv_token)) => {
                assert_eq!(router_id, charlie_router_id);
                assert_eq!(recv_token, token);
            }
            res => panic!("unexpected result: {res:?}"),
        }
    }

    #[tokio::test]
    async fn handle_hole_punch_message_too_short() {
        let alice = TestRouter::new(0).await;

        // create alice relay manager
        let mut alice_relay = RelayManager::<MockRuntime>::new(
            alice.intro_key,
            RouterContextBuilder::default()
                .with_router_info(
                    alice.router_info.clone(),
                    alice.static_key.clone(),
                    alice.signing_key.clone(),
                )
                .build(),
            Some(alice.socket.clone()),
            None,
        );

        let bob_router_id = RouterId::random();
        let charlie_router_id = RouterId::random();
        let relay_tag = 1337;
        let nonce = 1338;
        let src_id = (!(((nonce as u64) << 32) | (nonce as u64))).to_be();
        let charlie_signing_key = SigningPrivateKey::random(&mut MockRuntime::rng());

        alice_relay.active_outbound.insert(
            src_id,
            RelayProcess {
                bob_router_id: bob_router_id.clone(),
                charlie_router_id: charlie_router_id.clone(),
                created: MockRuntime::now(),
                charlie_verifying_key: charlie_signing_key.public(),
                relay_tag,
            },
        );

        match alice_relay.handle_hole_punch(vec![0u8; 16], 1337, src_id) {
            Err(Ssu2Error::Relay(RelayError::InvalidHolePunch)) => {}
            res => panic!("unexpected result: {res:?}"),
        }
    }

    #[tokio::test]
    async fn handle_hole_punch_unknown_relay() {
        let alice = TestRouter::new(0).await;

        // create alice relay manager
        let mut alice_relay = RelayManager::<MockRuntime>::new(
            alice.intro_key,
            RouterContextBuilder::default()
                .with_router_info(
                    alice.router_info.clone(),
                    alice.static_key.clone(),
                    alice.signing_key.clone(),
                )
                .build(),
            Some(alice.socket.clone()),
            None,
        );

        let bob_router_id = RouterId::random();
        let charlie_router_id = RouterId::random();
        let relay_tag = 1337;
        let nonce = 1338;
        let token = 13371338;
        let src_id = (!(((nonce as u64) << 32) | (nonce as u64))).to_be();
        let dst_id = (((nonce as u64) << 32) | (nonce as u64)).to_be();
        let charlie_signing_key = SigningPrivateKey::random(&mut MockRuntime::rng());

        alice_relay.active_outbound.insert(
            src_id,
            RelayProcess {
                bob_router_id: bob_router_id.clone(),
                charlie_router_id: charlie_router_id.clone(),
                created: MockRuntime::now(),
                charlie_verifying_key: charlie_signing_key.public(),
                relay_tag,
            },
        );

        let (message, signature) = {
            let mut payload = BytesMut::with_capacity(128);
            payload.put_slice(b"RelayAgreementOK");
            payload.put_slice(&bob_router_id.to_vec());
            payload.put_u32(nonce);
            payload.put_u32(MockRuntime::time_since_epoch().as_secs() as u32);
            payload.put_u8(2);
            payload.put_u8(6);
            payload.put_u16(8888);
            payload.put_slice(&[127, 0, 0, 1]);

            let signature = charlie_signing_key.sign(&payload);

            (
                payload.split_off(b"RelayAgreementOK".len() + ROUTER_HASH_LEN).to_vec(),
                signature,
            )
        };
        let mut pkt = HolePunchBuilder::new(&message, &signature)
            .with_src_id(src_id)
            .with_token(token)
            .with_dst_id(dst_id)
            .with_intro_key(alice.intro_key)
            .with_addres("127.0.0.1:8888".parse().unwrap())
            .build::<MockRuntime>()
            .to_vec();

        let mut reader = HeaderReader::new(alice.intro_key, &mut pkt).unwrap();
        let _dst_id = reader.dst_id();

        let pkt_num = match reader.parse(alice.intro_key).unwrap() {
            HeaderKind::HolePunch { pkt_num, .. } => pkt_num,
            _ => panic!("unexpected packet"),
        };

        match alice_relay.handle_hole_punch(pkt, pkt_num, dst_id) {
            Err(Ssu2Error::Relay(RelayError::UnknownRelayProcess)) => {}
            res => panic!("unexpected result: {res:?}"),
        }
    }

    #[tokio::test]
    async fn handle_hole_punch_decryption_failure() {
        let alice = TestRouter::new(0).await;

        // create alice relay manager
        let mut alice_relay = RelayManager::<MockRuntime>::new(
            alice.intro_key,
            RouterContextBuilder::default()
                .with_router_info(
                    alice.router_info.clone(),
                    alice.static_key.clone(),
                    alice.signing_key.clone(),
                )
                .build(),
            Some(alice.socket.clone()),
            None,
        );

        let bob_router_id = RouterId::random();
        let charlie_router_id = RouterId::random();
        let relay_tag = 1337;
        let nonce = 1338;
        let src_id = (!(((nonce as u64) << 32) | (nonce as u64))).to_be();
        let charlie_signing_key = SigningPrivateKey::random(&mut MockRuntime::rng());

        alice_relay.active_outbound.insert(
            src_id,
            RelayProcess {
                bob_router_id: bob_router_id.clone(),
                charlie_router_id: charlie_router_id.clone(),
                created: MockRuntime::now(),
                charlie_verifying_key: charlie_signing_key.public(),
                relay_tag,
            },
        );

        match alice_relay.handle_hole_punch(vec![0u8; 128], 1337, src_id) {
            Err(Ssu2Error::Chacha) => {}
            res => panic!("unexpected result: {res:?}"),
        }
    }

    #[tokio::test]
    async fn handle_hole_punch_no_relay_block() {
        let alice = TestRouter::new(0).await;

        // create alice relay manager
        let mut alice_relay = RelayManager::<MockRuntime>::new(
            alice.intro_key,
            RouterContextBuilder::default()
                .with_router_info(
                    alice.router_info.clone(),
                    alice.static_key.clone(),
                    alice.signing_key.clone(),
                )
                .build(),
            Some(alice.socket.clone()),
            None,
        );

        let bob_router_id = RouterId::random();
        let charlie_router_id = RouterId::random();
        let relay_tag = 1337;
        let nonce = 1338;
        let src_id = (!(((nonce as u64) << 32) | (nonce as u64))).to_be();
        let dst_id = (((nonce as u64) << 32) | (nonce as u64)).to_be();
        let charlie_signing_key = SigningPrivateKey::random(&mut MockRuntime::rng());

        alice_relay.active_outbound.insert(
            src_id,
            RelayProcess {
                bob_router_id: bob_router_id.clone(),
                charlie_router_id: charlie_router_id.clone(),
                created: MockRuntime::now(),
                charlie_verifying_key: charlie_signing_key.public(),
                relay_tag,
            },
        );

        let mut pkt = TokenRequestBuilder::default()
            .with_dst_id(dst_id)
            .with_src_id(src_id)
            .with_intro_key(alice.intro_key)
            .build::<MockRuntime>()
            .to_vec();
        let mut reader = HeaderReader::new(alice.intro_key, &mut pkt).unwrap();
        let _dst_id = reader.dst_id();
        let (pkt_num, src_id) = match reader.parse(alice.intro_key).unwrap() {
            HeaderKind::TokenRequest {
                pkt_num, src_id, ..
            } => (pkt_num, src_id),
            _ => panic!("unexpected packet"),
        };

        match alice_relay.handle_hole_punch(pkt, pkt_num, src_id) {
            Err(Ssu2Error::Relay(RelayError::NoRelayResponse)) => {}
            res => panic!("unexpected result: {res:?}"),
        }
    }

    #[tokio::test]
    async fn relay_request_timeout() {
        let alice = TestRouter::new(0).await;
        let bob = TestRouter::new(2).await;
        let charlie = TestRouter::with_intorducer(4, bob.router_id.clone(), 1337).await;

        // create alice relay manager
        let mut alice_relay = RelayManager::<MockRuntime>::new(
            alice.intro_key,
            RouterContextBuilder::default()
                .with_router_info(
                    alice.router_info.clone(),
                    alice.static_key.clone(),
                    alice.signing_key.clone(),
                )
                .build(),
            Some(alice.socket.clone()),
            None,
        );

        // add bob as a router that supports relay
        let (alice_bob_tx, _alice_bob_rx) = channel(16);
        alice_relay.add_session(&bob.router_id, alice_bob_tx, true);

        // send relay request to charlie via bob
        let RelayConnection { .. } =
            alice_relay.send_relay_request(charlie.parsed(), false).unwrap();

        match timeout!(alice_relay.next(), Duration::from_secs(30)).await.unwrap().unwrap() {
            RelayManagerEvent::RelayFailure { router_id } =>
                assert_eq!(router_id, charlie.router_id),
            _ => panic!("unexpected event"),
        }
    }
}
