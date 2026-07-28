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

use std::fmt;

use serde::{Deserialize, Serialize};

/// A monotonically increasing per-store revision counter.
///
/// Used for serialized mutation ordering and test evidence. Revisions start
/// at 0 for an empty store and increase with each committed mutation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct StateRevision(u64);

impl StateRevision {
    /// The initial revision for an empty store.
    pub const ZERO: Self = Self(0);

    /// Create a revision from a raw value.
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Return the raw revision value.
    pub const fn value(self) -> u64 {
        self.0
    }

    /// Advance to the next revision.
    pub fn next(self) -> Self {
        Self(self.0 + 1)
    }
}

impl Default for StateRevision {
    fn default() -> Self {
        Self::ZERO
    }
}

impl fmt::Display for StateRevision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<u64> for StateRevision {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl From<StateRevision> for u64 {
    fn from(revision: StateRevision) -> Self {
        revision.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn revision_zero_is_default() {
        assert_eq!(StateRevision::default(), StateRevision::ZERO);
        assert_eq!(StateRevision::ZERO.value(), 0);
    }

    #[test]
    fn revision_next_increments() {
        let r1 = StateRevision::ZERO;
        let r2 = r1.next();
        assert_eq!(r2.value(), 1);
        assert!(r2 > r1);
    }

    #[test]
    fn revision_ordering() {
        let r0 = StateRevision::ZERO;
        let r1 = r0.next();
        let r2 = r1.next();
        assert!(r0 < r1);
        assert!(r1 < r2);
        assert!(r0 < r2);
    }

    #[test]
    fn revision_serialization_roundtrip() {
        let r = StateRevision::new(42);
        let json = serde_json::to_string(&r).unwrap();
        let deserialized: StateRevision = serde_json::from_str(&json).unwrap();
        assert_eq!(r, deserialized);
    }

    #[test]
    fn revision_display() {
        assert_eq!(StateRevision::ZERO.to_string(), "0");
        assert_eq!(StateRevision::new(99).to_string(), "99");
    }

    #[test]
    fn revision_from_u64() {
        let r: StateRevision = 5u64.into();
        assert_eq!(r.value(), 5);
    }

    #[test]
    fn revision_into_u64() {
        let r = StateRevision::new(7);
        let v: u64 = r.into();
        assert_eq!(v, 7);
    }
}
