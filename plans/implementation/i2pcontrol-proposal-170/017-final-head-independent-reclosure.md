# I2PControl Proposal 170 Milestone 017 — Final-Head Independent Reclosure

Status: ready

Planning baseline: `43088a42881a76b3936c76f6e7eb8a51262504c4`

Activation rule:

- M016 implementation is complete or has an explicit accepted contract disposition.
- M016 has a frozen implementation head.
- `plans/registry.md` marks M016 `closing` and M017 `ready`.
- The M017 reviewer is distinct from the final M016 implementation executor.
- No production commit lands after the frozen head during review.

Source roadmap:

- `plans/subsystems/i2pcontrol-proposal-170-roadmap.md`

Reviewed implementation plan:

- `plans/implementation/i2pcontrol-proposal-170/016-sam-fencing-and-connection-proof-corrective-pass.md`

Primary class: focused independent closure and final-head scope reconciliation

## 1. Objective

Determine whether the exact Proposal 170 I2PControl workstream can be truthfully closed at the repository’s actual final head after M016.

M017 is not another implementation phase, a project-wide security audit, a CI project, or a full workspace verification campaign. It reviews only:

- the M016 changes;
- the three findings owned by M016;
- the already-landed M014 behaviors that those changes depend on;
- the two post-M015 commits that were not part of the prior frozen review;
- the final changed-file scope and targeted local evidence.

## 2. Why M015 is superseded

M015 reviewed frozen head `e59fa239eb03ea608d3b8123a36fc2fa72fcf675`, but current `master` subsequently gained:

- `19370671053b534751328a1e761d717696e55761` — repository-wide formatting/feature-check restoration;
- `43088a42881a76b3936c76f6e7eb8a51262504c4` — optional I2PControl integration-test feature gates.

The prior closure also accepted:

- unverified SAM unavailable-as-empty semantics;
- non-atomic same-category generation fencing;
- a below-limit connection test as saturation evidence.

Therefore `plans/closure/i2pcontrol-proposal-170/015-closure.md` remains a historical record but is not the current closure authority. M017 must create a new closure record rather than rewriting M015.

## 3. Independence and auditability

The reviewer must be distinct from the final M016 implementation executor.

The M017 closure record must state:

- frozen implementation head SHA;
- final reviewed head SHA;
- M016 implementation executor identity or agent/run identifier;
- M017 reviewer identity or distinct agent/run identifier;
- whether both commits use the same GitHub account;
- the concrete basis for treating the review as independent.

Using the same repository credential is acceptable only when the closure record identifies distinct execution/review agents or runs. A generic statement that the pass was “independent” is insufficient.

The reviewer may:

- inspect diffs and current files;
- run targeted commands;
- write the M017 closure record;
- update registry/roadmap status after accepting or rejecting closure;
- correct documentation-only typos that do not alter capability claims.

The reviewer must not repair production or behavioral test defects and close them in the same pass.

If a defect is found:

1. record severity and exact evidence;
2. reject closure;
3. return the defect to M016’s narrow boundary when possible;
4. do not create another milestone unless the defect cannot fit M016 without violating its scope;
5. do not add CI or broad verification to compensate for a missing local regression.

## 4. Review boundary

### 4.1 Required review set

Review:

- `43088a42881a76b3936c76f6e7eb8a51262504c4..FROZEN_M016_HEAD`;
- the current final head if closure/documentation commits follow the frozen M016 implementation head;
- M016-owned production files;
- affected existing tests;
- directly affected Proposal 170 documentation;
- current registry and roadmap claims.

### 4.2 Required dependency spot checks

Spot-check only the M014 behavior required by M016:

- live EventMetrics still feed RouterInfo totals;
- application LogRing still reaches RouterInfo;
- UDP/TCP/peer/recent-window unavailable paths remain explicit;
- pre-spawn connection semaphore still exists and remains independent from the handler semaphore;
- ClientServicesInfo still uses the shared service registry and TunnelManager inventory.

Do not reopen unrelated AddressBook, TunnelManager, authentication, persistence, router, transport, NetDB, tunnel, frontend, or general security reviews absent a direct diff regression.

### 4.3 Out-of-scope systems

Do not re-audit:

- cryptography;
- NTCP2 or SSU2;
- tunnel construction or peer selection;
- NetDB algorithms;
- general SAM protocol correctness beyond the read-only `ClientServicesInfo` mapping;
- frontend code;
- runtime address resolution;
- unrelated storage;
- packaging or release processes;
- existing CI architecture.

Existing hardened code outside the reviewed diff is treated as established.

## 5. Post-M015 scope reconciliation

M017 must explicitly reconcile the commits that landed after M015’s frozen head.

### 5.1 Commit `1937067` — broad formatting change

Review mechanically, not as a new security audit:

```bash
git diff --stat 1937067^ 1937067
git diff --check 1937067^ 1937067
git diff -w 1937067^ 1937067 -- emissary-core emissary-cli emissary-util
```

Required disposition:

- If the whitespace-ignored diff contains no semantic change relevant to Proposal 170 or the M016 dependency paths, record it as pre-existing out-of-scope formatting churn.
- If semantic changes are present in M016 dependency paths, review those exact hunks.
- If semantic changes are present only in unrelated hardened code, do not absorb them into Proposal 170. Record that they require their own repository owner/review if material.
- Do not create another repository-wide formatting or revert commit merely to clean the Proposal 170 history.
- Do not claim that M016 itself stayed in scope if M016 adds new unrelated formatting.

### 5.2 Commit `43088a4` — feature gates

Verify that:

- the two gated tests compile and execute under `--features i2pcontrol`;
- the targeted M016 test command includes the relevant suites;
- default builds no longer attempt optional I2PControl tests without the feature;
- no GitHub Actions workflow or required-check policy was added;
- the gate does not silently skip a required M016 regression during closure.

This is a local test-compilation correction, not authorization to expand CI.

### 5.3 Final-head rule

The closure record must name the actual reviewed final head. If a production or test commit lands after review begins:

- update the frozen head;
- inspect the new diff;
- rerun only affected targeted commands;
- do not retain a closure that names an older head.

Documentation/registry closure commits may follow the frozen production head, but their exact diff must be included in final-head review.

## 6. Required review questions

### 6.1 Proposal 170 SAM contract

- Is the official Proposal revision/date pinned?
- Is the adopted upstream i2pd implementation commit and serialization source pinned?
- Does the Emissary output match the adopted SAM map shape exactly?
- Are disabled, listening with zero sessions, listening with active sessions, and unavailable-source behavior distinguished as required?
- Is any empty object justified only by an example rather than defined behavior?
- If a bounded core accessor was added, is it read-only, on-demand, fixed-bound, and secret-free?
- Are session removal and zero-session transitions observable on the next request?
- Were no SAM lifecycle, registry, polling, cache, or protocol changes added?

Any unresolved ambiguity or fabricated empty success rejects strict closure.

### 6.2 Service registry fencing

- Are entries and generation ownership validated/updated in one atomic transaction?
- Can an old same-category handle overwrite after a new handle is allocated?
- Does a different category remain independent?
- Does the fixed six-category registry remain passive and free of task/lifecycle authority?
- Do tests prove same-category stale rejection, no stale overwrite, and cross-category isolation?

A split validation/write race rejects closure even if sequential tests pass.

### 6.3 Connection-task bound

- Does production still use the intended default limit of 128?
- Is any test seam non-public or test-oriented and free of user configuration impact?
- Does the behavioral test set a small limit and exceed it?
- Are the configured number of permits held by simultaneous incomplete/active connections?
- Is the over-limit connection observably rejected before JSON-RPC?
- Is capacity restored after disconnect or completion?
- Are timeouts bounded and the passing test free from long sleeps?
- Do TLS Authenticate/protected dispatch and plaintext rejection remain passing?

Ten successful connections below a limit of 128 do not satisfy this gate.

### 6.4 Scope and simplicity

- Did M016 touch only its permitted production/test/docs boundary?
- Was any core change limited to the one allowed SAM read-only seam?
- Were there no unrelated formatting changes?
- Were no dependencies added without a recorded stop/review decision?
- Were no CI workflows, matrices, release jobs, required checks, coverage gates, or evidence generators added?
- Was no broad security re-hardening performed?
- Were existing Proposal 170 methods and error envelopes preserved?

## 7. Required behavioral evidence

The closure record must contain a compact table with actual test names and outcomes for:

1. SAM disabled response.
2. SAM listening with zero sessions.
3. SAM active-session serialization, if required by the pinned contract.
4. SAM session removal or current-state transition, if entries are implemented.
5. SAM unavailable/source-failure behavior.
6. cross-category service-handle isolation.
7. same-category replacement rejects stale handle.
8. stale handle cannot overwrite current entry.
9. connection saturation above the configured test limit.
10. over-limit socket cannot reach JSON-RPC.
11. capacity restoration after disconnect.
12. real TLS Authenticate and one protected request.
13. plaintext rejection.
14. one live RouterInfo metric and one shared LogRing spot check.

Static source scans may supplement but cannot replace these behavioral cases.

## 8. Verification commands

Run once against the frozen/final reviewed head:

```bash
cargo fmt --all -- --check
cargo check -p emissary-cli --no-default-features --features i2pcontrol
cargo test -p emissary-cli --no-default-features --features i2pcontrol
cargo clippy -p emissary-cli --no-default-features --features i2pcontrol --all-targets -- -D warnings
```

Focused reruns are acceptable during review:

```bash
cargo test -p emissary-cli --no-default-features --features i2pcontrol --test client_services_live
cargo test -p emissary-cli --no-default-features --features i2pcontrol --test client_services_integration
cargo test -p emissary-cli --no-default-features --features i2pcontrol --test adversarial
cargo test -p emissary-cli --no-default-features --features i2pcontrol --test production_adapter
```

Only if M016 touched core:

```bash
cargo check -p emissary-core --features std,events
cargo test -p emissary-core
cargo clippy -p emissary-core --features std,events --all-targets -- -D warnings
```

Do not require or add:

- GitHub Actions evidence;
- full unrelated workspace testing;
- UI-feature builds;
- multi-platform matrices;
- nightly jobs;
- fuzzing;
- network farms;
- coverage thresholds;
- generated evidence bundles;
- release dry runs.

One successful targeted run on an unchanged frozen head is sufficient. Do not create repeated verification loops.

## 9. Closure record requirements

Create:

```text
plans/closure/i2pcontrol-proposal-170/017-closure.md
```

The record must include:

1. status: `closed` or `rejected`;
2. frozen M016 implementation head;
3. final reviewed head;
4. implementation/reviewer identity evidence;
5. changed-file scope table;
6. post-M015 commit reconciliation;
7. pinned SAM contract/upstream source decision;
8. M016 acceptance-criteria disposition;
9. actual behavioral test table;
10. exact command outcomes, including failures/skips;
11. unresolved findings by severity;
12. exact registry/roadmap disposition.

Do not copy entire test logs or generate an evidence archive. Concise command outcomes and specific failing evidence are sufficient.

## 10. Acceptance criteria

M017 may close Proposal 170 only when:

1. The reviewed final head is explicitly named and current.
2. Reviewer independence is auditable, not merely asserted.
3. M015 is retained as historical invalid closure and M017 is the current authority.
4. SAM behavior is pinned to official Proposal 170 and the adopted upstream implementation.
5. Active-session information is truthful, bounded, and exact, or an authoritative permitted unavailable behavior is used.
6. No unavailable SAM source is silently represented as successful empty state without contract authority.
7. Same-category generation validation and write are atomic.
8. Cross-category independence and same-category stale rejection pass.
9. A stale handle cannot overwrite current service state.
10. Production connection-task default remains unchanged.
11. The connection test exceeds its configured test limit and observes rejection.
12. Permit restoration after disconnect/completion is observed.
13. Real TLS auth/protected dispatch and plaintext rejection pass.
14. M014 live metrics/log and explicit-unavailable spot checks pass.
15. M016 contains no unrelated core/security/UI/transport/NetDB/tunnel changes.
16. The post-M015 broad formatting commit is reconciled without being silently attributed to M016.
17. Feature-gated I2PControl tests execute under the required feature command.
18. No CI/release/matrix/coverage/evidence infrastructure was added.
19. Targeted package format, check, tests, and clippy pass.
20. Zero unresolved high- or medium-severity findings remain.
21. Documentation accurately states supported, unavailable, and stubbed behavior.
22. Registry and roadmap move to `closed` only in or after the accepted M017 closure commit.

## 11. Mandatory rejection conditions

Reject closure if any of the following is present:

- SAM contract remains ambiguous but the implementation claims complete active-session support;
- active SAM returns an empty map solely because the source is unavailable;
- session output exposes secrets or mutable authority;
- same-category generation check/write remains split across independently released locks;
- the connection test does not exceed its configured limit;
- saturation is inferred only from source inspection;
- a required I2PControl test is skipped by feature gating;
- M016 includes unrelated repository-wide formatting or security changes;
- a production/test commit landed after the named reviewed head;
- reviewer independence cannot be distinguished from the final implementation pass;
- CI, release, or broad verification machinery was added;
- any high/medium finding remains.

## 12. Final disposition

If accepted:

- mark M016 `closed`;
- mark M017 `closed`;
- return Proposal 170 roadmap/registry to `closed`;
- state exact unsupported/unavailable selectors and tunnel stubs without calling them implemented data planes;
- retain M015 as a superseded historical closure.

If rejected:

- leave Proposal 170 active;
- record the exact blocker;
- return the defect to M016 if it fits the existing scope;
- do not create a broader milestone or CI program.
