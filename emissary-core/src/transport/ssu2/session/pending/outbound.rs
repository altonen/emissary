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
    constants,
    crypto::{
        chachapoly::ChaChaPoly, hmac::Hmac, EphemeralPrivateKey, SigningPublicKey,
        StaticPrivateKey, StaticPublicKey,
    },
    error::Ssu2Error,
    primitives::RouterId,
    runtime::{Runtime, UdpSocket},
    subsystem::SubsystemEvent,
    transport::{
        ssu2::{
            message::{
                handshake::{SessionConfirmedBuilder, SessionRequestBuilder, TokenRequestBuilder},
                Block, HeaderKind, HeaderReader,
            },
            session::{
                active::Ssu2SessionContext,
                pending::{
                    EncryptionContext, MlKemContext, PacketKind, PacketRetransmitter,
                    PacketRetransmitterEvent, PendingSsu2SessionStatus,
                },
                KeyContext,
            },
            Packet,
        },
        EncryptionKind, TerminationReason,
    },
};

use bytes::Bytes;
use futures::FutureExt;
use ml_kem::{kem::Kem, Decapsulate, KeyExport, MlKem512, MlKem768};
use thingbuf::mpsc::{Receiver, Sender};
use zeroize::Zeroize;

use alloc::{boxed::Box, collections::VecDeque, vec::Vec};
use core::{
    fmt,
    future::Future,
    mem,
    net::SocketAddr,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

/// Logging target for the file.
const LOG_TARGET: &str = "emissary::ssu2::pending::outbound";

/// Outbound SSU2 session context.
pub struct OutboundSsu2Context<R: Runtime> {
    /// Socket address of the remote router.
    pub address: SocketAddr,

    /// Destination connection ID.
    pub dst_id: u64,

    /// Encryption context.
    pub encryption_ctx: EncryptionContext,

    /// Local router intro key.
    pub local_intro_key: [u8; 32],

    /// Local static key.
    pub local_static_key: StaticPrivateKey,

    /// Maximum payload size.
    pub max_payload_size: usize,

    /// Network ID.
    pub net_id: u8,

    /// Remote router intro key.
    pub remote_intro_key: [u8; 32],

    /// Should relay tag be requested.
    pub request_tag: bool,

    /// ID of the remote router.
    pub router_id: RouterId,

    /// Serialized local router info.
    pub router_info: Bytes,

    /// RX channel for receiving datagrams from `Ssu2Socket`.
    pub rx: Receiver<Packet>,

    /// UDP socket.
    pub socket: R::UdpSocket,

    /// Source connection ID.
    pub src_id: u64,

    /// Remote router's static key.
    pub static_key: StaticPublicKey,

    /// TX channel for communicating with `SubsystemManager`.
    pub transport_tx: Sender<SubsystemEvent>,

    /// Verifying key of remote router.
    pub verifying_key: SigningPublicKey,
}

/// State for a pending outbound SSU2 session.
enum PendingSessionState {
    /// Send `SessionRequest` directly.
    SendSessionRequest {
        /// Local static key.
        local_static_key: StaticPrivateKey,

        /// Serialized local router info.
        router_info: Bytes,

        /// Remote router's static key.
        static_key: StaticPublicKey,

        /// Token received in `HolePunch`/`RelayResponse`.
        token: u64,
    },

    /// Awaiting `Retry` from remote router.
    AwaitingRetry {
        /// Local static key.
        local_static_key: StaticPrivateKey,

        /// Serialized local router info.
        router_info: Bytes,

        /// Remote router's static key.
        static_key: StaticPublicKey,
    },

    /// Awaiting `SessionCreated` message from remote router.
    AwaitingSessionCreated {
        /// Local ephemeral key.
        ephemeral_key: EphemeralPrivateKey,

        /// Local static key.
        local_static_key: StaticPrivateKey,

        /// ML-KEM contxt.
        ///
        /// `None` for x25519.
        ml_kem_context: Box<Option<MlKemContext>>,

        /// Serialized local router info.
        router_info: Bytes,
    },

    /// Awaiting first ACK to be received.
    AwaitingFirstAck {
        /// Relay tag, if we requested and received on.
        relay_tag: Option<u32>,
    },

    /// State has been poisoned.
    Poisoned,
}

impl fmt::Debug for PendingSessionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PendingSessionState::AwaitingRetry { .. } =>
                f.debug_struct("PendingSessionState::AwaitingRetry").finish_non_exhaustive(),
            PendingSessionState::SendSessionRequest { token, .. } => f
                .debug_struct("PendingSessionState::SendSessionRequest")
                .field("token", &token)
                .finish_non_exhaustive(),
            PendingSessionState::AwaitingSessionCreated { .. } => f
                .debug_struct("PendingSessionState::AwaitingSessionCreated")
                .finish_non_exhaustive(),
            PendingSessionState::AwaitingFirstAck { relay_tag } => f
                .debug_struct("PendingSessionState::AwaitingFirstAck")
                .field("relay_tag", &relay_tag)
                .finish(),
            PendingSessionState::Poisoned =>
                f.debug_struct("PendingSessionState::Poisoned").finish(),
        }
    }
}

/// Pending outbound SSU2 session.
pub struct OutboundSsu2Session<R: Runtime> {
    /// Socket address of the remote router.
    address: SocketAddr,

    /// Destination connection ID.
    dst_id: u64,

    /// Encryption context.
    encryption_ctx: EncryptionContext,

    /// Our external address.
    ///
    /// Received in a `Retry` message.
    external_address: Option<SocketAddr>,

    /// Local router intro key.
    local_intro_key: [u8; 32],

    /// Maximum payload size size.
    max_payload_size: usize,

    /// Network ID.
    net_id: u8,

    /// Packet retransmitter.
    pkt_retransmitter: PacketRetransmitter<R>,

    /// Remote router intro key.
    remote_intro_key: [u8; 32],

    /// Shoudl relay tag be requested in `SessionRequest`.
    request_tag: bool,

    /// ID of the remote router.
    router_id: RouterId,

    /// RX channel for receiving datagrams from `Ssu2Socket`.
    rx: Option<Receiver<Packet>>,

    /// UDP socket.
    socket: R::UdpSocket,

    /// Source connection ID.
    src_id: u64,

    /// When was the handshake started.
    started: R::Instant,

    /// Pending session state.
    state: PendingSessionState,

    /// TX channel for communicating with `SubsystemManager`.
    transport_tx: Sender<SubsystemEvent>,

    /// Verifying key of remote router.
    verifying_key: SigningPublicKey,

    /// Write buffer.
    write_buffer: VecDeque<Vec<u8>>,
}

impl<R: Runtime> OutboundSsu2Session<R> {
    /// Create new [`OutboundSsu2Session`].
    pub fn new(context: OutboundSsu2Context<R>) -> Self {
        let OutboundSsu2Context {
            address,
            dst_id,
            encryption_ctx,
            local_intro_key,
            local_static_key,
            max_payload_size,
            net_id,
            remote_intro_key,
            request_tag,
            router_id,
            router_info,
            rx,
            socket,
            src_id,
            static_key,
            transport_tx,
            verifying_key,
        } = context;

        tracing::trace!(
            target: LOG_TARGET,
            %router_id,
            ?dst_id,
            ?src_id,
            kind = %encryption_ctx,
            "send TokenRequest",
        );

        let pkt = TokenRequestBuilder::default()
            .with_dst_id(dst_id)
            .with_src_id(src_id)
            .with_intro_key(remote_intro_key)
            .with_net_id(net_id)
            .with_version(encryption_ctx.version())
            .build::<R>()
            .to_vec();

        Self {
            address,
            dst_id,
            encryption_ctx,
            external_address: None,
            local_intro_key,
            max_payload_size,
            net_id,
            pkt_retransmitter: PacketRetransmitter::token_request(pkt.clone()),
            remote_intro_key,
            request_tag,
            router_id,
            rx: Some(rx),
            verifying_key,
            socket,
            src_id,
            started: R::now(),
            state: PendingSessionState::AwaitingRetry {
                local_static_key,
                router_info,
                static_key,
            },
            transport_tx,
            write_buffer: VecDeque::from([pkt]),
        }
    }

    /// Create new [`OutboundSsu2Session`] from a token received from a successful relay process.
    pub fn from_token(context: OutboundSsu2Context<R>, token: u64) -> Self {
        let OutboundSsu2Context {
            address,
            dst_id,
            encryption_ctx,
            local_intro_key,
            local_static_key,
            max_payload_size,
            net_id,
            remote_intro_key,
            router_id,
            router_info,
            rx,
            socket,
            src_id,
            static_key,
            transport_tx,
            verifying_key,
            ..
        } = context;

        tracing::trace!(
            target: LOG_TARGET,
            %router_id,
            ?dst_id,
            ?src_id,
            ?token,
            ?address,
            "send SessionRequest",
        );

        Self {
            address,
            dst_id,
            encryption_ctx,
            external_address: None,
            local_intro_key,
            max_payload_size,
            net_id,
            pkt_retransmitter: PacketRetransmitter::inactive(Duration::from_secs(0)),
            remote_intro_key,
            request_tag: false, // don't request tag from a router that requires introduction
            router_id,
            rx: Some(rx),
            verifying_key,
            socket,
            src_id,
            started: R::now(),
            state: PendingSessionState::SendSessionRequest {
                token,
                local_static_key,
                router_info,
                static_key,
            },
            transport_tx,
            write_buffer: VecDeque::new(),
        }
    }

    /// Send `SessionRequest` with `token`.
    fn send_session_request(
        &mut self,
        local_static_key: StaticPrivateKey,
        router_info: Bytes,
        static_key: StaticPublicKey,
        token: u64,
    ) {
        // MixKey(DH()), es
        let ephemeral_key = EphemeralPrivateKey::random(R::rng());
        let cipher_key = self.encryption_ctx.noise_ctx().mix_key(&ephemeral_key, &static_key);

        // add remote router ID to `h` for spoof protection
        //
        // https://i2p.net/en/docs/specs/ssu2-hybrid/#sessionrequest-type-0
        match &mut self.encryption_ctx {
            EncryptionContext::X25519(_) => {}
            EncryptionContext::MlKem512X25519(noise_ctx)
            | EncryptionContext::MlKem768X25519(noise_ctx) => {
                noise_ctx.mix_hash(self.router_id.to_vec());
            }
            EncryptionContext::MlKem1024X25519(_) => unreachable!(),
        }

        // e1
        //
        // https://i2p.net/en/docs/specs/ssu2-hybrid/#alice-kdf-for-message-1
        let (encap_key, ml_kem_context) = match self.encryption_ctx {
            EncryptionContext::X25519(_) => (None, None),
            EncryptionContext::MlKem512X25519(_) => {
                let (decap_key, encap_key) = MlKem512::generate_keypair_from_rng(&mut R::rng());

                (
                    Some(encap_key.to_bytes().to_vec()),
                    Some(MlKemContext::MlKem512X25519(Box::new(decap_key))),
                )
            }
            EncryptionContext::MlKem768X25519(_) => {
                let (decap_key, encap_key) = MlKem768::generate_keypair_from_rng(&mut R::rng());

                (
                    Some(encap_key.to_bytes().to_vec()),
                    Some(MlKemContext::MlKem768X25519(Box::new(decap_key))),
                )
            }
            EncryptionContext::MlKem1024X25519(_) => unreachable!(),
        };

        let mut message = SessionRequestBuilder::default()
            .with_dst_id(self.dst_id)
            .with_src_id(self.src_id)
            .with_net_id(self.net_id)
            .with_max_payload_size(self.max_payload_size)
            .with_encapsulation_key(encap_key)
            .with_version(self.encryption_ctx.version())
            .with_ephemeral_key(ephemeral_key.public())
            .with_relay_tag_request(self.request_tag)
            .with_token(token)
            .build::<R>();

        // MixHash(header), MixHash(aepk)
        self.encryption_ctx
            .noise_ctx()
            .mix_hash(message.header())
            .mix_hash(ephemeral_key.public());

        // encrypt payload
        //
        // also encrypts encapsulation key if it was specified
        message.encrypt_payload(&cipher_key, self.encryption_ctx.noise_ctx());
        message.encrypt_header(self.remote_intro_key, self.remote_intro_key);

        // MixHash(ciphertext)
        self.encryption_ctx.noise_ctx().mix_hash(message.payload());

        // reset packet retransmitter to track `SessionRequest` and send the message to remote
        let pkt = message.build().to_vec();
        self.pkt_retransmitter = PacketRetransmitter::session_request(pkt.clone());
        self.write_buffer.push_back(pkt);

        self.state = PendingSessionState::AwaitingSessionCreated {
            ephemeral_key,
            local_static_key,
            ml_kem_context: Box::new(ml_kem_context),
            router_info,
        };
    }

    /// Handle `Retry`.
    ///
    /// Attempt to parse the header into `Retry` and if it succeeds, send a `SessionRequest` to
    /// remote using the token that was received in the `Retry` message. The state of the outbound
    /// connection proceeds to `AwaitingSessionCreated` which is handled by
    /// [`OutboundSsu2Session::on_session_created()`].
    ///
    /// <https://geti2p.net/spec/ssu2#kdf-for-retry>
    /// <https://geti2p.net/spec/ssu2#retry-type-9>
    fn on_retry(
        &mut self,
        mut pkt: Vec<u8>,
        local_static_key: StaticPrivateKey,
        router_info: Bytes,
        static_key: StaticPublicKey,
    ) -> Result<Option<PendingSsu2SessionStatus<R>>, Ssu2Error> {
        let (pkt_num, token) = match HeaderReader::new(self.remote_intro_key, &mut pkt)?
            .parse(self.remote_intro_key)?
        {
            HeaderKind::Retry {
                net_id,
                pkt_num,
                token,
                ..
            } => {
                if self.net_id != net_id {
                    return Err(Ssu2Error::NetworkMismatch);
                }

                (pkt_num, token)
            }
            kind => {
                tracing::warn!(
                    target: LOG_TARGET,
                    router_id = %self.router_id,
                    dst_id = ?self.dst_id,
                    src_id = ?self.src_id,
                    ?kind,
                    "unexpected message, expected Retry",
                );
                return Err(Ssu2Error::UnexpectedMessage);
            }
        };

        tracing::trace!(
            target: LOG_TARGET,
            router_id = %self.router_id,
            dst_id = ?self.dst_id,
            src_id = ?self.src_id,
            ?pkt_num,
            ?token,
            "handle Retry",
        );

        let mut payload = pkt[32..].to_vec();
        ChaChaPoly::with_nonce(&self.remote_intro_key, pkt_num as u64)
            .decrypt_with_ad(&pkt[..32], &mut payload)?;

        // check if the message contains a termination block
        let blocks = Block::parse::<R>(&payload).map_err(|error| {
            tracing::trace!(
                target: LOG_TARGET,
                router_id = %self.router_id,
                dst_id = ?self.dst_id,
                src_id = ?self.src_id,
                ?error,
                "failed to parse message blocks of Retry",
            );
            Ssu2Error::Malformed
        })?;

        // check if remote sent termination block in `Retry` and if so, exit early
        if let Some(Block::Termination { reason, .. }) =
            blocks.iter().find(|block| core::matches!(block, Block::Termination { .. }))
        {
            tracing::debug!(
                target: LOG_TARGET,
                router_id = %self.router_id,
                dst_id = ?self.dst_id,
                src_id = ?self.src_id,
                ?reason,
                "Retry contains a termination block",
            );

            return Err(Ssu2Error::SessionTerminated(TerminationReason::ssu2(
                *reason,
            )));
        }

        match blocks.iter().find(|block| core::matches!(block, Block::Address { .. })) {
            Some(Block::Address { address }) => {
                tracing::debug!(
                    target: LOG_TARGET,
                    router_id = %self.router_id,
                    dst_id = ?self.dst_id,
                    src_id = ?self.src_id,
                    ?address,
                    "external address discovered",
                );

                self.external_address = Some(*address);
            }
            Some(_) => {}
            None => tracing::warn!(
                router_id = %self.router_id,
                dst_id = ?self.dst_id,
                src_id = ?self.src_id,
                "Retry does not contain an address block",
            ),
        }

        // send session request
        //
        // state transitions to `AwaitingSessionCreated`
        self.send_session_request(local_static_key, router_info, static_key, token);

        Ok(None)
    }

    /// Handle `SessionCreated`.
    ///
    /// Attempt to parse the header into `SessionCrated` and if it succeeds, send a
    /// `SessionConfirmed` to remote. The state of the outbound connection proceeds to
    /// `AwaitingFirstAck` which is handled by [`OutboundSsu2Session::on_data()`]. Once an ACK for
    /// the `SessionConfirmed` message has been received, data phase keys are derived and the
    /// session is considered established
    ///
    /// <https://geti2p.net/spec/ssu2#kdf-for-session-created-and-session-confirmed-part-1>
    /// <https://geti2p.net/spec/ssu2#sessioncreated-type-1>
    fn on_session_created(
        &mut self,
        mut pkt: Vec<u8>,
        ephemeral_key: EphemeralPrivateKey,
        local_static_key: StaticPrivateKey,
        ml_kem_context: Box<Option<MlKemContext>>,
        router_info: Bytes,
    ) -> Result<Option<PendingSsu2SessionStatus<R>>, Ssu2Error> {
        let temp_key =
            Hmac::new(self.encryption_ctx.noise_ctx().chaining_key()).update([]).finalize();
        let k_header_2 =
            Hmac::new(&temp_key).update(b"SessCreateHeader").update([0x01]).finalize_new();

        let remote_ephemeral_key =
            match HeaderReader::new(self.remote_intro_key, &mut pkt)?.parse(k_header_2)? {
                HeaderKind::SessionCreated {
                    ephemeral_key,
                    net_id,
                    ..
                } => {
                    if self.net_id != net_id {
                        return Err(Ssu2Error::NetworkMismatch);
                    }

                    ephemeral_key
                }
                kind => {
                    tracing::debug!(
                        target: LOG_TARGET,
                        router_id = %self.router_id,
                        dst_id = ?self.dst_id,
                        src_id = ?self.src_id,
                        ?kind,
                        "unexpected message, expected SessionCreated",
                    );

                    self.state = PendingSessionState::AwaitingSessionCreated {
                        ephemeral_key,
                        local_static_key,
                        ml_kem_context,
                        router_info,
                    };
                    return Ok(None);
                }
            };

        tracing::trace!(
            target: LOG_TARGET,
            router_id = %self.router_id,
            dst_id = ?self.dst_id,
            src_id = ?self.src_id,
            "handle SessionCreated",
        );

        // MixHash(header), MixHash(bepk)
        self.encryption_ctx.noise_ctx().mix_hash(&pkt[..32]).mix_hash(&pkt[32..64]);

        // MixKey(DH()), ee
        let cipher_key =
            self.encryption_ctx.noise_ctx().mix_key(&ephemeral_key, &remote_ephemeral_key);

        // ekem1
        //
        // https://i2p.net/en/docs/specs/ssu2-hybrid/#alice-kdf-for-message-2
        let (cipher_key, offset) = match &mut self.encryption_ctx {
            EncryptionContext::X25519(_) => (cipher_key, 64usize),
            kind => {
                let kem_ciphertext_size =
                    kind.kem_ciphertext_size() + constants::crypto::POLY1305_MAC_SIZE;

                if pkt.len() <= 64 + kem_ciphertext_size {
                    tracing::warn!(
                        target: LOG_TARGET,
                        router_id = %self.router_id,
                        dst_id = ?self.dst_id,
                        src_id = ?self.src_id,
                        %kind,
                        size = ?pkt.len(),
                        expected = ?kem_ciphertext_size,
                        "SessionCreated is too short for ml-kem",
                    );
                    return Err(Ssu2Error::NotEnoughBytes);
                }

                let mut kem_ciphertext = pkt[64..64 + kem_ciphertext_size].to_vec();
                ChaChaPoly::new(&cipher_key)
                    .decrypt_with_ad(kind.noise_ctx().state(), &mut kem_ciphertext)?;

                // MixHash(kem_ciphertext_section)
                kind.noise_ctx().mix_hash(&pkt[64..64 + kem_ciphertext_size]);

                // MixKey(kem_shared_key)
                //
                // decapsulation key must exist since `encryption_ctx` indicates ml-kem
                let keydata = match ml_kem_context.expect("to exist") {
                    MlKemContext::MlKem512X25519(decap_key) => {
                        let shared = decap_key
                            .decapsulate_slice(&kem_ciphertext)
                            .map_err(|_| Ssu2Error::Malformed)?
                            .to_vec();
                        kind.noise_ctx().mix_key_from_shared_secret(&shared)
                    }
                    MlKemContext::MlKem768X25519(decap_key) => {
                        let shared = decap_key
                            .decapsulate_slice(&kem_ciphertext)
                            .map_err(|_| Ssu2Error::Malformed)?
                            .to_vec();
                        kind.noise_ctx().mix_key_from_shared_secret(&shared)
                    }
                };

                (keydata, kem_ciphertext_size + 64)
            }
        };

        if pkt.len() <= offset {
            tracing::warn!(
                target: LOG_TARGET,
                router_id = %self.router_id,
                dst_id = ?self.dst_id,
                src_id = ?self.src_id,
                kind = %self.encryption_ctx,
                size = ?pkt.len(),
                expected = ?offset,
                "SessionCreated is too small",
            );
            return Err(Ssu2Error::NotEnoughBytes);
        }

        // decrypt payload
        let mut payload = pkt[offset..].to_vec();
        ChaChaPoly::with_nonce(&cipher_key, 0u64)
            .decrypt_with_ad(self.encryption_ctx.noise_ctx().state(), &mut payload)?;

        let blocks = Block::parse::<R>(&payload).map_err(|_| Ssu2Error::Malformed)?;
        let relay_tag = blocks.iter().find_map(|block| match block {
            Block::RelayTag { relay_tag } => self.request_tag.then_some(*relay_tag),
            _ => None,
        });

        // MixHash(ciphertext)
        self.encryption_ctx.noise_ctx().mix_hash(&pkt[offset..]);

        let temp_key =
            Hmac::new(self.encryption_ctx.noise_ctx().chaining_key()).update([]).finalize();
        let k_header_2 =
            Hmac::new(&temp_key).update(b"SessionConfirmed").update([0x01]).finalize_new();

        let mut message = SessionConfirmedBuilder::default()
            .with_max_payload_size(self.max_payload_size)
            .with_dst_id(self.dst_id)
            .with_src_id(self.src_id)
            .with_static_key(local_static_key.public())
            .with_router_info(router_info)
            .build::<R>();

        // MixHash(header) & encrypt public key
        self.encryption_ctx.noise_ctx().mix_hash(message.header());
        message.encrypt_public_key(&cipher_key, 1u64, self.encryption_ctx.noise_ctx().state());

        // MixHash(apk)
        self.encryption_ctx.noise_ctx().mix_hash(message.public_key());

        // MixKey(DH())
        let mut cipher_key = self
            .encryption_ctx
            .noise_ctx()
            .mix_key(&local_static_key, &remote_ephemeral_key);

        message.encrypt_payload(&cipher_key, 0u64, self.encryption_ctx.noise_ctx().state());
        cipher_key.zeroize();

        // reset packet retransmitter to track `SessionConfirmed` and send the message to remote
        let pkts = message.build(self.remote_intro_key, k_header_2);
        self.pkt_retransmitter = PacketRetransmitter::session_confirmed(pkts.clone());
        self.write_buffer.extend(pkts);

        self.state = PendingSessionState::AwaitingFirstAck { relay_tag };
        Ok(None)
    }

    /// Handle `Data`, in other words an ACK for the `SessionConfirmed` message.
    ///
    /// Verify that a valid message was received, derive data phase keys and return session context
    /// for [`Ssu2Socket`] which starts an active session.
    ///
    /// <https://geti2p.net/spec/ssu2#kdf-for-session-confirmed-part-1-using-session-created-kdf>
    /// <https://geti2p.net/spec/ssu2#kdf-for-session-confirmed-part-2>
    /// <https://geti2p.net/spec/ssu2#sessionconfirmed-type-2>
    /// <https://geti2p.net/spec/ssu2#kdf-for-data-phase>
    fn on_data(
        &mut self,
        mut pkt: Vec<u8>,
        relay_tag: Option<u32>,
    ) -> Result<Option<PendingSsu2SessionStatus<R>>, Ssu2Error> {
        let temp_key =
            Hmac::new(self.encryption_ctx.noise_ctx().chaining_key()).update([]).finalize();
        let k_ab = Hmac::new(&temp_key).update([0x01]).finalize();
        let k_ba = Hmac::new(&temp_key).update(&k_ab).update([0x02]).finalize();

        let temp_key = Hmac::new(&k_ab).update([]).finalize();
        let k_data_ab =
            Hmac::new(&temp_key).update(b"HKDFSSU2DataKeys").update([0x01]).finalize_new();
        let k_header_2_ab = TryInto::<[u8; 32]>::try_into(
            Hmac::new(&temp_key)
                .update(k_data_ab)
                .update(b"HKDFSSU2DataKeys")
                .update([0x02])
                .finalize(),
        )
        .expect("to succeed");

        let temp_key = Hmac::new(&k_ba).update([]).finalize();
        let k_data_ba =
            Hmac::new(&temp_key).update(b"HKDFSSU2DataKeys").update([0x01]).finalize_new();
        let k_header_2_ba = Hmac::new(&temp_key)
            .update(k_data_ba)
            .update(b"HKDFSSU2DataKeys")
            .update([0x02])
            .finalize_new();

        let payload = match HeaderReader::new(self.local_intro_key, &mut pkt)?.parse(k_header_2_ba)
        {
            Ok(HeaderKind::Data { pkt_num, .. }) => {
                // ensure the data packet decrypts correctly
                //
                // failure to decrypt the payload could indicate that an incorrect packet was
                // received and that `OutboundSession` should resend `SessionConfirmed`
                //
                // TODO: unneeded clone
                let mut payload = pkt[16..].to_vec();
                if ChaChaPoly::with_nonce(&k_data_ba, pkt_num as u64)
                    .decrypt_with_ad(&pkt[..16], &mut payload)
                    .is_err()
                {
                    tracing::debug!(
                        target: LOG_TARGET,
                        router_id = %self.router_id,
                        dst_id = ?self.dst_id,
                        src_id = ?self.src_id,
                        "failed to decrypt Data message, ignoring",
                    );

                    self.state = PendingSessionState::AwaitingFirstAck { relay_tag };
                    return Ok(None);
                }

                payload
            }
            kind => {
                tracing::debug!(
                    target: LOG_TARGET,
                    router_id = %self.router_id,
                    dst_id = ?self.dst_id,
                    src_id = ?self.src_id,
                    ?kind,
                    "unexpected message, expected Data",
                );

                self.state = PendingSessionState::AwaitingFirstAck { relay_tag };
                return Ok(None);
            }
        };

        tracing::trace!(
            target: LOG_TARGET,
            router_id = %self.router_id,
            dst_id = ?self.dst_id,
            src_id = ?self.src_id,
            "handle Data (first ack)",
        );

        let relay_tag = match Block::parse::<R>(&payload) {
            Ok(blocks) => {
                if let Some(Block::Termination { reason, .. }) =
                    blocks.iter().find(|block| core::matches!(block, Block::Termination { .. }))
                {
                    tracing::debug!(
                        router_id = %self.router_id,
                        dst_id = ?self.dst_id,
                        src_id = ?self.src_id,
                        reason = ?TerminationReason::ssu2(*reason),
                        "first ack contains termination, aborting",
                    );

                    return Err(Ssu2Error::SessionTerminated(TerminationReason::ssu2(
                        *reason,
                    )));
                }

                match (
                    relay_tag,
                    blocks.iter().find(|block| core::matches!(block, Block::RelayTag { .. })),
                ) {
                    (None, Some(Block::RelayTag { relay_tag: tag })) => Some(*tag),
                    (Some(tag), None) => Some(tag),
                    (None, None) => None,
                    (Some(old_tag), Some(Block::RelayTag { relay_tag: new_tag })) => {
                        tracing::debug!(
                            target: LOG_TARGET,
                            router_id = %self.router_id,
                            dst_id = ?self.dst_id,
                            src_id = ?self.src_id,
                            ?old_tag,
                            ?new_tag,
                            "received two relay tags",
                        );

                        Some(*new_tag)
                    }
                    (_, _) => None,
                }
            }
            Err(error) => {
                tracing::debug!(
                    target: LOG_TARGET,
                    router_id = %self.router_id,
                    dst_id = ?self.dst_id,
                    src_id = ?self.src_id,
                    ?error,
                    "failed to parse Data message, ignoring",
                );

                self.state = PendingSessionState::AwaitingFirstAck { relay_tag };
                return Ok(None);
            }
        };

        Ok(Some(PendingSsu2SessionStatus::NewOutboundSession {
            context: Ssu2SessionContext {
                address: self.address,
                dst_id: self.dst_id,
                intro_key: self.remote_intro_key,
                max_payload_size: self.max_payload_size,
                pkt_rx: self.rx.take().expect("to exist"),
                recv_key_ctx: KeyContext::new(k_data_ba, k_header_2_ba),
                router_id: self.router_id.clone(),
                send_key_ctx: KeyContext::new(k_data_ab, k_header_2_ab),
                verifying_key: self.verifying_key.clone(),
            },
            external_address: self.external_address,
            relay_tag: self.request_tag.then_some(relay_tag).flatten(),
            src_id: self.src_id,
            started: self.started,
            encryption: match self.encryption_ctx {
                EncryptionContext::X25519(_) => EncryptionKind::X25519,
                EncryptionContext::MlKem512X25519(_) => EncryptionKind::MlKem512X25519,
                EncryptionContext::MlKem768X25519(_) => EncryptionKind::MlKem768X25519,
                EncryptionContext::MlKem1024X25519(_) => unreachable!(),
            },
        }))
    }

    /// Handle `pkt`.
    ///
    /// If the packet is the next in expected sequnce, the outbound session advances to the next
    /// state and if an ACK for `SessionConfirmed` has been received,
    /// [`PendingSsu2SessionStatus::NewOutboundSession`] is returned to the caller, shutting down
    /// this future and allowing [`Ssu2Socket`] to start a new future for the active session.
    ///
    /// If a fatal error occurs during handling of the packet,
    /// [`PendingSsu2SessionStatus::SessionTerminated`] is returned.
    fn on_packet(
        &mut self,
        pkt: Vec<u8>,
    ) -> Result<Option<PendingSsu2SessionStatus<R>>, Ssu2Error> {
        match mem::replace(&mut self.state, PendingSessionState::Poisoned) {
            PendingSessionState::AwaitingRetry {
                local_static_key,
                router_info,
                static_key,
            } => self.on_retry(pkt, local_static_key, router_info, static_key),
            PendingSessionState::AwaitingSessionCreated {
                ephemeral_key,
                local_static_key,
                ml_kem_context,
                router_info,
            } => self.on_session_created(
                pkt,
                ephemeral_key,
                local_static_key,
                ml_kem_context,
                router_info,
            ),
            PendingSessionState::AwaitingFirstAck { relay_tag } => self.on_data(pkt, relay_tag),
            PendingSessionState::Poisoned | PendingSessionState::SendSessionRequest { .. } => {
                tracing::warn!(
                    target: LOG_TARGET,
                    router_id = %self.router_id,
                    dst_id = ?self.dst_id,
                    src_id = ?self.src_id,
                    "outbound session state is poisoned",
                );
                debug_assert!(false);

                Ok(Some(PendingSsu2SessionStatus::SessionTerminated {
                    address: Some(self.address),
                    connection_id: self.src_id,
                    relay_tag: None,
                    router_id: Some(self.router_id.clone()),
                    started: self.started,
                    reason: TerminationReason::Unspecified,
                }))
            }
        }
    }

    /// Run the event loop of [`OutboundSsu2Session`].
    ///
    /// Convenient function for calling `OutboundSsu2Session::poll()` which, if an error occurred
    /// during negotiation, reports a connection failure to installed subsystems and returns the
    /// session status.
    pub async fn run(mut self) -> PendingSsu2SessionStatus<R> {
        match core::mem::replace(&mut self.state, PendingSessionState::Poisoned) {
            PendingSessionState::SendSessionRequest {
                local_static_key,
                router_info,
                static_key,
                token,
            } => self.send_session_request(local_static_key, router_info, static_key, token),
            state => {
                self.state = state;
            }
        }

        let status = (&mut self).await;

        if core::matches!(
            status,
            PendingSsu2SessionStatus::SessionTerminated { .. }
                | PendingSsu2SessionStatus::Timeout { .. }
                | PendingSsu2SessionStatus::SocketClosed { .. }
        ) {
            if let Err(error) = self
                .transport_tx
                .send(SubsystemEvent::ConnectionFailure {
                    router_id: self.router_id.clone(),
                })
                .await
            {
                tracing::warn!(
                    target: LOG_TARGET,
                    router_id = %self.router_id,
                    ?error,
                    "failed to report connection failure to subsystem manager",
                );
            }
        }

        status
    }
}

impl<R: Runtime> Future for OutboundSsu2Session<R> {
    type Output = PendingSsu2SessionStatus<R>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        loop {
            let pkt = match &mut self.rx {
                None =>
                    return Poll::Ready(PendingSsu2SessionStatus::SocketClosed {
                        started: self.started,
                    }),
                Some(rx) => match rx.poll_recv(cx) {
                    Poll::Pending => break,
                    Poll::Ready(None) =>
                        return Poll::Ready(PendingSsu2SessionStatus::SocketClosed {
                            started: self.started,
                        }),
                    Poll::Ready(Some(Packet { pkt, .. })) => pkt,
                },
            };

            match self.on_packet(pkt) {
                Ok(None) => {}
                Ok(Some(status)) => return Poll::Ready(status),
                Err(error) => {
                    tracing::debug!(
                        target: LOG_TARGET,
                        router_id = %self.router_id,
                        dst_id = ?self.dst_id,
                        src_id = ?self.src_id,
                        ?error,
                        "failed to handle packet",
                    );

                    return Poll::Ready(PendingSsu2SessionStatus::SessionTerminated {
                        address: Some(self.address),
                        connection_id: self.src_id,
                        relay_tag: None,
                        router_id: Some(self.router_id.clone()),
                        started: self.started,
                        reason: TerminationReason::from(error),
                    });
                }
            }
        }

        match self.pkt_retransmitter.poll_unpin(cx) {
            Poll::Pending => {}
            Poll::Ready(PacketRetransmitterEvent::Retransmit { pkt }) => {
                tracing::trace!(
                    target: LOG_TARGET,
                    dst_id = ?self.dst_id,
                    src_id = ?self.src_id,
                    state = ?self.state,
                    "retransmitting packet",
                );

                match pkt {
                    PacketKind::Single(pkt) => self.write_buffer.push_back(pkt),
                    PacketKind::Multi(pkts) => self.write_buffer.extend(pkts),
                }
            }
            Poll::Ready(PacketRetransmitterEvent::Timeout) =>
                return Poll::Ready(PendingSsu2SessionStatus::Timeout {
                    connection_id: self.src_id,
                    router_id: Some(self.router_id.clone()),
                    started: self.started,
                    address: Some(self.address),
                }),
        }

        loop {
            let Some(pkt) = self.write_buffer.pop_front() else {
                return Poll::Pending;
            };

            let address = self.address;
            match Pin::new(&mut self.socket).poll_send_to(cx, &pkt, address) {
                Poll::Pending => {
                    self.write_buffer.push_front(pkt);
                    return Poll::Pending;
                }
                Poll::Ready(None) =>
                    return Poll::Ready(PendingSsu2SessionStatus::SocketClosed {
                        started: self.started,
                    }),
                Poll::Ready(Some(_)) => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        crypto::{noise::NoiseContext, sha256::Sha256},
        primitives::RouterInfoBuilder,
        runtime::mock::MockRuntime,
        transport::ssu2::session::pending::inbound::{InboundSsu2Context, InboundSsu2Session},
    };
    use rand::Rng;
    use std::{net::Ipv4Addr, time::Duration};
    use thingbuf::mpsc::channel;

    struct InboundContext {
        inbound_session: InboundSsu2Session<MockRuntime>,
        inbound_session_tx: Sender<Packet>,
        inbound_socket_rx: Receiver<Packet>,
    }

    struct OutboundContext {
        outbound_session: OutboundSsu2Session<MockRuntime>,
        outbound_session_tx: Sender<Packet>,
        outbound_socket_rx: Receiver<Packet>,
        transport_rx: Receiver<SubsystemEvent>,
    }

    async fn create_session(ml_kem: Option<usize>) -> (InboundContext, OutboundContext) {
        let src_id = MockRuntime::rng().next_u64();
        let dst_id = MockRuntime::rng().next_u64();

        let (mut inbound_socket, inbound_address) = {
            let socket = <MockRuntime as Runtime>::UdpSocket::bind("127.0.0.1:0".parse().unwrap())
                .await
                .unwrap();
            let address = socket.local_address().unwrap();

            (socket, address)
        };
        let (mut outbound_socket, outbound_address) = {
            let socket = <MockRuntime as Runtime>::UdpSocket::bind("127.0.0.1:0".parse().unwrap())
                .await
                .unwrap();
            let address = socket.local_address().unwrap();

            (socket, address)
        };

        let outbound_static_key = StaticPrivateKey::random(MockRuntime::rng());
        let outbound_intro_key = {
            let mut key = [0u8; 32];
            MockRuntime::rng().fill_bytes(&mut key);

            key
        };
        let inbound_static_key = StaticPrivateKey::random(MockRuntime::rng());
        let inbound_intro_key = {
            let mut key = [0u8; 32];
            MockRuntime::rng().fill_bytes(&mut key);

            key
        };

        let (inbound_socket_tx, inbound_socket_rx) = channel(128);
        let (inbound_session_tx, inbound_session_rx) = channel(128);
        let (outbound_socket_tx, outbound_socket_rx) = channel(128);
        let (outbound_session_tx, outbound_session_rx) = channel(128);
        let (transport_tx, transport_rx) = channel(128);

        let (router_info, _, signing_key) = RouterInfoBuilder::default()
            .with_ssu2(crate::Ssu2Config {
                disable_pq: false,
                port: 8889,
                ipv4_host: Some(Ipv4Addr::new(127, 0, 0, 1)),
                ipv6_host: None,
                ipv4: true,
                ipv6: false,
                publish: true,
                static_key: TryInto::<[u8; 32]>::try_into(outbound_static_key.as_ref().to_vec())
                    .unwrap(),
                intro_key: outbound_intro_key,
                ipv4_mtu: None,
                ipv6_mtu: None,
                ml_kem: None,
            })
            .build();

        let (chaining_key, inbound_state, encryption_ctx) = {
            let state = Sha256::new()
                .update(match ml_kem {
                    None => "Noise_XKchaobfse+hs1+hs2+hs3_25519_ChaChaPoly_SHA256".as_bytes(),
                    Some(3) => "Noise_XKhfschaobfse+hs1+hs2+hs3_25519+MLKEM512_ChaChaPoly_SHA256"
                        .as_bytes(),
                    Some(4) => "Noise_XKhfschaobfse+hs1+hs2+hs3_25519+MLKEM768_ChaChaPoly_SHA256"
                        .as_bytes(),
                    _ => unreachable!(),
                })
                .finalize_new();

            let chaining_key = state;
            let outbound_state = Sha256::new().update(&state).finalize();
            let inbound_state = Sha256::new()
                .update(&outbound_state)
                .update(inbound_static_key.public().to_vec())
                .finalize_new();

            (
                chaining_key,
                inbound_state,
                match ml_kem {
                    None => EncryptionContext::X25519(NoiseContext::new(
                        TryInto::<[u8; 32]>::try_into(chaining_key.clone()).unwrap(),
                        TryInto::<[u8; 32]>::try_into(inbound_state.clone()).unwrap(),
                    )),
                    Some(3) => {
                        let inbound_state = Sha256::new()
                            .update(inbound_state)
                            .update(router_info.identity.id().to_vec())
                            .finalize();

                        EncryptionContext::MlKem512X25519(NoiseContext::new(
                            TryInto::<[u8; 32]>::try_into(chaining_key.clone()).unwrap(),
                            TryInto::<[u8; 32]>::try_into(inbound_state.clone()).unwrap(),
                        ))
                    }
                    Some(4) => {
                        let inbound_state = Sha256::new()
                            .update(inbound_state)
                            .update(router_info.identity.id().to_vec())
                            .finalize();

                        EncryptionContext::MlKem768X25519(NoiseContext::new(
                            TryInto::<[u8; 32]>::try_into(chaining_key.clone()).unwrap(),
                            TryInto::<[u8; 32]>::try_into(inbound_state.clone()).unwrap(),
                        ))
                    }
                    _ => unreachable!(),
                },
            )
        };

        let mut outbound = OutboundSsu2Session::new(OutboundSsu2Context {
            address: inbound_address,
            encryption_ctx: match ml_kem {
                None => EncryptionContext::X25519(NoiseContext::new(
                    TryInto::<[u8; 32]>::try_into(chaining_key.clone()).unwrap(),
                    TryInto::<[u8; 32]>::try_into(inbound_state.clone()).unwrap(),
                )),
                Some(3) => EncryptionContext::MlKem512X25519(NoiseContext::new(
                    TryInto::<[u8; 32]>::try_into(chaining_key.clone()).unwrap(),
                    TryInto::<[u8; 32]>::try_into(inbound_state.clone()).unwrap(),
                )),
                Some(4) => EncryptionContext::MlKem768X25519(NoiseContext::new(
                    TryInto::<[u8; 32]>::try_into(chaining_key.clone()).unwrap(),
                    TryInto::<[u8; 32]>::try_into(inbound_state.clone()).unwrap(),
                )),
                _ => unreachable!(),
            },
            dst_id,
            local_intro_key: outbound_intro_key,
            local_static_key: outbound_static_key,
            max_payload_size: 1500 - 28,
            net_id: 2u8,
            remote_intro_key: inbound_intro_key,
            router_id: router_info.identity.id(),
            router_info: Bytes::from(router_info.serialize(&signing_key)),
            rx: outbound_session_rx,
            verifying_key: signing_key.public(),
            socket: outbound_socket.clone(),
            src_id,
            static_key: inbound_static_key.public(),
            transport_tx,
            request_tag: false,
        });

        // read `Retry` from inbound socket and relay it to `outbound_socket_rx`
        let mut buffer = vec![0u8; 0xffff];

        tokio::select! {
            _ = &mut outbound => panic!("outbound sessionr returned"),
            event = inbound_socket.recv_from(&mut buffer) => {
                let (nread, from) = event.unwrap();
                outbound_socket_tx
                    .send(Packet {
                        pkt: buffer[..nread].to_vec(),
                        address: from,
                    })
                    .await
                    .unwrap();
            }
        }

        let (pkt, pkt_num, dst_id, src_id) = {
            let Packet { mut pkt, .. } = outbound_socket_rx.try_recv().unwrap();
            let mut reader = HeaderReader::new(inbound_intro_key, &mut pkt).unwrap();
            let dst_id = reader.dst_id();

            match reader.parse(inbound_intro_key) {
                Ok(HeaderKind::TokenRequest {
                    pkt_num, src_id, ..
                }) => (pkt, pkt_num, dst_id, src_id),
                _ => panic!("invalid message"),
            }
        };

        let inbound = InboundSsu2Session::<MockRuntime>::new(InboundSsu2Context {
            address: outbound_address,
            encryption_ctx,
            dst_id,
            intro_key: inbound_intro_key,
            mtu: 1500,
            net_id: 2u8,
            pkt,
            pkt_num,
            socket: inbound_socket.clone(),
            max_payload_size: 1472,
            relay_tag: 1337,
            rx: inbound_session_rx,
            src_id,
            static_key: inbound_static_key.clone(),
        })
        .unwrap();

        tokio::spawn(async move {
            let mut buffer = vec![0u8; 0xffff];

            loop {
                let (nread, from) = inbound_socket.recv_from(&mut buffer).await.unwrap();
                inbound_socket_tx
                    .send(Packet {
                        pkt: buffer[..nread].to_vec(),
                        address: from,
                    })
                    .await
                    .unwrap();
            }
        });

        tokio::spawn(async move {
            let mut buffer = vec![0u8; 0xffff];

            loop {
                let (nread, from) = outbound_socket.recv_from(&mut buffer).await.unwrap();
                outbound_socket_tx
                    .send(Packet {
                        pkt: buffer[..nread].to_vec(),
                        address: from,
                    })
                    .await
                    .unwrap();
            }
        });
        (
            InboundContext {
                inbound_socket_rx,
                inbound_session_tx,
                inbound_session: inbound,
            },
            OutboundContext {
                transport_rx,
                outbound_session: outbound,
                outbound_session_tx,
                outbound_socket_rx,
            },
        )
    }

    #[tokio::test(start_paused = true)]
    async fn token_request_timeout_x25519() {
        token_request_timeout(None).await;
    }

    #[tokio::test(start_paused = true)]
    async fn token_request_timeout_ml_kem_512() {
        token_request_timeout(Some(3)).await;
    }

    #[tokio::test(start_paused = true)]
    async fn token_request_timeout_ml_kem_768() {
        token_request_timeout(Some(4)).await;
    }

    async fn token_request_timeout(ml_kem: Option<usize>) {
        let (
            InboundContext {
                inbound_session_tx: _inbound_session_tx,
                inbound_socket_rx,
                ..
            },
            OutboundContext {
                outbound_session,
                outbound_session_tx: _ob_sess_tx,
                outbound_socket_rx: _outbound_socket_rx,
                transport_rx,
            },
        ) = create_session(ml_kem).await;
        let router_id = outbound_session.router_id.clone();
        let outbound_session = tokio::spawn(outbound_session.run());

        for _ in 0..2 {
            match tokio::time::timeout(Duration::from_secs(10), inbound_socket_rx.recv()).await {
                Err(_) => panic!("timeout"),
                Ok(None) => panic!("error"),
                Ok(Some(_)) => {}
            }
        }

        match tokio::time::timeout(Duration::from_secs(10), outbound_session).await {
            Err(_) => panic!("timeout"),
            Ok(Err(_)) => panic!("error"),
            Ok(Ok(PendingSsu2SessionStatus::Timeout { .. })) => {}
            _ => panic!("invalid result"),
        }

        match tokio::time::timeout(Duration::from_secs(5), transport_rx.recv())
            .await
            .expect("no timeout")
            .expect("to succeed")
        {
            SubsystemEvent::ConnectionFailure { router_id: router } => {
                assert_eq!(router, router_id)
            }
            _ => panic!("invalid event"),
        }
    }

    #[tokio::test(start_paused = true)]
    async fn session_request_timeout_x25519() {
        session_request_timeout(None).await;
    }

    #[tokio::test(start_paused = true)]
    async fn session_request_timeout_ml_kem_512() {
        session_request_timeout(Some(3)).await;
    }

    #[tokio::test(start_paused = true)]
    async fn session_request_timeout_ml_kem_768() {
        session_request_timeout(Some(4)).await;
    }

    async fn session_request_timeout(ml_kem: Option<usize>) {
        let (
            InboundContext {
                inbound_session,
                inbound_socket_rx,
                inbound_session_tx: _ib_sess_tx,
            },
            OutboundContext {
                outbound_session,
                outbound_session_tx: ob_sess_tx,
                outbound_socket_rx,
                transport_rx,
            },
        ) = create_session(ml_kem).await;

        let intro_key = outbound_session.remote_intro_key;
        let router_id = outbound_session.router_id.clone();
        let outbound_session = tokio::spawn(outbound_session.run());
        let _inbound_session = tokio::spawn(inbound_session.run());

        // send retry message to outbound session
        {
            let Packet { mut pkt, address } = outbound_socket_rx.recv().await.unwrap();
            let mut reader = HeaderReader::new(intro_key, &mut pkt).unwrap();
            let _connection_id = reader.dst_id();

            ob_sess_tx.send(Packet { pkt, address }).await.unwrap();
        }

        for _ in 0..3 {
            match tokio::time::timeout(Duration::from_secs(10), inbound_socket_rx.recv()).await {
                Err(_) => panic!("timeout"),
                Ok(None) => panic!("error"),
                Ok(Some(_)) => {}
            }
        }

        match tokio::time::timeout(Duration::from_secs(20), outbound_session).await {
            Err(_) => panic!("timeout"),
            Ok(Err(_)) => panic!("error"),
            Ok(Ok(PendingSsu2SessionStatus::Timeout { .. })) => {}
            _ => panic!("invalid result"),
        }

        match tokio::time::timeout(Duration::from_secs(5), transport_rx.recv())
            .await
            .expect("no timeout")
            .expect("to succeed")
        {
            SubsystemEvent::ConnectionFailure { router_id: router } => {
                assert_eq!(router, router_id)
            }
            _ => panic!("invalid event"),
        }
    }

    #[tokio::test(start_paused = true)]
    async fn session_confirmed_timeout_x25519() {
        session_confirmed_timeout(None).await;
    }

    #[tokio::test(start_paused = true)]
    async fn session_confirmed_timeout_ml_kem_512() {
        session_confirmed_timeout(Some(3)).await;
    }

    #[tokio::test(start_paused = true)]
    async fn session_confirmed_timeout_ml_kem_768() {
        session_confirmed_timeout(Some(4)).await;
    }

    async fn session_confirmed_timeout(ml_kem: Option<usize>) {
        let (
            InboundContext {
                inbound_session,
                inbound_socket_rx,
                inbound_session_tx: ib_sess_tx,
            },
            OutboundContext {
                outbound_session,
                outbound_session_tx: ob_sess_tx,
                outbound_socket_rx,
                transport_rx,
            },
        ) = create_session(ml_kem).await;

        let intro_key = outbound_session.remote_intro_key;
        let router_id = outbound_session.router_id.clone();
        let outbound_session = tokio::spawn(outbound_session.run());
        let _inbound_session = tokio::spawn(inbound_session.run());

        // send retry message to outbound session
        {
            let Packet { mut pkt, address } = outbound_socket_rx.recv().await.unwrap();
            let mut reader = HeaderReader::new(intro_key, &mut pkt).unwrap();
            let _connection_id = reader.dst_id();

            ob_sess_tx.send(Packet { pkt, address }).await.unwrap();
        }

        // read session request from outbound session and send it to inbound session
        {
            let Packet { mut pkt, address } = inbound_socket_rx.recv().await.unwrap();
            let mut reader = HeaderReader::new(intro_key, &mut pkt).unwrap();
            let _connection_id = reader.dst_id();
            ib_sess_tx.send(Packet { pkt, address }).await.unwrap();
        }

        // send session created to outbound session
        {
            let Packet { mut pkt, address } = outbound_socket_rx.recv().await.unwrap();
            let mut reader = HeaderReader::new(intro_key, &mut pkt).unwrap();
            let _connection_id = reader.dst_id();

            ob_sess_tx.send(Packet { pkt, address }).await.unwrap();
        }

        for _ in 0..3 {
            match tokio::time::timeout(Duration::from_secs(10), inbound_socket_rx.recv()).await {
                Err(_) => panic!("timeout"),
                Ok(None) => panic!("error"),
                Ok(Some(_)) => {}
            }
        }

        match tokio::time::timeout(Duration::from_secs(20), outbound_session).await {
            Err(_) => panic!("timeout"),
            Ok(Err(_)) => panic!("error"),
            Ok(Ok(PendingSsu2SessionStatus::Timeout { .. })) => {}
            _ => panic!("invalid result"),
        }

        match tokio::time::timeout(Duration::from_secs(5), transport_rx.recv())
            .await
            .expect("no timeout")
            .expect("to succeed")
        {
            SubsystemEvent::ConnectionFailure { router_id: router } => {
                assert_eq!(router, router_id)
            }
            _ => panic!("invalid event"),
        }
    }

    #[tokio::test(start_paused = true)]
    async fn duplicate_session_created_received_x25519() {
        duplicate_session_created_received(None).await;
    }

    #[tokio::test(start_paused = true)]
    async fn duplicate_session_created_received_ml_kem_512() {
        duplicate_session_created_received(Some(3)).await;
    }

    #[tokio::test(start_paused = true)]
    async fn duplicate_session_created_received_ml_kem_768() {
        duplicate_session_created_received(Some(4)).await;
    }

    async fn duplicate_session_created_received(ml_kem: Option<usize>) {
        let (
            InboundContext {
                inbound_session,
                inbound_socket_rx,
                inbound_session_tx: ib_sess_tx,
            },
            OutboundContext {
                mut outbound_session,
                outbound_session_tx: ob_sess_tx,
                outbound_socket_rx,
                transport_rx: _transport_rx,
            },
        ) = create_session(ml_kem).await;

        let intro_key = outbound_session.remote_intro_key;
        let outbound_intro_key = outbound_session.local_intro_key;
        let inbound_session = tokio::spawn(inbound_session.run());

        // send retry message to outbound session
        {
            let Packet { mut pkt, address } = outbound_socket_rx.recv().await.unwrap();
            let mut reader = HeaderReader::new(intro_key, &mut pkt).unwrap();
            let _connection_id = reader.dst_id();

            ob_sess_tx.send(Packet { pkt, address }).await.unwrap();
        }

        // read session request from outbound session and send it to inbound session
        {
            let Packet { mut pkt, address } = tokio::select! {
                _ = &mut outbound_session => unreachable!(),
                pkt = inbound_socket_rx.recv() => {
                    pkt.unwrap()
                }
            };

            let mut reader = HeaderReader::new(intro_key, &mut pkt).unwrap();
            let _connection_id = reader.dst_id();
            ib_sess_tx.send(Packet { pkt, address }).await.unwrap();
        }

        // read `SessionCreated` from inbound session twice and relay it to outbound session
        //
        // verify that outbound session handles the duplicate packet gracefully and keeps waiting
        // for the first ack packet
        for _ in 0..2 {
            {
                let Packet { mut pkt, address } = outbound_socket_rx.recv().await.unwrap();
                let mut reader = HeaderReader::new(intro_key, &mut pkt).unwrap();
                let _connection_id = reader.dst_id();
                ob_sess_tx.send(Packet { pkt, address }).await.unwrap();
            }

            // verify that outbound session responds
            let _pkt = tokio::select! {
                _ = &mut outbound_session => unreachable!(),
                pkt = inbound_socket_rx.recv() => {
                    pkt.unwrap()
                }
                _ = tokio::time::sleep(Duration::from_secs(5)) => panic!("timeout"),
            };

            match outbound_session.state {
                PendingSessionState::AwaitingFirstAck { .. } => {}
                _ => panic!("invalid state"),
            }
        }

        // read session created from inbound session and relay it to outbound session
        {
            let Packet { mut pkt, address } = outbound_socket_rx.recv().await.unwrap();
            let mut reader = HeaderReader::new(intro_key, &mut pkt).unwrap();
            let _connection_id = reader.dst_id();
            ob_sess_tx.send(Packet { pkt, address }).await.unwrap();
        }

        // read session confirmed from outbound session and relay it to inbound session
        {
            let Packet { mut pkt, address } = tokio::select! {
                _ = &mut outbound_session => unreachable!(),
                pkt = inbound_socket_rx.recv() => {
                    pkt.unwrap()
                }
                _ = tokio::time::sleep(Duration::from_secs(5)) => panic!("timeout"),
            };

            let mut reader = HeaderReader::new(intro_key, &mut pkt).unwrap();
            let _connection_id = reader.dst_id();
            ib_sess_tx.send(Packet { pkt, address }).await.unwrap();
        }

        // verify that inbound session considers the connection opened
        //
        // relay the first ack packet to outbound session
        match tokio::time::timeout(Duration::from_secs(5), inbound_session)
            .await
            .expect("no timeout")
            .expect("to succeed")
        {
            PendingSsu2SessionStatus::NewInboundSession {
                mut pkt, target, ..
            } => {
                let mut reader = HeaderReader::new(outbound_intro_key, &mut pkt).unwrap();
                let _connection_id = reader.dst_id();
                ob_sess_tx
                    .send(Packet {
                        pkt: pkt.to_vec(),
                        address: target,
                    })
                    .await
                    .unwrap();
            }
            _ => panic!("invalid session state"),
        }

        match tokio::time::timeout(Duration::from_secs(5), outbound_session)
            .await
            .expect("no timeout")
        {
            PendingSsu2SessionStatus::NewOutboundSession { .. } => {}
            _ => panic!("invalid session state"),
        }
    }
}
