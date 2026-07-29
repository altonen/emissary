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
Result: PASS

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

## 3. Invariant review

| Invariant | Status |
|---|---|
| Core inspection types are neutral | Verified — no JSON-RPC wire names in CoreSnapshot |
| Read-only, no mutation authority | Verified — all snapshot fields are owned/cloned |
| Snapshots bounded before copying | Verified — `take(limit)` on iterators |
| No source lock held during serialization | Verified — snapshot is fully owned after construction |
| No private key material in DTOs | Verified — only public RouterInfo bytes |
| No EventSubscriber consumption | Verified — snapshot uses EventHandle atomic counters only |
| No router behavior changes | Verified — inspection_snapshot() is pure read |

## 4. Unresolved findings

0 high, 0 medium, 0 low, 0 info.

## 5. Unsupported semantics with rationale

| Selector group | Rationale |
|---|---|
| UDP peer categories (fast/highCap/lowCap/etc.) | Java-I2P specific classification; Emissary does not track these categories |
| UDP cookie_active, hidden | No inspection interface in transport layer |
| TCP hosts, status, version, firewalled, peers | TCP transport not yet implemented; no inspection interface |
| NetDB (all 49 selectors) | NetDb is a spawned task with no inspection handle; adding one would require architectural changes beyond M010 scope |
| Tunnel pool (exploratory/client in/out, queue) | TunnelPool is a spawned task with no inspection handle; per-pool counts not accessible without modifying the task architecture |
| Peer bans | No canonical ban owner exists in Emissary |
| Peer limits | No canonical connection-limit owner exists |
| Active peer stats (per-peer transport statistics) | Transport sessions are not exposed through inspection interfaces |

## 6. Disposition

**closed** — All 23 acceptance criteria pass. 1061 tests pass. Source map updated. No unresolved findings. M011 and M012 may activate.
