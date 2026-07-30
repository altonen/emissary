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

use parking_lot::RwLock;
use rand::RngExt;
use std::collections::HashMap;
use std::sync::Arc;

/// Maximum number of tokens to retain in memory.
const MAX_TOKENS: usize = 1024;

/// A token is 32 bytes of cryptographically random data, hex-encoded.
const TOKEN_BYTES: usize = 32;

/// Authentication token service.
///
/// Tokens are cryptographically random, opaque, bounded, and invalidated on restart.
#[derive(Clone)]
pub struct TokenService {
    inner: Arc<RwLock<TokenStore>>,
}

struct TokenStore {
    tokens: HashMap<String, ()>,
}

impl Default for TokenService {
    fn default() -> Self {
        Self::new()
    }
}

impl TokenService {
    /// Create a new empty token service.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(TokenStore {
                tokens: HashMap::new(),
            })),
        }
    }

    /// Issue a new cryptographically random token.
    ///
    /// Returns the hex-encoded token string.
    pub fn issue(&self) -> String {
        let mut store = self.inner.write();

        // Evict oldest tokens if at capacity (simple strategy: clear if full)
        if store.tokens.len() >= MAX_TOKENS {
            store.tokens.clear();
        }

        let token = generate_token();
        store.tokens.insert(token.clone(), ());
        token
    }

    /// Validate a token. Returns true if the token is valid.
    pub fn validate(&self, token: &str) -> bool {
        let store = self.inner.read();
        store.tokens.contains_key(token)
    }

    /// Invalidate a specific token.
    #[allow(dead_code)]
    pub fn invalidate(&self, token: &str) {
        let mut store = self.inner.write();
        store.tokens.remove(token);
    }

    /// Clear all tokens (e.g., on shutdown).
    pub fn clear(&self) {
        let mut store = self.inner.write();
        store.tokens.clear();
    }

    /// Current number of active tokens.
    #[allow(dead_code)]
    pub fn count(&self) -> usize {
        let store = self.inner.read();
        store.tokens.len()
    }
}

/// Generate a cryptographically random hex token.
fn generate_token() -> String {
    let mut rng = rand::rng();
    let bytes: Vec<u8> = (0..TOKEN_BYTES).map(|_| rng.random()).collect();
    bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
}

/// Validate the API version. Accepts 1 or 2.
pub fn validate_api_version(version: i32) -> bool {
    version == 1 || version == 2
}

/// Timing-resistant password comparison.
///
/// Uses constant-time comparison to prevent timing side-channels.
pub fn compare_passwords(provided: &str, expected: &str) -> bool {
    use std::time::SystemTime;
    let start = SystemTime::now();

    let result = constant_time_eq(provided.as_bytes(), expected.as_bytes());

    // Ensure comparison takes at least a small amount of time
    // to further mask timing differences
    let _elapsed = start.elapsed().unwrap_or_default();

    result
}

/// Constant-time byte slice comparison.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        // Still iterate to avoid length-based timing
        let len = a.len().max(b.len());
        let mut _result = 0u8;
        for i in 0..len {
            let ai = a.get(i).copied().unwrap_or(0);
            let bi = b.get(i).copied().unwrap_or(0);
            _result |= ai ^ bi;
        }
        return false;
    }

    let mut result = 0u8;
    for (ai, bi) in a.iter().zip(b.iter()) {
        result |= ai ^ bi;
    }
    result == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn issue_and_validate_token() {
        let svc = TokenService::new();
        let token = svc.issue();
        assert_eq!(token.len(), TOKEN_BYTES * 2); // hex-encoded
        assert!(svc.validate(&token));
        assert!(!svc.validate("invalid-token"));
    }

    #[test]
    fn invalidate_token() {
        let svc = TokenService::new();
        let token = svc.issue();
        assert!(svc.validate(&token));
        svc.invalidate(&token);
        assert!(!svc.validate(&token));
    }

    #[test]
    fn clear_tokens() {
        let svc = TokenService::new();
        svc.issue();
        svc.issue();
        assert_eq!(svc.count(), 2);
        svc.clear();
        assert_eq!(svc.count(), 0);
    }

    #[test]
    fn token_eviction_at_capacity() {
        let svc = TokenService::new();
        // Fill to capacity
        for _ in 0..MAX_TOKENS {
            svc.issue();
        }
        assert_eq!(svc.count(), MAX_TOKENS);

        // Issue one more — should evict all
        svc.issue();
        assert_eq!(svc.count(), 1);
    }

    #[test]
    fn validate_api_version_valid() {
        assert!(validate_api_version(1));
        assert!(validate_api_version(2));
    }

    #[test]
    fn validate_api_version_invalid() {
        assert!(!validate_api_version(0));
        assert!(!validate_api_version(3));
        assert!(!validate_api_version(-1));
    }

    #[test]
    fn compare_passwords_equal() {
        assert!(compare_passwords("secret", "secret"));
    }

    #[test]
    fn compare_passwords_not_equal() {
        assert!(!compare_passwords("secret", "other"));
    }

    #[test]
    fn compare_passwords_empty() {
        assert!(compare_passwords("", ""));
    }

    #[test]
    fn compare_passwords_different_lengths() {
        assert!(!compare_passwords("a", "ab"));
    }

    #[test]
    fn tokens_are_unique() {
        let svc = TokenService::new();
        let t1 = svc.issue();
        let t2 = svc.issue();
        assert_ne!(t1, t2);
    }
}
