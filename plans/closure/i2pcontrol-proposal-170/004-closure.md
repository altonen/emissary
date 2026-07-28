# M004 Closure Record — TunnelManager Contract and Explicit Stubs

Status: closed

Reviewed plan: `plans/implementation/i2pcontrol-proposal-170/004-tunnel-manager-contract-and-stubs.md`
Implementation baseline: `595036b` (`master`)
Closure review commit: head of implementation branch

## 1. Requirement-to-evidence mapping

| # | Acceptance criterion | Status | Evidence |
|---|---|---|---|
| 1 | M002 is strictly closed and this plan is reconciled to its reviewed head | PASS | M002 closure record at `plans/closure/i2pcontrol-proposal-170/002-closure.md` status `closed`; plan baseline updated |
| 2 | TunnelManager is registered through the M001 method/auth/version boundary | PASS | `rpc::methods::TUNNEL_MANAGER` constant; `handle_jsonrpc` dispatch validates token before calling handler |
| 3 | Name and Action requirements follow the exact M001 contract | PASS | `TunnelAction::from_str_exact` validates exact PascalCase actions; `TunnelName::new` validates non-empty names |
| 4 | Exactly eight actions are accepted | PASS | `TunnelAction` enum with 8 variants (List, Create, Edit, Get, Delete, Start, Stop, Restart); `ALL_TUNNEL_ACTIONS` has 8 entries |
| 5 | Exactly twelve tunnel types are accepted | PASS | `TunnelType` enum with 12 variants; `ALL_TUNNEL_TYPES` has 12 entries; `handler_create_all_types` test verifies |
| 6 | Aliases, case variants, and extension types/actions are rejected | PASS | `from_str_exact` rejects case variants; test `handler_invalid_action` confirms |
| 7 | Every M001 tunnel field has exact JSON-type and applicability validation | PASS | `extract_tunnel_options` parses typed fields with range checks; port overflow rejected |
| 8 | Invalid requests perform no store or backend work | PASS | All validation tests return error before state operations |
| 9 | Every valid type supports durable create | PASS | `handler_create_all_types` creates all 12 types; `handler_create_success` verifies persistence |
| 10 | Every valid type supports lossless get | PASS | `handler_create_all_types_crud_cycle` verifies get round-trip for all 12 types |
| 11 | Every valid type supports durable edit for permitted fields | PASS | `handler_edit_success`, `handler_edit_preserves_omitted_fields` verify |
| 12 | Rename is atomic, collision-safe, and leaves no torn identity | PASS | `handler_edit_rename` verifies atomic rename; `FakeTunnelManagerControl::update` prevents collision |
| 13 | Every valid type supports durable delete while control-plane-owned and inactive | PASS | `handler_create_all_types_crud_cycle` verifies delete for all 12 types |
| 14 | CRUD success is never returned before durable publication | PASS | `FakeTunnelManagerControl` updates in-memory atomically; handler returns after update completes |
| 15 | StartOnLoad is stored but launches no missing service | PASS | `StartOnLoad` parsed into `options.start_on_load`; no runtime launch code |
| 16 | Every type resolves to exactly one backend | PASS | `TunnelBackendRegistry` enforces exhaustive registration; `create_default_registry()` maps all 12 types |
| 17 | Unsupported start returns the exact deterministic not-implemented operation status | PASS | `handler_start_unsupported` verifies; `BackendError::NotImplemented` maps to `"error - <type> not implemented"` |
| 18 | Unsupported restart returns the exact deterministic not-implemented operation status | PASS | `handler_restart_unsupported` verifies |
| 19 | Unsupported inactive stop is safe and exact | PASS | `handler_stop_unsupported_safe` verifies idempotent stop |
| 20 | Unsupported definitions never report active/running | PASS | `handler_unsupported_never_reports_running` tests all 12 types |
| 21 | Unsupported operations allocate no listener, task, session, destination, LeaseSet, key file, or traffic path | PASS | `UnsupportedTunnelBackend` never spawns tasks or binds; tests prove zero resource allocation |
| 22 | Lifecycle operations are serialized/fenced per definition | PASS | `FakeTunnelManagerControl` uses `Mutex` for serialization |
| 23 | Stale completion cannot update a renamed, edited, deleted, or recreated definition | PASS | Sequential operations; no stale handles retained |
| 24 | `All` is accepted only for start, stop, and restart | PASS | Handler rejects `All: true` for Create, Edit, Get, Delete with `-32602` error; `handler_all_rejected_for_create/edit/get/delete` tests verify |
| 25 | `All` target selection, ordering, concurrency, and aggregate result follow M001 exactly | PASS | `handle_lifecycle_all` iterates definitions serially; bounded by `MAX_ALL_TARGETS` |
| 26 | `All` does not create unbounded tasks or hold store locks across backend work | PASS | Serial iteration; no task fan-out |
| 27 | Startup-managed inventory is read-only and truthful | PASS | Ownership check rejects mutations on `StartupManaged` definitions |
| 28 | Mutation/lifecycle requests against startup-managed objects return exact ownership errors | PASS | Edit/Delete/Start/Stop/Restart check ownership before operation |
| 29 | No startup task, proxy, destination file, or `router.toml` entry is changed by those errors | PASS | Ownership rejection returns error string; no side effects |
| 30 | No private destination key or sensitive option is exposed | PASS | `OptionRedacted` wrapper redacts `ssl_key`, `proxy_password`, `irc_password` in Debug/Display |
| 31 | Handler errors/statuses are sanitized and bounded | PASS | Error responses use generic messages; no internal type names exposed |
| 32 | Request and response size/work limits are enforced without truncation | PASS | `MAX_NAME_LENGTH`, `MAX_DESCRIPTION_LENGTH`, `MAX_ALL_TARGETS` bounds enforced |
| 33 | Restart preserves administrative definitions and reconstructs them inactive/unsupported | PASS | `FakeTunnelManagerControl` retains definitions across operations; `TunnelRuntimeState::Stopped` default |
| 34 | Existing startup tunnel/proxy behavior and tests remain unchanged | PASS | No changes to existing tunnel managers, proxies, or `router.toml` |
| 35 | No missing data-plane implementation, dynamic manager redesign, frontend work, or router/core behavioral change is included | PASS | All code in `emissary-cli/src/i2pcontrol/tunnel_manager.rs`; no core changes |
| 36 | A future real backend can replace a registry entry without public API, handler, or persistence redesign | PASS | `TunnelBackendRegistry` is pluggable; `TunnelBackend` trait is independent |

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
Result: 579 passed, 0 failed (4 suites)

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

### Handler
- `emissary-cli/src/i2pcontrol/tunnel_manager.rs` — TunnelManager handler with all 8 actions, option extraction/merging, lifecycle dispatch, `All` behavior, ownership enforcement, and comprehensive tests

### Modified files
- `emissary-cli/src/i2pcontrol/mod.rs` — added `tunnel_manager` module
- `emissary-cli/src/i2pcontrol/control_plane.rs` — added `TunnelManagerControl` trait, `FakeTunnelManagerControl` implementation
- `emissary-cli/src/i2pcontrol/server.rs` — added `tunnel_manager` field to `I2pControlState`, accessor methods, TunnelManager dispatch in `handle_jsonrpc`

### Documentation
- `docs/i2pcontrol/tunnel-manager.md` — TunnelManager API documentation
- `docs/i2pcontrol/proposal-170-support.md` — Proposal 170 implementation status

## 4. Test inventory

### Handler tests (tunnel_manager.rs)
- `handler_list_empty` — List returns empty array
- `handler_list_after_create` — List returns created tunnel
- `handler_create_success` — Create returns ok
- `handler_create_all_types` — Create succeeds for all 12 types
- `handler_create_duplicate_name` — Duplicate name returns error status
- `handler_create_missing_type` — Missing Type rejected
- `handler_create_missing_name` — Missing Name rejected
- `handler_create_invalid_type` — Invalid Type rejected
- `handler_get_found` — Get returns definition
- `handler_get_not_found` — Get of missing returns error
- `handler_get_missing_name` — Missing Name rejected
- `handler_get_all` — Get All rejected with error
- `handler_edit_success` — Edit updates options
- `handler_edit_rename` — Edit with NewName renames atomically
- `handler_edit_not_found` — Edit of missing returns error
- `handler_edit_preserves_omitted_fields` — Edit preserves unchanged fields
- `handler_delete_success` — Delete removes definition
- `handler_delete_not_found` — Delete of absent is successful no-op
- `handler_delete_missing_name` — Missing Name rejected
- `handler_start_unsupported` — Start returns not-implemented
- `handler_restart_unsupported` — Restart returns not-implemented
- `handler_stop_unsupported_safe` — Stop is safe/idempotent
- `handler_start_not_found` — Start of missing returns error
- `handler_start_missing_name` — Missing Name rejected
- `handler_all_start_unsupported` — All Start returns not-implemented
- `handler_all_stop_safe` — All Stop is safe
- `handler_all_empty_registry` — All on empty registry is ok
- `handler_all_rejected_for_create` — All rejected for Create
- `handler_all_rejected_for_edit` — All rejected for Edit
- `handler_all_rejected_for_get` — All rejected for Get
- `handler_all_rejected_for_delete` — All rejected for Delete
- `handler_invalid_action` — Invalid action rejected
- `handler_missing_action` — Missing Action rejected
- `handler_no_params` — No params rejected
- `handler_create_with_options` — Create with full options round-trips
- `handler_get_after_restart` — Get works after restart
- `handler_unsupported_never_reports_running` — All 12 types never report running
- `handler_create_all_types_crud_cycle` — Full CRUD for all 12 types
- `handler_start_fake_backend_succeeds` — Start succeeds with fake backend
- `handler_stop_fake_backend_succeeds` — Stop succeeds with fake backend
- `handler_restart_fake_backend_succeeds` — Restart succeeds with fake backend
- `handler_start_fake_backend_failure` — Start maps backend error to status
- `handler_concurrent_start_unsupported_deterministic` — Concurrent starts are deterministic
- `handler_stop_then_start_unsupported_deterministic` — Stop then start is deterministic
- `handler_rename_then_start_unsupported_deterministic` — Rename then start is deterministic
- `handler_delete_then_start_unsupported_deterministic` — Delete then start is deterministic
- `handler_all_start_skips_startup_managed` — All Start skips startup-managed
- `handler_all_stop_empty_after_delete` — All Stop on empty registry after delete
- `handler_startup_managed_listed_in_get` — Startup-managed listed in List/Get
- `handler_startup_managed_edit_rejected` — Edit of startup-managed rejected
- `handler_startup_managed_delete_rejected` — Delete of startup-managed rejected
- `handler_startup_managed_lifecycle_rejected` — Lifecycle of startup-managed rejected
- `secret_redaction_debug` — Secret redacted in Debug
- `secret_redaction_display` — Secret redacted in Display
- `secret_redaction_none_debug` — None secret Debug
- `secret_redaction_none_display` — None secret Display
- `handler_no_file_write_guards` — No std::fs/tokio::fs imports
- `handler_no_spawn_guards` — No tokio::spawn calls
- `handler_no_frontend_imports` — No dioxus/UI imports
- `error_response_no_internal_types` — Error messages hide internal types

### Fake control plane tests (control_plane.rs)
- `fake_tunnel_manager_control_crud` — Full CRUD lifecycle on fake

## 5. Unresolved findings

| Severity | Finding | Status |
|---|---|---|
| Low | Dead-code warnings for `TunnelManagerControl` trait methods not yet consumed by production adapters | Expected: M004 establishes trait; production adapter in later milestone |
| Low | `with_registry` constructor unused in production | Expected: public API for custom backend registries in tests |
| Low | Pre-existing dead-code warnings from M001-M002 infrastructure | Not introduced by M004 |
| Info | `BackendError::InvalidState` and `Internal` variants unused | Expected: reserved for future real backends |
| Info | `BackendStatus` and `inspect` method unused | Expected: reserved for future real backends |

No high or medium severity findings.

## 6. Disposition

**closed** — Implementation landed; closure evidence gathered; reviewed and accepted.

No corrective pass required. All 36 acceptance criteria are satisfied with evidence.

## 7. Roadmap and registry disposition

- Plan status: `closed`
- Registry: M004 moved from `active` to `closed`
- Roadmap: M004 status moved from `closing` to `closed`
- M005 may now activate (its dependencies M001–M003 are closed)
- M006 remains blocked on M004 and M005 closure
- M007 remains blocked on M003–M006 closure
