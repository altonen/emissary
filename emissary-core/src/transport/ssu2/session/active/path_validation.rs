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

//! Connection migration.
//!
//! <https://i2p.net/en/docs/specs/ssu2/#connection-migration-1>

use crate::{
    runtime::{Counter, Instant, MetricsHandle, Runtime},
    transport::ssu2::{
        message::data::PathValidationBlock, metrics::*, session::active::Ssu2Session,
    },
};

use futures::FutureExt;
use rand::Rng;

use alloc::{vec, vec::Vec};
use core::{
    future::Future,
    net::{IpAddr, SocketAddr},
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

/// Logging target for the file.
const LOG_TARGET: &str = "emissary::ssu2::active::path-validation";

/// Maximum length for a challenge.
const CHALLENGE_MAX_SIZE: usize = 512usize;

/// Timeout for path validation.
const PATH_VALIDATION_TIMEOUT: Duration = Duration::from_secs(5);

/// Path validation state.
#[derive(Default)]
pub enum PathValidationState<R: Runtime> {
    /// No path validation active.
    #[default]
    Inactive,

    /// Path validation is in progress.
    Active {
        /// New address of the remote router.
        address: SocketAddr,

        /// Challenge for the path validation.
        challenge: Vec<u8>,

        /// When was the validation started.
        started: R::Instant,

        /// Timeout for path response.
        ///
        /// Doubled every time the path response times out.
        timeout: Duration,

        /// Timer for path response timeout.
        timer: R::Timer,
    },
}

impl<R: Runtime> Future for PathValidationState<R> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match Pin::into_inner(self) {
            Self::Inactive => Poll::Pending,
            Self::Active { timer, .. } => timer.poll_unpin(cx),
        }
    }
}

impl<R: Runtime> Ssu2Session<R> {
    /// Handle path challenge for our address.
    ///
    /// `address` is remote router's observed address that must be sent in `PathResponse`.
    pub fn handle_path_challenge(&mut self, address: SocketAddr, challenge: Vec<u8>) {
        tracing::trace!(
            target: LOG_TARGET,
            router_id = %self.router_id,
            "handle path challenge",
        );

        if challenge.len() > CHALLENGE_MAX_SIZE {
            tracing::warn!(
                target: LOG_TARGET,
                router_id = %self.router_id,
                challenge_len = ?challenge.len(),
                "rejecting excessively large path challenge",
            );
            return;
        }

        self.router_ctx
            .metrics_handle()
            .counter(NUM_INBOUND_PATH_CHALLENGES)
            .increment(1);
        self.transmission.schedule(PathValidationBlock::Response { challenge, address });
    }

    /// Validate packet address.
    ///
    /// If `new_address` is different from `self.address` and `pkt_num` is not a duplicate packet,
    /// attempt to start path validation for the new address.
    ///
    /// Validation is skipped if path validation is already active and an active validation is
    /// canceled if `new_address` is the same as `self.address`, meaning the old path works.
    ///
    /// <https://i2p.net/en/docs/specs/ssu2/#initiating-path-validation>
    pub fn validate_pkt_address(&mut self, new_address: SocketAddr, pkt_num: u32) {
        if self.transmission.is_duplicate(pkt_num) {
            return;
        }

        match core::mem::replace(
            &mut self.path_validation_state,
            PathValidationState::Inactive,
        ) {
            PathValidationState::Active {
                address,
                challenge,
                started,
                timeout,
                timer,
            } => {
                // if path validation is active for a new address but a valid, non-duplicate packet
                // is received, active validation is canceled as the old path is considered working
                //
                // <https://i2p.net/en/docs/specs/ssu2/#cancelling-path-validation>
                if self.address == new_address {
                    tracing::debug!(
                        target: LOG_TARGET,
                        current_address = ?self.address,
                        "current path works, cancelling active path validation",
                    );
                    self.transmission.unthrottle(false);
                    return;
                }

                // path validation already active, don't start a new validation task
                self.path_validation_state = PathValidationState::Active {
                    address,
                    challenge,
                    started,
                    timeout,
                    timer,
                };
            }
            PathValidationState::Inactive => {
                // path is the same, no need to do validation
                if self.address == new_address {
                    return;
                }

                match (self.address.ip(), new_address.ip()) {
                    (IpAddr::V4(_), IpAddr::V4(_)) => {}
                    (IpAddr::V6(_), IpAddr::V6(_)) => {}
                    (_, _) => {
                        tracing::warn!(
                            target: LOG_TARGET,
                            current_address = ?self.address,
                            ?new_address,
                            "cross-protocol address change, ignoring",
                        );
                        return;
                    }
                }

                let challenge = {
                    let mut challenge = vec![0u8; (R::rng().next_u32() % 16 + 8) as usize];
                    R::rng().fill_bytes(&mut challenge);

                    challenge
                };

                tracing::trace!(
                    target: LOG_TARGET,
                    router_id = %self.router_id,
                    current_address = ?self.address,
                    ?new_address,
                    "send path challenge",
                );

                self.transmission.throttle();
                self.transmission.schedule(PathValidationBlock::Challenge {
                    challenge: challenge.clone(),
                    address: new_address,
                });
                self.path_validation_state = PathValidationState::Active {
                    address: new_address,
                    challenge,
                    started: R::now(),
                    timeout: self.transmission.round_trip_time() * 2,
                    timer: R::timer(self.transmission.round_trip_time() * 2),
                };
            }
        }
    }

    /// Handle path response.
    pub fn handle_path_response(&mut self, challenge_response: Vec<u8>) {
        let PathValidationState::Active {
            challenge, address, ..
        } = core::mem::replace(
            &mut self.path_validation_state,
            PathValidationState::Inactive,
        )
        else {
            return;
        };

        if challenge != challenge_response {
            tracing::debug!(
                target: LOG_TARGET,
                router_id = %self.router_id,
                current_address = ?self.address,
                new_address = ?address,
                "challenge mismatch, keeping old path",
            );

            self.router_ctx
                .metrics_handle()
                .counter(NUM_PATH_CHALLENGES)
                .increment_with_label(1, "kind", "wrong-challenge");
            self.transmission.unthrottle(false);

            return;
        }

        tracing::trace!(
            target: LOG_TARGET,
            router_id = %self.router_id,
            current_address = ?self.address,
            new_address = ?address,
            "path validation succeeded",
        );

        // if the ip address changed, rto and rtt measurements must be reset
        //
        // <https://i2p.net/en/docs/specs/ssu2/#successful-path-validation>
        self.transmission.unthrottle(self.address.ip() != address.ip());

        self.address = address;
        self.router_ctx
            .metrics_handle()
            .counter(NUM_PATH_CHALLENGES)
            .increment_with_label(1, "kind", "success");
    }

    /// Handle path response timeout.
    ///
    /// If the validation hasn't timed out, resend the path challenge.
    ///
    /// If path validation has reached its time limit, treat the path validation as failed.
    pub fn handle_path_response_timeout(&mut self) {
        let PathValidationState::Active {
            challenge,
            address,
            started,
            timeout,
            ..
        } = core::mem::replace(
            &mut self.path_validation_state,
            PathValidationState::Inactive,
        )
        else {
            tracing::error!(
                target: LOG_TARGET,
                router_id = %self.router_id,
                "path response timed out but path validation is not active",
            );
            debug_assert!(false);
            return;
        };

        if started.elapsed() >= PATH_VALIDATION_TIMEOUT {
            tracing::debug!(
                target: LOG_TARGET,
                router_id = %self.router_id,
                "path challenge timed out",
            );
            self.router_ctx
                .metrics_handle()
                .counter(NUM_PATH_CHALLENGES)
                .increment_with_label(1, "kind", "timeout");
            self.transmission.unthrottle(false);
            return;
        }

        tracing::trace!(
            target: LOG_TARGET,
            router_id = %self.router_id,
            "path challenge timed out, sending another",
        );

        self.transmission.schedule(PathValidationBlock::Challenge {
            challenge: challenge.clone(),
            address,
        });
        self.path_validation_state = PathValidationState::Active {
            address,
            challenge,
            started,
            timeout: timeout * 2,
            timer: R::timer(timeout * 2),
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        crypto::SigningPrivateKey,
        primitives::RouterId,
        router::context::builder::RouterContextBuilder,
        runtime::{mock::MockRuntime, UdpSocket},
        transport::ssu2::{
            peer_test::types::{PeerTestEventRecycle, PeerTestHandle},
            relay::types::RelayHandle,
            session::{active::Ssu2SessionContext, KeyContext},
        },
    };
    use std::net::{Ipv4Addr, SocketAddrV4};
    use thingbuf::mpsc::{channel, with_recycle};

    const NEW_ADDRESS: SocketAddr =
        SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 7777));

    const OLD_ADDRESS: SocketAddr =
        SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 8888));

    async fn make_session() -> Ssu2Session<MockRuntime> {
        let (_socket_tx, socket_rx) = channel(128);
        let remote_signing_key = SigningPrivateKey::random(&mut MockRuntime::rng());
        let socket = <MockRuntime as Runtime>::UdpSocket::bind("127.0.0.1:0".parse().unwrap())
            .await
            .unwrap();
        let router_ctx = RouterContextBuilder::default().build();

        let ctx = Ssu2SessionContext {
            address: OLD_ADDRESS,
            dst_id: 1337u64,
            intro_key: [1u8; 32],
            pkt_rx: socket_rx,
            max_payload_size: 1472,
            recv_key_ctx: KeyContext {
                k_data: [2u8; 32],
                k_header_2: [3u8; 32],
            },
            router_id: RouterId::random(),
            send_key_ctx: KeyContext {
                k_data: [3u8; 32],
                k_header_2: [2u8; 32],
            },
            verifying_key: remote_signing_key.public(),
        };
        let (peer_test_event_tx, _peer_test_event_rx) =
            with_recycle(16, PeerTestEventRecycle::default());
        let peer_test_handle = PeerTestHandle::new(peer_test_event_tx);
        let (relay_event_tx, _relay_event_rx) = channel(16);
        let relay_handle = RelayHandle::new(relay_event_tx);
        let (transport_tx, _) = channel(16);

        Ssu2Session::<MockRuntime>::new(
            ctx,
            socket,
            transport_tx,
            router_ctx,
            peer_test_handle,
            relay_handle,
        )
    }

    #[tokio::test]
    async fn path_response_sent_to_new_address() {
        let mut session = make_session().await;

        session.handle_path_challenge(NEW_ADDRESS, vec![1, 3, 3, 7]);
        let pkts = session.transmission.drain().unwrap();
        let mut iter = pkts.into_iter();

        let (_pkt, address) = iter.next().unwrap();
        assert_eq!(address, Some(NEW_ADDRESS));
        assert!(iter.next().is_none());
    }

    #[tokio::test]
    async fn path_validation_succeeds() {
        let mut session = make_session().await;
        assert!(std::matches!(
            session.path_validation_state,
            PathValidationState::Inactive
        ));
        assert_eq!(session.address, OLD_ADDRESS);

        // packet with new address received
        session.validate_pkt_address(NEW_ADDRESS, 128);
        assert!(session.transmission.is_throttled());

        // verify that a packet to the new address is queued
        {
            let pkts = session.transmission.drain().unwrap();
            let mut iter = pkts.into_iter();

            let (_pkt, address) = iter.next().unwrap();
            assert_eq!(address, Some(NEW_ADDRESS));
            assert!(iter.next().is_none());
        }

        let challenge = match &session.path_validation_state {
            PathValidationState::Inactive => panic!("invalid state"),
            PathValidationState::Active {
                address, challenge, ..
            } => {
                assert_eq!(*address, NEW_ADDRESS);
                assert_ne!(*address, session.address);

                challenge.clone()
            }
        };

        session.handle_path_response(challenge);
        assert!(std::matches!(
            session.path_validation_state,
            PathValidationState::Inactive
        ));
        assert!(!session.transmission.is_throttled());
        assert_eq!(session.address, NEW_ADDRESS);
    }

    #[tokio::test]
    async fn path_validation_timeouts_then_succeeds() {
        let mut session = make_session().await;
        assert!(std::matches!(
            session.path_validation_state,
            PathValidationState::Inactive
        ));
        assert_eq!(session.address, OLD_ADDRESS);

        session.validate_pkt_address(NEW_ADDRESS, 128);
        assert!(session.transmission.is_throttled());

        // verify that a packet to the new address is queued
        {
            let pkts = session.transmission.drain().unwrap();
            let mut iter = pkts.into_iter();

            let (_pkt, address) = iter.next().unwrap();
            assert_eq!(address, Some(NEW_ADDRESS));
            assert!(iter.next().is_none());
        }

        let initial_timeout = match &mut session.path_validation_state {
            PathValidationState::Inactive => panic!("invalid state"),
            PathValidationState::Active {
                address,
                timer,
                timeout,
                ..
            } => {
                assert_eq!(*address, NEW_ADDRESS);
                assert_ne!(*address, session.address);

                // wait until the timer expires
                futures::future::poll_fn(|cx| timer.poll_unpin(cx)).await;

                *timeout
            }
        };

        // handle path response timeout and verify that a new packet is sent
        {
            session.handle_path_response_timeout();

            let pkts = session.transmission.drain().unwrap();
            let mut iter = pkts.into_iter();

            let (_pkt, address) = iter.next().unwrap();
            assert_eq!(address, Some(NEW_ADDRESS));
            assert!(iter.next().is_none());
        }

        // verify state is still pending but path response timeout is 2x
        let challenge = match &session.path_validation_state {
            PathValidationState::Inactive => panic!("invalid state"),
            PathValidationState::Active {
                address,
                timeout,
                challenge,
                ..
            } => {
                assert_eq!(*address, NEW_ADDRESS);
                assert_ne!(*address, session.address);
                assert_eq!(*timeout, initial_timeout * 2);

                challenge.clone()
            }
        };

        // receive valid path response
        session.handle_path_response(challenge);
        assert!(std::matches!(
            session.path_validation_state,
            PathValidationState::Inactive
        ));
        assert!(!session.transmission.is_throttled());
        assert_eq!(session.address, NEW_ADDRESS);
    }

    #[tokio::test]
    async fn path_validation_timeouts() {
        let mut session = make_session().await;
        assert!(std::matches!(
            session.path_validation_state,
            PathValidationState::Inactive
        ));
        assert_eq!(session.address, OLD_ADDRESS);

        session.validate_pkt_address(NEW_ADDRESS, 128);
        assert!(session.transmission.is_throttled());

        // verify that a packet to the new address is queued
        {
            let pkts = session.transmission.drain().unwrap();
            let mut iter = pkts.into_iter();

            let (_pkt, address) = iter.next().unwrap();
            assert_eq!(address, Some(NEW_ADDRESS));
            assert!(iter.next().is_none());
        }

        match &mut session.path_validation_state {
            PathValidationState::Inactive => panic!("invalid state"),
            PathValidationState::Active { address, .. } => {
                assert_eq!(*address, NEW_ADDRESS);
                assert_ne!(*address, session.address);
            }
        }

        tokio::time::sleep(PATH_VALIDATION_TIMEOUT + Duration::from_millis(100)).await;

        // handle path response timeout and verify that state is reset back to normal
        session.handle_path_response_timeout();
        assert!(std::matches!(
            session.path_validation_state,
            PathValidationState::Inactive
        ));
        assert_eq!(session.address, OLD_ADDRESS);
        assert!(!session.transmission.is_throttled());
    }

    #[tokio::test]
    async fn path_validation_fails() {
        let mut session = make_session().await;
        assert!(std::matches!(
            session.path_validation_state,
            PathValidationState::Inactive
        ));
        assert_eq!(session.address, OLD_ADDRESS);

        // packet with new address received
        session.validate_pkt_address(NEW_ADDRESS, 128);
        assert!(session.transmission.is_throttled());

        // verify that a packet to the new address is queued
        {
            let pkts = session.transmission.drain().unwrap();
            let mut iter = pkts.into_iter();

            let (_pkt, address) = iter.next().unwrap();
            assert_eq!(address, Some(NEW_ADDRESS));
            assert!(iter.next().is_none());
        }

        match &session.path_validation_state {
            PathValidationState::Inactive => panic!("invalid state"),
            PathValidationState::Active { address, .. } => {
                assert_eq!(*address, NEW_ADDRESS);
                assert_ne!(*address, session.address);
            }
        }

        // receive invalid challenge
        session.handle_path_response(vec![1, 3, 3, 7]);
        assert!(std::matches!(
            session.path_validation_state,
            PathValidationState::Inactive
        ));
        assert!(!session.transmission.is_throttled());
        assert_eq!(session.address, OLD_ADDRESS);
    }

    #[tokio::test]
    async fn duplicate_pkt_from_new_address_ignored() {
        let mut session = make_session().await;
        assert!(std::matches!(
            session.path_validation_state,
            PathValidationState::Inactive
        ));
        assert_eq!(session.address, OLD_ADDRESS);

        // add packet to transmission manager
        session.transmission.register_remote_pkt(128);

        // receive the same packet from new address
        session.validate_pkt_address(NEW_ADDRESS, 128);
        assert!(!session.transmission.is_throttled());
        assert!(std::matches!(
            session.path_validation_state,
            PathValidationState::Inactive
        ));
        assert_eq!(session.address, OLD_ADDRESS);
        assert!(session.transmission.drain().is_none());
    }

    #[tokio::test]
    async fn new_pkt_from_new_address_does_not_reset_state() {
        let mut session = make_session().await;
        assert!(std::matches!(
            session.path_validation_state,
            PathValidationState::Inactive
        ));
        assert_eq!(session.address, OLD_ADDRESS);

        // packet with new address received
        session.validate_pkt_address(NEW_ADDRESS, 128);
        assert!(session.transmission.is_throttled());

        // verify that a packet to the new address is queued
        {
            let pkts = session.transmission.drain().unwrap();
            let mut iter = pkts.into_iter();

            let (_pkt, address) = iter.next().unwrap();
            assert_eq!(address, Some(NEW_ADDRESS));
            assert!(iter.next().is_none());
        }

        let old_challenge = match &session.path_validation_state {
            PathValidationState::Inactive => panic!("invalid state"),
            PathValidationState::Active {
                address, challenge, ..
            } => {
                assert_eq!(*address, NEW_ADDRESS);
                assert_ne!(*address, session.address);

                challenge.clone()
            }
        };

        // packet from new address with new ptk number is received
        session.validate_pkt_address(NEW_ADDRESS, 129);
        assert!(session.transmission.is_throttled());

        // verify that the old path validation is kept unmodified
        match &session.path_validation_state {
            PathValidationState::Inactive => panic!("invalid state"),
            PathValidationState::Active {
                address, challenge, ..
            } => {
                assert_eq!(*address, NEW_ADDRESS);
                assert_ne!(*address, session.address);

                assert_eq!(challenge, &old_challenge);
            }
        }

        // verify that new packets are not sent
        assert!(session.transmission.drain().is_none());
    }

    #[tokio::test]
    async fn valid_pkt_from_old_address_cancels_validation() {
        let mut session = make_session().await;
        assert!(std::matches!(
            session.path_validation_state,
            PathValidationState::Inactive
        ));
        assert_eq!(session.address, OLD_ADDRESS);

        // packet with new address received
        session.validate_pkt_address(NEW_ADDRESS, 128);
        assert!(session.transmission.is_throttled());

        // verify that a packet to the new address is queued
        {
            let pkts = session.transmission.drain().unwrap();
            let mut iter = pkts.into_iter();

            let (_pkt, address) = iter.next().unwrap();
            assert_eq!(address, Some(NEW_ADDRESS));
            assert!(iter.next().is_none());
        }

        match &session.path_validation_state {
            PathValidationState::Inactive => panic!("invalid state"),
            PathValidationState::Active { address, .. } => {
                assert_eq!(*address, NEW_ADDRESS);
                assert_ne!(*address, session.address);
            }
        }

        // receive new packet from old address
        session.validate_pkt_address(OLD_ADDRESS, 129);
        assert!(!session.transmission.is_throttled());
        assert!(std::matches!(
            session.path_validation_state,
            PathValidationState::Inactive
        ));
        assert_eq!(session.address, OLD_ADDRESS);
    }

    #[tokio::test]
    async fn duplicate_pkt_from_old_address_does_not_cancel_path_validation() {
        let mut session = make_session().await;
        assert!(std::matches!(
            session.path_validation_state,
            PathValidationState::Inactive
        ));
        assert_eq!(session.address, OLD_ADDRESS);

        // packet with new address received
        session.validate_pkt_address(NEW_ADDRESS, 128);
        assert!(session.transmission.is_throttled());

        // verify that a packet to the new address is queued
        {
            let pkts = session.transmission.drain().unwrap();
            let mut iter = pkts.into_iter();

            let (_pkt, address) = iter.next().unwrap();
            assert_eq!(address, Some(NEW_ADDRESS));
            assert!(iter.next().is_none());
        }

        match &session.path_validation_state {
            PathValidationState::Inactive => panic!("invalid state"),
            PathValidationState::Active { address, .. } => {
                assert_eq!(*address, NEW_ADDRESS);
                assert_ne!(*address, session.address);
            }
        }

        // receive duplicate packet from old address
        session.transmission.register_remote_pkt(128);
        session.validate_pkt_address(OLD_ADDRESS, 128);

        match &session.path_validation_state {
            PathValidationState::Inactive => panic!("invalid state"),
            PathValidationState::Active { address, .. } => {
                assert_eq!(*address, NEW_ADDRESS);
                assert_ne!(*address, session.address);
            }
        }
        assert!(session.transmission.is_throttled());
        assert_eq!(session.address, OLD_ADDRESS);
    }

    #[tokio::test]
    async fn new_validation_rejected_when_validation_already_in_progress() {
        let mut session = make_session().await;
        assert!(std::matches!(
            session.path_validation_state,
            PathValidationState::Inactive
        ));
        assert_eq!(session.address, OLD_ADDRESS);

        // packet with new address received
        session.validate_pkt_address(NEW_ADDRESS, 128);
        assert!(session.transmission.is_throttled());

        // verify that a packet to the new address is queued
        {
            let pkts = session.transmission.drain().unwrap();
            let mut iter = pkts.into_iter();

            let (_pkt, address) = iter.next().unwrap();
            assert_eq!(address, Some(NEW_ADDRESS));
            assert!(iter.next().is_none());
        }

        match &session.path_validation_state {
            PathValidationState::Inactive => panic!("invalid state"),
            PathValidationState::Active { address, .. } => {
                assert_eq!(*address, NEW_ADDRESS);
                assert_ne!(*address, session.address);
            }
        }

        // receive new packet from yet another address
        session.validate_pkt_address("127.0.0.1:9999".parse().unwrap(), 129);

        match &session.path_validation_state {
            PathValidationState::Inactive => panic!("invalid state"),
            PathValidationState::Active { address, .. } => {
                assert_eq!(*address, NEW_ADDRESS);
                assert_ne!(*address, session.address);
            }
        }
        assert!(session.transmission.is_throttled());
        assert_eq!(session.address, OLD_ADDRESS);
        assert!(session.transmission.drain().is_none());
    }

    #[tokio::test]
    async fn address_change_resets_rto_and_rtt() {
        let mut session = make_session().await;
        assert!(std::matches!(
            session.path_validation_state,
            PathValidationState::Inactive
        ));
        assert_eq!(session.address, OLD_ADDRESS);

        let new_address_different_ip = "192.168.0.55:7575".parse().unwrap();
        let initial_rto = session.transmission.round_trip_time();
        assert_eq!(initial_rto, session.transmission.round_trip_time());

        // add rtt samples
        for i in 1..=3 {
            session.transmission.add_rtt_sample(Duration::from_millis(i * 10));
        }
        assert_ne!(initial_rto, session.transmission.round_trip_time());

        // packet with new address received (different ip address)
        session.validate_pkt_address(new_address_different_ip, 128);
        assert!(session.transmission.is_throttled());

        // verify that a packet to the new address is queued
        {
            let pkts = session.transmission.drain().unwrap();
            let mut iter = pkts.into_iter();

            let (_pkt, address) = iter.next().unwrap();
            assert_eq!(address, Some(new_address_different_ip));
            assert!(iter.next().is_none());
        }

        let challenge = match &session.path_validation_state {
            PathValidationState::Inactive => panic!("invalid state"),
            PathValidationState::Active {
                address, challenge, ..
            } => {
                assert_eq!(*address, new_address_different_ip);
                assert_ne!(*address, session.address);

                challenge.clone()
            }
        };

        session.handle_path_response(challenge);
        assert!(std::matches!(
            session.path_validation_state,
            PathValidationState::Inactive
        ));
        assert!(!session.transmission.is_throttled());
        assert_eq!(session.address, new_address_different_ip);

        // verify rtt is back to initial value
        assert_eq!(session.transmission.round_trip_time(), initial_rto);
    }
}
