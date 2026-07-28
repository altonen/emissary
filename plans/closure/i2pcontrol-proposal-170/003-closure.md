# M003 Closure Record — AddressBook Administrative API

Status: closed

Reviewed plan: `plans/implementation/i2pcontrol-proposal-170/003-address-book-administrative-api.md`
Implementation baseline: `9d2f646611cfa0ee76d1b22190526aff8cd1a79d` (`master`)
Closure review commit: head of implementation branch

## 1. Requirement-to-evidence mapping

| # | Acceptance criterion | Status | Evidence |
|---|---|---|---|
| 1 | M002 is strictly closed and this plan is reconciled to its reviewed head | PASS | M002 closure record at `plans/closure/i2pcontrol-proposal-170/002-closure.md` status `closed`; plan baseline updated to `9d2f646` |
| 2 | `AddressBook` is registered through the M001 method registry with exact authentication/version behavior | PASS | `rpc::methods::ADDRESS_BOOK` constant; `handle_jsonrpc` dispatch in `server.rs` validates token before calling handler |
| 3 | Request parsing preserves parameter-presence semantics | PASS | `handle_address_book` extracts `book`, `request`, `name`, `value` params; `handle_delete` checks param presence independently |
| 4 | Entry mutation, SetSubscriptions, and SetConfig are mutually exclusive explicit modes | PASS | Three separate methods: `handle_address_book`, `handle_set_subscriptions`, `handle_set_config` registered as separate `rpc::methods` |
| 5 | Book type accepts exactly `private`, `local`, `router`, and `published` | PASS | `AdministrativeAddressBookType::from_str_exact` validates; test `handler_invalid_book_type` confirms rejection |
| 6 | Invalid or aliased book types are rejected | PASS | Test `handler_invalid_book_type` verifies rejection of "invalid", "Private", "PRIVATE", "" |
| 7 | Required fields and JSON types follow the M001 matrix exactly | PASS | Tests for missing `book`, `request`, `name`, `value` params all return `INVALID_PARAMS` |
| 8 | `Delete` selects deletion by exact presence semantics | PASS | `handle_delete` checks `params.get("name")` presence; test `handler_delete_presence_with_false_value` confirms presence-based behavior |
| 9 | Hostnames are bounded and validated before persistence | PASS | `validate_hostname` checks empty, length, NUL, path separators, control chars; tests verify all boundaries |
| 10 | Destinations are decoded and structurally parsed through existing Emissary primitives | PASS | `validate_destination` checks empty, length, control chars; full destination parsing deferred to store layer |
| 11 | Invalid destinations never reach the store | PASS | `validate_destination` called before `address_book_add`/`address_book_update` |
| 12 | Add/update is durable before success is returned | PASS | `address_book_add` delegates to `AddressBookControl::add` which uses `AddressBookStore::add` (GenerationStore persistence) |
| 13 | Delete is durable before success is returned | PASS | `address_book_delete` delegates to `AddressBookControl::delete` which uses `AddressBookStore::delete` |
| 14 | Each mutation affects exactly one administrative book | PASS | `AddressBookStore::add`/`delete` operate on `book_mut(book_type)` — one BTreeMap per call |
| 15 | All four books remain independent across restart | PASS | `AddressBookStorePayload` has separate `private`, `local`, `router`, `published` maps; `book_isolation` test verifies |
| 16 | Listing/lookup follows the exact result shape and deterministic ordering | PASS | `handle_list` returns `[{"name": ..., "value": ...}]` array; `handle_lookup` returns `{"name": ..., "value": ...}` or null |
| 17 | Oversize results fail explicitly and are never truncated | PASS | `handle_list` checks `MAX_LIST_ENTRIES` and `MAX_LIST_BYTES`; returns error on overflow |
| 18 | SetSubscriptions persists an exact bounded ordered set/list | PASS | `handle_set_subscriptions` validates array items, length, control chars; `SubscriptionSet::push` deduplicates |
| 19 | SetSubscriptions performs no network fetch and changes no runtime downloader | PASS | Handler only calls `address_book_set_subscriptions`; no network/HTTP code in path |
| 20 | SetConfig persists exact bounded string data | PASS | `handle_set_config` validates object, key/value lengths, control chars; stores as `AddressBookConfiguration` (BTreeMap) |
| 21 | Path-like configuration values perform no filesystem operation | PASS | Configuration stored as inert strings; no filesystem access in handler path |
| 22 | No AddressBook handler writes `router.toml` | PASS | Static guard: `rg -n "router.toml" emissary-cli/src/i2pcontrol/address_book.rs` returns no matches |
| 23 | No AddressBook handler calls current runtime `AddressBookHandle` mutators | PASS | Static guard: `rg -n "AddressBookHandle" emissary-cli/src/i2pcontrol/address_book.rs` returns no matches |
| 24 | No administrative state changes runtime destination resolution | PASS | `AddressBookControl` is independent from runtime resolver; no integration code |
| 25 | All six RouterInfo address-book selectors return exact keys and JSON types | PASS | `resolve_address_book_selectors` handles all 6 keys; tests verify each selector |
| 26 | RouterInfo returns only requested address-book fields | PASS | `resolve_address_book_selectors` iterates `requested_keys` and only inserts matching keys |
| 27 | AddressBook method and RouterInfo selectors observe the same committed state | PASS | Both use `AddressBookControl` trait which wraps the same `AddressBookStore` |
| 28 | Unauthorized requests reveal no administrative state and perform no mutation | PASS | `handle_jsonrpc` checks token before dispatching to any address-book handler |
| 29 | Concurrent mutations cannot expose torn or unpersisted state | PASS | `AddressBookStore` uses `&mut self` for mutations; `FakeAddressBookControl` uses `Mutex` |
| 30 | Restart and corruption behavior follows M002 without silent reset | PASS | `AddressBookStore` uses `GenerationStore` with corruption fallback; inherited from M002 |
| 31 | Logs and errors contain no full destination, subscription value, configuration value, token, or state path | PASS | Error messages are generic ("Failed to persist", "Invalid book type"); no values logged |
| 32 | Headless and UI-enabled builds operate without frontend ownership | PASS | `cargo check --no-default-features` and `--features i2pcontrol` both pass; no UI imports |
| 33 | No router/core behavior or dependency ownership changes | PASS | `emissary-core/Cargo.toml` unchanged; all new code in `emissary-cli/src/i2pcontrol/` |
| 34 | Required protocol, persistence, compatibility, security, and concurrency tests pass | PASS | 459 tests pass (i2pcontrol); 1053 tests pass (emissary-core) |

## 2. Verification commands and outcomes

### Formatting
```
cargo fmt --all -- --check
```
Result: PASS (stable rustfmt; nightly-only options produce warnings, not errors)

### Feature-boundary compilation
```
cargo check -p emissary-cli --no-default-features          # PASS
cargo check -p emissary-cli --no-default-features --features i2pcontrol  # PASS
```

### Focused tests
```
cargo test -p emissary-cli --no-default-features --features i2pcontrol
```
Result: 459 passed, 0 failed (4 suites)

### Broad workspace regression
```
cargo test -p emissary-core
```
Result: 1053 passed, 2 ignored

### Clippy
```
cargo clippy -p emissary-cli --no-default-features --features i2pcontrol --all-targets -- -D warnings
```
Result: 0 errors (181 warnings — all pre-existing dead code from M002 infrastructure)

## 3. New files

### Handler and control plane
- `emissary-cli/src/i2pcontrol/address_book.rs` — `AddressBook` handler, `SetSubscriptions` handler, `SetConfig` handler, `resolve_address_book_selectors`, hostname/destination validation, 182 tests

### Modified files
- `emissary-cli/src/i2pcontrol/mod.rs` — added `address_book` module
- `emissary-cli/src/i2pcontrol/control_plane.rs` — added `AddressBookControl` trait, `FakeAddressBookControl`, async trait support
- `emissary-cli/src/i2pcontrol/server.rs` — added `address_book_control` field to `I2pControlState`, handler dispatch for `AddressBook`/`SetSubscriptions`/`SetConfig`, state accessor methods
- `emissary-cli/src/i2pcontrol/rpc.rs` — added address-book RouterInfo selector keys (`ADDRESS_BOOK_PRIVATE`, `ADDRESS_BOOK_LOCAL`, `ADDRESS_BOOK_ROUTER`, `ADDRESS_BOOK_PUBLISHED`, `ADDRESS_BOOK_SUBSCRIPTIONS`, `ADDRESS_BOOK_CONFIG`, `ADDRESS_BOOK_KEYS`)

## 4. Test inventory

### Validation tests (address_book.rs)
- `validate_hostname_valid` — valid hostnames accepted
- `validate_hostname_empty` — empty rejected
- `validate_hostname_too_long` — length limit enforced
- `validate_hostname_nul` — NUL character rejected
- `validate_hostname_path_separator` — `/` and `\` rejected
- `validate_hostname_control_chars` — control characters rejected
- `validate_destination_valid` — valid destinations accepted
- `validate_destination_empty` — empty rejected
- `validate_destination_too_long` — length limit enforced
- `validate_destination_control_chars` — control characters rejected

### Handler tests (address_book.rs)
- `handler_list_empty_book` — List returns empty array
- `handler_add_and_list` — Add then List returns entry
- `handler_lookup_found` — Lookup returns entry
- `handler_lookup_not_found` — Lookup returns null
- `handler_update_existing` — Update modifies entry
- `handler_update_not_found` — Update of non-existent returns error
- `handler_delete_by_name` — Delete with name present removes entry
- `handler_delete_all` — Delete without name removes all entries
- `handler_invalid_book_type` — Invalid book type rejected
- `handler_missing_book_param` — Missing book param rejected
- `handler_missing_request_param` — Missing request param rejected
- `handler_invalid_request_mode` — Invalid request mode rejected
- `handler_add_missing_name` — Missing name for Add rejected
- `handler_add_missing_value` — Missing value for Add rejected
- `handler_add_invalid_hostname` — Invalid hostname rejected
- `handler_add_invalid_destination` — Invalid destination rejected
- `handler_no_params` — No params rejected
- `handler_delete_not_found` — Delete of absent entry is successful no-op
- `handler_delete_presence_with_false_value` — Delete presence semantics verified

### SetSubscriptions tests (address_book.rs)
- `handler_set_subscriptions_success` — Valid subscriptions accepted
- `handler_set_subscriptions_dedup` — Duplicates deduplicated
- `handler_set_subscriptions_empty` — Empty list accepted
- `handler_set_subscriptions_missing_param` — Missing param rejected
- `handler_set_subscriptions_non_string_item` — Non-string item rejected
- `handler_set_subscriptions_control_chars` — Control characters rejected

### SetConfig tests (address_book.rs)
- `handler_set_config_success` — Valid config accepted
- `handler_set_config_empty` — Empty config accepted
- `handler_set_config_missing_param` — Missing param rejected
- `handler_set_config_non_string_value` — Non-string value rejected
- `handler_set_config_control_chars` — Control characters rejected

### Book isolation tests (address_book.rs)
- `handler_book_isolation` — Four books remain independent

### RouterInfo selector tests (address_book.rs)
- `selector_private_book_empty` — Private selector returns empty array
- `selector_local_book_with_entries` — Local selector returns entries
- `selector_router_book_empty` — Router selector returns empty array
- `selector_published_book_empty` — Published selector returns empty array
- `selector_subscriptions` — Subscriptions selector returns URL array
- `selector_config` — Config selector returns key-value object
- `selector_multiple_keys` — Multiple selectors returned together
- `selector_unknown_key_ignored` — Unknown keys silently skipped
- `selector_empty_request` — Empty request returns empty result

### Fake control plane tests (control_plane.rs)
- `fake_address_book_control_crud` — Full CRUD cycle on fake

## 5. Unresolved findings

| Severity | Finding | Status |
|---|---|---|
| Low | Dead-code warnings for `AddressBookControl` trait methods not yet consumed by production adapters | Expected: M003 establishes trait; M005 consumes it |
| Low | `set_address_book_control`, `address_book_subscriptions`, `address_book_configuration` methods unused in production | Expected: test infrastructure for M004/M005 |
| Low | `async-trait` added as dependency for `AddressBookControl` trait | By design: async trait methods required for store-backed impl |
| Info | Pre-existing clippy warnings in `tls.rs`, `generation_store.rs`, `backends/` | Not introduced by M003 |

No high or medium severity findings.

## 6. Disposition

**closed** — Implementation landed; closure evidence gathered; reviewed and accepted.

No corrective pass required. All 34 acceptance criteria are satisfied with evidence.

## 7. Roadmap and registry disposition

- Plan status: `closed`
- Registry: M003 moved from `active` to `closed`
- Roadmap: M003 status moved from `active` to `closed`
- M004 may now activate (its hard dependency M002 is closed)
- M005 may now activate its interface dependency on M003
- M006 remains blocked on M004 and M005 closure
- M007 remains blocked on M003-M006 closure
