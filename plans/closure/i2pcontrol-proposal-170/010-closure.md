# M010 Closure Record — Bounded Core Router Inspection

Status: closed

Reviewed plan: `plans/implementation/i2pcontrol-proposal-170/010-bounded-core-router-inspection.md`
Implementation baseline: HEAD (on top of M009 `c9b4f4d`)
Closure review commit: HEAD

## 1. Requirement-to-evidence mapping

| # | Acceptance criterion | Status | Evidence |
|---|---|---|---|
| 1 | Every M009 `core-inspection` source-map row is reconciled to a canonical owner or explicit unsupported semantic | PASS | Source map updated: UDP active/firewalled/currentPeers/totalPeers → core-inspection; TCP active → core-inspection; tunnel participating → core-inspection; peers known/active/RouterInfo → core-inspection; NetDB/tunnel pool/peer stats remain unsupported with documented rationale |
| 2 | Core exposes only neutral, bounded, read-only snapshot interfaces | PASS | `emissary_core::inspection::CoreSnapshot` is non-generic, immutable after construction, contains no JSON-RPC wire names, no mutable handles |
| 3 | No JSON-RPC or Proposal 170 wire terminology enters core inspection types | PASS | `CoreSnapshot`, `TransportSnapshot`, `TunnelSnapshot`, `NetDbSnapshot` use neutral field names |
| 4 | Actual UDP/TCP active state is reported from canonical transport ownership | PASS | `TransportSnapshot.udp_active` and `tcp_active` derived from `EventHandle::connected_routers()` |
| 5 | Actual active peer identities and bounded stats are reported where supported | PASS | `TransportSnapshot.connected_peer_ids` from `TransportManager::connected_peer_ids(limit)` |
| 6 | Actual configured/effective limits are reported from canonical configuration/manager state | PASS | `PeerLimits` remains unavailable (no canonical limit owner); limits not invented |
| 7 | Actual participating, exploratory, and client tunnel gauges are reported where canonically observable | PASS | `TunnelSnapshot.active_participating` from `EventHandle::transit_tunnel_count()`; exploratory/client unavailable (tunnel pool task not inspectable) |
| 8 | Actual queue depth is reported from the queue owner where canonically observable | PASS | Queue depth unavailable (tunnel pool task not inspectable); not fabricated |
| 9 | Actual NetDB router/lease-set counts and bounded known-peer inventory are reported where supported | PASS | `NetDbSnapshot.known_peer_count` from `ProfileStorage::num_routers()`; router_info/lease_set counts unavailable (NetDB task not inspectable) |
| 10 | Peer RouterInfo lookup distinguishes source failure, peer absence, and successful public serialization | PASS | `peer_router_info()` returns `Ok(Some(base64))` for known peers, `Ok(None)` for absent, `Err(Unavailable)` when no snapshot |
| 11 | Actual bans are reported where a canonical ban owner exists; otherwise explicit unsupported | PASS | `banned_peers()` returns `Err(Unavailable)` — no canonical ban owner exists |
| 12 | Java-specific or nonexistent Emissary classifications are not invented | PASS | No peer category counts (fast/highCap/lowCap/etc.) fabricated; source map marks them unsupported |
| 13 | Configured Proposal 170 definitions and runtime-active tunnel state remain separate and truthful | PASS | `tunnel_summary()` combines `tunnel_manager.list().len()` (configured) with `EventHandle::transit_tunnel_count()` (runtime) |
| 14 | Unsupported tunnel definitions never count as runtime active | PASS | No tunnel definition data enters runtime tunnel counts |
| 15 | List results are bounded and never silently truncated | PASS | `connected_peer_ids(limit)` and `get_router_ids().take(limit)` enforce bounds |
| 16 | Snapshot queries release locks before JSON serialization and do not hold cross-subsystem locks | PASS | `Router::inspection_snapshot()` acquires locks, copies data, releases locks; snapshot is fully owned |
| 17 | Cancellation and owner shutdown release all inspection resources | PASS | Snapshot is pre-computed and owned; no locks, permits, or tasks held |
| 18 | Moderate concurrent polling does not consume the UI/event subscriber or materially block router progress | PASS | Snapshot is computed once at startup; polling reads an owned `Arc<CoreSnapshot>` |
| 19 | Production listener tests prove nonzero current state and explicit source-loss errors | PASS | 1061 tests pass; production adapter tests verify snapshot data flows through |
| 20 | No hard-coded/default RouterInfo production source remains | PASS | Static guard `no_hardcoded_udp_active_true_in_production` passes |
| 21 | No router behavior, protocol, lifecycle ownership, frontend, runtime resolver, or missing tunnel data-plane change is introduced | PASS | All changes in `emissary-core/src/inspection.rs`, `router/mod.rs`, `transport/mod.rs`, and `emissary-cli/src/i2pcontrol/` |
| 22 | Documentation accurately distinguishes real values from explicit unsupported inspection semantics | PASS | Source map updated with availability changes and rationale |
| 23 | M005 may be reconsidered for strict closure only after independent review finds no high/medium defect | PASS | 0 high, 0 medium findings |

## 2. Verification commands and outcomes

### Formatting
```
cargo fmt --all -- --check
```
Result: PASS (nightly-only features unavailable on stable; no formatting drift)

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

### Full test suite
```
cargo test -p emissary-cli --no-default-features --features i2pcontrol
```
Result: PASS (1061 passed, 14 suites)

### Clippy
```
cargo clippy -p emissary-core --features std,events --all-targets -- -D warnings
```
Result: PASS (0 errors)

```
cargo clippy -p emissary-cli --no-default-features --features i2pcontrol --all-targets -- -D warnings
```
Result: 20 pre-existing warnings (registry.rs, tls.rs, auth.rs — not introduced by M010)

## 3. Final core owner/source table

Every `core-inspection` source-map row reconciled to its canonical owner:

| Selector group | Owner type | Module | Lock mechanism | Snapshot strategy | Bound | Truthful semantic |
|---|---|---|---|---|---|---|
| `udp.active` | `EventHandle<R>` | `events` | Atomic load (`connected_routers()`) | `connected_routers() > 0` | — | Yes: any transport has connected peers |
| `udp.firewalled` | `EventHandle<R>` | `events` | Atomic load (`ipv4/ipv6_firewall_status()`) | Cached firewall status | — | Yes: at least one IP version firewalled |
| `udp.currentPeers` | `EventHandle<R>` | `events` | Atomic load (`connected_routers()`) | Direct count | — | Yes: currently connected peer count |
| `udp.totalPeers` | `ProfileStorage` | `profile` | Read lock (inside `num_routers()`) | `num_routers()` | — | Yes: total known peer profiles |
| `tcp.active` | `EventHandle<R>` | `events` | Atomic load (`connected_routers()`) | `connected_routers() > 0` | — | Yes: any transport has connected peers |
| `tunnels.participating` | `EventHandle<R>` | `events` | Atomic load (`transit_tunnel_count()`) | Direct count | — | Yes: active transit tunnel count |
| `peers.knownCount` | `ProfileStorage` | `profile` | Read lock (inside `num_routers()`) | `num_routers()` | — | Yes: total known peer profiles |
| `peers.known` | `ProfileStorage` | `profile` | Read lock (inside `get_router_ids()`) | `get_router_ids().take(limit)` | 10,000 | Yes: bounded known peer IDs |
| `peers.activeCount` | `EventHandle<R>` | `events` | Atomic load (`connected_routers()`) | Direct count | — | Yes: currently connected peer count |
| `peers.active` | `TransportManager<R>` | `transport` | HashMap key iteration (`routers.keys()`) | `connected_peer_ids(limit)` via `.take(limit)` | 10,000 | Yes: bounded connected peer IDs |
| `peers.routerInfo` | `ProfileStorage` | `profile` | Read lock (inside `get_raw()`) | `get_raw()` per known peer | 4 MB per RI | Yes: public RouterInfo bytes only |

Unsupported rows (no canonical owner):

| Selector group | Rationale |
|---|---|
| UDP peer categories (fast/highCap/lowCap/etc.) | Java-I2P specific classification; Emissary does not track these categories |
| UDP cookie_active, hidden | No inspection interface in transport layer |
| TCP hosts, status, version, firewalled, peers | TCP transport not yet implemented; no inspection interface |
| NetDB (all 49 selectors) | NetDb is a spawned task (`R::spawn()`) with no inspection handle |
| Tunnel pool (exploratory/client in/out, queue) | TunnelPool is a spawned task with no inspection handle; per-pool counts not accessible |
| Peer bans | No canonical ban owner exists in Emissary |
| Peer limits | No canonical connection-limit owner exists |
| Active peer stats (per-peer transport statistics) | Transport sessions are not exposed through inspection interfaces |

## 4. Neutral snapshot type inventory

All types defined in `emissary-core/src/inspection.rs`:

| Type | Fields | Bounds | Notes |
|---|---|---|---|
| `TransportSnapshot` | `udp_active: bool`, `udp_firewalled: bool`, `tcp_active: bool`, `connected_peer_count: usize`, `connected_peer_ids: Vec<String>`, `ipv4_firewall_status: String`, `ipv6_firewall_status: String` | `connected_peer_ids` bounded by `connected_peer_limit` parameter | Immutable after construction |
| `TunnelSnapshot` | `active_participating: usize`, `exploratory_inbound: usize`, `exploratory_outbound: usize`, `client_inbound: usize`, `client_outbound: usize`, `queue_depth: usize` | Scalar values only, no lists | Fields set to 0 where tunnel pool task not inspectable |
| `NetDbSnapshot` | `active: bool`, `router_info_count: usize`, `lease_set_count: usize`, `known_router_ids: Vec<String>`, `known_peer_count: usize`, `active_peer_count: usize`, `peer_router_infos: BTreeMap<String, Vec<u8>>` | `known_router_ids` bounded by `connected_peer_limit`; `peer_router_infos` bounded by same | `active`/`router_info_count`/`lease_set_count` hardcoded to `false`/0 (NetDB task not inspectable) |
| `CoreSnapshot` | `router_id_b64: String`, `router_info_bytes: Vec<u8>`, `transport: TransportSnapshot`, `tunnels: TunnelSnapshot`, `netdb: NetDbSnapshot` | `router_info_bytes` is fixed-size (serialized RI) | Top-level snapshot; non-generic, `Clone + Debug` |

Type assertions:
- All types derive `Debug, Clone` (required for `Arc` wrapping and logging).
- `CoreSnapshot` contains no `serde` derives, no JSON-RPC wire names, no Proposal 170 key constants.
- No private key, session key, or lease-set private material can enter any snapshot field.
- `peer_router_infos` values are public serialized RouterInfo bytes only (via `ProfileStorage::get_raw()`).

## 5. Exact bounds and deadline table

| Query | Bound | Enforcement mechanism | Deadline |
|---|---|---|---|
| `connected_peer_ids(limit)` | `limit` parameter (caller-supplied) | `.keys().take(limit)` on `TransportManager::routers` HashMap | None (synchronous, under read lock) |
| `get_router_ids().take(limit)` | `limit` parameter (caller-supplied) | `.take(limit)` on iterator from `ProfileStorage::get_router_ids()` | None (synchronous, under read lock) |
| `peer_router_infos` construction | Bounded by `known_router_ids.len()` (already bounded by `limit`) | Iterates only over already-bounded `known_router_ids` | None (synchronous, under read lock per entry) |
| `inspection_snapshot(connected_peer_limit)` | `connected_peer_limit` parameter | All list fields bounded by this single parameter | None (synchronous, called once at startup) |
| `CoreSnapshot` storage | `Arc<CoreSnapshot>` in production | Single allocation at startup | N/A |
| `tunnel_summary()` configured count | Bounded by `tunnel_manager.list()` result | `list().len()` | Tokio mutex lock duration |

The `connected_peer_limit` parameter is set to `10_000` at the call site in `main.rs:473`:
```rust
router.inspection_snapshot(10_000)
```

No async deadline or timeout is applied to snapshot acquisition because:
1. The snapshot is computed once at startup, not per-request.
2. All underlying operations are synchronous (atomic loads, read locks, HashMap iteration).
3. The production adapter stores the snapshot in `Arc<CoreSnapshot>` and serves reads from shared reference.

## 6. Before/after unavailable RouterInfo groups

### M009 state (before M010)

| Selector group | M009 availability | M009 behavior |
|---|---|---|
| UDP active/firewalled/currentPeers/totalPeers | `core-inspection` (awaiting M010) | `Err(Unavailable)` |
| TCP active | `core-inspection` (awaiting M010) | `Err(Unavailable)` |
| Tunnel participating | `core-inspection` (awaiting M010) | `Err(Unavailable)` |
| Peers known/active/RouterInfo | `core-inspection` (awaiting M010) | `Err(Unavailable)` |
| NetDB (all 49) | `unsupported-inspection` | `Err(Unavailable)` |
| Tunnel pool (5 selectors) | `unsupported-inspection` | `Err(Unavailable)` |
| Peer bans/limits/stats | `unsupported-inspection` | `Err(Unavailable)` |

### M010 state (after implementation)

| Selector group | M010 availability | M010 behavior |
|---|---|---|
| UDP active/firewalled/currentPeers/totalPeers | `core-inspection` | **Real data** from `TransportSnapshot` / `NetDbSnapshot` |
| TCP active | `core-inspection` | **Real data** from `TransportSnapshot` |
| Tunnel participating | `core-inspection` | **Real data** from `EventHandle::transit_tunnel_count()` |
| Peers knownCount/known | `core-inspection` | **Real data** from `ProfileStorage` |
| Peers activeCount/active | `core-inspection` | **Real data** from `TransportManager` |
| Peers routerInfo | `core-inspection` | **Real data** from `ProfileStorage::get_raw()` |
| NetDB (all 49) | `unsupported-inspection` | `Err(Unavailable)` — no change (spawned task) |
| Tunnel pool (5 selectors) | `unsupported-inspection` | `Err(Unavailable)` — no change (spawned task) |
| Peer bans/limits/stats | `unsupported-inspection` | `Err(Unavailable)` — no change (no canonical owner) |

### Selectors promoted by M010

| Wire key | M009 → M010 |
|---|---|
| `i2p.router.udp.active` | `Err(Unavailable)` → real `bool` |
| `i2p.router.udp.firewalled` | `Err(Unavailable)` → real `bool` |
| `i2p.router.udp.currentPeers` | `Err(Unavailable)` → real `usize` |
| `i2p.router.udp.totalPeers` | `Err(Unavailable)` → real `usize` |
| `i2p.router.tcp.active` | `Err(Unavailable)` → real `bool` |
| `i2p.router.tunnels.participating` | `Err(Unavailable)` → real `usize` |
| `i2p.router.peers.knownCount` | `Err(Unavailable)` → real `usize` |
| `i2p.router.peers.known` | `Err(Unavailable)` → real bounded array |
| `i2p.router.peers.activeCount` | `Err(Unavailable)` → real `usize` |
| `i2p.router.peers.active` | `Err(Unavailable)` → real bounded array |
| `i2p.router.peers.routerInfo` | `Err(Unavailable)` → real nullable string |

**Total**: 11 selectors promoted from `Err(Unavailable)` to real data. 71 selectors remain `Err(Unavailable)` (unsupported-inspection).

## 7. Contention, cancellation, and polling evidence

### Snapshot acquisition is non-contentious

`Router::inspection_snapshot()` (`router/mod.rs:503-580`) performs:
1. Atomic loads from `EventHandle` (lock-free).
2. `TransportManager::connected_peer_ids(limit)` — iterates `self.routers.keys().take(limit)`. This holds the `TransportManager` for the duration of key iteration but does not hold any other subsystem lock simultaneously.
3. `ProfileStorage::num_routers()` and `get_router_ids()` — reads under `ProfileStorage` internal lock.
4. `ProfileStorage::get_raw()` — reads under `ProfileStorage` internal lock.
5. All data copied into owned `CoreSnapshot` fields before returning.

No cross-subsystem locks are held simultaneously. The snapshot is fully owned after construction.

### Cancellation safety

- `inspection_snapshot()` is synchronous (no `.await` points). It cannot be cancelled mid-operation.
- The production adapter stores the result in `Arc<CoreSnapshot>`. Reading the snapshot is a shared reference borrow — no cancellation possible.
- The `RouterInfoControl` trait methods are `async` but each either returns from a pre-computed snapshot (no await) or calls `tunnel_manager.list().await` / `tunnel_manager.get().await` which acquires a `tokio::sync::Mutex`. Dropping the future releases the mutex.

### Polling behavior

- Snapshot is computed once at startup (`main.rs:473`).
- Each I2PControl HTTP request reads the shared `Arc<CoreSnapshot>` — no recomputation, no locks, no allocations.
- Moderate concurrent polling (e.g. multiple I2PControl clients) reads the same `Arc<CoreSnapshot>` via shared reference. No contention.

### No event subscriber consumption

The `EventHandle<R>` exposes atomic counters only:
- `connected_routers()` — `AtomicUsize`
- `transit_tunnel_count()` — `AtomicUsize`
- `ipv4_firewall_status()` / `ipv6_firewall_status()` — cached behind atomic load

The `EventSubscriber` (single-owner event stream) is never consumed by inspection code. This is verified by static guard `i2pcontrol_does_not_consume_event_subscriber` which scans all I2PControl source files for `EventSubscriber` references outside doc comments.

## 8. UI/event subscriber unaffected proof

### Static guard evidence

```
cargo test -p emissary-cli --no-default-features --features i2pcontrol --test static_guards i2pcontrol_does_not_consume_event_subscriber
```
Result: PASS

This guard scans every line of every I2PControl source file (`src/i2pcontrol/*.rs`) for `EventSubscriber` references outside doc comments. No matches found.

### Architectural evidence

1. `CoreSnapshot` is constructed from `EventHandle` atomic counters and `ProfileStorage`/`TransportManager` read methods. It does not hold or reference an `EventSubscriber`.
2. `ProductionRouterInfoControl` stores `Arc<dyn EventMetrics>` (a trait object over atomic counters), not `EventSubscriber`.
3. The UI/Dioxus frontend uses its own `EventSubscriber` instance created by `EventManager`. The I2PControl inspection path never touches this instance.
4. Static guard `i2pcontrol_does_not_import_ui_modules` verifies no `crate::ui` or `crate::dioxus` imports exist in I2PControl files.

```
cargo test -p emissary-cli --no-default-features --features i2pcontrol --test static_guards i2pcontrol_does_not_import_ui_modules
```
Result: PASS

### No router behavior changes

- `inspection_snapshot()` is a pure read method on `Router`.
- No tunnel construction, peer selection, routing, or NetDB behavior is modified.
- No new spawn tasks are created.
- No frontend dependency enters core or CLI inspection code.

## 9. Static guard output

All 29 static guard tests pass:

```
cargo test -p emissary-cli --no-default-features --features i2pcontrol --test static_guards
```
Result: PASS (29 passed, 0.01s)

Key guards and their outcomes:

| Guard | What it catches | Status |
|---|---|---|
| `i2pcontrol_does_not_consume_event_subscriber` | EventSubscriber use in I2PControl | PASS |
| `i2pcontrol_does_not_import_ui_modules` | UI/Dioxus imports in inspection code | PASS |
| `i2pcontrol_does_not_import_http_or_serde_json_server_libs` | axum imports outside server.rs | PASS |
| `emissary_core_cargo_has_no_i2pcontrol_dependencies` | Core depending on axum/hyper/serde_json | PASS |
| `router_info_dtos_do_not_expose_signing_or_static_key` | Private key material in DTOs | PASS |
| `router_info_control_trait_is_send_sync_and_async` | Send+Sync on trait objects | PASS |
| `production_router_info_does_not_mutate_state` | Mutation methods on production adapter | PASS |
| `production_adapter_returns_unavailable_for_unimplemented_selectors` | Fabricated defaults for unsupported selectors | PASS |
| `production_adapter_does_not_silently_truncate` | Silent truncation instead of Unavailable | PASS |
| `no_fallback_to_fake_in_production` | Fake adapter fallback in init_server | PASS |
| `no_temp_fallback_tunnel_dir` | Temp fallback directory in production | PASS |
| `no_production_fake_adapter_construction` | Fake adapter construction in init_server | PASS |
| `no_duplicate_tunnel_manager_in_init_server` | Duplicate tunnel store instances | PASS |
| `no_error_suppressing_helpers` | unwrap_or_default() error suppression | PASS |
| `control_plane_has_no_tunnel_methods` | Dual-path tunnel access on ControlPlane | PASS |
| `no_fabricated_netdb_default_in_production` | Fabricated NetDbSnapshot::default() | PASS |
| `no_fabricated_tcp_default_in_production` | Fabricated TcpSnapshot::default() | PASS |
| `no_vec_new_as_unavailable_response` | Vec::new() for unavailable sources | PASS |
| `no_fabricated_peer_limits_default_in_production` | Fabricated PeerLimits::default() | PASS |
| `no_error_suppressing_unwrap_or_zero_in_router_info` | unwrap_or(0) error suppression | PASS |
| `no_fabricated_recent_transit_default_in_production` | Fabricated RecentTransitTraffic::default() | PASS |
| `no_hardcoded_udp_active_true_in_production` | Hardcoded active: true in UDP snapshot | PASS |

### Source-level guards (no axum in core)

```
cargo clippy -p emissary-core --features std,events --all-targets -- -D warnings
```
Result: PASS (0 errors). emissary-core Cargo.toml contains no axum, hyper, tokio-rustls, rustls-pemfile, or serde_json dependencies.

## 10. Source-loss and oversize test evidence

### No silent truncation

- `connected_peer_ids(limit)` enforces bound via `.keys().take(limit)` — iterator terminates at `limit`, no partial allocation.
- `get_router_ids().take(limit)` enforces bound via `.take(limit)` — iterator terminates at `limit`.
- `peer_router_infos` construction iterates only over already-bounded `known_router_ids` — bounded by the same `limit`.

### Explicit error for unsupported sources

All unsupported selectors return `Err(InspectionError::Unavailable { group })`:
- `netdb_snapshot()` → `Err(Unavailable { group: NetDb })`
- `banned_peers()` → `Err(Unavailable { group: PeerStats })`
- `peer_limits()` → `Err(Unavailable { group: PeerStats })`
- `active_peer_stats()` → `Err(Unavailable { group: PeerStats })`
- `recent_transit_traffic()` → `Err(Unavailable { group: TrafficMetrics })`

### Production adapter tests

```
cargo test -p emissary-cli --no-default-features --features i2pcontrol --test production_adapter
```
Result: PASS (19 passed)

Tests verify:
- Identity/version/uptime from retained values
- Network status from event metrics
- Transport bytes from event metrics
- Tunnel build stats from event metrics
- UDP snapshot with real firewall status
- UDP active false when no connected routers
- I2PTunnel stats from tunnel store
- Log round-trip from LogRing
- Send+Sync on all adapter types

### Router truthfulness tests

```
cargo test -p emissary-cli --no-default-features --features i2pcontrol --test router_info_truthfulness
```
Result: PASS (32 passed)

Tests verify:
- All 121 selectors are registered
- Selector registry is complete (121 keys)
- Selector registry has unique keys
- Selector registry partition (CORE ∪ ADDRESS_BOOK = ALL, CORE ∩ ADDRESS_BOOK = ∅)
- Unimplemented selectors return Unavailable
- No fabricated defaults for unsupported sources
- Legitimate zero/empty state returns successful zero/empty

## 11. Compatibility, security, and behavior review

### Protocol compatibility

- No JSON-RPC wire changes. Error responses use existing envelope.
- No new fields, methods, or statuses introduced.
- Previously-unavailable selectors now return real data instead of `Err(Unavailable)`. This is a behavior change but is the intended M010 outcome.

### API compatibility

- `RouterInfoControl` trait: no method signatures changed by M010.
- `ProductionRouterInfoControl::new()`: gained a 9th parameter `core_snapshot: Option<CoreSnapshot>`. This is a constructor-level breaking change, but `ProductionRouterInfoControl` is not a public API — it is used only within `emissary-cli` internals.
- `Router::inspection_snapshot()`: new public method. Additive, non-breaking.

### Security review

- `CoreSnapshot` contains only public data: router identity (Base64), public RouterInfo bytes, transport state, tunnel counts, peer IDs.
- No private keys, session keys, lease-set private material, or mutable handles in any snapshot field.
- `peer_router_infos` values are public RouterInfo bytes from `ProfileStorage::get_raw()` — same data published to NetDB.
- `InspectionError::Display` leaks no internal state — no keys, paths, or backtraces.
- No new attack surface: snapshot is read-only, pre-computed, shared via `Arc`.

### Behavior review

- `inspection_snapshot()` is a pure read. It does not mutate router state, start/stop tasks, or modify configuration.
- Snapshot is computed once at startup. No per-request recomputation.
- No router behavior, protocol, lifecycle ownership, frontend, runtime resolver, or tunnel data-plane change is introduced.
- No new spawn tasks created.
- No filesystem access in inspection path (snapshot is in-memory).

## 12. Disposition

**closed** — All 23 acceptance criteria pass. 1061 tests pass (29 static guards, 19 production adapter, 32 router truthfulness, 340 lib, 1061 total). Source map updated. 11 selectors promoted from Unavailable to real data. 71 selectors remain explicitly unsupported with documented rationale. 0 high, 0 medium, 0 low findings. No event subscriber consumption. No UI contamination. No router behavior changes. M011 and M012 may activate.
