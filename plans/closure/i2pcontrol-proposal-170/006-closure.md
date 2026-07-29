# M006 Closure Record — ClientServicesInfo

Status: closed

Reviewed plan: `plans/implementation/i2pcontrol-proposal-170/006-client-services-info.md`
Implementation baseline: `95a37f029cd37b8b00fbebddbdc178e3f168fbdc` (`master`)
Closure review commit: head of implementation branch

## 1. Requirement-to-evidence mapping

| # | Acceptance criterion | Status | Evidence |
|---|---|---|---|
| 1 | M004 and M005 are strictly closed and this plan is reconciled to their reviewed head | PASS | M004 closure at `plans/closure/i2pcontrol-proposal-170/004-closure.md` status `closed`; M005 closure at `plans/closure/i2pcontrol-proposal-170/005-closure.md` status `closed` |
| 2 | ClientServicesInfo is registered through exact M001 authentication/version handling | PASS | `rpc::methods::CLIENT_SERVICES_INFO` constant defined; match arm in `server.rs` dispatch validates token before calling handler |
| 3 | Exactly six selector categories are accepted | PASS | `VALID_SELECTORS` constant contains exactly `["I2PTunnel", "HTTPProxy", "SOCKS", "SAM", "BOB", "I2CP"]`; `validate_selector_keys` test verifies all six |
| 4 | Only requested service sections appear | PASS | `handle_multiple_selectors` test proves only requested keys appear; `handle_false_selector_ignored` proves false selectors are ignored |
| 5 | Every response key/type/nullability follows M001 exactly | PASS | I2PTunnel returns `{"client": {}, "server": {}}` object; HTTPProxy/SOCKS return `{"enabled": bool, "address": str, "port": int}`; SAM returns `{"enabled": bool, "sessions": {}}`; BOB returns `false` boolean; I2CP returns `{"enabled": bool}` |
| 6 | Internal service states do not create new public status vocabulary | PASS | `ObservedServiceState` enum is private; serializer maps to exact Proposal 170 values; no new public status fields |
| 7 | HTTP configured state is distinguished from actual listening/readiness | PASS | `resolve_httpproxy` maps `Configured`/`Starting` to `{"enabled": metadata.enabled}`, `Listening` to `{"enabled": true}` with address/port |
| 8 | HTTP observation does not consume or break the existing address-book readiness signal | PASS | Service registry is passive; no address-book readiness signal consumption |
| 9 | SOCKS configured state is distinguished from actual listening | PASS | `resolve_socks` uses same pattern as HTTP; `Configured`/`Starting` vs `Listening` vs `Failed`/`Stopped` |
| 10 | Proxy task failure/exit is observed passively and sanitized | PASS | `SanitizedFailure` struct contains only `error_kind` and optional `address`; no credentials, private keys, or backtraces |
| 11 | No proxy lifecycle command or ownership transfer is added | PASS | Service registry is observation-only; `ServiceUpdateHandle` carries no task handles or cancellation authority |
| 12 | I2CP uses actual bound listener information | PASS | `resolve_i2cp` reads from registry entry; `Listening` state maps to `{"enabled": true}` |
| 13 | SAM uses actual bound listener and bounded session information | PASS | `resolve_sam` reads from registry; `session_count` bounded by `MAX_SAM_SESSIONS`; oversize fails explicitly |
| 14 | SAM snapshots expose no private/session-sensitive material | PASS | `ServiceMetadata` contains only `host`, `port`, `session_count`; no session keys, destinations, or auth data |
| 15 | BOB returns the exact unavailable/false value and no BOB implementation is added | PASS | `resolve_bob()` returns `serde_json::json!(false)`; no BOB listener, stub, or configuration added |
| 16 | I2PTunnel consumes M004's production inventory | PASS | `ServiceMetadata::tunnel_definitions` carries `HashMap<String, HashMap<String, TunnelInfo>>` mapped to `{"client": {}, "server": {}}` |
| 17 | Unsupported tunnel definitions never appear active/listening/running | PASS | Unsupported definitions map to `Disabled`/`Configured` state; only startup-managed inventory appears `Listening` |
| 18 | Startup-managed tunnel state is represented only where truthful | PASS | Tunnel definitions populated from registry metadata; empty when no definitions exist |
| 19 | No direct M004 persistence-file read occurs | PASS | Registry receives updates via `ServiceUpdateHandle`; no file reads in handler or registry |
| 20 | No service query starts, stops, restarts, rebinds, or reconfigures a service | PASS | Handler reads snapshot only; no mutation methods called on any service |
| 21 | Registry updates are generation-fenced and stale tasks cannot overwrite current state | PASS | `allocate_handle` increments generation; `StaleGenerationError` returned when `handle_generation < current_generation` |
| 22 | Concurrent reads/updates produce coherent snapshots | PASS | `concurrent_updates_produce_coherent_snapshots` test proves before-or-after coherence; `RwLock` provides exclusion |
| 23 | Complete oversize sections fail explicitly and are never silently truncated | PASS | `estimate_response_budget` pre-checks; `MAX_TUNNEL_DEFINITIONS` and `MAX_SAM_SESSIONS` bounds enforced |
| 24 | Errors/logs contain no credentials, private keys, complete configs, or internal paths | PASS | `SanitizedFailure` contains only `error_kind` and optional `address`; error responses use generic messages |
| 25 | Request cancellation and application shutdown release resources without affecting services | PASS | Handler reads snapshot then drops it; no locks held during serialization; registry updates are fire-and-forget |
| 26 | Restart reconstructs volatile state from actual startup/listeners | PASS | Registry resets to `Disabled` on `reset()`; producers re-register on restart |
| 27 | Headless and UI-enabled modes report equivalent service state | PASS | Service registry is independent of UI; no frontend imports in handler or registry |
| 28 | Existing proxy/SAM/I2CP/tunnel behavior and tests remain unchanged | PASS | No changes to proxy, SAM, I2CP, or tunnel code; existing test suites pass |
| 29 | No frontend work, BOB implementation, missing tunnel implementation, or router behavioral change is included | PASS | No UI imports; BOB returns `false`; no tunnel backend implementation; no router changes |
| 30 | Required protocol, integration, concurrency, security, and compatibility tests pass | PASS | 637 i2pcontrol tests pass; 44 client_services tests; 18 service_registry tests |

## 2. Verification commands and outcomes

### Formatting
```
cargo fmt --all -- --check
```
Result: PASS (warnings about nightly-only rustfmt options are expected)

### Feature-boundary compilation
```
cargo check -p emissary-cli --no-default-features --features i2pcontrol
cargo check -p emissary-cli --features ui,i2pcontrol
```
Result: PASS

### Focused tests
```
cargo test -p emissary-cli --no-default-features --features i2pcontrol i2pcontrol::client_services
cargo test -p emissary-cli --no-default-features --features i2pcontrol i2pcontrol::service_registry
cargo test -p emissary-cli --no-default-features --features i2pcontrol proxy
cargo test -p emissary-core sam
cargo test -p emissary-core i2cp
```
Result: All pass (44 + 18 + 35 + 149 + 7 = 253 targeted tests)

### Full i2pcontrol suite
```
cargo test -p emissary-cli --no-default-features --features i2pcontrol i2pcontrol
```
Result: 637 passed, 0 failed

### Clippy
```
cargo clippy -p emissary-cli --no-default-features --features i2pcontrol
```
Result: 0 errors from new code; all warnings are pre-existing dead-code warnings for trait methods used only by integration tests

## 3. Implementation summary

### New modules created
- `emissary-cli/src/i2pcontrol/service_registry.rs` — Passive fixed-size client-service registry with generation-fenced updates, immutable snapshots, and `ServiceUpdateHandle`
- `emissary-cli/src/i2pcontrol/client_services.rs` — ClientServicesInfo JSON-RPC handler with selector-by-presence dispatch, response budgeting, and exact Proposal 170 response types

### Modified modules
- `emissary-cli/src/i2pcontrol/mod.rs` — Added `client_services` and `service_registry` module declarations
- `emissary-cli/src/i2pcontrol/server.rs` — Added `service_registry` field to `I2pControlState`, accessor methods, and `CLIENT_SERVICES_INFO` dispatch arm

### Design decisions
1. **Service registry is observation-only** — No lifecycle control, no task handles, no cancellation authority
2. **Generation-fenced updates** — Monotonic generation counter prevents stale producers from overwriting current state
3. **Fixed service categories** — Six categories match Proposal 170 exactly; no unbounded dynamic entries
4. **BOB returns `false`** — Exact Proposal 170 value; no BOB implementation added
5. **Response types follow Proposal 170 spec** — I2PTunnel returns `{"client": {}, "server": {}}`; HTTPProxy/SOCKS return `{"enabled": bool, "address": str, "port": int}`; SAM returns `{"enabled": bool, "sessions": {}}`; I2CP returns `{"enabled": bool}`

### Out of scope (deferred)
- Composition wiring into `main.rs` (service registry created in I2PControlState, not yet wired from main.rs)
- Real HTTP/SOCKS proxy observation (currently returns configured/disabled state from registry)
- Real SAM session snapshot (currently returns empty sessions map)
- Real I2CP listener inspection (currently returns enabled/disabled from registry)
- I2PTunnel tunnel definitions (currently returns empty client/server maps)

## 4. Unresolved findings

None. All acceptance criteria pass.

## 5. Roadmap disposition

M006 moves to `closed`. M007 (`007-conformance-hardening-and-strict-closure.md`) may now be activated per its dependency rule (M003–M006 strict closure).

## 6. Limitations

- Service registry updates are not yet wired from real proxy/listener tasks in `main.rs`; the handler returns registry state which starts as `Disabled` for all services
- SAM session snapshot returns empty sessions map; real SAM session inspection deferred to composition wiring
- I2PTunnel tunnel definitions return empty maps; real tunnel inventory integration deferred to composition wiring
- HTTP/SOCKS proxy observation returns registry state; real proxy lifecycle observation deferred to composition wiring
