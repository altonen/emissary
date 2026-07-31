# M014 Closure Record — Spec-Constrained Truthfulness and Local Hardening

Status: corrective pass required

Implementation baseline: `2f0508dc73b8d8e5d7429effcbe4dbee8797833c`

Implementation evidence heads:

- `69b4569` — spec-constrained truthfulness and local hardening
- `e59fa23` — live RouterInfo metrics and TLS integration evidence
- `9047fee` — atomic fencing and connection saturation proof

## Disposition

The principal M014 implementation landed, but M014 cannot be marked closed while a listening SAM bridge can return successful `sessions: {}` despite active sessions existing in the canonical SAM runtime.

M015 is not acceptance evidence because it reviewed an older head and accepted the SAM empty-output interpretation without a canonical active-session source.

## Requirement-to-evidence matrix

| Requirement | Evidence | Status |
|---|---|---|
| Remove startup-owned RouterInfo success | production adapter uses live sources or explicit unavailable behavior | PASS |
| Preserve transport-specific truthfulness | UDP/TCP do not infer state from aggregate connections | PASS |
| Use live production metrics | EventMetrics adapter is wired through composition | PASS |
| Use tracing-backed bounded log ring | application ring reaches RouterInfo retrieval and clear | PASS |
| Keep service ownership independent and atomic | one registry state lock validates generation and replaces entries | PASS |
| Keep SAM output truthful | active-session map still lacks a completed canonical bounded read path | CORRECTIVE PASS REQUIRED |
| Bound accepted connection work before spawn | instance-owned semaphore plus real saturation/restoration regression | PASS |
| Preserve TLS/authentication boundaries | TLS authentication, protected dispatch, and plaintext rejection remain covered | PASS |
| Preserve unsupported tunnel and administrative behavior | existing suites remain unchanged and passing | PASS |
| Keep scope within Proposal 170 | no router, transport, NetDB, tunnel data-plane, frontend, resolver, CI, release, or broad security expansion | PASS |

## Verification evidence

Recorded targeted outcomes at the current corrective implementation state:

```text
cargo check -p emissary-cli --no-default-features --features i2pcontrol        PASS
cargo test -p emissary-cli --no-default-features --features i2pcontrol         PASS
cargo clippy -p emissary-cli --no-default-features --features i2pcontrol --all-targets -- -D warnings
                                                                                PASS
```

Repository-wide formatting remained affected by unrelated baseline stable/nightly differences. No unrelated formatting rewrite is required for this closure.

## Remaining finding

- High: none.
- Medium: active SAM session state can be represented as successful empty output because I2PControl has no completed bounded current observation path.
- Low: none.

## Authorized corrective path

The architecture owner has now authorized one fixed-capacity, read-only SAM session-observation handle under amended M016:

- `plans/implementation/i2pcontrol-proposal-170/016-sam-fencing-and-connection-proof-corrective-pass.md`

The authorized handle may:

- maintain sanitized adopted i2pd metadata at existing SAM session/socket lifecycle transitions;
- expose bounded on-demand snapshots through a cloneable read-only handle;
- be captured before `SamServer` moves into the router runtime;
- fail explicitly on overflow rather than return partial or empty success.

It may not expose mutable authority or sensitive session internals, add generic observation infrastructure, change SAM protocol behavior, or expand unrelated scope.

## Downstream status

- M014 remains `corrective pass required`.
- M016 is `ready` under the amended bounded observation-handle plan.
- M017 remains `blocked` until M016 lands on a frozen head and moves to `closing`.
- Only M017 may independently accept final closure at the actual final head.

This record does not itself authorize closure. It identifies the exact remaining finding and the narrow implementation plan that must resolve it.