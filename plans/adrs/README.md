# Architecture Decision Records

This directory contains durable Emissary decisions that affect several milestones, ownership boundaries, security behavior, storage, or public compatibility.

Use an ADR when a question cannot be safely resolved inside one implementation plan without establishing a reusable architectural contract.

## Naming

```text
ADR-NNNN-short-title.md
```

Numbers increase monotonically and are never reused.

## Status lifecycle

```text
proposed -> accepted -> deprecated or superseded
         `-> rejected
```

Accepted ADRs are historical records. Do not rewrite an accepted ADR to make a later decision appear original. Create a new ADR and mark the previous decision superseded.

## ADR template

```markdown
# ADR-NNNN: Title

Status: proposed

Date: YYYY-MM-DD

Decision owners: project maintainers

Related canonical sections:

- `plans/000-long-term-specification.md#...`
- `plans/001-terminology-and-domain-model.md#...`

Affected subsystem roadmaps:

- `plans/subsystems/...`

## Context

Describe the architectural problem, repository state, constraints, and why the decision is required.

## Decision drivers

- ...

## Considered options

### Option A — Name

Description, benefits, costs, and failure modes.

### Option B — Name

Description, benefits, costs, and failure modes.

## Decision

State the selected option precisely, including ownership and interface boundaries.

## Consequences

### Positive

- ...

### Negative

- ...

### Neutral or deferred

- ...

## Compatibility and migration

Describe storage, API, protocol, configuration, and operational effects.

## Security and reliability implications

Describe authorization, secret handling, contention, cancellation, restart, recovery, and denial-of-service effects.

## Verification

Describe evidence required to prove conformity.

## Supersession

None.
```

## ADR threshold

An ADR is normally required when a decision:

- establishes or changes a public compatibility contract;
- changes ownership between CLI, core, runtime managers, or frontends;
- introduces durable storage or migration semantics;
- changes authentication or authorization;
- changes consistency, recovery, cancellation, or lifecycle authority;
- selects a durable external standard or dependency;
- materially changes a canonical non-goal.

An ADR is usually unnecessary for local refactors, internal naming, reversible optimizations, or file placement that preserves established contracts.
