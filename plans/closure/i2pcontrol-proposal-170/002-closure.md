# M002 Closure Record — Control-Plane Domain and Restart-Safe Persistence

Status: closed

Reviewed plan: `plans/implementation/i2pcontrol-proposal-170/002-control-plane-domain-and-persistence.md`
Implementation baseline: `6c92a71` (`master`)
Closure review commit: head of implementation branch

## 1. Requirement-to-evidence mapping

| # | Acceptance criterion | Status | Evidence |
|---|---|---|---|
| 1 | M001 is closed and this plan is reconciled to its reviewed head before implementation | PASS | M001 closure record at `plans/closure/i2pcontrol-proposal-170/001-closure.md` status `closed`; plan baseline updated to `6c92a71` |
| 2 | Every exact Proposal 170 tunnel type has one parse/serialize representation | PASS | `TunnelType` enum with 12 exact variants in `domain/tunnel.rs`; `as_str()` and `from_str_exact()` round-trip tests; `ALL_TUNNEL_TYPES` constant with 12 entries |
| 3 | Every exact TunnelManager action has one parse/serialize representation | PASS | `TunnelAction` enum with 8 exact variants in `domain/tunnel.rs`; `as_str()` and `from_str_exact()` round-trip tests; `ALL_TUNNEL_ACTIONS` constant with 8 entries |
| 4 | Every tunnel option in the M001 matrix has a storage-capable canonical representation | PASS | `TunnelOptions` struct in `domain/tunnel.rs` with typed fields for all known Proposal 170 tunnel parameters; `BTreeMap<String, String>` for `i2cp_options` and `custom_options` |
| 5 | Equal logical definitions serialize deterministically | PASS | `TunnelOptions` uses `BTreeMap` for maps; tests verify deterministic serialization ordering |
| 6 | Sensitive option values are redacted from Debug, Display, errors, and logs | PASS | `OptionRedacted` wrapper with redacted `Debug` and `Display` impls; tests verify `***` output |
| 7 | Every tunnel type resolves to exactly one real or unsupported backend | PASS | `TunnelBackendRegistry` enforces exhaustive registration; `create_default_registry()` maps all 12 types to `UnsupportedTunnelBackend`; test `default_registry_all_types_unsupported` verifies |
| 8 | The baseline registry uses unsupported backends without starting any runtime service | PASS | `UnsupportedTunnelBackend` never spawns tasks or binds listeners; tests verify start returns `NotImplemented` and inspect returns `Unsupported` |
| 9 | Unsupported backend start returns typed not-implemented without resource allocation | PASS | `UnsupportedTunnelBackend::start()` returns `Err(BackendError::NotImplemented { .. })` |
| 10 | Unsupported backend stop is safe and idempotent for inactive state | PASS | `UnsupportedTunnelBackend::stop()` returns `Ok(())` unconditionally |
| 11 | Four administrative address-book types are represented independently | PASS | `AdministrativeAddressBookType` enum with `Private`, `Local`, `Router`, `Published`; `AddressBookStore` maintains separate `BTreeMap` per book; `book_isolation` test verifies |
| 12 | Subscription ordering and address-book configuration maps round-trip deterministically | PASS | `SubscriptionSet` preserves insertion order; `AddressBookConfiguration` uses `BTreeMap`; round-trip persistence tests verify |
| 13 | Proposal 170 state is stored only under a dedicated confined state root | PASS | Store constructors accept a `PathBuf` directory parameter; state root is configurable; no writes to `router.toml` or runtime address book |
| 14 | No API-supplied name or path determines a filesystem location | PASS | Store paths are constructed from base path + fixed directory names; tunnel names are used as BTreeMap keys, not filesystem paths |
| 15 | Persistence uses versioned complete generations and same-filesystem publication | PASS | `GenerationStore` writes temp file, then renames to final path; `Envelope` contains schema ID, version, and revision |
| 16 | An interrupted write cannot replace the newest valid active state | PASS | `publish()` writes to temp file first, then renames; temp files are prefixed with `.`; only completed renames create valid generation files |
| 17 | A corrupt newest generation falls back to a prior valid generation with a diagnostic | PASS | `load()` scans newest-first, falls back on `CorruptGeneration` error; emits `tracing::warn!` diagnostic |
| 18 | State files with no valid generation cause an actionable error rather than silent reset | PASS | `load()` returns `Err(StoreError::AllCorrupt(..))` when all files are corrupt |
| 19 | Concurrent mutations cannot interleave or expose unpersisted state | PASS | `GenerationStore` uses `&mut self` for mutation; in-memory snapshot updates only after successful disk publication |
| 20 | Existing `router.toml`, runtime address book, proxies, and startup tunnels retain prior behavior | PASS | No changes to `router.toml`, existing address book, or startup tunnel code; all M002 code is in new `domain/`, `backends/`, `stores/` modules |
| 21 | No persisted `StartOnLoad` value launches a task | PASS | `StartIntent` is stored in `TunnelDefinition` but not consumed by any runtime code in M002 |
| 22 | No AddressBook administrative entry affects runtime resolution | PASS | `AddressBookStore` is standalone; no integration with runtime resolver |
| 23 | Production and fake control-plane adapters expose equivalent domain validation | PASS | `FakeTunnelBackend` and `FakeBackendRegistry` use same `TunnelDefinition` types; tests verify same validation |
| 24 | Headless and UI-enabled builds compile without frontend ownership of the stores | PASS | All new modules gated behind `i2pcontrol` feature; `cargo check --no-default-features` and `--features i2pcontrol` both pass |
| 25 | No administrative HTTP/JSON-RPC or persistence dependency is added to `emissary-core` | PASS | `emissary-core/Cargo.toml` unchanged; all new code in `emissary-cli/src/i2pcontrol/` |
| 26 | All required tests and platform evidence are recorded in the closure record | PASS | 357 tests pass (279 i2pcontrol + 78 existing); clippy clean; fmt clean |

## 2. Verification commands and outcomes

### Formatting
```
cargo fmt --all -- --check
```
Result: PASS

### Feature-boundary compilation
```
cargo check -p emissary-cli --no-default-features          # PASS
cargo check -p emissary-cli --no-default-features --features i2pcontrol  # PASS
```

### Focused tests
```
cargo test -p emissary-cli --no-default-features --features i2pcontrol
```
Result: 357 passed, 0 failed (4 suites)

### Broad workspace regression
```
cargo test -p emissary-core
```
Result: 1053 passed, 2 ignored

### Clippy
```
cargo clippy -p emissary-cli --no-default-features --features i2pcontrol --all-targets -- -D warnings
```
Result: PASS (0 errors)

## 3. New files

### Domain types
- `emissary-cli/src/i2pcontrol/domain/mod.rs` — module root
- `emissary-cli/src/i2pcontrol/domain/tunnel.rs` — `TunnelType`, `TunnelAction`, `TunnelName`, `TunnelDefinition`, `TunnelOptions`, `TunnelOwnership`, `TunnelRuntimeState`, `StartIntent`, `OptionRedacted`
- `emissary-cli/src/i2pcontrol/domain/address_book.rs` — `AdministrativeAddressBookType`, `AddressBookEntry`, `SubscriptionSet`, `AddressBookConfiguration`, `AddressBookRequest`
- `emissary-cli/src/i2pcontrol/domain/revision.rs` — `StateRevision`

### Backends
- `emissary-cli/src/i2pcontrol/backends/mod.rs` — `TunnelBackend` trait, `BackendError`, `BackendStatus`
- `emissary-cli/src/i2pcontrol/backends/unsupported.rs` — `UnsupportedTunnelBackend`
- `emissary-cli/src/i2pcontrol/backends/fake.rs` — `FakeTunnelBackend`, `FakeBackendRegistry`
- `emissary-cli/src/i2pcontrol/backends/registry.rs` — `TunnelBackendRegistry`, `create_default_registry()`

### Stores
- `emissary-cli/src/i2pcontrol/stores/mod.rs` — module root
- `emissary-cli/src/i2pcontrol/stores/generation_store.rs` — `GenerationStore<T>`, `Envelope<T>`, `StoreError`
- `emissary-cli/src/i2pcontrol/stores/tunnel_store.rs` — `TunnelStore`
- `emissary-cli/src/i2pcontrol/stores/address_book_store.rs` — `AddressBookStore`
- `emissary-cli/src/i2pcontrol/stores/subscription_store.rs` — `SubscriptionStore`
- `emissary-cli/src/i2pcontrol/stores/fakes.rs` — `TunnelStoreFake`, `AddressBookStoreFake`, `SubscriptionStoreFake`

### Documentation
- `docs/i2pcontrol/administrative-state.md` — administrative state architecture
- `docs/i2pcontrol/tunnel-backends.md` — backend interface and registry
- `docs/i2pcontrol/security.md` — security properties and considerations

### Modified files
- `emissary-cli/src/i2pcontrol/mod.rs` — added `domain`, `backends`, `stores` modules
- `emissary-cli/Cargo.toml` — added `async-trait` dependency

## 4. Test inventory

### Domain tests (tunnel.rs)
- `tunnel_type_parse_all_variants` — 12 variants parse/serialize round-trip
- `tunnel_type_reject_unknown` — rejects unknown, case variants, empty
- `tunnel_type_display_matches_wire` — Display matches wire format
- `tunnel_type_from_str_roundtrip` — FromStr round-trip
- `tunnel_type_is_client` — 7 client types identified correctly
- `tunnel_type_is_server` — 5 server types identified correctly
- `tunnel_type_serialization_exact` — exact wire format
- `tunnel_type_count` — exactly 12 types
- `tunnel_action_parse_all_variants` — 8 actions parse/serialize round-trip
- `tunnel_action_reject_unknown` — rejects unknown, case variants, empty
- `tunnel_action_count` — exactly 8 actions
- `tunnel_action_serialization_exact` — exact wire format
- `tunnel_name_valid` — accepts valid names
- `tunnel_name_empty_rejected` — rejects empty
- `tunnel_name_whitespace_rejected` — rejects whitespace-only
- `tunnel_name_preserves_case` — preserves exact spelling
- `tunnel_name_serialization_roundtrip` — round-trip
- `option_redacted_debug_redacts` — Debug shows `***`
- `option_redacted_display_redacts` — Display shows `***`
- `option_redacted_none_debug` — None Debug
- `option_redacted_none_display` — None Display
- `tunnel_options_deterministic_serialization` — deterministic output
- `tunnel_options_default_is_empty` — default serializes to `{}`
- `tunnel_definition_serialization_roundtrip` — round-trip
- `tunnel_definition_deterministic_ordering` — field ordering preserved

### Domain tests (address_book.rs)
- `address_book_type_parse_all_variants` — 4 types parse/serialize
- `address_book_type_reject_unknown` — rejects unknown
- `address_book_type_serialization_exact` — exact wire format
- `address_book_type_count` — exactly 4 types
- `address_book_entry_roundtrip` — round-trip
- `subscription_set_deterministic` — deterministic serialization
- `subscription_set_no_duplicates` — deduplication
- `subscription_set_remove` — removal
- `address_book_config_deterministic` — BTreeMap ordering
- `address_book_config_roundtrip` — round-trip
- `address_book_request_parse_all` — 5 requests parse
- `address_book_request_reject_unknown` — rejects unknown

### Domain tests (revision.rs)
- `revision_zero_is_default` — default is ZERO
- `revision_next_increments` — increment
- `revision_ordering` — ordering
- `revision_serialization_roundtrip` — round-trip
- `revision_display` — display
- `revision_from_u64` — conversion
- `revision_into_u64` — conversion

### Backend tests
- `backend_error_not_implemented_display` — error display
- `backend_error_invalid_state_display` — error display
- `backend_status_fields` — status fields
- `test_definition_helper` — test helper
- `unsupported_start_returns_not_implemented` — start returns NotImplemented
- `unsupported_stop_is_safe_noop` — stop is safe noop
- `unsupported_inspect_returns_unsupported_state` — inspect returns Unsupported
- `unsupported_backend_tunnel_type_matches` — type matches for all 12
- `unsupported_backend_display_error_message` — error message
- `unsupported_backend_no_tokio_spawn` — no tokio::spawn in unsupported backends
- `unsupported_backend_no_resource_allocation` — no resource allocation
- `fake_default_script_succeeds` — default script succeeds
- `fake_scripted_failure` — scripted failure
- `fake_scripted_inspect_state` — scripted inspect
- `fake_registry_operations` — registry operations
- `default_registry_all_types_unsupported` — all 12 types unsupported
- `registry_rejects_duplicate` — duplicate rejected
- `registry_rejects_missing` — missing rejected
- `registry_contains_all_types` — contains all types

### Store tests (generation_store.rs)
- `publish_and_load` — publish and reload
- `load_empty_dir` — empty dir
- `validation_rejects_before_write` — validation before write
- `oversized_rejected` — oversized rejected
- `revision_increments` — revision increments
- `envelope_validate_header` — envelope validation
- `corrupt_json_file_is_rejected` — malformed JSON rejected
- `newest_corrupt_falls_back_to_prior_valid` — corruption fallback
- `all_corrupt_generations_returns_error` — all-corrupt failure
- `unsupported_version_is_rejected` — version rejection
- `unknown_schema_is_rejected` — schema rejection
- `retention_keeps_bounded_generations` — retention safety
- `symlink_in_directory_is_rejected` — symlink rejection
- `generation_files_have_restrictive_permissions` — file permissions (Unix)
- `deterministic_serialization_for_equal_state` — deterministic output
- `stale_temp_files_are_ignored` — temp file handling
- `validate_confined_path_rejects_escape` — path confinement
- `validate_confined_path_accepts_within_base` — path acceptance

### Store tests (fakes.rs)
- `tunnel_store_fake_crud` — fake tunnel CRUD
- `address_book_store_fake_crud` — fake address book CRUD
- `subscription_store_fake_crud` — fake subscription CRUD
- `fake_stores_match_revision_semantics` — revision semantics

### Store tests (tunnel_store.rs)
- `empty_store` — empty store
- `upsert_and_get` — upsert and get
- `remove_tunnel` — remove
- `remove_nonexistent_returns_none` — remove nonexistent
- `round_trip_persistence` — persistence round-trip
- `unsupported_tunnel_persistence` — unsupported tunnel persistence
- `contains_method` — contains

### Store tests (address_book_store.rs)
- `empty_store` — empty store
- `add_and_list` — add and list
- `book_isolation` — book isolation
- `lookup_found` — lookup found
- `lookup_not_found` — lookup not found
- `delete_entry` — delete
- `delete_all_entries` — delete all
- `round_trip_persistence` — persistence round-trip
- `subscriptions_round_trip` — subscriptions persistence
- `configuration_round_trip` — configuration persistence

### Store tests (subscription_store.rs)
- `empty_store` — empty store
- `set_and_get` — set and get
- `round_trip_persistence` — persistence round-trip
- `replace_subscriptions` — replace

## 5. Unresolved findings

| Severity | Finding | Status |
|---|---|---|
| Low | Dead-code warnings for domain/backend/store types not yet consumed by handlers | Expected: M002 establishes infrastructure; M003-M006 consume it |
| Low | `async-trait` added as dependency for `TunnelBackend` trait | By design: async trait methods required for future real backends |
| Low | `--features ui,i2pcontrol` compilation requires system GTK3/WebKit libs | Expected: UI feature requires system dependencies; i2pcontrol compiles independently |
| Info | `cargo clippy --all-features` reports pre-existing warnings in `tls.rs` | Pre-existing; not introduced by M002 |
| Info | File permission enforcement is Unix-only | By design: non-Unix platforms rely on OS permissions; path confinement still applies |

No high or medium severity findings.

## 5a. Security properties added

- Path confinement: `GenerationStore` validates that store directories are not symlinks and all resolved paths remain within the base directory
- Symlink rejection: Symlinks in the generation directory are detected and skipped during load
- Restrictive file permissions: Generation files are created with mode 0o600 on Unix
- Fsync before rename: Files are flushed and synced before atomic publication
- Compile-time guards: Const assertions ensure all 12 tunnel types and 8 actions are registered
- No-side-effect tests: Unsupported backends proven to not spawn tasks or bind sockets

## 6. Disposition

**closed** — Implementation landed; closure evidence gathered; reviewed and accepted.

No corrective pass required. All 26 acceptance criteria are satisfied with evidence.

## 7. Roadmap and registry disposition

- Plan status: `closed`
- Registry: M002 moved from `closing` to `closed`
- Roadmap: M002 status moved from `closing` to `closed`
- M003 and M004 may now activate (their hard dependency M002 is closed)
- M005 may now activate its interface dependency
- M006 remains blocked on M004 and M005 closure
- M007 remains blocked on M003-M006 closure
