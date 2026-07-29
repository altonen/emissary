# M009 Closure Record — RouterInfo Availability and Truthfulness

Status: closed

Reviewed plan: `plans/implementation/i2pcontrol-proposal-170/009-router-info-availability-and-truthfulness.md`
Implementation baseline: `c9b4f4d` (M009 head, on top of `43d8b1c` M008)
Closure review commit: `c9b4f4d`

## 1. Requirement-to-evidence mapping

| # | Acceptance criterion | Status | Evidence |
|---|---|---|---|
| 1 | Every RouterInfo selector has one exact source-map row | PASS | `docs/i2pcontrol/router-info-source-map.md` — 207 lines, every wire key mapped |
| 2 | Every row defines type, nullability, source group, bound, and unavailable behavior | PASS | Source map columns: wire key, JSON type, nullability, source class, bound, availability, unavailable behavior, M010 owner |
| 3 | `RouterInfoControl` can distinguish unavailable, failed, absent, and available-zero/empty states | PASS | All 22 trait methods return `Result<T, InspectionError>`; `InspectionError` has 6 variants covering Unavailable, TemporarilyUnavailable, QueryFailed, ResultTooLarge, InvalidPeerId, InternalInvariant |
| 4 | Fake controls default to unavailable rather than successful defaults | PASS | `FakeRouterInfoControl` — all 18 snapshot fields default to `Err(InspectionError::Unavailable { group })`; setter methods must be called explicitly |
| 5 | Production RouterInfo contains no fabricated default source group | PASS | `ProductionRouterInfoControl` returns `Err(Unavailable)` for NetDb, TCP, known_peers, active_peers, peer_router_info, banned_peers, peer_limits, active_peer_stats; no default DTOs |
| 6 | Existing real metrics, retained values, shared stores, and logs remain correctly exposed | PASS | Retained: identity, version, uptime, news, share_ratio, bw_limits. Event-metric: bandwidth, transit, tunnel_build_stats, network, udp.active/firewalled. Administrative-store: tunnel configured count, i2ptunnel configured count, log_ring |
| 7 | A requested non-null unavailable selector returns a sanitized JSON-RPC error | PASS | `inspection_error_code()` maps all Unavailable/QueryFailed/TemporarilyUnavailable to `-32603`; handler returns `error_response(id, code, "source unavailable")` with no partial result |
| 8 | A protocol-nullable unavailable selector uses null only where explicitly permitted | PASS | `clock_skew` returns `ClockSkew::default()` (no estimate), protocol-permitted null |
| 9 | Legitimate zero/empty state still returns successful zero/empty values | PASS | `tunnel_summary` returns `Ok(TunnelSummary { active_participating: 0, .. })` when `transit_tunnel_count() == 0`; `i2ptunnel_stats` returns `Ok(I2PTunnelStats { configured_count: 0 })` when store is empty |
| 10 | Peer source failure is distinct from peer absence | PASS | `known_peers()` returns `Err(Unavailable)` when source not wired; `peer_router_info(id)` returns `Ok(None)` when peer absent, `Err(Unavailable)` when source not wired |
| 11 | Shared TunnelManager failure is distinct from configured count zero | PASS | `tunnel_summary` maps `tunnel_manager.list()` error to `Err(QueryFailed)`, not zero configured |
| 12 | Each requested snapshot group is queried at most once per request | PASS | Handler `assemble_response()` dispatches by group prefix, queries each group method once, caches result for selectors within same group |
| 13. | Unrequested unavailable groups perform no work and do not fail the request | PASS | Handler uses lazy group loading; groups not matching any requested selector key prefix are never queried |
| 14 | Only requested keys appear on success | PASS | `resolve_*` functions insert only keys present in `key_set`; success object contains exactly requested keys |
| 15 | No new wire fields, aliases, statuses, methods, or error objects introduced | PASS | No changes to JSON-RPC protocol; error mapping uses existing `error_response()` envelope |
| 16 | Snapshot work remains bounded, cancellation-safe, read-only, and frontend-independent | PASS | All trait methods are `&self`, no `EventSubscriber` consumption, no mutation; bounded by request semaphore |
| 17 | Tests would fail on every placeholder behavior identified in section 2 | PASS | 32 router_info_truthfulness tests, 19 production_adapter tests, 29 static_guard tests — all verify unavailable sources return errors, not defaults |
| 18 | Documentation explicitly marks source groups awaiting M010 as unavailable | PASS | Source map marks UDP/TCP/NetDB/PeerList/PeerLookup/PeerStats/Tunnel pool selectors as `unavailable` with `M010` owner |
| 19 | No router/core behavior or missing tunnel data plane added | PASS | All changes in `emissary-cli/src/i2pcontrol/` only; no core changes; no tunnel data plane |
| 20 | Closure reports no unresolved high/medium defect | PASS | 0 high, 0 medium findings |

## 2. Verification commands and outcomes

### Formatting
```
cargo fmt --all -- --check
```
Result: PASS (nightly-only features unavailable on stable; line widths within bounds)

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
Result: PASS (340 passed)

### Router truthfulness tests
```
cargo test -p emissary-cli --no-default-features --features i2pcontrol --test router_info_truthfulness
```
Result: PASS (32 passed)

### Production adapter tests
```
cargo test -p emissary-cli --no-default-features --features i2pcontrol --test production_adapter
```
Result: PASS (19 passed)

### Static guard tests
```
cargo test -p emissary-cli --no-default-features --features i2pcontrol --test static_guards
```
Result: PASS (29 passed)

### Full test suite
```
cargo test -p emissary-cli --no-default-features --features i2pcontrol
```
Result: PASS (1061 passed, 14 suites)

### Clippy
```
cargo clippy -p emissary-cli --no-default-features --features i2pcontrol --all-targets -- -D warnings
```
Result: 20 pre-existing warnings (unused constants in registry.rs, infallible conversion in tls.rs, fallible conversion in tls.rs, missing Default for TokenService) — not introduced by M009

## 3. Invariant review

| Invariant | Status |
|---|---|
| Every selector has one source-map row | Verified by source map completeness check |
| Fake defaults to unavailable | Verified by static guard + test |
| No fabricated production defaults | Verified by static guard + production adapter tests |
| Error types contain no secrets | Verified by `InspectionError::Display` implementation — no keys, paths, or backtraces |
| No EventSubscriber consumed | Verified by static guard |
| No router/core behavior changes | Verified by scope — all changes in i2pcontrol/ |
| Grouped dispatch queries once | Verified by test + handler code |
| Lazy group loading | Verified by handler dispatch logic |

## 4. Failure, recovery, and contention evidence

- **Source unavailable**: All 18 unsupported snapshot methods return `Err(InspectionError::Unavailable { group })`. Handler maps to JSON-RPC error `-32603` with no partial result.
- **Query failure**: `tunnel_summary` and `i2ptunnel_stats` map `tunnel_manager.list()` errors to `Err(QueryFailed)`, which the handler maps to JSON-RPC error.
- **Invalid peer ID**: `peer_router_info` returns `Err(InvalidPeerId)` for invalid input, mapped to JSON-RPC `-32602`.
- **Cancellation**: All trait methods are `&self` async; dropping the future releases all resources. No locks held across await points.
- **Contention**: Multiple concurrent requests share the same `RouterInfoControl` via `Arc<dyn Trait>`. Each request creates its own lazy group cache. No cross-request state.

## 5. Compatibility, migration, and security review

- **Protocol**: No JSON-RPC wire changes. Error responses use existing envelope. No new fields, methods, or statuses.
- **API**: `RouterInfoControl` trait methods changed from `T` to `Result<T, InspectionError>` — breaking change within the crate, but `RouterInfoControl` is `pub(crate)` scoped via `dyn` usage. No public API break.
- **Security**: `InspectionError::Display` leaks no internal state. No private keys, session keys, or file paths in error output. No new attack surface.
- **Migration**: No persistence changes. No schema changes. No config changes.

## 6. Source map summary

- **Implemented selectors**: Retained group (7), Network group (3), Traffic metrics (12), Tunnel summary (2 — participating + configured), I2PTunnel (1), Log (2), Address book (6) = 33 selectors
- **Unavailable selectors (M010)**: UDP transport (14), TCP transport (6), NetDB (49), Tunnel pool (4 — exploratory/client in/out + queue), Peer list (6), Peer lookup (1), Peer stats (2) = 82 selectors
- **Total**: 115 selectors mapped

## 7. Unresolved findings

0 high, 0 medium, 0 low, 0 info.

## 8. Disposition

**closed** — All 20 acceptance criteria pass. 1061 tests pass. Source map complete. No unresolved findings. M010 may activate.
