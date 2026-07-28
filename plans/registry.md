# Emissary Active Planning Registry

This file is the compact control surface for active planning. It links current documents and blockers without duplicating their detailed requirements.

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
- **closing** — implementation landed and closure evidence is being gathered.
- **closed** — closure record accepted.
- **conditionally closed** — substantial work landed, but a named correctness or evidence finding prevents strict closure.
- **superseded** — replaced by another document.
- **archived** — inactive and retained for traceability.

## Active subsystem roadmaps

| Subsystem | Status | Roadmap | Current milestone | Dependencies or blockers |
|---|---|---|---|---|
| I2PControl Proposal 170 | active | `plans/subsystems/i2pcontrol-proposal-170-roadmap.md` | M001 ready | No hard dependency; ADR-0001 records the contract/stub boundary |

## Dependency-ready implementation plans

| Subsystem | Milestone | Status | Implementation plan | Dependencies |
|---|---|---|---|---|
| I2PControl Proposal 170 | 001 — contract matrix and I2PControl foundation | ready | `plans/implementation/i2pcontrol-proposal-170/001-contract-matrix-and-i2pcontrol-foundation.md` | Canonical documents and ADR-0001 present; repository baseline recorded in the plan |

## Active closure work

| Subsystem | Milestone | Status | Implementation plan | Evidence commit | Closure record |
|---|---|---|---|---|---|

No closure work is active. M001 has not been implemented.

## Blocked work

| Subsystem | Milestone | Status | Implementation plan | Blocker |
|---|---|---|---|---|

No registered implementation plan is blocked.

## Deferred unregistered work

The following work is intentionally outside the active Proposal 170 handoff and MUST NOT be pulled into M001:

- real runtime implementations for missing tunnel types;
- lifecycle migration of existing startup-managed tunnels;
- runtime resolver adoption and precedence for Proposal 170 address books;
- frontend management surfaces;
- protocol additions beyond Proposal 170;
- broader router, NetDB, transport, or tunnel behavior changes.

## Recently closed or conditionally closed work

| Subsystem | Milestone | Closure record | Closed/reviewed at commit | Follow-up |
|---|---|---|---|---|

No Proposal 170 milestone has closed.

## Registry maintenance rules

1. Add a subsystem roadmap only when it is active enough to reason about.
2. Register an implementation plan as ready only after dependency and handoff review.
3. Move it to active when implementation starts.
4. Move it to closing when production work lands and independent closure review begins.
5. Mark it closed only when the linked closure record says closed and no unresolved high- or medium-severity finding remains.
6. Use conditionally closed when named evidence or correctness findings prevent strict closure.
7. Record blockers precisely and link the document that owns resolution.
8. Move closed rows out of active sections after recording them under recently closed work.
9. Preserve traceability when archiving.
10. Do not copy detailed milestone requirements into this registry.
11. When one milestone closes, create and register only the next dependency-ready handoff.
