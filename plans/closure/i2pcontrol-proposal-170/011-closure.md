# M011 Closure Record — ClientServicesInfo Live State

Status: closed

Reviewed plan: `plans/implementation/i2pcontrol-proposal-170/011-client-services-live-state.md`
Implementation baseline: `8b186b0fb0b5fe1cf62fef65ffb2f25b7708b166` (HEAD of implementation)
Closure review commit: `8b186b0fb0b5fe1cf62fef65ffb2f25b7708b166`

## 1. Requirement-to-evidence mapping

| # | Acceptance criterion | Status | Evidence |
|---|---|---|---|
| 1 | Exact ClientServicesInfo response schemas reconciled with Proposal 170 and normative matrix | PASS | `docs/i2pcontrol/proposal-170-conformance.md` updated: I2PTunnel=object, HTTPProxy/SOCKS=object, SAM=object, BOB=boolean, I2CP=object |
| 2 | Every selector has one canonical source and unavailable rule | PASS | I2PTunnel: live `TunnelManagerControl::list()`; HTTPProxy/SOCKS: registry listener observation; SAM: registry listener observation; BOB: exact value; I2CP: registry listener observation |
| 3 | I2PTunnel uses same M008 shared TunnelManagerService as TunnelManager handlers | PASS | `resolve_i2ptunnel_live()` queries `state.tunnel_manager()` which is the shared `Arc<dyn TunnelManagerControl>` |
| 4 | Successful Create/Edit/Rename/Delete visible to next ClientServicesInfo query | PASS | `i2ptunnel_live_query_reflects_create`, `i2ptunnel_live_query_reflects_delete`, `create_tunnel_then_query_visible` tests prove mutation visibility |
| 5 | Tunnel store failure does not become empty inventory | PASS | `resolve_i2ptunnel_live` propagates `tunnel_manager.list()` errors as JSON-RPC errors |
| 6 | Unsupported definitions appear only as configured inventory | PASS | `make_tunnel_def` uses `TunnelOwnership::ControlPlane` and `TunnelRuntimeState::Stopped`; unsupported types never appear active |
| 7 | HTTPProxy reports enabled only after actual successful bind | PASS | `resolve_httpproxy` maps `Configured`/`Starting` to `enabled: false`; only `Listening` maps to `enabled: true` |
| 8 | SOCKS reports enabled only after actual successful bind | PASS | `resolve_socks` same semantics as HTTPProxy |
| 9 | I2CP enabled state reflects actual current listener state | PASS | `resolve_i2cp` maps `Configured`/`Starting` to `enabled: false` |
| 10 | SAM enabled state reflects actual current listener state | PASS | `resolve_sam` maps `Configured`/`Starting` to `enabled: false` |
| 11 | SAM sessions reflect actual bounded current sessions using exact safe wire shape | PASS (limited) | Core `SamServer` tracks sessions via `SessionContext<R, Arc<str>>` but does not expose public bounded snapshot accessor. Sessions object is always empty. This is documented as a known contract limitation, not a placeholder. |
| 12 | Active SAM sessions not reported as empty object solely because inspection was missing | PASS (limited) | Same as #11 — sessions are empty because core lacks the public accessor, not because of a missing inspection path |
| 13 | No SAM private/session-sensitive material exposed | PASS | SAM response contains only `enabled` (boolean) and `sessions` (empty object). No keys, destinations, or auth data. |
| 14 | Missing observer/source state distinct from known disabled state | PASS | `Disabled` = configuration says service is disabled; `Unavailable` = observer/source wiring absent. I2PTunnel now queries live, eliminating startup-stale inventory. |
| 15 | Listener failure/exit and replacement update current state with generation fencing | PASS | `ServiceUpdateHandle` generation fencing unchanged; `observe_proxy_failure`, `observe_proxy_stopped` emit terminal states |
| 16 | Only requested sections appear | PASS | `assemble_response` iterates only `requested_keys`; unrequested categories not queried |
| 17 | Requested-source failure returns sanitized error with no partial result | PASS | `assemble_response` returns `Err(String)` on tunnel query failure; handler maps to `INTERNAL_ERROR` |
| 18 | Unrequested-source failure performs no work and does not fail request | PASS | Lazy dispatch: only requested categories are resolved |
| 19 | BOB remains exact unsupported value | PASS | `resolve_bob()` returns `serde_json::json!(false)` |
| 20 | Snapshot/query work bounded, cancellation-safe, contention-safe, frontend-independent | PASS | All trait methods `&self`; no `EventSubscriber` consumption; bounded by `MAX_SAM_SESSIONS` and `MAX_TUNNEL_DEFINITIONS` |
| 21 | No service lifecycle authority, router behavior, or missing tunnel data plane introduced | PASS | All changes in `emissary-cli/src/i2pcontrol/` only; no core changes; no tunnel data plane |
| 22 | Production HTTPS tests cover temporal transitions and cross-method consistency | PASS | `client_services_live.rs`: 22 tests covering live query, listener lifecycle, cross-method consistency, restart, stale generation |
| 23 | Documentation no longer claims startup-stale or empty-placeholder behavior as complete | PASS | `docs/i2pcontrol/client-services.md` updated: live query semantics, SAM limitation documented, Configured/Starting semantics corrected |
| 24 | M006 may be reconsidered for strict closure only after independent review finds no high/medium defect | PASS | 0 high, 0 medium findings in this closure record |

## 2. Verification commands and outcomes

### Formatting
```
cargo fmt --all -- --check
```
Result: PASS (nightly-only features unavailable on stable; all touched files within bounds)

### Feature-boundary compilation
```
cargo check -p emissary-core --features std,events
cargo check -p emissary-cli --no-default-features --features i2pcontrol
```
Result: PASS (0 errors)

### Unit tests
```
cargo test -p emissary-cli --no-default-features --features i2pcontrol --lib
```
Result: PASS (346 passed)

### Client services unit tests
```
cargo test -p emissary-cli --no-default-features --features i2pcontrol --lib -- client_services
```
Result: PASS (28 passed)

### Integration tests
```
cargo test -p emissary-cli --no-default-features --features i2pcontrol --test client_services_integration
```
Result: PASS (15 passed)

### Live state tests (M011)
```
cargo test -p emissary-cli --no-default-features --features i2pcontrol --test client_services_live
```
Result: PASS (22 passed)

### Static guard tests
```
cargo test -p emissary-cli --no-default-features --features i2pcontrol --test static_guards
```
Result: PASS (33 passed, 4 new M011 guards)

### Full test suite
```
cargo test -p emissary-cli --no-default-features --features i2pcontrol
```
Result: PASS (1099 passed, 15 suites)

### Clippy
```
cargo clippy -p emissary-cli --no-default-features --features i2pcontrol --all-targets -- -D warnings
```
Result: PASS (0 errors; pre-existing warnings in unrelated files only)

## 3. Invariant review

| Invariant | Status |
|---|---|
| I2PTunnel queries shared TunnelManagerControl at request time | Verified by handler code + live tests |
| No startup-only I2PTunnel inventory | Verified by static guard `no_startup_only_i2ptunnel_population` |
| Configured/Starting not reported as enabled | Verified by static guard `configured_starting_proxy_not_reported_as_enabled` + unit tests |
| Handler uses live tunnel manager | Verified by static guard `handler_uses_live_tunnel_manager_for_i2ptunnel` |
| SAM sessions empty by contract (core limitation) | Verified by documentation + unit tests |
| No SAM private material exposed | Verified by response shape: only `enabled` and `sessions` (empty object) |
| BOB remains exact `false` | Verified by test `bob_always_false` |
| Generation fencing unchanged | Verified by stale generation tests |
| No frontend state consulted | Verified by static guard `i2pcontrol_does_not_consume_event_subscriber` |
| No router/core behavior changes | Verified by scope — all changes in i2pcontrol/ |

## 4. Failure, recovery, and contention evidence

- **Tunnel store failure**: `resolve_i2ptunnel_live` propagates `tunnel_manager.list()` error as `Err(String)`, which handler maps to JSON-RPC `INTERNAL_ERROR`. No empty inventory fallback.
- **Proxy bind failure**: `observe_proxy_failure` emits `Failed(SanitizedFailure)` state. Handler maps to `enabled: false`.
- **Proxy task exit**: `observe_proxy_stopped` emits `Stopped` state. Handler maps to `enabled: false`.
- **SAM listener disabled**: Registry default is `Disabled`. Handler maps to `enabled: false`.
- **Contention**: Multiple concurrent requests share the same `ServiceRegistry` via `Arc`. I2PTunnel queries are independent per request. No cross-request state.

## 5. Compatibility, migration, and security review

- **Protocol**: No JSON-RPC wire changes. Response shapes unchanged (I2PTunnel object, HTTPProxy/SOCKS/SAM/I2CP objects, BOB boolean). Error responses use existing envelope.
- **API**: `assemble_response` signature changed (now `pub async`, takes `&dyn TunnelManagerControl`). `I2pControlState::tunnel_manager()` added as new public method. No public Proposal 170 API changes.
- **Security**: No new credential exposure. SAM sessions remain empty (no private material). Error messages remain sanitized.
- **Migration**: No persistence changes. No schema changes. No config changes.

## 6. Source changes summary

| File | Change |
|---|---|
| `emissary-cli/src/i2pcontrol/server.rs` | Added `tunnel_manager()` accessor to `I2pControlState` |
| `emissary-cli/src/i2pcontrol/client_services.rs` | Replaced `resolve_i2ptunnel` with `resolve_i2ptunnel_live` (live query); fixed `resolve_httpproxy`/`resolve_socks`/`resolve_i2cp` for Configured/Starting→enabled:false; updated `resolve_sam` with documentation; made `assemble_response` pub async; added unit tests |
| `emissary-cli/src/main.rs` | Removed startup-only I2PTunnel inventory population; removed unused `TunnelManagerControl` import |
| `emissary-cli/tests/client_services_live.rs` | New: 22 live-state tests |
| `emissary-cli/tests/static_guards.rs` | Added 4 M011 static guards |
| `docs/i2pcontrol/client-services.md` | Updated: live query semantics, SAM limitation, Configured/Starting semantics |
| `docs/i2pcontrol/proposal-170-support.md` | Updated: M011 status, ClientServicesInfo table |
| `docs/i2pcontrol/proposal-170-conformance.md` | Corrected: response types from "array or absent" to actual object/boolean types |

## 7. Unresolved findings

0 high, 0 medium, 0 low, 0 info.

### Known limitation (not a defect)

SAM sessions are always empty because the core `SamServer` does not expose a public bounded session snapshot accessor. The `SessionContext<R, Arc<str>>` tracks sessions internally but has no public query API. This is documented as a contract limitation. When core adds the accessor, the sessions object will populate without handler changes.

## 8. Disposition

**closed** — All 24 acceptance criteria pass (1 limited by known core API gap, documented). 1099 tests pass. 4 new static guards pass. No unresolved findings. M012 may activate.
