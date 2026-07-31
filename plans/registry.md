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
| I2PControl Proposal 170 | active narrow corrective work | `plans/subsystems/i2pcontrol-proposal-170-roadmap.md` | M016 blocked | SAM active-session contract is pinned, but the exact i2pd map cannot be populated through an allowed Emissary owner seam |

## Dependency-ready implementation plans

| Subsystem | Milestone | Status | Implementation plan | Dependencies |
|---|---|---|---|---|
| — | — | — | — | — |

## Active closure work

| Subsystem | Milestone | Status | Implementation plan | Evidence commit | Closure record |
|---|---|---|---|---|---|
| — | — | — | — | — | — |

## Blocked implementation plans

| Subsystem | Milestone | Status | Implementation plan | Blocker |
|---|---|---|---|---|
| I2PControl Proposal 170 | 016 — SAM, fencing, and connection-proof corrective pass | blocked | `plans/implementation/i2pcontrol-proposal-170/016-sam-fencing-and-connection-proof-corrective-pass.md` | Official Proposal 170 and pinned i2pd require active SAM session information (`name`, `address`, `sockets`), but safely supplying it requires a new shared owner/observer seam forbidden by M016; no permitted existing unavailable response is defined |
| I2PControl Proposal 170 | 017 — final-head independent reclosure | blocked | `plans/implementation/i2pcontrol-proposal-170/017-final-head-independent-reclosure.md` | M016 remains blocked; no frozen complete implementation head or accepted contract disposition |

## M016/M017 scope guard

M016/M017 are confined to exact Proposal 170 correctness.

Primary production boundary:

- `emissary-cli/src/i2pcontrol/client_services.rs`
- `emissary-cli/src/i2pcontrol/service_registry.rs`
- `emissary-cli/src/i2pcontrol/server.rs`
- the smallest existing I2PControl DTO/control trait file required by the exact SAM mapping

Permitted exception:

- one minimal bounded read-only SAM-owner accessor only if the official Proposal 170 and pinned adopted i2pd implementation establish an exact active-session map that cannot be populated through an existing handle.

The workstream must not add or modify:

- `.github/workflows/**`;
- CI jobs, nightly checks, platform matrices, required checks, coverage gates, or generated evidence bundles;
- release, crates.io, GitHub publishing, packaging, or version automation;
- router, transport, NetDB, tunnel-pool, peer-selection, congestion, cryptographic, or general security architecture;
- frontend management surfaces;
- runtime address-book precedence;
- missing tunnel data planes or BOB;
- general metrics, logging, task-supervision, inspection, service-registry, connection-management, or verification frameworks;
- unrelated files through a repository-wide formatting pass.

## Corrective finding ownership

| Finding | Severity | Owner |
|---|---|---|
| Active SAM can return successful empty sessions when the current source is unavailable, without pinned contract authority | medium, contract-dependent | M016 |
| Same-category service generation validation and entry replacement are not atomic | medium | M016 |
| TLS connection-limit test does not exceed the configured limit | medium evidence defect | M016 |
| M015 reviewed an older head and claimed zero findings despite the above defects | closure defect | M017 |
| Post-M015 broad formatting commit was outside the frozen review | scope/evidence reconciliation | M017 |
| Reviewer independence was not auditable beyond a generic assertion | closure defect | M017 |

## Deferred and out-of-scope work

The following remain intentionally outside Proposal 170 corrective scope:

- real runtime implementations for missing tunnel types;
- lifecycle migration of existing startup-managed tunnels;
- runtime resolver adoption and precedence for Proposal administrative address books;
- frontend management surfaces;
- protocol additions beyond Proposal 170;
- broader router, NetDB, transport, tunnel, peer-selection, congestion, cryptographic, or security work;
- release or publishing automation;
- CI expansion;
- cleanup or reversal of unrelated repository-wide formatting unless its own owner determines that separately.

## Historical records and current authority

M001–M004 remain historical foundations.

M005–M007 are superseded.

M008 materially corrected production composition and is not reopened absent a direct regression.

M009 remains the availability/error boundary.

M010–M012 residual truthfulness/resource defects were substantially corrected by M014.

M013 was superseded by M015.

M014 materially landed but its strict closure is reopened only for the three M016 findings.

M015 remains a historical closure record but is not the current closure authority. M017 supersedes it as the final gate.

| Milestone | Historical closure | Current disposition |
|---|---|---|
| 008 | `plans/closure/i2pcontrol-proposal-170/008-closure.md` | retained |
| 009 | `plans/closure/i2pcontrol-proposal-170/009-closure.md` | retained |
| 010 | `plans/closure/i2pcontrol-proposal-170/010-closure.md` | residual issues substantially corrected by M014 |
| 011 | `plans/closure/i2pcontrol-proposal-170/011-closure.md` | M016 owns remaining SAM/fencing defects |
| 012 | `plans/closure/i2pcontrol-proposal-170/012-closure.md` | M016 owns remaining saturation-proof defect |
| 013 | `plans/closure/i2pcontrol-proposal-170/013-closure.md` | superseded |
| 014 | no standalone accepted final closure | materially landed; narrow corrective pass M016 required |
| 015 | `plans/closure/i2pcontrol-proposal-170/015-closure.md` | strict closure invalidated; superseded by M017 |

## Registry maintenance rules

1. Register only dependency-ready work as `ready`.
2. Keep prewritten dependent work `blocked` until its exact activation rule is satisfied.
3. Move implementation to `closing`, not `closed`, when code lands.
4. Final closure review must be independent from the final implementation executor and auditable in the closure record.
5. Mark `closed` only with an accepted closure record at the actual final reviewed head and zero unresolved high/medium findings.
6. Do not conceal implementation or contract defects behind `conditionally closed`.
7. Preserve exact scope and blocker ownership.
8. Do not expand planning into CI, release, broad security, or repository-wide formatting loops.
9. For M016/M017, targeted local package verification is sufficient; absence of remote CI is not itself a blocker.
10. If the SAM contract cannot be established exactly, stop M016 with a named blocker rather than inventing wire behavior.
11. If M017 rejects closure, amend M016 narrowly before creating another milestone.
12. Do not rewrite M015 into a passing closure; preserve it as historical evidence.
