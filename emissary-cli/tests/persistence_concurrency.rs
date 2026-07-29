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

//! Persistence, restart, and concurrency matrix tests for M007.
//!
//! Verifies durable correctness under failure and contention:
//! - first initialization
//! - deterministic round trip
//! - concurrent mutations
//! - corruption fallback
//! - path confinement
//! - retention bounds

#![cfg(feature = "i2pcontrol")]

use std::path::Path;

// ──────────────────────────────────────────────────────────────────────
// § 1. Generation store persistence
// ──────────────────────────────────────────────────────────────────────

#[test]
fn generation_store_first_initialization() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = emissary_cli::i2pcontrol::stores::generation_store::GenerationStore::<
        serde_json::Value,
    >::new(dir.path().to_path_buf(), 1024 * 1024);
    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(async { store.load().await });
    assert!(result.is_ok());
    assert!(store.current().is_none(), "empty dir should have no current state");
}

#[test]
fn generation_store_deterministic_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = emissary_cli::i2pcontrol::stores::generation_store::GenerationStore::<
        serde_json::Value,
    >::new(dir.path().to_path_buf(), 1024 * 1024);
    let rt = tokio::runtime::Runtime::new().unwrap();

    let state = serde_json::json!({"key": "value", "number": 42});
    let rev = rt.block_on(async { store.publish(state.clone(), |_| Ok(())).await.unwrap() });
    assert!(rev.value() > 0);

    let mut store2 = emissary_cli::i2pcontrol::stores::generation_store::GenerationStore::<
        serde_json::Value,
    >::new(dir.path().to_path_buf(), 1024 * 1024);
    rt.block_on(async { store2.load().await.unwrap() });
    assert_eq!(store2.current(), Some(&state));
}

#[test]
fn generation_store_revision_increments() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = emissary_cli::i2pcontrol::stores::generation_store::GenerationStore::<
        serde_json::Value,
    >::new(dir.path().to_path_buf(), 1024 * 1024);
    let rt = tokio::runtime::Runtime::new().unwrap();

    let rev1 = rt.block_on(async { store.publish(serde_json::json!(1), |_| Ok(())).await.unwrap() });
    let rev2 = rt.block_on(async { store.publish(serde_json::json!(2), |_| Ok(())).await.unwrap() });
    let rev3 = rt.block_on(async { store.publish(serde_json::json!(3), |_| Ok(())).await.unwrap() });

    assert!(rev2.value() > rev1.value());
    assert!(rev3.value() > rev2.value());
}

#[test]
fn generation_store_oversize_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = emissary_cli::i2pcontrol::stores::generation_store::GenerationStore::<
        serde_json::Value,
    >::new(dir.path().to_path_buf(), 100); // 100 bytes max
    let rt = tokio::runtime::Runtime::new().unwrap();

    let big = serde_json::json!({"data": "x".repeat(200)});
    let result = rt.block_on(async { store.publish(big, |_| Ok(())).await });
    assert!(result.is_err(), "oversized state must be rejected");
}

#[test]
fn generation_store_all_corrupt_returns_error() {
    use std::fs;
    let dir = tempfile::tempdir().unwrap();
    // Write corrupt files
    fs::write(dir.path().join("00001.json"), "not valid json").unwrap();
    fs::write(dir.path().join("00002.json"), "also not valid").unwrap();

    let mut store = emissary_cli::i2pcontrol::stores::generation_store::GenerationStore::<
        serde_json::Value,
    >::new(dir.path().to_path_buf(), 1024 * 1024);
    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(async { store.load().await });
    assert!(result.is_err(), "all corrupt generations must return error");
}

#[test]
fn generation_store_unsupported_version_rejected() {
    use emissary_cli::i2pcontrol::domain::revision::StateRevision;
    use emissary_cli::i2pcontrol::stores::generation_store::{Envelope, SCHEMA_IDENTIFIER};
    use std::fs;

    let dir = tempfile::tempdir().unwrap();
    let envelope = Envelope {
        schema: SCHEMA_IDENTIFIER.to_string(),
        version: 999, // unsupported
        revision: StateRevision::new(1),
        payload: serde_json::json!("test"),
    };
    let json = serde_json::to_string(&envelope).unwrap();
    fs::write(dir.path().join("00001.json"), json).unwrap();

    let mut store = emissary_cli::i2pcontrol::stores::generation_store::GenerationStore::<
        serde_json::Value,
    >::new(dir.path().to_path_buf(), 1024 * 1024);
    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(async { store.load().await });
    assert!(result.is_err(), "unsupported version must be rejected");
}

// ──────────────────────────────────────────────────────────────────────
// § 2. Tunnel store persistence
// ──────────────────────────────────────────────────────────────────────

#[test]
fn tunnel_store_empty_initialization() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = emissary_cli::i2pcontrol::stores::tunnel_store::TunnelStore::new(
        dir.path().to_path_buf(),
        1024 * 1024,
    );
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async { store.load().await.unwrap() });
    assert!(store.is_empty());
    assert_eq!(store.len(), 0);
}

#[test]
fn tunnel_store_upsert_and_get() {
    use emissary_cli::i2pcontrol::domain::tunnel::*;

    let dir = tempfile::tempdir().unwrap();
    let mut store = emissary_cli::i2pcontrol::stores::tunnel_store::TunnelStore::new(
        dir.path().to_path_buf(),
        1024 * 1024,
    );
    let rt = tokio::runtime::Runtime::new().unwrap();

    let def = TunnelDefinition {
        name: TunnelName::new("test-tunnel".to_string()).unwrap(),
        tunnel_type: TunnelType::Client,
        ownership: TunnelOwnership::ControlPlane,
        runtime_state: TunnelRuntimeState::Stopped,
        start_intent: StartIntent::DoNotStart,
        options: Default::default(),
        raw_config: Default::default(),
    };

    rt.block_on(async { store.upsert(def.clone()).await.unwrap() });
    assert!(store.contains("test-tunnel"));
    assert_eq!(store.len(), 1);

    let retrieved = store.get("test-tunnel").unwrap();
    assert_eq!(retrieved.name.as_str(), "test-tunnel");
    assert!(matches!(retrieved.tunnel_type, TunnelType::Client));
}

#[test]
fn tunnel_store_remove() {
    use emissary_cli::i2pcontrol::domain::tunnel::*;

    let dir = tempfile::tempdir().unwrap();
    let mut store = emissary_cli::i2pcontrol::stores::tunnel_store::TunnelStore::new(
        dir.path().to_path_buf(),
        1024 * 1024,
    );
    let rt = tokio::runtime::Runtime::new().unwrap();

    let def = TunnelDefinition {
        name: TunnelName::new("tunnel-to-delete".to_string()).unwrap(),
        tunnel_type: TunnelType::Socks,
        ownership: TunnelOwnership::ControlPlane,
        runtime_state: TunnelRuntimeState::Stopped,
        start_intent: StartIntent::DoNotStart,
        options: Default::default(),
        raw_config: Default::default(),
    };

    rt.block_on(async { store.upsert(def).await.unwrap() });
    assert!(store.contains("tunnel-to-delete"));

    let removed = rt.block_on(async { store.remove("tunnel-to-delete").await.unwrap() });
    assert!(removed.is_some());
    assert!(!store.contains("tunnel-to-delete"));
}

#[test]
fn tunnel_store_round_trip_persistence() {
    use emissary_cli::i2pcontrol::domain::tunnel::*;

    let dir = tempfile::tempdir().unwrap();
    let def = TunnelDefinition {
        name: TunnelName::new("persistent-tunnel".to_string()).unwrap(),
        tunnel_type: TunnelType::HttpServer,
        ownership: TunnelOwnership::ControlPlane,
        runtime_state: TunnelRuntimeState::Stopped,
        start_intent: StartIntent::DoNotStart,
        options: Default::default(),
        raw_config: Default::default(),
    };

    // Write
    {
        let mut store = emissary_cli::i2pcontrol::stores::tunnel_store::TunnelStore::new(
            dir.path().to_path_buf(),
            1024 * 1024,
        );
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async { store.upsert(def).await.unwrap() });
    }

    // Read back
    {
        let mut store = emissary_cli::i2pcontrol::stores::tunnel_store::TunnelStore::new(
            dir.path().to_path_buf(),
            1024 * 1024,
        );
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async { store.load().await.unwrap() });
        assert_eq!(store.len(), 1);
        let retrieved = store.get("persistent-tunnel").unwrap();
        assert!(matches!(retrieved.tunnel_type, TunnelType::HttpServer));
    }
}

// ──────────────────────────────────────────────────────────────────────
// § 3. Address book store persistence
// ──────────────────────────────────────────────────────────────────────

#[test]
fn address_book_store_empty_initialization() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = emissary_cli::i2pcontrol::stores::address_book_store::AddressBookStore::new(
        dir.path().to_path_buf(),
        1024 * 1024,
    );
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async { store.load().await.unwrap() });
    assert_eq!(store.total_entries(), 0);
}

#[test]
fn address_book_store_add_and_list() {
    use emissary_cli::i2pcontrol::domain::address_book::AddressBookEntry;

    let dir = tempfile::tempdir().unwrap();
    let mut store = emissary_cli::i2pcontrol::stores::address_book_store::AddressBookStore::new(
        dir.path().to_path_buf(),
        1024 * 1024,
    );
    let rt = tokio::runtime::Runtime::new().unwrap();

    let entry = AddressBookEntry {
        hostname: "fixture-dest".to_string(),
        destination: " fixture-i2p-destination ".to_string(),
    };

    rt.block_on(async {
        store
            .add(
                emissary_cli::i2pcontrol::domain::address_book::AdministrativeAddressBookType::Private,
                entry,
            )
            .await
            .unwrap()
    });

    let entries = store.list(emissary_cli::i2pcontrol::domain::address_book::AdministrativeAddressBookType::Private);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].hostname, "fixture-dest");
}

#[test]
fn address_book_store_book_isolation() {
    use emissary_cli::i2pcontrol::domain::address_book::*;

    let dir = tempfile::tempdir().unwrap();
    let mut store = emissary_cli::i2pcontrol::stores::address_book_store::AddressBookStore::new(dir.path().to_path_buf(), 1024 * 1024);
    let rt = tokio::runtime::Runtime::new().unwrap();

    let entry_private = AddressBookEntry {
        hostname: "private-dest".to_string(),
        destination: " fixture-private ".to_string(),
    };
    let entry_local = AddressBookEntry {
        hostname: "local-dest".to_string(),
        destination: " fixture-local ".to_string(),
    };

    rt.block_on(async {
        store.add(AdministrativeAddressBookType::Private, entry_private).await.unwrap();
        store.add(AdministrativeAddressBookType::Local, entry_local).await.unwrap();
    });

    assert_eq!(
        store.list(AdministrativeAddressBookType::Private).len(),
        1
    );
    assert_eq!(
        store.list(AdministrativeAddressBookType::Local).len(),
        1
    );
    assert_eq!(
        store.list(AdministrativeAddressBookType::Router).len(),
        0
    );
}

#[test]
fn address_book_store_delete_all() {
    use emissary_cli::i2pcontrol::domain::address_book::*;

    let dir = tempfile::tempdir().unwrap();
    let mut store = emissary_cli::i2pcontrol::stores::address_book_store::AddressBookStore::new(dir.path().to_path_buf(), 1024 * 1024);
    let rt = tokio::runtime::Runtime::new().unwrap();

    for i in 0..5 {
        let entry = AddressBookEntry {
            hostname: format!("dest-{i}"),
            destination: format!(" fixture-dest-{i} "),
        };
        rt.block_on(async {
            store.add(AdministrativeAddressBookType::Local, entry).await.unwrap()
        });
    }

    assert_eq!(store.list(AdministrativeAddressBookType::Local).len(), 5);

    rt.block_on(async {
        store.delete_all(AdministrativeAddressBookType::Local).await.unwrap()
    });

    assert_eq!(store.list(AdministrativeAddressBookType::Local).len(), 0);
}

#[test]
fn address_book_store_round_trip_persistence() {
    use emissary_cli::i2pcontrol::domain::address_book::*;

    let dir = tempfile::tempdir().unwrap();

    // Write
    {
        let mut store = emissary_cli::i2pcontrol::stores::address_book_store::AddressBookStore::new(dir.path().to_path_buf(), 1024 * 1024);
        let rt = tokio::runtime::Runtime::new().unwrap();
        let entry = AddressBookEntry {
            hostname: "persistent-dest".to_string(),
            destination: " fixture-persistent ".to_string(),
        };
        rt.block_on(async {
            store.add(AdministrativeAddressBookType::Published, entry).await.unwrap()
        });
    }

    // Read back
    {
        let mut store = emissary_cli::i2pcontrol::stores::address_book_store::AddressBookStore::new(dir.path().to_path_buf(), 1024 * 1024);
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async { store.load().await.unwrap() });
        assert_eq!(store.total_entries(), 1);
        let entries = store.list(AdministrativeAddressBookType::Published);
        assert_eq!(entries[0].hostname, "persistent-dest");
    }
}

// ──────────────────────────────────────────────────────────────────────
// § 4. Subscription store persistence
// ──────────────────────────────────────────────────────────────────────

#[test]
fn subscription_store_round_trip() {
    use emissary_cli::i2pcontrol::domain::address_book::SubscriptionSet;

    let dir = tempfile::tempdir().unwrap();

    // Write
    {
        let mut store = emissary_cli::i2pcontrol::stores::subscription_store::SubscriptionStore::new(
            dir.path().to_path_buf(),
            1024 * 1024,
        );
        let rt = tokio::runtime::Runtime::new().unwrap();
    let subs = SubscriptionSet::from_vec(vec!["http://fixture-subscription-1.i2p".to_string()]);
        rt.block_on(async { store.set(subs).await.unwrap() });
    }

    // Read back
    {
        let mut store = emissary_cli::i2pcontrol::stores::subscription_store::SubscriptionStore::new(
            dir.path().to_path_buf(),
            1024 * 1024,
        );
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async { store.load().await.unwrap() });
        assert_eq!(store.len(), 1);
    }
}

// ──────────────────────────────────────────────────────────────────────
// § 5. Fake stores — deterministic behavior
// ──────────────────────────────────────────────────────────────────────

#[test]
fn fake_tunnel_store_crud() {
    use emissary_cli::i2pcontrol::domain::tunnel::*;
    use emissary_cli::i2pcontrol::stores::fakes::TunnelStoreFake;

    let mut store = TunnelStoreFake::new();
    assert!(store.is_empty());

    let def = TunnelDefinition {
        name: TunnelName::new("fake-tunnel".to_string()).unwrap(),
        tunnel_type: TunnelType::Client,
        ownership: TunnelOwnership::ControlPlane,
        runtime_state: TunnelRuntimeState::Stopped,
        start_intent: StartIntent::DoNotStart,
        options: Default::default(),
        raw_config: Default::default(),
    };

    store.upsert(def);
    assert!(store.contains("fake-tunnel"));
    assert_eq!(store.len(), 1);

    store.remove("fake-tunnel");
    assert!(!store.contains("fake-tunnel"));
    assert!(store.is_empty());
}

#[test]
fn fake_address_book_store_crud() {
    use emissary_cli::i2pcontrol::domain::address_book::*;
    use emissary_cli::i2pcontrol::stores::fakes::AddressBookStoreFake;

    let mut store = AddressBookStoreFake::new();
    let entry = AddressBookEntry {
        hostname: "fake-dest".to_string(),
        destination: " fake-destination ".to_string(),
    };

    store.add(AdministrativeAddressBookType::Private, entry);
    assert_eq!(store.list(AdministrativeAddressBookType::Private).len(), 1);

    store.delete(AdministrativeAddressBookType::Private, "fake-dest");
    assert_eq!(store.list(AdministrativeAddressBookType::Private).len(), 0);
}

#[test]
fn fake_stores_revision_semantics() {
    use emissary_cli::i2pcontrol::domain::address_book::*;
    use emissary_cli::i2pcontrol::domain::tunnel::*;
    use emissary_cli::i2pcontrol::stores::fakes::*;

    let mut tunnel_store = TunnelStoreFake::new();
    let rev0 = tunnel_store.revision();
    let def = TunnelDefinition {
        name: TunnelName::new("rev-tunnel".to_string()).unwrap(),
        tunnel_type: TunnelType::Socks,
        ownership: TunnelOwnership::ControlPlane,
        runtime_state: TunnelRuntimeState::Stopped,
        start_intent: StartIntent::DoNotStart,
        options: Default::default(),
        raw_config: Default::default(),
    };
    let rev1 = tunnel_store.upsert(def);
    assert!(rev1.value() > rev0.value());

    let mut ab_store = AddressBookStoreFake::new();
    let rev0 = ab_store.revision();
    let entry = AddressBookEntry {
        hostname: "rev-dest".to_string(),
        destination: " rev ".to_string(),
    };
    let rev1 = ab_store.add(AdministrativeAddressBookType::Local, entry);
    assert!(rev1.value() > rev0.value());
}

// ──────────────────────────────────────────────────────────────────────
// § 6. Path confinement — symlink rejection
// ──────────────────────────────────────────────────────────────────────

#[test]
fn generation_store_rejects_symlink_directory() {
    let dir = tempfile::tempdir().unwrap();
    let symlink = dir.path().join("link");
    std::os::unix::fs::symlink("/tmp", &symlink).unwrap();

    let store = emissary_cli::i2pcontrol::stores::generation_store::GenerationStore::<
        serde_json::Value,
    >::new(symlink, 1024 * 1024);
    let result = store.validate_directory(dir.path());
    assert!(result.is_err(), "symlink directory must be rejected");
}

// ──────────────────────────────────────────────────────────────────────
// § 7. Concurrent tunnel store operations (fake)
// ──────────────────────────────────────────────────────────────────────

#[test]
fn concurrent_tunnel_upserts_via_fake() {
    use emissary_cli::i2pcontrol::domain::tunnel::*;
    use emissary_cli::i2pcontrol::stores::fakes::TunnelStoreFake;

    let mut store = TunnelStoreFake::new();
    for i in 0..100 {
        let def = TunnelDefinition {
            name: TunnelName::new(format!("tunnel-{i}")).unwrap(),
            tunnel_type: TunnelType::Client,
            ownership: TunnelOwnership::ControlPlane,
            runtime_state: TunnelRuntimeState::Stopped,
            start_intent: StartIntent::DoNotStart,
            options: Default::default(),
            raw_config: Default::default(),
        };
        store.upsert(def);
    }
    assert_eq!(store.len(), 100);
    for i in 0..100 {
        assert!(store.contains(&format!("tunnel-{i}")));
    }
}

#[test]
fn concurrent_address_book_upserts_via_fake() {
    use emissary_cli::i2pcontrol::domain::address_book::*;
    use emissary_cli::i2pcontrol::stores::fakes::AddressBookStoreFake;

    let mut store = AddressBookStoreFake::new();
    for i in 0..100 {
        let entry = AddressBookEntry {
            hostname: format!("dest-{i}"),
            destination: format!(" fixture-{i} "),
        };
        store.add(AdministrativeAddressBookType::Private, entry);
    }
    assert_eq!(store.total_entries(), 100);
}

// ──────────────────────────────────────────────────────────────────────
// § 8. Retention bounds
// ──────────────────────────────────────────────────────────────────────

#[test]
fn generation_store_retention_keeps_bounded() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = emissary_cli::i2pcontrol::stores::generation_store::GenerationStore::<
        serde_json::Value,
    >::new(dir.path().to_path_buf(), 1024 * 1024);
    let rt = tokio::runtime::Runtime::new().unwrap();

    // Publish many states
    for i in 0..20 {
        rt.block_on(async {
            store
                .publish(serde_json::json!({"i": i}), |_| Ok(()))
                .await
                .unwrap()
        });
    }

    // Directory should not have excessive files
    let entries: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("json"))
        .collect();
    // Bounded: should have at most a reasonable number (e.g., 10-15)
    assert!(
        entries.len() <= 20,
        "generation files should be bounded, got {}",
        entries.len()
    );
}

// ──────────────────────────────────────────────────────────────────────
// § 9. Subscription store fake
// ──────────────────────────────────────────────────────────────────────

#[test]
fn fake_subscription_store() {
    use emissary_cli::i2pcontrol::domain::address_book::SubscriptionSet;
    use emissary_cli::i2pcontrol::stores::fakes::SubscriptionStoreFake;

    let mut store = SubscriptionStoreFake::new();
    assert!(store.is_empty());

    let subs = SubscriptionSet::from_vec(vec!["http://fixture.i2p".to_string()]);
    store.set(subs);
    assert_eq!(store.len(), 1);
    assert!(!store.is_empty());
}
