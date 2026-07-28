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

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::generation_store::{GenerationStore, StoreResult};
use crate::i2pcontrol::domain::address_book::{
    AddressBookConfiguration, AddressBookEntry, AdministrativeAddressBookType, SubscriptionSet,
};
use crate::i2pcontrol::domain::revision::StateRevision;

/// Persistent address book store payload.
///
/// Stores all four administrative books as independent maps plus
/// subscriptions and configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AddressBookStorePayload {
    /// Private address book entries.
    pub private: BTreeMap<String, AddressBookEntry>,

    /// Local address book entries.
    pub local: BTreeMap<String, AddressBookEntry>,

    /// Router address book entries.
    pub router: BTreeMap<String, AddressBookEntry>,

    /// Published address book entries.
    pub published: BTreeMap<String, AddressBookEntry>,

    /// Subscription set.
    pub subscriptions: SubscriptionSet,

    /// Address book configuration.
    pub configuration: AddressBookConfiguration,
}

impl AddressBookStorePayload {
    /// Create an empty payload.
    pub fn empty() -> Self {
        Self {
            private: BTreeMap::new(),
            local: BTreeMap::new(),
            router: BTreeMap::new(),
            published: BTreeMap::new(),
            subscriptions: SubscriptionSet::new(),
            configuration: AddressBookConfiguration::new(),
        }
    }

    /// Get the map for a given book type.
    pub fn book(
        &self,
        book_type: AdministrativeAddressBookType,
    ) -> &BTreeMap<String, AddressBookEntry> {
        match book_type {
            AdministrativeAddressBookType::Private => &self.private,
            AdministrativeAddressBookType::Local => &self.local,
            AdministrativeAddressBookType::Router => &self.router,
            AdministrativeAddressBookType::Published => &self.published,
        }
    }

    /// Get a mutable map for a given book type.
    pub fn book_mut(
        &mut self,
        book_type: AdministrativeAddressBookType,
    ) -> &mut BTreeMap<String, AddressBookEntry> {
        match book_type {
            AdministrativeAddressBookType::Private => &mut self.private,
            AdministrativeAddressBookType::Local => &mut self.local,
            AdministrativeAddressBookType::Router => &mut self.router,
            AdministrativeAddressBookType::Published => &mut self.published,
        }
    }
}

/// Persistent address book store.
///
/// Stores all four Proposal 170 administrative address books, subscriptions,
/// and configuration using versioned generation persistence.
pub struct AddressBookStore {
    inner: GenerationStore<AddressBookStorePayload>,
}

impl AddressBookStore {
    /// Create a new address book store.
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

    /// List all entries in a given book.
    pub fn list(&self, book_type: AdministrativeAddressBookType) -> Vec<&AddressBookEntry> {
        self.inner
            .current()
            .map(|p| p.book(book_type).values().collect())
            .unwrap_or_default()
    }

    /// Look up an entry by hostname in a given book.
    pub fn lookup(
        &self,
        book_type: AdministrativeAddressBookType,
        hostname: &str,
    ) -> Option<&AddressBookEntry> {
        self.inner.current().and_then(|p| p.book(book_type).get(hostname))
    }

    /// Add an entry to a given book.
    pub async fn add(
        &mut self,
        book_type: AdministrativeAddressBookType,
        entry: AddressBookEntry,
    ) -> StoreResult<StateRevision> {
        let mut payload =
            self.inner.current().cloned().unwrap_or_else(AddressBookStorePayload::empty);
        payload.book_mut(book_type).insert(entry.hostname.clone(), entry);
        self.inner.publish(payload, |_| Ok(())).await
    }

    /// Update an existing entry in a given book.
    pub async fn update(
        &mut self,
        book_type: AdministrativeAddressBookType,
        entry: AddressBookEntry,
    ) -> StoreResult<Option<StateRevision>> {
        let mut payload = match self.inner.current() {
            Some(p) => p.clone(),
            None => return Ok(None),
        };
        let book = payload.book_mut(book_type);
        if !book.contains_key(&entry.hostname) {
            return Ok(None);
        }
        book.insert(entry.hostname.clone(), entry);
        let rev = self.inner.publish(payload, |_| Ok(())).await?;
        Ok(Some(rev))
    }

    /// Delete an entry from a given book.
    pub async fn delete(
        &mut self,
        book_type: AdministrativeAddressBookType,
        hostname: &str,
    ) -> StoreResult<Option<StateRevision>> {
        let mut payload = match self.inner.current() {
            Some(p) => p.clone(),
            None => return Ok(None),
        };
        let book = payload.book_mut(book_type);
        let removed = book.remove(hostname);
        if removed.is_none() {
            return Ok(None);
        }
        let rev = self.inner.publish(payload, |_| Ok(())).await?;
        Ok(Some(rev))
    }

    /// Delete all entries from a given book.
    pub async fn delete_all(
        &mut self,
        book_type: AdministrativeAddressBookType,
    ) -> StoreResult<Option<StateRevision>> {
        let mut payload = match self.inner.current() {
            Some(p) => p.clone(),
            None => return Ok(None),
        };
        let book = payload.book_mut(book_type);
        if book.is_empty() {
            return Ok(None);
        }
        book.clear();
        let rev = self.inner.publish(payload, |_| Ok(())).await?;
        Ok(Some(rev))
    }

    /// Get the subscription set.
    pub fn subscriptions(&self) -> SubscriptionSet {
        self.inner.current().map(|p| p.subscriptions.clone()).unwrap_or_default()
    }

    /// Set the subscription set.
    pub async fn set_subscriptions(
        &mut self,
        subscriptions: SubscriptionSet,
    ) -> StoreResult<StateRevision> {
        let mut payload =
            self.inner.current().cloned().unwrap_or_else(AddressBookStorePayload::empty);
        payload.subscriptions = subscriptions;
        self.inner.publish(payload, |_| Ok(())).await
    }

    /// Get the address book configuration.
    pub fn configuration(&self) -> AddressBookConfiguration {
        self.inner.current().map(|p| p.configuration.clone()).unwrap_or_default()
    }

    /// Set the address book configuration.
    pub async fn set_configuration(
        &mut self,
        configuration: AddressBookConfiguration,
    ) -> StoreResult<StateRevision> {
        let mut payload =
            self.inner.current().cloned().unwrap_or_else(AddressBookStorePayload::empty);
        payload.configuration = configuration;
        self.inner.publish(payload, |_| Ok(())).await
    }

    /// Return total entry count across all books.
    pub fn total_entries(&self) -> usize {
        self.inner
            .current()
            .map(|p| p.private.len() + p.local.len() + p.router.len() + p.published.len())
            .unwrap_or(0)
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
        let mut store = AddressBookStore::new(dir.clone(), 1024 * 1024);
        let loaded = store.load().await.unwrap();
        assert_eq!(loaded, None);
        assert_eq!(store.total_entries(), 0);
        assert!(store.list(AdministrativeAddressBookType::Private).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn add_and_list() {
        let dir = test_dir();
        let mut store = AddressBookStore::new(dir.clone(), 1024 * 1024);

        let entry = AddressBookEntry::new("example.i2p", "base64dest");
        store.add(AdministrativeAddressBookType::Private, entry.clone()).await.unwrap();

        let entries = store.list(AdministrativeAddressBookType::Private);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].hostname, "example.i2p");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn book_isolation() {
        let dir = test_dir();
        let mut store = AddressBookStore::new(dir.clone(), 1024 * 1024);

        store
            .add(
                AdministrativeAddressBookType::Private,
                AddressBookEntry::new("private.i2p", "priv-dest"),
            )
            .await
            .unwrap();
        store
            .add(
                AdministrativeAddressBookType::Local,
                AddressBookEntry::new("local.i2p", "local-dest"),
            )
            .await
            .unwrap();

        assert_eq!(store.list(AdministrativeAddressBookType::Private).len(), 1);
        assert_eq!(store.list(AdministrativeAddressBookType::Local).len(), 1);
        assert_eq!(store.list(AdministrativeAddressBookType::Router).len(), 0);
        assert_eq!(
            store.list(AdministrativeAddressBookType::Published).len(),
            0
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn lookup_found() {
        let dir = test_dir();
        let mut store = AddressBookStore::new(dir.clone(), 1024 * 1024);

        store
            .add(
                AdministrativeAddressBookType::Local,
                AddressBookEntry::new("test.i2p", "dest"),
            )
            .await
            .unwrap();

        let found = store.lookup(AdministrativeAddressBookType::Local, "test.i2p");
        assert!(found.is_some());
        assert_eq!(found.unwrap().destination, "dest");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn lookup_not_found() {
        let dir = test_dir();
        let store = AddressBookStore::new(dir.clone(), 1024 * 1024);
        let found = store.lookup(AdministrativeAddressBookType::Local, "missing.i2p");
        assert!(found.is_none());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn delete_entry() {
        let dir = test_dir();
        let mut store = AddressBookStore::new(dir.clone(), 1024 * 1024);

        store
            .add(
                AdministrativeAddressBookType::Router,
                AddressBookEntry::new("del.i2p", "dest"),
            )
            .await
            .unwrap();

        let deleted = store.delete(AdministrativeAddressBookType::Router, "del.i2p").await.unwrap();
        assert!(deleted.is_some());
        assert!(store.lookup(AdministrativeAddressBookType::Router, "del.i2p").is_none());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn delete_all_entries() {
        let dir = test_dir();
        let mut store = AddressBookStore::new(dir.clone(), 1024 * 1024);

        store
            .add(
                AdministrativeAddressBookType::Published,
                AddressBookEntry::new("a.i2p", "dest-a"),
            )
            .await
            .unwrap();
        store
            .add(
                AdministrativeAddressBookType::Published,
                AddressBookEntry::new("b.i2p", "dest-b"),
            )
            .await
            .unwrap();

        let deleted = store.delete_all(AdministrativeAddressBookType::Published).await.unwrap();
        assert!(deleted.is_some());
        assert_eq!(
            store.list(AdministrativeAddressBookType::Published).len(),
            0
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn round_trip_persistence() {
        let dir = test_dir();
        {
            let mut store = AddressBookStore::new(dir.clone(), 1024 * 1024);
            store
                .add(
                    AdministrativeAddressBookType::Private,
                    AddressBookEntry::new("p.i2p", "p-dest"),
                )
                .await
                .unwrap();
            store
                .add(
                    AdministrativeAddressBookType::Local,
                    AddressBookEntry::new("l.i2p", "l-dest"),
                )
                .await
                .unwrap();
        }

        {
            let mut store = AddressBookStore::new(dir.clone(), 1024 * 1024);
            let loaded = store.load().await.unwrap();
            assert!(loaded.is_some());
            assert_eq!(store.total_entries(), 2);
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn subscriptions_round_trip() {
        let dir = test_dir();
        {
            let mut store = AddressBookStore::new(dir.clone(), 1024 * 1024);
            let mut subs = SubscriptionSet::new();
            subs.push("http://sub1.example.com".to_string());
            subs.push("http://sub2.example.com".to_string());
            store.set_subscriptions(subs).await.unwrap();
        }

        {
            let mut store = AddressBookStore::new(dir.clone(), 1024 * 1024);
            store.load().await.unwrap();
            assert_eq!(store.subscriptions().len(), 2);
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn configuration_round_trip() {
        let dir = test_dir();
        {
            let mut store = AddressBookStore::new(dir.clone(), 1024 * 1024);
            let mut config = AddressBookConfiguration::new();
            config.insert("mode".to_string(), "aggressive".to_string());
            store.set_configuration(config).await.unwrap();
        }

        {
            let mut store = AddressBookStore::new(dir.clone(), 1024 * 1024);
            store.load().await.unwrap();
            assert_eq!(store.configuration().get("mode"), Some("aggressive"));
        }

        let _ = std::fs::remove_dir_all(&dir);
    }
}
