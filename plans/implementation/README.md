# Milestone Implementation Plans

This directory contains bounded plans handed directly to implementation agents.

Implementation plans are operational documents tied to current repository state. They may be corrected, superseded, or archived without modifying canonical direction.

## Layout and naming

```text
implementation/<subsystem>/NNN-short-title.md
```

Milestone numbering is local to the subsystem roadmap unless that roadmap defines another stable identifier.

## Required implementation-plan template

```markdown
# <Subsystem> Milestone NNN — <Title>

Status: ready for handoff | active | blocked | implemented | superseded

Repository baseline: `<commit SHA or branch state>`

Source roadmap:

- `plans/subsystems/<subsystem>-roadmap.md#...`

Canonical requirements:

- `plans/000-long-term-specification.md#...`
- `plans/001-terminology-and-domain-model.md#...`

Applicable ADRs:

- `plans/adrs/ADR-NNNN-...md`

Primary class: invariant | capability | infrastructure | polish

## 1. Objective

State one bounded outcome.

## 2. Why this milestone is ready

List closed hard dependencies and stable interface dependencies.

## 3. Current implementation evidence

Describe relevant code, tests, storage, protocols, guards, and known gaps at the baseline.

## 4. Invariants that must not regress

- ...

## 5. Scope

### In scope

- ...

### Explicitly out of scope

- ...

## 6. Required production changes

### Core/domain

### Storage and migrations

### Protocol and DTOs

### Runtime and concurrency

### Frontend or operator surface

### Security and authorization

### Documentation and static guards

## 7. Ordered work packages

### Work package A — Title

Intent:

Required changes:

Acceptance evidence:

## 8. Failure, cancellation, restart, and contention semantics

State expected behavior for partial failure, duplicate requests, process restart, cancellation races, stale state, and concurrent callers.

## 9. Compatibility and migration

Describe backward compatibility, protocol negotiation, configuration fallback, data migration, and legacy-path behavior.

## 10. Required tests

### Focused unit tests

### Integration tests

### Restart and recovery tests

### Contention and cancellation tests

### Security and negative tests

### Migration and compatibility tests

## 11. Required verification commands

```bash
# narrow tests first

# static guards

# formatting and linting

# broader suite appropriate to the change
```

Do not claim commands that were not actually run in the closure record.

## 12. Documentation updates

- ...

## 13. Acceptance criteria

Use externally observable or contract-level statements.

## 14. Stop conditions

The agent must stop and report rather than improvise when:

- an unresolved architecture decision materially changes ownership;
- a hard dependency is absent;
- required migration cannot be made safely;
- repository evidence contradicts a canonical invariant;
- external evidence is required but unavailable;
- scope would expand into another subsystem roadmap.

## 15. Closure evidence required

List exact evidence the later closure record must contain.

## 16. Handoff notes

Include known hazards, serial-test requirements, environment constraints, and preserved user changes.
```

## Handoff rules

Before assigning a plan:

- confirm the repository baseline is current;
- confirm all hard dependencies are closed;
- confirm unresolved durable decisions have ADRs or are explicitly out of scope;
- ensure the milestone can be completed in one coherent pass;
- ensure tests and closure evidence are specific;
- register the plan in `plans/registry.md`.

An implementation agent may adjust file-level mechanics based on current code. It may not weaken canonical invariants or silently enlarge scope.

## Corrective plans

Corrective work receives a new plan in the same subsystem directory. It must reference the original plan and closure record, enumerate unclosed findings, and add regression evidence that would have caught them.
