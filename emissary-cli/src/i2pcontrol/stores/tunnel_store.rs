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
use crate::i2pcontrol::domain::revision::StateRevision;
use crate::i2pcontrol::domain::tunnel::TunnelDefinition;

/// Persistent tunnel definitions envelope payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TunnelStorePayload {
    /// Tunnel definitions keyed by tunnel name.
    pub tunnels: BTreeMap<String, TunnelDefinition>,
}

impl TunnelStorePayload {
    /// Create an empty payload.
    pub fn empty() -> Self {
        Self {
            tunnels: BTreeMap::new(),
        }
    }
}

/// Persistent tunnel definition store.
///
/// Stores all Proposal 170 tunnel definitions using versioned generation
/// persistence. Thread-safe via async mutation serialization.
pub struct TunnelStore {
    inner: GenerationStore<TunnelStorePayload>,
}

impl TunnelStore {
    /// Create a new tunnel store.
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

    /// List all tunnel definitions.
    pub fn list(&self) -> Vec<&TunnelDefinition> {
        self.inner.current().map(|p| p.tunnels.values().collect()).unwrap_or_default()
    }

    /// Get a tunnel definition by name.
    pub fn get(&self, name: &str) -> Option<&TunnelDefinition> {
        self.inner.current().and_then(|p| p.tunnels.get(name))
    }

    /// Add or replace a tunnel definition.
    pub async fn upsert(&mut self, definition: TunnelDefinition) -> StoreResult<StateRevision> {
        let name = definition.name.as_str().to_string();
        let current = self.inner.current().cloned().unwrap_or_else(TunnelStorePayload::empty);
        let mut tunnels = current.tunnels;
        tunnels.insert(name, definition);
        let payload = TunnelStorePayload { tunnels };
        self.inner.publish(payload, |_| Ok(())).await
    }

    /// Remove a tunnel definition by name.
    pub async fn remove(&mut self, name: &str) -> StoreResult<Option<StateRevision>> {
        let current = match self.inner.current() {
            Some(p) => p.clone(),
            None => return Ok(None),
        };
        let mut tunnels = current.tunnels;
        let removed = tunnels.remove(name);
        if removed.is_none() {
            return Ok(None);
        }
        let payload = TunnelStorePayload { tunnels };
        let rev = self.inner.publish(payload, |_| Ok(())).await?;
        Ok(Some(rev))
    }

    /// Return the number of stored tunnel definitions.
    pub fn len(&self) -> usize {
        self.inner.current().map(|p| p.tunnels.len()).unwrap_or(0)
    }

    /// Return true if the store is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Return true if a tunnel with the given name exists.
    pub fn contains(&self, name: &str) -> bool {
        self.inner.current().map(|p| p.tunnels.contains_key(name)).unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i2pcontrol::domain::tunnel::{
        StartIntent, TunnelName, TunnelOptions, TunnelOwnership, TunnelRuntimeState, TunnelType,
    };

    fn test_dir() -> PathBuf {
        tempfile::tempdir().unwrap().keep()
    }

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

    #[tokio::test]
    async fn empty_store() {
        let dir = test_dir();
        let mut store = TunnelStore::new(dir.clone(), 1024 * 1024);
        let loaded = store.load().await.unwrap();
        assert_eq!(loaded, None);
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
        assert!(store.list().is_empty());
        assert!(store.get("test").is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn upsert_and_get() {
        let dir = test_dir();
        let mut store = TunnelStore::new(dir.clone(), 1024 * 1024);

        let def = test_definition("my-tunnel", TunnelType::Socks);
        store.upsert(def.clone()).await.unwrap();

        let found = store.get("my-tunnel").unwrap();
        assert_eq!(found.name.as_str(), "my-tunnel");
        assert_eq!(found.tunnel_type, TunnelType::Socks);
        assert_eq!(store.len(), 1);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn remove_tunnel() {
        let dir = test_dir();
        let mut store = TunnelStore::new(dir.clone(), 1024 * 1024);

        store.upsert(test_definition("t1", TunnelType::Client)).await.unwrap();
        store.upsert(test_definition("t2", TunnelType::Server)).await.unwrap();
        assert_eq!(store.len(), 2);

        let removed = store.remove("t1").await.unwrap();
        assert!(removed.is_some());
        assert_eq!(store.len(), 1);
        assert!(store.get("t1").is_none());
        assert!(store.get("t2").is_some());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn remove_nonexistent_returns_none() {
        let dir = test_dir();
        let mut store = TunnelStore::new(dir.clone(), 1024 * 1024);
        let removed = store.remove("nonexistent").await.unwrap();
        assert!(removed.is_none());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn round_trip_persistence() {
        let dir = test_dir();
        {
            let mut store = TunnelStore::new(dir.clone(), 1024 * 1024);
            store.upsert(test_definition("t1", TunnelType::HttpClient)).await.unwrap();
            store.upsert(test_definition("t2", TunnelType::IrcServer)).await.unwrap();
        }

        // Reload from disk
        {
            let mut store = TunnelStore::new(dir.clone(), 1024 * 1024);
            let loaded = store.load().await.unwrap();
            assert!(loaded.is_some());
            assert_eq!(store.len(), 2);
            assert!(store.get("t1").is_some());
            assert!(store.get("t2").is_some());
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn unsupported_tunnel_persistence() {
        let dir = test_dir();
        let mut store = TunnelStore::new(dir.clone(), 1024 * 1024);

        let def = TunnelDefinition {
            name: TunnelName::new("unsupported-test").unwrap(),
            tunnel_type: TunnelType::StreamrServer,
            ownership: TunnelOwnership::Unsupported,
            runtime_state: TunnelRuntimeState::Unsupported,
            start_intent: StartIntent::DoNotStart,
            options: TunnelOptions::default(),
            raw_config: std::collections::BTreeMap::new(),
        };
        store.upsert(def.clone()).await.unwrap();

        let found = store.get("unsupported-test").unwrap();
        assert_eq!(found.ownership, TunnelOwnership::Unsupported);
        assert_eq!(found.runtime_state, TunnelRuntimeState::Unsupported);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn contains_method() {
        let dir = test_dir();
        let mut store = TunnelStore::new(dir.clone(), 1024 * 1024);
        assert!(!store.contains("test"));

        store.upsert(test_definition("test", TunnelType::Client)).await.unwrap();
        assert!(store.contains("test"));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
