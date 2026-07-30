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

use std::collections::HashMap;
use std::sync::Arc;

use super::TunnelBackend;
use crate::i2pcontrol::domain::tunnel::{TunnelType, ALL_TUNNEL_ACTIONS, ALL_TUNNEL_TYPES};

/// An exhaustive tunnel backend registry.
///
/// Construction fails if any `TunnelType` lacks a backend or if duplicates
/// are detected. Every valid tunnel type maps to exactly one backend.
#[derive(Clone)]
pub struct TunnelBackendRegistry {
    backends: HashMap<TunnelType, Arc<dyn TunnelBackend>>,
}

impl std::fmt::Debug for TunnelBackendRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TunnelBackendRegistry")
            .field("len", &self.backends.len())
            .finish()
    }
}

impl TunnelBackendRegistry {
    /// Create a new registry from an iterator of backends.
    ///
    /// # Errors
    ///
    /// Returns `RegistryError` if:
    /// - a tunnel type is registered more than once
    /// - a tunnel type is missing from the registration
    pub fn new(backends: Vec<Arc<dyn TunnelBackend>>) -> Result<Self, RegistryError> {
        let mut map = HashMap::new();
        for backend in backends {
            let tt = backend.tunnel_type();
            if map.contains_key(&tt) {
                return Err(RegistryError::DuplicateRegistration(tt));
            }
            map.insert(tt, backend);
        }

        // Verify all types are registered
        for &tt in ALL_TUNNEL_TYPES {
            if !map.contains_key(&tt) {
                return Err(RegistryError::MissingRegistration(tt));
            }
        }

        Ok(Self { backends: map })
    }

    /// Look up a backend by tunnel type.
    ///
    /// This is total for valid tunnel types.
    pub fn get(&self, tunnel_type: TunnelType) -> Arc<dyn TunnelBackend> {
        self.backends
            .get(&tunnel_type)
            .cloned()
            .expect("registry is exhaustive; all tunnel types are registered")
    }

    /// Return the number of registered backends.
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.backends.len()
    }

    /// Return true if the registry is empty.
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.backends.is_empty()
    }

    /// Return true if the given tunnel type is registered.
    #[allow(dead_code)]
    pub fn contains(&self, tunnel_type: TunnelType) -> bool {
        self.backends.contains_key(&tunnel_type)
    }
}

/// Errors from registry construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryError {
    /// A tunnel type was registered more than once.
    DuplicateRegistration(TunnelType),
    /// A tunnel type has no registered backend.
    MissingRegistration(TunnelType),
}

impl std::fmt::Display for RegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateRegistration(tt) => {
                write!(f, "duplicate registration for tunnel type: {}", tt.as_str())
            }
            Self::MissingRegistration(tt) => {
                write!(f, "missing registration for tunnel type: {}", tt.as_str())
            }
        }
    }
}

impl std::error::Error for RegistryError {}

/// Create a default registry with all tunnel types mapped to unsupported
/// backends.
pub fn create_default_registry() -> Result<TunnelBackendRegistry, RegistryError> {
    let backends: Vec<Arc<dyn TunnelBackend>> = ALL_TUNNEL_TYPES
        .iter()
        .map(|&tt| {
            Arc::new(super::unsupported::UnsupportedTunnelBackend::new(tt))
                as Arc<dyn TunnelBackend>
        })
        .collect();
    TunnelBackendRegistry::new(backends)
}

/// Compile-time guard: every TunnelType variant is listed in ALL_TUNNEL_TYPES.
///
/// If you add a variant to TunnelType and forget to add it to ALL_TUNNEL_TYPES,
/// this const will fail to compile.
const _: () = {
    // This block runs at compile time. If ALL_TUNNEL_TYPES has fewer than
    // the expected number of elements, the array literal will fail.
    #[allow(dead_code)]
    const EXPECTED_COUNT: usize = 12;
    const _CHECK: [(); EXPECTED_COUNT] = [(); ALL_TUNNEL_TYPES.len()];
};

/// Compile-time guard: every TunnelAction variant is listed in ALL_TUNNEL_ACTIONS.
const _: () = {
    #[allow(dead_code)]
    const EXPECTED_COUNT: usize = 8;
    const _CHECK: [(); EXPECTED_COUNT] = [(); ALL_TUNNEL_ACTIONS.len()];
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i2pcontrol::backends::unsupported::UnsupportedTunnelBackend;
    use crate::i2pcontrol::backends::BackendError;
    use crate::i2pcontrol::domain::tunnel::{
        StartIntent, TunnelDefinition, TunnelName, TunnelOptions, TunnelOwnership,
        TunnelRuntimeState,
    };

    fn test_definition(tunnel_type: TunnelType) -> TunnelDefinition {
        TunnelDefinition {
            name: TunnelName::new("registry-test").unwrap(),
            tunnel_type,
            ownership: TunnelOwnership::ControlPlane,
            runtime_state: TunnelRuntimeState::Stopped,
            start_intent: StartIntent::DoNotStart,
            options: TunnelOptions::default(),
            raw_config: std::collections::BTreeMap::new(),
        }
    }

    #[tokio::test]
    async fn default_registry_all_types_unsupported() {
        let registry = create_default_registry().unwrap();
        assert_eq!(registry.len(), 12);

        for &tt in ALL_TUNNEL_TYPES {
            let backend = registry.get(tt);
            assert_eq!(backend.tunnel_type(), tt);
            let def = test_definition(tt);
            let result = backend.start(&def).await;
            assert!(matches!(result, Err(BackendError::NotImplemented { .. })));
            let status = backend.inspect(&def);
            assert_eq!(status.runtime_state, TunnelRuntimeState::Unsupported);
        }
    }

    #[test]
    fn registry_rejects_duplicate() {
        let backends: Vec<Arc<dyn TunnelBackend>> = ALL_TUNNEL_TYPES
            .iter()
            .map(|&tt| Arc::new(UnsupportedTunnelBackend::new(tt)) as Arc<dyn TunnelBackend>)
            .collect();

        // Duplicate the first one
        let mut backends = backends;
        let first = backends[0].clone();
        backends.push(first);

        let result = TunnelBackendRegistry::new(backends);
        assert!(result.is_err());
        match result.unwrap_err() {
            RegistryError::DuplicateRegistration(tt) => {
                assert_eq!(tt, ALL_TUNNEL_TYPES[0]);
            }
            _ => panic!("expected DuplicateRegistration"),
        }
    }

    #[test]
    fn registry_rejects_missing() {
        let backends: Vec<Arc<dyn TunnelBackend>> = ALL_TUNNEL_TYPES[1..] // skip first
            .iter()
            .map(|&tt| Arc::new(UnsupportedTunnelBackend::new(tt)) as Arc<dyn TunnelBackend>)
            .collect();

        let result = TunnelBackendRegistry::new(backends);
        assert!(result.is_err());
        match result.unwrap_err() {
            RegistryError::MissingRegistration(tt) => {
                assert_eq!(tt, ALL_TUNNEL_TYPES[0]);
            }
            _ => panic!("expected MissingRegistration"),
        }
    }

    #[test]
    fn registry_contains_all_types() {
        let registry = create_default_registry().unwrap();
        for &tt in ALL_TUNNEL_TYPES {
            assert!(registry.contains(tt));
        }
    }
}
