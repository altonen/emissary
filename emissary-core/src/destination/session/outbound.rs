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

//! Outbound ECIES-X25519-AEAD-Ratchet session implementation.

use crate::{
    crypto::{
        chachapoly::ChaChaPoly, hmac::Hmac, sha256::Sha256, StaticPrivateKey, StaticPublicKey,
    },
    destination::session::tag_set::{TagSet, TagSetEntry},
    error::SessionError,
    primitives::DestinationId,
    runtime::Runtime,
};

use bytes::Bytes;
use curve25519_elligator2::{MapToPointVariant, Randomized};
use ml_kem::{Decapsulate, DecapsulationKey, MlKem1024, MlKem512, MlKem768};
use zeroize::Zeroize;

use alloc::{boxed::Box, vec::Vec};
use core::{fmt, marker::PhantomData, ops::Range};

/// Logging target for the file.
const LOG_TARGET: &str = "emissary::destination::session::outbound";

/// Number of tags to generate for `NewSessionReply`.
const NSR_TAG_COUNT: usize = 16usize;

/// Minimum length for `NewSessionReply` message.
const NSR_MINIMUM_LEN: usize = 60usize;

/// Ephemeral public key offset in `NewSessionReply` message.
const NSR_EPHEMERAL_PUBKEY_OFFSET: Range<usize> = 12..44;

/// Index for the endo of the ephemeral public key offset in `NewSessionReply` message.
const NSR_EPHEMERAL_PUBKEY_SECTION_END: usize = 44;

/// Poly1305 MAC section size in `NewSessionReply` message.
const NSR_POLY1305_SECTION_SIZE: usize = 16usize;

/// Poly1305 MAC size.
const POLY1035_MAC_SIZE: usize = 16usize;

/// ML-KEM context for hybrid PQ ratchet.
pub enum MlKemContext {
    /// ML-KEM-512-x25519 ECIES-Rachet.
    MlKem512X25519(Box<DecapsulationKey<MlKem512>>),

    /// ML-KEM-768-x25519 ECIES-Rachet.
    MlKem768X25519(Box<DecapsulationKey<MlKem768>>),

    /// ML-KEM-1024-x25519 ECIES-Rachet.
    MlKem1024X25519(Box<DecapsulationKey<MlKem1024>>),
}

impl MlKemContext {
    /// Get KEM ciphertext size.
    fn ciphertext_size(&self) -> usize {
        match self {
            Self::MlKem512X25519(_) => 768,
            Self::MlKem768X25519(_) => 1088,
            Self::MlKem1024X25519(_) => 1568,
        }
    }

    /// Attempt to calculate shared key.
    ///
    /// Returns `None` if decapsulation fails.
    fn decapsulate(&self, ciphertext: &[u8]) -> Option<Vec<u8>> {
        match self {
            Self::MlKem512X25519(decap_key) =>
                decap_key.decapsulate_slice(ciphertext).ok().map(|key| key.0.to_vec()),
            Self::MlKem768X25519(decap_key) =>
                decap_key.decapsulate_slice(ciphertext).ok().map(|key| key.0.to_vec()),
            Self::MlKem1024X25519(decap_key) =>
                decap_key.decapsulate_slice(ciphertext).ok().map(|key| key.0.to_vec()),
        }
    }
}

impl fmt::Display for MlKemContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MlKem512X25519(_) => write!(f, "ml-kem-512-x25519"),
            Self::MlKem768X25519(_) => write!(f, "ml-kem-768-x25519"),
            Self::MlKem1024X25519(_) => write!(f, "ml-kem-1024-x25519"),
        }
    }
}

/// Outbound session.
pub struct OutboundSession<R: Runtime> {
    /// Chaining key.
    pub chaining_key: Vec<u8>,

    /// Destination ID.
    pub destination_id: DestinationId,

    /// Ephemeral private key.
    pub ephemeral_private_key: StaticPrivateKey,

    /// ML-KEM context.
    ///
    /// `None` for x25519.
    pub ml_kem_context: Option<MlKemContext>,

    /// State (`h` from the specification).
    pub state: Bytes,

    /// Static private key.
    pub static_private_key: StaticPrivateKey,

    /// Marker for `Runtime`.
    pub _runtime: PhantomData<R>,
}

impl<R: Runtime> OutboundSession<R> {
    /// Create new [`OutboundSession`].
    pub fn new(
        destination_id: DestinationId,
        state: Bytes,
        static_private_key: StaticPrivateKey,
        ephemeral_private_key: StaticPrivateKey,
        chaining_key: Vec<u8>,
        ml_kem_context: Option<MlKemContext>,
    ) -> Self {
        Self {
            chaining_key,
            destination_id,
            ephemeral_private_key,
            ml_kem_context,
            state,
            static_private_key,
            _runtime: Default::default(),
        }
    }

    /// Generate garlic tags for the incoming `NewSessionReply`
    ///
    /// This function can only be called once, after the outbound session has been initialized and
    /// its state is `OutboundSessionPending`, for other states the call will panic.
    pub fn generate_new_session_reply_tags(
        &self,
        ratchet_threshold: u16,
    ) -> impl Iterator<Item = TagSetEntry> {
        let mut temp_key = Hmac::new(&self.chaining_key).update([]).finalize();
        let tag_set_key =
            Hmac::new(&temp_key).update(b"SessionReplyTags").update([0x01]).finalize();

        let mut nsr_tag_set = TagSet::new(&self.chaining_key, tag_set_key, ratchet_threshold);

        temp_key.zeroize();

        (0..NSR_TAG_COUNT).map(move |_| nsr_tag_set.next_entry().expect("to succeed"))
    }

    /// Handle `NewSessionReply` from remote destination.
    ///
    /// Decrypt `message` using `tag_set_entry`, derive send and receive tag sets
    /// and return the parsed inner payload of `message`.
    ///
    /// Session is considered active after this function has returned successfully.
    ///
    /// https://geti2p.net/spec/ecies#kdf-for-flags-static-key-section-encrypted-contents
    pub fn handle_new_session_reply(
        &mut self,
        tag_set_entry: TagSetEntry,
        message: Vec<u8>,
        ratchet_threshold: u16,
    ) -> Result<(Vec<u8>, TagSet, TagSet), SessionError> {
        if message.len() < NSR_MINIMUM_LEN {
            tracing::warn!(
                target: LOG_TARGET,
                remote = %self.destination_id,
                payload_len = ?message.len(),
                "`NewSessionReply` is too short",
            );
            debug_assert!(false);

            return Err(SessionError::Malformed);
        }

        // extract and decode elligator2-encoded public key of the remote destination
        let public_key = {
            // conversion must succeed since the provided range is correct and
            // the payload has been confirmed to be large enough to hold the public key
            let public_key = TryInto::<[u8; 32]>::try_into(&message[NSR_EPHEMERAL_PUBKEY_OFFSET])
                .expect("to succeed");
            let new_pubkey = Randomized::from_representative(&public_key)
                .into_option()
                .ok_or(SessionError::Malformed)?
                .to_montgomery()
                .to_bytes();

            StaticPublicKey::from_bytes(new_pubkey)
        };

        // calculate new state with garlic tag & remote's ephemeral public key
        let state = {
            let state = Sha256::new()
                .update(&self.state)
                .update(tag_set_entry.tag.to_le_bytes())
                .finalize();

            Sha256::new().update(&state).update::<&[u8]>(public_key.as_ref()).finalize()
        };

        // calculate keys from shared secrets derived from ee & es
        //
        // ephemeral-ephemeral
        let mut shared = self.ephemeral_private_key.diffie_hellman(&public_key);
        let mut temp_key = Hmac::new(&self.chaining_key).update(&shared).finalize();
        let chaining_key = Hmac::new(&temp_key).update(b"").update([0x01]).finalize();
        let mut keydata =
            Hmac::new(&temp_key).update(&chaining_key).update(b"").update([0x02]).finalize();

        // ekem1
        //
        // <https://i2p.net/en/docs/specs/ecies-hybrid/#bob-kdf-for-nsr-message>
        let (mut chaining_key, state, offset) = match &self.ml_kem_context {
            None => (chaining_key, state, NSR_EPHEMERAL_PUBKEY_SECTION_END),
            Some(context) => {
                if message.len() <= context.ciphertext_size() {
                    tracing::warn!(
                        target: LOG_TARGET,
                        remote = %self.destination_id,
                        kind = %context,
                        "NSR is too short",
                    );
                    return Err(SessionError::Malformed);
                }

                tracing::trace!(
                    target: LOG_TARGET,
                    remote = %self.destination_id,
                    kind = %context,
                    "processing ml-kem NSR",
                );

                // decrypt kem ciphertext
                let kem_ciphertext_offset = NSR_EPHEMERAL_PUBKEY_SECTION_END
                    ..NSR_EPHEMERAL_PUBKEY_SECTION_END
                        + context.ciphertext_size()
                        + POLY1035_MAC_SIZE;
                let mut kem_ciphertext = message[kem_ciphertext_offset.clone()].to_vec();

                ChaChaPoly::new(&keydata).decrypt_with_ad(&state, &mut kem_ciphertext)?;
                keydata.zeroize();

                // decapsulate and acquire shared key.
                let shared_key = context
                    .decapsulate(&kem_ciphertext)
                    .ok_or(SessionError::DecapsulationFailure)?;
                kem_ciphertext.zeroize();

                let temp_key = Hmac::new(&chaining_key).update(&shared_key).finalize_new();
                let chaining_key = Hmac::new(&temp_key).update(b"").update([0x01]).finalize();

                (
                    chaining_key,
                    Sha256::new().update(&state).update(&message[kem_ciphertext_offset]).finalize(),
                    NSR_EPHEMERAL_PUBKEY_SECTION_END
                        + context.ciphertext_size()
                        + POLY1035_MAC_SIZE,
                )
            }
        };

        // ensure the message has space for at least the NSR mac and payload's poly1305 mac
        if message.len() < offset + NSR_POLY1305_SECTION_SIZE + POLY1035_MAC_SIZE {
            tracing::warn!(
                target: LOG_TARGET,
                remote = %self.destination_id,
                "NSR is too short",
            );
            return Err(SessionError::Malformed);
        }

        // static-ephemeral
        shared = self.static_private_key.diffie_hellman(&public_key);
        temp_key = Hmac::new(&chaining_key).update(&shared).finalize();
        chaining_key = Hmac::new(&temp_key).update(b"").update([0x01]).finalize();
        let keydata =
            Hmac::new(&temp_key).update(&chaining_key).update(b"").update([0x02]).finalize();

        // poly1305 mac for the key section (empty payload)
        let mut ciphertext = message[offset..offset + NSR_POLY1305_SECTION_SIZE].to_vec();

        // payload section of the `NewSessionReply`
        let mut payload = message[offset + NSR_POLY1305_SECTION_SIZE..].to_vec();

        // verify they poly1305 mac for the key section is correct and return updated state
        let state = {
            let updated_state = Sha256::new().update(&state).update(&ciphertext).finalize();
            ChaChaPoly::new(&keydata).decrypt_with_ad(&state, &mut ciphertext)?;

            updated_state
        };

        // split key into send and receive keys
        let (send_key, recv_key) = {
            let mut temp_key = Hmac::new(&chaining_key).update([]).finalize();
            let send_key = Hmac::new(&temp_key).update([0x01]).finalize();
            let recv_key = Hmac::new(&temp_key).update(&send_key).update([0x02]).finalize();

            temp_key.zeroize();

            (send_key, recv_key)
        };

        // initialize send and receive tag sets
        let send_tag_set = TagSet::new(&chaining_key, send_key, ratchet_threshold);
        let recv_tag_set = TagSet::new(chaining_key, &recv_key, ratchet_threshold);

        // decode payload of the `NewSessionReply` message
        let mut temp_key = Hmac::new(&recv_key).update([]).finalize();
        let mut payload_key =
            Hmac::new(&temp_key).update(b"AttachPayloadKDF").update([0x01]).finalize();

        ChaChaPoly::new(&payload_key).decrypt_with_ad(&state, &mut payload)?;

        temp_key.zeroize();
        payload_key.zeroize();

        Ok((payload, send_tag_set, recv_tag_set))
    }
}
