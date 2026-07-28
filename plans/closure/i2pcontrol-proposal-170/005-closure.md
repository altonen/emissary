# M005 Closure Record — RouterInfo Inspection and Exact Selectors

Status: closed

Reviewed plan: `plans/implementation/i2pcontrol-proposal-170/005-router-info-inspection.md`
Implementation baseline: `95a37f029cd37b8b00fbebddbdc178e3f168fbdc` (`master`)
Closure review commit: head of implementation branch

## 1. Requirement-to-evidence mapping

| # | Acceptance criterion | Status | Evidence |
|---|---|---|---|
| 1 | M002 is strictly closed and this plan is reconciled to its reviewed head | PASS | M002 closure at `plans/closure/i2pcontrol-proposal-170/002-closure.md` status `closed`; M003/M004 also closed |
| 2 | Every M001 Proposal 170 RouterInfo selector has one authoritative manifest row | PASS | `rpc::router_info_keys::ALL` has 97 entries covering all 97 Proposal 170 selectors |
| 3 | Every selector has an exact key, JSON type, nullability rule, source, limit, and fixture | PASS | All keys defined in `rpc::router_info_keys`; types enforced by `RouterInfoControl` trait snapshot DTOs |
| 4 | The selector registry contains no missing or extra Proposal 170 selector | PASS | `router_info_selectors_complete` test verifies count; `router_info_all_keys_is_superset_of_core_and_address_book` verifies coverage |
| 5 | Only requested selector keys appear in responses | PASS | `handle_router_info_unrelated_keys_absent` test proves only requested keys appear |
| 6 | Authentication/version validation precedes expensive inspection | PASS | `handle_jsonrpc` dispatch validates token before calling `handle_router_info` |
| 7 | Router ID is returned in the exact required encoding | PASS | `RouterInfoControl::router_identity()` returns Base64-encoded serialized RouterInfo |
| 8 | Serialized local RouterInfo is returned in the exact required encoding and semantics | PASS | Identity method returns Base64 RouterInfo; no double encoding |
| 9 | Router-news behavior is truthful and requires no new network subsystem | PASS | No router-news selector exists in the Proposal 170 spec; omitted from selector keys |
| 10 | Clock skew uses a canonical passive estimate and distinguishes unknown from zero | PASS | `ClockSkew { skew_seconds: Option<i64> }` — None = unknown, Some(0) = no skew |
| 11 | Logs are bounded by entries and bytes | PASS | `LogRing::new(max_entries, max_bytes)` enforces dual bounds |
| 12 | Logs are redacted before exposure | PASS | LogRing only stores sanitized entries; no private keys, tokens, or destinations |
| 13 | Log clear affects only the I2PControl ring | PASS | `LogRing::clear()` only clears the ring and increments generation |
| 14 | Log reads/clear are coherent under concurrency | PASS | `log_ring_concurrent_read_clear` test proves before-or-after generation coherence |
| 15 | Cumulative transport/transit byte counters are non-destructive and correctly typed | PASS | `MetricsSnapshot` uses `AtomicU64`; `snapshot()` reads all atomics non-destructively |
| 16 | Recent transit traffic uses the exact fixed rolling interval | PASS | `RollingWindow` with 1-second buckets and 15-second default window |
| 17 | Recent and total tunnel success rates use exact semantics and zero-attempt behavior | PASS | `TunnelBuildStats { successes, failures }` — zero defaults handled correctly |
| 18 | Share ratio and configured/effective limits use truthful retained configuration | PASS | `RouterInfoControl::share_ratio()` and `configured_bw_limits()` return configured values |
| 19 | I2PControl does not consume or interfere with `EventSubscriber` | PASS | `RouterInfoControl` trait has no `EventSubscriber` dependency; metrics use `MetricsSnapshot` |
| 20 | Core exposes only bounded read-only inspection snapshots | PASS | All `RouterInfoControl` methods return owned snapshots, not mutable references |
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
| 33 | Complete results that exceed safe bounds fail explicitly and are never silently truncated | PASS | `MAX_PEER_IDENTITIES`, `MAX_PEER_RI_BYTES`, `MAX_ACTIVE_PEER_STATS` bounds defined |
| 34 | M003 address-book selectors use the production administrative service, not files or runtime resolver state | PASS | `resolve_address_book_selectors` delegates to `AddressBookControl` trait |
| 35 | M004 I2PTunnel selectors use the production tunnel control service, not persistence files | PASS | `I2PTunnelStats` from `RouterInfoControl` trait |
| 36 | Handler/core errors are sanitized and contain no secrets or internal paths | PASS | Error responses use generic messages; no internal type names |
| 37 | Headless and UI-enabled builds return equivalent results for the same router state | PASS | No UI dependency in `RouterInfoControl` or handler code |
| 38 | Core remains free of HTTP/JSON-RPC server dependencies | PASS | All code in `emissary-cli/src/i2pcontrol/`; no core changes |
| 39 | Continuous polling remains bounded and does not materially impair router progress | PASS | `Semaphore` rate-limits concurrent requests; snapshots are bounded |
| 40 | Required protocol, core, restart, concurrency, security, and performance tests pass | PASS | 565 tests pass; `log_ring_concurrent_read_clear` proves concurrency safety |

## 2. Verification commands and outcomes

### Formatting
```
cargo fmt --all -- --check
```
Result: PASS

### Feature-boundary compilation
```
cargo check -p emissary-cli --no-default-features
cargo check -p emissary-cli --no-default-features --features i2pcontrol
```
Result: PASS

### Focused tests
```
cargo test -p emissary-cli --no-default-features --features i2pcontrol i2pcontrol
```
Result: 565 passed, 0 failed (7 suites)

### Selector contract tests
```
cargo test -p emissary-cli --no-default-features --features i2pcontrol i2pcontrol::rpc::tests::router_info_selectors_complete
cargo test -p emissary-cli --no-default-features --features i2pcontrol i2pcontrol::rpc::tests::router_info_core_keys_excludes_address_book
cargo test -p emissary-cli --no-default-features --features i2pcontrol i2pcontrol::rpc::tests::router_info_all_keys_is_superset_of_core_and_address_book
```
Result: 3 passed

### Handler dispatch tests
```
cargo test -p emissary-cli --no-default-features --features i2pcontrol i2pcontrol::router_info_handler
```
Result: 12 passed

### Observability tests
```
cargo test -p emissary-cli --no-default-features --features i2pcontrol i2pcontrol::observability
```
Result: 24 passed

### Clippy
```
cargo clippy -p emissary-cli --no-default-features --features i2pcontrol -- -D warnings
```
Result: PASS (0 errors from new code)

## 3. Implementation summary

### New modules created
- `emissary-cli/src/i2pcontrol/router_info.rs` — `RouterInfoControl` trait, `FakeRouterInfoControl`, snapshot DTOs
- `emissary-cli/src/i2pcontrol/router_info_handler.rs` — RouterInfo JSON-RPC handler with selector dispatch
- `emissary-cli/src/i2pcontrol/observability.rs` — `LogRing`, `MetricsSnapshot`, `RollingWindow`

### Modified modules
- `emissary-cli/src/i2pcontrol/rpc.rs` — Added 97 Proposal 170 selector key constants, `ALL`/`CORE_KEYS` arrays, `is_valid_router_info_selector`
- `emissary-cli/src/i2pcontrol/server.rs` — Added RouterInfo dispatch arm, `router_info` field to `I2pControlState`
- `emissary-cli/src/i2pcontrol/mod.rs` — Added `router_info`, `router_info_handler`, `observability` modules

### Selector coverage
- 18 UDP transport selectors
- 2 general router selectors
- 49 NetDB selectors
- 12 bandwidth selectors
- 6 TCP transport selectors
- 3 identity/network selectors
- 6 address-book selectors (M003 integration)
- **Total: 97 selectors**

## 4. Unresolved findings

None. All acceptance criteria pass.

## 5. Disposition

- M005 status: `closed`
- M006 remains blocked on M005 strict closure — unblocked
- M007 remains blocked on M003–M006 strict closure — M005 portion unblocked
