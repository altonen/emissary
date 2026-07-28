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

/// Control plane interface for I2PControl method handlers.
///
/// This trait defines the typed internal boundary used by JSON-RPC handlers.
/// It coordinates authentication-independent operations for router inspection,
/// address books, tunnel definitions, service inspection, logs, and persistence.
///
/// M001 provides a minimal interface with a fake implementation for tests.
/// Later milestones extend this with real router inspection, address book,
/// tunnel management, and client service adapters.
pub trait ControlPlane: Send + Sync {
    /// Get a list of all tunnel names and their types.
    fn tunnel_list(&self) -> Result<HashMap<String, String>, String>;

    /// Get a tunnel definition by name.
    fn tunnel_get(&self, name: &str) -> Result<Option<serde_json::Value>, String>;

    /// Check if a tunnel type is supported for runtime operations.
    fn is_tunnel_type_supported(&self, tunnel_type: &str) -> bool;

    /// Get the router identity (base64 RouterInfo).
    fn router_identity(&self) -> Result<String, String>;

    /// Get router uptime in milliseconds.
    fn router_uptime_ms(&self) -> u64;

    /// Get router version string.
    fn router_version(&self) -> String;
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
    fn tunnel_list(&self) -> Result<HashMap<String, String>, String> {
        Ok(HashMap::new())
    }

    fn tunnel_get(&self, _name: &str) -> Result<Option<serde_json::Value>, String> {
        Ok(None)
    }

    fn is_tunnel_type_supported(&self, _tunnel_type: &str) -> bool {
        false
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_control_plane_returns_stubs() {
        let cp = FakeControlPlane::new();
        assert!(cp.tunnel_list().unwrap().is_empty());
        assert!(cp.tunnel_get("test").unwrap().is_none());
        assert!(!cp.is_tunnel_type_supported("client"));
        assert_eq!(cp.router_uptime_ms(), 0);
    }
}
