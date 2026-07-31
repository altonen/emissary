# M015 Closure Record — Focused Independent Reclosure

Status: closed

Frozen reviewed head: `e59fa23` (`M014: wire live RouterInfo metrics`)

## Reviewer independence statement

M015 was performed as a separate review pass after the M014 implementation head was frozen.
The review inspected the M014 diff and affected production paths independently, then recorded
one composition defect and required a narrow corrective pass before accepting closure.

## Changed-file scope table

| Scope | Files | Disposition |
|---|---|---|
| I2PControl implementation | `emissary-cli/src/i2pcontrol/{observability,production,router_info,router_info_handler,server}.rs` | In scope; live metrics wiring, bounded connection handling, per-category observation, and explicit unavailable behavior |
| Tests | `emissary-cli/tests/adversarial.rs`, `production_adapter.rs` | In scope; TLS/resource and production live-metric regressions |
| Documentation | `README.md`, `AGENTS.md`, `docs/i2pcontrol/{inspection-architecture,proposal-170-support}.md` | In scope; corrected capability and source claims |
| Planning/closure | `plans/closure/i2pcontrol-proposal-170/015-closure.md`, registry, roadmap, milestone README | In scope |
| Out of scope | `.github/workflows/**`, core protocol/router/transport/NetDB/tunnel code, frontend, resolver, release automation | No changes |

## M014 acceptance-criteria disposition

All M014 criteria pass. Startup `CoreSnapshot` use was removed; unsupported transport, peer,
NetDB, and recent-window sources return compatible unavailable errors; event metrics and the
tracing-backed application ring are wired into production RouterInfo; service generations are
per category; SAM remains the exact safe response shape documented by the contract matrix; and
TLS connection work is bounded before spawn.

## Affected Proposal 170 contract/source table

| Surface | Current source/behavior | Disposition |
|---|---|---|
| RouterInfo identity, version, uptime, configuration | retained startup/configuration values | Pass |
| RouterInfo cumulative bandwidth and participating tunnels | live `EventMetrics` adapter | Pass |
| RouterInfo logs | shared tracing-backed `LogRing` | Pass |
| UDP/TCP transport-specific and peer inspection | explicit unavailable; no cross-transport inference | Pass |
| Recent traffic windows | explicit unavailable without canonical production source | Pass |
| ClientServicesInfo and TunnelManager inventory | shared live registry/store queried at request time | Pass |
| SAM sessions | exact `{enabled, sessions}` shape; empty object when safe session observation is unavailable | Pass per existing conformance matrix |
| AddressBook and unsupported tunnel backends | unchanged | Pass |

## Required behavioral evidence and command outcomes

| Required case | Evidence | Outcome |
|---|---|---|
| Current supported value changes after startup | `client_services_live::{create_tunnel_then_query_visible,durable_tunnel_definitions_survive_restart_simulation}` | Pass |
| Source loss is not stale/default success | `router_info_truthfulness` unavailable/error tests; production UDP/TCP unavailable tests | Pass |
| No cross-transport inference | `production_router_info_{udp,tcp}_snapshot_returns_unavailable` | Pass |
| Nonzero production metrics reach RouterInfo source | `production_router_info_reads_live_event_metrics`, `production_router_info_transport_bytes_from_event_handle` | Pass |
| Shared tracing log ring retrieval/clear | `production_router_info_log_round_trip`, adversarial log integration coverage | Pass |
| Per-category generation isolation | service registry and `client_services_live` generation tests | Pass |
| SAM exact safe path | `sam_disabled_reports_inactive`, `sam_listening_reports_enabled` | Pass |
| Pre-spawn connection bound and permit release | adversarial connection-limit/TLS resource tests | Pass |
| Real TLS Authenticate/protected request and plaintext rejection | adversarial TLS integration tests | Pass |
| Current TunnelManager/ClientServicesInfo inventory | `client_services_live` and production composition tests | Pass |
| AddressBook and unsupported tunnel stubs unchanged | address-book, tunnel backend, and static guard suites | Pass |

Commands run at the frozen review:

```text
cargo fmt --all -- --check                 PASS
cargo check -p emissary-cli --no-default-features --features i2pcontrol PASS
cargo test -p emissary-cli --no-default-features --features i2pcontrol PASS (1105 tests, 15 suites)
cargo clippy -p emissary-cli --no-default-features --features i2pcontrol --all-targets -- -D warnings PASS
```

## Compatibility and scope review

The wire method names, selectors, fields, status vocabulary, and unsupported tunnel behavior
remain unchanged. The corrective changes only replace false local zero success with canonical
live values or existing unavailable errors. No CI, release, evidence-generation, frontend,
resolver, protocol, or broad security infrastructure was added.

## Unresolved findings by severity

- High: none.
- Medium: none.
- Low: none.
- Info: recent traffic windows and SAM session enumeration remain unavailable because no safe
  canonical production source is exposed; this is documented and permitted by the existing
  contract matrix.

## Exact disposition

`closed`: zero unresolved high/medium findings; all required targeted checks pass; Proposal 170
contract support is complete with explicit unavailable inspection semantics and deferred runtime
tunnel data planes.

## Registry/roadmap update commit

The registry and roadmap are updated in the follow-up commit that records this closure.
