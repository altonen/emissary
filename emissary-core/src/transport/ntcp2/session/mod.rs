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

//! NTCP2 Noise handshake implementation.
//!
//! https://geti2p.net/spec/ntcp2#overview
//!
//! Implementation refers to `ck` as `chaining_key` and to `h` as `state`.
//1 This implementation also refers to Alice as initiator and to Bob as responder.
//!
//! [`SessionManager::create_session()`] and [`SessionManager::create_session()`]
//! return futures which negotiate connection for initiators and responders, respectively.
//!
//! These two functions do not themselves implement code from the specification in order
//! to prevent mixing that code with I/O code. Handshake implementations for initiator
//! and responder can be found from `initiator.rs` and `responder.rs`.

use crate::{
    crypto::{noise::NoiseContext, sha256::Sha256, siphash::SipHash, StaticPrivateKey},
    error::Ntcp2Error,
    events::EventHandle,
    primitives::{RouterAddress, RouterId, RouterInfo, Str},
    profile::ProfileStorage,
    router::context::RouterContext,
    runtime::{Runtime, TcpStream},
    subsystem::SubsystemEvent,
    transport::{
        ntcp2::session::{initiator::Initiator, responder::Responder},
        Direction,
    },
    util::{is_global, AsyncReadExt, AsyncWriteExt},
};

use bytes::Bytes;
use ml_kem::{array::Array, Encapsulate, EncapsulationKey, MlKem1024, MlKem512, MlKem768};
use thingbuf::mpsc::Sender;

use alloc::{sync::Arc, vec, vec::Vec};
use core::{
    fmt,
    future::Future,
    net::{IpAddr, SocketAddr},
    time::Duration,
};

mod active;
mod initiator;
mod responder;

pub use active::Ntcp2Session;

/// Logging target for the file.
const LOG_TARGET: &str = "emissary::ntcp2::session";

/// Maximum allowed clock skew.
const MAX_CLOCK_SKEW: Duration = Duration::from_secs(60);

mod constants {
    pub mod x25519 {
        /// Noise protocol name.
        pub const PROTOCOL_NAME: &str = "Noise_XKaesobfse+hs2+hs3_25519_ChaChaPoly_SHA256";
    }

    pub mod ml_kem_512 {
        /// Noise protocol name.
        pub const PROTOCOL_NAME: &str =
            "Noise_XKhfsaesobfse+hs2+hs3_25519+MLKEM512_ChaChaPoly_SHA256";
    }

    pub mod ml_kem_768 {
        /// Noise protocol name.
        pub const PROTOCOL_NAME: &str =
            "Noise_XKhfsaesobfse+hs2+hs3_25519+MLKEM768_ChaChaPoly_SHA256";
    }

    pub mod ml_kem_1024 {
        /// Noise protocol name.
        pub const PROTOCOL_NAME: &str =
            "Noise_XKhfsaesobfse+hs2+hs3_25519+MLKEM1024_ChaChaPoly_SHA256";
    }
}

/// ML-KEM context.
#[derive(Clone)]
enum MlKemContext<T> {
    /// ML-KEM-512-x25519
    MlKem512X25519(T),

    /// ML-KEM-768-x25519
    MlKem768X25519(T),

    /// ML-KEM-1024-x25519
    MlKem1024X25519(T),
}

/// Inbound state.
#[derive(Clone)]
struct InboundState {
    /// Protocol state for the selected ML-KEM variant.
    ///
    /// First value is chaining key and second value is inbound state.
    ///
    /// `None` if ML-KEM has not been enabled.
    ml_kem: Option<MlKemContext<([u8; 32], [u8; 32])>>,

    /// Protocol state for x25519.
    ///
    /// First value is chaining key and second value is inbound state.
    x25519: ([u8; 32], [u8; 32]),
}

/// Outbound states for all supportd protocols.
// TODO: not good
struct OutboundState {
    /// Outbound state for ML-KEM-1024.
    ml_kem_1024: ([u8; 32], [u8; 32]),

    /// Outbound state for ML-KEM-512.
    ml_kem_512: ([u8; 32], [u8; 32]),

    /// Outbound state for ML-KEM-768.
    ml_kem_768: ([u8; 32], [u8; 32]),

    /// Outbound state for X25519.
    x25519: ([u8; 32], [u8; 32]),
}

/// Encryption context for outbound connections.
enum EncryptionContext {
    /// X25519.
    X25519(NoiseContext),

    /// ML-KEM-512-x25519
    MlKem512X25519(NoiseContext),

    /// ML-KEM-768-x25519
    MlKem768X25519(NoiseContext),

    /// ML-KEM-1024-x25519
    MlKem1024X25519(NoiseContext),
}

impl TryFrom<InboundState> for EncryptionContext {
    type Error = ();

    fn try_from(value: InboundState) -> Result<Self, Self::Error> {
        match value.ml_kem {
            None => Err(()),
            Some(MlKemContext::MlKem512X25519((chaining_key, inbound_state))) => Ok(
                EncryptionContext::MlKem512X25519(NoiseContext::new(chaining_key, inbound_state)),
            ),
            Some(MlKemContext::MlKem768X25519((chaining_key, inbound_state))) => Ok(
                EncryptionContext::MlKem768X25519(NoiseContext::new(chaining_key, inbound_state)),
            ),
            Some(MlKemContext::MlKem1024X25519((chaining_key, inbound_state))) => Ok(
                EncryptionContext::MlKem1024X25519(NoiseContext::new(chaining_key, inbound_state)),
            ),
        }
    }
}

impl EncryptionContext {
    /// Get mutable reference to inner `NoiseContext`.
    pub fn noise_ctx(&mut self) -> &mut NoiseContext {
        match self {
            Self::X25519(ctx) => ctx,
            Self::MlKem512X25519(ctx) => ctx,
            Self::MlKem768X25519(ctx) => ctx,
            Self::MlKem1024X25519(ctx) => ctx,
        }
    }

    /// Get the size of ML-KEM encapsulation key.
    ///
    /// Returns `0` for x25519.
    pub fn encapsulation_key_size(&self) -> usize {
        match self {
            Self::X25519(_) => 0,
            Self::MlKem512X25519(_) => 800,
            Self::MlKem768X25519(_) => 1184,
            Self::MlKem1024X25519(_) => 1568,
        }
    }

    /// Get ML-KEM ciphertext length.
    ///
    /// Panics if called for x25519.
    pub fn kem_ciphertext_size(&self) -> usize {
        match self {
            Self::X25519(_) => unreachable!(),
            Self::MlKem512X25519(_) => 768,
            Self::MlKem768X25519(_) => 1088,
            Self::MlKem1024X25519(_) => 1568,
        }
    }

    /// Encapsulate and derive KEM ciphertext and shared secret.
    ///
    /// Panics if called for x25519.
    pub fn encapsulate<R: Runtime>(&self, encapsulation_key: &[u8]) -> Option<(Vec<u8>, Vec<u8>)> {
        match self {
            Self::X25519(_) => unreachable!(),
            Self::MlKem512X25519(_) => {
                let key = Array::try_from(encapsulation_key).ok()?;
                let key = EncapsulationKey::<MlKem512>::new(&key).ok()?;
                let (ciphertext, shared_key) = key.encapsulate_with_rng(&mut R::rng());

                Some((ciphertext.to_vec(), shared_key.to_vec()))
            }
            Self::MlKem768X25519(_) => {
                let key = Array::try_from(encapsulation_key).ok()?;
                let key = EncapsulationKey::<MlKem768>::new(&key).ok()?;
                let (ciphertext, shared_key) = key.encapsulate_with_rng(&mut R::rng());

                Some((ciphertext.to_vec(), shared_key.to_vec()))
            }
            Self::MlKem1024X25519(_) => {
                let key = Array::try_from(encapsulation_key).ok()?;
                let key = EncapsulationKey::<MlKem1024>::new(&key).ok()?;
                let (ciphertext, shared_key) = key.encapsulate_with_rng(&mut R::rng());

                Some((ciphertext.to_vec(), shared_key.to_vec()))
            }
        }
    }
}

impl fmt::Display for EncryptionContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::X25519(_) => write!(f, "x25519"),
            Self::MlKem512X25519(_) => write!(f, "ml-kem-512-x25519"),
            Self::MlKem768X25519(_) => write!(f, "ml-kem-768-x25519"),
            Self::MlKem1024X25519(_) => write!(f, "ml-kem-1024-x25519"),
        }
    }
}

/// Role of the session.
#[derive(Debug, Clone, Copy)]
pub enum Role {
    /// Initiator (Alice).
    Initiator,

    /// Responder (Bob).
    Responder,
}

/// Key context.
pub(super) struct KeyContext {
    /// Key used to encrypt outbound messages.
    pub send_key: Vec<u8>,

    /// Key used to decrypt inbound messages.
    pub recv_key: Vec<u8>,

    /// SipHash context for (de)obfuscating message lengths.
    pub sip: SipHash,
}

impl KeyContext {
    /// Create new [`KeyContext`].
    pub fn new(send_key: Vec<u8>, recv_key: Vec<u8>, sip: SipHash) -> Self {
        Self {
            send_key,
            recv_key,
            sip,
        }
    }
}

/// Session manager.
///
/// Responsible for creating context for inbound and outboudn NTCP2 sessions.
pub struct SessionManager<R: Runtime> {
    /// Allow local addresses.
    allow_local: bool,

    /// Allow inbound and outbond PQ connections.
    allow_pq: bool,

    /// Inbound state.
    inbound: InboundState,

    /// Outbound state.
    outbound: Arc<OutboundState>,

    /// Local NTCP2 IV.
    local_iv: [u8; 16],

    /// Router context.
    router_ctx: RouterContext<R>,

    /// Local NTCP2 static key.
    static_key: StaticPrivateKey,

    /// TX channel for sending events to `SubsystemManager`.
    transport_tx: Sender<SubsystemEvent>,
}

impl<R: Runtime> SessionManager<R> {
    /// Create new [`SessionManager`].
    ///
    /// This function initializes the common state for both inbound and outbound connections.
    ///
    /// See the beginning of [1] for steps on generating start state.
    ///
    /// [1]: https://geti2p.net/spec/ntcp2#key-derivation-function-kdf-for-handshake-message-1
    pub fn new(
        static_key: StaticPrivateKey,
        local_iv: [u8; 16],
        router_ctx: RouterContext<R>,
        allow_local: bool,
        allow_pq: bool,
        transport_tx: Sender<SubsystemEvent>,
    ) -> Self {
        let public_key = static_key.public();
        let make_key_context = |protocol_name: &str| -> ([u8; 32], [u8; 32], [u8; 32]) {
            let chaining_key = Sha256::new().update(protocol_name.as_bytes()).finalize_new();

            let outbound_state = Sha256::new().update(chaining_key).finalize_new();
            let inbound_state =
                Sha256::new().update(outbound_state).update(&public_key).finalize_new();

            (chaining_key, outbound_state, inbound_state)
        };

        let x25519 = make_key_context(constants::x25519::PROTOCOL_NAME);
        let ml_kem_512 = make_key_context(constants::ml_kem_512::PROTOCOL_NAME);
        let ml_kem_768 = make_key_context(constants::ml_kem_768::PROTOCOL_NAME);
        let ml_kem_1024 = make_key_context(constants::ml_kem_1024::PROTOCOL_NAME);

        Self {
            allow_local,
            allow_pq,
            inbound: InboundState {
                ml_kem: match static_key {
                    StaticPrivateKey::X25519(_) => None,
                    StaticPrivateKey::MlKem512X25519(_) =>
                        Some(MlKemContext::MlKem512X25519((ml_kem_512.0, ml_kem_512.2))),
                    StaticPrivateKey::MlKem768X25519(_) =>
                        Some(MlKemContext::MlKem768X25519((ml_kem_768.0, ml_kem_768.2))),
                    StaticPrivateKey::MlKem1024X25519(_) => Some(MlKemContext::MlKem1024X25519((
                        ml_kem_1024.0,
                        ml_kem_1024.2,
                    ))),
                },
                x25519: (x25519.0, x25519.2),
            },
            outbound: Arc::new(OutboundState {
                x25519: (x25519.0, x25519.1),
                ml_kem_512: (ml_kem_512.0, ml_kem_512.1),
                ml_kem_768: (ml_kem_768.0, ml_kem_768.1),
                ml_kem_1024: (ml_kem_1024.0, ml_kem_1024.1),
            }),
            local_iv,
            static_key,
            router_ctx,
            transport_tx,
        }
    }

    /// Called by [`SessionManager::create_session()`] to open outbound session to `router`.
    async fn create_session_inner(
        router_info: RouterInfo,
        net_id: u8,
        local_info: Bytes,
        local_key: StaticPrivateKey,
        outbound_state: Arc<OutboundState>,
        allow_local: bool,
        allow_pq: bool,
        event_handle: EventHandle<R>,
        transport_tx: Sender<SubsystemEvent>,
        started: R::Instant,
        metrics_handle: R::MetricsHandle,
        ipv4: bool,
        ipv6: bool,
    ) -> Result<Ntcp2Session<R>, Ntcp2Error> {
        let router_id = router_info.identity.id();

        let (remote_key, iv, socket_address, noise_ctx) = {
            let Some(RouterAddress::Ntcp2 {
                socket_address: Some(socket_address),
                static_key,
                iv: Some(iv),
                options,
                ..
            }) = router_info.addresses().find(|address| match address {
                RouterAddress::Ntcp2 {
                    socket_address: Some(socket_address),
                    iv: Some(_),
                    ..
                } => match socket_address.ip() {
                    IpAddr::V4(address) if !is_global(address) && !allow_local => {
                        tracing::warn!(
                            target: LOG_TARGET,
                            %router_id,
                            ?address,
                            "tried to dial local ipv4 address but local addresses were disabled",
                        );
                        false
                    }
                    IpAddr::V4(address) if !ipv4 => {
                        tracing::trace!(
                            target: LOG_TARGET,
                            %router_id,
                            ?address,
                            "ignoring ipv4 address, not supported locally",
                        );
                        false
                    }
                    IpAddr::V6(address) if address.is_loopback() && !allow_local => {
                        tracing::warn!(
                            target: LOG_TARGET,
                            ?address,
                            "tried to dial local ipv6 address but local addresses were disabled",
                        );
                        false
                    }
                    IpAddr::V6(address) if !ipv6 => {
                        tracing::trace!(
                            target: LOG_TARGET,
                            %router_id,
                            ?address,
                            "ignoring ipv6 address, not supported locally",
                        );
                        false
                    }
                    _ => true,
                },
                RouterAddress::Ssu2 { .. } => false,
                RouterAddress::Ntcp2 { .. } => false,
            })
            else {
                tracing::debug!(
                    target: LOG_TARGET,
                    "router doesn't have a dialable address",
                );
                return Err(Ntcp2Error::NoAddress);
            };

            let noise_ctx = match (options.get(&Str::from("pq")), allow_pq) {
                (Some(option), true) => match &**option {
                    "3" => EncryptionContext::MlKem512X25519(NoiseContext::new(
                        outbound_state.ml_kem_512.0,
                        outbound_state.ml_kem_512.1,
                    )),
                    "4" => EncryptionContext::MlKem768X25519(NoiseContext::new(
                        outbound_state.ml_kem_768.0,
                        outbound_state.ml_kem_768.1,
                    )),
                    "5" => EncryptionContext::MlKem1024X25519(NoiseContext::new(
                        outbound_state.ml_kem_1024.0,
                        outbound_state.ml_kem_1024.1,
                    )),
                    _ => EncryptionContext::X25519(NoiseContext::new(
                        outbound_state.x25519.0,
                        outbound_state.x25519.1,
                    )),
                },
                _ => EncryptionContext::X25519(NoiseContext::new(
                    outbound_state.x25519.0,
                    outbound_state.x25519.1,
                )),
            };

            (static_key, iv, socket_address, noise_ctx)
        };

        tracing::trace!(
            target: LOG_TARGET,
            %router_id,
            ?socket_address,
            "start dialing remote peer",
        );

        let Some(mut stream) = R::TcpStream::connect(*socket_address).await else {
            tracing::debug!(
                target: LOG_TARGET,
                %router_id,
                "failed to dial router",
            );
            return Err(Ntcp2Error::ConnectFailure);
        };
        let router_hash = router_info.identity.hash().to_vec();

        // create `SessionRequest` message and send it remote peer
        let (mut initiator, message) = Initiator::new::<R>(
            noise_ctx,
            local_info,
            local_key,
            remote_key,
            router_hash,
            *iv,
            net_id,
        )?;
        stream.write_all(&message).await.map_err(|_| Ntcp2Error::IoError)?;

        tracing::trace!(
            target: LOG_TARGET,
            %router_id,
            "SessionRequest sent, read SessonCreated",
        );

        // read `SessionCreated` and decrypt & parse it to find padding length
        let mut reply = alloc::vec![0u8; initiator.session_created_size()?];
        stream.read_exact::<R>(&mut reply).await.map_err(|_| Ntcp2Error::IoError)?;

        let padding_len = initiator.register_session_created::<R>(&reply)?;

        // read padding and finalize session by sending `SessionConfirmed`
        let mut reply = alloc::vec![0u8; padding_len];
        stream.read_exact::<R>(&mut reply).await.map_err(|_| Ntcp2Error::IoError)?;

        tracing::trace!(
            target: LOG_TARGET,
            %router_id,
            "padding for SessionCreated read, create and send SessionConfirmed",
        );

        let (key_context, message, encryption) = initiator.finalize(&reply)?;
        stream.write_all(&message).await.map_err(|_| Ntcp2Error::IoError)?;

        Ok(Ntcp2Session::<R>::new(
            Role::Initiator,
            *socket_address,
            router_info,
            stream,
            key_context,
            Direction::Outbound,
            event_handle,
            transport_tx,
            started,
            metrics_handle,
            encryption,
        ))
    }

    /// Create new [`Handshaker`] for initiator (Alice).
    ///
    /// Implements the key generation from [1], creates a `SessionRequest` message and returns
    /// that message together with an [`Initiator`] object which allows the call to drive progress
    /// on the opening connection.
    ///
    /// [1]: https://geti2p.net/spec/ntcp2#key-derivation-function-kdf-for-handshake-message-1
    ///
    /// `ipv4` and `ipv6` parameters indicate whether `NtcpSession` is able to dial remote routers
    /// over those protocols.
    pub fn create_session(
        &self,
        router: RouterInfo,
        ipv4: bool,
        ipv6: bool,
    ) -> impl Future<Output = Result<Ntcp2Session<R>, (Option<RouterId>, Ntcp2Error)>> {
        let net_id = self.router_ctx.net_id();
        let local_info = self.router_ctx.router_info();
        let local_key = self.static_key.clone();
        let allow_local = self.allow_local;
        let allow_pq = self.allow_pq;
        let event_handle = self.router_ctx.event_handle().clone();
        let router_id = router.identity.id();
        let transport_tx = self.transport_tx.clone();
        let metrics_handle = self.router_ctx.metrics_handle().clone();
        let outbound_state = Arc::clone(&self.outbound);
        let started = R::now();

        async move {
            match Self::create_session_inner(
                router,
                net_id,
                local_info,
                local_key,
                outbound_state,
                allow_local,
                allow_pq,
                event_handle,
                transport_tx.clone(),
                started,
                metrics_handle,
                ipv4,
                ipv6,
            )
            .await
            {
                Ok(session) => Ok(session),
                Err(error) => {
                    tracing::debug!(
                        target: LOG_TARGET,
                        %router_id,
                        ?error,
                        "failed to handshake with remote router",
                    );

                    if let Err(error) = transport_tx
                        .send(SubsystemEvent::ConnectionFailure {
                            router_id: router_id.clone(),
                        })
                        .await
                    {
                        tracing::warn!(
                            target: LOG_TARGET,
                            %router_id,
                            ?error,
                            "failed to report connection failure to subsystem manager",
                        );
                    }

                    Err((Some(router_id), error))
                }
            }
        }
    }

    /// Called by [`SessionManager::accept_session()`] to accept an inbound session.
    async fn accept_session_inner(
        mut stream: R::TcpStream,
        address: SocketAddr,
        net_id: u8,
        local_router_hash: Vec<u8>,
        inbound_state: InboundState,
        local_key: StaticPrivateKey,
        iv: [u8; 16],
        profile_storage: ProfileStorage<R>,
        event_handle: EventHandle<R>,
        transport_tx: Sender<SubsystemEvent>,
        started: R::Instant,
        metrics_handle: R::MetricsHandle,
    ) -> Result<Ntcp2Session<R>, Ntcp2Error> {
        tracing::trace!(
            target: LOG_TARGET,
            "read SessionRequest from socket",
        );

        // read public key which is fixed 32-bytes long
        let mut message = vec![0u8; 32];
        stream.read_exact::<R>(&mut message).await.map_err(|_| Ntcp2Error::IoError)?;

        // initialize responder state and get the size of payload
        //
        // payload size is not fixed since ml-kem encapsulation keys are variable-sized
        let (mut responder, payload_size) = Responder::new(
            inbound_state,
            local_key,
            local_router_hash,
            iv,
            net_id,
            message,
        )?;

        let mut message = vec![0u8; payload_size];
        stream.read_exact::<R>(&mut message).await.map_err(|_| Ntcp2Error::IoError)?;

        // process `SessionRequest` and receive padding length
        let padding_len = responder.handle_session_request::<R>(message)?;

        // read padding and create session if the peer is accepted
        let mut padding = alloc::vec![0u8; padding_len];
        stream.read_exact::<R>(&mut padding).await.map_err(|_| Ntcp2Error::IoError)?;

        let (message, message_len) = responder.create_session::<R>(padding)?;
        stream.write_all(&message).await.map_err(|_| Ntcp2Error::IoError)?;

        // read `SessionConfirmed` message and finalize session
        let mut message = alloc::vec![0u8; message_len];
        stream.read_exact::<R>(&mut message).await.map_err(|_| Ntcp2Error::IoError)?;

        match responder.finalize::<R>(message) {
            Ok((key_context, router_info, serialized, encryption)) => {
                if router_info.net_id() != net_id {
                    tracing::warn!(
                        target: LOG_TARGET,
                        local_net_id = ?net_id,
                        remote_net_id = ?router_info.net_id(),
                        "remote router is part of a different network",
                    );

                    let _ = stream.close().await;
                    return Err(Ntcp2Error::NetworkMismatch);
                }

                // add router to router storage so we can later on use it for outbound connections
                profile_storage.discover_router(router_info.clone(), serialized);

                Ok(Ntcp2Session::new(
                    Role::Responder,
                    address,
                    router_info,
                    stream,
                    key_context,
                    Direction::Inbound,
                    event_handle,
                    transport_tx,
                    started,
                    metrics_handle,
                    encryption,
                ))
            }
            Err(error) => {
                tracing::debug!(
                    target: LOG_TARGET,
                    ?error,
                    "failed to accept session",
                );
                let _ = stream.close().await;

                Err(error)
            }
        }
    }

    /// Accept inbound TCP connection and negotiate NTCP2 session parameters for it.
    pub fn accept_session(
        &self,
        stream: R::TcpStream,
        address: SocketAddr,
    ) -> impl Future<Output = Result<Ntcp2Session<R>, (Option<RouterId>, Ntcp2Error)>> {
        let net_id = self.router_ctx.net_id();
        let local_router_hash = self.router_ctx.router_id().to_vec();
        let local_key = self.static_key.clone();
        let iv = self.local_iv;
        let profile_storage = self.router_ctx.profile_storage().clone();
        let event_handle = self.router_ctx.event_handle().clone();
        let transport_tx = self.transport_tx.clone();
        let metrics_handle = self.router_ctx.metrics_handle().clone();
        let inbound_state = self.inbound.clone();
        let started = R::now();

        async move {
            Self::accept_session_inner(
                stream,
                address,
                net_id,
                local_router_hash,
                inbound_state,
                local_key,
                iv,
                profile_storage,
                event_handle,
                transport_tx,
                started,
                metrics_handle,
            )
            .await
            .map_err(|error| (None, error))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        crypto::{SigningPrivateKey, StaticPrivateKey},
        events::EventManager,
        i2np::{Message, MessageType, I2NP_MESSAGE_EXPIRATION},
        primitives::{Capabilities, Date, Mapping, RouterAddress, RouterIdentity, RouterInfo, Str},
        profile::ProfileStorage,
        router::context::builder::RouterContextBuilder,
        runtime::{
            mock::{MockRuntime, MockTcpListener, MockTcpStream},
            Runtime, TcpListener as _,
        },
        subsystem::OutboundMessage,
        timeout,
        transport::ntcp2::{listener::Ntcp2Listener, session::SessionManager, TerminationReason},
    };
    use bytes::Bytes;
    use futures::StreamExt;
    use std::{
        marker::PhantomData,
        net::{Ipv4Addr, Ipv6Addr, SocketAddr},
        time::Duration,
    };
    use thingbuf::mpsc::channel;
    use tokio::net::TcpListener;

    struct Ntcp2Builder<R: Runtime> {
        net_id: u8,
        router_address: Option<RouterAddress>,
        ntcp2_iv: [u8; 16],
        ntcp2_key: [u8; 32],
        ipv6: bool,
        ml_kem: Option<usize>,
        _runtime: PhantomData<R>,
    }

    impl<R: Runtime> Ntcp2Builder<R> {
        fn new() -> Self {
            use rand::Rng;

            let ntcp2_key = {
                let mut local_key = [0u8; 32];
                R::rng().fill_bytes(&mut local_key);
                local_key
            };
            let ntcp2_iv = {
                let mut local_iv = [0u8; 16];
                R::rng().fill_bytes(&mut local_iv);
                local_iv
            };

            Self {
                net_id: 2u8,
                router_address: None,
                ntcp2_iv,
                ntcp2_key,
                ipv6: false,
                ml_kem: None,
                _runtime: PhantomData::default(),
            }
        }

        fn with_net_id(mut self, net_id: u8) -> Self {
            self.net_id = net_id;
            self
        }

        fn with_ipv6(mut self) -> Self {
            self.ipv6 = true;
            self
        }

        fn with_ml_kem(mut self, ml_kem: Option<usize>) -> Self {
            self.ml_kem = ml_kem;
            self
        }

        fn with_router_address(mut self, port: u16) -> Self {
            self.router_address = Some(RouterAddress::new_published_ntcp2(
                self.ntcp2_key.clone(),
                self.ntcp2_iv,
                self.ml_kem,
                false,
                "127.0.0.1".parse().unwrap(),
                SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), port),
            ));
            self
        }

        fn with_router_address_ipv6(mut self, port: u16) -> Self {
            self.router_address = Some(RouterAddress::new_published_ntcp2(
                self.ntcp2_key.clone(),
                self.ntcp2_iv,
                self.ml_kem,
                false,
                "::1".parse().unwrap(),
                SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), port),
            ));
            self
        }

        fn build(mut self) -> Ntcp2 {
            let signing_key = SigningPrivateKey::random(R::rng());
            let static_key = StaticPrivateKey::random(R::rng());
            let identity =
                RouterIdentity::from_keys::<MockRuntime>(&static_key, &signing_key).unwrap();
            let router_info = RouterInfo {
                identity,
                published: Date::new(
                    (MockRuntime::time_since_epoch() - Duration::from_secs(2 * 60)).as_millis()
                        as u64,
                ),
                addresses: Vec::from_iter([self.router_address.take().unwrap_or_else(|| {
                    if self.ipv6 {
                        RouterAddress::new_unpublished_ntcp2(
                            self.ntcp2_key.clone(),
                            SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 8888),
                        )
                    } else {
                        RouterAddress::new_unpublished_ntcp2(
                            self.ntcp2_key.clone(),
                            SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 8888),
                        )
                    }
                })]),
                options: Mapping::from_iter([
                    (Str::from("netId"), Str::from(self.net_id.to_string())),
                    (Str::from("caps"), Str::from("L")),
                ]),
                net_id: self.net_id,
                capabilities: Capabilities::parse(&Str::from("L")).unwrap(),
            };

            Ntcp2 {
                ntcp2_iv: self.ntcp2_iv,
                ntcp2_key: self.ntcp2_key,
                router_info,
                signing_key,
                static_key,
            }
        }
    }

    struct Ntcp2 {
        ntcp2_iv: [u8; 16],
        ntcp2_key: [u8; 32],
        router_info: RouterInfo,
        signing_key: SigningPrivateKey,
        static_key: StaticPrivateKey,
    }

    #[tokio::test]
    async fn connection_succeeds_x25519() {
        connection_succeeds(None).await;
    }

    #[tokio::test]
    async fn connection_succeeds_ml_kem_512() {
        connection_succeeds(Some(3)).await;
    }

    #[tokio::test]
    async fn connection_succeeds_ml_kem_768() {
        connection_succeeds(Some(4)).await;
    }

    #[tokio::test]
    async fn connection_succeeds_ml_kem_1024() {
        connection_succeeds(Some(5)).await;
    }

    async fn connection_succeeds(kind: Option<usize>) {
        let (_event_mgr, _event_subscriber, event_handle) =
            EventManager::new(None, MockRuntime::register_metrics(vec![], None));
        let (transport_tx1, _transport_rx1) = channel(16);
        let local = Ntcp2Builder::<MockRuntime>::new().with_ml_kem(kind).build();
        let local_manager = SessionManager::new(
            match kind {
                None => StaticPrivateKey::from_bytes(local.ntcp2_key),
                Some(3) => StaticPrivateKey::from_bytes_ml_kem_512(local.ntcp2_key),
                Some(4) => StaticPrivateKey::from_bytes_ml_kem_768(local.ntcp2_key),
                Some(5) => StaticPrivateKey::from_bytes_ml_kem_1024(local.ntcp2_key),
                _ => unreachable!(),
            },
            local.ntcp2_iv,
            RouterContext::new(
                MockRuntime::register_metrics(Vec::new(), None),
                ProfileStorage::<MockRuntime>::new(&[], &[]),
                local.router_info.identity.id(),
                Bytes::from(local.router_info.serialize(&local.signing_key)),
                local.static_key,
                local.signing_key,
                2u8,
                event_handle.clone(),
            ),
            true,
            true,
            transport_tx1,
        );

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let remote = Ntcp2Builder::<MockRuntime>::new()
            .with_ml_kem(kind)
            .with_router_address(listener.local_addr().unwrap().port())
            .build();
        let (transport_tx2, _transport_rx2) = channel(16);
        let remote_manager = SessionManager::new(
            match kind {
                None => StaticPrivateKey::from_bytes(remote.ntcp2_key),
                Some(3) => StaticPrivateKey::from_bytes_ml_kem_512(remote.ntcp2_key),
                Some(4) => StaticPrivateKey::from_bytes_ml_kem_768(remote.ntcp2_key),
                Some(5) => StaticPrivateKey::from_bytes_ml_kem_1024(remote.ntcp2_key),
                _ => unreachable!(),
            },
            remote.ntcp2_iv,
            RouterContext::new(
                MockRuntime::register_metrics(Vec::new(), None),
                ProfileStorage::<MockRuntime>::new(&[], &[]),
                remote.router_info.identity.id(),
                Bytes::from(remote.router_info.serialize(&remote.signing_key)),
                remote.static_key,
                remote.signing_key,
                2u8,
                event_handle.clone(),
            ),
            true,
            true,
            transport_tx2,
        );

        let handle = tokio::spawn(async move {
            local_manager.create_session(remote.router_info.clone(), true, false).await
        });

        let (stream, address) = timeout!(listener.accept()).await.unwrap().unwrap();
        let (res1, res2) = tokio::join!(
            remote_manager.accept_session(MockTcpStream::new(stream), address),
            handle
        );

        assert!(res1.is_ok());
        assert!(res2.unwrap().is_ok());
    }

    #[tokio::test]
    async fn invalid_network_id_initiator_x25519() {
        invalid_network_id_initiator(None).await;
    }

    #[tokio::test]
    async fn invalid_network_id_initiator_ml_kem_512() {
        invalid_network_id_initiator(Some(3)).await;
    }

    #[tokio::test]
    async fn invalid_network_id_initiator_ml_kem_768() {
        invalid_network_id_initiator(Some(4)).await;
    }

    #[tokio::test]
    async fn invalid_network_id_initiator_ml_kem_1024() {
        invalid_network_id_initiator(Some(5)).await;
    }

    async fn invalid_network_id_initiator(kind: Option<usize>) {
        let (_event_mgr, _event_subscriber, event_handle) =
            EventManager::new(None, MockRuntime::register_metrics(vec![], None));
        let local = Ntcp2Builder::<MockRuntime>::new().with_ml_kem(kind).with_net_id(128).build();
        let (transport_tx1, _transport_rx1) = channel(16);
        let local_manager = SessionManager::new(
            match kind {
                None => StaticPrivateKey::from_bytes(local.ntcp2_key),
                Some(3) => StaticPrivateKey::from_bytes_ml_kem_512(local.ntcp2_key),
                Some(4) => StaticPrivateKey::from_bytes_ml_kem_768(local.ntcp2_key),
                Some(5) => StaticPrivateKey::from_bytes_ml_kem_1024(local.ntcp2_key),
                _ => unreachable!(),
            },
            local.ntcp2_iv,
            RouterContext::new(
                MockRuntime::register_metrics(Vec::new(), None),
                ProfileStorage::<MockRuntime>::new(&[], &[]),
                local.router_info.identity.id(),
                Bytes::from(local.router_info.serialize(&local.signing_key)),
                local.static_key,
                local.signing_key,
                128,
                event_handle.clone(),
            ),
            true,
            true,
            transport_tx1,
        );

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let (transport_tx2, _transport_rx2) = channel(16);
        let remote = Ntcp2Builder::<MockRuntime>::new()
            .with_ml_kem(kind)
            .with_router_address(listener.local_addr().unwrap().port())
            .build();
        let remote_manager = SessionManager::new(
            match kind {
                None => StaticPrivateKey::from_bytes(remote.ntcp2_key),
                Some(3) => StaticPrivateKey::from_bytes_ml_kem_512(remote.ntcp2_key),
                Some(4) => StaticPrivateKey::from_bytes_ml_kem_768(remote.ntcp2_key),
                Some(5) => StaticPrivateKey::from_bytes_ml_kem_1024(remote.ntcp2_key),
                _ => unreachable!(),
            },
            remote.ntcp2_iv,
            RouterContext::new(
                MockRuntime::register_metrics(Vec::new(), None),
                ProfileStorage::<MockRuntime>::new(&[], &[]),
                remote.router_info.identity.id(),
                Bytes::from(remote.router_info.serialize(&remote.signing_key)),
                remote.static_key,
                remote.signing_key,
                2u8,
                event_handle.clone(),
            ),
            true,
            true,
            transport_tx2,
        );

        let handle = tokio::spawn(async move {
            let (stream, address) = timeout!(listener.accept()).await.unwrap().unwrap();
            remote_manager.accept_session(MockTcpStream::new(stream), address).await
        });

        assert!(local_manager
            .create_session(remote.router_info.clone(), true, false)
            .await
            .is_err());
        assert!(handle.await.unwrap().is_err());
    }

    #[tokio::test]
    async fn invalid_network_id_responder_x25519() {
        invalid_network_id_responder(None).await;
    }

    #[tokio::test]
    async fn invalid_network_id_responder_ml_kem_512() {
        invalid_network_id_responder(Some(3)).await;
    }

    #[tokio::test]
    async fn invalid_network_id_responder_ml_kem_768() {
        invalid_network_id_responder(Some(4)).await;
    }

    #[tokio::test]
    async fn invalid_network_id_responder_ml_kem_1024() {
        invalid_network_id_initiator(Some(5)).await;
    }

    async fn invalid_network_id_responder(kind: Option<usize>) {
        let (_event_mgr, _event_subscriber, event_handle) =
            EventManager::new(None, MockRuntime::register_metrics(vec![], None));
        let local = Ntcp2Builder::<MockRuntime>::new().with_ml_kem(kind).build();
        let (transport_tx1, _transport_rx1) = channel(16);
        let local_manager = SessionManager::new(
            match kind {
                None => StaticPrivateKey::from_bytes(local.ntcp2_key),
                Some(3) => StaticPrivateKey::from_bytes_ml_kem_512(local.ntcp2_key),
                Some(4) => StaticPrivateKey::from_bytes_ml_kem_768(local.ntcp2_key),
                Some(5) => StaticPrivateKey::from_bytes_ml_kem_1024(local.ntcp2_key),
                _ => unreachable!(),
            },
            local.ntcp2_iv,
            RouterContext::new(
                MockRuntime::register_metrics(Vec::new(), None),
                ProfileStorage::<MockRuntime>::new(&[], &[]),
                local.router_info.identity.id(),
                Bytes::from(local.router_info.serialize(&local.signing_key)),
                local.static_key,
                local.signing_key,
                2u8,
                event_handle.clone(),
            ),
            true,
            true,
            transport_tx1,
        );

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let remote = Ntcp2Builder::<MockRuntime>::new()
            .with_net_id(128)
            .with_ml_kem(kind)
            .with_router_address(listener.local_addr().unwrap().port())
            .build();
        let (transport_tx2, _transport_rx2) = channel(16);
        let remote_manager = SessionManager::new(
            match kind {
                None => StaticPrivateKey::from_bytes(remote.ntcp2_key),
                Some(3) => StaticPrivateKey::from_bytes_ml_kem_512(remote.ntcp2_key),
                Some(4) => StaticPrivateKey::from_bytes_ml_kem_768(remote.ntcp2_key),
                Some(5) => StaticPrivateKey::from_bytes_ml_kem_1024(remote.ntcp2_key),
                _ => unreachable!(),
            },
            remote.ntcp2_iv,
            RouterContext::new(
                MockRuntime::register_metrics(Vec::new(), None),
                ProfileStorage::<MockRuntime>::new(&[], &[]),
                remote.router_info.identity.id(),
                Bytes::from(remote.router_info.serialize(&remote.signing_key)),
                remote.static_key,
                remote.signing_key,
                128u8,
                event_handle.clone(),
            ),
            true,
            true,
            transport_tx2,
        );

        let handle = tokio::spawn(async move {
            let (stream, address) = timeout!(listener.accept()).await.unwrap().unwrap();
            remote_manager.accept_session(MockTcpStream::new(stream), address).await
        });

        assert!(local_manager
            .create_session(remote.router_info.clone(), true, false)
            .await
            .is_err());
        assert!(handle.await.unwrap().is_err());
    }

    #[tokio::test]
    async fn dialer_local_addresses_disabled() {
        let (_event_mgr, _event_subscriber, event_handle) =
            EventManager::new(None, MockRuntime::register_metrics(vec![], None));
        let (transport_tx1, _transport_rx1) = channel(16);
        let local = Ntcp2Builder::<MockRuntime>::new().build();
        let local_manager = SessionManager::new(
            StaticPrivateKey::from_bytes(local.ntcp2_key),
            local.ntcp2_iv,
            RouterContext::new(
                MockRuntime::register_metrics(Vec::new(), None),
                ProfileStorage::<MockRuntime>::new(&[], &[]),
                local.router_info.identity.id(),
                Bytes::from(local.router_info.serialize(&local.signing_key)),
                local.static_key,
                local.signing_key,
                2u8,
                event_handle.clone(),
            ),
            false,
            true,
            transport_tx1,
        );

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let remote = Ntcp2Builder::<MockRuntime>::new()
            .with_net_id(128)
            .with_router_address(listener.local_addr().unwrap().port())
            .build();
        let (transport_tx2, _transport_rx2) = channel(16);
        let remote_manager = SessionManager::new(
            StaticPrivateKey::from_bytes(remote.ntcp2_key),
            remote.ntcp2_iv,
            RouterContext::new(
                MockRuntime::register_metrics(Vec::new(), None),
                ProfileStorage::<MockRuntime>::new(&[], &[]),
                remote.router_info.identity.id(),
                Bytes::from(remote.router_info.serialize(&remote.signing_key)),
                remote.static_key,
                remote.signing_key,
                2u8,
                event_handle.clone(),
            ),
            true,
            true,
            transport_tx2,
        );

        tokio::spawn(async move {
            let (stream, address) = timeout!(listener.accept()).await.unwrap().unwrap();
            remote_manager.accept_session(MockTcpStream::new(stream), address).await
        });

        assert!(local_manager
            .create_session(remote.router_info.clone(), true, false)
            .await
            .is_err());
    }

    #[tokio::test]
    async fn listener_local_addresses_disabled() {
        let (_event_mgr, _event_subscriber, event_handle) =
            EventManager::new(None, MockRuntime::register_metrics(vec![], None));
        let (transport_tx1, _transport_rx1) = channel(16);
        let local = Ntcp2Builder::<MockRuntime>::new().build();
        let local_manager = SessionManager::new(
            StaticPrivateKey::from_bytes(local.ntcp2_key),
            local.ntcp2_iv,
            RouterContext::new(
                MockRuntime::register_metrics(Vec::new(), None),
                ProfileStorage::<MockRuntime>::new(&[], &[]),
                local.router_info.identity.id(),
                Bytes::from(local.router_info.serialize(&local.signing_key)),
                local.static_key,
                local.signing_key,
                2u8,
                event_handle.clone(),
            ),
            true,
            true,
            transport_tx1,
        );

        let listener = MockTcpListener::bind("127.0.0.1:0".parse().unwrap()).await.unwrap();
        let mut listener = Ntcp2Listener::<MockRuntime>::new(listener, false);
        let remote = Ntcp2Builder::<MockRuntime>::new()
            .with_net_id(128)
            .with_router_address(listener.local_address().port())
            .build();

        tokio::spawn(async move { while let Some(_) = listener.next().await {} });

        assert!(local_manager
            .create_session(remote.router_info.clone(), true, false)
            .await
            .is_err());
    }

    #[tokio::test]
    async fn received_expired_message_x25519() {
        received_expired_message(None).await;
    }

    #[tokio::test]
    async fn received_expired_message_ml_kem_512() {
        received_expired_message(Some(3)).await;
    }

    #[tokio::test]
    async fn received_expired_message_ml_kem_768() {
        received_expired_message(Some(4)).await;
    }

    #[tokio::test]
    async fn received_expired_message_ml_kem_1024() {
        received_expired_message(Some(5)).await;
    }

    async fn received_expired_message(kind: Option<usize>) {
        let local = Ntcp2Builder::<MockRuntime>::new().with_ml_kem(kind).build();
        let (_event_mgr, _event_subscriber, event_handle) =
            EventManager::new(None, MockRuntime::register_metrics(vec![], None));
        let (local_tx, local_rx) = channel(16);
        let local_manager = SessionManager::new(
            match kind {
                None => StaticPrivateKey::from_bytes(local.ntcp2_key),
                Some(3) => StaticPrivateKey::from_bytes_ml_kem_512(local.ntcp2_key),
                Some(4) => StaticPrivateKey::from_bytes_ml_kem_768(local.ntcp2_key),
                Some(5) => StaticPrivateKey::from_bytes_ml_kem_1024(local.ntcp2_key),
                _ => unreachable!(),
            },
            local.ntcp2_iv,
            RouterContext::new(
                MockRuntime::register_metrics(Vec::new(), None),
                ProfileStorage::<MockRuntime>::new(&[], &[]),
                local.router_info.identity.id(),
                Bytes::from(local.router_info.serialize(&local.signing_key)),
                local.static_key,
                local.signing_key,
                2u8,
                event_handle.clone(),
            ),
            true,
            true,
            local_tx,
        );

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let remote = Ntcp2Builder::<MockRuntime>::new()
            .with_ml_kem(kind)
            .with_router_address(listener.local_addr().unwrap().port())
            .build();
        let (remote_tx, remote_rx) = channel(16);

        let remote_manager = SessionManager::new(
            match kind {
                None => StaticPrivateKey::from_bytes(remote.ntcp2_key),
                Some(3) => StaticPrivateKey::from_bytes_ml_kem_512(remote.ntcp2_key),
                Some(4) => StaticPrivateKey::from_bytes_ml_kem_768(remote.ntcp2_key),
                Some(5) => StaticPrivateKey::from_bytes_ml_kem_1024(remote.ntcp2_key),
                _ => unreachable!(),
            },
            remote.ntcp2_iv,
            RouterContext::new(
                MockRuntime::register_metrics(Vec::new(), None),
                ProfileStorage::<MockRuntime>::new(&[], &[]),
                remote.router_info.identity.id(),
                Bytes::from(remote.router_info.serialize(&remote.signing_key)),
                remote.static_key,
                remote.signing_key,
                2u8,
                event_handle.clone(),
            ),
            true,
            true,
            remote_tx,
        );

        let handle = tokio::spawn(async move {
            local_manager.create_session(remote.router_info.clone(), true, false).await
        });

        let (stream, address) = timeout!(listener.accept()).await.unwrap().unwrap();
        let (res1, res2) = tokio::join!(
            remote_manager.accept_session(MockTcpStream::new(stream), address),
            handle
        );

        tokio::spawn(res1.unwrap().run());
        tokio::spawn(res2.unwrap().unwrap().run());

        let (_local_router, remote_command_tx) =
            tokio::time::timeout(Duration::from_secs(5), async {
                match remote_rx.recv().await {
                    Some(SubsystemEvent::ConnectionEstablished { router_id, tx }) =>
                        (router_id, tx),
                    _ => panic!("invalid event received"),
                }
            })
            .await
            .expect("no timeout");
        let (_remote_router, _local_command_tx) =
            tokio::time::timeout(Duration::from_secs(5), async {
                match local_rx.recv().await {
                    Some(SubsystemEvent::ConnectionEstablished { router_id, tx }) =>
                        (router_id, tx),
                    _ => panic!("invalid event received"),
                }
            })
            .await
            .expect("no timeout");

        // send non-expired database message
        remote_command_tx
            .send(OutboundMessage::Message(Message {
                expiration: MockRuntime::time_since_epoch() + I2NP_MESSAGE_EXPIRATION,
                message_type: MessageType::DatabaseStore,
                message_id: 1337u32,
                payload: vec![1, 1, 1, 1],
            }))
            .await
            .unwrap();

        tokio::time::timeout(Duration::from_secs(5), async {
            match local_rx.recv().await {
                Some(SubsystemEvent::Message { mut messages }) => {
                    assert_eq!(messages.len(), 1);
                    let (_, message) = messages.pop().unwrap();

                    assert_eq!(message.message_type, MessageType::DatabaseStore);
                    assert_eq!(message.message_id, 1337u32);
                    assert_eq!(message.payload, vec![1, 1, 1, 1]);
                }
                _ => panic!("invalid event received"),
            }
        })
        .await
        .expect("no timeout");

        // send expired database message
        remote_command_tx
            .send(OutboundMessage::Message(Message {
                expiration: MockRuntime::time_since_epoch() - Duration::from_secs(5),
                message_type: MessageType::DatabaseStore,
                message_id: 1338u32,
                payload: vec![2, 2, 2, 2],
            }))
            .await
            .unwrap();

        // operation times out because the message was expired
        tokio::time::timeout(Duration::from_secs(1), async {
            match local_rx.recv().await {
                _ => panic!("didn't expect to receive anything"),
            }
        })
        .await
        .unwrap_err();

        // send another non-expired database message
        remote_command_tx
            .send(OutboundMessage::Message(Message {
                expiration: MockRuntime::time_since_epoch() + I2NP_MESSAGE_EXPIRATION,
                message_type: MessageType::DatabaseStore,
                message_id: 1339u32,
                payload: vec![3, 3, 3, 3],
            }))
            .await
            .unwrap();

        tokio::time::timeout(Duration::from_secs(5), async {
            match local_rx.recv().await {
                Some(SubsystemEvent::Message { mut messages }) => {
                    assert_eq!(messages.len(), 1);
                    let (_, message) = messages.pop().unwrap();

                    assert_eq!(message.message_type, MessageType::DatabaseStore);
                    assert_eq!(message.message_id, 1339u32);
                    assert_eq!(message.payload, vec![3, 3, 3, 3]);
                }
                _ => panic!("invalid event received"),
            }
        })
        .await
        .expect("no timeout");
    }

    #[tokio::test]
    async fn clock_skew_too_far_in_the_future_x25519() {
        clock_skew_too_far_in_the_future(None).await;
    }

    #[tokio::test]
    async fn clock_skew_too_far_in_the_future_ml_kem_512() {
        clock_skew_too_far_in_the_future(Some(3)).await;
    }

    #[tokio::test]
    async fn clock_skew_too_far_in_the_future_ml_kem_768() {
        clock_skew_too_far_in_the_future(Some(4)).await;
    }

    #[tokio::test]
    async fn clock_skew_too_far_in_the_future_ml_kem_1024() {
        clock_skew_too_far_in_the_future(Some(5)).await;
    }

    async fn clock_skew_too_far_in_the_future(kind: Option<usize>) {
        let (_event_mgr, _event_subscriber, event_handle) =
            EventManager::new(None, MockRuntime::register_metrics(vec![], None));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let (transport_tx1, _transport_rx1) = channel(16);
        let remote = Ntcp2Builder::<MockRuntime>::new()
            .with_ml_kem(kind)
            .with_router_address(listener.local_addr().unwrap().port())
            .build();
        let remote_manager = SessionManager::new(
            match kind {
                None => StaticPrivateKey::from_bytes(remote.ntcp2_key),
                Some(3) => StaticPrivateKey::from_bytes_ml_kem_512(remote.ntcp2_key),
                Some(4) => StaticPrivateKey::from_bytes_ml_kem_768(remote.ntcp2_key),
                Some(5) => StaticPrivateKey::from_bytes_ml_kem_1024(remote.ntcp2_key),
                _ => unreachable!(),
            },
            remote.ntcp2_iv,
            RouterContext::new(
                MockRuntime::register_metrics(Vec::new(), None),
                ProfileStorage::<MockRuntime>::new(&[], &[]),
                remote.router_info.identity.id(),
                Bytes::from(remote.router_info.serialize(&remote.signing_key)),
                remote.static_key,
                remote.signing_key,
                2u8,
                event_handle.clone(),
            ),
            true,
            true,
            transport_tx1,
        );

        let handle = std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async move {
                MockRuntime::set_time(Some(
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .expect("to succeed")
                        + 2 * MAX_CLOCK_SKEW,
                ));

                let (transport_tx1, _transport_rx1) = channel(16);
                let local = Ntcp2Builder::<MockRuntime>::new().with_ml_kem(kind).build();
                let local_manager = SessionManager::new(
                    match kind {
                        None => StaticPrivateKey::from_bytes(local.ntcp2_key),
                        Some(3) => StaticPrivateKey::from_bytes_ml_kem_512(local.ntcp2_key),
                        Some(4) => StaticPrivateKey::from_bytes_ml_kem_768(local.ntcp2_key),
                        Some(5) => StaticPrivateKey::from_bytes_ml_kem_1024(local.ntcp2_key),
                        _ => unreachable!(),
                    },
                    local.ntcp2_iv,
                    RouterContext::new(
                        MockRuntime::register_metrics(Vec::new(), None),
                        ProfileStorage::<MockRuntime>::new(&[], &[]),
                        local.router_info.identity.id(),
                        Bytes::from(local.router_info.serialize(&local.signing_key)),
                        local.static_key,
                        local.signing_key,
                        2u8,
                        event_handle.clone(),
                    ),
                    true,
                    true,
                    transport_tx1,
                );

                local_manager.create_session(remote.router_info.clone(), true, false).await
            })
        });

        let (stream, address) = timeout!(listener.accept()).await.unwrap().unwrap();
        let future = tokio::task::spawn_blocking(move || handle.join().unwrap());
        let (res1, _res2) = tokio::join!(
            remote_manager.accept_session(MockTcpStream::new(stream), address),
            future
        );

        assert!(res1.is_err());
    }

    #[tokio::test]
    async fn clock_skew_too_far_in_the_past_x25519() {
        clock_skew_too_far_in_the_past(None).await;
    }

    #[tokio::test]
    async fn clock_skew_too_far_in_the_past_ml_kem_512() {
        clock_skew_too_far_in_the_past(Some(3)).await;
    }

    #[tokio::test]
    async fn clock_skew_too_far_in_the_past_ml_kem_768() {
        clock_skew_too_far_in_the_past(Some(4)).await;
    }

    #[tokio::test]
    async fn clock_skew_too_far_in_the_past_ml_kem_1024() {
        clock_skew_too_far_in_the_past(Some(5)).await;
    }

    async fn clock_skew_too_far_in_the_past(kind: Option<usize>) {
        let (_event_mgr, _event_subscriber, event_handle) =
            EventManager::new(None, MockRuntime::register_metrics(vec![], None));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let (transport_tx1, _transport_rx1) = channel(16);
        let remote = Ntcp2Builder::<MockRuntime>::new()
            .with_ml_kem(kind)
            .with_router_address(listener.local_addr().unwrap().port())
            .build();
        let remote_manager = SessionManager::new(
            match kind {
                None => StaticPrivateKey::from_bytes(remote.ntcp2_key),
                Some(3) => StaticPrivateKey::from_bytes_ml_kem_512(remote.ntcp2_key),
                Some(4) => StaticPrivateKey::from_bytes_ml_kem_768(remote.ntcp2_key),
                Some(5) => StaticPrivateKey::from_bytes_ml_kem_1024(remote.ntcp2_key),
                _ => unreachable!(),
            },
            remote.ntcp2_iv,
            RouterContext::new(
                MockRuntime::register_metrics(Vec::new(), None),
                ProfileStorage::<MockRuntime>::new(&[], &[]),
                remote.router_info.identity.id(),
                Bytes::from(remote.router_info.serialize(&remote.signing_key)),
                remote.static_key,
                remote.signing_key,
                2u8,
                event_handle.clone(),
            ),
            true,
            true,
            transport_tx1,
        );

        let handle = std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async move {
                MockRuntime::set_time(Some(
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .expect("to succeed")
                        - 2 * MAX_CLOCK_SKEW,
                ));

                let (transport_tx1, _transport_rx1) = channel(16);
                let local = Ntcp2Builder::<MockRuntime>::new().with_ml_kem(kind).build();
                let local_manager = SessionManager::new(
                    match kind {
                        None => StaticPrivateKey::from_bytes(local.ntcp2_key),
                        Some(3) => StaticPrivateKey::from_bytes_ml_kem_512(local.ntcp2_key),
                        Some(4) => StaticPrivateKey::from_bytes_ml_kem_768(local.ntcp2_key),
                        Some(5) => StaticPrivateKey::from_bytes_ml_kem_1024(local.ntcp2_key),
                        _ => unreachable!(),
                    },
                    local.ntcp2_iv,
                    RouterContext::new(
                        MockRuntime::register_metrics(Vec::new(), None),
                        ProfileStorage::<MockRuntime>::new(&[], &[]),
                        local.router_info.identity.id(),
                        Bytes::from(local.router_info.serialize(&local.signing_key)),
                        local.static_key,
                        local.signing_key,
                        2u8,
                        event_handle.clone(),
                    ),
                    true,
                    true,
                    transport_tx1,
                );

                local_manager.create_session(remote.router_info.clone(), true, false).await
            })
        });

        let (stream, address) = timeout!(listener.accept()).await.unwrap().unwrap();
        let future = tokio::task::spawn_blocking(move || handle.join().unwrap());
        let (res1, _res2) = tokio::join!(
            remote_manager.accept_session(MockTcpStream::new(stream), address),
            future
        );

        assert!(res1.is_err());
    }

    #[tokio::test]
    async fn idle_timeout() {
        let (_event_mgr, _event_subscriber, event_handle) =
            EventManager::new(None, MockRuntime::register_metrics(vec![], None));
        let (transport_tx1, transport_rx1) = channel(16);
        let local = Ntcp2Builder::<MockRuntime>::new().build();
        let local_manager = SessionManager::new(
            StaticPrivateKey::from_bytes(local.ntcp2_key),
            local.ntcp2_iv,
            RouterContext::new(
                MockRuntime::register_metrics(Vec::new(), None),
                ProfileStorage::<MockRuntime>::new(&[], &[]),
                local.router_info.identity.id(),
                Bytes::from(local.router_info.serialize(&local.signing_key)),
                local.static_key,
                local.signing_key,
                2u8,
                event_handle.clone(),
            ),
            true,
            true,
            transport_tx1,
        );

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let remote = Ntcp2Builder::<MockRuntime>::new()
            .with_router_address(listener.local_addr().unwrap().port())
            .build();
        let (transport_tx2, transport_rx2) = channel(16);
        let remote_manager = SessionManager::new(
            StaticPrivateKey::from_bytes(remote.ntcp2_key),
            remote.ntcp2_iv,
            RouterContext::new(
                MockRuntime::register_metrics(Vec::new(), None),
                ProfileStorage::<MockRuntime>::new(&[], &[]),
                remote.router_info.identity.id(),
                Bytes::from(remote.router_info.serialize(&remote.signing_key)),
                remote.static_key,
                remote.signing_key,
                2u8,
                event_handle.clone(),
            ),
            true,
            true,
            transport_tx2,
        );

        let handle = tokio::spawn(async move {
            local_manager.create_session(remote.router_info.clone(), true, false).await
        });

        let (stream, address) = tokio::time::timeout(Duration::from_secs(5), listener.accept())
            .await
            .unwrap()
            .unwrap();
        let stream = MockTcpStream::new(stream);
        let (res1, res2) = tokio::join!(remote_manager.accept_session(stream, address), handle);

        let handle1 = tokio::spawn(res1.unwrap().run());
        let handle2 = tokio::spawn(res2.unwrap().unwrap().run());

        let tx1 = match timeout!(transport_rx1.recv()).await.unwrap().unwrap() {
            SubsystemEvent::ConnectionEstablished { tx, .. } => tx,
            _ => panic!("unexpected event"),
        };
        let _tx2 = match timeout!(transport_rx2.recv()).await.unwrap().unwrap() {
            SubsystemEvent::ConnectionEstablished { tx, .. } => tx,
            _ => panic!("unexpected event"),
        };

        tx1.send(OutboundMessage::Message(Message {
            message_type: MessageType::DatabaseStore,
            message_id: 1337,
            expiration: MockRuntime::time_since_epoch() + I2NP_MESSAGE_EXPIRATION,
            payload: vec![1, 3, 3, 7],
        }))
        .await
        .unwrap();

        let _tx2 = match transport_rx2.recv().await.unwrap() {
            SubsystemEvent::Message { messages } => {
                assert_eq!(messages.len(), 1);
                assert_eq!(messages[0].1.message_type, MessageType::DatabaseStore);
                assert_eq!(messages[0].1.message_id, 1337);
                assert_eq!(&messages[0].1.payload, &[1, 3, 3, 7]);
            }
            _ => panic!("unexpected event"),
        };

        assert_eq!(handle1.await.unwrap().1, TerminationReason::IdleTimeout);
        assert_eq!(handle2.await.unwrap().1, TerminationReason::IdleTimeout);
    }

    #[tokio::test]
    async fn dial_ipv4_only_ipv6_supported() {
        let local = Ntcp2Builder::<MockRuntime>::new().with_net_id(128).build();
        let (transport_tx1, _transport_rx1) = channel(16);
        let local_manager = SessionManager::new(
            StaticPrivateKey::from_bytes(local.ntcp2_key),
            local.ntcp2_iv,
            RouterContextBuilder::default()
                .with_router_info(
                    local.router_info.clone(),
                    local.static_key.clone(),
                    local.signing_key.clone(),
                )
                .build(),
            true,
            true,
            transport_tx1,
        );

        // remote only supports ipv6
        let listener = TcpListener::bind("[::]:0").await.unwrap();
        let remote = Ntcp2Builder::<MockRuntime>::new()
            .with_router_address_ipv6(listener.local_addr().unwrap().port())
            .build();

        assert!(local_manager
            .create_session(remote.router_info.clone(), true, false)
            .await
            .is_err());
    }

    #[tokio::test]
    async fn dial_ipv6_only_ipv4_supported() {
        let local = Ntcp2Builder::<MockRuntime>::new().with_ipv6().with_net_id(128).build();
        let (transport_tx1, _transport_rx1) = channel(16);
        let local_manager = SessionManager::new(
            StaticPrivateKey::from_bytes(local.ntcp2_key),
            local.ntcp2_iv,
            RouterContextBuilder::default()
                .with_router_info(
                    local.router_info.clone(),
                    local.static_key.clone(),
                    local.signing_key.clone(),
                )
                .build(),
            true,
            true,
            transport_tx1,
        );

        // remote only supports ipv4
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let remote = Ntcp2Builder::<MockRuntime>::new()
            .with_router_address(listener.local_addr().unwrap().port())
            .build();

        assert!(local_manager
            .create_session(remote.router_info.clone(), false, true)
            .await
            .is_err());
    }

    #[tokio::test]
    async fn dial_ipv6_only_both_supported() {
        let local = Ntcp2Builder::<MockRuntime>::new().with_ipv6().with_net_id(128).build();
        let (transport_tx1, _transport_rx1) = channel(16);
        let local_manager = SessionManager::new(
            StaticPrivateKey::from_bytes(local.ntcp2_key),
            local.ntcp2_iv,
            RouterContextBuilder::default()
                .with_router_info(
                    local.router_info.clone(),
                    local.static_key.clone(),
                    local.signing_key.clone(),
                )
                .with_net_id(128)
                .build(),
            true,
            true,
            transport_tx1,
        );

        let listener = TcpListener::bind("[::]:0").await.unwrap();
        let (transport_tx1, _transport_rx1) = channel(16);
        let remote = Ntcp2Builder::<MockRuntime>::new()
            .with_net_id(128)
            .with_router_address_ipv6(listener.local_addr().unwrap().port())
            .build();
        let remote_manager = SessionManager::new(
            StaticPrivateKey::from_bytes(remote.ntcp2_key),
            remote.ntcp2_iv,
            RouterContextBuilder::default()
                .with_router_info(
                    remote.router_info.clone(),
                    remote.static_key.clone(),
                    remote.signing_key.clone(),
                )
                .with_net_id(128)
                .build(),
            true,
            true,
            transport_tx1,
        );

        let handle = tokio::spawn(async move {
            let (stream, address) = timeout!(listener.accept()).await.unwrap().unwrap();
            remote_manager
                .accept_session(MockTcpStream::new(stream), address)
                .await
                .unwrap()
        });

        assert!(local_manager
            .create_session(remote.router_info.clone(), true, true)
            .await
            .is_ok());
        assert!(handle.await.is_ok());
    }

    #[tokio::test]
    async fn dial_ipv4_only_both_supported() {
        let local = Ntcp2Builder::<MockRuntime>::new().with_ipv6().with_net_id(128).build();
        let (transport_tx1, _transport_rx1) = channel(16);
        let local_manager = SessionManager::new(
            StaticPrivateKey::from_bytes(local.ntcp2_key),
            local.ntcp2_iv,
            RouterContextBuilder::default()
                .with_router_info(
                    local.router_info.clone(),
                    local.static_key.clone(),
                    local.signing_key.clone(),
                )
                .with_net_id(128)
                .build(),
            true,
            true,
            transport_tx1,
        );

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let (transport_tx1, _transport_rx1) = channel(16);
        let remote = Ntcp2Builder::<MockRuntime>::new()
            .with_net_id(128)
            .with_router_address(listener.local_addr().unwrap().port())
            .build();
        let remote_manager = SessionManager::new(
            StaticPrivateKey::from_bytes(remote.ntcp2_key),
            remote.ntcp2_iv,
            RouterContextBuilder::default()
                .with_router_info(
                    remote.router_info.clone(),
                    remote.static_key.clone(),
                    remote.signing_key.clone(),
                )
                .with_net_id(128)
                .build(),
            true,
            true,
            transport_tx1,
        );

        let handle = tokio::spawn(async move {
            let (stream, address) = timeout!(listener.accept()).await.unwrap().unwrap();
            remote_manager
                .accept_session(MockTcpStream::new(stream), address)
                .await
                .unwrap()
        });

        assert!(local_manager
            .create_session(remote.router_info.clone(), true, true)
            .await
            .is_ok());
        assert!(handle.await.is_ok());
    }
}
