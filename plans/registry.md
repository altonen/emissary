# Emissary Active Planning Registry

This file is the compact control surface for active planning. It links current documents and blockers without duplicating detailed requirements.

Canonical direction:

- `plans/000-long-term-specification.md`
- `plans/001-terminology-and-domain-model.md`
- `plans/002-long-term-roadmap.md`
- `plans/003-planning-process.md`

## Status vocabulary

- **proposed** — document exists but is not approved for execution.
- **ready** — dependencies and interfaces are satisfied; plan may be handed off.
- **active** — implementation or closure work is in progress.
- **blocked** — a named dependency or evidence requirement prevents progress.
- **closing** — implementation landed and independent closure evidence is being gathered.
- **closed** — closure record accepted.
- **conditionally closed** — substantial work landed, but a genuinely external limitation prevents strict closure.
- **corrective pass required** — a prior closure was invalidated by a material implementation or evidence defect.
- **superseded** — replaced by another document.
- **archived** — inactive and retained for traceability.

## Active subsystem roadmaps

| Subsystem | Status | Roadmap | Current milestone | Dependencies or blockers |
|---|---|---|---|---|
| I2PControl Proposal 170 | active narrow corrective work | `plans/subsystems/i2pcontrol-proposal-170-roadmap.md` | M014 closing | M015 blocked on frozen M014 head and independent reviewer |

## Dependency-ready implementation plans

| Subsystem | Milestone | Status | Implementation plan | Dependencies |
|---|---|---|---|---|
| I2PControl Proposal 170 | 014 — spec-constrained truthfulness and local hardening | closing | `plans/implementation/i2pcontrol-proposal-170/014-spec-constrained-truthfulness-and-local-hardening.md` | implementation landed; frozen head for M015 independent review |

## Active closure work

| Subsystem | Milestone | Status | Implementation plan | Evidence commit | Closure record |
|---|---|---|---|---|---|
| — | — | — | — | — | — |

## Blocked implementation plans

| Subsystem | Milestone | Status | Implementation plan | Blocker |
|---|---|---|---|---|
| I2PControl Proposal 170 | 015 — focused independent reclosure | blocked | `plans/implementation/i2pcontrol-proposal-170/015-focused-independent-reclosure.md` | M014 closing; frozen reviewed head required; reviewer distinct from final implementation agent |

## Corrective scope guard

M014/M015 are confined to exact Proposal 170 correctness.

Primary production boundary:

- `emissary-cli/src/i2pcontrol/**`

Permitted exceptions:

- minimal composition wiring in `emissary-cli/src/main.rs` and `emissary-cli/src/logger.rs`;
- one minimal bounded read-only core seam only when the exact Proposal contract cannot be met through existing handles or compatible unavailable behavior.

The workstream must not add or modify:

- `.github/workflows/**`;
- CI jobs, platform matrices, required checks, coverage gates, or generated evidence bundles;
- release, crates.io, GitHub publishing, or packaging automation;
- frontend management surfaces;
- runtime address-book precedence;
- missing tunnel data planes or BOB;
- router, transport, NetDB, tunnel-pool, peer-selection, congestion, or general security architecture;
- general metrics, logging, task-supervision, inspection, or verification frameworks.

## Corrective finding ownership

| Finding | Severity | Owner |
|---|---|---|
| Startup `CoreSnapshot` presented as current RouterInfo state | medium | M014 |
| UDP/TCP active inferred from aggregate connected-router count | medium | M014 |
| bandwidth/recent traffic selectors read disconnected local state | medium | M014 |
| RouterInfo uses a fresh ring instead of tracing-backed application ring | medium | M014 |
| service observer generation fencing is global across categories | medium | M014 |
| active SAM sessions can appear as unconditional empty success | medium, contract-dependent | M014 |
| TLS connection tasks not count-bounded before spawn | medium | M014 |
| M013 closure and support docs overstate strict completion | closure defect | M015 |

## Deferred unregistered work

The following remain intentionally outside Proposal 170 corrective scope:

- real runtime implementations for missing tunnel types;
- lifecycle migration of existing startup-managed tunnels;
- runtime resolver adoption and precedence for Proposal administrative address books;
- frontend management surfaces;
- protocol additions beyond Proposal 170;
- broader router, NetDB, transport, tunnel, peer-selection, congestion, or security work;
- release or publishing automation;
- CI expansion.

## Historical records

M001–M004 remain historical foundations.

M005–M007 are superseded.

M008 materially corrected production composition and is not reopened absent a direct regression.

M009 remains the availability/error boundary; M014 corrects residual source wiring.

M010–M012 have residual corrective findings owned by M014.

M013 is no longer accepted as the current strict closure gate and is superseded by M015.

| Milestone | Historical closure | Current disposition |
|---|---|---|
| 008 | `plans/closure/i2pcontrol-proposal-170/008-closure.md` | retained |
| 009 | `plans/closure/i2pcontrol-proposal-170/009-closure.md` | retained with residual M014 corrections |
| 010 | `plans/closure/i2pcontrol-proposal-170/010-closure.md` | corrective pass required; M014 |
| 011 | `plans/closure/i2pcontrol-proposal-170/011-closure.md` | corrective pass required; M014 |
| 012 | `plans/closure/i2pcontrol-proposal-170/012-closure.md` | corrective pass required; M014 |
| 013 | `plans/closure/i2pcontrol-proposal-170/013-closure.md` | strict closure invalidated; superseded by M015 |

## Registry maintenance rules

1. Register only dependency-ready work as `ready`.
2. Keep prewritten dependent work `blocked` until its exact activation rule is satisfied.
3. Move implementation to `closing`, not `closed`, when code lands.
4. Final closure review must be independent from the final implementation agent.
5. Mark `closed` only with an accepted closure record and zero unresolved high/medium findings.
6. Do not conceal implementation defects behind `conditionally closed`.
7. Preserve exact scope and blocker ownership.
8. Do not expand planning into CI, release, or broad security loops.
9. For M014/M015, targeted local package verification is sufficient; absence of remote CI is not itself a blocker.
10. If M015 rejects closure, amend M014 narrowly before creating additional milestones.
