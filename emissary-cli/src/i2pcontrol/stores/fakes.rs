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

//! In-memory fake stores for testing handler logic without filesystem I/O.
//!
//! These fakes implement the same mutation/revision semantics as the
//! production stores but store everything in memory. They are intended
//! for M004 handler tests where persistence is not under test.

use std::collections::BTreeMap;

use crate::i2pcontrol::domain::address_book::{
    AddressBookConfiguration, AddressBookEntry, AdministrativeAddressBookType, SubscriptionSet,
};
use crate::i2pcontrol::domain::revision::StateRevision;
use crate::i2pcontrol::domain::tunnel::TunnelDefinition;

/// In-memory tunnel definition store.
///
/// Provides the same CRUD semantics as `TunnelStore` without filesystem I/O.
/// Revision increments with each mutation.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct TunnelStoreFake {
    tunnels: BTreeMap<String, TunnelDefinition>,
    revision: StateRevision,
}

#[allow(dead_code)]
impl TunnelStoreFake {
    /// Create a new empty fake store.
    pub fn new() -> Self {
        Self {
            tunnels: BTreeMap::new(),
            revision: StateRevision::ZERO,
        }
    }

    /// Return the current revision.
    pub fn revision(&self) -> StateRevision {
        self.revision
    }

    /// List all tunnel definitions.
    pub fn list(&self) -> Vec<&TunnelDefinition> {
        self.tunnels.values().collect()
    }

    /// Get a tunnel definition by name.
    pub fn get(&self, name: &str) -> Option<&TunnelDefinition> {
        self.tunnels.get(name)
    }

    /// Add or replace a tunnel definition.
    pub fn upsert(&mut self, definition: TunnelDefinition) -> StateRevision {
        let name = definition.name.as_str().to_string();
        self.tunnels.insert(name, definition);
        self.revision = self.revision.next();
        self.revision
    }

    /// Remove a tunnel definition by name.
    pub fn remove(&mut self, name: &str) -> Option<StateRevision> {
        if self.tunnels.remove(name).is_some() {
            self.revision = self.revision.next();
            Some(self.revision)
        } else {
            None
        }
    }

    /// Return the number of stored tunnel definitions.
    pub fn len(&self) -> usize {
        self.tunnels.len()
    }

    /// Return true if the store is empty.
    pub fn is_empty(&self) -> bool {
        self.tunnels.is_empty()
    }

    /// Return true if a tunnel with the given name exists.
    pub fn contains(&self, name: &str) -> bool {
        self.tunnels.contains_key(name)
    }
}

impl Default for TunnelStoreFake {
    fn default() -> Self {
        Self::new()
    }
}

/// In-memory address book store.
///
/// Provides the same CRUD semantics as `AddressBookStore` without filesystem I/O.
/// Stores all four administrative books as independent maps plus subscriptions
/// and configuration.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct AddressBookStoreFake {
    private: BTreeMap<String, AddressBookEntry>,
    local: BTreeMap<String, AddressBookEntry>,
    router: BTreeMap<String, AddressBookEntry>,
    published: BTreeMap<String, AddressBookEntry>,
    subscriptions: SubscriptionSet,
    configuration: AddressBookConfiguration,
    revision: StateRevision,
}

#[allow(dead_code)]
impl AddressBookStoreFake {
    /// Create a new empty fake store.
    pub fn new() -> Self {
        Self {
            private: BTreeMap::new(),
            local: BTreeMap::new(),
            router: BTreeMap::new(),
            published: BTreeMap::new(),
            subscriptions: SubscriptionSet::new(),
            configuration: AddressBookConfiguration::new(),
            revision: StateRevision::ZERO,
        }
    }

    /// Return the current revision.
    pub fn revision(&self) -> StateRevision {
        self.revision
    }

    fn book(
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

    fn book_mut(
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

    /// List all entries in a given book.
    pub fn list(&self, book_type: AdministrativeAddressBookType) -> Vec<&AddressBookEntry> {
        self.book(book_type).values().collect()
    }

    /// Look up an entry by hostname in a given book.
    pub fn lookup(
        &self,
        book_type: AdministrativeAddressBookType,
        hostname: &str,
    ) -> Option<&AddressBookEntry> {
        self.book(book_type).get(hostname)
    }

    /// Add an entry to a given book.
    pub fn add(
        &mut self,
        book_type: AdministrativeAddressBookType,
        entry: AddressBookEntry,
    ) -> StateRevision {
        self.book_mut(book_type).insert(entry.hostname.clone(), entry);
        self.revision = self.revision.next();
        self.revision
    }

    /// Update an existing entry in a given book.
    pub fn update(
        &mut self,
        book_type: AdministrativeAddressBookType,
        entry: AddressBookEntry,
    ) -> Option<StateRevision> {
        let book = self.book_mut(book_type);
        if !book.contains_key(&entry.hostname) {
            return None;
        }
        book.insert(entry.hostname.clone(), entry);
        self.revision = self.revision.next();
        Some(self.revision)
    }

    /// Delete an entry from a given book.
    pub fn delete(
        &mut self,
        book_type: AdministrativeAddressBookType,
        hostname: &str,
    ) -> Option<StateRevision> {
        let book = self.book_mut(book_type);
        if book.remove(hostname).is_some() {
            self.revision = self.revision.next();
            Some(self.revision)
        } else {
            None
        }
    }

    /// Delete all entries from a given book.
    pub fn delete_all(
        &mut self,
        book_type: AdministrativeAddressBookType,
    ) -> Option<StateRevision> {
        let book = self.book_mut(book_type);
        if book.is_empty() {
            return None;
        }
        book.clear();
        self.revision = self.revision.next();
        Some(self.revision)
    }

    /// Get the subscription set.
    pub fn subscriptions(&self) -> &SubscriptionSet {
        &self.subscriptions
    }

    /// Set the subscription set.
    pub fn set_subscriptions(&mut self, subscriptions: SubscriptionSet) -> StateRevision {
        self.subscriptions = subscriptions;
        self.revision = self.revision.next();
        self.revision
    }

    /// Get the address book configuration.
    pub fn configuration(&self) -> &AddressBookConfiguration {
        &self.configuration
    }

    /// Set the address book configuration.
    pub fn set_configuration(&mut self, configuration: AddressBookConfiguration) -> StateRevision {
        self.configuration = configuration;
        self.revision = self.revision.next();
        self.revision
    }

    /// Return total entry count across all books.
    pub fn total_entries(&self) -> usize {
        self.private.len() + self.local.len() + self.router.len() + self.published.len()
    }
}

impl Default for AddressBookStoreFake {
    fn default() -> Self {
        Self::new()
    }
}

/// In-memory subscription store.
///
/// Provides the same semantics as `SubscriptionStore` without filesystem I/O.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct SubscriptionStoreFake {
    subscriptions: SubscriptionSet,
    revision: StateRevision,
}

#[allow(dead_code)]
impl SubscriptionStoreFake {
    /// Create a new empty fake store.
    pub fn new() -> Self {
        Self {
            subscriptions: SubscriptionSet::new(),
            revision: StateRevision::ZERO,
        }
    }

    /// Return the current revision.
    pub fn revision(&self) -> StateRevision {
        self.revision
    }

    /// Get the current subscription set.
    pub fn subscriptions(&self) -> &SubscriptionSet {
        &self.subscriptions
    }

    /// Set the subscription set.
    pub fn set(&mut self, subscriptions: SubscriptionSet) -> StateRevision {
        self.subscriptions = subscriptions;
        self.revision = self.revision.next();
        self.revision
    }

    /// Return the number of subscriptions.
    pub fn len(&self) -> usize {
        self.subscriptions.len()
    }

    /// Return true if the subscription set is empty.
    pub fn is_empty(&self) -> bool {
        self.subscriptions.is_empty()
    }
}

impl Default for SubscriptionStoreFake {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i2pcontrol::domain::tunnel::{
        StartIntent, TunnelName, TunnelOptions, TunnelOwnership, TunnelRuntimeState, TunnelType,
    };

    fn test_definition(name: &str, tunnel_type: TunnelType) -> TunnelDefinition {
        TunnelDefinition {
            name: TunnelName::new(name).unwrap(),
            tunnel_type,
            ownership: TunnelOwnership::ControlPlane,
            runtime_state: TunnelRuntimeState::Stopped,
            start_intent: StartIntent::DoNotStart,
            options: TunnelOptions::default(),
            raw_config: std::collections::BTreeMap::new(),
        }
    }

    #[test]
    fn tunnel_store_fake_crud() {
        let mut store = TunnelStoreFake::new();
        assert!(store.is_empty());

        let rev1 = store.upsert(test_definition("t1", TunnelType::Socks));
        assert_eq!(rev1, StateRevision::new(1));
        assert_eq!(store.len(), 1);

        let rev2 = store.upsert(test_definition("t2", TunnelType::Client));
        assert_eq!(rev2, StateRevision::new(2));
        assert_eq!(store.len(), 2);

        assert!(store.get("t1").is_some());
        assert!(store.get("t2").is_some());

        let removed = store.remove("t1");
        assert!(removed.is_some());
        assert_eq!(store.len(), 1);
        assert!(store.get("t1").is_none());

        let removed_none = store.remove("nonexistent");
        assert!(removed_none.is_none());
    }

    #[test]
    fn address_book_store_fake_crud() {
        let mut store = AddressBookStoreFake::new();
        assert_eq!(store.total_entries(), 0);

        let rev1 = store.add(
            AdministrativeAddressBookType::Private,
            AddressBookEntry::new("host.i2p", "dest"),
        );
        assert_eq!(rev1, StateRevision::new(1));
        assert_eq!(store.total_entries(), 1);

        let entries = store.list(AdministrativeAddressBookType::Private);
        assert_eq!(entries.len(), 1);

        let found = store.lookup(AdministrativeAddressBookType::Private, "host.i2p");
        assert!(found.is_some());

        let not_found = store.lookup(AdministrativeAddressBookType::Local, "host.i2p");
        assert!(not_found.is_none());

        store.add(
            AdministrativeAddressBookType::Local,
            AddressBookEntry::new("local.i2p", "local-dest"),
        );
        assert_eq!(store.total_entries(), 2);

        let deleted = store.delete(AdministrativeAddressBookType::Private, "host.i2p");
        assert!(deleted.is_some());
        assert_eq!(store.total_entries(), 1);

        let deleted_none = store.delete(AdministrativeAddressBookType::Private, "missing.i2p");
        assert!(deleted_none.is_none());
    }

    #[test]
    fn subscription_store_fake_crud() {
        let mut store = SubscriptionStoreFake::new();
        assert!(store.is_empty());

        let mut subs = SubscriptionSet::new();
        subs.push("http://sub1.example.com".to_string());
        subs.push("http://sub2.example.com".to_string());

        let rev = store.set(subs);
        assert_eq!(rev, StateRevision::new(1));
        assert_eq!(store.len(), 2);

        let mut subs2 = SubscriptionSet::new();
        subs2.push("http://sub3.example.com".to_string());
        store.set(subs2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn fake_stores_match_revision_semantics() {
        let mut tunnel = TunnelStoreFake::new();
        let mut address_book = AddressBookStoreFake::new();
        let subscriptions = SubscriptionStoreFake::new();

        // All start at ZERO
        assert_eq!(tunnel.revision(), StateRevision::ZERO);
        assert_eq!(address_book.revision(), StateRevision::ZERO);
        assert_eq!(subscriptions.revision(), StateRevision::ZERO);

        // Mutations increment independently
        tunnel.upsert(test_definition("t1", TunnelType::Client));
        assert_eq!(tunnel.revision(), StateRevision::new(1));
        assert_eq!(address_book.revision(), StateRevision::ZERO);

        address_book.add(
            AdministrativeAddressBookType::Private,
            AddressBookEntry::new("h.i2p", "d"),
        );
        assert_eq!(tunnel.revision(), StateRevision::new(1));
        assert_eq!(address_book.revision(), StateRevision::new(1));
    }
}
