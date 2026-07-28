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

use super::{BackendError, BackendResult, BackendStatus, TunnelBackend};
use crate::i2pcontrol::domain::tunnel::{TunnelDefinition, TunnelRuntimeState, TunnelType};

/// A tunnel backend for unsupported tunnel types.
///
/// This backend:
/// - is constructible for any declared tunnel type;
/// - returns typed `NotImplemented` from `start` and `restart` composition;
/// - treats `stop` of an inactive definition as safe and resource-free;
/// - inspects as internal `Unsupported`;
/// - never spawns or binds anything.
pub struct UnsupportedTunnelBackend {
    tunnel_type: TunnelType,
}

impl UnsupportedTunnelBackend {
    /// Create a new unsupported backend for the given tunnel type.
    pub fn new(tunnel_type: TunnelType) -> Self {
        Self { tunnel_type }
    }
}

#[async_trait::async_trait]
impl TunnelBackend for UnsupportedTunnelBackend {
    fn tunnel_type(&self) -> TunnelType {
        self.tunnel_type
    }

    async fn start(&self, _definition: &TunnelDefinition) -> BackendResult<()> {
        Err(BackendError::NotImplemented {
            tunnel_type: self.tunnel_type,
        })
    }

    async fn stop(&self, _definition: &TunnelDefinition) -> BackendResult<()> {
        // Stop of an inactive definition is safe and resource-free.
        Ok(())
    }

    fn inspect(&self, _definition: &TunnelDefinition) -> BackendStatus {
        BackendStatus {
            tunnel_type: self.tunnel_type,
            runtime_state: TunnelRuntimeState::Unsupported,
            message: format!("{} backend is not implemented", self.tunnel_type.as_str()),
        }
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
            name: TunnelName::new("test-tunnel").unwrap(),
            tunnel_type,
            ownership: TunnelOwnership::Unsupported,
            runtime_state: TunnelRuntimeState::Stopped,
            start_intent: StartIntent::DoNotStart,
            options: TunnelOptions::default(),
            raw_config: std::collections::BTreeMap::new(),
        }
    }

    #[tokio::test]
    async fn unsupported_start_returns_not_implemented() {
        let backend = UnsupportedTunnelBackend::new(TunnelType::Socks);
        let def = test_definition(TunnelType::Socks);
        let result = backend.start(&def).await;
        assert_eq!(
            result,
            Err(BackendError::NotImplemented {
                tunnel_type: TunnelType::Socks,
            })
        );
    }

    #[tokio::test]
    async fn unsupported_stop_is_safe_noop() {
        let backend = UnsupportedTunnelBackend::new(TunnelType::HttpClient);
        let def = test_definition(TunnelType::HttpClient);
        let result = backend.stop(&def).await;
        assert!(result.is_ok());
    }

    #[test]
    fn unsupported_inspect_returns_unsupported_state() {
        let backend = UnsupportedTunnelBackend::new(TunnelType::IrcServer);
        let def = test_definition(TunnelType::IrcServer);
        let status = backend.inspect(&def);
        assert_eq!(status.tunnel_type, TunnelType::IrcServer);
        assert_eq!(status.runtime_state, TunnelRuntimeState::Unsupported);
        assert!(status.message.contains("ircserver"));
    }

    #[test]
    fn unsupported_backend_tunnel_type_matches() {
        for tt in crate::i2pcontrol::domain::tunnel::ALL_TUNNEL_TYPES {
            let backend = UnsupportedTunnelBackend::new(*tt);
            assert_eq!(backend.tunnel_type(), *tt);
        }
    }

    #[test]
    fn unsupported_backend_display_error_message() {
        let backend = UnsupportedTunnelBackend::new(TunnelType::StreamrServer);
        let err = BackendError::NotImplemented {
            tunnel_type: backend.tunnel_type(),
        };
        let msg = format!("{}", err);
        assert!(msg.contains("streamrserver"));
        assert!(msg.contains("not implemented"));
    }
}
