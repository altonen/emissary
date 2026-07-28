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
| I2PControl Proposal 170 | active | `plans/subsystems/i2pcontrol-proposal-170-roadmap.md` | M004 ready | M001–M003 closed |

## Dependency-ready implementation plans

| Subsystem | Milestone | Status | Implementation plan | Dependencies |
|---|---|---|---|---|
| I2PControl Proposal 170 | 005 — RouterInfo inspection and exact selectors | ready | `plans/implementation/i2pcontrol-proposal-170/005-router-info-inspection.md` | M001–M003 closed; M003 address-book selectors integrated |

## Active closure work

| Subsystem | Milestone | Status | Implementation plan | Evidence commit | Closure record |
|---|---|---|---|---|---|
No active closure work.
No active closure work.

## Blocked implementation plans

These plans are intentionally prewritten for full-workstream handoff and dependency visibility. They are not authorized for execution until their activation rules are satisfied and the registry moves them to `ready`.

| Subsystem | Milestone | Status | Implementation plan | Blocker |
|---|---|---|---|---|
| I2PControl Proposal 170 | 006 — ClientServicesInfo | blocked | `plans/implementation/i2pcontrol-proposal-170/006-client-services-info.md` | M004 and M005 strict closure |
| I2PControl Proposal 170 | 007 — conformance, hardening, and strict closure | blocked | `plans/implementation/i2pcontrol-proposal-170/007-conformance-hardening-and-strict-closure.md` | M003–M006 strict closure and activation audit |

## Deferred unregistered work

The following work is intentionally outside the active Proposal 170 handoff and MUST NOT be pulled into any registered milestone:

- real runtime implementations for missing tunnel types;
- lifecycle migration of existing startup-managed tunnels;
- runtime resolver adoption and precedence for Proposal 170 address books;
- frontend management surfaces;
- protocol additions beyond Proposal 170;
- broader router, NetDB, transport, or tunnel behavior changes.

## Recently closed or conditionally closed work

| Subsystem | Milestone | Closure record | Closed/reviewed at commit | Follow-up |
|---|---|---|---|---|
| I2PControl Proposal 170 | 001 — contract matrix and I2PControl foundation | `plans/closure/i2pcontrol-proposal-170/001-closure.md` | `9b43484a21d5a1291c4881cdae62a36c527f8c0f` | M002 activated and closed |
| I2PControl Proposal 170 | 002 — control-plane domain and persistence | `plans/closure/i2pcontrol-proposal-170/002-closure.md` | `6c92a71` | M003 and M004 activated |
| I2PControl Proposal 170 | 003 — AddressBook administrative API | `plans/closure/i2pcontrol-proposal-170/003-closure.md` | `9d2f646` | M004 and M005 activated |
| I2PControl Proposal 170 | 004 — TunnelManager contract and explicit stubs | `plans/closure/i2pcontrol-proposal-170/004-closure.md` | `595036b` | M005 activated; M006 remains blocked on M005 |

## Registry maintenance rules

1. Add a subsystem roadmap only when it is active enough to reason about.
2. Register an implementation plan as ready only after dependency and handoff review.
3. Prewritten future plans remain blocked until their explicit activation rule is satisfied.
4. Before activating a prewritten plan, update its repository baseline and reconcile it against dependency closure evidence and current code.
5. Move a ready plan to active when implementation starts.
6. Move it to closing when production work lands and independent closure review begins.
7. Mark it closed only when the linked closure record says closed and no unresolved high- or medium-severity finding remains.
8. Use conditionally closed only when named external or operational evidence prevents strict closure without concealing an implementation defect.
9. Record blockers precisely and link the document that owns resolution.
10. Move closed rows out of active sections after recording them under recently closed work.
11. Preserve traceability when archiving.
12. Do not copy detailed milestone requirements into this registry.
13. When one milestone closes, activate only the next dependency-ready handoff or explicitly approved parallel set.
