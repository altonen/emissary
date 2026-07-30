# I2PControl Proposal 170 Roadmap

Status: active narrow corrective work

Current corrective planning baseline: `2f0508dc73b8d8e5d7429effcbe4dbee8797833c`

Canonical references:

- `plans/000-long-term-specification.md`
- `plans/001-terminology-and-domain-model.md`
- `plans/002-long-term-roadmap.md`
- `plans/003-planning-process.md`
- `plans/adrs/ADR-0001-proposal-170-contract-and-stub-boundary.md`
- `docs/i2pcontrol/proposal-170-conformance.md`
- I2P Proposal 170 and the existing I2PControl JSON-RPC contract

## 1. Purpose

This subsystem owns the exact Proposal 170 I2PControl API contract for Emissary:

- authenticated JSON-RPC over the independent I2PControl TLS listener;
- RouterInfo selectors;
- four administrative AddressBook stores and operations;
- TunnelManager configuration and explicit unsupported backends;
- ClientServicesInfo;
- only the bounded read-only observation and local resource controls required by those methods.

The workstream does not own router algorithms, transport behavior, NetDB behavior, tunnel construction, peer selection, frontend behavior, runtime address resolution, missing tunnel data planes, release automation, or general CI/security policy.

## 2. Canonical invariants

- Proposal 170 wire names, methods, selectors, actions, tunnel types, JSON types, and error envelopes remain exact.
- Existing I2PControl authentication and version behavior remains compatible.
- Unsupported tunnel runtimes remain explicit inactive backends.
- Missing or failing state is not disguised as zero, false, empty, absent, stale, or default success.
- A value described as current must come from current canonical state.
- Existing real Emissary sources are reused; shadow metrics, logs, caches, and stores are not introduced.
- Where Emissary has no safe canonical equivalent, use Proposal-compatible null/error behavior rather than create new router semantics.
- Core inspection, if unavoidable, is read-only, bounded, neutral, secret-free, and behavior-preserving.
- The service remains frontend-independent.
- TLS is required before JSON-RPC.
- Remote connection and request work is locally bounded inside the I2PControl server.
- Existing hardened code outside the I2PControl boundary is not broadly reopened.
- No CI, release, publishing, matrix, coverage, or generated-evidence machinery is added by this workstream.

## 3. Non-goals

The following remain outside Proposal 170 corrective work:

- implementing missing client/server tunnel data planes;
- adding BOB;
- migrating startup-managed tunnels into Proposal lifecycle ownership;
- adopting Proposal administrative address books for runtime resolution;
- frontend management surfaces;
- Java-I2P-specific peer classifications or counters without an Emissary equivalent;
- new protocol extensions, aliases, pagination, richer statuses, or partial-result envelopes;
- router, transport, NetDB, tunnel-pool, peer-selection, or congestion redesign;
- a general metrics, logging, inspection, task-supervision, security, CI, or release framework;
- broad revalidation of code outside the M014 diff.

## 4. Historical sequence

M001–M004 established the contract, administrative domain, AddressBook, and TunnelManager/stub boundaries.

M005–M007 were superseded after production truthfulness and closure defects were found.

M008–M013 implemented the first corrective sequence. M008 production composition and the principal M012 TLS-serving correction materially improved the implementation. A later review found that strict M013 closure was still overstated because current-state, source wiring, service fencing, SAM, and pre-spawn connection bounds were incomplete.

Historical closure records remain under:

```text
plans/closure/i2pcontrol-proposal-170/
```

They remain useful evidence but do not override the current M014/M015 corrective status.

## 5. Findings reopening M013

M014 owns only these findings:

| Finding | Current severity | Corrective rule |
|---|---|---|
| RouterInfo uses an immutable startup `CoreSnapshot` for values described as current | medium | current bounded source or explicit unavailable |
| UDP and TCP active are inferred from one aggregate connected-router count | medium | transport-specific source or explicit unavailable |
| bandwidth/recent traffic selectors read disconnected I2PControl-local state | medium | existing live source or explicit unavailable |
| RouterInfo receives a fresh log ring rather than the tracing-backed application ring | medium | inject existing ring |
| service generations are global rather than per category | medium | per-category fencing inside I2PControl |
| active SAM can return unconditional empty sessions due to missing observation | medium, contract-dependent | exact spec-compliant bounded source or unavailable/blocker |
| accepted TLS connection tasks are not count-bounded before spawn | medium | one local pre-spawn semaphore |
| M013 registry/docs claim strict closure despite these findings | closure defect | M015 independent reclosure |

No unrelated finding is activated by this table.

## 6. Corrective dependency graph

```text
M014 Spec-constrained truthfulness and local hardening
    |
    v
M015 Focused independent reclosure
```

Only M014 is ready. M015 remains blocked until M014 lands, freezes a reviewed head, and moves to `closing`.

This is the complete corrective sequence unless M015 identifies a defect that cannot fit an amended M014 boundary. Do not split the work by subsystem merely to create more milestones.

## 7. Milestone 014 — Spec-constrained truthfulness and local hardening

Class: narrow correctness corrective pass

Plan:

- `plans/implementation/i2pcontrol-proposal-170/014-spec-constrained-truthfulness-and-local-hardening.md`

Objective:

- replace stale/current-state success with live source or compatible unavailable behavior;
- wire existing production metrics and tracing ring into I2PControl;
- make service generation fencing independent per category;
- resolve SAM sessions strictly to Proposal 170 without creating a SAM framework;
- add one local count bound before spawning TLS connection work;
- correct support documentation;
- leave unrelated hardened code and project infrastructure untouched.

Primary production boundary:

- `emissary-cli/src/i2pcontrol/**`

Permitted exceptions:

- minimal handle wiring in `emissary-cli/src/main.rs` and `emissary-cli/src/logger.rs`;
- one minimal bounded read-only core seam only when the exact contract cannot be met through existing handles or compatible unavailable behavior.

Exit conditions:

- all M014 acceptance criteria pass;
- targeted local package checks/tests pass;
- no new CI/release/verification infrastructure;
- no unrelated security rework;
- frozen implementation head recorded;
- registry moves M014 to `closing` and M015 to `ready`.

## 8. Milestone 015 — Focused independent reclosure

Class: narrow independent closure review

Plan:

- `plans/implementation/i2pcontrol-proposal-170/015-focused-independent-reclosure.md`

Objective:

Independently review the frozen M014 diff and affected Proposal 170 behavior without reopening the rest of the repository.

Review boundary:

- M014 changed files;
- affected RouterInfo, ClientServicesInfo, metrics/log composition, TLS connection bounds, and exact contract behavior;
- focused regressions and targeted local commands;
- absence of scope, CI, release, or unrelated hardening expansion.

Exit conditions:

- reviewer distinct from final M014 implementation agent;
- zero unresolved high/medium finding;
- exact contract remains unchanged except correction from false success to truthful current/unavailable behavior;
- no required evidence is replaced by static or tautological checks;
- closure record accepted before roadmap/registry return to `closed`.

## 9. File-scope policy

### Primary allowed production files

- `emissary-cli/src/i2pcontrol/**`

### Composition-only exceptions

- `emissary-cli/src/main.rs`
- `emissary-cli/src/logger.rs`

### Core exception

Only a small bounded read-only inspection seam directly required by Proposal 170 may touch `emissary-core`. It must not change router behavior, task ownership, persistence, protocols, selection logic, or security policy.

If implementation requires broad core edits, preserve explicit unavailable behavior and stop.

### Prohibited files and systems

- `.github/workflows/**`
- release and publishing automation
- frontend management code
- unrelated router/transport/NetDB/tunnel behavior
- general project security policy
- general task supervision
- project-wide verification frameworks

## 10. Verification policy

Verification is local, targeted, and proportional.

Required commands for M014/M015:

```bash
cargo fmt --all -- --check
cargo check -p emissary-cli --no-default-features --features i2pcontrol
cargo test -p emissary-cli --no-default-features --features i2pcontrol
cargo clippy -p emissary-cli --no-default-features --features i2pcontrol --all-targets -- -D warnings
```

Run equivalent core commands only if core is touched.

Do not add these commands to GitHub Actions as part of this workstream.

Not required:

- full unrelated workspace verification;
- UI builds blocked by desktop libraries;
- multi-platform CI;
- release dry runs;
- network farms;
- coverage gates;
- generated evidence bundles;
- repeated closure loops.

Tests should be a small set of falsifiable regressions added to existing suites where practical.

## 11. Completion definition

The workstream may return to `closed` only after M015 confirms:

- exact Proposal 170 contract adherence;
- current supported values are actually current;
- unavailable sources are explicit rather than fabricated;
- existing canonical metrics and log sources are wired;
- service observation is coherent per category;
- SAM follows the exact contract without unsafe or false state;
- TLS connection work is count-bounded before spawn;
- unsupported tunnel runtimes remain separate explicit stubs;
- no router behavior, frontend, resolver, CI, release, or broad security scope entered;
- zero unresolved high/medium findings.

The accurate completion statement remains:

> Emissary implements the complete Proposal 170 I2PControl API contract. Unsupported tunnel data planes are wired through explicit stubs and remain separate implementation work.

Documentation must qualify selectors that use Proposal-compatible unavailable behavior because Emissary has no safe canonical equivalent.

## 12. Milestone status

| Milestone | Status | Plan | Current disposition |
|---|---|---|---|
| 001 | historical closed | `plans/implementation/i2pcontrol-proposal-170/001-contract-matrix-and-i2pcontrol-foundation.md` | retained |
| 002 | historical closed | `plans/implementation/i2pcontrol-proposal-170/002-control-plane-domain-and-persistence.md` | retained |
| 003 | historical closed | `plans/implementation/i2pcontrol-proposal-170/003-address-book-administrative-api.md` | retained |
| 004 | historical closed | `plans/implementation/i2pcontrol-proposal-170/004-tunnel-manager-contract-and-stubs.md` | retained |
| 005 | superseded | `plans/implementation/i2pcontrol-proposal-170/005-router-info-inspection.md` | superseded by M009/M010/M014 |
| 006 | superseded | `plans/implementation/i2pcontrol-proposal-170/006-client-services-info.md` | superseded by M011/M014 |
| 007 | superseded | `plans/implementation/i2pcontrol-proposal-170/007-conformance-hardening-and-strict-closure.md` | superseded by M012/M013/M015 |
| 008 | historical closed | `plans/implementation/i2pcontrol-proposal-170/008-production-composition-and-durable-state-integrity.md` | retained; no reopening absent direct regression |
| 009 | historical closed | `plans/implementation/i2pcontrol-proposal-170/009-router-info-availability-and-truthfulness.md` | retained; M014 corrects residual sources |
| 010 | corrective pass required | `plans/implementation/i2pcontrol-proposal-170/010-bounded-core-router-inspection.md` | residual current-state defects owned by M014 |
| 011 | corrective pass required | `plans/implementation/i2pcontrol-proposal-170/011-client-services-live-state.md` | residual fencing/SAM defects owned by M014 |
| 012 | corrective pass required | `plans/implementation/i2pcontrol-proposal-170/012-real-tls-and-request-resource-hardening.md` | residual connection-bound evidence owned by M014 |
| 013 | corrective pass required | `plans/implementation/i2pcontrol-proposal-170/013-production-conformance-and-independent-reclosure.md` | superseded as final gate by M015 |
| 014 | ready | `plans/implementation/i2pcontrol-proposal-170/014-spec-constrained-truthfulness-and-local-hardening.md` | only active handoff |
| 015 | blocked | `plans/implementation/i2pcontrol-proposal-170/015-focused-independent-reclosure.md` | blocked on frozen M014 head and reviewer independence |
