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

pub mod fake;
pub mod registry;
pub mod unsupported;

use std::fmt;

use super::domain::tunnel::{TunnelDefinition, TunnelRuntimeState, TunnelType};

/// Errors produced by tunnel backend operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackendError {
    /// The tunnel type is not supported by this backend.
    NotImplemented { tunnel_type: TunnelType },
    /// The tunnel is not in a state where the operation can be performed.
    InvalidState {
        tunnel_type: TunnelType,
        current_state: TunnelRuntimeState,
        attempted_action: &'static str,
    },
    /// An internal backend error occurred.
    Internal { message: String },
}

impl fmt::Display for BackendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotImplemented { tunnel_type } => {
                write!(f, "error - {} not implemented", tunnel_type.as_str())
            }
            Self::InvalidState {
                tunnel_type,
                current_state,
                attempted_action,
            } => {
                write!(
                    f,
                    "error - {} {} failed: tunnel is {}",
                    tunnel_type.as_str(),
                    attempted_action,
                    current_state
                )
            }
            Self::Internal { message } => {
                write!(f, "internal error: {}", message)
            }
        }
    }
}

impl std::error::Error for BackendError {}

/// Result type for backend operations.
pub type BackendResult<T> = Result<T, BackendError>;

/// Status information returned by backend inspect operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendStatus {
    /// The tunnel type.
    pub tunnel_type: TunnelType,

    /// The current runtime state.
    pub runtime_state: TunnelRuntimeState,

    /// Human-readable status message.
    pub message: String,
}

/// Trait defining the interface for tunnel runtime backends.
///
/// Each tunnel type resolves to exactly one backend. The backend is
/// independent from JSON-RPC and persistence policy.
///
/// # Contract
///
/// - `start` must not allocate listeners, destinations, sessions, tasks,
///   or traffic paths for unsupported backends.
/// - `stop` of an inactive definition must be safe and resource-free.
/// - `inspect` must return the current state without side effects.
/// - All methods must honor caller deadlines without blocking.
#[async_trait::async_trait]
pub trait TunnelBackend: Send + Sync {
    /// Return the tunnel type this backend handles.
    fn tunnel_type(&self) -> TunnelType;

    /// Start a tunnel with the given definition.
    ///
    /// Returns `NotImplemented` for unsupported backends.
    async fn start(&self, definition: &TunnelDefinition) -> BackendResult<()>;

    /// Stop a tunnel with the given definition.
    ///
    /// For unsupported backends, this is a safe no-op.
    async fn stop(&self, definition: &TunnelDefinition) -> BackendResult<()>;

    /// Inspect the current state of a tunnel.
    fn inspect(&self, definition: &TunnelDefinition) -> BackendStatus;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i2pcontrol::domain::tunnel::{
        StartIntent, TunnelName, TunnelOptions, TunnelOwnership,
    };

    fn test_definition(tunnel_type: TunnelType) -> TunnelDefinition {
        TunnelDefinition {
            name: TunnelName::new("test-tunnel").unwrap(),
            tunnel_type,
            ownership: TunnelOwnership::ControlPlane,
            runtime_state: TunnelRuntimeState::Stopped,
            start_intent: StartIntent::DoNotStart,
            options: TunnelOptions::default(),
            raw_config: std::collections::BTreeMap::new(),
        }
    }

    #[test]
    fn backend_error_not_implemented_display() {
        let err = BackendError::NotImplemented {
            tunnel_type: TunnelType::Socks,
        };
        let msg = format!("{}", err);
        assert_eq!(msg, "error - socks not implemented");
    }

    #[test]
    fn backend_error_invalid_state_display() {
        let err = BackendError::InvalidState {
            tunnel_type: TunnelType::HttpClient,
            current_state: TunnelRuntimeState::Running,
            attempted_action: "Start",
        };
        let msg = format!("{}", err);
        assert!(msg.contains("httpclient"));
        assert!(msg.contains("Start"));
        assert!(msg.contains("running"));
    }

    #[test]
    fn backend_status_fields() {
        let status = BackendStatus {
            tunnel_type: TunnelType::Server,
            runtime_state: TunnelRuntimeState::Stopped,
            message: "not running".to_string(),
        };
        assert_eq!(status.tunnel_type, TunnelType::Server);
        assert_eq!(status.runtime_state, TunnelRuntimeState::Stopped);
    }

    #[test]
    fn test_definition_helper() {
        let def = test_definition(TunnelType::Socks);
        assert_eq!(def.tunnel_type, TunnelType::Socks);
        assert_eq!(def.name.as_str(), "test-tunnel");
    }
}
