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

use std::{collections::HashMap, sync::Arc};

use parking_lot::RwLock;

use super::{BackendError, BackendResult, BackendStatus, TunnelBackend};
use crate::i2pcontrol::domain::tunnel::{TunnelDefinition, TunnelRuntimeState, TunnelType};

/// Scripted behavior for a fake backend operation.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum FakeAction {
    /// Operation succeeds.
    Success,
    /// Operation fails with the given error.
    Error(BackendError),
}

/// Configuration for a fake backend's scripted behavior.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct FakeBackendScript {
    pub start_action: FakeAction,
    pub stop_action: FakeAction,
    pub inspect_state: TunnelRuntimeState,
    pub inspect_message: String,
}

impl Default for FakeBackendScript {
    fn default() -> Self {
        Self {
            start_action: FakeAction::Success,
            stop_action: FakeAction::Success,
            inspect_state: TunnelRuntimeState::Stopped,
            inspect_message: "fake backend".to_string(),
        }
    }
}

/// A fake tunnel backend for deterministic testing.
///
/// Supports scripted success/failure/state for handler tests without
/// network activity. Thread-safe via `Arc<RwLock<>>`.
#[allow(dead_code)]
pub struct FakeTunnelBackend {
    tunnel_type: TunnelType,
    script: Arc<RwLock<FakeBackendScript>>,
}

#[allow(dead_code)]
impl FakeTunnelBackend {
    /// Create a new fake backend with the default script (all success).
    pub fn new(tunnel_type: TunnelType) -> Self {
        Self {
            tunnel_type,
            script: Arc::new(RwLock::new(FakeBackendScript::default())),
        }
    }

    /// Create with a custom script.
    pub fn with_script(tunnel_type: TunnelType, script: FakeBackendScript) -> Self {
        Self {
            tunnel_type,
            script: Arc::new(RwLock::new(script)),
        }
    }

    /// Update the script at runtime.
    pub fn set_script(&self, script: FakeBackendScript) {
        *self.script.write() = script;
    }

    /// Get a clone of the current script.
    pub fn script(&self) -> FakeBackendScript {
        self.script.read().clone()
    }
}

#[async_trait::async_trait]
impl TunnelBackend for FakeTunnelBackend {
    fn tunnel_type(&self) -> TunnelType {
        self.tunnel_type
    }

    async fn start(&self, _definition: &TunnelDefinition) -> BackendResult<()> {
        let script = self.script.read().clone();
        match script.start_action {
            FakeAction::Success => Ok(()),
            FakeAction::Error(err) => Err(err),
        }
    }

    async fn stop(&self, _definition: &TunnelDefinition) -> BackendResult<()> {
        let script = self.script.read().clone();
        match script.stop_action {
            FakeAction::Success => Ok(()),
            FakeAction::Error(err) => Err(err),
        }
    }

    fn inspect(&self, _definition: &TunnelDefinition) -> BackendStatus {
        let script = self.script.read().clone();
        BackendStatus {
            tunnel_type: self.tunnel_type,
            runtime_state: script.inspect_state,
            message: script.inspect_message,
        }
    }
}

/// An in-memory backend registry for tests.
#[allow(dead_code)]
pub struct FakeBackendRegistry {
    backends: HashMap<TunnelType, Arc<FakeTunnelBackend>>,
}

#[allow(dead_code)]
impl FakeBackendRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            backends: HashMap::new(),
        }
    }

    /// Register a fake backend.
    pub fn register(&mut self, backend: Arc<FakeTunnelBackend>) {
        self.backends.insert(backend.tunnel_type(), backend);
    }

    /// Get a backend by tunnel type.
    pub fn get(&self, tunnel_type: TunnelType) -> Option<Arc<FakeTunnelBackend>> {
        self.backends.get(&tunnel_type).cloned()
    }

    /// Return the number of registered backends.
    pub fn len(&self) -> usize {
        self.backends.len()
    }

    /// Return true if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.backends.is_empty()
    }
}

impl Default for FakeBackendRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i2pcontrol::domain::tunnel::{
        StartIntent, TunnelName, TunnelOptions, TunnelOwnership,
    };

    fn test_definition(tunnel_type: TunnelType) -> TunnelDefinition {
        TunnelDefinition {
            name: TunnelName::new("fake-test").unwrap(),
            tunnel_type,
            ownership: TunnelOwnership::ControlPlane,
            runtime_state: TunnelRuntimeState::Stopped,
            start_intent: StartIntent::DoNotStart,
            options: TunnelOptions::default(),
            raw_config: std::collections::BTreeMap::new(),
        }
    }

    #[tokio::test]
    async fn fake_default_script_succeeds() {
        let backend = FakeTunnelBackend::new(TunnelType::Client);
        let def = test_definition(TunnelType::Client);
        assert!(backend.start(&def).await.is_ok());
        assert!(backend.stop(&def).await.is_ok());
        let status = backend.inspect(&def);
        assert_eq!(status.runtime_state, TunnelRuntimeState::Stopped);
    }

    #[tokio::test]
    async fn fake_scripted_failure() {
        let script = FakeBackendScript {
            start_action: FakeAction::Error(BackendError::NotImplemented {
                tunnel_type: TunnelType::Socks,
            }),
            ..Default::default()
        };
        let backend = FakeTunnelBackend::with_script(TunnelType::Socks, script);
        let def = test_definition(TunnelType::Socks);
        let result = backend.start(&def).await;
        assert!(result.is_err());
    }

    #[test]
    fn fake_scripted_inspect_state() {
        let script = FakeBackendScript {
            inspect_state: TunnelRuntimeState::Running,
            inspect_message: "running in test".to_string(),
            ..Default::default()
        };
        let backend = FakeTunnelBackend::with_script(TunnelType::Server, script);
        let def = test_definition(TunnelType::Server);
        let status = backend.inspect(&def);
        assert_eq!(status.runtime_state, TunnelRuntimeState::Running);
        assert_eq!(status.message, "running in test");
    }

    #[test]
    fn fake_registry_operations() {
        let mut registry = FakeBackendRegistry::new();
        assert!(registry.is_empty());

        let backend = Arc::new(FakeTunnelBackend::new(TunnelType::HttpServer));
        registry.register(backend);
        assert_eq!(registry.len(), 1);

        let found = registry.get(TunnelType::HttpServer);
        assert!(found.is_some());
        assert!(registry.get(TunnelType::Client).is_none());
    }
}
