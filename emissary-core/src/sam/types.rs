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
    crypto::{StaticPrivateKey, StaticPublicKey},
    primitives::{Destination as Dest, DestinationId, Mapping},
    protocol::Protocol,
    runtime::Runtime,
    sam::{parser::SessionKind, socket::SamSocket},
};

use hashbrown::HashMap;

use alloc::{
    boxed::Box,
    string::{String, ToString},
    sync::Arc,
    vec,
    vec::Vec,
};
use core::fmt;

/// Recycling strategy for [`SamSessionCommand`].
#[derive(Default, Clone)]
pub(super) struct SamSessionCommandRecycle(());

impl<R: Runtime> thingbuf::Recycle<SamSessionCommand<R>> for SamSessionCommandRecycle {
    fn new_element(&self) -> SamSessionCommand<R> {
        SamSessionCommand::Dummy
    }

    fn recycle(&self, element: &mut SamSessionCommand<R>) {
        *element = SamSessionCommand::Dummy;
    }
}

/// SAMv3 session commands.
#[derive(Default)]
pub enum SamSessionCommand<R: Runtime> {
    /// Open virtual stream to `destination` over this connection.
    Connect {
        /// SAMv3 socket associated with the outbound stream.
        socket: Box<SamSocket<R>>,

        /// Destination ID.
        destination_id: DestinationId,

        /// Options.
        options: HashMap<String, String>,

        /// Session ID.
        session_id: Arc<str>,
    },

    /// Accept inbond virtual stream over this connection.
    Accept {
        /// SAMv3 socket associated with the inbound stream.
        socket: Box<SamSocket<R>>,

        /// Options.
        options: HashMap<String, String>,

        /// Session ID.
        session_id: Arc<str>,
    },

    /// Forward incoming virtual streams to a TCP listener listening to `port`.
    Forward {
        /// SAMv3 socket associated with forwarding.
        socket: Box<SamSocket<R>>,

        /// Port which the TCP listener is listening.
        port: u16,

        /// Options.
        options: HashMap<String, String>,

        /// Session ID.
        session_id: Arc<str>,
    },

    /// Send repliable datagram to remote destination.
    SendDatagram {
        /// Destination of the receiver.
        destination: Box<Dest>,

        /// Datagram.
        datagram: Vec<u8>,

        /// Session ID.
        session_id: Arc<str>,

        /// Options.
        options: Option<Mapping>,
    },

    /// Dummy event, never constructed.
    #[default]
    Dummy,
}

/// State of a pending outbound session.
pub enum PendingSessionState<R: Runtime> {
    /// Awaiting lease set query result.
    AwaitingLeaseSet {
        /// SAMv3 client socket.
        socket: Box<SamSocket<R>>,

        /// Stream options.
        options: HashMap<String, String>,
    },

    /// Awaiting session to be created
    AwaitingSession {
        /// Stream ID assigned by [`StreamManager`].
        stream_id: u32,
    },
}

impl<R: Runtime> fmt::Debug for PendingSessionState<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AwaitingLeaseSet { .. } =>
                f.debug_struct("PendingSessionState::AwaitingLeaseSet").finish_non_exhaustive(),
            Self::AwaitingSession { stream_id } => f
                .debug_struct("PendingSessionState::AwaitingSession")
                .field("stream_id", &stream_id)
                .finish(),
        }
    }
}

/// Pending sessions.
///
/// Session is considered pending if it's lease set is being queried.
///
/// Streams are also considered pending if one or more `SYN`s have been sent but no response
/// has been received yet.
#[derive(Default)]
pub struct PendingSession<R: Runtime> {
    /// Pending streams.
    ///
    /// Contains one or more pending streams for the remote destination.
    pub streams: Vec<PendingSessionState<R>>,

    /// Pending datagrams.
    ///
    /// Only set if there are pending datagrams for the remote destination.
    pub datagrams: Option<(Dest, Vec<(Protocol, Vec<u8>, Option<Mapping>)>)>,
}

impl<R: Runtime> PendingSession<R> {
    /// Create new [`PendingSession`].
    pub fn new() -> Self {
        Self {
            streams: Vec::new(),
            datagrams: None,
        }
    }
}

/// Session kind for [`SamSession`].
pub enum SamSessionKind {
    /// [`SamSession`] is configured to be a primary sessions, supporting multiple sub-sessions.
    Primary {
        /// Registered sub-sessions.
        sub_sessions: HashMap<Arc<str>, SessionKind>,
    },

    /// [`SamSession`] is configured to be a stream session.
    Stream,

    /// [`SamSession`] is configured to be a datagram session.
    Datagram {
        /// Datagram kind.
        kind: SessionKind,
    },
}

impl SamSessionKind {
    /// Does [`SamSession`] support `STREAM CONNECT`/`STREAM ACCEPT`/`STREAM FORWARD`
    ///
    /// `session_id` is the ID that the client gave when it sent the command and it's either the ID
    /// that was given in `SESSION CREATE` or a ID of the sub-session given in `SESSION ADD`.
    pub fn supports_streams(&self, session_id: &Arc<str>) -> bool {
        match self {
            Self::Stream => true,
            Self::Datagram { .. } => false,
            Self::Primary { sub_sessions } => sub_sessions
                .get(session_id)
                .is_some_and(|kind| core::matches!(kind, SessionKind::Stream)),
        }
    }

    /// Does [`SamSession`] support datagrams.
    ///
    /// `session_id` is the ID that the client gave when it sent the command and it's either the ID
    /// that was given in `SESSION CREATE` or a ID of the sub-session given in `SESSION ADD`.
    pub fn supports_datagrams(&self, session_id: &Arc<str>) -> bool {
        match self {
            Self::Stream => false,
            Self::Datagram { .. } => true,
            Self::Primary { sub_sessions } => sub_sessions.get(session_id).is_some_and(|kind| {
                core::matches!(
                    kind,
                    SessionKind::Datagram | SessionKind::Anonymous | SessionKind::Datagram2,
                )
            }),
        }
    }

    /// Convert [`SamSessionKind`] into [`Protocol`].
    ///
    /// Panics if [`SamSessionKind`] is `Primary` and `session_id` doesn't exist.
    pub fn as_protocol(&self, session_id: &Arc<str>) -> Protocol {
        match self {
            Self::Stream => Protocol::Streaming,
            Self::Datagram { kind } => match kind {
                SessionKind::Datagram => Protocol::Datagram,
                SessionKind::Anonymous => Protocol::Anonymous,
                SessionKind::Datagram2 => Protocol::Datagram2,
                _ => unreachable!(),
            },
            Self::Primary { sub_sessions } => match sub_sessions.get(session_id).expect("to exist")
            {
                SessionKind::Stream => Protocol::Streaming,
                SessionKind::Datagram => Protocol::Datagram,
                SessionKind::Anonymous => Protocol::Anonymous,
                SessionKind::Datagram2 => Protocol::Datagram2,
                _ => unreachable!(),
            },
        }
    }
}

impl fmt::Debug for SamSessionKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stream => f.debug_struct("SamSessionKind::Stream").finish(),
            Self::Datagram { .. } =>
                f.debug_struct("SamSessionKind::Datagram").finish_non_exhaustive(),
            Self::Primary { .. } =>
                f.debug_struct("SamSessionKind::Primary").finish_non_exhaustive(),
        }
    }
}

/// Public key context.
pub struct PublicKeyContext {
    /// Private key.
    private_key: StaticPrivateKey,

    /// Public key for the primary encryption type.
    ///
    /// Defaults to ML-KEM-768-x25519.
    primary: StaticPublicKey,

    /// Public key for the secondary encryption type.
    ///
    /// `None` if the user specified a single encryption type.
    secondary: Option<StaticPublicKey>,
}

impl PublicKeyContext {
    /// Create new `PublicKeyContext` from `options`.
    pub fn new<R: Runtime>(options: &HashMap<String, String>) -> Self {
        // `SamParser` has ensured `i2cp.leaseSetEncType` exists and that they're valid
        // and supported types and that at least one key exists
        let encryption_types = options
            .get(&"i2cp.leaseSetEncType".to_string())
            .expect("to exist")
            .split(",")
            .map(|value| value.parse::<usize>().expect("valid encryption type"))
            .collect::<Vec<_>>();

        let private_key = match encryption_types[0] {
            4 => StaticPrivateKey::random(R::rng()),
            5 => StaticPrivateKey::random_ml_kem_512(R::rng()),
            6 => StaticPrivateKey::random_ml_kem_768(R::rng()),
            7 => StaticPrivateKey::random_ml_kem_1024(R::rng()),
            _ => unreachable!(),
        };
        let primary = private_key.public();

        let secondary = encryption_types.get(1).map(|encryption_type| match encryption_type {
            4 => StaticPublicKey::try_from_bytes(primary.as_ref()).expect("to succeed"),
            5 => StaticPublicKey::try_from_bytes_ml_kem_512(primary.as_ref()).expect("to succeed"),
            6 => StaticPublicKey::try_from_bytes_ml_kem_768(primary.as_ref()).expect("to succeed"),
            7 => StaticPublicKey::try_from_bytes_ml_kem_1024(primary.as_ref()).expect("to succeed"),
            _ => unreachable!(),
        });

        Self {
            private_key,
            primary,
            secondary,
        }
    }

    /// Get private key.
    pub fn private_key(&self) -> StaticPrivateKey {
        self.private_key.clone()
    }

    /// Get public keys.
    pub fn public_keys(&self) -> Vec<StaticPublicKey> {
        if let Some(secondary) = &self.secondary {
            return vec![self.primary.clone(), secondary.clone()];
        }

        vec![self.primary.clone()]
    }
}
