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
    error::DialError,
    primitives::{RouterId, RouterInfo},
};

use futures::Stream;

use core::{net::SocketAddr, time::Duration};

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

impl From<TerminationReason> for &'static str {
    fn from(value: TerminationReason) -> Self {
        match value {
            TerminationReason::Unspecified => "unspecified",
            TerminationReason::TerminationReceived => "termination-received",
            TerminationReason::IdleTimeout => "idle-timeout",
            TerminationReason::IoError => "io-error",
            TerminationReason::RouterShutdown => "router-shutdown",
            TerminationReason::AeadFailure => "aead-failure",
            TerminationReason::IncompatibleOptions => "incompatible-options",
            TerminationReason::IncompatibleSignatureKind => "incompatible-signature",
            TerminationReason::ClockSkew => "clock-skew",
            TerminationReason::PaddinViolation => "paddin-violation",
            TerminationReason::PayloadFormatError => "payload-format-error",
            TerminationReason::AeadFramingError => "aead-framing-error",
            TerminationReason::Ntcp2HandshakeError(_) => "ntcp2-handshake-error",
            TerminationReason::Ssu2HandshakeError(_) => "ssu2-handshake-error",
            TerminationReason::IntraFrameReadTimeout => "intra-frame-read-timeout",
            TerminationReason::InvalidRouterInfo => "invalid-router-info",
            TerminationReason::Banned => "banned",
            TerminationReason::Timeout => "timeout",
            TerminationReason::BadToken => "bad-token",
            TerminationReason::ConnectionLimits => "connection-limits",
            TerminationReason::IncompatibleVersion => "incompatible-version",
            TerminationReason::WrongNetId => "wrong-net-id",
            TerminationReason::ReplacedByNewSession => "replaced-by-new-session",
        }
    }
}

/// Firewall status.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum FirewallStatus {
    /// Router's firewall status is unknown.
    #[default]
    Unknown,

    /// SSU2 has detected that the router is firewalled.
    Firewalled,

    /// Firewall is open.
    Ok,

    /// Router is behind a symmetric NAT.
    SymmetricNat,
}

impl From<FirewallStatus> for &'static str {
    fn from(value: FirewallStatus) -> Self {
        match value {
            FirewallStatus::Unknown => "unknown",
            FirewallStatus::Firewalled => "firewalled",
            FirewallStatus::Ok => "ok",
            FirewallStatus::SymmetricNat => "symnat",
        }
    }
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

        /// Reason for dial failure.
        reason: DialError,
    },

    /// SSU2 has learned something about the router's firewall status.
    FirewallStatus {
        /// Firewall status.
        status: FirewallStatus,

        /// Is this a firewall result for IPv4.
        ipv4: bool,
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

        /// Is this an IPv4 introducer.
        ipv4: bool,
    },

    /// Introducer removed.
    IntroducerRemoved {
        /// Router ID of Bob.
        router_id: RouterId,

        /// Was this an IPv4 introducer.
        ipv4: bool,
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
