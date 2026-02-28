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
    primitives::{RouterId, RouterInfo, TransportKind},
    router::context::RouterContext,
    runtime::{Runtime, UdpSocket},
    transport::ssu2::{
        message::HolePunchBuilder,
        relay::types::{
            BobRejectionReason, CharlieRejectionReason, RejectionReason, RelayCommand, RelayEvent,
            RelayHandle,
        },
    },
};

use bytes::{BufMut, BytesMut};
use futures::Stream;
use hashbrown::{HashMap, HashSet};
use rand::Rng;
use thingbuf::mpsc::{channel, Receiver, Sender};

use alloc::{boxed::Box, collections::VecDeque, vec::Vec};
use core::{
    net::{IpAddr, SocketAddr},
    pin::Pin,
    task::{Context, Poll},
};

pub mod types;

/// Logging target for the file.
const LOG_TARGET: &str = "emissary::ssu2::relay";

/// Router hash length.
const ROUTER_HASH_LEN: usize = 32usize;

/// Relay client.
struct RelayClient {
    /// TX channel for sending commands to the active session.
    cmd_tx: Sender<RelayCommand>,

    /// ID of remote router.
    router_id: RouterId,
}

/// Relay server.
struct RelayServer {
    /// Router ID of Bob.
    #[allow(unused)]
    router_id: RouterId,
}

/// Relay manager.
pub struct RelayManager<R: Runtime> {
    /// Active relay processes.
    ///
    /// Indexed by nonce, the senders are used to send relay responses
    /// received from Charlie to Alice.
    active_relays: HashMap<u32, Sender<RelayCommand>>,

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

    /// Relay tags currently in use.
    relay_tags: HashSet<u32>,

    /// Router context.
    router_ctx: RouterContext<R>,

    /// Active relay servers.
    ///
    /// IOW, context for all Bob's who've agreed to act as relay for us.
    servers: HashMap<u32, RelayServer>,

    /// UDP socket.
    socket: R::UdpSocket,

    /// Tokens for inbound sessions.
    ///
    /// These are returned to `Ssu2Socket` so it can accept inbound connections
    /// that are the result of a successful relay process.
    tokens: VecDeque<u64>,

    /// Write buffer.
    write_buffer: VecDeque<(BytesMut, SocketAddr)>,
}

impl<R: Runtime> RelayManager<R> {
    /// Create new `RelayManager`.
    pub fn new(router_ctx: RouterContext<R>, socket: R::UdpSocket) -> Self {
        let (event_tx, event_rx) = channel(128);

        Self {
            clients: HashMap::new(),
            servers: HashMap::new(),
            active_relays: HashMap::new(),
            event_rx,
            event_tx,
            external_address: None,
            id_mappings: HashMap::new(),
            relay_tags: HashSet::new(),
            router_ctx,
            socket,
            tokens: VecDeque::new(),
            write_buffer: VecDeque::new(),
        }
    }

    /// Get `RelayHandle` for an active session.
    pub fn handle(&self) -> RelayHandle {
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
    #[allow(unused)]
    pub fn register_relay_server(&mut self, router_id: RouterId, relay_tag: u32) {
        tracing::debug!(
            target: LOG_TARGET,
            %router_id,
            ?relay_tag,
            num_servers = ?self.servers.len(),
            "register relay server",
        );

        self.servers.insert(relay_tag, RelayServer { router_id });
    }

    /// Register closed connection to `RelayManager`.
    pub fn register_closed_connection(&mut self, router_id: &RouterId) {
        if let Some(tag) = self.id_mappings.remove(router_id) {
            self.clients.remove(&tag);
        }
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

            // calculate signature only if the message is rejected by charlie
            let signature = core::matches!(reason, RejectionReason::Charlie(_))
                .then(|| self.router_ctx.signing_key().sign(&message));

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

                self.active_relays.insert(nonce, tx);
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

    /// Handl relay intro from Bob.
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

        let alice_address = match router_info.addresses.get(&TransportKind::Ssu2) {
            Some(address) => match address.socket_address {
                Some(address) => address,
                None => {
                    tracing::warn!(
                        target: LOG_TARGET,
                        ?relay_tag,
                        ?nonce,
                        "alice doesn't have a published ssu2 address",
                    );
                    debug_assert!(false);
                    return;
                }
            },
            None => {
                tracing::warn!(
                    target: LOG_TARGET,
                    ?relay_tag,
                    ?nonce,
                    "alice doesn't support ssu2",
                );
                debug_assert!(false);
                return;
            }
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

        let Some(intro_key) = router_info.ssu2_intro_key() else {
            tracing::warn!(
                target: LOG_TARGET,
                %alice_router_id,
                ?nonce,
                ?relay_tag,
                "no intro key for in alice's router info, rejecting",
            );
            debug_assert!(false);
            return;
        };

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
        self.tokens.push_back(token);

        if let Err(error) = tx.try_send(RelayCommand::RelayResponse {
            nonce,
            rejection: None,
            message: relay_response,
            signature: Some(signature),
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
        signature: Option<Vec<u8>>,
    ) {
        tracing::debug!(
            target: LOG_TARGET,
            ?nonce,
            "handle relay response",
        );

        match self.active_relays.remove(&nonce) {
            Some(tx) => {
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
            }
            None => tracing::debug!(
                target: LOG_TARGET,
                ?nonce,
                "active relay agreement does not exist, ignoring",
            ),
        }
    }
}

impl<R: Runtime> Stream for RelayManager<R> {
    type Item = u64;

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

        if let Some(token) = self.tokens.pop_front() {
            return Poll::Ready(Some(token));
        }

        while let Some((pkt, address)) = self.write_buffer.pop_front() {
            match Pin::new(&mut self.socket).poll_send_to(cx, &pkt, address) {
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
        crypto::{chachapoly::ChaChaPoly, SigningPrivateKey, StaticPrivateKey},
        primitives::RouterInfoBuilder,
        profile::ProfileStorage,
        router::context::builder::RouterContextBuilder,
        runtime::mock::{MockRuntime, MockUdpSocket},
        timeout,
        transport::ssu2::message::{Block, HeaderKind, HeaderReader},
        Ssu2Config,
    };
    use bytes::Bytes;
    use futures::{FutureExt, StreamExt};

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
        async fn new(seed: u8) -> Self {
            let socket = <MockRuntime as Runtime>::UdpSocket::bind("127.0.0.1:0".parse().unwrap())
                .await
                .unwrap();
            let (router_info, static_key, signing_key) = RouterInfoBuilder::default()
                .with_ssu2(Ssu2Config {
                    port: socket.local_address().unwrap().port(),
                    host: Some("127.0.0.1".parse().unwrap()),
                    publish: true,
                    static_key: [seed; 32],
                    intro_key: [seed + 1; 32],
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
            bob.socket.clone(),
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
        assert!(relay.active_relays.get(&nonce).is_some());
    }

    #[tokio::test]
    async fn relay_request_session_not_found() {
        let alice = TestRouter::new(0).await;
        let bob = TestRouter::new(2).await;
        let charlie = TestRouter::new(4).await;

        // we are bob
        let mut relay = RelayManager::<MockRuntime>::new(
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
            bob.socket.clone(),
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
                signature,
                token,
            } => {
                assert_eq!(nonce, 1338);
                assert_eq!(
                    rejection,
                    Some(RejectionReason::Bob(BobRejectionReason::RelayTagNotFound))
                );
                assert_eq!(token, None);
                assert_eq!(signature, None);

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
        assert!(relay.active_relays.get(&nonce).is_none());
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
            RouterContextBuilder::default()
                .with_router_info(
                    bob.router_info.clone(),
                    bob.static_key.clone(),
                    bob.signing_key.clone(),
                )
                .build(),
            bob.socket.clone(),
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
                signature,
                token,
            } => {
                assert_eq!(nonce, 1338);
                assert_eq!(
                    rejection,
                    Some(RejectionReason::Bob(BobRejectionReason::AliceNotFound))
                );
                assert_eq!(token, None);
                assert_eq!(signature, None);

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
        assert!(relay.active_relays.get(&nonce).is_none());
    }

    #[tokio::test]
    async fn relay_request_invalid_signature() {
        let alice = TestRouter::new(0).await;
        let bob = TestRouter::new(2).await;
        let charlie = TestRouter::new(4).await;

        // we are bob
        let mut relay = RelayManager::<MockRuntime>::new(
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
            bob.socket.clone(),
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
                signature,
                token,
            } => {
                assert_eq!(nonce, 1338);
                assert_eq!(
                    rejection,
                    Some(RejectionReason::Bob(BobRejectionReason::SignatureFailure))
                );
                assert_eq!(token, None);
                assert_eq!(signature, None);

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
        assert!(relay.active_relays.get(&nonce).is_none());
    }

    #[tokio::test]
    async fn relay_intro_accepted() {
        let mut alice = TestRouter::new(0).await;
        let bob = TestRouter::new(2).await;
        let charlie = TestRouter::new(4).await;

        // we are charlie
        let mut relay = RelayManager::<MockRuntime>::new(
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
            charlie.socket.clone(),
        );

        // create context for the test
        let relay_tag = 1337;
        let nonce = 1338;
        let (bob_tx, bob_rx) = channel(16);

        // add external address for charlie
        relay.add_external_address(charlie.socket.local_address().unwrap());

        // register bob as relay server
        relay.register_relay_server(bob.router_id.clone(), relay_tag);

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
                signature,
                token,
                ..
            } => {
                assert_eq!(nonce, 1338);
                assert_eq!(rejection, None);
                assert_ne!(signature, None);
                assert_ne!(token, None);
            }
            _ => panic!("invalid command"),
        }

        // verify the relay manager returns a token for `SessionRequest`
        let session_request_token = relay.next().now_or_never().unwrap();

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

        assert!(Block::parse(&pkt).unwrap().iter().any(|block| match block {
            Block::RelayResponse { token, .. } => token == &session_request_token,
            _ => false,
        }));
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
            RouterContextBuilder::default()
                .with_router_info(
                    charlie.router_info.clone(),
                    charlie.static_key.clone(),
                    charlie.signing_key.clone(),
                )
                .build(),
            charlie.socket.clone(),
        );

        // create context for the test
        let relay_tag = 1337;
        let nonce = 1338;
        let (bob_tx, bob_rx) = channel(16);

        // add external address for charlie
        relay.add_external_address(charlie.socket.local_address().unwrap());

        // register bob as relay server
        relay.register_relay_server(bob.router_id.clone(), relay_tag);

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
                signature,
                token,
                ..
            } => {
                assert_eq!(nonce, 1338);
                assert_eq!(rejection, None);
                assert_ne!(signature, None);
                assert_ne!(token, None);
            }
            _ => panic!("invalid command"),
        }

        // verify the relay manager returns a token for `SessionRequest`
        let session_request_token = relay.next().now_or_never().unwrap();

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

        assert!(Block::parse(&pkt).unwrap().iter().any(|block| match block {
            Block::RelayResponse { token, .. } => token == &session_request_token,
            _ => false,
        }));
    }

    #[tokio::test]
    async fn relay_intro_relay_server_not_found() {
        let alice = TestRouter::new(0).await;
        let bob = TestRouter::new(2).await;
        let charlie = TestRouter::new(4).await;

        // we are charlie
        let mut relay = RelayManager::<MockRuntime>::new(
            RouterContextBuilder::default()
                .with_router_info(
                    charlie.router_info.clone(),
                    charlie.static_key.clone(),
                    charlie.signing_key.clone(),
                )
                .build(),
            charlie.socket.clone(),
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
                signature,
                token,
                ..
            } => {
                assert_eq!(nonce, 1338);
                assert_ne!(signature, None);
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
            RouterContextBuilder::default()
                .with_router_info(
                    charlie.router_info.clone(),
                    charlie.static_key.clone(),
                    charlie.signing_key.clone(),
                )
                .build(),
            charlie.socket.clone(),
        );

        // create context for the test
        let relay_tag = 1337;
        let nonce = 1338;
        let (bob_tx, bob_rx) = channel(16);

        // add external address for charlie
        relay.add_external_address(charlie.socket.local_address().unwrap());

        // register bob as relay server
        relay.register_relay_server(bob.router_id.clone(), relay_tag);

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
                signature,
                token,
                ..
            } => {
                assert_eq!(nonce, 1338);
                assert_ne!(signature, None);
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
            RouterContextBuilder::default()
                .with_router_info(
                    charlie.router_info.clone(),
                    charlie.static_key.clone(),
                    charlie.signing_key.clone(),
                )
                .build(),
            charlie.socket.clone(),
        );

        // create context for the test
        let relay_tag = 1337;
        let nonce = 1338;
        let (bob_tx, bob_rx) = channel(16);

        // add external address for charlie
        relay.add_external_address(charlie.socket.local_address().unwrap());

        // register bob as relay server
        relay.register_relay_server(bob.router_id.clone(), relay_tag);

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
                signature,
                token,
                ..
            } => {
                assert_eq!(nonce, 1338);
                assert_ne!(signature, None);
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
            RouterContextBuilder::default()
                .with_router_info(
                    charlie.router_info.clone(),
                    charlie.static_key.clone(),
                    charlie.signing_key.clone(),
                )
                .build(),
            charlie.socket.clone(),
        );

        // create context for the test
        let relay_tag = 1337;
        let nonce = 1338;
        let (bob_tx, bob_rx) = channel(16);

        // charlie doesn't have external address so the intro gets rejected

        // register bob as relay server
        relay.register_relay_server(bob.router_id.clone(), relay_tag);

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
                signature,
                token,
                ..
            } => {
                assert_eq!(nonce, 1338);
                assert_ne!(signature, None);
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
            RouterContextBuilder::default()
                .with_router_info(
                    bob.router_info.clone(),
                    bob.static_key.clone(),
                    bob.signing_key.clone(),
                )
                .build(),
            bob.socket.clone(),
        );

        // create context for the test
        let nonce = 1338;
        let (alice_tx, alice_rx) = channel(16);

        // create new active relay manually
        relay.active_relays.insert(nonce, alice_tx);

        // handle relay response for an active session
        relay.handle_relay_response(
            nonce,
            None,
            None,
            Some(RejectionReason::Charlie(
                CharlieRejectionReason::Unspecified,
            )),
            vec![],
            Some(vec![]),
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
                assert_eq!(signature, Some(vec![]));
                assert_eq!(token, None);
            }
            _ => panic!("invalid command"),
        }
        assert!(!relay.active_relays.contains_key(&nonce));
    }

    #[tokio::test]
    async fn relay_response_no_session() {
        let bob = TestRouter::new(2).await;

        // we are bob
        let mut relay = RelayManager::<MockRuntime>::new(
            RouterContextBuilder::default()
                .with_router_info(
                    bob.router_info.clone(),
                    bob.static_key.clone(),
                    bob.signing_key.clone(),
                )
                .build(),
            bob.socket.clone(),
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
            Some(vec![]),
        );
        assert!(!relay.active_relays.contains_key(&nonce));
    }
}
