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

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::generation_store::{GenerationStore, StoreResult};
use crate::i2pcontrol::domain::address_book::SubscriptionSet;
use crate::i2pcontrol::domain::revision::StateRevision;

/// Persistent subscription store payload.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SubscriptionStorePayload {
    /// The subscription set.
    pub subscriptions: SubscriptionSet,
}

#[allow(dead_code)]
impl SubscriptionStorePayload {
    /// Create an empty payload.
    pub fn empty() -> Self {
        Self {
            subscriptions: SubscriptionSet::new(),
        }
    }
}

/// Persistent subscription store.
///
/// Stores address book subscription URLs using versioned generation
/// persistence. Separate from the address book store for independent
/// mutation semantics.
#[allow(dead_code)]
pub struct SubscriptionStore {
    inner: GenerationStore<SubscriptionStorePayload>,
}

#[allow(dead_code)]
impl SubscriptionStore {
    /// Create a new subscription store.
    pub fn new(dir: PathBuf, max_size: usize) -> Self {
        Self {
            inner: GenerationStore::new(dir, max_size),
        }
    }

    /// Load existing state from disk.
    pub async fn load(&mut self) -> StoreResult<Option<StateRevision>> {
        self.inner.load().await
    }

    /// Return the current revision.
    pub fn revision(&self) -> StateRevision {
        self.inner.revision()
    }

    /// Get the current subscription set.
    pub fn subscriptions(&self) -> SubscriptionSet {
        self.inner.current().map(|p| p.subscriptions.clone()).unwrap_or_default()
    }

    /// Set the subscription set.
    pub async fn set(&mut self, subscriptions: SubscriptionSet) -> StoreResult<StateRevision> {
        let payload = SubscriptionStorePayload { subscriptions };
        self.inner.publish(payload, |_| Ok(())).await
    }

    /// Return the number of subscriptions.
    pub fn len(&self) -> usize {
        self.subscriptions().len()
    }

    /// Return true if the subscription set is empty.
    pub fn is_empty(&self) -> bool {
        self.subscriptions().is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn test_dir() -> PathBuf {
        tempfile::tempdir().unwrap().keep()
    }

    #[tokio::test]
    async fn empty_store() {
        let dir = test_dir();
        let mut store = SubscriptionStore::new(dir.clone(), 1024 * 1024);
        let loaded = store.load().await.unwrap();
        assert_eq!(loaded, None);
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn set_and_get() {
        let dir = test_dir();
        let mut store = SubscriptionStore::new(dir.clone(), 1024 * 1024);

        let mut subs = SubscriptionSet::new();
        subs.push("http://sub1.example.com".to_string());
        subs.push("http://sub2.example.com".to_string());
        store.set(subs).await.unwrap();

        assert_eq!(store.len(), 2);
        assert!(store.subscriptions().contains("http://sub1.example.com"));
        assert!(store.subscriptions().contains("http://sub2.example.com"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn round_trip_persistence() {
        let dir = test_dir();
        {
            let mut store = SubscriptionStore::new(dir.clone(), 1024 * 1024);
            let mut subs = SubscriptionSet::new();
            subs.push("http://a.example.com".to_string());
            subs.push("http://b.example.com".to_string());
            subs.push("http://c.example.com".to_string());
            store.set(subs).await.unwrap();
        }

        {
            let mut store = SubscriptionStore::new(dir.clone(), 1024 * 1024);
            let loaded = store.load().await.unwrap();
            assert!(loaded.is_some());
            assert_eq!(store.len(), 3);
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn replace_subscriptions() {
        let dir = test_dir();
        let mut store = SubscriptionStore::new(dir.clone(), 1024 * 1024);

        let mut subs1 = SubscriptionSet::new();
        subs1.push("http://old.example.com".to_string());
        store.set(subs1).await.unwrap();
        assert_eq!(store.len(), 1);

        let mut subs2 = SubscriptionSet::new();
        subs2.push("http://new.example.com".to_string());
        store.set(subs2).await.unwrap();
        assert_eq!(store.len(), 1);
        assert!(!store.subscriptions().contains("http://old.example.com"));
        assert!(store.subscriptions().contains("http://new.example.com"));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
