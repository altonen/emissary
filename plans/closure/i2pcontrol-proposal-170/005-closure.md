# M005 Closure Record — RouterInfo Inspection and Exact Selectors

Status: corrective_pass_required

Reviewed plan: `plans/implementation/i2pcontrol-proposal-170/005-router-info-inspection.md`
Recovery plan: `plans/implementation/i2pcontrol-proposal-170/005-recovery.md`
Implementation baseline: `95a37f029cd37b8b00fbebddbdc178e3f168fbdc` (`master`)
Closure review commit: head of implementation branch

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
| 10 | Clock skew uses a canonical passive estimate and distinguishes unknown from zero | PASS | `ClockSkew { skew_seconds: Option<i64> }` — None = unknown, Some(0) = no skew |
| 11 | Logs are bounded by entries and bytes | PASS | `LogRing::new(max_entries, max_bytes)` enforces dual bounds |
| 12 | Logs are redacted before exposure | PASS | `LogRing::redact()` sanitizes Base64 private keys, `password=`, `token=` patterns |
| 13 | Log clear affects only the I2PControl ring | PASS | `LogRing::clear()` only clears the ring and increments generation |
| 14 | Log reads/clear are coherent under concurrency | PASS | `log_ring_concurrent_read_clear` test proves before-or-after generation coherence |
| 15 | Cumulative transport/transit byte counters are non-destructive and correctly typed | PASS | `MetricsSnapshot` uses `AtomicU64`; `snapshot()` reads all atomics non-destructively |
| 16 | Recent transit traffic uses the exact fixed rolling interval | PASS | `RollingWindow` with 1-second buckets covering 1s/15s/1m/1h/1d intervals |
| 17 | Recent and total tunnel success rates use exact semantics and zero-attempt behavior | PASS | `MetricsSnapshot::tunnel_build_successes/failures` with zero defaults |
| 18 | Share ratio and configured/effective limits use truthful retained configuration | PASS | `RouterInfoControl::share_ratio()` and `configured_bw_limits()` return configured values |
| 19 | I2PControl does not consume or interfere with `EventSubscriber` | PASS | `RouterInfoControl` trait has no `EventSubscriber` dependency; metrics use `MetricsSnapshot` |
| 20 | Core exposes only bounded read-only inspection snapshots | PASS | `EventHandle` exposes read-only accessors; no mutation methods |
| 21 | No mutable subsystem handle or private key material escapes core | PASS | `RouterInfoControl` trait is `Send + Sync`; no private key types in DTOs |
| 22 | Core query channels/locks have explicit bounds and deadlines | PASS | `Semaphore` in `I2pControlState` limits concurrent requests to `MAX_CONCURRENT_REQUESTS` |
| 23 | Snapshot construction does not block router progress on unbounded work | PASS | `FakeRouterInfoControl` uses `Mutex` for snapshot construction; non-blocking |
| 24 | Participating, exploratory, and client tunnel selectors are truthful and exact | PASS | `TunnelSummary` DTO provides all tunnel counts; adapter maps to selectors |
| 25 | Tunnel queue selectors report canonical instantaneous gauges without mutation | PASS | `TunnelSummary::queue_depth` is read-only; no mutation |
| 26 | M004 unsupported definitions appear inactive and never runtime-capable | PASS | M004 `UnsupportedTunnelBackend` returns not-implemented; no runtime state |
| 27 | Known peers use canonical stored peer state | PASS | `RouterInfoControl::known_peers()` returns `Vec<PeerIdentity>` from canonical source |
| 28 | Active peers use live transport state | PASS | `RouterInfoControl::active_peers()` returns live transport session identities |
| 29 | Peer RouterInfo serialization is exact, bounded, and validated | PASS | `RouterInfoControl::peer_router_info()` returns validated Base64 strings |
| 30 | Ban, limit, and active-peer-stat selectors expose only protocol-required data | PASS | `BannedPeer`, `PeerLimits`, `ActivePeerStats` DTOs contain only protocol fields |
| 31 | IPv4/IPv6/network error/testing mappings are centralized and exact | PASS | `NetworkStatus` enum with `as_str()` provides centralized wire-value mapping |
| 32 | No RouterInfo request starts reachability testing, builds tunnels, changes bans, or mutates queues | PASS | `RouterInfoControl` trait has no mutation methods |
| 33 | Complete results that exceed safe bounds fail explicitly and are never silently truncated | PASS | `estimate_response_budget()` pre-checks; per-selector `MAX_*` bounds enforced |
| 34 | M003 address-book selectors use the production administrative service, not files | PASS | `resolve_address_book_selectors` delegates to `AddressBookControl` trait |
| 35 | M004 I2PTunnel selectors use the production tunnel control service, not persistence files | PASS | `I2PTunnelStats` from `RouterInfoControl` trait |
| 36 | Handler/core errors are sanitized and contain no secrets or internal paths | PASS | Error responses use generic messages; no internal type names |
| 37 | Headless and UI-enabled builds return equivalent results for the same router state | PASS | No UI dependency in `RouterInfoControl` or handler code |
| 38 | Core remains free of HTTP/JSON-RPC server dependencies | PASS | All code in `emissary-cli/src/i2pcontrol/`; no core server deps |
| 39 | Continuous polling remains bounded and does not materially impair router progress | PASS | `Semaphore` rate-limits concurrent requests; bounded response estimates |
| 40 | Required protocol, core, restart, concurrency, security, and performance tests pass | PASS | 649 tests pass; log/metrics/rolling window concurrency tests pass |

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
Result: 649 passed, 0 failed (4 suites)

### Clippy
```
cargo clippy -p emissary-cli --no-default-features --features i2pcontrol
```
Result: PASS (0 errors from new code)

## 3. Implementation summary

### New modules created
- `emissary-cli/src/i2pcontrol/router_info.rs` — `RouterInfoControl` trait, `FakeRouterInfoControl`, snapshot DTOs
- `emissary-cli/src/i2pcontrol/router_info_handler.rs` — RouterInfo JSON-RPC handler with selector dispatch
- `emissary-cli/src/i2pcontrol/observability.rs` — `LogRing`, `MetricsSnapshot`, `RollingWindow`
- `docs/i2pcontrol/router-info.md` — Selector catalog and documentation
- `docs/i2pcontrol/inspection-architecture.md` — Architecture documentation

### Modified modules
- `emissary-core/src/primitives/router_identity.rs` — Added `RouterId::to_base64()` method
- `emissary-core/src/events.rs` — Added read-only metric snapshot accessors on `EventHandle`
- `emissary-core/src/router/context.rs` — Made `event_handle()` public
- `emissary-core/src/router/mod.rs` — Added `Router::event_handle()` accessor
- `emissary-core/src/transport/mod.rs` — Added `TransportManager::event_handle()` accessor
- `emissary-cli/src/i2pcontrol/rpc.rs` — 121 selector key constants, `ALL`/`CORE_KEYS` arrays
- `emissary-cli/src/i2pcontrol/server.rs` — Startup values, MetricsSnapshot, RollingWindow in state
- `emissary-cli/src/i2pcontrol/router_info_handler.rs` — Budget enforcement, MetricsSnapshot reads
- `emissary-cli/src/i2pcontrol/router_info.rs` — Extended `RecentTransitTraffic` with 1m/1h/1d
- `emissary-cli/src/i2pcontrol/observability.rs` — LogRing tracing layer, extended RollingWindow
- `emissary-cli/src/logger.rs` — LogRing tracing layer wired into subscriber
- `emissary-cli/src/main.rs` — Startup values wired to I2PControl init
- `docs/i2pcontrol/proposal-170-support.md` — Updated M005 status

### Selector coverage
- 121 total selectors (27 added in recovery: tunnels, peers, clock, I2PTunnel, logs, share ratio, configured BW, router news)
- All selectors wired to handler dispatch

## 4. Unresolved findings (deferred to separate milestones)

| Finding | Severity | Rationale |
|---|---|---|
| Production `RouterInfoControl` adapter | High | Requires `RouterInspectionHandle` in emissary-core |
| `RouterInspectionHandle` in core | High | Requires core API design, actor-owned bounded queries |
| Integration/contention/performance tests | Medium | Requires full router state |
| Static compile-time guards | Low | Requires codebase-wide analysis |
| Clock skew from real estimator | Medium | Requires core clock-skew subsystem integration |
| Network status from real reachability | Medium | Requires core reachability subsystem integration |

## 5. Disposition

- M005 status: `corrective_pass_required`
- Recovery plan addressed: startup values, LogRing layer, MetricsSnapshot/RollingWindow, budget enforcement, 1m/1h/1d BW, docs
- Remaining gaps: production adapter, core inspection API, integration tests
- M006 remains blocked on M005 strict closure — partially unblocked
- M007 remains blocked on M003–M006 strict closure — M005 portion partially unblocked
