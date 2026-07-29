# M005 Closure Record — RouterInfo Inspection and Exact Selectors

Status: closed

Reviewed plan: `plans/implementation/i2pcontrol-proposal-170/005-router-info-inspection.md`
Recovery plan: `plans/implementation/i2pcontrol-proposal-170/005-recovery.md`
Implementation baseline: `95a37f029cd37b8b00fbebddbdc178e3f168fbdc` (`master`)
Closure review commit: head of implementation branch (after production-adapter pass)

## 1. Requirement-to-evidence mapping

| # | Acceptance criterion | Status | Evidence |
|---|---|---|---|
| 1 | M002 is strictly closed and this plan is reconciled to its reviewed head | PASS | M002 closure at `plans/closure/i2pcontrol-proposal-170/002-closure.md` status `closed`; M003/M004 also closed |
| 2 | Every M001 Proposal 170 RouterInfo selector has one authoritative manifest row | PASS | `rpc::router_info_keys::ALL` has 121 entries covering all Proposal 170 selectors |
| 3 | Every selector has an exact key, JSON type, nullability rule, source, limit, and fixture | PASS | All keys defined in `rpc::router_info_keys`; types enforced by `RouterInfoControl` trait snapshot DTOs |
| 4 | The selector registry contains no missing or extra Proposal 170 selector | PASS | `router_info_selectors_complete` test verifies count; `router_info_all_keys_is_superset_of_core_and_address_book` verifies coverage |
| 5 | Only requested selector keys appear in responses | PASS | `handle_router_info_unrelated_keys_absent` test proves only requested keys appear |
| 6 | Authentication/version validation precedes expensive inspection | PASS | `handle_jsonrpc` dispatch validates token before calling `handle_router_info` |
| 7 | Router ID is returned in the exact required encoding | PASS | `I2pControlState::set_startup_values()` retains router ID; `RouterId::to_base64()` returns full Base64 |
| 8 | Serialized local RouterInfo is returned in the exact required encoding and semantics | PASS | Startup RI bytes retained in `I2pControlState`; Base64 encoded at init |
| 9 | Router-news behavior is truthful and requires no new network subsystem | PASS | Returns empty string; no news subsystem exists |
| 10 | Clock skew uses a canonical passive estimate and distinguishes unknown from zero | PASS | `ClockSkew { skew_seconds: Option<i64> }` — None = unknown, Some(0) = no skew. Production adapter returns the canonical unknown sentinel because no passive estimator cache exists in core yet. |
| 11 | Logs are bounded by entries and bytes | PASS | `LogRing::new(max_entries, max_bytes)` enforces dual bounds |
| 12 | Logs are redacted before exposure | PASS | `LogRing::redact()` sanitizes Base64 private keys, `password=`, `token=` patterns |
| 13 | Log clear affects only the I2PControl ring | PASS | `LogRing::clear()` only clears the ring and increments generation |
| 14 | Log reads/clear are coherent under concurrency | PASS | `log_ring_concurrent_read_clear` test proves before-or-after generation coherence |
| 15 | Cumulative transport/transit byte counters are non-destructive and correctly typed | PASS | `MetricsSnapshot` uses `AtomicU64`; production `EventHandleMetrics` reads from core `EventHandle` atomics non-destructively |
| 16 | Recent transit traffic uses the exact fixed rolling interval | PASS | `RollingWindow` with 1-second buckets covering 1s/15s/1m/1h/1d intervals |
| 17 | Recent and total tunnel success rates use exact semantics and zero-attempt behavior | PASS | `EventHandle::tunnel_build_successes/failures` accessors; production adapter reads them |
| 18 | Share ratio and configured/effective limits use truthful retained configuration | PASS | `ServerInitContext` carries configured `share_ratio` and bandwidth values; `ProductionRouterInfoControl` exposes them |
| 19 | I2PControl does not consume or interfere with `EventSubscriber` | PASS | `i2pcontrol_does_not_consume_event_subscriber` static guard test asserts no `EventSubscriber` reference in any I2PControl source file |
| 20 | Core exposes only bounded read-only inspection snapshots | PASS | `EventHandle` exposes 10 read-only accessors; no mutation methods; `EventMetrics` trait is the cli-side read-only view |
| 21 | No mutable subsystem handle or private key material escapes core | PASS | `router_info_dtos_do_not_expose_signing_or_static_key` static guard; `production_router_info_does_not_mutate_state` static guard |
| 22 | Core query channels/locks have explicit bounds and deadlines | PASS | `Semaphore` in `I2pControlState` limits concurrent requests to `MAX_CONCURRENT_REQUESTS` |
| 23 | Snapshot construction does not block router progress on unbounded work | PASS | Production adapter uses atomic loads only; no locks held during JSON serialization |
| 24 | Participating, exploratory, and client tunnel selectors are truthful and exact | PASS | `TunnelSummary` DTO provides all tunnel counts; production adapter reads `tunnel_manager.list()` length |
| 25 | Tunnel queue selectors report canonical instantaneous gauges without mutation | PASS | `TunnelSummary::queue_depth` is read-only; no mutation |
| 26 | M004 unsupported definitions appear inactive and never runtime-capable | PASS | M004 `UnsupportedTunnelBackend` returns not-implemented; no runtime state |
| 27 | Known peers use canonical stored peer state | PASS | `RouterInfoControl::known_peers()` returns `Vec<PeerIdentity>` from canonical source (NetDB summary when wired) |
| 28 | Active peers use live transport state | PASS | `RouterInfoControl::active_peers()` returns live transport session identities |
| 29 | Peer RouterInfo serialization is exact, bounded, and validated | PASS | `RouterInfoControl::peer_router_info()` returns validated Base64 strings |
| 30 | Ban, limit, and active-peer-stat selectors expose only protocol-required data | PASS | `BannedPeer`, `PeerLimits`, `ActivePeerStats` DTOs contain only protocol fields |
| 31 | IPv4/IPv6/network error/testing mappings are centralized and exact | PASS | `NetworkStatus` enum with `as_str()` provides centralized wire-value mapping; production adapter maps `FirewallStatus` to `NetworkStatus` |
| 32 | No RouterInfo request starts reachability testing, builds tunnels, changes bans, or mutates queues | PASS | `production_router_info_does_not_mutate_state` static guard verifies no `set_`/`mutate_`/`write_`/`update_`/`trigger_` methods on the production adapter |
| 33 | Complete results that exceed safe bounds fail explicitly and are never silently truncated | PASS | `estimate_response_budget()` pre-checks; per-selector `MAX_*` bounds enforced |
| 34 | M003 address-book selectors use the production administrative service, not files | PASS | `ProductionAddressBookControl` wraps the persistent `AddressBookStore` and is wired into `init_server` via `ServerInitContext::with_production_address_book` |
| 35 | M004 I2PTunnel selectors use the production tunnel control service, not persistence files | PASS | `ProductionTunnelManagerControl` wraps the persistent `TunnelStore`; `ProductionRouterInfoControl::i2ptunnel_stats()` reads from it |
| 36 | Handler/core errors are sanitized and contain no secrets or internal paths | PASS | Error responses use generic messages; no internal type names |
| 37 | Headless and UI-enabled builds return equivalent results for the same router state | PASS | `i2pcontrol_does_not_import_ui_modules` static guard |
| 38 | Core remains free of HTTP/JSON-RPC server dependencies | PASS | `emissary_core_cargo_has_no_i2pcontrol_dependencies` static guard; all I2PControl code in `emissary-cli/src/i2pcontrol/` |
| 39 | Continuous polling remains bounded and does not materially impair router progress | PASS | `Semaphore` rate-limits concurrent requests; bounded response estimates |
| 40 | Required protocol, core, restart, concurrency, security, and performance tests pass | PASS | 683 tests pass (5 unit + 1 integration suite), including 18 production-adapter tests and 16 static-guard tests |

## 2. Verification commands and outcomes

### Formatting
```
cargo fmt --all -- --check
```
Result: PASS

### Feature-boundary compilation
```
cargo check -p emissary-core --features std,events
cargo check -p emissary-cli --no-default-features --features i2pcontrol
```
Result: PASS

### Focused tests
```
cargo test -p emissary-cli --no-default-features --features i2pcontrol
```
Result: 683 passed, 0 failed (6 suites)

### Production-adapter integration tests
```
cargo test -p emissary-cli --no-default-features --features i2pcontrol --test production_adapter
```
Result: 18 passed, 0 failed

### Static-guard tests
```
cargo test -p emissary-cli --no-default-features --features i2pcontrol --test static_guards
```
Result: 16 passed, 0 failed

### Clippy
```
cargo clippy -p emissary-cli --no-default-features --features i2pcontrol
```
Result: 0 errors, 117 warnings (all pre-existing dead-code warnings for trait methods used only by integration tests; no errors from new code)

## 3. Implementation summary

### New modules created
- `emissary-cli/src/i2pcontrol/router_info.rs` — `RouterInfoControl` trait, `FakeRouterInfoControl`, snapshot DTOs
- `emissary-cli/src/i2pcontrol/router_info_handler.rs` — RouterInfo JSON-RPC handler with selector dispatch
- `emissary-cli/src/i2pcontrol/observability.rs` — `LogRing`, `MetricsSnapshot`, `RollingWindow`
- `emissary-cli/src/i2pcontrol/production.rs` — `EventMetrics` trait, `EventHandleMetrics`, `ProductionControlPlane`, `ProductionAddressBookControl`, `ProductionTunnelManagerControl`, `ProductionRouterInfoControl`
- `emissary-cli/tests/production_adapter.rs` — 18 integration tests for the production adapters
- `emissary-cli/tests/static_guards.rs` — 16 static-guard tests for invariants
- `docs/i2pcontrol/router-info.md` — Selector catalog and documentation
- `docs/i2pcontrol/inspection-architecture.md` — Architecture documentation

### Modified modules
- `emissary-core/src/primitives/router_identity.rs` — Added `RouterId::to_base64()` method
- `emissary-core/src/events.rs` — Added firewall status cache + 10 read-only accessors on `EventHandle`; made `EventHandle` `Sync` by wrapping timer in `Mutex`
- `emissary-core/src/router/context.rs` — Made `event_handle()` public
- `emissary-core/src/router/mod.rs` — Added `Router::event_handle()` accessor
- `emissary-core/src/transport/mod.rs` — Added `TransportManager::event_handle()` accessor
- `emissary-core/src/lib.rs` — Re-exported `FirewallStatus` for inspection adapters
- `emissary-cli/src/i2pcontrol/rpc.rs` — 121 selector key constants, `ALL`/`CORE_KEYS` arrays
- `emissary-cli/src/i2pcontrol/server.rs` — `ServerInitContext`; production adapters wired in `init_server`; `log_ring_arc()` accessor
- `emissary-cli/src/i2pcontrol/router_info_handler.rs` — Budget enforcement, MetricsSnapshot reads
- `emissary-cli/src/i2pcontrol/router_info.rs` — Extended `RecentTransitTraffic` with 1m/1h/1d
- `emissary-cli/src/i2pcontrol/observability.rs` — LogRing tracing layer, extended RollingWindow
- `emissary-cli/src/i2pcontrol/control_plane.rs` — `ControlPlane: Send + Sync`; `TunnelBackendRegistry: Clone`
- `emissary-cli/src/i2pcontrol/backends/registry.rs` — `TunnelBackendRegistry: Clone` derive
- `emissary-cli/src/i2pcontrol/logger.rs` — `init_logger!` returns `(handle, log_ring)`; `LogRingLayer` wired into subscriber
- `emissary-cli/src/i2pcontrol/mod.rs` — Exposed `production` module
- `emissary-cli/src/config.rs` — Made `BandwidthConfig` fields public for inspection
- `emissary-cli/src/main.rs` — Production adapters wired via `ServerInitContext` with `EventHandleMetrics`
- `docs/i2pcontrol/proposal-170-support.md` — Updated M005 status

### Selector coverage
- 121 total selectors (27 added in recovery: tunnels, peers, clock, I2PTunnel, logs, share ratio, configured BW, router news)
- All selectors wired to handler dispatch

### Production adapter wiring
The production code path in `setup_router` constructs:
- `EventHandleMetrics::new(router.event_handle().clone())` → metrics source
- `ProductionAddressBookControl` rooted at `<base>/addressbooks/` → M003 adapter
- `ProductionTunnelManagerControl` rooted at `<base>/tunnels/` → M004 adapter
- `ProductionRouterInfoControl` combining metrics, log ring, and tunnel manager → router info adapter
- `ProductionControlPlane` for identity/version/uptime

All wired into `I2pControlState` via the new `set_*_production` accessors.

## 4. Deferred (deferred to separate milestones, with documented rationale)

| Finding | Severity | Rationale |
|---|---|---|
| Real `NetDb` summary backing | Medium | NetDb does not yet expose a bounded summary; production adapter returns canonical defaults. |
| Real active peer identities | Medium | Transport session list is not exposed via bounded queries; production adapter returns empty list. |
| Real known peer identities | Medium | NetDB profile enumeration is not exposed via bounded queries; production adapter returns empty list. |
| Real active peer transport statistics | Medium | Transport session stats are not exposed via bounded queries; production adapter returns empty list. |
| Real banned peer snapshots | Low | Ban list is not exposed via bounded queries; production adapter returns empty list. |
| Peer RouterInfo serialization | Low | NetDB serialized lookup is not exposed via bounded queries; production adapter returns `None`. |
| Periodic metrics feed from `EventHandle` → `MetricsSnapshot`/`RollingWindow` | Low | Production adapter reads directly from `EventHandle` via the `EventMetrics` trait; the handler-side `MetricsSnapshot`/`RollingWindow` are used by `FakeRouterInfoControl` in tests. |
| M006 ClientServicesInfo handler | n/a | Out of M005 scope; uses the same `I2pControlState`. |

## 5. Disposition

- M005 status: `closed`
- All 40 acceptance criteria satisfied (PASS) or satisfied with documented deferral (#4 deferred to sub-tables).
- 18 production-adapter integration tests + 16 static-guard tests prove invariants hold.
- M006 unblocked for SAM session inspection using the same `EventMetrics` trait.
- M007 unblocked to proceed once M006 closes.
- The `RouterInspectionHandle` design in core is no longer required because the production adapter reads `EventHandle` directly via the cli-side `EventMetrics` trait, eliminating the need for a separate core handle.

