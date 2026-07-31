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

//! NTCP2 Noise handshake implementation for initiator (Alice)
//!
//! <https://geti2p.net/spec/ntcp2#overview>
//! <https://i2p.net/en/docs/specs/ntcp2-hybrid/>

use crate::{
    crypto::{
        aes::cbc::Aes, base64_encode, chachapoly::ChaChaPoly, hmac::Hmac, siphash::SipHash,
        EphemeralPrivateKey, StaticPrivateKey, StaticPublicKey,
    },
    error::Ntcp2Error,
    runtime::Runtime,
    transport::{
        ntcp2::{
            message::MessageBlock,
            options::{InitiatorOptions, ResponderOptions},
            session::{EncryptionContext, KeyContext, MAX_CLOCK_SKEW},
        },
        EncryptionKind,
    },
};

use bytes::{BufMut, Bytes, BytesMut};
use ml_kem::{kem::Kem, Decapsulate, DecapsulationKey, KeyExport, MlKem1024, MlKem512, MlKem768};
use rand::Rng;
use zeroize::Zeroize;

use alloc::{boxed::Box, vec, vec::Vec};
use core::{fmt, time::Duration};

/// Logging target for the file.
const LOG_TARGET: &str = "emissary::ntcp2::initiator";

/// Poly1305 MAC size.
const POLY1305_MAC_SIZE: usize = 16usize;

/// ML-KEM context.
enum MlKemContext {
    /// ML-KEM-512-x25519
    MlKem512X25519(Box<DecapsulationKey<MlKem512>>),

    /// ML-KEM-768-x25519
    MlKem768X25519(Box<DecapsulationKey<MlKem768>>),

    /// ML-KEM-1024-x25519
    MlKem1024X25519(Box<DecapsulationKey<MlKem1024>>),
}

/// Initiator state.
#[derive(Default)]
enum InitiatorState {
    /// Initiator has sent `SessionCreated` message to remote
    /// and is waitint to hear a response.
    SessionRequested {
        /// Decapsulation key.
        ///
        /// `None` for x25519.
        decapsulation_key: Option<MlKemContext>,

        /// Encryption context.
        encryption_ctx: EncryptionContext,

        /// Ephemeral private key.
        ephemeral_key: EphemeralPrivateKey,

        /// AES IV.
        iv: [u8; 16],

        /// Local router info.
        local_info: Bytes,

        /// Static private key.
        local_static_key: StaticPrivateKey,

        /// Router hash.
        router_hash: Vec<u8>,
    },

    /// Responder has accepted the session request and is waiting for initiator to confirm the
    /// session
    SessionCreated {
        /// Encryption context.
        encryption_ctx: EncryptionContext,

        /// Local router info.
        local_info: Bytes,

        /// Static private key.
        local_static_key: StaticPrivateKey,

        /// Remote key.
        remote_key: Vec<u8>,

        // Responder's public key.
        responder_public: Box<StaticPublicKey>,

        /// Router hash.
        router_hash: Vec<u8>,
    },

    /// Initiator state has been poisoned.
    #[default]
    Poisoned,
}

impl fmt::Debug for InitiatorState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SessionRequested { .. } =>
                f.debug_struct("SessionRequested").finish_non_exhaustive(),
            Self::SessionCreated { .. } => f.debug_struct("SessionCreated").finish_non_exhaustive(),
            Self::Poisoned => f.debug_struct("Poisoned").finish(),
        }
    }
}

/// Noise handshake initiator
pub struct Initiator {
    /// Initiator state.
    state: InitiatorState,
}

impl Initiator {
    /// Create new [`Handshaker`] for initiator.
    ///
    /// Implements KDF from [1], creates a `SessionRequest` message and returns that message
    /// together with an [`Initiator`] object which allows the call to drive progress on the
    /// opening connection.
    ///
    /// [1]: <https://geti2p.net/spec/ntcp2#key-derivation-function-kdf-for-handshake-message-1>
    pub fn new<R: Runtime>(
        mut encryption_ctx: EncryptionContext,
        local_info: Bytes,
        local_static_key: StaticPrivateKey,
        remote_static_key: &StaticPublicKey,
        router_hash: Vec<u8>,
        remote_iv: [u8; 16],
        net_id: u8,
    ) -> Result<(Self, BytesMut), Ntcp2Error> {
        tracing::trace!(
            target: LOG_TARGET,
            router = ?base64_encode(&router_hash),
            "initiate new connection"
        );

        // generate ephemeral key pair and apply MixHash(epub)
        let sk = EphemeralPrivateKey::random(R::rng());
        let mut pk = sk.public().to_vec();

        // MixHash(rs), MixHash(e.pubkey)
        encryption_ctx.noise_ctx().mix_hash(remote_static_key).mix_hash(&pk);

        // MixKey(DH()), es
        let mut local_key = encryption_ctx.noise_ctx().mix_key(&sk, remote_static_key);

        // for PQ connections, set MSB of the key to 1
        let pk = match &encryption_ctx {
            EncryptionContext::X25519(_) => {
                tracing::trace!(
                    target: LOG_TARGET,
                    "establish x25519 connection",
                );

                pk
            }
            kind => {
                tracing::trace!(
                    target: LOG_TARGET,
                    %kind,
                    "establish ml-kem connection",
                );

                pk[31] |= 0x80;
                pk
            }
        };

        // encrypt X
        let mut aes = Aes::new_encryptor(&router_hash, &remote_iv);
        let encrypted_x = aes.encrypt(pk);

        // create `SessionRequest` message
        let mut out = BytesMut::with_capacity(96 + encryption_ctx.encapsulation_key_size());

        // e1
        //
        // https://i2p.net/en/docs/specs/ntcp2-hybrid/#alice-kdf-for-message-1
        let (nonce, encap_key, decap_key) = match &mut encryption_ctx {
            EncryptionContext::X25519(_) => (0u64, None, None),
            kind => {
                let (mut encap_key, context) = match kind {
                    EncryptionContext::X25519(_) => unreachable!(),
                    EncryptionContext::MlKem512X25519(_) => {
                        let (decap_key, encap_key) =
                            MlKem512::generate_keypair_from_rng(&mut R::rng());

                        (
                            encap_key.to_bytes().to_vec(),
                            Some(MlKemContext::MlKem512X25519(Box::new(decap_key))),
                        )
                    }
                    EncryptionContext::MlKem768X25519(_) => {
                        let (decap_key, encap_key) =
                            MlKem768::generate_keypair_from_rng(&mut R::rng());

                        (
                            encap_key.to_bytes().to_vec(),
                            Some(MlKemContext::MlKem768X25519(Box::new(decap_key))),
                        )
                    }
                    EncryptionContext::MlKem1024X25519(_) => {
                        let (decap_key, encap_key) =
                            MlKem1024::generate_keypair_from_rng(&mut R::rng());

                        (
                            encap_key.to_bytes().to_vec(),
                            Some(MlKemContext::MlKem1024X25519(Box::new(decap_key))),
                        )
                    }
                };

                ChaChaPoly::with_nonce(&local_key, 0u64)
                    .encrypt_with_ad_new(kind.noise_ctx().state(), &mut encap_key)
                    .map_err(|_| Ntcp2Error::Chacha)?;

                // MixHash(ciphertext)
                kind.noise_ctx().mix_hash(&encap_key);

                (1u64, Some(encap_key), context)
            }
        };

        let padding = {
            let mut padding = vec![0u8; (R::rng().next_u32() % 16 + 8) as usize];
            R::rng().fill_bytes(&mut padding);

            padding
        };

        let mut options = InitiatorOptions {
            network_id: net_id,
            version: 2u8,
            padding_length: padding.len() as u16,
            timestamp: R::time_since_epoch().as_secs() as u32,
            m3_p2_len: local_info.len() as u16 + 20u16,
        }
        .serialize()
        .to_vec();

        ChaChaPoly::with_nonce(&local_key, nonce)
            .encrypt_with_ad_new(encryption_ctx.noise_ctx().state(), &mut options)
            .map_err(|_| Ntcp2Error::Chacha)?;
        local_key.zeroize();

        out.put_slice(&encrypted_x);
        if let Some(encap_key) = encap_key {
            out.put_slice(&encap_key);
        }
        let offset = out.len();
        out.put_slice(&options);
        out.put_slice(&padding);

        // MixHash(encrypted payload), MixHash(padding)
        encryption_ctx
            .noise_ctx()
            .mix_hash(&out[offset..offset + 32])
            .mix_hash(&out[offset + 32..offset + 32 + padding.len()]);

        Ok((
            Self {
                state: InitiatorState::SessionRequested {
                    decapsulation_key: decap_key,
                    ephemeral_key: sk,
                    iv: aes.iv(),
                    local_info,
                    local_static_key,
                    encryption_ctx,
                    router_hash,
                },
            },
            out,
        ))
    }

    /// Register `SessionCreated` message from responder (Bob).
    ///
    /// Decrypt `Y` and perform KDF for messages 2 and 3 part 1
    ///
    /// <https://geti2p.net/spec/ntcp2#key-derivation-function-kdf-for-handshake-message-2-and-message-3-part-1>
    pub fn register_session_created<R: Runtime>(
        &mut self,
        bytes: &[u8],
    ) -> Result<usize, Ntcp2Error> {
        let InitiatorState::SessionRequested {
            decapsulation_key,
            ephemeral_key,
            iv,
            local_info,
            local_static_key,
            mut encryption_ctx,
            router_hash,
        } = core::mem::take(&mut self.state)
        else {
            return Err(Ntcp2Error::InvalidState);
        };

        tracing::trace!(
            target: LOG_TARGET,
            router = ?base64_encode(&router_hash),
            "session created"
        );

        // decrypt `Y`
        let mut aes = Aes::new_decryptor(&router_hash, &iv);
        let y = aes.decrypt(&bytes[..32]);

        // MixHash(e.pubkey)
        encryption_ctx.noise_ctx().mix_hash(&y);

        // MixKey(DH())
        let responder_public =
            StaticPublicKey::try_from_bytes(&y).ok_or(Ntcp2Error::InvalidData)?;
        let remote_key = encryption_ctx.noise_ctx().mix_key(&ephemeral_key, &responder_public);

        // ekem1
        //
        // https://i2p.net/en/docs/specs/ntcp2-hybrid/#alice-kdf-for-message-2
        let (remote_key, offset) = match &mut encryption_ctx {
            EncryptionContext::X25519(_) => (remote_key, 32usize),
            kind => {
                let kem_ciphertext_size = kind.kem_ciphertext_size() + POLY1305_MAC_SIZE;

                if bytes.len() <= kem_ciphertext_size {
                    tracing::warn!(
                        target: LOG_TARGET,
                        size = ?bytes.len(),
                        expected = ?kem_ciphertext_size,
                        "SessionCreated is too short for ml-kem",
                    );
                    return Err(Ntcp2Error::InvalidData);
                }

                let mut kem_ciphertext = bytes[32..32 + kem_ciphertext_size].to_vec();
                ChaChaPoly::new(&remote_key)
                    .decrypt_with_ad(kind.noise_ctx().state(), &mut kem_ciphertext)
                    .map_err(|_| Ntcp2Error::Chacha)?;

                // MixHash(kem_ciphertext_section)
                kind.noise_ctx().mix_hash(&bytes[32..32 + kem_ciphertext_size]);

                // MixKey(kem_shared_key)
                //
                // decapsulation key must exist since `encryption_ctx` indicates ml-kem
                let keydata = match decapsulation_key.expect("to exist") {
                    MlKemContext::MlKem512X25519(decap_key) => {
                        let shared = decap_key
                            .decapsulate_slice(&kem_ciphertext)
                            .map_err(|_| Ntcp2Error::InvalidData)?
                            .to_vec();
                        kind.noise_ctx().mix_key_from_shared_secret(&shared)
                    }
                    MlKemContext::MlKem768X25519(decap_key) => {
                        let shared = decap_key
                            .decapsulate_slice(&kem_ciphertext)
                            .map_err(|_| Ntcp2Error::InvalidData)?
                            .to_vec();
                        kind.noise_ctx().mix_key_from_shared_secret(&shared)
                    }
                    MlKemContext::MlKem1024X25519(decap_key) => {
                        let shared = decap_key
                            .decapsulate_slice(&kem_ciphertext)
                            .map_err(|_| Ntcp2Error::InvalidData)?
                            .to_vec();
                        kind.noise_ctx().mix_key_from_shared_secret(&shared)
                    }
                };

                (keydata, kem_ciphertext_size + 32)
            }
        };

        // decrypt the chacha20poly1305 frame with generated remote key, deserialize
        // `ResponderOptions` and validate timestamp
        let padding = {
            let mut options = bytes[offset..offset + 32].to_vec();
            ChaChaPoly::new(&remote_key)
                .decrypt_with_ad(encryption_ctx.noise_ctx().state(), &mut options)
                .map_err(|_| Ntcp2Error::Chacha)?;

            // check clock skew
            let options = ResponderOptions::parse(&options).ok_or(Ntcp2Error::InvalidOptions)?;
            let now = R::time_since_epoch();
            let remote_time = Duration::from_secs(options.timestamp as u64);
            let future = remote_time.saturating_sub(now);
            let past = now.saturating_sub(remote_time);

            if past > MAX_CLOCK_SKEW || future > MAX_CLOCK_SKEW {
                tracing::warn!(
                    target: LOG_TARGET,
                    our_time = ?now,
                    ?remote_time,
                    ?past,
                    ?future,
                    "excessive clock skew",
                );
                return Err(Ntcp2Error::ClockSkew);
            }

            options.padding_length as usize
        };

        // MixHash(ciphertext)
        //
        // https://geti2p.net/spec/ntcp2#encryption-for-for-handshake-message-3-part-1-using-message-2-kdf
        encryption_ctx.noise_ctx().mix_hash(&bytes[offset..offset + 32]);

        self.state = InitiatorState::SessionCreated {
            local_info,
            local_static_key,
            encryption_ctx,
            remote_key: remote_key.to_vec(),
            responder_public: Box::new(responder_public),
            router_hash,
        };

        Ok(padding)
    }

    /// Finalize session.
    ///
    /// Include `padding` bytes to current state, perform Diffie-Hellman key exchange
    /// between responder's public key and local private key, create `SessionConfirmed`
    /// message which contains local public key & router info and finally derive keys
    /// for the data phase.
    ///
    /// <https://geti2p.net/spec/ntcp2#key-derivation-function-kdf-for-data-phase>
    pub fn finalize(
        &mut self,
        padding: &[u8],
    ) -> Result<(KeyContext, BytesMut, EncryptionKind), Ntcp2Error> {
        let InitiatorState::SessionCreated {
            local_info,
            local_static_key,
            mut encryption_ctx,
            remote_key,
            responder_public,
            router_hash,
        } = core::mem::take(&mut self.state)
        else {
            return Err(Ntcp2Error::InvalidState);
        };

        tracing::trace!(
            target: LOG_TARGET,
            router = ?base64_encode(&router_hash),
            "confirm session"
        );

        // MixHash(padding)
        encryption_ctx.noise_ctx().mix_hash(padding);

        // encrypt local public key
        let mut s_p_bytes = local_static_key.public().to_vec();
        let mut cipher = ChaChaPoly::with_nonce(&remote_key, 1);
        cipher
            .encrypt_with_ad_new(encryption_ctx.noise_ctx().state(), &mut s_p_bytes)
            .map_err(|_| Ntcp2Error::Chacha)?;

        // MixHash(ciphertext)
        encryption_ctx.noise_ctx().mix_hash(&s_p_bytes);

        // perform diffie-hellman key exchange and derive keys for data phase
        let (key_context, message) = {
            // MixKey(DH())
            let mut k = encryption_ctx.noise_ctx().mix_key(&local_static_key, &*responder_public);

            // h from message 3 part 1 is used as the associated data
            // for the AEAD in message 3 part 2
            let mut message = MessageBlock::new_router_info(&local_info);
            ChaChaPoly::with_nonce(&k, 0)
                .encrypt_with_ad_new(encryption_ctx.noise_ctx().state(), &mut message)
                .map_err(|_| Ntcp2Error::Chacha)?;

            // MixHash(ciphertext)
            encryption_ctx.noise_ctx().mix_hash(&message);

            // create `SessionConfirmed` message
            let mut out = BytesMut::with_capacity(local_info.len() + 20 + 48);
            out.put_slice(&s_p_bytes);
            out.put_slice(&message);

            // create send and receive keys
            let temp_key =
                Hmac::new(encryption_ctx.noise_ctx().chaining_key()).update([]).finalize();
            let send_key = Hmac::new(&temp_key).update([0x01]).finalize();
            let receive_key = Hmac::new(&temp_key).update(&send_key).update([0x02]).finalize();

            // siphash context for (de)obfuscating message sizes
            let sip = SipHash::new_initiator(&temp_key, encryption_ctx.noise_ctx().state());

            k.zeroize();

            (KeyContext::new(send_key, receive_key, sip), out)
        };

        Ok((
            key_context,
            message,
            match encryption_ctx {
                EncryptionContext::X25519(_) => EncryptionKind::X25519,
                EncryptionContext::MlKem512X25519(_) => EncryptionKind::MlKem512X25519,
                EncryptionContext::MlKem768X25519(_) => EncryptionKind::MlKem768X25519,
                EncryptionContext::MlKem1024X25519(_) => EncryptionKind::MlKem1024X25519,
            },
        ))
    }

    /// Get the size of `SessionCreated` message.
    pub fn session_created_size(&self) -> Result<usize, Ntcp2Error> {
        match &self.state {
            InitiatorState::SessionRequested { encryption_ctx, .. } => match encryption_ctx {
                EncryptionContext::X25519(_) => Ok(64),
                kind => Ok(kind.kem_ciphertext_size() + POLY1305_MAC_SIZE + 64),
            },
            _ => Err(Ntcp2Error::InvalidState),
        }
    }
}
