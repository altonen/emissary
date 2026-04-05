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

use crate::crypto::{hmac::Hmac, sha256::Sha256, SecretKey};

use zeroize::Zeroize;

/// Noise context.
#[derive(Clone)]
pub struct NoiseContext {
    /// Chaining key.
    chaining_key: [u8; 32],

    /// AEAD state.
    state: [u8; 32],
}

impl Drop for NoiseContext {
    fn drop(&mut self) {
        self.chaining_key.zeroize();
        self.state.zeroize();
    }
}

impl NoiseContext {
    /// Create new [`NoiseContext`].
    pub fn new(chaining_key: [u8; 32], state: [u8; 32]) -> Self {
        Self {
            chaining_key,
            state,
        }
    }

    /// Get reference to chaining key.
    pub fn chaining_key(&self) -> &[u8] {
        &self.chaining_key
    }

    /// Get reference to AEAD state.
    pub fn state(&self) -> &[u8] {
        &self.state
    }

    /// Performn `MixHash()` for `input`
    pub fn mix_hash(&mut self, input: impl AsRef<[u8]>) -> &mut Self {
        self.state = Sha256::new().update(self.state).update(input).finalize_new();
        self
    }

    /// Perform `MixKey()` with `secret_key` and `public_key`.
    pub fn mix_key<S: SecretKey, P: AsRef<x25519_dalek::PublicKey>>(
        &mut self,
        secret_key: &S,
        public_key: &P,
    ) -> [u8; 32] {
        self.mix_key_from_shared_secret(&secret_key.diffie_hellman(public_key))
    }

    /// Perform `MixKey()` from pre-computed shared secet
    //
    // TODO: merge this and `Self::mix_key()`
    pub fn mix_key_from_shared_secret(&mut self, shared_secret: &[u8]) -> [u8; 32] {
        let mut temp_key = Hmac::new(&self.chaining_key).update(shared_secret).finalize_new();
        self.chaining_key = Hmac::new(&temp_key).update([0x01]).finalize_new();
        let key = Hmac::new(&temp_key).update(self.chaining_key).update([0x02]).finalize_new();

        temp_key.zeroize();

        key
    }
}
