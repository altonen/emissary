# M017 Closure Record — Final-Head Independent Reclosure

Status: closed

Frozen M016 production head: `355e24325d602ca10fa2ac7bb434a925f381df60`

Final reviewed implementation/test head: `b2f45deb48303ec4f2359720cb6d61f36dc093d0`

The commits after `355e243` are limited to regression-test and serializer
evidence corrections in the already-authorized M016 I2PControl boundary. The
closure and planning-state commit follows this reviewed head and is
documentation-only.

## Independence and auditability

The M016 implementation executor is the distinct prior Codex implementation
run recorded in `plans/closure/i2pcontrol-proposal-170/016-implementation-disposition.md`,
which produced `355e243` and its handoff record `1bec23c`. The M017 reviewer is
the current Codex final-head review run, performed after the implementation
head was frozen. Both runs used the same repository GitHub account, but they
are distinct auditable execution/review runs with separate commit sequences.

## Changed-file scope

| Scope | Files or commits | Disposition |
|---|---|---|
| M014 production behavior | `69b4569`, `e59fa23`, `9047fee` and their permitted I2PControl/composition paths | In scope; live truthfulness, metrics/log wiring, fencing, TLS bounds |
| M016 production behavior | `355e243` and permitted SAM/router/composition paths | In scope; bounded read-only SAM observation |
| Final evidence corrections | `df092c2`, `dbbd107`, `96a0fbb`, `b2f45de` | In scope; test harness alignment, unavailable-source regression, pinned active-shape serializer evidence, and static-guard preservation |
| Documentation and planning | current M014/M016/M017 docs, registry, roadmap, and closure records | In scope; support claims and closure state reconciled |
| Out of scope | `.github/workflows/**`, release/publishing, frontend, resolver, router algorithms, transports, NetDB, tunnel data planes, cryptography, and broad security work | No changes introduced by this closure |

## Post-M015 reconciliation

Commit `19370671053b534751328a1e761d717696e55761` is repository-wide formatting
churn. Its whitespace-ignored diff contains no semantic change relevant to
Proposal 170 or the M016 dependency paths; it is retained as historical
out-of-scope formatting. Commit `43088a42881a76b3936c76f6e7eb8a51262504c4`
adds only `cfg(feature = "i2pcontrol")` gates to two optional integration test
files. The full feature-enabled CLI test command executes those suites, and
no workflow, required-check policy, or CI matrix was added.

## Pinned contract and source decision

The adopted Proposal 170 source is the official I2P proposal page, created and
last updated 2026-05-20:
<https://i2p.net/en/proposals/170-i2pcontrol-expansion/>.

For the `ClientServicesInfo` SAM session fields marked as adopted from i2pd,
the implementation pins i2pd commit
`7866f644d3d3dea3d1adf5374a6ea378c8efd536`,
`daemon/I2PControl.cpp`, `I2PControlService::SAMInfoHandler`, as recorded in
the M016 disposition. Active primary sessions are keyed by SAM session ID and
contain only `name`, `address`, and bounded sockets with `type` and `peer`.
Disabled SAM returns `enabled: false`; a listening zero-session source returns
a genuine empty snapshot; a missing or overflowed active source returns an
internal error rather than fabricated empty success.

## Requirement and evidence matrix

| Requirement | Evidence | Result |
|---|---|---|
| No stale RouterInfo success | `ProductionRouterInfoControl` live EventMetrics/TunnelManager paths and explicit unavailable groups | Pass |
| UDP/TCP truthfulness | transport-specific selectors return existing unavailable errors; no aggregate inference | Pass |
| Live metrics and logs | `production_router_info_reads_live_event_metrics`, `production_router_info_log_round_trip`, shared logger composition | Pass |
| Per-category fencing | service-registry atomic state tests and cross-category integration coverage | Pass |
| Bounded current SAM observation | `activation_publishes_exact_session_fields`, `socket_add_and_remove_are_visible`, `session_removal_is_visible`, `per_session_socket_bound_is_explicit`, `session_bound_is_explicit` | Pass |
| SAM serializer and unavailable behavior | `serialize_sam_sessions_preserves_pinned_active_shape`, `resolve_sam_listening_without_observation_is_unavailable`, `sam_listening_reports_enabled` | Pass |
| TLS connection bound | `tls_connection_bound_enforced` exceeds the test limit, rejects before TLS/JSON-RPC, and observes capacity restoration | Pass |
| TLS authentication boundaries | `tls_client_authenticates_and_dispatches`, `plaintext_rejected_by_tls_server` | Pass |
| Existing live inventory | `client_services_live` and `production_composition` suites | Pass |
| Scope/invariants | static guards, changed-file review, no API/CI/release/data-plane expansion | Pass |

## Verification outcomes

Required commands and outcomes at the final reviewed head:

```text
cargo fmt --all -- --check
  FAIL: pre-existing formatting differences in untouched files, including
        examples/rust-tutorial/src/main.rs; no touched-file formatting failure.
rustup run nightly rustfmt --check emissary-cli/src/i2pcontrol/client_services.rs
rustup run nightly rustfmt --check emissary-cli/tests/client_services_live.rs
rustup run nightly rustfmt --check emissary-core/src/sam/mod.rs
  PASS
cargo check -p emissary-cli --no-default-features --features i2pcontrol
  PASS
cargo test -p emissary-cli --no-default-features --features i2pcontrol
  PASS: 1111 tests across 15 suites
cargo clippy -p emissary-cli --no-default-features --features i2pcontrol --all-targets -- -D warnings
  PASS
cargo check -p emissary-core --features std,events
  PASS
cargo test -p emissary-core
  PASS with ulimit -n 65536: 994 unit tests, 4 IPv6, 16 ML-KEM, and 50 SAM integration tests
cargo clippy -p emissary-core --features std,events --all-targets -- -D warnings
  PASS
```

The first two normal-limit core attempts exposed unrelated integration
flakiness (`client_ml_kem_768_server_x25519`) and then the process file
descriptor limit (`EMFILE`). The isolated test passed, and the complete core
command passed with the per-process descriptor limit raised; no repository
change was made for either environmental condition.

## Compatibility, security, and invariant review

No Proposal 170 method, selector, field, status, error envelope, or extension
was added. SAM observation exposes no keys, destinations, payloads,
authentication material, sockets, streams, or mutable lifecycle authority.
Observation and response collections are fixed-bound and fail closed on
overflow. Existing unsupported tunnel backends remain explicit stubs; no
runtime tunnel data plane, address-book precedence, router algorithm, or
frontend behavior was introduced.

## Unresolved findings

- High: none.
- Medium: none.
- Low: none.
- Informational: transport/peer/NetDB/recent-window selectors without a
  canonical Emissary source remain explicit unavailable responses; unsupported
  tunnel data planes remain documented unsupported stubs.

## Final disposition and planning state

`closed`: M014’s corrective implementation is accepted at the actual final
head after M016 resolved the SAM truthfulness finding and M017 independently
reviewed the final implementation/test state. M014, M016, and M017 are marked
closed; M015 remains a retained superseded historical record. No future plan
is dependency-blocked or newly unblocked: the Proposal 170 workstream is
complete unless a new finding requires a new plan.
