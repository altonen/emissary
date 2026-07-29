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

use async_trait::async_trait;

use crate::i2pcontrol::backends::registry::TunnelBackendRegistry;
use crate::i2pcontrol::backends::{BackendError, TunnelBackend};
use crate::i2pcontrol::domain::address_book::{
    AddressBookConfiguration, AddressBookEntry, AdministrativeAddressBookType, SubscriptionSet,
};
use crate::i2pcontrol::domain::tunnel::{TunnelDefinition, TunnelName, TunnelType};
use crate::i2pcontrol::stores::fakes::TunnelStoreFake;

/// Control plane interface for I2PControl method handlers.
///
/// This trait defines the typed internal boundary used by JSON-RPC handlers.
/// It coordinates authentication-independent operations for router identity,
/// version, and uptime.
///
/// Tunnel operations are delegated exclusively to [`TunnelManagerControl`].
/// This trait intentionally does not include tunnel queries to prevent
/// dual-path access and ensure all tunnel consumers share one service object.
pub trait ControlPlane: Send + Sync {
    /// Get the router identity (base64 RouterInfo).
    fn router_identity(&self) -> Result<String, String>;

    /// Get router uptime in milliseconds.
    fn router_uptime_ms(&self) -> u64;

    /// Get router version string.
    fn router_version(&self) -> String;
}

/// Address book control plane interface.
///
/// Provides async operations for the four Proposal 170 administrative address
/// books, subscriptions, and configuration. Implementations must use durable
/// persistence and return success only after atomic commit.
///
/// # Invariants
///
/// - Only one administrative book is mutated per operation.
/// - All four books remain independent across operations.
/// - Success means durable commit; failure leaves prior state active.
/// - No implementation writes to `router.toml` or the runtime address book.
/// - No implementation performs network fetches or filesystem side effects.
#[async_trait]
pub trait AddressBookControl: Send + Sync {
    /// List all entries in the specified book.
    async fn list(
        &self,
        book_type: AdministrativeAddressBookType,
    ) -> Result<Vec<AddressBookEntry>, String>;

    /// Look up an entry by hostname in the specified book.
    async fn lookup(
        &self,
        book_type: AdministrativeAddressBookType,
        hostname: &str,
    ) -> Result<Option<AddressBookEntry>, String>;

    /// Add an entry to the specified book.
    ///
    /// Returns `Ok(())` on durable commit.
    async fn add(
        &self,
        book_type: AdministrativeAddressBookType,
        entry: AddressBookEntry,
    ) -> Result<(), String>;

    /// Update an existing entry in the specified book.
    ///
    /// Returns `Ok(true)` if the entry existed and was updated,
    /// `Ok(false)` if the entry was not found.
    async fn update(
        &self,
        book_type: AdministrativeAddressBookType,
        entry: AddressBookEntry,
    ) -> Result<bool, String>;

    /// Delete an entry from the specified book.
    ///
    /// Returns `Ok(true)` if the entry was deleted, `Ok(false)` if not found.
    async fn delete(
        &self,
        book_type: AdministrativeAddressBookType,
        hostname: &str,
    ) -> Result<bool, String>;

    /// Delete all entries from the specified book.
    ///
    /// Returns `Ok(true)` if any entries were deleted, `Ok(false)` if empty.
    async fn delete_all(&self, book_type: AdministrativeAddressBookType) -> Result<bool, String>;

    /// Get the current subscription set.
    async fn subscriptions(&self) -> Result<SubscriptionSet, String>;

    /// Replace the subscription set atomically.
    async fn set_subscriptions(&self, subscriptions: SubscriptionSet) -> Result<(), String>;

    /// Get the current address book configuration.
    async fn configuration(&self) -> Result<AddressBookConfiguration, String>;

    /// Set the address book configuration atomically.
    async fn set_configuration(
        &self,
        configuration: AddressBookConfiguration,
    ) -> Result<(), String>;
}

/// Fake control plane for testing.
///
/// Returns stub values without accessing any real router state.
pub struct FakeControlPlane;

impl FakeControlPlane {
    pub fn new() -> Self {
        Self
    }
}

impl Default for FakeControlPlane {
    fn default() -> Self {
        Self::new()
    }
}

impl ControlPlane for FakeControlPlane {
    fn router_identity(&self) -> Result<String, String> {
        Ok(String::new())
    }

    fn router_uptime_ms(&self) -> u64 {
        0
    }

    fn router_version(&self) -> String {
        String::from("Emissary 0.4.0")
    }
}

/// Fake address book control plane for testing.
///
/// Uses in-memory storage with the same semantics as the production adapter.
pub struct FakeAddressBookControl {
    inner: std::sync::Mutex<crate::i2pcontrol::stores::fakes::AddressBookStoreFake>,
}

impl FakeAddressBookControl {
    pub fn new() -> Self {
        Self {
            inner: std::sync::Mutex::new(
                crate::i2pcontrol::stores::fakes::AddressBookStoreFake::new(),
            ),
        }
    }
}

impl Default for FakeAddressBookControl {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AddressBookControl for FakeAddressBookControl {
    async fn list(
        &self,
        book_type: AdministrativeAddressBookType,
    ) -> Result<Vec<AddressBookEntry>, String> {
        let store = self.inner.lock().map_err(|e| format!("Lock poisoned: {e}"))?;
        Ok(store.list(book_type).into_iter().cloned().collect())
    }

    async fn lookup(
        &self,
        book_type: AdministrativeAddressBookType,
        hostname: &str,
    ) -> Result<Option<AddressBookEntry>, String> {
        let store = self.inner.lock().map_err(|e| format!("Lock poisoned: {e}"))?;
        Ok(store.lookup(book_type, hostname).cloned())
    }

    async fn add(
        &self,
        book_type: AdministrativeAddressBookType,
        entry: AddressBookEntry,
    ) -> Result<(), String> {
        let mut store = self.inner.lock().map_err(|e| format!("Lock poisoned: {e}"))?;
        store.add(book_type, entry);
        Ok(())
    }

    async fn update(
        &self,
        book_type: AdministrativeAddressBookType,
        entry: AddressBookEntry,
    ) -> Result<bool, String> {
        let mut store = self.inner.lock().map_err(|e| format!("Lock poisoned: {e}"))?;
        Ok(store.update(book_type, entry).is_some())
    }

    async fn delete(
        &self,
        book_type: AdministrativeAddressBookType,
        hostname: &str,
    ) -> Result<bool, String> {
        let mut store = self.inner.lock().map_err(|e| format!("Lock poisoned: {e}"))?;
        Ok(store.delete(book_type, hostname).is_some())
    }

    async fn delete_all(&self, book_type: AdministrativeAddressBookType) -> Result<bool, String> {
        let mut store = self.inner.lock().map_err(|e| format!("Lock poisoned: {e}"))?;
        Ok(store.delete_all(book_type).is_some())
    }

    async fn subscriptions(&self) -> Result<SubscriptionSet, String> {
        let store = self.inner.lock().map_err(|e| format!("Lock poisoned: {e}"))?;
        Ok(store.subscriptions().clone())
    }

    async fn set_subscriptions(&self, subscriptions: SubscriptionSet) -> Result<(), String> {
        let mut store = self.inner.lock().map_err(|e| format!("Lock poisoned: {e}"))?;
        store.set_subscriptions(subscriptions);
        Ok(())
    }

    async fn configuration(&self) -> Result<AddressBookConfiguration, String> {
        let store = self.inner.lock().map_err(|e| format!("Lock poisoned: {e}"))?;
        Ok(store.configuration().clone())
    }

    async fn set_configuration(
        &self,
        configuration: AddressBookConfiguration,
    ) -> Result<(), String> {
        let mut store = self.inner.lock().map_err(|e| format!("Lock poisoned: {e}"))?;
        store.set_configuration(configuration);
        Ok(())
    }
}

/// TunnelManager control plane interface.
///
/// Provides async operations for Proposal 170 tunnel definition CRUD and
/// lifecycle dispatch. Implementations must use durable persistence and
/// return success only after atomic commit.
///
/// # Invariants
///
/// - CRUD operations affect exactly one tunnel definition per call.
/// - Lifecycle operations are serialized per definition.
/// - No implementation writes to `router.toml`.
/// - No implementation performs network or filesystem side effects beyond
///   persistence of tunnel definitions.
/// - Unsupported tunnel types return deterministic not-implemented errors.
/// - Startup-managed definitions are read-only and reject mutations.
#[async_trait]
pub trait TunnelManagerControl: Send + Sync {
    /// List all tunnel definitions.
    async fn list(&self) -> Result<Vec<TunnelDefinition>, String>;

    /// Get a tunnel definition by name.
    async fn get(&self, name: &str) -> Result<Option<TunnelDefinition>, String>;

    /// Create a new tunnel definition.
    ///
    /// Returns `Ok(())` on durable commit.
    async fn create(&self, definition: TunnelDefinition) -> Result<(), String>;

    /// Update an existing tunnel definition (edit and/or rename).
    ///
    /// Returns `Ok(true)` if updated, `Ok(false)` if not found.
    async fn update(
        &self,
        name: &str,
        definition: TunnelDefinition,
        new_name: Option<TunnelName>,
    ) -> Result<bool, String>;

    /// Delete a tunnel definition by name.
    ///
    /// Returns `Ok(true)` if deleted, `Ok(false)` if not found.
    async fn delete(&self, name: &str) -> Result<bool, String>;

    /// Start a tunnel by name through the backend registry.
    async fn start(&self, name: &str) -> Result<String, String>;

    /// Stop a tunnel by name through the backend registry.
    async fn stop(&self, name: &str) -> Result<String, String>;

    /// Restart a tunnel by name through the backend registry.
    async fn restart(&self, name: &str) -> Result<String, String>;

    /// Look up the backend for a given tunnel type.
    fn get_backend(&self, tunnel_type: TunnelType) -> Option<std::sync::Arc<dyn TunnelBackend>>;

    /// Return the backend registry reference.
    fn registry(&self) -> &TunnelBackendRegistry;
}

/// Fake tunnel manager control plane for testing.
///
/// Uses in-memory storage with the same semantics as the production adapter.
pub struct FakeTunnelManagerControl {
    store: std::sync::Mutex<TunnelStoreFake>,
    registry: TunnelBackendRegistry,
}

impl FakeTunnelManagerControl {
    /// Create a new fake with default (all-unsupported) registry.
    pub fn new() -> Self {
        Self {
            store: std::sync::Mutex::new(TunnelStoreFake::new()),
            registry: super::backends::registry::create_default_registry()
                .expect("default registry is exhaustive"),
        }
    }

    /// Create a new fake with a custom backend registry.
    pub fn with_registry(registry: TunnelBackendRegistry) -> Self {
        Self {
            store: std::sync::Mutex::new(TunnelStoreFake::new()),
            registry,
        }
    }
}

impl Default for FakeTunnelManagerControl {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TunnelManagerControl for FakeTunnelManagerControl {
    async fn list(&self) -> Result<Vec<TunnelDefinition>, String> {
        let store = self.store.lock().map_err(|e| format!("Lock poisoned: {e}"))?;
        Ok(store.list().into_iter().cloned().collect())
    }

    async fn get(&self, name: &str) -> Result<Option<TunnelDefinition>, String> {
        let store = self.store.lock().map_err(|e| format!("Lock poisoned: {e}"))?;
        Ok(store.get(name).cloned())
    }

    async fn create(&self, definition: TunnelDefinition) -> Result<(), String> {
        let mut store = self.store.lock().map_err(|e| format!("Lock poisoned: {e}"))?;
        if store.contains(definition.name.as_str()) {
            return Err(format!(
                "error - tunnel '{}' already exists",
                definition.name.as_str()
            ));
        }
        store.upsert(definition);
        Ok(())
    }

    async fn update(
        &self,
        name: &str,
        definition: TunnelDefinition,
        new_name: Option<TunnelName>,
    ) -> Result<bool, String> {
        let mut store = self.store.lock().map_err(|e| format!("Lock poisoned: {e}"))?;
        if store.get(name).is_none() {
            return Ok(false);
        }
        // If renaming, check new name doesn't collide
        if let Some(ref nn) = new_name {
            if nn.as_str() != name && store.contains(nn.as_str()) {
                return Err(format!(
                    "error - tunnel name '{}' already exists",
                    nn.as_str()
                ));
            }
        }
        store.remove(name);
        // When renaming, use the new name as the storage key
        let mut def = definition;
        if let Some(ref nn) = new_name {
            def.name = nn.clone();
        }
        store.upsert(def);
        Ok(true)
    }

    async fn delete(&self, name: &str) -> Result<bool, String> {
        let mut store = self.store.lock().map_err(|e| format!("Lock poisoned: {e}"))?;
        Ok(store.remove(name).is_some())
    }

    async fn start(&self, name: &str) -> Result<String, String> {
        let def = {
            let store = self.store.lock().map_err(|e| format!("Lock poisoned: {e}"))?;
            store
                .get(name)
                .ok_or_else(|| format!("error - tunnel '{}' not found", name))?
                .clone()
        };

        let backend = self.registry.get(def.tunnel_type);
        match backend.start(&def).await {
            Ok(()) => Ok(format!("ok - {} started", def.tunnel_type.as_str())),
            Err(BackendError::NotImplemented { tunnel_type }) => {
                Ok(format!("error - {} not implemented", tunnel_type.as_str()))
            }
            Err(e) => Ok(format!("error - {}", e)),
        }
    }

    async fn stop(&self, name: &str) -> Result<String, String> {
        let def = {
            let store = self.store.lock().map_err(|e| format!("Lock poisoned: {e}"))?;
            store
                .get(name)
                .ok_or_else(|| format!("error - tunnel '{}' not found", name))?
                .clone()
        };

        let backend = self.registry.get(def.tunnel_type);
        match backend.stop(&def).await {
            Ok(()) => Ok("ok".to_string()),
            Err(e) => Ok(format!("error - {}", e)),
        }
    }

    async fn restart(&self, name: &str) -> Result<String, String> {
        let def = {
            let store = self.store.lock().map_err(|e| format!("Lock poisoned: {e}"))?;
            store
                .get(name)
                .ok_or_else(|| format!("error - tunnel '{}' not found", name))?
                .clone()
        };

        let backend = self.registry.get(def.tunnel_type);
        // Restart = stop then start
        let _ = backend.stop(&def).await;
        match backend.start(&def).await {
            Ok(()) => Ok(format!("ok - {} restarted", def.tunnel_type.as_str())),
            Err(BackendError::NotImplemented { tunnel_type }) => {
                Ok(format!("error - {} not implemented", tunnel_type.as_str()))
            }
            Err(e) => Ok(format!("error - {}", e)),
        }
    }

    fn get_backend(&self, tunnel_type: TunnelType) -> Option<std::sync::Arc<dyn TunnelBackend>> {
        if self.registry.contains(tunnel_type) {
            Some(self.registry.get(tunnel_type))
        } else {
            None
        }
    }

    fn registry(&self) -> &TunnelBackendRegistry {
        &self.registry
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_control_plane_returns_stubs() {
        let cp = FakeControlPlane::new();
        assert_eq!(cp.router_uptime_ms(), 0);
        assert_eq!(cp.router_version(), "Emissary 0.4.0");
        assert!(cp.router_identity().unwrap().is_empty());
    }

    #[tokio::test]
    async fn fake_address_book_control_crud() {
        let cp = FakeAddressBookControl::new();

        // List empty
        let entries = cp.list(AdministrativeAddressBookType::Private).await.unwrap();
        assert!(entries.is_empty());

        // Add
        cp.add(
            AdministrativeAddressBookType::Private,
            AddressBookEntry::new("test.i2p", "dest"),
        )
        .await
        .unwrap();

        // List
        let entries = cp.list(AdministrativeAddressBookType::Private).await.unwrap();
        assert_eq!(entries.len(), 1);

        // Lookup
        let found = cp.lookup(AdministrativeAddressBookType::Private, "test.i2p").await.unwrap();
        assert!(found.is_some());

        // Update
        let updated = cp
            .update(
                AdministrativeAddressBookType::Private,
                AddressBookEntry::new("test.i2p", "new-dest"),
            )
            .await
            .unwrap();
        assert!(updated);

        // Delete
        let deleted = cp.delete(AdministrativeAddressBookType::Private, "test.i2p").await.unwrap();
        assert!(deleted);

        // Subscriptions
        let mut subs = SubscriptionSet::new();
        subs.push("http://sub.example.com".to_string());
        cp.set_subscriptions(subs).await.unwrap();
        let subs = cp.subscriptions().await.unwrap();
        assert_eq!(subs.len(), 1);

        // Configuration
        let mut config = AddressBookConfiguration::new();
        config.insert("key".to_string(), "value".to_string());
        cp.set_configuration(config).await.unwrap();
        let config = cp.configuration().await.unwrap();
        assert_eq!(config.get("key"), Some("value"));
    }
}
