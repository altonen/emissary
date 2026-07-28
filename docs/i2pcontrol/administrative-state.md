# I2PControl Administrative State

Status: M002 infrastructure implemented

This document describes the Proposal 170 administrative state persistence layer in Emissary.

## Overview

The administrative state stores Proposal 170 control-plane data independently from the existing runtime configuration. It provides restart-safe persistence with versioned generation files, atomic publication, and corruption recovery.

## State root location

All Proposal 170 administrative data is stored under a dedicated state root:

```
<base_path>/i2pcontrol/
    state/
        tunnels/          # Tunnel definitions
        address-books/    # Administrative address books
        address-book-config/  # Address book configuration
        subscriptions/    # Subscription URLs
```

The exact directory names are fixed and documented here. Future schema changes may add new directories but will not rename existing ones.

## What is stored

### Tunnel definitions

Each tunnel definition is stored as a complete `TunnelDefinition` including:

- Validated tunnel name (exact user spelling preserved)
- Exact Proposal 170 tunnel type (one of 12 types)
- Ownership classification (ControlPlane, StartupManaged, Unsupported)
- Internal runtime state (Stopped, Running, etc.)
- Start intent (StartOnLoad or DoNotStart)
- Complete typed tunnel options
- Original raw configuration for lossless `get` behavior

### Administrative address books

Four independent books are stored:

- **Private** - User's private address book
- **Local** - Local addresses
- **Router** - Router-level addresses
- **Published** - Published addresses

Each book is an independent map keyed by hostname. Books are isolated - mutations to one book do not affect others.

### Subscriptions

Address book subscription URLs are stored as an ordered set. Insertion order is preserved. Duplicates are automatically rejected.

### Address book configuration

String-keyed configuration map with deterministic ordering (BTreeMap). Used for address book behavior settings.

## Generation store design

### Versioned envelopes

Each committed state is wrapped in a versioned envelope:

```json
{
  "schema": "emissary-i2pcontrol",
  "version": 1,
  "revision": 42,
  "payload": { ... }
}
```

The schema identifier and version allow future migration. The revision is a monotonically increasing counter per store.

### Atomic publication

State updates follow this sequence:

1. Validate the proposed state
2. Increment the revision
3. Serialize deterministically
4. Write to a temporary file (`.tmp-gen-NNNNNN.json`)
5. Flush and sync the file
6. Set restrictive permissions (0o600 on Unix)
7. Rename to the final generation path (`gen-NNNNNN.json`)
8. Update the in-memory snapshot

The rename is atomic on the same filesystem. If the process crashes between steps 4-6, the temporary file is detected and skipped on next load. If the process crashes between step 7 and 8, the new generation is loaded on next startup.

### Loading and corruption recovery

On load, the store:

1. Scans the directory for `.json` files (bounded to 100 files)
2. Rejects symlinks in the generation directory
3. Sorts by filename (newest first, due to zero-padded revision numbers)
4. Tries each file, newest first
5. Falls back to the previous valid generation on corruption
6. Returns an actionable error if all generations are corrupt

The newest corrupt generation does not prevent loading a prior valid generation. A diagnostic is emitted for each failed generation.

### Retention

The store retains at most 5 prior good generations plus the current one (6 total). Older generations are cleaned up after each successful publication. Cleanup is best-effort and never deletes the active generation.

## What is NOT stored

- **router.toml** remains unchanged. Proposal 170 state does not modify router configuration.
- **Runtime address book** (`<base>/addressbook/`) is not touched. Administrative books are separate.
- **Startup-managed tunnels** are not migrated. Existing client/server/proxy startup definitions continue to work through their existing paths.
- **Frontend state** is not stored. I2PControl runs independently of the UI.
- **No task state** is persisted. StartOnLoad is stored but not executed by the persistence layer.

## Schema versioning

The current schema version is 1. Future schema changes:

- Must read older versions into a validated domain object before writing a new generation
- In-place mutation of generation files is prohibited
- Downgrade behavior: older binaries ignore the separate state root rather than corrupt it

## Module structure

```text
emissary-cli/src/i2pcontrol/
    domain/
        mod.rs            # Module root with re-exports
        tunnel.rs         # TunnelType, TunnelAction, TunnelName, TunnelDefinition, etc.
        address_book.rs   # AdministrativeAddressBookType, AddressBookEntry, etc.
        revision.rs       # StateRevision monotonic counter
    backends/
        mod.rs            # TunnelBackend trait, BackendError, BackendStatus
        unsupported.rs    # UnsupportedTunnelBackend
        fake.rs           # FakeTunnelBackend, FakeBackendRegistry
        registry.rs       # TunnelBackendRegistry (exhaustive), create_default_registry()
    stores/
        mod.rs            # Module root
        generation_store.rs   # Generic GenerationStore<T>, Envelope<T>, StoreError
        tunnel_store.rs       # TunnelStore
        address_book_store.rs # AddressBookStore
        subscription_store.rs # SubscriptionStore
        fakes.rs              # In-memory fake stores for testing
```

## In-memory fakes

For handler tests that don't need persistence, in-memory fake stores are available:

- `TunnelStoreFake` - In-memory tunnel definitions
- `AddressBookStoreFake` - In-memory address books, subscriptions, and configuration
- `SubscriptionStoreFake` - In-memory subscriptions

These implement the same mutation/revision semantics as production stores without touching the filesystem.
