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
| I2PControl Proposal 170 | active narrow corrective work | `plans/subsystems/i2pcontrol-proposal-170-roadmap.md` | M016 closing | bounded SAM observation implemented; M017 independent reclosure ready |

## Dependency-ready implementation plans

| Subsystem | Milestone | Status | Implementation plan | Dependencies |
|---|---|---|---|---|
| I2PControl Proposal 170 | 016 — bounded SAM session observation corrective pass | closing | `plans/implementation/i2pcontrol-proposal-170/016-sam-fencing-and-connection-proof-corrective-pass.md` | implementation frozen at `355e243`; closure evidence in `plans/closure/i2pcontrol-proposal-170/016-implementation-disposition.md` |

## Active closure work

| Subsystem | Milestone | Status | Implementation plan | Evidence commit | Closure record |
|---|---|---|---|---|---|
| I2PControl Proposal 170 | 016 | closing | `plans/implementation/i2pcontrol-proposal-170/016-sam-fencing-and-connection-proof-corrective-pass.md` | `355e243` | `plans/closure/i2pcontrol-proposal-170/016-implementation-disposition.md` |

## Blocked plans

| Subsystem | Milestone | Status | Plan | Blocker |
|---|---|---|---|---|
| I2PControl Proposal 170 | 017 — final-head independent reclosure | ready | `plans/implementation/i2pcontrol-proposal-170/017-final-head-independent-reclosure.md` | review frozen M016 head `355e243`; reviewer must be distinct and auditable |

## M016 scope guard

M016 now owns one remaining finding only:

| Finding | Severity | Owner | State |
|---|---|---|---|
| Listening SAM can return successful empty sessions while active sessions exist because no canonical read source reaches I2PControl | medium | M016 | implementation complete at `355e243`; awaiting M017 independent acceptance |

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
| 014 | `plans/closure/i2pcontrol-proposal-170/014-closure.md` | corrective pass required until the remaining SAM truthfulness finding is resolved |
| 015 | `plans/closure/i2pcontrol-proposal-170/015-closure.md` | strict closure invalidated; superseded by M017 |
| 016 | `plans/closure/i2pcontrol-proposal-170/016-implementation-disposition.md` | implementation closing at frozen head `355e243`; pre-amendment blocker retained as history |
| 017 | no closure record | ready independent final gate |

## Registry maintenance rules

1. M016 is in `closing` with its implementation head frozen.
2. M017 is ready and must review the frozen M016 head independently.
3. Keep the subsystem open until M017 accepts the final head.
4. M017 must be performed by a distinct, auditable reviewer against the actual final head.
5. Mark the subsystem closed only with zero unresolved high/medium findings.
6. Do not add another milestone for defects that fit the amended M016 boundary.
7. Do not conceal a field-level SAM blocker with empty, partial, stale, or default success.
8. Verification remains local and package-scoped; remote CI is not required.
9. Preserve M015 and the pre-amendment M016 blocker record as historical evidence rather than rewriting them into passing closure records.
