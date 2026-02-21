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

use crate::primitives::RouterId;

use futures::Stream;
use thingbuf::mpsc::{channel, Receiver, Sender};

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

        /// Timestamp as seconds since UNIX epoch.
        timestamp: u32,

        /// Alice's socket address.
        address: SocketAddr,

        /// Message, i.e., the part of `RelayRequest` covered by `signature`.
        message: Vec<u8>,

        /// Signature for `message`.
        signature: Vec<u8>,

        /// TX channel for sending a command back to the active session.
        tx: Sender<RelayCommand>,
    },

    /// Dummy event.
    #[default]
    Dummy,
}

/// Commands sent by `RelayManager` to active SSU2 sessions.
#[derive(Clone, Default)]
pub enum RelayCommand {
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
    },

    /// Send relay intro to Charlie.
    RelayIntro {},

    /// Dummy event.
    #[default]
    Dummy,
}

/// Relay handle given to active SSU2 sessions, allowing them to interact with `RelayManager`.
pub struct RelayHandle {
    /// RX channel for receiving `PeerTestCommand`s from `RelayManager`.
    cmd_rx: Receiver<RelayCommand>,

    /// TX channel given to `RelayManager`.
    cmd_tx: Sender<RelayCommand>,

    /// TX channel for sending events to `RelayManager`.
    event_tx: Sender<RelayEvent>,
}

impl RelayHandle {
    /// Create new `RelayHandle`.
    pub fn new(event_tx: Sender<RelayEvent>) -> Self {
        let (cmd_tx, cmd_rx) = channel(32);

        Self {
            event_tx,
            cmd_rx,
            cmd_tx,
        }
    }

    /// Send relay request to `RelayManager` for processing.
    pub fn handle_relay_request(
        &self,
        alice_router_id: RouterId,
        nonce: u32,
        relay_tag: u32,
        timestamp: u32,
        address: SocketAddr,
        message: Vec<u8>,
        signature: Vec<u8>,
    ) {
        let _ = self.event_tx.try_send(RelayEvent::RelayRequest {
            alice_router_id,
            nonce,
            relay_tag,
            timestamp,
            address,
            message,
            signature,
            tx: self.cmd_tx.clone(),
        });
    }
}

impl Stream for RelayHandle {
    type Item = RelayCommand;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Poll::Ready(futures::ready!(self.cmd_rx.poll_recv(cx)))
    }
}

/// Rejection reason.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum RejectionReason {
    /// Unspecified.
    Unspecified,

    /// Limit exceeded.
    LimitExceeded,

    /// Signature failure.
    SignatureFailure,

    /// Relay tag not found.
    RelayTagNotFound,

    /// Alice's router info not found.
    AliceNotFound,

    /// Unsupported address.
    UnsupportedAddress,

    /// Alice is already connected.
    AlreadyConnected,

    /// Alice/Charlie is banned.
    Banned,
}

impl From<u8> for RejectionReason {
    fn from(value: u8) -> Self {
        match value {
            0 => unreachable!(),
            1 => Self::Unspecified,
            2 => Self::Banned,
            3 => Self::LimitExceeded,
            4 => Self::SignatureFailure,
            5 => Self::RelayTagNotFound,
            6 => Self::AliceNotFound,
            7..=64 => Self::Unspecified,
            64 => Self::Unspecified,
            65 => Self::UnsupportedAddress,
            66 => Self::LimitExceeded,
            67 => Self::SignatureFailure,
            68 => Self::AlreadyConnected,
            69 => Self::Banned,
            70 => Self::AliceNotFound,
            71..=127 => Self::Unspecified,
            128 => Self::Unspecified,
            129..=255 => Self::Unspecified,
        }
    }
}

impl RejectionReason {
    /// Convert `RejectionReason` to a status code from Bob.
    pub fn as_bob(self) -> u8 {
        match self {
            Self::Unspecified => 1,
            Self::Banned => 2,
            Self::LimitExceeded => 3,
            Self::SignatureFailure => 4,
            Self::RelayTagNotFound => 5,
            Self::AliceNotFound => 6,
            _ => 1,
        }
    }

    /// Convert `RejectionReason` to a status code from Charlie.
    pub fn as_charlie(self) -> u8 {
        match self {
            Self::Unspecified => 64,
            Self::UnsupportedAddress => 65,
            Self::LimitExceeded => 66,
            Self::SignatureFailure => 67,
            Self::AlreadyConnected => 68,
            Self::Banned => 69,
            Self::AliceNotFound => 70,
            _ => 128,
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
