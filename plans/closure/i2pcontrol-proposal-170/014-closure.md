# M014 Closure Record — Spec-Constrained Truthfulness and Local Hardening

Status: corrective pass required

Implementation baseline: `2f0508dc73b8d8e5d7429effcbe4dbee8797833c`

Implementation evidence heads:

- `69b4569` — spec-constrained truthfulness and local hardening
- `e59fa23` — live RouterInfo metrics and TLS integration evidence
- `9047fee` — subsequent narrow fencing and connection-proof corrective changes

This record formally evaluates M014 under the repository planning process. The
principal M014 implementation landed, but the milestone cannot be marked closed
because a medium contract/ownership finding remains and was correctly handed to
M016. M015's historical closure is not reused as acceptance evidence because its
reviewed head was later invalidated.

## Requirement-to-evidence matrix

| Requirement | Evidence | Status |
|---|---|---|
| Remove startup-owned RouterInfo success | `ProductionRouterInfoControl` uses live production adapters and explicit unavailable results for unsupported inspection groups | PASS |
| Preserve transport-specific truthfulness | UDP/TCP snapshots return unavailable rather than infer state from aggregate connected-router metrics | PASS |
| Use live production metrics | `EventMetrics` is composed into the production control plane; RouterInfo reads the live adapter | PASS |
| Use the tracing-backed bounded log ring | logger-owned `LogRing` is passed through composition and used by RouterInfo retrieval/clear | PASS |
| Keep service ownership independent by category | fixed-category registry uses one state lock and atomic generation validation/replacement; unit regressions cover isolation and stale writes | PASS |
| Keep SAM output spec-constrained and truthful | Proposal 170 and adopted i2pd require active-session information, but the current allowed Emissary owner exposes only listener addresses; the successful empty map is therefore not accepted as a truthful unavailable fallback | CORRECTIVE PASS REQUIRED |
| Bound accepted connection work before spawn | instance-owned semaphore is acquired before TLS task spawn; the limit-2 adversarial test proves rejection and permit restoration | PASS |
| Preserve TLS/authentication boundaries | real TLS authentication/protected dispatch and plaintext rejection remain covered by `adversarial` | PASS |
| Preserve unsupported tunnel and administrative behavior | existing TunnelManager, AddressBook, and explicit unsupported backend suites remain passing | PASS |
| Keep scope within Proposal 170 | no router, transport, NetDB, tunnel data-plane, frontend, resolver, CI, release, or broad security changes entered M014 | PASS |

## Verification outcomes

Commands run on the current checkout:

```text
cargo fmt --all -- --check
  BLOCKED by unrelated repository-wide stable/nightly rustfmt differences;
  no formatting changes were made outside the affected scope.
cargo check -p emissary-cli --no-default-features --features i2pcontrol
  PASS
cargo test -p emissary-cli --no-default-features --features i2pcontrol
  PASS (1107 tests, 15 suites)
cargo clippy -p emissary-cli --no-default-features --features i2pcontrol --all-targets -- -D warnings
  PASS
```

The format result is an environmental/repository-baseline limitation, not a
failure in the touched I2PControl implementation. The required feature-gated
build, tests, and lint checks pass.

## Invariant, compatibility, and security review

The Proposal 170 wire contract remains unchanged: no method, selector, action,
key, status, error extension, or tunnel data plane was added. Current values are
read from live canonical production handles where available; unsupported or
unsafe inspection remains an existing unavailable/error result. I2PControl
connection work is locally bounded before spawn and remains separate from the
handler bound. No sensitive SAM material is exposed.

## Unresolved findings

- High: none.
- Medium: active SAM can be reported as a successful empty `sessions` object
  even though the canonical SAM owner does not expose the required active-session
  map. M016 has pinned the Proposal 170/i2pd behavior and recorded that resolving
  this requires either an allowed minimal owner read or an authoritatively
  permitted existing unavailable response. The current M016 scope forbids
  inventing a cache, observer, event stream, or lifecycle seam.
- Low: none.

## Planning disposition and downstream status

M014 is formally evaluated as `corrective pass required`, not `closed`, because
the medium SAM truthfulness finding remains. M016 stays `blocked` on the named
contract/ownership blocker, and M017 stays `blocked` because it requires a
complete frozen M016 head plus an independent final-head review. No future plan
can be unblocked by this record.

The next authorized action is the narrow M016 contract/architecture-owner
decision and implementation path described in its plan. This record does not
authorize broad observation infrastructure or changes outside the Proposal 170
administrative boundary.
