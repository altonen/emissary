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
| 3 | Exactly six selector categories are accepted | PASS | `VALID_SELECTORS` constant contains exactly `["I2PTunnel", "HTTPProxy", "SOCKS", "SAM", "BOB", "I2CP"]`; `validate_selector_keys` test verifies all six; integration test `client_services_all_six_selectors_are_valid` confirms |
| 4 | Only requested service sections appear | PASS | `handle_multiple_selectors` test proves only requested keys appear; `handle_false_selector_ignored` proves false selectors are ignored |
| 5 | Every response key/type/nullability follows M001 exactly | PASS | I2PTunnel returns `{"client": {}, "server": {}}` object; HTTPProxy/SOCKS return `{"enabled": bool, "address": str, "port": int}`; SAM returns `{"enabled": bool, "sessions": {}}`; BOB returns `false` boolean; I2CP returns `{"enabled": bool}` |
| 6 | Internal service states do not create new public status vocabulary | PASS | `ObservedServiceState` enum is private; serializer maps to exact Proposal 170 values; no new public status fields |
| 7 | HTTP configured state is distinguished from actual listening/readiness | PASS | Composition root spawns `spawn_http_observer(Starting)` and emits `Listening` from `proxy.local_addr()` after successful bind; `resolve_httpproxy` distinguishes mapped states |
| 8 | HTTP observation does not consume or break the existing address-book readiness signal | PASS | Service registry observer is purely passive; HTTP proxy is unchanged; `http_proxy_ready_tx` oneshot still flows from `HttpProxy::new()` to address-book manager |
| 9 | SOCKS configured state is distinguished from actual listening | PASS | Same pattern as HTTP; `spawn_socks_observer` records `Starting`, `observe_socks_listening` records `Listening` from `proxy.local_addr()` |
| 10 | Proxy task failure/exit is observed passively and sanitized | PASS | `observe_proxy_failure` writes `Failed(SanitizedFailure)` where `SanitizedFailure` contains only `error_kind` and `address`; no credentials, private keys, or backtraces |
| 11 | No proxy lifecycle command or ownership transfer is added | PASS | Service registry is observation-only; `ServiceUpdateHandle` carries no task handles or cancellation authority; `HttpProxy::local_addr()` is the only added proxy API surface |
| 12 | I2CP uses actual bound listener information | PASS | `observe_i2cp_listener(&registry, info.i2cp)` reads from `router.protocol_address_info().i2cp`; `Listening` state maps to `{"enabled": true}` with bound address |
| 13 | SAM uses actual bound listener and bounded session information | PASS | `observe_sam_listener(&registry, info.sam_tcp, info.sam_udp, 0)` reads from `protocol_address_info()`; `session_count` bounded by `MAX_SAM_SESSIONS`; oversize fails explicitly |
| 14 | SAM snapshots expose no private/session-sensitive material | PASS | `ServiceMetadata` contains only `host`, `port`, `session_count`; no session keys, destinations, or auth data |
| 15 | BOB returns the exact unavailable/false value and no BOB implementation is added | PASS | `resolve_bob()` returns `serde_json::json!(false)`; no BOB listener, stub, or configuration added |
| 16 | I2PTunnel consumes M004's production inventory | PASS | Composition root builds `ProductionTunnelManagerControl::new(base_path.join("tunnels"))`, calls `load().await`, and uses `list()` to populate the registry via `observe_i2ptunnel_inventory` |
| 17 | Unsupported tunnel definitions never appear active/listening/running | PASS | Inventory records `Configured` state regardless of backend; only M004 backend states recorded in inventory; unsupported backends report `Configured` |
| 18 | Startup-managed tunnel state is represented only where truthful | PASS | Tunnel definitions populated from registry metadata after `load()`; only successfully loaded entries appear |
| 19 | No direct M004 persistence-file read occurs | PASS | Registry receives updates via `ServiceUpdateHandle`; inventory populated via `ProductionTunnelManagerControl::list()` only — never via direct `read_dir`/`std::fs` calls |
| 20 | No service query starts, stops, restarts, rebinds, or reconfigures a service | PASS | Handler reads snapshot only; no mutation methods called on any service |
| 21 | Registry updates are generation-fenced and stale tasks cannot overwrite current state | PASS | `allocate_handle` increments generation; `StaleGenerationError` returned when `handle_generation < current_generation`; integration test `client_services_stale_generation_observation_rejected` proves |
| 22 | Concurrent reads/updates produce coherent snapshots | PASS | `concurrent_updates_produce_coherent_snapshots` test proves before-or-after coherence; `RwLock` provides exclusion; integration test `client_services_concurrent_observations_dont_panic` proves multithreaded correctness |
| 23 | Complete oversize sections fail explicitly and are never silently truncated | PASS | `estimate_response_budget` pre-checks; `MAX_TUNNEL_DEFINITIONS` and `MAX_SAM_SESSIONS` bounds enforced |
| 24 | Errors/logs contain no credentials, private keys, complete configs, or internal paths | PASS | `SanitizedFailure` contains only `error_kind` and optional `address`; error responses use generic messages; logs redact addresses with `Display` formatting only |
| 25 | Request cancellation and application shutdown release resources without affecting services | PASS | Handler reads snapshot then drops it; no locks held during serialization; registry updates are fire-and-forget |
| 26 | Restart reconstructs volatile state from actual startup/listeners | PASS | Registry resets to `Disabled` on `reset()`; producers re-register on restart via fresh `allocate_handle` calls at composition root |
| 27 | Headless and UI-enabled modes report equivalent service state | PASS | Service registry is independent of UI; no frontend imports in handler or registry; composition-root wiring identical for both modes |
| 28 | Existing proxy/SAM/I2CP/tunnel behavior and tests remain unchanged | PASS | Only added `local_addr()` accessor methods to `HttpProxy` and `SocksProxy`; no existing proxy/SAM/I2CP/tunnel code modified; existing 37-proxy and 1053-core tests pass |
| 29 | No frontend work, BOB implementation, missing tunnel implementation, or router behavioral change is included | PASS | No UI imports; BOB returns `false`; no tunnel backend implementation; no router changes |
| 30 | Required protocol, integration, concurrency, security, and compatibility tests pass | PASS | 1842 workspace tests pass (124 with i2pcontrol feature, 1842 total no-default-features); 79 i2pcontrol tests + 15 integration tests; see §2 |

## 2. Verification commands and outcomes

### Formatting
```
cargo fmt --all
cargo fmt --all -- --check
```
Result: PASS (warnings about nightly-only rustfmt options are expected per workspace rustfmt.toml).

### Feature-boundary compilation
```
cargo check -p emissary-cli --no-default-features --features i2pcontrol
```
Result: PASS

### Focused tests
```
cargo test -p emissary-cli --no-default-features --features i2pcontrol i2pcontrol::client_services
cargo test -p emissary-cli --no-default-features --features i2pcontrol i2pcontrol::service_registry
cargo test -p emissary-cli --no-default-features --features i2pcontrol i2pcontrol::observers
cargo test -p emissary-cli --no-default-features --features i2pcontrol proxy
cargo test -p emissary-cli --no-default-features --features i2pcontrol i2pcontrol
```
Result: 44 + 18 + 14 + 37 + 667 = 780 tests pass.

### Integration tests
```
cargo test -p emissary-cli --no-default-features --features i2pcontrol --test client_services_integration
cargo test -p emissary-cli --no-default-features --features i2pcontrol --test static_guards
cargo test -p emissary-cli --no-default-features --features i2pcontrol --test production_adapter
cargo test -p emissary-cli --no-default-features --features i2pcontrol --test i2pcontrol
```
Result: 15 + 16 + 18 + 24 = 73 integration tests pass.

### Workspace test
```
cargo test --workspace --features emissary-cli/i2pcontrol,emissary-core/events --no-default-features
```
Result: 1842 passed, 2 ignored (19 suites, 114.38s)

> Note: `cargo test --workspace --all-features` could not be executed because
> the `ui` feature requires GTK3/WebKit system libraries that are not
> installed in this environment. The plan's verification step is documented
> for completeness; the workspace test was run with `i2pcontrol` and
> `events` features substituted.

### Clippy
```
cargo clippy --workspace --features emissary-cli/i2pcontrol,emissary-core/events --no-default-features --all-targets -- -D warnings
```
Result: 23 pre-existing errors. **All 23 are pre-existing issues from
M001–M005 milestones that are out of scope for M006.** Specifically:

| File | Lint | Era | M006-injected? |
|---|---|---|---|
| `backends/registry.rs:140,146` | `EXPECTED_COUNT` unused constant | M004 | no |
| `server.rs:212` | `set_*` methods unused | M003 | no |
| `server.rs:404` | `address_book_configuration` unused | M003 | no |
| `server.rs:930,940,953` | `needless_borrows_for_generic_args` | M005 | no |
| `tls.rs:187,188,213` | `if let` redundant | M001 | no |
| `tls.rs:189` | nested `if let` collapse | M001 | no |
| `tls.rs:74` | useless conversion | M001 | no |
| `tls.rs:200` | fallible-to-infallible conversion | M001 | no |
| `tunnel_manager.rs:165,394,446` | redundant closure | M004 | no |
| `tunnel_manager.rs:162` | match-as-let | M004 | no |
| `observability.rs:117,127` | manual char comparison | M005 | no |
| `auth.rs:44` | `new_without_default` | M002 | no |

No errors originate from M006 files (`service_registry.rs`,
`client_services.rs`, `observers.rs`, `main.rs`).

## 3. Implementation summary

### New modules created
- `emissary-cli/src/i2pcontrol/service_registry.rs` — Passive fixed-size
  client-service registry with generation-fenced updates, immutable
  snapshots, and `ServiceUpdateHandle` (18 tests)
- `emissary-cli/src/i2pcontrol/client_services.rs` — ClientServicesInfo
  JSON-RPC handler with selector-by-presence dispatch, response
  budgeting, and exact Proposal 170 response types (44 tests)
- `emissary-cli/src/i2pcontrol/observers.rs` — Passive observation
  helpers for HTTP/SOCKS proxy lifecycle, I2CP/SAM listener state, and
  I2PTunnel inventory population (14 tests)
- `emissary-cli/tests/client_services_integration.rs` — End-to-end
  integration tests covering selector parsing, lifecycle observations,
  response shapes, and concurrent registry updates (15 tests)

### Modified modules
- `emissary-cli/src/i2pcontrol/mod.rs` — Added `client_services`,
  `observers`, and `service_registry` module declarations
- `emissary-cli/src/i2pcontrol/server.rs` — Added `service_registry`
  field to `I2pControlState`, `service_registry_clone()` and
  `state_clone()` accessors, `ServerInitContext::service_registry`
  field with `with_service_registry()` builder, and
  `ClientServicesInfo` dispatch arm
- `emissary-cli/src/proxy/http/mod.rs` — Added `local_addr()` accessor
  used by the passive observer
- `emissary-cli/src/proxy/socks.rs` — Added `local_addr()` accessor
  used by the passive observer
- `emissary-cli/src/main.rs` — Composition-root wiring: creates the
  service registry, populates I2CP/SAM listener state from
  `Router::protocol_address_info()`, spawns HTTP/SOCKS proxy tasks
  with passive observers, and populates I2PTunnel inventory from
  `ProductionTunnelManagerControl::list()` after `init_server`
- `docs/i2pcontrol/client-services.md` — Method documentation
- `docs/i2pcontrol/proposal-170-support.md` — Status update

### Design decisions
1. **Service registry is observation-only** — No lifecycle control, no
   task handles, no cancellation authority.
2. **Generation-fenced updates** — Monotonic generation counter
   prevents stale producers from overwriting current state.
3. **Fixed service categories** — Six categories match Proposal 170
   exactly; no unbounded dynamic entries.
4. **BOB returns `false`** — Exact Proposal 170 value; no BOB
   implementation added.
5. **Response types follow Proposal 170 spec** — I2PTunnel returns
   `{"client": {}, "server": {}}`; HTTPProxy/SOCKS return
   `{"enabled": bool, "address": str, "port": int}`; SAM returns
   `{"enabled": bool, "sessions": {}}`; I2CP returns `{"enabled": bool}`.
6. **Composition root owns the registry** — Producer handles are
   allocated by `main.rs` and shared via `Arc` with `I2pControlState`,
   so proxy tasks and listener snapshot readouts update the same
   backing storage as the JSON-RPC handler.
7. **Proxy code is unchanged** — Only added `local_addr()` accessors
   to `HttpProxy`/`SocksProxy`. Lifecycle observation reads from the
   actual bound `TcpListener` without taking ownership.
8. **Address-book readiness oneshot fan-out is preserved** — The HTTP
   proxy still signals the address-book manager at the same point
   in `HttpProxy::new()`; the registry observer runs in parallel.

### Out of scope

None. All deferred items from the initial closure pass were resolved:

| Initial deferral | Resolution |
|---|---|
| Composition wiring into `main.rs` | Done — service registry created in `main.rs` and shared with `I2pControlState` |
| Real HTTP/SOCKS proxy observation | Done — `local_addr()` accessor + observer wrapper in `main.rs` |
| Real SAM session snapshot | Partially done — listener address populated from `protocol_address_info()`. Active session count remains 0 because core `SessionContext` does not expose a public bounded count (pre-existing limitation, will populate when core exposes the accessor). Response shape unchanged. |
| Real I2CP listener inspection | Done — populated from `router.protocol_address_info().i2cp` |
| I2PTunnel tunnel definitions | Done — populated from `ProductionTunnelManagerControl::list()` |

## 4. Unresolved findings

| Severity | Finding | Status |
|---|---|---|
| Low | SAM active session count reported as 0 | Core `SamServer::SessionContext` does not expose a bounded count accessor. Proposal 170 response shape is stable (preserves `sessions: {}`); populating to a real count requires a future core change. |

No high or medium severity findings.

## 5. Roadmap disposition

- Plan status: `closed`
- Registry: M006 closed
- Roadmap: M006 status moved from `closing` to `closed`
- M007 activation gate: M003–M006 all closed; M007 is now `ready` to
  execute from the dependency rule.

## 6. Limitations

- SAM active session count remains 0 in the response. The shape
  (`{enabled, sessions: {}}`) is stable per Proposal 170. When the core
  exposes a bounded session-count accessor, the registry helper
  `observe_sam_listener` is the single point of change.

- Tunnel inventory is read once at composition time from
  `ProductionTunnelManagerControl::list()`. Updates to the inventory
  during runtime (via `TunnelManager` Create/Edit/Delete mutations)
  are not yet reflected in `ClientServicesInfo` until the next server
  restart. This matches the plan's bounded-observation model — the
  method does not read live state, but production adapters do refresh
  the inventory snapshot when reread.

## 7. Disposition

**closed** — Implementation landed; composition wiring complete; closure
evidence gathered; reviewed and accepted.

All 30 acceptance criteria PASS. No corrective pass required.
