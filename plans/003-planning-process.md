# Emissary Planning and Agent-Handoff Process

Status: normative planning governance

This document defines how canonical Emissary direction is translated into bounded implementation work. The keywords MUST, MUST NOT, REQUIRED, SHOULD, SHOULD NOT, and MAY are normative.

## 1. Planning horizons

Planning uses two separate horizons:

1. **Canonical planning** defines architectural ownership, public contracts, invariants, non-goals, dependency order, and end-state completion.
2. **Interim planning** defines bounded work against a specific repository baseline for implementation-agent handoff.

Interim evidence MAY reveal that canonical direction must change. An implementation plan MUST NOT silently weaken canonical requirements to match the easiest code path.

## 2. Document classes

### 2.1 Canonical documents

Canonical documents are:

- `plans/000-long-term-specification.md`;
- `plans/001-terminology-and-domain-model.md`;
- `plans/002-long-term-roadmap.md`;
- this planning-governance document.

They may be amended only when maintainers intentionally change direction, resolve a material contradiction or omission, accept an ADR that changes the end state, or explicitly request an architecture revision.

### 2.2 Architecture decision records

An ADR records a durable decision affecting public compatibility, ownership, security, storage, lifecycle, or several milestones.

An ADR MUST state context, drivers, options, decision, consequences, compatibility effects, security/reliability implications, verification, and status.

Accepted ADRs are historical records. Later decisions supersede them rather than rewriting history.

### 2.3 Subsystem roadmaps

A subsystem roadmap translates canonical requirements into a coherent workstream. It MUST define:

- purpose and ownership boundary;
- canonical and ADR references;
- invariants, capabilities, infrastructure, and polish;
- explicit non-goals;
- current-state evidence;
- target architecture;
- dependency graph and dependency classes;
- ordered milestones and exit conditions;
- cross-cutting storage, protocol, security, lifecycle, observability, and operational requirements;
- risks and deferred work;
- milestone status.

Roadmaps SHOULD avoid fragile line numbers and blind file-edit instructions.

### 2.4 Implementation plans

An implementation plan is the primary coding-agent handoff. It MUST be independently executable, bounded, and tied to a repository baseline.

It MUST include:

- source roadmap and milestone;
- canonical requirements and applicable ADRs;
- one bounded objective;
- readiness and current evidence;
- invariants and explicit non-goals;
- required production changes;
- ordered work packages;
- failure, cancellation, restart, and contention semantics;
- compatibility and migration effects;
- focused and broad tests;
- verification commands;
- documentation and static guards;
- acceptance and stop conditions;
- closure evidence required.

Material deviations MUST be recorded rather than hidden.

### 2.5 Closure records

A closure record decides whether a milestone is actually complete. It MUST include:

- implementation commits or pull requests;
- a requirement-to-evidence matrix;
- exact verification commands and outcomes;
- invariant review;
- failure/recovery and contention evidence;
- compatibility, migration, and security review;
- documentation and operational evidence;
- unresolved findings with severity;
- disposition: closed, conditionally closed, corrective pass required, or blocked.

A code commit, compilation result, or implementation-agent assertion is not closure evidence by itself.

### 2.6 Archive records

Completed, superseded, rejected, or abandoned interim documents SHOULD move under `plans/archive/` when they no longer represent active work. Archive moves preserve traceability and original subsystem grouping.

Canonical documents and accepted ADRs are not archived merely because initial implementation finished.

## 3. Work classification

Every planned item receives one primary class.

### Invariant

A property that must remain true across releases and implementation strategies. Invariant work normally requires architecture checks, negative tests, static guards, or property evidence.

Proposal 170 examples include no protocol expansion, no fabricated state, frontend independence, and exhaustive tunnel backend registration.

### Capability

Externally visible behavior. Capability completion requires end-to-end evidence, not only internal types.

### Infrastructure

Internal machinery consumed by capabilities. Infrastructure must not be represented as completed API capability until a real request path consumes it.

### Polish

Diagnostics, ergonomics, performance, cleanup, or documentation that does not establish the principal contract boundary.

## 4. Dependency model

Each milestone dependency is classified as:

- **hard** — implementation cannot correctly begin before closure;
- **interface** — work may proceed against a stable written contract or test double;
- **soft** — parallel work is possible but final integration depends on the other milestone;
- **operational** — code may land, but release or deployment depends on external evidence.

A milestone is dependency-ready only when every hard dependency is closed and every interface dependency is stable and written.

The registry MUST name blocked milestones and exact blockers.

## 5. Milestone sizing

A milestone SHOULD be small enough for one implementation agent to understand ownership, implement production behavior, add focused tests, run verification, update documentation, and report residual risk in one coherent pass.

Prefer vertical contract slices over broad refactors with no consumer.

A milestone is too large when it combines independent API methods, unrelated persistence migrations, and unresolved architecture decisions. It is too small when it produces no meaningful contract or closure boundary unless it is a necessary corrective pass.

## 6. Agent handoff contract

Authority order is:

1. canonical specification and terminology;
2. accepted ADRs;
3. subsystem roadmap;
4. implementation plan;
5. current repository evidence.

When code reality conflicts with the plan, the agent MUST preserve canonical invariants, record the discrepancy, and make only the smallest coherent adjustment. It MUST NOT invent a broader architecture to complete a checklist.

The implementation agent MUST:

- inspect current code before editing;
- preserve unrelated changes;
- maintain exact protocol spelling and types;
- keep administrative, runtime, and frontend ownership distinct;
- add tests and documentation with production changes;
- identify incomplete work explicitly;
- leave closure judgment to the closure record.

## 7. Corrective passes

A corrective pass is a new implementation plan. It MUST:

- reference the original plan and closure record;
- enumerate each unclosed requirement or defect;
- explain why prior verification missed it;
- add regression evidence that would have caught it;
- avoid reopening unrelated closed scope.

Repeated corrective passes indicate that roadmap decomposition or acceptance criteria require revision.

## 8. Registry requirements

`plans/registry.md` is the active planning control surface. It SHOULD contain only:

- active subsystem roadmaps;
- dependency-ready, active, or closing implementation plans;
- active closure work;
- blockers;
- recently closed milestones;
- explicitly deferred unregistered work.

The registry links source documents and does not duplicate their requirements.

Only the next dependency-ready implementation plan should normally be registered. Future milestones remain in the roadmap until dependencies close.

## 9. Required handoff review

Before registration, review an implementation plan for:

1. correct canonical and ADR references;
2. resolved architecture decisions;
3. dependency readiness;
4. bounded scope and non-goals;
5. explicit ownership and invariants;
6. storage, protocol, compatibility, and migration effects;
7. failure, cancellation, restart, and contention semantics;
8. security and authorization effects;
9. test and static-guard evidence;
10. unambiguous closure criteria.

If these cannot be answered, the plan is not ready for handoff.

## 10. Proposal 170-specific guards

Every Proposal 170 implementation plan MUST explicitly verify that it does not:

- implement a deferred tunnel data plane;
- add protocol fields, aliases, statuses, methods, or tunnel types;
- change router algorithms or network behavior;
- make administrative address books authoritative at runtime;
- couple I2PControl to frontend state;
- report unsupported services as active;
- consume a single-owner event receiver used by another subsystem;
- replace truthful unavailability with fabricated defaults.

## 11. Planning anti-patterns

The following are prohibited or strongly discouraged:

- transient TODOs in canonical documents;
- one implementation plan covering the whole roadmap;
- equating compilation with closure;
- changing public terminology in local modules;
- retaining stale active plans after repository reality changes;
- copying detailed requirements into the registry;
- recording only successful evidence;
- claiming contract completion from infrastructure alone;
- broad core refactors justified only by API convenience;
- implementing deferred runtime behavior during a protocol-contract milestone.
