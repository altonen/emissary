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
use rand_core::RngCore;
use thingbuf::mpsc::{channel, Receiver, Sender};

use alloc::collections::VecDeque;
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

/// Relay manager.
pub struct RelayManager<R: Runtime> {
    /// Active relay clients.
    ///
    /// IOW, context for all Charlies we've agreed to act as a relay for.
    active_inbound: HashMap<u32, RelayClient>,

    /// Active relay processes.
    ///
    /// Indexed by nonce, the senders are used to send relay responses
    /// received from Charlie to Alice.
    active_relays: HashMap<u32, Sender<RelayCommand>>,

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
            active_inbound: HashMap::new(),
            active_relays: HashMap::new(),
            event_rx,
            event_tx,
            id_mappings: HashMap::new(),
            tokens: VecDeque::new(),
            external_address: None,
            relay_tags: HashSet::new(),
            router_ctx,
            socket,
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
            "register relay client",
        );

        self.id_mappings.insert(router_id.clone(), relay_tag);
        self.active_inbound.insert(relay_tag, RelayClient { cmd_tx, router_id });
    }

    /// Register closed connection to `RelayManager`.
    pub fn register_closed_connection(&mut self, router_id: &RouterId) {
        if let Some(tag) = self.id_mappings.remove(router_id) {
            self.active_inbound.remove(&tag);
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
        }) = self.active_inbound.get(&relay_tag)
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
                    "failed to verify siganture, rejecting relay request",
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
        _message: Vec<u8>,
        _signature: Vec<u8>,
        tx: Sender<RelayCommand>,
    ) {
        tracing::debug!(
            target: LOG_TARGET,
            ?nonce,
            ?relay_tag,
            "handle relay intro",
        );

        let router_info = match alice_router_info {
            Some(router_info) => *router_info,
            None => match self.router_ctx.profile_storage().get(&alice_router_id) {
                Some(router_info) => router_info,
                None => {
                    tracing::debug!(
                        target: LOG_TARGET,
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
                        "alice doesn't have a published ssu2 address",
                    );
                    debug_assert!(false);
                    return;
                }
            },
            None => {
                tracing::warn!(
                    target: LOG_TARGET,
                    "alice doesn't support ssu2",
                );
                debug_assert!(false);
                return;
            }
        };

        // TODO: verify `signature`

        let Some(intro_key) = router_info.ssu2_intro_key() else {
            tracing::warn!(
                target: LOG_TARGET,
                %alice_router_id,
                ?nonce,
                ?relay_tag,
                "no intro key for in alice's router info, rejecting",
            );
            debug_assert!(false);

            return self.reject_relay(
                nonce,
                RejectionReason::Charlie(CharlieRejectionReason::Unspecified),
                &bob_router_id,
                tx,
            );
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
