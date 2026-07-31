# Emissary Active Planning Registry

This file is the compact control surface for active planning.

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
- **closed against pinned revision** — closure accepted against an explicitly named revision of an open external specification.
- **corrective pass required** — a prior closure was invalidated by a material implementation or evidence defect.
- **superseded** — replaced by another document.
- **archived** — inactive and retained for traceability.

## Active subsystem roadmaps

| Subsystem | Status | Roadmap | Current milestone | Dependencies or blockers |
|---|---|---|---|---|
| I2PControl Proposal 170 | closed against pinned revision | `plans/subsystems/i2pcontrol-proposal-170-roadmap.md` | M019 accepted at `db5e067` | Proposal 170 remains Open; closure is bound to the 2026-05-20 revision |

## Dependency-ready implementation plans

| Subsystem | Milestone | Status | Implementation plan | Dependencies |
|---|---|---|---|---|
| — | — | — | — | — |

## Recently closed milestones

| Subsystem | Milestone | Status | Implementation plan | Evidence commit | Closure record |
|---|---|---|---|---|---|
| I2PControl Proposal 170 | 018 — exact wire-contract reconciliation | closed | `plans/implementation/i2pcontrol-proposal-170/018-exact-wire-contract-reconciliation.md` | `ea35de9` | `plans/closure/i2pcontrol-proposal-170/018-implementation-disposition.md` |
| I2PControl Proposal 170 | 019 — pinned-revision independent reclosure | closed against pinned revision | `plans/implementation/i2pcontrol-proposal-170/019-pinned-revision-independent-reclosure.md` | `db5e067` | `plans/closure/i2pcontrol-proposal-170/019-closure.md` |

## Blocked plans

| Subsystem | Milestone | Status | Plan | Blocker |
|---|---|---|---|---|
| — | — | — | — | — |

## Current exact-contract findings

| Finding | Severity | Owner | State |
|---|---|---|---|
| Exact Proposal 170 RouterInfo keys are absent or renamed, and a 121-key legacy catalog is mislabeled as Proposal 170 | high | M018 | resolved and independently accepted by M019 at `db5e067` |
| AddressBook canonical `Type`/`Hostname`/`Destination`/optional `Delete` and in-method `SetSubscriptions`/`SetConfig` modes are missing | high | M018 | resolved and independently accepted by M019 at `db5e067` |
| TunnelManager lowercase actions and structured result envelopes are missing; `List` and capitalized actions are extensions | high | M018 | resolved and independently accepted by M019 at `db5e067` |
| ClientServicesInfo direct parameter-by-presence form is missing | high | M018 | resolved and independently accepted by M019 at `db5e067` |
| Real-session-to-production-ClientServicesInfo SAM evidence is incomplete | informational evidence limitation | M018/M019 | accepted bounded environmental limitation; closest-production composition and retained lifecycle evidence recorded |
| Wire, source, and runtime support claims are conflated | medium documentation | M018 | resolved and independently accepted by M019 at `db5e067` |
| M017 claimed zero unresolved high/medium findings | closure defect | M019 | superseded by independent M019 closure; historical record preserved |

## Pinned Proposal 170 authority

Current corrective work is pinned to:

- proposal: `I2PControl Expansion`, Proposal 170;
- status: `Open`;
- created: `2026-05-20`;
- last updated: `2026-05-20`;
- canonical page: `https://i2p.net/en/proposals/170-i2pcontrol-expansion/`.

Because the proposal is Open, final closure must recheck the source and state `closed against pinned revision`. A changed proposal revision blocks M019 until implementation and fixtures are reconciled.

## M018 scope guard

M018 owns exact public wire reconciliation only:

- exact 43 RouterInfo additions, direct parameter presence, response keys, and JSON types;
- exact AddressBook operation modes and response adjudication;
- exact lowercase TunnelManager actions, option inventory, and structured results;
- exact direct-parameter ClientServicesInfo requests;
- compatibility alias separation;
- literal official-example fixtures;
- one strongest-available SAM production-composition integration path;
- directly affected conformance/support documents.

Existing compatibility forms may remain, but they must be labeled extensions and must not substitute for canonical Proposal 170 forms.

M018 must not add:

- `.github/workflows/**`, CI/nightly/platform/coverage/evidence machinery;
- release, publishing, packaging, or version automation;
- missing tunnel data planes;
- broad router, transport, NetDB, peer, tunnel, cryptographic, resolver, frontend, SAM, or I2CP redesign;
- generic protocol/schema/fixture/inspection frameworks;
- repository-wide formatting;
- fabricated values for unavailable sources.

## Historical records and current authority

M001–M004 remain historical implementation foundations. M005–M007 are superseded. M008–M014 and M016 contain retained implementation evidence. M015 remains an invalid historical closure. M017's SAM/fencing/TLS review evidence is retained, but its broad Proposal 170 closure is invalidated.

| Milestone | Record | Current disposition |
|---|---|---|
| 014 | `plans/closure/i2pcontrol-proposal-170/014-closure.md` | implementation retained; broad final acceptance reopened by M018/M019 |
| 015 | `plans/closure/i2pcontrol-proposal-170/015-closure.md` | invalid historical closure |
| 016 | `plans/closure/i2pcontrol-proposal-170/016-implementation-disposition.md` | bounded SAM implementation retained |
| 017 | `plans/closure/i2pcontrol-proposal-170/017-closure.md` and `017-closure-invalidation.md` | closure invalidated; historical evidence only |
| 018 | `plans/implementation/i2pcontrol-proposal-170/018-exact-wire-contract-reconciliation.md` | closing; frozen head `ea35de9`; disposition recorded |
| 019 | `plans/implementation/i2pcontrol-proposal-170/019-pinned-revision-independent-reclosure.md` | ready final gate |

## Registry maintenance rules

1. M018 implementation is frozen and accepted; M019 is the completed independent review.
2. Final Proposal 170 subsystem closure is recorded against the pinned revision.
3. Do not count compatibility aliases, legacy/base keys, unavailable sources, or unsupported runtimes as canonical operational coverage.
4. Preserve M017 and its invalidation as history; do not rewrite M017 into passing evidence.
5. If the open Proposal 170 source changes, rebase the contract manifest before closure.
6. M018 is `closed` and M019 is `closed against pinned revision` after all applicable acceptance criteria passed.
7. M019 used a distinct auditable review run and independently compared literal fixtures to the pinned source.
8. Any future high/medium finding within this boundary returns to M018; no duplicate corrective milestone is created.
9. Final status is `closed against pinned revision` with zero unresolved high/medium findings.
10. Verification remains local and package-scoped; remote CI is not required.
11. Do not expand this corrective line into CI, release, broad security, missing tunnel runtime, or generic framework work.
