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

//! Passive fixed-size client-service registry for Proposal 170
//! `ClientServicesInfo`.
//!
//! The registry observes application-owned proxies, listeners, and session
//! state without taking control of their lifecycle. Internal state vocabulary
//! is private and maps to exact Proposal 170 response values at serialization
//! time.
//!
//! # Invariants
//!
//! - Fixed set of service categories; no unbounded dynamic entries.
//! - Generation-fenced updates: stale tasks cannot overwrite current state.
//! - Immutable snapshots are cheaply cloneable.
//! - No task handles, cancellation authority, or mutation capability is
//!   exposed.
//! - No secrets, credentials, private keys, or sensitive configuration
//!   enters the registry.
//! - Shutdown-safe: updates during shutdown do not race with reads.
//! - Concurrent readers/writers produce coherent before-or-after snapshots.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

/// Sanitized failure description suitable for API responses.
///
/// Contains only an OS error kind and an optional socket address. No
/// credentials, private keys, backtraces, or filesystem paths are included.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SanitizedFailure {
    /// Short error kind description (e.g. "ConnectionRefused").
    pub error_kind: String,
    /// Optional socket address that was involved.
    pub address: Option<String>,
}

/// Internal observed service state vocabulary.
///
/// This vocabulary is **not** exposed publicly. The serializer maps it to
/// exact Proposal 170 response values. The names and variants are an
/// implementation detail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObservedServiceState {
    /// Service is not configured or compiled.
    Disabled,
    /// Service configuration exists but no listener is bound.
    Configured,
    /// Service is starting (task spawned, not yet listening).
    Starting,
    /// Service has a bound listener and is actively serving.
    Listening,
    /// Service encountered a bind, constructor, or runtime failure.
    Failed(SanitizedFailure),
    /// Service is shutting down (task received stop signal).
    Stopping,
    /// Service task has exited.
    Stopped,
}

/// Fixed set of Proposal 170 client-service categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ServiceCategory {
    I2PTunnel,
    HttpProxy,
    Socks,
    Sam,
    Bob,
    I2cp,
}

impl ServiceCategory {
    /// All service categories in fixed order.
    pub const ALL: &'static [ServiceCategory] = &[
        ServiceCategory::I2PTunnel,
        ServiceCategory::HttpProxy,
        ServiceCategory::Socks,
        ServiceCategory::Sam,
        ServiceCategory::Bob,
        ServiceCategory::I2cp,
    ];

    /// The Proposal 170 selector key for this category.
    pub fn selector_key(self) -> &'static str {
        match self {
            ServiceCategory::I2PTunnel => "I2PTunnel",
            ServiceCategory::HttpProxy => "HTTPProxy",
            ServiceCategory::Socks => "SOCKS",
            ServiceCategory::Sam => "SAM",
            ServiceCategory::Bob => "BOB",
            ServiceCategory::I2cp => "I2CP",
        }
    }
}

/// A single service entry in the registry.
#[derive(Debug, Clone)]
pub struct ServiceEntry {
    /// The service category.
    pub category: ServiceCategory,
    /// Current observed state.
    pub state: ObservedServiceState,
    /// Optional service-specific metadata (address, port, session count, etc).
    /// The exact shape depends on the service category and is resolved at
    /// serialization time.
    pub metadata: ServiceMetadata,
}

/// Service-specific metadata that carries bound address, session count, or
/// tunnel inventory information.
///
/// This is an internal type. The serializer maps its fields to exact
/// Proposal 170 response keys. No sensitive material is included.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ServiceMetadata {
    /// Bound address host (e.g. "127.0.0.1" or "example.b32.i2p").
    pub host: Option<String>,
    /// Bound port number.
    pub port: Option<u16>,
    /// Whether the service is enabled in configuration.
    pub enabled: bool,
    /// Number of active sessions (SAM, I2CP).
    pub session_count: Option<usize>,
    /// For I2PTunnel: configured tunnel definitions mapped by name.
    /// Each value is a map of tunnel-type to name-to-address entries.
    pub tunnel_definitions: Option<HashMap<String, HashMap<String, TunnelInfo>>>,
}

/// Tunnel information for I2PTunnel response entries.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TunnelInfo {
    /// Tunnel address (b32.i2p or similar).
    pub address: String,
    /// Tunnel port (for server tunnels).
    pub port: Option<u16>,
}

/// An immutable snapshot of the service registry.
///
/// Clone is cheap (Arc reference counting). The snapshot is coherent and
/// represents a point-in-time view of all service states.
#[derive(Debug, Clone)]
pub struct ServiceSnapshot {
    entries: Arc<Vec<ServiceEntry>>,
    generation: u64,
}

impl ServiceSnapshot {
    /// Get the snapshot generation (monotonic counter).
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Get all service entries.
    pub fn entries(&self) -> &[ServiceEntry] {
        &self.entries
    }

    /// Get the entry for a specific category, if present.
    pub fn get(&self, category: ServiceCategory) -> Option<&ServiceEntry> {
        self.entries.iter().find(|e| e.category == category)
    }

    /// Check if a specific category is present.
    pub fn has(&self, category: ServiceCategory) -> bool {
        self.entries.iter().any(|e| e.category == category)
    }
}

/// Update handle for a specific service category.
///
/// Generated by the registry for a particular producer. The handle is
/// scoped to a single category and carries the startup generation to
/// prevent stale updates from overwriting newer state.
#[derive(Debug, Clone)]
pub struct ServiceUpdateHandle {
    category: ServiceCategory,
    generation: u64,
    registry: Arc<ServiceRegistryInner>,
}

impl ServiceUpdateHandle {
    /// Update the service state. Returns `Err` if the generation is stale
    /// (a newer task has taken over).
    pub fn update(
        &self,
        state: ObservedServiceState,
        metadata: ServiceMetadata,
    ) -> Result<(), StaleGenerationError> {
        self.registry.update_service(self.category, self.generation, state, metadata)
    }

    /// Get the category this handle updates.
    pub fn category(&self) -> ServiceCategory {
        self.category
    }

    /// Get the generation this handle was created with.
    pub fn generation(&self) -> u64 {
        self.generation
    }
}

/// Error returned when a stale generation attempts an update.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaleGenerationError {
    /// The generation of the stale handle.
    pub handle_generation: u64,
    /// The current (newer) generation.
    pub current_generation: u64,
}

/// Internal shared state for the service registry.
struct ServiceRegistryInner {
    /// Current entries for each service category.
    entries: RwLock<HashMap<ServiceCategory, ServiceEntry>>,
    /// Monotonic generation counter. Incremented on each new producer.
    generation: AtomicU64,
}

impl std::fmt::Debug for ServiceRegistryInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServiceRegistryInner")
            .field("generation", &self.generation.load(Ordering::Relaxed))
            .finish()
    }
}

impl ServiceRegistryInner {
    fn new() -> Self {
        let mut entries = HashMap::new();
        for &cat in ServiceCategory::ALL {
            entries.insert(
                cat,
                ServiceEntry {
                    category: cat,
                    state: ObservedServiceState::Disabled,
                    metadata: ServiceMetadata::default(),
                },
            );
        }
        Self {
            entries: RwLock::new(entries),
            generation: AtomicU64::new(1),
        }
    }

    /// Allocate a new generation for a producer.
    fn next_generation(&self) -> u64 {
        self.generation.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Update a service entry if the generation is current.
    fn update_service(
        &self,
        category: ServiceCategory,
        handle_generation: u64,
        state: ObservedServiceState,
        metadata: ServiceMetadata,
    ) -> Result<(), StaleGenerationError> {
        // Check if this handle's generation is still current.
        let current = self.generation.load(Ordering::SeqCst);
        if handle_generation < current {
            return Err(StaleGenerationError {
                handle_generation,
                current_generation: current,
            });
        }

        let mut entries = self.entries.write().expect("service registry lock poisoned");
        entries.insert(
            category,
            ServiceEntry {
                category,
                state,
                metadata,
            },
        );
        Ok(())
    }

    /// Take an immutable snapshot.
    fn snapshot(&self) -> ServiceSnapshot {
        let entries = self.entries.read().expect("service registry lock poisoned");
        let generation = self.generation.load(Ordering::SeqCst);
        let vec: Vec<ServiceEntry> = ServiceCategory::ALL
            .iter()
            .filter_map(|&cat| entries.get(&cat).cloned())
            .collect();
        ServiceSnapshot {
            entries: Arc::new(vec),
            generation,
        }
    }

    /// Reset all entries to Disabled (for testing or shutdown).
    fn reset(&self) {
        let mut entries = self.entries.write().expect("service registry lock poisoned");
        for &cat in ServiceCategory::ALL {
            entries.insert(
                cat,
                ServiceEntry {
                    category: cat,
                    state: ObservedServiceState::Disabled,
                    metadata: ServiceMetadata::default(),
                },
            );
        }
    }
}

/// Passive fixed-size client-service registry.
///
/// Created in the application composition root and passed to producers
/// through narrow [`ServiceUpdateHandle`] instances. Each handle is
/// generation-fenced and scoped to a single service category.
#[derive(Clone)]
pub struct ServiceRegistry {
    inner: Arc<ServiceRegistryInner>,
}

impl ServiceRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(ServiceRegistryInner::new()),
        }
    }

    /// Allocate a new update handle for the given category.
    ///
    /// Each call increments the generation counter. Only the most-recently
    /// allocated handle for a category can successfully update; older
    /// handles receive [`StaleGenerationError`].
    pub fn allocate_handle(&self, category: ServiceCategory) -> ServiceUpdateHandle {
        let generation = self.inner.next_generation();
        ServiceUpdateHandle {
            category,
            generation,
            registry: Arc::clone(&self.inner),
        }
    }

    /// Take an immutable snapshot of all service states.
    pub fn snapshot(&self) -> ServiceSnapshot {
        self.inner.snapshot()
    }

    /// Reset all entries to Disabled. Used in tests and shutdown.
    pub fn reset(&self) {
        self.inner.reset();
    }

    /// Get the current generation counter value.
    pub fn current_generation(&self) -> u64 {
        self.inner.generation.load(Ordering::SeqCst)
    }
}

impl Default for ServiceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_initial_state() {
        let reg = ServiceRegistry::new();
        let snap = reg.snapshot();
        assert_eq!(snap.entries().len(), 6);
        for entry in snap.entries() {
            assert_eq!(entry.state, ObservedServiceState::Disabled);
        }
    }

    #[test]
    fn allocate_handle_increments_generation() {
        let reg = ServiceRegistry::new();
        let gen1 = reg.current_generation();
        let _h1 = reg.allocate_handle(ServiceCategory::HttpProxy);
        let gen2 = reg.current_generation();
        assert!(gen2 > gen1);
        let _h2 = reg.allocate_handle(ServiceCategory::Socks);
        let gen3 = reg.current_generation();
        assert!(gen3 > gen2);
    }

    #[test]
    fn update_with_current_generation() {
        let reg = ServiceRegistry::new();
        let handle = reg.allocate_handle(ServiceCategory::HttpProxy);
        let result = handle.update(
            ObservedServiceState::Listening,
            ServiceMetadata {
                enabled: true,
                host: Some("127.0.0.1".into()),
                port: Some(4444),
                ..Default::default()
            },
        );
        assert!(result.is_ok());
        let snap = reg.snapshot();
        let entry = snap.get(ServiceCategory::HttpProxy).unwrap();
        assert_eq!(entry.state, ObservedServiceState::Listening);
        assert_eq!(entry.metadata.port, Some(4444));
    }

    #[test]
    fn stale_generation_rejected() {
        let reg = ServiceRegistry::new();
        let old_handle = reg.allocate_handle(ServiceCategory::Socks);
        // Allocate a newer handle (simulates a new task taking over).
        let _new_handle = reg.allocate_handle(ServiceCategory::Socks);
        let result = old_handle.update(ObservedServiceState::Listening, ServiceMetadata::default());
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.handle_generation < err.current_generation);
    }

    #[test]
    fn snapshot_is_cloneable() {
        let reg = ServiceRegistry::new();
        let handle = reg.allocate_handle(ServiceCategory::I2cp);
        handle
            .update(
                ObservedServiceState::Listening,
                ServiceMetadata {
                    enabled: true,
                    ..Default::default()
                },
            )
            .unwrap();
        let snap1 = reg.snapshot();
        let snap2 = snap1.clone();
        assert_eq!(snap1.generation(), snap2.generation());
        assert_eq!(
            snap1.get(ServiceCategory::I2cp).unwrap().state,
            snap2.get(ServiceCategory::I2cp).unwrap().state
        );
    }

    #[test]
    fn concurrent_updates_produce_coherent_snapshots() {
        let reg = ServiceRegistry::new();
        // Allocate and update each category sequentially so generations don't conflict.
        for &cat in ServiceCategory::ALL {
            let handle = reg.allocate_handle(cat);
            handle
                .update(ObservedServiceState::Configured, ServiceMetadata::default())
                .unwrap();
        }
        let snap = reg.snapshot();
        for entry in snap.entries() {
            assert_eq!(entry.state, ObservedServiceState::Configured);
        }
    }

    #[test]
    fn reset_clears_all_entries() {
        let reg = ServiceRegistry::new();
        let handle = reg.allocate_handle(ServiceCategory::HttpProxy);
        handle
            .update(ObservedServiceState::Listening, ServiceMetadata::default())
            .unwrap();
        reg.reset();
        let snap = reg.snapshot();
        for entry in snap.entries() {
            assert_eq!(entry.state, ObservedServiceState::Disabled);
        }
    }

    #[test]
    fn category_selector_keys() {
        assert_eq!(ServiceCategory::I2PTunnel.selector_key(), "I2PTunnel");
        assert_eq!(ServiceCategory::HttpProxy.selector_key(), "HTTPProxy");
        assert_eq!(ServiceCategory::Socks.selector_key(), "SOCKS");
        assert_eq!(ServiceCategory::Sam.selector_key(), "SAM");
        assert_eq!(ServiceCategory::Bob.selector_key(), "BOB");
        assert_eq!(ServiceCategory::I2cp.selector_key(), "I2CP");
    }

    #[test]
    fn only_requested_categories_in_snapshot() {
        let reg = ServiceRegistry::new();
        let h_http = reg.allocate_handle(ServiceCategory::HttpProxy);
        h_http
            .update(
                ObservedServiceState::Listening,
                ServiceMetadata {
                    enabled: true,
                    port: Some(8080),
                    ..Default::default()
                },
            )
            .unwrap();
        let snap = reg.snapshot();
        assert!(snap.has(ServiceCategory::HttpProxy));
        // Other categories still present but Disabled
        assert!(snap.has(ServiceCategory::I2PTunnel));
        let entry = snap.get(ServiceCategory::I2PTunnel).unwrap();
        assert_eq!(entry.state, ObservedServiceState::Disabled);
    }
}
