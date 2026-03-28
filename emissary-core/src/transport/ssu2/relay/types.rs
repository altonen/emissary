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
    primitives::{RouterId, RouterInfo},
    runtime::Runtime,
    transport::ssu2::duplicate::DuplicateFilter,
};

use futures::{FutureExt, Stream};
use thingbuf::mpsc::{channel, Receiver, Sender};

use alloc::{boxed::Box, vec::Vec};
use core::{
    net::SocketAddr,
    pin::Pin,
    task::{Context, Poll},
};

/// Events emitted by active SSU2 session and send to `RelayManager`.
#[derive(Clone, Default)]
pub enum RelayEvent {
    /// Handle relay request received to an active session (from Alice to Bob).
    RelayRequest {
        /// Router ID of Alice.
        alice_router_id: RouterId,

        /// Random nonce
        nonce: u32,

        /// Relay tag from Charlie's router info.
        relay_tag: u32,

        /// Alice's socket address.
        address: SocketAddr,

        /// Message, i.e., the part of `RelayRequest` covered by `signature`.
        message: Vec<u8>,

        /// Signature for `message`.
        signature: Vec<u8>,

        /// TX channel for sending a command back to the active session.
        tx: Sender<RelayCommand>,
    },

    /// Handle relay intro received to an active session (from Bob to Charlie).
    RelayIntro {
        /// Alice's router ID.
        alice_router_id: RouterId,

        /// Bob's router ID.
        bob_router_id: RouterId,

        /// Alice's router info, if received in a router info block.
        alice_router_info: Option<Box<RouterInfo>>,

        /// Random nonce.
        nonce: u32,

        /// Relay tag.
        relay_tag: u32,

        /// Alice's socket address.
        address: SocketAddr,

        /// Message, i.e., the part of `RelayIntro` covered by `signature`.
        message: Vec<u8>,

        /// Signature for `message`.
        signature: Vec<u8>,

        /// TX channel for sending a command back to the active session.
        tx: Sender<RelayCommand>,
    },

    /// Handle relay response received to an active session (from Bob or Charlie).
    RelayResponse {
        /// Random nonce.
        nonce: u32,

        /// Charlie's socket address, if accepted.
        address: Option<SocketAddr>,

        /// Token used in `SessionRequest` by Alice, if accepted.
        token: Option<u64>,

        /// Rejection.
        ///
        /// `None` if accepted.
        rejection: Option<RejectionReason>,

        /// Message, i.e., the part of `RelayResponse` covered by `signature`.
        message: Vec<u8>,

        /// Signature for `message`.
        signature: Vec<u8>,
    },

    /// Dummy event.
    #[default]
    Dummy,
}

/// Commands sent by `RelayManager` to active SSU2 sessions.
#[derive(Clone, Default)]
pub enum RelayCommand {
    /// Send relay request to Bob.
    RelayRequest {
        /// Random nonce.
        nonce: u32,

        /// Message.
        message: Vec<u8>,

        /// Signature for `message`.
        signature: Vec<u8>,
    },

    /// Send relay response to Alice/Bob.
    RelayResponse {
        /// Random nonce.
        nonce: u32,

        /// Rejection reason.
        ///
        /// `None` if accepted.
        rejection: Option<RejectionReason>,

        /// Message.
        message: Vec<u8>,

        /// Signature for `message`.
        signature: Vec<u8>,

        /// Token for `SessionRequest`.
        ///
        /// `None` if rejected by Bob or Charlie.
        token: Option<u64>,
    },

    /// Send relay intro to Charlie.
    RelayIntro {
        /// Alice's router ID.
        router_id: Vec<u8>,

        /// Alice's serialized router info.
        router_info: Vec<u8>,

        /// Message received from Alice.
        message: Vec<u8>,

        /// Signature for `message`.
        signature: Vec<u8>,
    },

    /// Dummy event.
    #[default]
    Dummy,
}

/// Relay handle given to active SSU2 sessions, allowing them to interact with `RelayManager`.
pub struct RelayHandle<R: Runtime> {
    /// RX channel for receiving `PeerTestCommand`s from `RelayManager`.
    cmd_rx: Receiver<RelayCommand>,

    /// TX channel given to `RelayManager`.
    cmd_tx: Sender<RelayCommand>,

    /// Duplicate filter.
    duplicate_filter: DuplicateFilter<R>,

    /// TX channel for sending events to `RelayManager`.
    event_tx: Sender<RelayEvent>,
}

impl<R: Runtime> RelayHandle<R> {
    /// Create new `RelayHandle`.
    pub fn new(event_tx: Sender<RelayEvent>) -> Self {
        let (cmd_tx, cmd_rx) = channel(32);

        Self {
            cmd_rx,
            cmd_tx,
            duplicate_filter: DuplicateFilter::new(),
            event_tx,
        }
    }

    /// Get clone of command channel.
    pub fn cmd_tx(&self) -> Sender<RelayCommand> {
        self.cmd_tx.clone()
    }

    /// Send relay request to `RelayManager` for processing.
    pub fn handle_relay_request(
        &self,
        alice_router_id: RouterId,
        nonce: u32,
        relay_tag: u32,
        address: SocketAddr,
        message: Vec<u8>,
        signature: Vec<u8>,
    ) {
        let _ = self.event_tx.try_send(RelayEvent::RelayRequest {
            alice_router_id,
            nonce,
            relay_tag,
            address,
            message,
            signature,
            tx: self.cmd_tx.clone(),
        });
    }

    /// Send relay intro to `RelayManager` for processing.
    pub fn handle_relay_intro(
        &self,
        alice_router_id: RouterId,
        bob_router_id: RouterId,
        alice_router_info: Option<Box<RouterInfo>>,
        nonce: u32,
        relay_tag: u32,
        address: SocketAddr,
        message: Vec<u8>,
        signature: Vec<u8>,
    ) {
        let _ = self.event_tx.try_send(RelayEvent::RelayIntro {
            alice_router_id,
            bob_router_id,
            alice_router_info,
            nonce,
            relay_tag,
            address,
            message,
            signature,
            tx: self.cmd_tx.clone(),
        });
    }

    /// Send relay response to `RelayManager` for processing.
    pub fn handle_relay_response(
        &self,
        nonce: u32,
        address: Option<SocketAddr>,
        token: Option<u64>,
        rejection: Option<RejectionReason>,
        message: Vec<u8>,
        signature: Vec<u8>,
    ) {
        let _ = self.event_tx.try_send(RelayEvent::RelayResponse {
            nonce,
            address,
            token,
            rejection,
            message,
            signature,
        });
    }
}

impl<R: Runtime> Stream for RelayHandle<R> {
    type Item = RelayCommand;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if let Poll::Ready(event) = self.cmd_rx.poll_recv(cx) {
            return Poll::Ready(event);
        }

        let _ = self.duplicate_filter.poll_unpin(cx);

        Poll::Pending
    }
}

/// Rejection reasons for Bob.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum BobRejectionReason {
    /// Unspecified.
    Unspecified,

    /// Charlie is banned.
    Banned,

    /// Limit exceeded.
    LimitExceeded,

    /// Signature failure.
    SignatureFailure,

    /// Relay tag not found.
    RelayTagNotFound,

    /// Alice's router info not found.
    AliceNotFound,
}

/// Rejection reasons for Charlie.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum CharlieRejectionReason {
    /// Unspecified.
    Unspecified,

    /// Unsupported address.
    UnsupportedAddress,

    /// Limit exceeded.
    LimitExceeded,

    /// Signature failure.
    SignatureFailure,

    /// Alice is already connected.
    AlreadyConnected,

    /// Alice is banned.
    Banned,

    /// Alice's router info not found.
    AliceNotFound,
}

/// Rejection reason.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum RejectionReason {
    Bob(BobRejectionReason),
    Charlie(CharlieRejectionReason),
    Unspecified,
}

impl From<u8> for RejectionReason {
    fn from(value: u8) -> Self {
        match value {
            0 => unreachable!(),
            1 => Self::Bob(BobRejectionReason::Unspecified),
            2 => Self::Bob(BobRejectionReason::Banned),
            3 => Self::Bob(BobRejectionReason::LimitExceeded),
            4 => Self::Bob(BobRejectionReason::SignatureFailure),
            5 => Self::Bob(BobRejectionReason::RelayTagNotFound),
            6 => Self::Bob(BobRejectionReason::AliceNotFound),
            7..=63 => Self::Bob(BobRejectionReason::Unspecified),
            64 => Self::Charlie(CharlieRejectionReason::Unspecified),
            65 => Self::Charlie(CharlieRejectionReason::UnsupportedAddress),
            66 => Self::Charlie(CharlieRejectionReason::LimitExceeded),
            67 => Self::Charlie(CharlieRejectionReason::SignatureFailure),
            68 => Self::Charlie(CharlieRejectionReason::AlreadyConnected),
            69 => Self::Charlie(CharlieRejectionReason::Banned),
            70 => Self::Charlie(CharlieRejectionReason::AliceNotFound),
            71..=127 => Self::Charlie(CharlieRejectionReason::Unspecified),
            _ => Self::Unspecified,
        }
    }
}

impl RejectionReason {
    pub fn as_u8(&self) -> u8 {
        match self {
            Self::Bob(reason) => match reason {
                BobRejectionReason::Unspecified => 1,
                BobRejectionReason::Banned => 2,
                BobRejectionReason::LimitExceeded => 3,
                BobRejectionReason::SignatureFailure => 4,
                BobRejectionReason::RelayTagNotFound => 5,
                BobRejectionReason::AliceNotFound => 6,
            },
            Self::Charlie(reason) => match reason {
                CharlieRejectionReason::Unspecified => 64,
                CharlieRejectionReason::UnsupportedAddress => 65,
                CharlieRejectionReason::LimitExceeded => 66,
                CharlieRejectionReason::SignatureFailure => 67,
                CharlieRejectionReason::AlreadyConnected => 68,
                CharlieRejectionReason::Banned => 69,
                CharlieRejectionReason::AliceNotFound => 70,
            },
            Self::Unspecified => 128,
        }
    }
}

impl From<RejectionReason> for &'static str {
    fn from(value: RejectionReason) -> Self {
        match value {
            RejectionReason::Bob(reason) => match reason {
                BobRejectionReason::Unspecified => "bob-unspecified",
                BobRejectionReason::Banned => "bob-banned",
                BobRejectionReason::LimitExceeded => "bob-limit-exceeded",
                BobRejectionReason::SignatureFailure => "bob-signature-failure",
                BobRejectionReason::RelayTagNotFound => "bob-relay-tag-not-found",
                BobRejectionReason::AliceNotFound => "bob-alice-not-found",
            },
            RejectionReason::Charlie(reason) => match reason {
                CharlieRejectionReason::Unspecified => "charlie-unspecified",
                CharlieRejectionReason::UnsupportedAddress => "charlie-unsupported-address",
                CharlieRejectionReason::LimitExceeded => "charlie-limit-exceeded",
                CharlieRejectionReason::SignatureFailure => "charlie-signature-failure",
                CharlieRejectionReason::AlreadyConnected => "charlie-already-connected",
                CharlieRejectionReason::Banned => "charlie-banned",
                CharlieRejectionReason::AliceNotFound => "charlie-alice-not-found",
            },
            RejectionReason::Unspecified => "unspecified",
        }
    }
}

/// Relay tag request result.
///
/// Used by `InboundSsu2Session` to inform `RelayManager` whether the inbound router requested
/// a relay tag.
#[derive(Debug, Copy, Clone)]
pub enum RelayTagRequested {
    /// Remote router requested relay from us.
    Yes(u32),

    /// Remote router did not requested relay from us.
    No(u32),
}

impl RelayTagRequested {
    /// Get relay tay.
    pub fn tag(&self) -> u32 {
        match self {
            Self::Yes(tag) => *tag,
            Self::No(tag) => *tag,
        }
    }
}
