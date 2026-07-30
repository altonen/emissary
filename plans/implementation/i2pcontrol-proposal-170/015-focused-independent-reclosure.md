# I2PControl Proposal 170 Milestone 015 — Focused Independent Reclosure

Status: blocked

Planning baseline: `2f0508dc73b8d8e5d7429effcbe4dbee8797833c`

Activation rule:

- M014 implementation is complete.
- M014 has a frozen reviewed head.
- `plans/registry.md` marks M014 `closing` and M015 `ready`.
- The M015 reviewer is not the final M014 implementation agent.

Source roadmap:

- `plans/subsystems/i2pcontrol-proposal-170-roadmap.md`

Reviewed implementation plan:

- `plans/implementation/i2pcontrol-proposal-170/014-spec-constrained-truthfulness-and-local-hardening.md`

Primary class: narrow independent closure review

## 1. Objective

Determine whether the exact Proposal 170 I2PControl workstream can be truthfully reclosed after M014, using a focused review of the affected API paths and regressions only.

M015 is not a second implementation phase, a general security audit, a CI project, or a full repository verification program. It reviews the frozen M014 head against Proposal 170, M014 acceptance criteria, and the known post-M013 defects.

## 2. Review boundary

Review only:

- commits from baseline `2f0508dc73b8d8e5d7429effcbe4dbee8797833c` through the frozen M014 head;
- Proposal 170 request/response behavior affected by M014;
- production composition paths supplying I2PControl metrics, logs, inspection, service state, and TLS connection limits;
- targeted tests and documentation changed by M014;
- absence of forbidden scope and verification infrastructure changes.

Do not re-audit:

- router cryptography;
- NTCP2/SSU2 protocol hardening;
- tunnel construction or peer selection;
- NetDB behavior;
- general SAM protocol implementation;
- frontend security;
- storage subsystems unrelated to Proposal 170 administrative stores;
- existing release or CI architecture;
- unrelated dependencies or historical security findings.

Existing hardened code outside the M014 diff is treated as established unless M014 directly changed or bypassed it.

## 3. Independence rule

The closure reviewer must be distinct from the final M014 implementation agent.

The reviewer may:

- inspect code and tests;
- run commands;
- write the closure record;
- update planning status after accepting or rejecting closure;
- make documentation-only corrections that do not change claims or behavior.

The reviewer must not silently repair production defects and close them in the same review commit.

If a production or test defect is found:

1. record it with severity and exact evidence;
2. leave M014 as `corrective pass required` or `closing` as appropriate;
3. keep M015 blocked or rejected;
4. return implementation work to M014 or a narrowly amended successor;
5. do not create another broad milestone sequence unless the defect cannot fit the M014 boundary.

## 4. Required review questions

### 4.1 Specification and scope

- Do all changed wire behaviors match Proposal 170 and the existing I2PControl contract?
- Were any new methods, selectors, aliases, statuses, extension fields, partial-result behavior, or pagination added?
- Were unsupported tunnel data planes left explicit and inactive?
- Were router, transport, NetDB, tunnel, frontend, resolver, or service-lifecycle behaviors changed solely for inspection?
- Were changes outside the permitted M014 files minimal composition/read-only seams?

### 4.2 RouterInfo truthfulness

- Are values described as current actually queried from current canonical state?
- Is any startup snapshot still presented as live state?
- Are UDP and TCP states transport-specific or explicitly unavailable?
- Do actual production metrics reach the corresponding selectors?
- Are recent traffic values real, or explicitly unavailable rather than zeroed?
- Does log retrieval use the tracing-backed application ring?
- Do unsupported Java-specific or nonexistent Emissary semantics remain explicit unavailable/error behavior?
- Are real zero/empty and unavailable source states distinguishable?

### 4.3 ClientServicesInfo

- Does TunnelManager inventory remain live and shared?
- Are service generations independent by category?
- Does same-category producer replacement reject stale updates?
- Do HTTP, SOCKS, I2CP, and SAM enabled states reflect actual listener state?
- Does SAM session output follow the exact specification?
- Is an unavailable session source prevented from appearing as successful current empty state unless the contract explicitly defines that result?
- Is all session/private material excluded?

### 4.4 TLS and local resource bounds

- Does every request still require TLS before JSON-RPC?
- Is request-body limiting applied before full extraction?
- Is accepted TLS/connection work count-bounded before task spawn?
- Does each task own and release its connection permit on success, error, timeout, disconnect, and cancellation?
- Is handler concurrency separately bounded?
- Does shutdown stop accepting new work without introducing a process-wide task framework?

### 4.5 Verification proportionality

- Were regressions added to existing suites where practical?
- Are assertions behavioral and falsifiable?
- Were no new GitHub Actions workflows, release gates, matrices, evidence generators, coverage policies, or unrelated required checks introduced?
- Are the local commands sufficient for the touched package boundary?
- Are environmental skips clearly separated from required failures?

## 5. Required behavioral evidence

The closure record must cite executable evidence for these cases:

1. a supported RouterInfo value changes after startup and the next query observes it;
2. unavailable/source-loss behavior produces an existing compatible error rather than stale/default success;
3. transport-specific activity is not inferred from a cross-transport aggregate;
4. nonzero production metrics reach RouterInfo;
5. tracing-backed log retrieval and clear operate on the shared application ring;
6. service observer generations are isolated per category;
7. SAM follows the chosen specification-compliant current-state or unavailable path;
8. pre-spawn TLS connection count is bounded and capacity returns after task exit;
9. real TLS Authenticate plus one protected method succeeds;
10. plaintext cannot reach JSON-RPC;
11. TunnelManager/ClientServicesInfo cross-method inventory remains current;
12. AddressBook and explicit unsupported tunnel-stub behavior are unchanged by the corrective pass.

Direct unit tests may support but must not replace the production-shaped behavior for items 1, 4, 5, 8, 9, and 10.

## 6. Review of file scope

The closure record must list every production file changed by M014 and classify it as:

- I2PControl implementation;
- permitted composition wiring;
- permitted minimal read-only core seam;
- documentation/test/planning;
- out-of-scope.

Any out-of-scope production file requires rejection unless the change is a mechanical compile adjustment with no behavior effect and is explicitly justified.

Reject closure if M014 adds or modifies:

- `.github/workflows/**`;
- release/publishing automation;
- general project-wide security policy;
- unrelated runtime supervision;
- unrelated transport/router/NetDB/tunnel logic;
- new missing tunnel data planes;
- frontend management functionality.

## 7. Local verification commands

Run the targeted commands against the frozen M014 head:

```bash
cargo fmt --all -- --check
cargo check -p emissary-cli --no-default-features --features i2pcontrol
cargo test -p emissary-cli --no-default-features --features i2pcontrol
cargo clippy -p emissary-cli --no-default-features --features i2pcontrol --all-targets -- -D warnings
```

If M014 touched core:

```bash
cargo check -p emissary-core --features std,events
cargo test -p emissary-core
cargo clippy -p emissary-core --features std,events --all-targets -- -D warnings
```

Rerun the focused regression suites named by M014 when they are not already clearly identifiable in the full package output.

Do not require or create:

- full unrelated workspace test runs;
- UI builds blocked by desktop libraries;
- multi-platform CI;
- network test farms;
- release dry runs;
- coverage thresholds;
- benchmark gates;
- generated conformance artifacts;
- signed evidence bundles.

The reviewer may inspect existing GitHub status but absence of CI is not a closure defect because this workstream intentionally uses local targeted verification. Closure must not imply that remote CI ran when it did not.

## 8. Closure record structure

Create:

```text
plans/closure/i2pcontrol-proposal-170/015-closure.md
```

The record must contain only:

1. status and frozen reviewed head;
2. reviewer independence statement;
3. changed-file scope table;
4. M014 acceptance-criteria disposition;
5. affected Proposal 170 contract/source table;
6. required behavioral evidence and command outcomes;
7. compatibility and scope review;
8. unresolved findings by severity;
9. exact disposition;
10. registry/roadmap update commit.

Do not recreate a full repository-wide security report or duplicate the entire Proposal 170 conformance matrix.

## 9. Severity and disposition

Use these rules:

- **High:** plaintext reaches dispatch, production fake/fallback state returns, secret material leaks, or unbounded remote work is readily exploitable.
- **Medium:** current-state API reports stale/fabricated success, required Proposal 170 state is knowingly misreported, cross-service observation corrupts state, or a required regression is absent.
- **Low:** documentation mismatch or nonmaterial diagnostic defect that does not affect contract behavior.
- **Info:** optional polish with no correctness implication.

Disposition:

- `closed` only with zero unresolved high and medium findings;
- `conditionally closed` only for a genuinely external operational limitation, never for an implementation or required-test defect;
- `corrective pass required` when any high/medium defect remains or a required M014 criterion is not met.

A known missing canonical Emissary source is not automatically a defect if Proposal 170 is satisfied by explicit compatible unavailable/error behavior. It is a defect when the endpoint instead reports stale, zero, false, or empty success.

## 10. Acceptance criteria

M015 may close the workstream only when:

1. The reviewed head is frozen and recorded.
2. The reviewer is independent from the final M014 implementation agent.
3. Every M014 acceptance criterion has a supported pass disposition or an explicitly inapplicable rationale.
4. Exact Proposal 170 wire behavior remains unchanged except for correction from false success to truthful current/unavailable behavior.
5. No protocol extension or missing tunnel data plane was introduced.
6. No unrelated hardened code was redesigned or broadly reopened.
7. No CI, release, publishing, matrix, or evidence-loop machinery was added.
8. Current supported RouterInfo values are actually current.
9. Unsupported RouterInfo semantics return compatible unavailable/error behavior.
10. Production metrics and logs use canonical existing sources.
11. Client-service generation fencing is correct per category.
12. SAM sessions follow the exact contract without a false empty placeholder.
13. TLS connection work is count-bounded before spawn.
14. Real TLS and plaintext rejection have production-shaped behavioral evidence.
15. Required local checks, tests, and clippy pass for touched packages.
16. AddressBook, TunnelManager, explicit unsupported stubs, and frontend/runtime-resolver boundaries remain intact.
17. Documentation accurately distinguishes complete API contract support from deferred runtime tunnel implementations and unavailable inspection semantics.
18. No unresolved high or medium finding remains.
19. The closure statement can be made without hiding skipped required evidence.
20. Roadmap and registry are marked closed only in or after the accepted M015 closure commit.

## 11. Exact completion statement

Use the existing completion statement only if all acceptance criteria pass:

> Emissary implements the complete Proposal 170 I2PControl API contract. Unsupported tunnel data planes are wired through explicit stubs and remain separate implementation work.

Qualify the statement in documentation when selectors use Proposal-compatible unavailable behavior because Emissary has no safe canonical equivalent. Do not imply that Java-I2P-specific internal observability or missing tunnel runtimes were implemented.

## 12. Rejection conditions

Reject closure if any of the following is present:

- a startup snapshot is still described as current;
- UDP/TCP state is inferred from an unrelated aggregate;
- production bandwidth/log selectors read disconnected shadow state;
- cross-category service handles invalidate each other;
- active SAM sessions are knowingly returned as empty success without contract justification;
- TLS connection tasks remain count-unbounded;
- a required endpoint regression was replaced by a static scan or tautological assertion;
- production defects were fixed by the closure reviewer and closed in the same pass;
- scope expanded into router, transport, NetDB, tunnel data planes, frontend, runtime resolver, CI, release, or general security work.