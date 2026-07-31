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
- **corrective pass required** — a prior closure was invalidated by a material implementation or evidence defect.
- **superseded** — replaced by another document.
- **archived** — inactive and retained for traceability.

## Active subsystem roadmaps

| Subsystem | Status | Roadmap | Current milestone | Dependencies or blockers |
|---|---|---|---|---|
| I2PControl Proposal 170 | closed | `plans/subsystems/i2pcontrol-proposal-170-roadmap.md` | M017 closed | final-head review accepted; zero unresolved high/medium findings |

## Dependency-ready implementation plans

| Subsystem | Milestone | Status | Implementation plan | Dependencies |
|---|---|---|---|---|
| I2PControl Proposal 170 | none | — | — | No active dependency-ready implementation plan; workstream closed by M017 |

## Active closure work

| Subsystem | Milestone | Status | Implementation plan | Evidence commit | Closure record |
|---|---|---|---|---|---|
| I2PControl Proposal 170 | none | — | — | — | No active closure work |

## Blocked plans

| Subsystem | Milestone | Status | Plan | Blocker |
|---|---|---|---|---|
| I2PControl Proposal 170 | none | — | — | — |

## M016 scope guard

M016 now owns one remaining finding only:

| Finding | Severity | Owner | State |
|---|---|---|---|
| Listening SAM can return successful empty sessions while active sessions exist because no canonical read source reaches I2PControl | medium | M016 | resolved at `355e243`; accepted by M017 |

Resolved at `9047feecde046dac8e0208bbf1acf2e3883f97ae` and not active:

- same-category service generation validation/write race;
- below-limit TLS connection test and missing saturation/restoration proof.

### Allowed production boundary

- `emissary-core/src/sam/mod.rs`
- `emissary-core/src/sam/session.rs`
- one existing SAM stream/socket module only when required for the exact sanitized socket summary
- the smallest router/composition file that constructs and moves `SamServer`
- `emissary-cli/src/i2pcontrol/client_services.rs`
- the smallest existing I2PControl DTO/control trait and composition seam required to pass the read handle

### Authorized exception

One fixed-capacity, read-only SAM session-observation handle is authorized. Its private writer may update sanitized metadata only at existing session/socket lifecycle transitions.

The handle must not expose lifecycle authority, mutable session handles, sockets, stream objects, private keys, destinations, payloads, authentication data, or command channels.

### Prohibited

- `.github/workflows/**` and CI/nightly/platform/coverage/evidence expansion;
- release, publishing, packaging, or version automation;
- repository-wide formatting;
- SAM protocol, routing, tunnel, listener, or lifecycle redesign;
- generic observer/event/cache/registry/polling/persistence frameworks;
- unrelated router, transport, NetDB, tunnel, frontend, resolver, cryptographic, or security work;
- Proposal 170 wire extensions.

## Historical records and authority

M001–M004 remain historical foundations. M005–M007 are superseded. M008–M009 remain retained. M010–M012 residual behavior was materially corrected by M014 and `9047fee`. M013 is superseded.

| Milestone | Closure/current record | Current disposition |
|---|---|---|
| 014 | `plans/closure/i2pcontrol-proposal-170/014-closure.md` and `plans/closure/i2pcontrol-proposal-170/017-closure.md` | closed by M017 after the SAM truthfulness finding was resolved |
| 015 | `plans/closure/i2pcontrol-proposal-170/015-closure.md` | strict closure invalidated; superseded by M017 |
| 016 | `plans/closure/i2pcontrol-proposal-170/016-implementation-disposition.md` and `plans/closure/i2pcontrol-proposal-170/017-closure.md` | closed at final reviewed head `dbbd107`; pre-amendment blocker retained as history |
| 017 | `plans/closure/i2pcontrol-proposal-170/017-closure.md` | closed; final-head independent review accepted |

## Registry maintenance rules

1. M016 and M017 are closed by the accepted final-head closure record.
2. Keep the historical M015 and pre-amendment M016 records unchanged as history.
3. The subsystem remains closed unless a new finding is recorded against a new head.
4. Mark the subsystem closed only with zero unresolved high/medium findings.
5. Do not add another milestone for defects that fit the amended M016 boundary.
6. Do not conceal a field-level SAM blocker with empty, partial, stale, or default success.
7. Verification remains local and package-scoped; remote CI is not required.
8. Preserve M015 and the pre-amendment M016 blocker record as historical evidence rather than rewriting them into passing closure records.
