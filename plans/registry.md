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
- **corrective pass required** — a prior closure was invalidated by a material implementation or evidence defect.
- **superseded** — replaced by another document.
- **archived** — inactive and retained for traceability.

## Active subsystem roadmaps

| Subsystem | Status | Roadmap | Current milestone | Dependencies or blockers |
|---|---|---|---|---|
| I2PControl Proposal 170 | active corrective work | `plans/subsystems/i2pcontrol-proposal-170-roadmap.md` | M008 ready | Prior M005–M007 strict closure invalidated by production truthfulness, shared-state, live-service, TLS, resource-evidence, and review-independence defects |

## Dependency-ready implementation plans

| Subsystem | Milestone | Status | Implementation plan | Dependencies |
|---|---|---|---|---|
| I2PControl Proposal 170 | 009 — RouterInfo availability and truthfulness | ready | `plans/implementation/i2pcontrol-proposal-170/009-router-info-availability-and-truthfulness.md` | M008 strict closure and baseline reconciliation |

## Active closure work

| Subsystem | Milestone | Status | Implementation plan | Evidence commit | Closure record |
|---|---|---|---|---|---|
| No active closure work. | — | — | — | — | — |

## Blocked implementation plans

These plans are written for full-workstream handoff but MUST NOT execute until the named activation rule is satisfied and the registry moves the plan to `ready`.

| Subsystem | Milestone | Status | Implementation plan | Blocker |
|---|---|---|---|---|
| I2PControl Proposal 170 | 009 — RouterInfo availability and truthfulness | ready | `plans/implementation/i2pcontrol-proposal-170/009-router-info-availability-and-truthfulness.md` | None (M008 closed) |
| I2PControl Proposal 170 | 010 — bounded core router inspection | blocked | `plans/implementation/i2pcontrol-proposal-170/010-bounded-core-router-inspection.md` | M009 strict closure and stable selector source map |
| I2PControl Proposal 170 | 011 — ClientServicesInfo live state | blocked | `plans/implementation/i2pcontrol-proposal-170/011-client-services-live-state.md` | M008 and M009 strict closure; reconcile with M010 integration head before final closure |
| I2PControl Proposal 170 | 012 — real TLS and request resource hardening | blocked | `plans/implementation/i2pcontrol-proposal-170/012-real-tls-and-request-resource-hardening.md` | M008/M009 interfaces stable; activate after M009, may run parallel with M010/M011 |
| I2PControl Proposal 170 | 013 — production conformance and independent reclosure | blocked | `plans/implementation/i2pcontrol-proposal-170/013-production-conformance-and-independent-reclosure.md` | M010, M011, and M012 strict closure; M001–M004 revalidation; reviewer independence |

## Deferred unregistered work

The following work remains intentionally outside the active Proposal 170 corrective handoff and MUST NOT be pulled into M008–M013:

- real runtime implementations for missing tunnel types;
- lifecycle migration of existing startup-managed tunnels;
- runtime resolver adoption and precedence for Proposal 170 address books;
- frontend management surfaces;
- protocol additions beyond Proposal 170;
- broader router, NetDB, transport, tunnel, peer-selection, or congestion behavior changes;
- release or publishing automation.

## Corrective finding ownership

| Finding | Severity at reopening | Owning milestone |
|---|---|---|
| Production store failures fall back to fake controls | high | M008 |
| RouterInfo and handlers do not share one loaded tunnel control | high | M008 |
| Query failures are suppressed into empty/absent/zero state | high | M008/M009 |
| RouterInfo hard-coded/default production snapshots | high | M009/M010 |
| Missing bounded NetDB/peer/transport/tunnel inspection | medium/high by selector | M010 |
| ClientServices I2PTunnel inventory stale until restart | medium | M011 |
| SAM sessions always empty and proxy starting state can appear enabled | medium | M011 |
| TLS acceptor discarded; raw listener served as plaintext | high | M012 |
| Body/resource limits enforced after buffering or not proven | high | M012 |
| Tautological adversarial tests counted as evidence | high evidence defect | M012/M013 |
| Final closure was not independent | high closure defect | M013 |

## Recently closed or historically recorded work

These rows preserve traceability. M005–M007 historical closure records are not accepted as current strict closure after the corrective review.

| Subsystem | Milestone | Historical closure record | Historical reviewed commit | Current follow-up |
|---|---|---|---|---|
| I2PControl Proposal 170 | 008 — production composition and durable-state integrity | `plans/closure/i2pcontrol-proposal-170/008-closure.md` | `b35d9ad` baseline | Strictly closed; M009 may activate |
| I2PControl Proposal 170 | 001 — contract matrix and I2PControl foundation | `plans/closure/i2pcontrol-proposal-170/001-closure.md` | `9b43484a21d5a1291c4881cdae62a36c527f8c0f` | Revalidate at M013; TLS correction owned by M012 |
| I2PControl Proposal 170 | 002 — control-plane domain and persistence | `plans/closure/i2pcontrol-proposal-170/002-closure.md` | `6c92a71` | Revalidate at M013; production composition owned by M008 |
| I2PControl Proposal 170 | 003 — AddressBook administrative API | `plans/closure/i2pcontrol-proposal-170/003-closure.md` | `9d2f646` | Revalidate fail-closed behavior at M008/M013 |
| I2PControl Proposal 170 | 004 — TunnelManager contract and explicit stubs | `plans/closure/i2pcontrol-proposal-170/004-closure.md` | `595036b` | Revalidate shared service/live visibility at M008/M011/M013 |
| I2PControl Proposal 170 | 005 — RouterInfo inspection | `plans/closure/i2pcontrol-proposal-170/005-closure.md` | historical HEAD | corrective pass required; M009/M010 |
| I2PControl Proposal 170 | 006 — ClientServicesInfo | `plans/closure/i2pcontrol-proposal-170/006-closure.md` | historical HEAD | corrective pass required; M011 |
| I2PControl Proposal 170 | 007 — conformance and strict closure | `plans/closure/i2pcontrol-proposal-170/007-closure.md` | `d708d30818c0f09b9b1d50131b2ff61a66a8b246` | superseded as closure gate by M012/M013 |

## Registry maintenance rules

1. Add a subsystem roadmap only when it is active enough to reason about.
2. Register an implementation plan as ready only after dependency and handoff review.
3. Prewritten future plans remain blocked until their explicit activation rule is satisfied.
4. Before activating a prewritten plan, update its repository baseline and reconcile it against dependency closure evidence and current code.
5. Move a ready plan to active when implementation starts.
6. Move it to closing when production work lands and independent closure review begins.
7. Mark it closed only when the linked closure record says closed and no unresolved high- or medium-severity finding remains.
8. Use conditionally closed only when named external or operational evidence prevents strict closure without concealing an implementation defect.
9. Use corrective pass required when a later review invalidates a prior closure.
10. Record blockers precisely and link the document that owns resolution.
11. Move closed rows out of active sections after recording them under recently closed work.
12. Preserve traceability when archiving or superseding.
13. Do not copy detailed milestone requirements into this registry.
14. When one milestone closes, activate only the next dependency-ready handoff or explicitly approved parallel set.
15. Final closure review must be independent from the final implementation agent.