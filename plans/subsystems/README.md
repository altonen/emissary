# Subsystem Roadmaps

Subsystem roadmaps translate canonical Emissary direction into coherent, dependency-aware workstreams. They are not direct coding-agent checklists.

Each roadmap should remain useful across several implementation milestones and repository revisions. Commit-specific mechanics belong in `plans/implementation/`.

## Naming

```text
<subsystem>-roadmap.md
```

## Required roadmap structure

```markdown
# <Subsystem> Roadmap

Status: proposed | active | closing | closed | superseded

Canonical references:

- `plans/000-long-term-specification.md#...`
- `plans/001-terminology-and-domain-model.md#...`
- `plans/002-long-term-roadmap.md#...`

Related ADRs:

- `plans/adrs/ADR-NNNN-...md`

## 1. Purpose and ownership boundary

Define what the subsystem owns, consumes, and must not own.

## 2. Work classification

### Invariants

### Capabilities

### Infrastructure

### Polish

## 3. Non-goals

## 4. Current state

Summarize repository evidence, existing contracts, compatibility paths, and known gaps. Avoid fragile line-number references unless essential.

## 5. Target architecture

Describe the end-state module, storage, protocol, ownership, and lifecycle model.

## 6. Dependency graph

Classify dependencies as hard, interface, soft, or operational.

## 7. Milestones

### Milestone N — Title

Class:

Objective:

Dependencies:

Deliverable boundary:

User or operator value:

Exit conditions:

Deferred work:

## 8. Cross-cutting requirements

### Storage and migration

### Protocol and compatibility

### Security and authorization

### Concurrency, cancellation, and recovery

### Observability and audit

### Performance and resource use

### Documentation and operations

## 9. Verification strategy

## 10. Risks and decision points

## 11. Completion definition

## 12. Milestone status

| Milestone | Status | Implementation plan | Closure record | Blockers |
|---|---|---|---|---|
```

## Roadmap rules

A subsystem roadmap MUST:

- link canonical requirements rather than copying them wholesale;
- define ownership boundaries before milestones;
- distinguish infrastructure from completed capability;
- expose dependencies and decision points;
- preserve completed milestone history;
- link each active milestone to one implementation plan and later one closure record;
- state non-goals clearly;
- remain at subsystem level rather than becoming a file-by-file checklist.

A roadmap MAY evolve when implementation evidence changes sequencing or decomposition. Material changes must record why.

Create only roadmaps ready for active reasoning. Do not generate speculative subsystem files merely to populate the directory.
