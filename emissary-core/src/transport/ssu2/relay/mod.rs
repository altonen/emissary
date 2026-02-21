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

#![allow(unused)]

use crate::{
    primitives::RouterId,
    router::context::RouterContext,
    runtime::Runtime,
    transport::ssu2::relay::types::{
        RejectionReason, RelayCommand, RelayEvent, RelayHandle, RelayTagRequested,
    },
};

use bytes::{BufMut, BytesMut};
use hashbrown::{HashMap, HashSet};
use rand_core::RngCore;
use thingbuf::mpsc::{channel, Receiver, Sender};

use core::{
    future::Future,
    net::SocketAddr,
    pin::Pin,
    task::{Context, Poll},
};

pub mod types;

/// Logging target for the file.
const LOG_TARGET: &str = "emissary::ssu2::relay";

/// Bob's relay request rejection message size.
///
/// Prologue (16) + router hash (32) + nonce (4) + timestamp (4) + version (1) + address size (1).
const BOB_REJECT_MESSAGE_LEN: usize = 58usize;

/// Router hash length.
const ROUTER_HASH_LEN: usize = 32usize;

/// Relay manager.
pub struct RelayManager<R: Runtime> {
    /// Active inbound relay agreements.
    ///
    /// IOW, context for all Charlies we've agreed to act as a relay for.
    active_inbound: HashMap<u32, ()>,

    /// RX channel for receiving relay events.
    event_rx: Receiver<RelayEvent>,

    /// TX channel given to `RelayHandle`s.
    event_tx: Sender<RelayEvent>,

    /// Relay tags currently in use.
    relay_tags: HashSet<u32>,

    /// Router context.
    router_ctx: RouterContext<R>,
}

impl<R: Runtime> RelayManager<R> {
    /// Create new `RelayManager`.
    pub fn new(router_ctx: RouterContext<R>) -> Self {
        let (event_tx, event_rx) = channel(128);

        Self {
            active_inbound: HashMap::new(),
            event_rx,
            event_tx,
            relay_tags: HashSet::new(),
            router_ctx,
        }
    }

    /// Get `RelayHandle` for an active session.
    pub fn handle(&self) -> RelayHandle {
        RelayHandle::new(self.event_tx.clone())
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

    pub fn register_relay_tag_request_result(&mut self, relay_tag_request: RelayTagRequested) {
        match relay_tag_request {
            RelayTagRequested::Yes(_) => {
                // TODO: do something here
            }
            RelayTagRequested::No(tag) => self.deallocate_relay_tag(tag),
        }
    }

    /// Send relay request rejection.
    fn reject_relay_request(&self, nonce: u32, reason: RejectionReason, tx: Sender<RelayCommand>) {
        let (message, signature) = {
            let mut message = BytesMut::with_capacity(58);
            message.put_slice(b"RelayAgreementOK");
            message.put_slice(&self.router_ctx.router_id().to_vec());
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
        timestamp: u32,
        address: SocketAddr,
        message: Vec<u8>,
        signature: Vec<u8>,
        tx: Sender<RelayCommand>,
    ) {
        tracing::trace!(
            target: LOG_TARGET,
            %alice_router_id,
            ?nonce,
            ?relay_tag,
            ?address,
            "handle relay request",
        );

        let Some(ctx) = self.active_inbound.get(&relay_tag) else {
            tracing::debug!(
                target: LOG_TARGET,
                %alice_router_id,
                ?nonce,
                ?relay_tag,
                "relay agreement doesn't exist, rejecting",
            );

            return self.reject_relay_request(nonce, RejectionReason::RelayTagNotFound, tx);
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
                    tracing::debug!(
                        target: LOG_TARGET,
                        %alice_router_id,
                        router_info_found = %router_info.is_some(),
                        serialized_found = %serialized.is_some(),
                        "alice's router info not available, ignoring relay request",
                    );

                    return self.reject_relay_request(nonce, RejectionReason::AliceNotFound, tx);
                }
            }
        };

        // let Some(router_info) = self.router_ctx.profile_storage().get(&alice_router_id) else {
        //     tracing::warn!(
        //         target: LOG_TARGET,
        //         %alice_router_id,
        //         ?nonce,
        //         ?relay_tag,
        //         "alice router info not found, cannot handle relay request",
        //     );
        //     debug_assert!(false);
        //     return;
        // };
    }
}

impl<R: Runtime> Future for RelayManager<R> {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        loop {
            match self.event_rx.poll_recv(cx) {
                Poll::Pending => break,
                Poll::Ready(None) => return Poll::Ready(()),
                Poll::Ready(Some(event)) => match event {
                    RelayEvent::RelayRequest {
                        alice_router_id,
                        nonce,
                        relay_tag,
                        timestamp,
                        address,
                        message,
                        signature,
                        tx,
                    } => self.handle_relay_request(
                        alice_router_id,
                        nonce,
                        relay_tag,
                        timestamp,
                        address,
                        message,
                        signature,
                        tx,
                    ),
                    RelayEvent::Dummy => unreachable!(),
                },
            }
        }

        Poll::Pending
    }
}
