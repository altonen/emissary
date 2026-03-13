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

//! Relay protocol for an active SSU2 session.

use crate::{
    primitives::RouterId,
    runtime::{Counter, MetricsHandle, Runtime},
    transport::ssu2::{
        message::data::RelayBlock,
        metrics::DUPLICATE_PKT_COUNT,
        relay::types::{RejectionReason, RelayCommand},
        session::active::Ssu2Session,
    },
};

use alloc::vec::Vec;
use core::net::SocketAddr;

/// Logging target for the file.
const LOG_TARGET: &str = "emissary::ssu2::active::relay";

impl<R: Runtime> Ssu2Session<R> {
    /// Insert `nonce` into duplicate filter and return `true` if `nonce` is unique.
    fn insert_relay_message(&mut self, nonce: u32) -> bool {
        if self.duplicate_filter.insert(nonce) {
            return true;
        }

        tracing::debug!(
            target: LOG_TARGET,
            router_id = %self.router_id,
            ?nonce,
            "ignoring duplicate message",
        );
        self.router_ctx.metrics_handle().counter(DUPLICATE_PKT_COUNT).increment(1);

        false
    }

    /// Handle relay request.
    pub fn handle_relay_request(
        &mut self,
        nonce: u32,
        relay_tag: u32,
        address: SocketAddr,
        message: Vec<u8>,
        signature: Vec<u8>,
    ) {
        if self.insert_relay_message(nonce) {
            self.relay_handle.handle_relay_request(
                self.router_id.clone(),
                nonce,
                relay_tag,
                address,
                message,
                signature,
            );
        }
    }

    /// Handle relay intro.
    pub fn handle_relay_intro(
        &mut self,
        alice_router_id: RouterId,
        nonce: u32,
        relay_tag: u32,
        address: SocketAddr,
        message: Vec<u8>,
        signature: Vec<u8>,
    ) {
        if self.insert_relay_message(nonce) {
            self.relay_handle.handle_relay_intro(
                alice_router_id,
                self.router_id.clone(),
                self.pending_router_info.take(),
                nonce,
                relay_tag,
                address,
                message,
                signature,
            );
        }
    }

    /// Handle relay response.
    pub fn handle_relay_response(
        &mut self,
        nonce: u32,
        address: Option<SocketAddr>,
        token: Option<u64>,
        rejection: Option<RejectionReason>,
        message: Vec<u8>,
        signature: Vec<u8>,
    ) {
        if self.insert_relay_message(nonce) {
            self.relay_handle
                .handle_relay_response(nonce, address, token, rejection, message, signature);
        }
    }

    /// Handle command received from `RelayManager`.
    pub fn handle_relay_command(&mut self, command: RelayCommand) {
        match command {
            RelayCommand::RelayRequest {
                nonce,
                message,
                signature,
            } => {
                tracing::trace!(
                    target: LOG_TARGET,
                    router_id = %self.router_id,
                    ?nonce,
                    "send relay request",
                );

                self.transmission.schedule(RelayBlock::Request { message, signature });
            }

            RelayCommand::RelayResponse {
                nonce,
                rejection,
                message,
                signature,
                token,
            } => {
                tracing::trace!(
                    target: LOG_TARGET,
                    router_id = %self.router_id,
                    ?nonce,
                    "send relay response",
                );

                self.transmission.schedule(RelayBlock::Response {
                    rejection,
                    message,
                    signature,
                    token,
                });
            }
            RelayCommand::RelayIntro {
                router_id,
                router_info,
                message,
                signature,
            } => {
                tracing::trace!(
                    target: LOG_TARGET,
                    router_id = %self.router_id,
                    "send relay intro",
                );

                self.transmission.schedule((
                    RelayBlock::Intro {
                        router_id,
                        message,
                        signature,
                    },
                    router_info,
                ));
            }
            RelayCommand::Dummy => unreachable!(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        crypto::SigningPrivateKey,
        router::context::builder::RouterContextBuilder,
        runtime::{mock::MockRuntime, UdpSocket},
        subsystem::SubsystemEvent,
        transport::ssu2::{
            peer_test::types::{PeerTestEvent, PeerTestEventRecycle, PeerTestHandle},
            relay::types::{RelayEvent, RelayHandle},
            session::{active::Ssu2SessionContext, KeyContext},
            Packet,
        },
    };
    use thingbuf::mpsc::{channel, with_recycle, Receiver, Sender};

    async fn make_session() -> (
        Ssu2Session<MockRuntime>,
        Sender<Packet>,
        Receiver<SubsystemEvent>,
        Receiver<PeerTestEvent, PeerTestEventRecycle>,
        Receiver<RelayEvent>,
    ) {
        let (pkt_tx, pkt_rx) = channel(16);
        let ctx = Ssu2SessionContext {
            address: "127.0.0.1:8888".parse().unwrap(),
            dst_id: 1337,
            intro_key: [0xaa; 32],
            pkt_rx,
            recv_key_ctx: KeyContext {
                k_data: [0xbb; 32],
                k_header_2: [0xcc; 32],
            },
            router_id: RouterId::random(),
            send_key_ctx: KeyContext {
                k_data: [0xdd; 32],
                k_header_2: [0xee; 32],
            },
            verifying_key: SigningPrivateKey::random(&mut MockRuntime::rng()).public(),
        };

        let socket = <MockRuntime as Runtime>::UdpSocket::bind("127.0.0.1:0".parse().unwrap())
            .await
            .unwrap();
        let (transport_tx, transport_rx) = channel(16);
        let router_ctx = RouterContextBuilder::default().build();
        let (peer_test_tx, peer_test_rx) = with_recycle(16, PeerTestEventRecycle::default());
        let peer_test_handle = PeerTestHandle::new(peer_test_tx);
        let (relay_tx, relay_rx) = channel(16);
        let relay_handle = RelayHandle::new(relay_tx);

        (
            Ssu2Session::<MockRuntime>::new(
                ctx,
                socket,
                transport_tx,
                router_ctx,
                peer_test_handle,
                relay_handle,
            ),
            pkt_tx,
            transport_rx,
            peer_test_rx,
            relay_rx,
        )
    }

    #[tokio::test]
    async fn duplicate_relay_request() {
        let (mut session, _pkt_tx, _transport_rx, _peer_test_rx, relay_rx) = make_session().await;

        // relay request received
        session.handle_relay_request(
            1337,
            1338,
            "127.0.0.1:9999".parse().unwrap(),
            vec![1, 3, 3, 7],
            vec![0xff; 64],
        );

        // relay request sent to relay manager
        match relay_rx.try_recv().unwrap() {
            RelayEvent::RelayRequest {
                nonce,
                relay_tag,
                message,
                ..
            } => {
                assert_eq!(nonce, 1337);
                assert_eq!(relay_tag, 1338);
                assert_eq!(message, vec![1, 3, 3, 7]);
            }
            _ => panic!("invalid event"),
        }

        // relay request gets retransmitted
        session.handle_relay_request(
            1337,
            1338,
            "127.0.0.1:9999".parse().unwrap(),
            vec![1, 3, 3, 7],
            vec![0xff; 64],
        );

        // verify that the duplicate message is not sent to relay manager
        assert!(relay_rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn duplicate_relay_intro() {
        let (mut session, _pkt_tx, _transport_rx, _peer_test_rx, relay_rx) = make_session().await;

        // relay intro received
        session.handle_relay_intro(
            RouterId::random(),
            1337,
            1338,
            "127.0.0.1:9999".parse().unwrap(),
            vec![1, 3, 3, 7],
            vec![0xff; 64],
        );

        // relay request sent to relay manager
        match relay_rx.try_recv().unwrap() {
            RelayEvent::RelayIntro {
                nonce,
                relay_tag,
                message,
                ..
            } => {
                assert_eq!(nonce, 1337);
                assert_eq!(relay_tag, 1338);
                assert_eq!(message, vec![1, 3, 3, 7]);
            }
            _ => panic!("invalid event"),
        }

        // relay intro gets retransmitted
        session.handle_relay_intro(
            RouterId::random(),
            1337,
            1338,
            "127.0.0.1:9999".parse().unwrap(),
            vec![1, 3, 3, 7],
            vec![0xff; 64],
        );

        // verify that the duplicate message is not sent to relay manager
        assert!(relay_rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn duplicate_relay_response() {
        let (mut session, _pkt_tx, _transport_rx, _peer_test_rx, relay_rx) = make_session().await;

        // relay response received
        session.handle_relay_response(
            1337,
            None,
            None,
            Some(RejectionReason::Unspecified),
            vec![1, 3, 3, 7],
            vec![0xaa; 64],
        );

        // relay response sent to relay manager
        match relay_rx.try_recv().unwrap() {
            RelayEvent::RelayResponse {
                nonce,
                rejection,
                message,
                ..
            } => {
                assert_eq!(nonce, 1337);
                assert_eq!(rejection, Some(RejectionReason::Unspecified));
                assert_eq!(message, vec![1, 3, 3, 7]);
            }
            _ => panic!("invalid event"),
        }

        // relay response gets retransmitted
        session.handle_relay_response(
            1337,
            None,
            None,
            Some(RejectionReason::Unspecified),
            vec![1, 3, 3, 7],
            vec![0xaa; 64],
        );

        // verify that the duplicate message is not sent to relay manager
        assert!(relay_rx.try_recv().is_err());
    }
}
