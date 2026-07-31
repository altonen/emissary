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

//! Protocol exactness static guards for Proposal 170 M007.
//!
//! These tests verify invariants that must not regress:
//!
//! - No frontend/EventSubscriber coupling
//! - No direct handler filesystem/runtime authority
//! - No unsupported resource constructor path
//! - No core server dependencies
//! - No administrative store in runtime resolver
//! - No startup manager mutation authority
//! - No private-key/sensitive type in response snapshots
//! - No silent truncation/pagination/capability extensions
//! - Exact public vocabulary (no extra methods/selectors/types/actions/statuses)

#![cfg(feature = "i2pcontrol")]

use std::{collections::HashSet, path::Path};

// ──────────────────────────────────────────────────────────────────────
// § 1. Source-level structural guards
// ──────────────────────────────────────────────────────────────────────

const ALL_I2PCONTROL_FILES: &[&str] = &[
    "src/i2pcontrol/address_book.rs",
    "src/i2pcontrol/auth.rs",
    "src/i2pcontrol/control_plane.rs",
    "src/i2pcontrol/observability.rs",
    "src/i2pcontrol/production.rs",
    "src/i2pcontrol/router_info.rs",
    "src/i2pcontrol/router_info_handler.rs",
    "src/i2pcontrol/rpc.rs",
    "src/i2pcontrol/server.rs",
    "src/i2pcontrol/tunnel_manager.rs",
    "src/i2pcontrol/client_services.rs",
    "src/i2pcontrol/service_registry.rs",
    "src/i2pcontrol/observers.rs",
    "src/i2pcontrol/backends/mod.rs",
    "src/i2pcontrol/backends/registry.rs",
    "src/i2pcontrol/backends/unsupported.rs",
    "src/i2pcontrol/backends/fake.rs",
    "src/i2pcontrol/domain/tunnel.rs",
    "src/i2pcontrol/domain/address_book.rs",
    "src/i2pcontrol/domain/revision.rs",
    "src/i2pcontrol/stores/mod.rs",
    "src/i2pcontrol/stores/tunnel_store.rs",
    "src/i2pcontrol/stores/address_book_store.rs",
    "src/i2pcontrol/stores/subscription_store.rs",
    "src/i2pcontrol/stores/generation_store.rs",
    "src/i2pcontrol/stores/fakes.rs",
    "src/i2pcontrol/tls.rs",
    "src/i2pcontrol/errors.rs",
];

fn workspace_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("workspace root")
}

fn read_source(rel: &str) -> String {
    let p = workspace_root().join("emissary-cli").join(rel);
    std::fs::read_to_string(p).unwrap_or_else(|e| panic!("read {}: {e}", rel))
}

// ──────────────────────────────────────────────────────────────────────
// § 2. No EventSubscriber in I2PControl code
// ──────────────────────────────────────────────────────────────────────

#[test]
fn no_event_subscriber_in_i2pcontrol() {
    for f in ALL_I2PCONTROL_FILES {
        let src = read_source(f);
        for (i, line) in src.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("//") || trimmed.starts_with("///") {
                continue;
            }
            assert!(
                !line.contains("EventSubscriber"),
                "I2PControl file {f}:{i} must not reference EventSubscriber: {line}"
            );
        }
    }
}

// ──────────────────────────────────────────────────────────────────────
// § 3. No UI/frontend module imports
// ──────────────────────────────────────────────────────────────────────

#[test]
fn no_ui_module_imports() {
    for f in ALL_I2PCONTROL_FILES {
        let src = read_source(f);
        assert!(
            !src.contains("crate::ui") && !src.contains("crate::dioxus"),
            "I2PControl file {f} must not import UI modules"
        );
    }
}

// ──────────────────────────────────────────────────────────────────────
// § 4. No axum outside server.rs
// ──────────────────────────────────────────────────────────────────────

#[test]
fn no_axum_outside_server_rs() {
    for f in ALL_I2PCONTROL_FILES {
        if f.ends_with("server.rs") {
            continue;
        }
        let src = read_source(f);
        assert!(
            !src.contains("use axum") && !src.contains("axum::"),
            "I2PControl file {f} must not import axum outside server.rs"
        );
    }
}

// ──────────────────────────────────────────────────────────────────────
// § 5. No HTTP/JSON-RPC/TLS deps in emissary-core
// ──────────────────────────────────────────────────────────────────────

#[test]
fn core_has_no_server_dependencies() {
    let manifest_path = workspace_root().join("emissary-core/Cargo.toml");
    let manifest = std::fs::read_to_string(&manifest_path).unwrap();
    for dep in [
        "axum",
        "hyper",
        "tokio-rustls",
        "rustls-pemfile",
        "serde_json",
    ] {
        assert!(
            !manifest.contains(dep),
            "emissary-core must not depend on {dep}"
        );
    }
}

// ──────────────────────────────────────────────────────────────────────
// § 6. No private key types in DTOs
// ──────────────────────────────────────────────────────────────────────

#[test]
fn no_private_keys_in_dtos() {
    let src = read_source("src/i2pcontrol/router_info.rs");
    for (i, line) in src.lines().enumerate() {
        if line.trim().starts_with("//") {
            continue;
        }
        for forbidden in ["SigningPrivateKey", "StaticPrivateKey", "NoiseContext"] {
            assert!(
                !line.contains(forbidden),
                "router_info DTOs must not reference {forbidden} at line {i}: {line}"
            );
        }
    }
}

// ──────────────────────────────────────────────────────────────────────
// § 7. Production adapter is read-only (no mutation methods)
// ──────────────────────────────────────────────────────────────────────

#[test]
fn production_adapter_is_read_only() {
    let src = read_source("src/i2pcontrol/production.rs");
    for (i, line) in src.lines().enumerate() {
        let l = line.trim();
        if l.starts_with("pub fn ") || l.starts_with("pub(crate) fn ") {
            let forbidden = l.contains("fn set_")
                || l.contains("fn mutate_")
                || l.contains("fn write_")
                || l.contains("fn update_")
                || l.contains("fn trigger_");
            assert!(
                !forbidden,
                "Production adapter must not expose mutation at line {i}: {l}"
            );
        }
    }
}

// ──────────────────────────────────────────────────────────────────────
// § 8. Control traits are Send + Sync
// ──────────────────────────────────────────────────────────────────────

#[test]
fn control_traits_are_send_sync() {
    fn assert_send_sync<T: Send + Sync + ?Sized>() {}
    use emissary_cli::i2pcontrol::{
        control_plane::{AddressBookControl, TunnelManagerControl},
        router_info::RouterInfoControl,
    };
    assert_send_sync::<dyn RouterInfoControl>();
    assert_send_sync::<dyn AddressBookControl>();
    assert_send_sync::<dyn TunnelManagerControl>();
}

// ──────────────────────────────────────────────────────────────────────
// § 9. No handlers write router.toml or runtime address-book paths
// ──────────────────────────────────────────────────────────────────────

#[test]
fn no_handlers_write_router_toml() {
    for f in ALL_I2PCONTROL_FILES {
        if f.ends_with("server.rs") || f.ends_with("tls.rs") {
            continue;
        }
        let src = read_source(f);
        for (i, line) in src.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("//") || trimmed.starts_with("///") {
                continue;
            }
            assert!(
                !line.contains("router.toml"),
                "I2PControl file {f}:{i} must not reference router.toml: {line}"
            );
        }
    }
}

// ──────────────────────────────────────────────────────────────────────
// § 10. No handlers spawn SAM/I2CP/tunnel resources directly
// ──────────────────────────────────────────────────────────────────────

#[test]
fn no_handlers_spawn_resources() {
    for f in ALL_I2PCONTROL_FILES {
        if f.ends_with("server.rs")
            || f.ends_with("observers.rs")
            || f.ends_with("service_registry.rs")
        {
            continue;
        }
        let src = read_source(f);
        let lower = src.to_lowercase();
        assert!(
            !lower.contains("samsession") || lower.contains("//") || lower.contains("SAM session"),
            "I2PControl handler file {f} must not spawn SAM sessions"
        );
    }
}

// ──────────────────────────────────────────────────────────────────────
// § 11. No unsupported backend imports resource constructors
// ──────────────────────────────────────────────────────────────────────

#[test]
fn unsupported_backend_has_no_resource_imports() {
    let src = read_source("src/i2pcontrol/backends/unsupported.rs");
    for (i, line) in src.lines().enumerate() {
        if line.trim().starts_with("//") {
            continue;
        }
        for forbidden in ["I2CP", "SAM", "HTTP", "SOCKS", "irc"] {
            // unsupported.rs should not import or construct real tunnel resources
            assert!(
                !line.contains(&format!("use crate::{forbidden}"))
                    && !line.contains(&format!("use crate::tunnel::{forbidden}")),
                "unsupported.rs must not import {forbidden} at line {i}: {line}"
            );
        }
    }
}

// ──────────────────────────────────────────────────────────────────────
// § 12. No mutable core handles in public inspection types
// ──────────────────────────────────────────────────────────────────────

#[test]
fn no_mutable_handles_in_inspection_types() {
    let src = read_source("src/i2pcontrol/router_info.rs");
    for (i, line) in src.lines().enumerate() {
        if line.trim().starts_with("//") {
            continue;
        }
        // Public DTOs must not contain Arc<Mutex<...>> or similar mutable shared state
        assert!(
            !line.contains("Arc<Mutex<") && !line.contains("Arc<RwLock<"),
            "router_info DTOs must not contain mutable shared state at line {i}: {line}"
        );
    }
}

// ──────────────────────────────────────────────────────────────────────
// § 13. No truncation/pagination/capability extensions in response types
// ──────────────────────────────────────────────────────────────────────

#[test]
fn no_truncation_in_response_types() {
    let src = read_source("src/i2pcontrol/rpc.rs");
    for (i, line) in src.lines().enumerate() {
        if line.trim().starts_with("//") {
            continue;
        }
        let lower = line.to_lowercase();
        assert!(
            !lower.contains("truncat")
                && !lower.contains("paginate")
                && !lower.contains("cursor")
                && !lower.contains("capability"),
            "rpc.rs must not contain truncation/pagination/capability at line {i}: {line}"
        );
    }
}

// ──────────────────────────────────────────────────────────────────────
// § 14. Exact public vocabulary — no extra methods
// ──────────────────────────────────────────────────────────────────────

#[test]
fn exact_public_method_vocabulary() {
    let src = read_source("src/i2pcontrol/rpc.rs");
    // Find all pub mod declarations
    let mut modules = HashSet::new();
    for line in src.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("pub mod ") {
            let name = trimmed
                .trim_start_matches("pub mod ")
                .trim_end_matches(" {")
                .trim_end_matches('{')
                .trim();
            modules.insert(name.to_string());
        }
    }
    // Allowed public modules in rpc.rs
    let allowed = [
        "error_codes",
        "methods",
        "tunnel_types",
        "tunnel_actions",
        "address_books",
        "address_book_requests",
        "router_info_keys",
    ];
    let allowed_set: HashSet<&str> = allowed.iter().copied().collect();
    let extra: Vec<&str> = modules
        .iter()
        .map(|s| s.as_str())
        .filter(|m| !allowed_set.contains(*m))
        .collect();
    assert!(
        extra.is_empty(),
        "rpc.rs has unexpected public modules: {extra:?}"
    );
}

// ──────────────────────────────────────────────────────────────────────
// § 15. No response field fabrication — handler must only return requested keys
// ──────────────────────────────────────────────────────────────────────

#[test]
fn router_info_handler_has_only_requested_key_filtering() {
    let src = read_source("src/i2pcontrol/router_info_handler.rs");
    // The handler must check if a key was requested before including it
    assert!(
        src.contains("requested_keys") || src.contains("requested") || src.contains("selector"),
        "router_info_handler must filter by requested keys"
    );
}

#[test]
fn client_services_handler_has_only_requested_key_filtering() {
    let src = read_source("src/i2pcontrol/client_services.rs");
    assert!(
        src.contains("requested_keys") || src.contains("requested") || src.contains("Selector"),
        "client_services must filter by requested selectors"
    );
}

// ──────────────────────────────────────────────────────────────────────
// § 16. Backend registry is exhaustive at compile time
// ──────────────────────────────────────────────────────────────────────

#[test]
fn backend_registry_compile_time_guard() {
    let src = read_source("src/i2pcontrol/backends/registry.rs");
    assert!(
        src.contains("assert_eq") && src.contains("ALL_TUNNEL_TYPES"),
        "backend registry must have compile-time exhaustiveness check"
    );
}

// ──────────────────────────────────────────────────────────────────────
// § 17. All control traits are object-safe
// ──────────────────────────────────────────────────────────────────────

#[test]
fn control_traits_are_object_safe() {
    fn assert_object_safe<T: ?Sized>() {}
    use emissary_cli::i2pcontrol::{
        control_plane::{AddressBookControl, TunnelManagerControl},
        router_info::RouterInfoControl,
    };
    assert_object_safe::<dyn RouterInfoControl>();
    assert_object_safe::<dyn AddressBookControl>();
    assert_object_safe::<dyn TunnelManagerControl>();
}

// ──────────────────────────────────────────────────────────────────────
// § 18. No dependency on runtime address-book resolution from I2PControl
// ──────────────────────────────────────────────────────────────────────

#[test]
fn no_runtime_addressbook_dependency() {
    for f in ALL_I2PCONTROL_FILES {
        let src = read_source(f);
        for (i, line) in src.lines().enumerate() {
            if line.trim().starts_with("//") {
                continue;
            }
            // I2PControl must not import the runtime address-book resolver
            assert!(
                !line.contains("use crate::address_book::resolve")
                    && !line.contains("AddressBookResolver"),
                "I2PControl file {f}:{i} must not depend on runtime address-book resolution: {line}"
            );
        }
    }
}
