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
    crypto::{
        chachapoly::ChaChaPoly, hmac::Hmac, sha256::Sha256, StaticPrivateKey, StaticPublicKey,
    },
    destination::session::{
        inbound::{InboundSession, MlKemContext as InboundMlKemContext},
        outbound::{MlKemContext as OutboundMlKemContext, OutboundSession},
    },
    error::SessionError,
    i2np::{
        database::store::{DatabaseStoreBuilder, DatabaseStoreKind},
        garlic::{DeliveryInstructions as GarlicDeliveryInstructions, GarlicMessageBuilder},
        MessageType, I2NP_MESSAGE_EXPIRATION,
    },
    primitives::{DestinationId, MessageId},
    runtime::Runtime,
};

use bytes::{BufMut, Bytes, BytesMut};
use curve25519_elligator2::{MapToPointVariant, Randomized};
use ml_kem::{kem::Kem, KeyExport, MlKem1024, MlKem512, MlKem768};
use rand::Rng;
use zeroize::Zeroize;

use alloc::{boxed::Box, vec::Vec};
use core::{fmt, marker::PhantomData, ops::Range};

/// Logging target for the file.
const LOG_TARGET: &str = "emissary::destination::session::context";

mod constants {
    pub mod ml_kem_512 {
        /// Size of the encapsulation key.
        pub const KEY_SIZE: usize = 800usize;

        /// Noise protocol name for ML-KEM-512-x25519.
        pub const PROTOCOL_NAME: &str = "Noise_IKhfselg2_25519+MLKEM512_ChaChaPoly_SHA256";
    }

    pub mod ml_kem_768 {
        /// Size of the encapsulation key.
        pub const KEY_SIZE: usize = 1184usize;

        /// Noise protocol name for ML-KEM-768-x25519.
        pub const PROTOCOL_NAME: &str = "Noise_IKhfselg2_25519+MLKEM768_ChaChaPoly_SHA256";
    }

    pub mod ml_kem_1024 {
        /// Size of the encapsulation key.
        pub const KEY_SIZE: usize = 1568usize;

        /// Noise protocol name for ML-KEM-1024-x25519.
        pub const PROTOCOL_NAME: &str = "Noise_IKhfselg2_25519+MLKEM1024_ChaChaPoly_SHA256";
    }

    pub mod x25519 {
        /// Noise protocol name for x25519.
        pub const PROTOCOL_NAME: &str = "Noise_IKelg2+hs2_25519_ChaChaPoly_SHA256";
    }
}

/// Ephemeral public key offset in `NewSession` message.
const NS_EPHEMERAL_PUBKEY_OFFSET: Range<usize> = 4..36;

/// Static public key section size.
///
/// 32 bytes for the key and 16 for Poly1305 MAC.
const NS_STATIC_PUBKEY_SECTION_SIZE: usize = 32usize + 16usize;

/// Minimum size for `NewSession` message.
const NS_MINIMUM_SIZE: usize = 100usize;

/// Poly1305 MAC size.
const POLY1035_MAC_SIZE: usize = 16usize;

/// ML-KEM kind.
#[derive(Clone)]
pub enum MlKemKind {
    /// ML-KEM-512-x25519.
    MlKem512X25519 {
        /// Chaining key.
        chaining_key: Bytes,

        /// Outbound state.
        inbound_state: Bytes,
    },

    /// ML-KEM-512-x25519.
    MlKem768X25519 {
        /// Chaining key.
        chaining_key: Bytes,

        /// Outbound state.
        inbound_state: Bytes,
    },

    /// ML-KEM-512-x25519.
    MlKem1024X25519 {
        /// Chaining key.
        chaining_key: Bytes,

        /// Outbound state.
        inbound_state: Bytes,
    },
}

impl MlKemKind {
    /// Get key size.
    fn key_size(&self) -> usize {
        match self {
            Self::MlKem512X25519 { .. } => constants::ml_kem_512::KEY_SIZE,
            Self::MlKem768X25519 { .. } => constants::ml_kem_768::KEY_SIZE,
            Self::MlKem1024X25519 { .. } => constants::ml_kem_1024::KEY_SIZE,
        }
    }

    /// Get inbound state.
    fn inbound_state(&self) -> &[u8] {
        match self {
            Self::MlKem512X25519 { inbound_state, .. } => inbound_state.as_ref(),
            Self::MlKem768X25519 { inbound_state, .. } => inbound_state.as_ref(),
            Self::MlKem1024X25519 { inbound_state, .. } => inbound_state.as_ref(),
        }
    }

    /// Get chaining key.
    fn chaining_key(&self) -> &[u8] {
        match self {
            Self::MlKem512X25519 { chaining_key, .. } => chaining_key.as_ref(),
            Self::MlKem768X25519 { chaining_key, .. } => chaining_key.as_ref(),
            Self::MlKem1024X25519 { chaining_key, .. } => chaining_key.as_ref(),
        }
    }
}

impl fmt::Display for MlKemKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MlKem512X25519 { .. } => write!(f, "ml-kem-512-x25519"),
            Self::MlKem768X25519 { .. } => write!(f, "ml-kem-768-x25519"),
            Self::MlKem1024X25519 { .. } => write!(f, "ml-kem-1024-x25519"),
        }
    }
}

impl fmt::Debug for MlKemKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        <Self as fmt::Display>::fmt(self, f)
    }
}

/// Outbound key context.
#[derive(Clone)]
struct OutboundKeyContext {
    /// Chaining key.
    chaining_key: Bytes,

    /// Outbound state.
    outbound_state: Bytes,
}

/// Inbound key context.
#[derive(Clone)]
struct InboundContext {
    /// Chaining key and outobund state for ML-KEM.
    ///
    /// `None` if not enabled.
    ml_kem: Option<MlKemKind>,

    /// Chaining key and outbound state for x25519.
    ///
    /// `None` if not enabled
    x25519: Option<(Bytes, Bytes)>,
}

/// Decryption state for ML-KEM NS messages.
struct MlKemNsState {
    /// Chaining key.
    chaining_key: [u8; 32],

    /// Cipher key.
    cipher_key: [u8; 32],

    /// ML-KEM context for an inbound session.
    ml_kem_context: InboundMlKemContext,

    /// Offset for the end of encapsulation key section.
    offset: usize,

    /// Current state.
    state: [u8; 32],
}

/// Key context for an ECIES-X25519-AEAD-Ratchet session.
///
/// Inbound connections/outbound connections use the protocol specified in the list of supported
/// keys. The used protocol is chosen based on locally available keys and remote's preferences.
#[derive(Clone)]
pub struct KeyContext<R: Runtime> {
    /// Key context for x25519.
    x25519: OutboundKeyContext,

    /// Key context for ML-KEM-1024-x25519.
    ml_kem_1024_x25519: OutboundKeyContext,

    /// Key context for ML-KEM-512-x25519.
    ml_kem_512_x25519: OutboundKeyContext,

    /// Key context for ML-KEM-768-x25519.
    ml_kem_768_x25519: OutboundKeyContext,

    /// Inbound context.
    ///
    /// Has state for x25519 and one of the ML-KEM variants.
    inbound_context: InboundContext,

    /// Static private key of the session.
    private_key: StaticPrivateKey,

    /// Static public key of the session.
    public_key: StaticPublicKey,

    /// Marker for `Runtime`.
    _runtime: PhantomData<R>,
}

impl<R: Runtime> KeyContext<R> {
    /// Create new `KeyContext` from a private key and supported public keys.
    ///
    /// `public_keys` is guaranteed to contain at least one element corresponding to the private key
    /// but may contain two elements where the order in the vector specifies the preference.
    ///
    /// <https://i2p.net/en/docs/specs/ecies/#1f-kdfs-for-new-session-message>
    /// <https://i2p.net/en/docs/specs/ecies-hybrid/#noise-identifiers>
    pub fn from_keys(private_key: StaticPrivateKey, public_keys: Vec<StaticPublicKey>) -> Self {
        let public_key = private_key.public();
        let make_key_context = |protocol_name: &str| -> (Bytes, Bytes, Bytes) {
            let chaining_key = Sha256::new().update(protocol_name.as_bytes()).finalize();

            let outbound_state = Sha256::new().update(&chaining_key).finalize();
            let inbound_state =
                Sha256::new().update(&outbound_state).update(&public_key).finalize();

            (
                Bytes::from(chaining_key),
                Bytes::from(outbound_state),
                Bytes::from(inbound_state),
            )
        };

        // create key contexts for each supported protocol
        let x25519 = make_key_context(constants::x25519::PROTOCOL_NAME);
        let ml_kem_512_x25519 = make_key_context(constants::ml_kem_512::PROTOCOL_NAME);
        let ml_kem_768_x25519 = make_key_context(constants::ml_kem_768::PROTOCOL_NAME);
        let ml_kem_1024_x25519 = make_key_context(constants::ml_kem_1024::PROTOCOL_NAME);

        // `public_keys` has been validated by the caller to contain 1 or 2 keys, no more no less
        //
        // caller has also validated that the combination is valid, i.e., if there are two keys, one
        // of them is x25519 and the other is an ml-kem variant but that there are never two ml-kem
        // keys
        //
        // TODO: revisit this logic in the future, it's no good
        let (x25519_enaled, ml_kem) = match (&public_keys[0], public_keys.get(1)) {
            (StaticPublicKey::X25519(_), key) => (true, key),
            (key @ StaticPublicKey::MlKem512X25519(_), x25519) => (x25519.is_some(), Some(key)),
            (key @ StaticPublicKey::MlKem768X25519(_), x25519) => (x25519.is_some(), Some(key)),
            (key @ StaticPublicKey::MlKem1024X25519(_), x25519) => (x25519.is_some(), Some(key)),
        };
        let inbound_context = InboundContext {
            ml_kem: ml_kem.map(|key| match key {
                StaticPublicKey::X25519(_) => unreachable!(),
                StaticPublicKey::MlKem512X25519(_) => MlKemKind::MlKem512X25519 {
                    chaining_key: ml_kem_512_x25519.0.clone(),
                    inbound_state: ml_kem_512_x25519.2.clone(),
                },
                StaticPublicKey::MlKem768X25519(_) => MlKemKind::MlKem768X25519 {
                    chaining_key: ml_kem_768_x25519.0.clone(),
                    inbound_state: ml_kem_768_x25519.2.clone(),
                },
                StaticPublicKey::MlKem1024X25519(_) => MlKemKind::MlKem1024X25519 {
                    chaining_key: ml_kem_1024_x25519.0.clone(),
                    inbound_state: ml_kem_1024_x25519.2.clone(),
                },
            }),
            x25519: x25519_enaled.then(|| (x25519.0.clone(), x25519.2.clone())),
        };

        tracing::debug!(
            target: LOG_TARGET,
            x25519 = ?inbound_context.x25519.is_some(),
            ml_kem = ?inbound_context.ml_kem,
            "session key context created",
        );

        Self {
            inbound_context,
            private_key,
            public_key,
            x25519: OutboundKeyContext {
                chaining_key: x25519.0,
                outbound_state: x25519.1,
            },
            ml_kem_512_x25519: OutboundKeyContext {
                chaining_key: ml_kem_512_x25519.0,
                outbound_state: ml_kem_512_x25519.1,
            },
            ml_kem_768_x25519: OutboundKeyContext {
                chaining_key: ml_kem_768_x25519.0,
                outbound_state: ml_kem_768_x25519.1,
            },
            ml_kem_1024_x25519: OutboundKeyContext {
                chaining_key: ml_kem_1024_x25519.0,
                outbound_state: ml_kem_1024_x25519.1,
            },
            _runtime: PhantomData,
        }
    }

    /// Generate private key which can be Elligator2-encoded.
    pub fn generate_ephemeral_keypair() -> ([u8; 32], u8) {
        let mut rng = R::rng();
        let tweak = rng.next_u32() as u8;

        loop {
            let mut private = [0u8; 32];
            rng.fill_bytes(&mut private);

            if Randomized::to_representative(&private, tweak).into_option().is_some() {
                return (private, tweak);
            }
        }
    }

    /// Create new outbound session.
    ///
    /// <https://geti2p.net/spec/ecies#f-kdfs-for-new-session-message>
    /// <https://i2p.net/en/docs/specs/ecies-hybrid/#alice-kdf-for-ns-message>
    pub fn create_outbound_session(
        &mut self,
        local: DestinationId,
        remote: DestinationId,
        remote_public_key: &StaticPublicKey,
        lease_set: Bytes,
        payload: &[u8],
    ) -> (OutboundSession<R>, Vec<u8>) {
        // create garlic message for establishing a new session
        //
        // the message consists of three parts
        //  * date time block
        //  * bundled leaseset
        //  * garlic clove for upper-level protocol data
        //
        // this garlic message is wrapped inside a `NewSession` message
        // and sent to remote
        let database_store = DatabaseStoreBuilder::new(
            Bytes::from(local.to_vec()),
            DatabaseStoreKind::LeaseSet2 {
                lease_set: lease_set.clone(),
            },
        )
        .build();

        // get correct chaining key and outbound state using the public key type
        let (chaining_key, outbound_state) = match remote_public_key {
            StaticPublicKey::X25519(_) => (&self.x25519.chaining_key, &self.x25519.outbound_state),
            StaticPublicKey::MlKem512X25519(_) => (
                &self.ml_kem_512_x25519.chaining_key,
                &self.ml_kem_512_x25519.outbound_state,
            ),
            StaticPublicKey::MlKem768X25519(_) => (
                &self.ml_kem_768_x25519.chaining_key,
                &self.ml_kem_768_x25519.outbound_state,
            ),
            StaticPublicKey::MlKem1024X25519(_) => (
                &self.ml_kem_1024_x25519.chaining_key,
                &self.ml_kem_1024_x25519.outbound_state,
            ),
        };

        let hash = remote.to_vec();
        let payload = GarlicMessageBuilder::default()
            .with_date_time(R::time_since_epoch().as_secs() as u32)
            .with_garlic_clove(
                MessageType::DatabaseStore,
                MessageId::from(R::rng().next_u32()),
                R::time_since_epoch() + I2NP_MESSAGE_EXPIRATION,
                GarlicDeliveryInstructions::Local,
                &database_store,
            )
            .with_garlic_clove(
                MessageType::Data,
                MessageId::from(R::rng().next_u32()),
                R::time_since_epoch() + I2NP_MESSAGE_EXPIRATION,
                GarlicDeliveryInstructions::Destination { hash: &hash },
                &{
                    let mut out = BytesMut::with_capacity(payload.len() + 4);

                    out.put_u32(payload.len() as u32);
                    out.put_slice(payload);

                    out.freeze().to_vec()
                },
            )
            .build();

        // generate new elligator2-encodable ephemeral keypair
        let (private_key, public_key, representative) = {
            let (private_key, tweak) = Self::generate_ephemeral_keypair();
            // conversion is expected to succeed since the key was generated by us
            let sk = StaticPrivateKey::try_from_bytes(&private_key).expect("to succeed");
            let public_key = StaticPublicKey::from_bytes(
                Randomized::mul_base_clamped(private_key).to_montgomery().0,
            );

            // elligator2 conversion must succeed because `Self::generate_ephemeral_keypair()`
            // has ensured that the public key is encodable
            let representative =
                Randomized::to_representative(&private_key, tweak).expect("to succeed");

            (sk, public_key, representative)
        };

        let state = {
            let state = Sha256::new()
                .update(outbound_state)
                .update::<&[u8]>(remote_public_key.as_ref())
                .finalize();

            Sha256::new().update(&state).update(&public_key).finalize()
        };

        // derive keys for encrypting initiator's static key and possible for ml-kem section
        let mut shared = private_key.diffie_hellman(remote_public_key);
        let mut temp_key = Hmac::new(chaining_key).update(&shared).finalize();
        let chaining_key = Hmac::new(&temp_key).update(b"").update([0x01]).finalize();
        let mut cipher_key =
            Hmac::new(&temp_key).update(&chaining_key).update(b"").update([0x02]).finalize();

        // encrypt ml-kem encapsulation key if remote public key indicates hybrid
        //
        // <https://i2p.net/en/docs/specs/ecies-hybrid/#message-format>
        let (nonce, state, encap_ciphertext, ml_kem_context) = if let StaticPublicKey::X25519(_) =
            &remote_public_key
        {
            tracing::trace!(
                target: LOG_TARGET,
                %local,
                %remote,
                "creating outbound x25519 session",
            );

            (0u64, state, None, None)
        } else {
            let (mut encap_key, ml_kem_context) = match remote_public_key {
                StaticPublicKey::X25519(_) => unreachable!(),
                StaticPublicKey::MlKem512X25519(_) => {
                    tracing::trace!(
                        target: LOG_TARGET,
                        %local,
                        %remote,
                        "creating outbound ml-kem-512-x25519 session",
                    );

                    let (decap_key, encap_key) = MlKem512::generate_keypair_from_rng(&mut R::rng());
                    (
                        encap_key.to_bytes().to_vec(),
                        Some(OutboundMlKemContext::MlKem512X25519(Box::new(decap_key))),
                    )
                }
                StaticPublicKey::MlKem768X25519(_) => {
                    tracing::trace!(
                        target: LOG_TARGET,
                        %local,
                        %remote,
                        "creating outbound ml-kem-768-x25519 session",
                    );

                    let (decap_key, encap_key) = MlKem768::generate_keypair_from_rng(&mut R::rng());
                    (
                        encap_key.to_bytes().to_vec(),
                        Some(OutboundMlKemContext::MlKem768X25519(Box::new(decap_key))),
                    )
                }
                StaticPublicKey::MlKem1024X25519(_) => {
                    tracing::trace!(
                        target: LOG_TARGET,
                        %local,
                        %remote,
                        "creating outbound ml-kem-1024-x25519 session",
                    );

                    let (decap_key, encap_key) =
                        MlKem1024::generate_keypair_from_rng(&mut R::rng());
                    (
                        encap_key.to_bytes().to_vec(),
                        Some(OutboundMlKemContext::MlKem1024X25519(Box::new(decap_key))),
                    )
                }
            };

            ChaChaPoly::with_nonce(&cipher_key, 0u64)
                .encrypt_with_ad_new(&state, &mut encap_key)
                .expect("to succeed");

            (
                1u64,
                Sha256::new().update(&state).update(&encap_key).finalize(),
                Some(encap_key),
                ml_kem_context,
            )
        };

        // encrypt static key section
        let static_key_ciphertext = {
            // encrypt initiator's static public key
            //
            // `encrypt_with_ad()` must succeed as it's called with valid parameters
            let mut static_key = {
                let mut out = BytesMut::with_capacity(32 + 16);
                out.put_slice(self.public_key.as_ref());

                out.freeze().to_vec()
            };

            ChaChaPoly::with_nonce(&cipher_key, nonce)
                .encrypt_with_ad_new(&state, &mut static_key)
                .expect("to succeed");

            static_key
        };

        shared.zeroize();
        temp_key.zeroize();
        cipher_key.zeroize();

        // state for payload section
        let state = Sha256::new().update(&state).update(&static_key_ciphertext).finalize();

        // encrypt payload section
        let (chaining_key, payload_ciphertext) = {
            let mut shared = self.private_key.diffie_hellman(remote_public_key);
            let mut temp_key = Hmac::new(&chaining_key).update(&shared).finalize();
            let chaining_key = Hmac::new(&temp_key).update(b"").update([0x01]).finalize();
            let mut cipher_key =
                Hmac::new(&temp_key).update(&chaining_key).update(b"").update([0x02]).finalize();

            // create buffer with 16 extra bytes for poly1305 auth tag
            let mut payload = {
                let mut out = BytesMut::with_capacity(payload.len() + 16);
                out.put_slice(&payload);

                out.freeze().to_vec()
            };

            // `encrypt_with_ad()` must succeed as it's called with valid parameters
            ChaChaPoly::with_nonce(&cipher_key, 0u64)
                .encrypt_with_ad_new(&state, &mut payload)
                .expect("to succeed");

            shared.zeroize();
            temp_key.zeroize();
            cipher_key.zeroize();

            (chaining_key, payload)
        };

        // state for new session reply kdf
        let state =
            Bytes::from(Sha256::new().update(&state).update(&payload_ciphertext).finalize());

        let payload = {
            let mut out = BytesMut::with_capacity(
                representative
                    .len()
                    .saturating_add(static_key_ciphertext.len())
                    .saturating_add(payload_ciphertext.len())
                    .saturating_add(encap_ciphertext.as_ref().map_or(0, |key| key.len())),
            );
            out.put_slice(&representative);
            if let Some(encap) = encap_ciphertext {
                out.put_slice(&encap);
            }
            out.put_slice(&static_key_ciphertext);
            out.put_slice(&payload_ciphertext);

            out.freeze().to_vec()
        };

        (
            OutboundSession::new(
                remote,
                state,
                self.private_key.clone(),
                private_key,
                chaining_key,
                ml_kem_context,
            ),
            payload,
        )
    }

    /// Attempt to create inbound ML-KEM session.
    ///
    /// If decryption succeeds, `MlKemNsContext` is returned which contains state that the caller
    /// must use to process the rest of the message.
    ///
    /// If an error is returned, the caller must attempt to handle the message as a normal x25519 NS
    /// message.
    fn create_inbound_session_ml_kem(
        &self,
        public_key: &StaticPublicKey,
        message: &[u8],
    ) -> Result<MlKemNsState, SessionError> {
        let Some(ref kind) = self.inbound_context.ml_kem else {
            return Err(SessionError::MlKemNotEnabled);
        };

        // encapsulation key is right after the elligator2-encoded public key
        //
        // the section is variable length and ends in a poly1305 mac
        let encap_offset = 4 + 32 + kind.key_size() + POLY1035_MAC_SIZE;
        if message.len() <= encap_offset {
            return Err(SessionError::TooShortForMlKem);
        }

        tracing::trace!(
            target: LOG_TARGET,
            %kind,
            "trying to decrypt with ml-kem",
        );

        // calculate new state based on remote's ephemeral public key
        let state = Sha256::new().update(kind.inbound_state()).update(public_key).finalize();

        // generate chaining key and cipher key for decrypting remote's public key
        let (chaining_key, cipher_key) = {
            let mut shared = self.private_key.diffie_hellman(&public_key);
            let mut temp_key = Hmac::new(kind.chaining_key()).update(&shared).finalize();
            let chaining_key = Hmac::new(&temp_key).update(b"").update([0x01]).finalize_new();
            let cipher_key = Hmac::new(&temp_key)
                .update(chaining_key)
                .update(b"")
                .update([0x02])
                .finalize_new();

            shared.zeroize();
            temp_key.zeroize();

            (chaining_key, cipher_key)
        };

        // attempt to decrypt the encapsulatd key
        let mut encap_key = message[4 + 32..encap_offset].to_vec();
        ChaChaPoly::with_nonce(&cipher_key, 0u64)
            .decrypt_with_ad(&state, &mut encap_key)
            .inspect_err(|error| {
                tracing::debug!(
                    target: LOG_TARGET,
                    ?error,
                    "failed to decrypt encap key section",
                );
            })?;

        Ok(MlKemNsState {
            chaining_key,
            cipher_key,
            ml_kem_context: InboundMlKemContext::new::<R>(kind, encap_key)
                .ok_or(SessionError::EncapsulationFailure)?,
            offset: encap_offset,
            state: Sha256::new()
                .update(&state)
                .update(&message[4 + 32..encap_offset])
                .finalize_new(),
        })
    }

    /// Create inbound session from serialized `NewSession` message.
    ///
    /// <https://geti2p.net/spec/ecies#f-kdfs-for-new-session-message>
    /// <https://i2p.net/en/docs/specs/ecies-hybrid/#bob-kdf-for-ns-message>
    pub fn create_inbound_session(
        &self,
        message: Vec<u8>,
    ) -> Result<(InboundSession<R>, Vec<u8>), SessionError> {
        if message.len() < NS_MINIMUM_SIZE {
            tracing::warn!(
                target: LOG_TARGET,
                message_len = ?message.len(),
                "`NewSession` message is too short",
            );

            return Err(SessionError::Malformed);
        }

        // extract and decode elligator2-encoded public key
        let public_key = {
            // conversion must succeed as `message` has been ensured to be long enough
            // to hold the elligator2-encoded ephemeral public key
            let representative =
                TryInto::<[u8; 32]>::try_into(message[NS_EPHEMERAL_PUBKEY_OFFSET].to_vec())
                    .expect("to succeed");

            let new_pubkey = Randomized::from_representative(&representative)
                .into_option()
                .ok_or_else(|| {
                    tracing::warn!(
                        target: LOG_TARGET,
                        ?representative,
                        "failed to decode elligator2-encoded public key",
                    );

                    SessionError::Malformed
                })?
                .to_montgomery();

            StaticPublicKey::from_bytes(new_pubkey.0)
        };

        // attempt to first handle the message with ml-kem
        //
        // if the call fails, retry with regular x25519 logic
        //
        // <https://i2p.net/en/docs/specs/ecies-hybrid/#shared-tunnels>
        let (chaining_key, mut cipher_key, state, nonce, offset, ml_kem_context) = match self
            .create_inbound_session_ml_kem(&public_key, &message)
        {
            Ok(MlKemNsState {
                chaining_key,
                cipher_key,
                ml_kem_context,
                offset,
                state,
            }) => (
                chaining_key,
                cipher_key,
                state,
                1u64,
                offset,
                Some(ml_kem_context),
            ),
            Err(SessionError::EncapsulationFailure) => {
                tracing::warn!(
                    target: LOG_TARGET,
                    "failed to encapsulate key received from alice",
                );
                return Err(SessionError::EncapsulationFailure);
            }
            Err(error) => {
                // user may have specified pq-only so bail out early if x25519 is not enabled
                let Some((chaining_key, inbound_state)) = &self.inbound_context.x25519 else {
                    tracing::trace!(
                        target: LOG_TARGET,
                        ?error,
                        "x25519 not enabled, unable to handle NewSession",
                    );
                    return Err(error);
                };

                tracing::debug!(
                    target: LOG_TARGET,
                    ?error,
                    "failed to handle NewSession as ml-kem, trying x25519",
                );

                // calculate new state based on remote's ephemeral public key
                let state = Sha256::new().update(inbound_state).update(&public_key).finalize_new();

                // generate chaining key and cipher key for decrypting remote's public key
                let mut shared = self.private_key.diffie_hellman(&public_key);
                let mut temp_key = Hmac::new(chaining_key).update(&shared).finalize();
                let chaining_key = Hmac::new(&temp_key).update(b"").update([0x01]).finalize_new();
                let cipher_key = Hmac::new(&temp_key)
                    .update(chaining_key)
                    .update(b"")
                    .update([0x02])
                    .finalize_new();

                shared.zeroize();
                temp_key.zeroize();

                (chaining_key, cipher_key, state, 0u64, 36usize, None)
            }
        };

        if message.len() <= offset + NS_STATIC_PUBKEY_SECTION_SIZE {
            tracing::warn!(
                target: LOG_TARGET,
                size = ?message.len(),
                expected = ?(offset + NS_STATIC_PUBKEY_SECTION_SIZE),
                "NewSession is too short",
            );
            return Err(SessionError::Malformed);
        }

        // decrypt remote's static key and calculate new state
        let (static_key, state) = {
            let mut static_key = message[offset..offset + NS_STATIC_PUBKEY_SECTION_SIZE].to_vec();
            ChaChaPoly::with_nonce(&cipher_key, nonce)
                .decrypt_with_ad(&state, &mut static_key)
                .inspect_err(|error| {
                    tracing::debug!(
                        target: LOG_TARGET,
                        ?error,
                        "failed to decrypt static key section",
                    );
                })?;

            cipher_key.zeroize();

            (
                StaticPublicKey::try_from_bytes(&static_key).expect("to succeed"),
                Sha256::new()
                    .update(state)
                    .update(&message[offset..offset + NS_STATIC_PUBKEY_SECTION_SIZE])
                    .finalize(),
            )
        };

        // decrypt payload section
        let (chaining_key, payload) = {
            let mut shared = self.private_key.diffie_hellman(&static_key);
            let mut temp_key = Hmac::new(&chaining_key).update(&shared).finalize();
            let chaining_key = Hmac::new(&temp_key).update(b"").update([0x01]).finalize();
            let mut cipher_key =
                Hmac::new(&temp_key).update(&chaining_key).update(b"").update([0x02]).finalize();

            let mut payload = message[offset + NS_STATIC_PUBKEY_SECTION_SIZE..].to_vec();
            ChaChaPoly::with_nonce(&cipher_key, 0u64)
                .decrypt_with_ad(&state, &mut payload)
                .inspect_err(|error| {
                    tracing::debug!(
                        target: LOG_TARGET,
                        ?error,
                        "failed decrypt payload section",
                    );
                })?;

            shared.zeroize();
            temp_key.zeroize();
            cipher_key.zeroize();

            (chaining_key, payload)
        };

        Ok((
            InboundSession::new(
                static_key,
                public_key,
                chaining_key,
                Sha256::new()
                    .update(&state)
                    .update(&message[offset + NS_STATIC_PUBKEY_SECTION_SIZE..])
                    .finalize(),
                ml_kem_context,
            ),
            payload,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::mock::MockRuntime;

    fn extract_public_key(message: &[u8]) -> StaticPublicKey {
        let representative =
            TryInto::<[u8; 32]>::try_into(message[NS_EPHEMERAL_PUBKEY_OFFSET].to_vec()).unwrap();

        let new_pubkey = Randomized::from_representative(&representative)
            .into_option()
            .unwrap()
            .to_montgomery();

        StaticPublicKey::from_bytes(new_pubkey.0)
    }

    #[test]
    fn ml_kem_512_x25519() {
        let outbound_private_key = StaticPrivateKey::random(MockRuntime::rng());
        let mut outbound_key_context = KeyContext::<MockRuntime>::from_keys(
            outbound_private_key.clone(),
            vec![
                outbound_private_key.public(),
                StaticPublicKey::try_from_bytes_ml_kem_512(outbound_private_key.public().as_ref())
                    .unwrap(),
            ],
        );
        let inbound_private_key = StaticPrivateKey::random(MockRuntime::rng());
        let inbound_key_context = KeyContext::<MockRuntime>::from_keys(
            inbound_private_key.clone(),
            vec![
                inbound_private_key.public(),
                StaticPublicKey::try_from_bytes_ml_kem_512(inbound_private_key.public().as_ref())
                    .unwrap(),
            ],
        );

        // create outbound session with dummy data
        let (_, message) = outbound_key_context.create_outbound_session(
            DestinationId::random(),
            DestinationId::random(),
            // force x25519
            &StaticPublicKey::try_from_bytes(inbound_key_context.public_key.as_ref()).unwrap(),
            Bytes::new(),
            &[],
        );
        let mut out = BytesMut::with_capacity(4 + message.len());
        out.put_u32(message.len() as u32);
        out.put_slice(&message);

        // verify that the message is rejected as too short
        match inbound_key_context.create_inbound_session_ml_kem(&extract_public_key(&out), &out) {
            Err(SessionError::TooShortForMlKem) => {}
            _ => panic!("unexpected result"),
        }

        // verify that fallback to x25519 works
        assert!(inbound_key_context.create_inbound_session(out.to_vec()).is_ok());
    }

    #[test]
    fn ml_kem_512_ml_kem_512() {
        let outbound_private_key = StaticPrivateKey::random_ml_kem_512(MockRuntime::rng());
        let mut outbound_key_context = KeyContext::<MockRuntime>::from_keys(
            outbound_private_key.clone(),
            vec![outbound_private_key.public()],
        );
        let inbound_private_key = StaticPrivateKey::random_ml_kem_512(MockRuntime::rng());
        let inbound_key_context = KeyContext::<MockRuntime>::from_keys(
            inbound_private_key.clone(),
            vec![inbound_private_key.public()],
        );

        // create outbound session with dummy data
        let (_, message) = outbound_key_context.create_outbound_session(
            DestinationId::random(),
            DestinationId::random(),
            &inbound_key_context.public_key,
            Bytes::new(),
            &[],
        );
        let mut out = BytesMut::with_capacity(4 + message.len());
        out.put_u32(message.len() as u32);
        out.put_slice(&message);

        // verify the encap key section is parsed correctly
        let public_key = extract_public_key(&out);
        assert!(inbound_key_context.create_inbound_session_ml_kem(&public_key, &out).is_ok());
        assert!(inbound_key_context.create_inbound_session(out.to_vec()).is_ok());
    }

    #[test]
    fn ml_kem_768_x25519() {
        let outbound_private_key = StaticPrivateKey::random(MockRuntime::rng());
        let mut outbound_key_context = KeyContext::<MockRuntime>::from_keys(
            outbound_private_key.clone(),
            vec![
                outbound_private_key.public(),
                StaticPublicKey::try_from_bytes_ml_kem_768(outbound_private_key.public().as_ref())
                    .unwrap(),
            ],
        );
        let inbound_private_key = StaticPrivateKey::random(MockRuntime::rng());
        let inbound_key_context = KeyContext::<MockRuntime>::from_keys(
            inbound_private_key.clone(),
            vec![
                inbound_private_key.public(),
                StaticPublicKey::try_from_bytes_ml_kem_768(inbound_private_key.public().as_ref())
                    .unwrap(),
            ],
        );

        // create outbound session with dummy data
        let (_, message) = outbound_key_context.create_outbound_session(
            DestinationId::random(),
            DestinationId::random(),
            // force x25519
            &StaticPublicKey::try_from_bytes(inbound_key_context.public_key.as_ref()).unwrap(),
            Bytes::new(),
            &[],
        );
        let mut out = BytesMut::with_capacity(4 + message.len());
        out.put_u32(message.len() as u32);
        out.put_slice(&message);

        // verify that the message is rejected as too short
        match inbound_key_context.create_inbound_session_ml_kem(&extract_public_key(&out), &out) {
            Err(SessionError::TooShortForMlKem) => {}
            _ => panic!("unexpected result"),
        }

        // verify that fallback to x25519 works
        assert!(inbound_key_context.create_inbound_session(out.to_vec()).is_ok());
    }

    #[test]
    fn ml_kem_768_ml_kem_768() {
        let outbound_private_key = StaticPrivateKey::random_ml_kem_768(MockRuntime::rng());
        let mut outbound_key_context = KeyContext::<MockRuntime>::from_keys(
            outbound_private_key.clone(),
            vec![outbound_private_key.public()],
        );
        let inbound_private_key = StaticPrivateKey::random_ml_kem_768(MockRuntime::rng());
        let inbound_key_context = KeyContext::<MockRuntime>::from_keys(
            inbound_private_key.clone(),
            vec![inbound_private_key.public()],
        );

        // create outbound session with dummy data
        let (_, message) = outbound_key_context.create_outbound_session(
            DestinationId::random(),
            DestinationId::random(),
            &inbound_key_context.public_key,
            Bytes::new(),
            &[],
        );
        let mut out = BytesMut::with_capacity(4 + message.len());
        out.put_u32(message.len() as u32);
        out.put_slice(&message);

        // verify the encap key section is parsed correctly
        let public_key = extract_public_key(&out);
        assert!(inbound_key_context.create_inbound_session_ml_kem(&public_key, &out).is_ok());
        assert!(inbound_key_context.create_inbound_session(out.to_vec()).is_ok());
    }

    #[test]
    fn ml_kem_1024_x25519() {
        let outbound_private_key = StaticPrivateKey::random(MockRuntime::rng());
        let mut outbound_key_context = KeyContext::<MockRuntime>::from_keys(
            outbound_private_key.clone(),
            vec![
                outbound_private_key.public(),
                StaticPublicKey::try_from_bytes_ml_kem_1024(outbound_private_key.public().as_ref())
                    .unwrap(),
            ],
        );
        let inbound_private_key = StaticPrivateKey::random(MockRuntime::rng());
        let inbound_key_context = KeyContext::<MockRuntime>::from_keys(
            inbound_private_key.clone(),
            vec![
                inbound_private_key.public(),
                StaticPublicKey::try_from_bytes_ml_kem_1024(inbound_private_key.public().as_ref())
                    .unwrap(),
            ],
        );

        // create outbound session with dummy data
        let (_, message) = outbound_key_context.create_outbound_session(
            DestinationId::random(),
            DestinationId::random(),
            // force x25519
            &StaticPublicKey::try_from_bytes(inbound_key_context.public_key.as_ref()).unwrap(),
            Bytes::new(),
            &[],
        );
        let mut out = BytesMut::with_capacity(4 + message.len());
        out.put_u32(message.len() as u32);
        out.put_slice(&message);

        // verify that the message is rejected as too short
        match inbound_key_context.create_inbound_session_ml_kem(&extract_public_key(&out), &out) {
            Err(SessionError::TooShortForMlKem) => {}
            _ => panic!("unexpected result"),
        }

        // verify that fallback to x25519 works
        assert!(inbound_key_context.create_inbound_session(out.to_vec()).is_ok());
    }

    #[test]
    fn ml_kem_1024_ml_kem_1024() {
        let outbound_private_key = StaticPrivateKey::random_ml_kem_1024(MockRuntime::rng());
        let mut outbound_key_context = KeyContext::<MockRuntime>::from_keys(
            outbound_private_key.clone(),
            vec![outbound_private_key.public()],
        );
        let inbound_private_key = StaticPrivateKey::random_ml_kem_1024(MockRuntime::rng());
        let inbound_key_context = KeyContext::<MockRuntime>::from_keys(
            inbound_private_key.clone(),
            vec![inbound_private_key.public()],
        );

        // create outbound session with dummy data
        let (_, message) = outbound_key_context.create_outbound_session(
            DestinationId::random(),
            DestinationId::random(),
            &inbound_key_context.public_key,
            Bytes::new(),
            &[],
        );
        let mut out = BytesMut::with_capacity(4 + message.len());
        out.put_u32(message.len() as u32);
        out.put_slice(&message);

        // verify the encap key section is parsed correctly
        let public_key = extract_public_key(&out);
        assert!(inbound_key_context.create_inbound_session_ml_kem(&public_key, &out).is_ok());
    }

    #[test]
    fn large_ns_ml_kem_512() {
        let outbound_private_key = StaticPrivateKey::random(MockRuntime::rng());
        let mut outbound_key_context = KeyContext::<MockRuntime>::from_keys(
            outbound_private_key.clone(),
            vec![
                outbound_private_key.public(),
                StaticPublicKey::try_from_bytes_ml_kem_512(outbound_private_key.public().as_ref())
                    .unwrap(),
            ],
        );
        let inbound_private_key = StaticPrivateKey::random(MockRuntime::rng());
        let inbound_key_context = KeyContext::<MockRuntime>::from_keys(
            inbound_private_key.clone(),
            vec![
                inbound_private_key.public(),
                StaticPublicKey::try_from_bytes_ml_kem_512(inbound_private_key.public().as_ref())
                    .unwrap(),
            ],
        );

        // create outbound session with dummy data
        let (_, message) = outbound_key_context.create_outbound_session(
            DestinationId::random(),
            DestinationId::random(),
            &StaticPublicKey::try_from_bytes(inbound_key_context.public_key.as_ref()).unwrap(),
            Bytes::new(),
            &vec![0xaa; constants::ml_kem_512::KEY_SIZE + 16],
        );
        let mut out = BytesMut::with_capacity(4 + message.len());
        out.put_u32(message.len() as u32);
        out.put_slice(&message);

        // verify that the message is rejected as too short
        match inbound_key_context.create_inbound_session_ml_kem(&extract_public_key(&out), &out) {
            Err(SessionError::Chacha) => {}
            _ => panic!("unexpected result"),
        }

        // verify that fallback to x25519 works
        assert!(inbound_key_context.create_inbound_session(out.to_vec()).is_ok());
    }

    #[test]
    fn large_ns_ml_kem_768() {
        let outbound_private_key = StaticPrivateKey::random(MockRuntime::rng());
        let mut outbound_key_context = KeyContext::<MockRuntime>::from_keys(
            outbound_private_key.clone(),
            vec![
                outbound_private_key.public(),
                StaticPublicKey::try_from_bytes_ml_kem_768(outbound_private_key.public().as_ref())
                    .unwrap(),
            ],
        );
        let inbound_private_key = StaticPrivateKey::random(MockRuntime::rng());
        let inbound_key_context = KeyContext::<MockRuntime>::from_keys(
            inbound_private_key.clone(),
            vec![
                inbound_private_key.public(),
                StaticPublicKey::try_from_bytes_ml_kem_768(inbound_private_key.public().as_ref())
                    .unwrap(),
            ],
        );

        // create outbound session with dummy data
        let (_, message) = outbound_key_context.create_outbound_session(
            DestinationId::random(),
            DestinationId::random(),
            &StaticPublicKey::try_from_bytes(inbound_key_context.public_key.as_ref()).unwrap(),
            Bytes::new(),
            &vec![0xaa; constants::ml_kem_768::KEY_SIZE + 16],
        );
        let mut out = BytesMut::with_capacity(4 + message.len());
        out.put_u32(message.len() as u32);
        out.put_slice(&message);

        // verify that the message is rejected as too short
        match inbound_key_context.create_inbound_session_ml_kem(&extract_public_key(&out), &out) {
            Err(SessionError::Chacha) => {}
            _ => panic!("unexpected result"),
        }

        // verify that fallback to x25519 works
        assert!(inbound_key_context.create_inbound_session(out.to_vec()).is_ok());
    }

    #[test]
    fn large_ns_ml_kem_1024() {
        let outbound_private_key = StaticPrivateKey::random(MockRuntime::rng());
        let mut outbound_key_context = KeyContext::<MockRuntime>::from_keys(
            outbound_private_key.clone(),
            vec![
                outbound_private_key.public(),
                StaticPublicKey::try_from_bytes_ml_kem_1024(outbound_private_key.public().as_ref())
                    .unwrap(),
            ],
        );
        let inbound_private_key = StaticPrivateKey::random(MockRuntime::rng());
        let inbound_key_context = KeyContext::<MockRuntime>::from_keys(
            inbound_private_key.clone(),
            vec![
                inbound_private_key.public(),
                StaticPublicKey::try_from_bytes_ml_kem_1024(inbound_private_key.public().as_ref())
                    .unwrap(),
            ],
        );

        // create outbound session with dummy data
        let (_, message) = outbound_key_context.create_outbound_session(
            DestinationId::random(),
            DestinationId::random(),
            &StaticPublicKey::try_from_bytes(inbound_key_context.public_key.as_ref()).unwrap(),
            Bytes::new(),
            &vec![0xaa; constants::ml_kem_1024::KEY_SIZE + 16],
        );
        let mut out = BytesMut::with_capacity(4 + message.len());
        out.put_u32(message.len() as u32);
        out.put_slice(&message);

        // verify that the message is rejected as too short
        match inbound_key_context.create_inbound_session_ml_kem(&extract_public_key(&out), &out) {
            Err(SessionError::Chacha) => {}
            _ => panic!("unexpected result"),
        }

        // verify that fallback to x25519 works
        assert!(inbound_key_context.create_inbound_session(out.to_vec()).is_ok());
    }
}
