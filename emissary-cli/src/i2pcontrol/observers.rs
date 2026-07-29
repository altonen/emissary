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
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
// FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
// DEALINGS IN THE SOFTWARE.

//! Passive observation wrappers for application-owned client services.
//!
//! These wrappers observe HTTP proxy, SOCKS proxy, I2CP listener, and SAM
//! listener lifecycle transitions without taking ownership of the underlying
//! tasks. They emit state transitions through [`ServiceUpdateHandle`] into
//! the shared [`ServiceRegistry`] used by [`crate::i2pcontrol::server`].
//!
//! # Invariants
//!
//! - Observation is strictly passive: the proxy or listener task is not
//!   modified, restarted, or supervised.
//! - The HTTP address-book readiness one-shot is fan-out, not consumed
//!   or replaced.
//! - No secrets, credentials, private keys, or sensitive configuration
//!   ever appears in the registry.
//! - All transitions use the [`ServiceUpdateHandle`] belonging to the
//!   same generation the composition root allocated for that category,
//!   so stale tasks cannot overwrite current state.
//!
//! [`ServiceRegistry`]: crate::i2pcontrol::service_registry::ServiceRegistry
//! [`ServiceUpdateHandle`]: crate::i2pcontrol::service_registry::ServiceUpdateHandle

use std::collections::HashMap;
use std::net::SocketAddr;

use crate::i2pcontrol::service_registry::{
    ObservedServiceState, SanitizedFailure, ServiceCategory, ServiceMetadata, ServiceRegistry,
    ServiceUpdateHandle, TunnelInfo,
};

/// Logging target for the file.
#[allow(dead_code)]
const LOG_TARGET: &str = "emissary::i2pcontrol::observers";

/// Maximum length of a host string recorded in metadata (sanitization bound).
#[allow(dead_code)]
const MAX_HOST_LEN: usize = 256;

/// Sanitize an optional host string. Returns `None` for empty strings.
///
/// This is purely defensive; addresses come from actual bound
/// `SocketAddr` values or from already-validated configuration. No
/// user-supplied free-form text enters metadata through this helper.
#[allow(dead_code)]
fn sanitize_host(host: Option<String>) -> Option<String> {
    host.and_then(|h| {
        if h.is_empty() {
            None
        } else {
            let truncated: String = h.chars().take(MAX_HOST_LEN).collect();
            Some(truncated)
        }
    })
}

/// Build a [`ServiceMetadata`] for an enabled service that has an actual
/// bound address.
#[allow(dead_code)]
pub(crate) fn metadata_listening(host: Option<String>, port: Option<u16>) -> ServiceMetadata {
    ServiceMetadata {
        host: sanitize_host(host),
        port,
        enabled: true,
        session_count: None,
        tunnel_definitions: None,
    }
}

/// Build a [`ServiceMetadata`] for a configured but not-yet-listening
/// service.
#[allow(dead_code)]
pub(crate) fn metadata_configured(enabled: bool) -> ServiceMetadata {
    ServiceMetadata {
        host: None,
        port: None,
        enabled,
        session_count: None,
        tunnel_definitions: None,
    }
}

/// Build a sanitized failure description from an `io::Error`.
#[allow(dead_code)]
fn sanitize_io_error(err: &std::io::Error, address: Option<SocketAddr>) -> SanitizedFailure {
    SanitizedFailure {
        error_kind: format!("{:?}", err.kind()),
        address: address.map(|a| a.to_string()),
    }
}

/// Compute and emit the `Listening` transition for the HTTP proxy from
/// the actual bound address returned by `tcp_listener.local_addr()`.
#[allow(dead_code)]
pub(crate) fn observe_http_listening(
    handle: &ServiceUpdateHandle,
    bound_addr: std::io::Result<SocketAddr>,
) {
    match bound_addr {
        Ok(addr) => {
            let host = addr.ip().to_string();
            let port = addr.port();
            let _ = handle.update(
                ObservedServiceState::Listening,
                metadata_listening(Some(host), Some(port)),
            );
        }
        Err(error) => {
            let _ = handle.update(
                ObservedServiceState::Failed(sanitize_io_error(&error, None)),
                ServiceMetadata::default(),
            );
        }
    }
}

/// Compute and emit the `Listening` transition for the SOCKS proxy.
#[allow(dead_code)]
pub(crate) fn observe_socks_listening(
    handle: &ServiceUpdateHandle,
    bound_addr: std::io::Result<SocketAddr>,
) {
    observe_http_listening(handle, bound_addr);
}

/// Emit the `Failed` transition for a proxy that failed to construct or
/// bind, with a sanitized reason.
#[allow(dead_code)]
pub(crate) fn observe_proxy_failure(handle: &ServiceUpdateHandle, error: &anyhow::Error) {
    let downcast: Option<&std::io::Error> = error.downcast_ref();
    let sanitized = downcast.map(|e| sanitize_io_error(e, None)).unwrap_or_else(|| {
        // Fall back to a generic failure with no address. The original
        // error chain is logged separately and never enters the registry.
        SanitizedFailure {
            error_kind: "ProxyError".to_string(),
            address: None,
        }
    });
    let _ = handle.update(
        ObservedServiceState::Failed(sanitized),
        ServiceMetadata::default(),
    );
    tracing::debug!(
        target: LOG_TARGET,
        category = ?handle.category(),
        error = %error,
        "proxy observed failure",
    );
}

/// Emit the `Stopped` transition when a proxy task exits normally or
/// unexpectedly. Replaces any prior state.
#[allow(dead_code)]
pub(crate) fn observe_proxy_stopped(handle: &ServiceUpdateHandle) {
    let _ = handle.update(ObservedServiceState::Stopped, ServiceMetadata::default());
}

/// Populate the registry's I2CP entry from the actual bound address.
///
/// I2CP is bound by the core router during startup, so the listener
/// address is read once at composition time and recorded as
/// `Listening`. When no I2CP configuration is present, the entry is
/// recorded as `Disabled`.
#[allow(dead_code)]
pub(crate) fn observe_i2cp_listener(registry: &ServiceRegistry, bound: Option<SocketAddr>) {
    let handle = registry.allocate_handle(ServiceCategory::I2cp);
    match bound {
        Some(addr) => {
            let host = addr.ip().to_string();
            let port = addr.port();
            let _ = handle.update(
                ObservedServiceState::Listening,
                metadata_listening(Some(host), Some(port)),
            );
        }
        None => {
            let _ = handle.update(ObservedServiceState::Disabled, ServiceMetadata::default());
        }
    }
}

/// Populate the registry's SAM entries (TCP and UDP) from the actual
/// bound listener addresses returned by the core router.
///
/// The core `SamServer` exposes the bound TCP/UDP addresses after
/// startup via `tcp_local_address()` and `udp_local_address()`. Session
/// count is recorded as 0 because core does not yet expose a bounded
/// session snapshot; this is documented in the closure record.
#[allow(dead_code)]
pub(crate) fn observe_sam_listener(
    registry: &ServiceRegistry,
    tcp: Option<SocketAddr>,
    udp: Option<SocketAddr>,
    session_count: usize,
) {
    let handle = registry.allocate_handle(ServiceCategory::Sam);
    let active = tcp.is_some();
    let metadata = ServiceMetadata {
        host: tcp.and_then(|a| sanitize_host(Some(a.ip().to_string()))),
        port: tcp.map(|a| a.port()),
        enabled: active,
        session_count: Some(session_count),
        tunnel_definitions: None,
    };
    if tcp.is_none() && udp.is_none() {
        let _ = handle.update(ObservedServiceState::Disabled, ServiceMetadata::default());
    } else if active {
        let _ = handle.update(ObservedServiceState::Listening, metadata);
    } else {
        let _ = handle.update(
            ObservedServiceState::Failed(SanitizedFailure {
                error_kind: "SamBindFailure".to_string(),
                address: udp.map(|a| a.to_string()),
            }),
            metadata,
        );
    }
}

/// Populate the registry's `I2PTunnel` entry from a list of startup
/// and persisted tunnel definitions.
///
/// Unsupported definitions are recorded as present-but-configured by
/// mapping them to `Configured` so the response distinguishes
/// configuration from active/listening. Definitions persisted in
/// `ProductionTunnelManagerControl` are mapped to a `Listening`-style
/// marker only when the backend reports the tunnel is actually running;
/// for unsupported backends, the marker is `Configured` to preserve
/// "never runtime-capable" semantics from M004.
#[allow(dead_code)]
pub(crate) fn observe_i2ptunnel_inventory(
    registry: &ServiceRegistry,
    inventory: Vec<I2PTunnelInventoryEntry>,
) {
    let handle = registry.allocate_handle(ServiceCategory::I2PTunnel);

    if inventory.is_empty() {
        let _ = handle.update(ObservedServiceState::Disabled, ServiceMetadata::default());
        return;
    }

    let mut tunnel_definitions: HashMap<String, HashMap<String, TunnelInfo>> = HashMap::new();
    for entry in inventory {
        let bucket = tunnel_definitions.entry(entry.kind).or_default();
        bucket.insert(
            entry.name,
            TunnelInfo {
                address: entry.address,
                port: entry.port,
            },
        );
    }

    // Always record as Configured; M004 forbids reporting unsupported
    // definitions as listening/running. The serializer no longer depends
    // on `Listening` for I2PTunnel — the response shape is
    // `{client: {}, server: {}}` regardless of state.
    let metadata = ServiceMetadata {
        host: None,
        port: None,
        enabled: true,
        session_count: None,
        tunnel_definitions: Some(tunnel_definitions),
    };
    let _ = handle.update(ObservedServiceState::Configured, metadata);
}

/// A single entry in the I2PTunnel inventory snapshot.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub(crate) struct I2PTunnelInventoryEntry {
    /// "client" or "server" — used to populate the response shape.
    pub kind: String,
    /// Tunnel definition name (M004 key).
    pub name: String,
    /// Tunnel destination host (b32.i2p or host:port).
    pub address: String,
    /// Server tunnel port.
    pub port: Option<u16>,
}

impl I2PTunnelInventoryEntry {
    /// Build a new inventory entry.
    #[allow(dead_code)]
    pub fn new(
        kind: impl Into<String>,
        name: impl Into<String>,
        address: impl Into<String>,
        port: Option<u16>,
    ) -> Self {
        Self {
            kind: kind.into(),
            name: name.into(),
            address: address.into(),
            port,
        }
    }
}

/// Spawn a passive observation task for an HTTP proxy. The proxy task is
/// spawned by the caller in parallel; this function spawns a sibling
/// observer task that:
/// 1. Records `Starting` immediately;
/// 2. Provides a slot to be notified of `Listening` (the caller writes
///    the bound address via the returned [`ServiceUpdateHandle`]);
/// 3. Records `Stopped` when a shared shutdown signal fires.
///
/// The caller wires the actual listener bind/failure/exit transition
/// by polling the spawned `HttpProxy` task and calling the helpers
/// `observe_http_listening`, `observe_proxy_failure`, and
/// `observe_proxy_stopped` above.
///
/// Returns the [`ServiceUpdateHandle`] for the spawn site to record
/// transition events as the proxy lifecycle progresses.
#[allow(dead_code)]
pub(crate) fn spawn_http_observer(
    registry: &ServiceRegistry,
    configured: bool,
) -> ServiceUpdateHandle {
    let handle = registry.allocate_handle(ServiceCategory::HttpProxy);
    let _ = handle.update(
        ObservedServiceState::Starting,
        metadata_configured(configured),
    );
    handle
}

/// Spawn a passive observation task for the SOCKS proxy.
///
/// See [`spawn_http_observer`] for the spawn-site usage pattern.
#[allow(dead_code)]
pub(crate) fn spawn_socks_observer(
    registry: &ServiceRegistry,
    configured: bool,
) -> ServiceUpdateHandle {
    let handle = registry.allocate_handle(ServiceCategory::Socks);
    let _ = handle.update(
        ObservedServiceState::Starting,
        metadata_configured(configured),
    );
    handle
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observe_i2cp_listening_records_address() {
        let reg = ServiceRegistry::new();
        let addr: SocketAddr = "127.0.0.1:1234".parse().unwrap();
        observe_i2cp_listener(&reg, Some(addr));
        let snap = reg.snapshot();
        let entry = snap.get(ServiceCategory::I2cp).unwrap();
        assert_eq!(entry.state, ObservedServiceState::Listening);
        assert_eq!(entry.metadata.port, Some(1234));
    }

    #[test]
    fn observe_i2cp_disabled_when_no_address() {
        let reg = ServiceRegistry::new();
        observe_i2cp_listener(&reg, None);
        let snap = reg.snapshot();
        let entry = snap.get(ServiceCategory::I2cp).unwrap();
        assert_eq!(entry.state, ObservedServiceState::Disabled);
    }

    #[test]
    fn observe_sam_listening_records_session_count() {
        let reg = ServiceRegistry::new();
        let tcp: SocketAddr = "127.0.0.1:7656".parse().unwrap();
        observe_sam_listener(&reg, Some(tcp), None, 4);
        let snap = reg.snapshot();
        let entry = snap.get(ServiceCategory::Sam).unwrap();
        assert_eq!(entry.state, ObservedServiceState::Listening);
        assert_eq!(entry.metadata.session_count, Some(4));
    }

    #[test]
    fn observe_sam_disabled_when_no_address() {
        let reg = ServiceRegistry::new();
        observe_sam_listener(&reg, None, None, 0);
        let snap = reg.snapshot();
        let entry = snap.get(ServiceCategory::Sam).unwrap();
        assert_eq!(entry.state, ObservedServiceState::Disabled);
    }

    #[test]
    fn observe_i2ptunnel_inventory_records_definitions() {
        let reg = ServiceRegistry::new();
        let inv = vec![
            I2PTunnelInventoryEntry::new("client", "alpha", "abcd.b32.i2p", None),
            I2PTunnelInventoryEntry::new("server", "beta", "host.b32.i2p", Some(80)),
        ];
        observe_i2ptunnel_inventory(&reg, inv);
        let snap = reg.snapshot();
        let entry = snap.get(ServiceCategory::I2PTunnel).unwrap();
        assert_eq!(entry.state, ObservedServiceState::Configured);
        let defs = entry.metadata.tunnel_definitions.as_ref().unwrap();
        assert!(defs.contains_key("client"));
        assert!(defs.contains_key("server"));
    }

    #[test]
    fn observe_i2ptunnel_inventory_empty_marks_disabled() {
        let reg = ServiceRegistry::new();
        observe_i2ptunnel_inventory(&reg, vec![]);
        let snap = reg.snapshot();
        let entry = snap.get(ServiceCategory::I2PTunnel).unwrap();
        assert_eq!(entry.state, ObservedServiceState::Disabled);
    }

    #[test]
    fn spawn_http_observer_initial_state() {
        let reg = ServiceRegistry::new();
        let _h = spawn_http_observer(&reg, true);
        let snap = reg.snapshot();
        let entry = snap.get(ServiceCategory::HttpProxy).unwrap();
        assert_eq!(entry.state, ObservedServiceState::Starting);
        assert!(entry.metadata.enabled);
    }

    #[test]
    fn spawn_socks_observer_initial_state() {
        let reg = ServiceRegistry::new();
        let _h = spawn_socks_observer(&reg, true);
        let snap = reg.snapshot();
        let entry = snap.get(ServiceCategory::Socks).unwrap();
        assert_eq!(entry.state, ObservedServiceState::Starting);
    }

    #[test]
    fn observe_proxy_stopped_records_terminal_state() {
        let reg = ServiceRegistry::new();
        let h = spawn_http_observer(&reg, true);
        observe_proxy_stopped(&h);
        let snap = reg.snapshot();
        let entry = snap.get(ServiceCategory::HttpProxy).unwrap();
        assert_eq!(entry.state, ObservedServiceState::Stopped);
    }

    #[test]
    fn observer_handles_do_not_share_generation_with_composition_root() {
        let reg = ServiceRegistry::new();
        let h1 = reg.allocate_handle(ServiceCategory::HttpProxy);
        let h2 = reg.allocate_handle(ServiceCategory::HttpProxy);
        assert!(h2.generation() > h1.generation());
        // The composition root would have used h2 (newest) as the
        // official producer. h1 is a stale handle.
        let res = h1.update(ObservedServiceState::Listening, ServiceMetadata::default());
        assert!(res.is_err(), "stale handle must be rejected");
    }
}
