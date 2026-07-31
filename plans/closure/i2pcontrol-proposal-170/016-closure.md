# M016 Closure Record — SAM, Fencing, and Connection-Proof Corrective Pass

Status: blocked

Frozen implementation head: `9047feecde046dac8e0208bbf1acf2e3883f97ae`

Implementation commit: `9047feecde046dac8e0208bbf1acf2e3883f97ae`

## Scope and source reconciliation

The implementation is confined to the M016 boundary: ClientServicesInfo SAM
documentation/qualification, the service registry, the I2PControl server’s
pre-spawn connection bound, and the existing adversarial test.

Authoritative sources pinned before this disposition:

- Official Proposal 170, created and last updated 2026-05-20:
  <https://i2p.net/en/proposals/170-i2pcontrol-expansion/>
- Current upstream i2pd commit `7866f644d3d3dea3d1adf5374a6ea378c8efd536`,
  `daemon/I2PControl.cpp`, `SAMInfoHandler`:
  <https://gitlab.com/PurpleI2P/i2pd/-/blob/7866f644d3d3dea3d1adf5374a6ea378c8efd536/daemon/I2PControl.cpp>

Proposal 170 describes SAM as returning active-session information. The pinned
i2pd implementation serializes each session under its session ID with
`name`, `address`, and `sockets[]`, where each socket has `type` and `peer`.

The Emissary `SamServer` is moved directly into the runtime by `Router::new()`
and exposes only listener addresses. Supplying the adopted map later would
require shared mutable observation state, an event stream, cache, or
lifecycle/owner handle. Those are explicitly forbidden by M016, and no
existing Proposal 170 unavailable/error envelope is defined for this case.

## Requirement-to-evidence matrix

| Requirement | Evidence | Status |
|---|---|---|
| Pin official Proposal 170 and adopted i2pd SAM behavior | Sources above and exact serialization path | PASS |
| Do not treat an example empty object as unavailable-source proof | Production comments/docs call the empty result qualified compatibility behavior, not proof of zero sessions | PASS |
| Implement exact active SAM map or permitted unavailable behavior | Exact map cannot be populated through an allowed owner seam; no permitted unavailable behavior is defined | BLOCKED |
| Preserve fixed passive registry ownership | `ServiceRegistryState` contains fixed entries/generations only | PASS |
| Atomically fence generation and replace entry | One `RwLock<ServiceRegistryState>` guards allocation, validation, replacement, and snapshots | PASS |
| Cross-category isolation and stale rejection | Registry unit/integration regressions, including `stale_generation_cannot_overwrite_new_state` | PASS |
| Keep production connection limit at 128 | `init_server` installs `MAX_CONNECTION_TASKS` (128) | PASS |
| Test-only small connection limit | `ServerInstance::new_for_test_with_connection_limit` | PASS |
| Prove saturation/rejection before TLS/JSON-RPC | `tls_connection_bound_enforced` holds two incomplete handshakes at limit 2 and rejects the third | PASS |
| Prove permit restoration | Same test drops a held connection and completes authenticated TLS JSON-RPC | PASS |
| Preserve TLS auth/protected dispatch/plaintext rejection | Full adversarial suite | PASS |
| Avoid unrelated CI/release/formatting changes | Changed-file review; unrelated formatter output restored | PASS |

## Verification outcomes

```text
cargo check -p emissary-cli --no-default-features --features i2pcontrol        PASS (warning-free)
cargo test -p emissary-cli --no-default-features --features i2pcontrol         PASS (1107 tests, 15 suites)
cargo clippy -p emissary-cli --no-default-features --features i2pcontrol --all-targets -- -D warnings
                                                                                PASS
rustfmt +nightly --check <four touched Rust files>                              PASS
```

The repository-wide `cargo fmt --all -- --check` remains blocked by unrelated
baseline formatting differences, including `examples/rust-tutorial/src/main.rs`,
from the repository’s nightly-only rustfmt settings. No repository-wide
formatter rewrite is included in this corrective pass.

## Invariant, compatibility, and security review

No Proposal 170 method, selector, request key, response key, status, or error
envelope was added. Unsupported tunnel data planes remain unchanged. The
connection semaphore is instance-owned and separate from the handler semaphore.
Registry state remains fixed-size, passive, and secret-free. The retained SAM
empty object is explicitly qualified and is not accepted as evidence of an
empty active-session set.

## Unresolved findings

- High: none.
- Medium: M016-F1 remains blocked. The adopted contract requires active-session
  information, but the exact safe map cannot be sourced under M016’s allowed
  ownership boundary; inventing a successful empty fallback or adding a new
  observer/cache seam would violate the plan.
- Low: none.

## Planning disposition and downstream review

M016 is formally `blocked`, not closed. M015 remains untouched as a historical
invalidated closure record. M017 remains `blocked`: its activation rule requires
a complete M016 implementation or accepted contract disposition, a frozen
complete implementation head, and an auditable independent reviewer. No other
future plan is dependency-ready or can be unblocked by this disposition.

The next action is contract/architecture-owner review of the named SAM seam;
this record does not authorize expanding M016 into a generic observer, cache,
event stream, or lifecycle API.
