# Closure and Verification Records

This directory contains evidence-based completion records for implementation milestones.

A closure record is not a retrospective summary alone. It is the gate that determines whether a subsystem milestone can be marked complete.

## Layout and naming

```text
closure/<subsystem>/NNN-status.md
```

Use the same milestone number as the source implementation plan.

## Required closure-record template

```markdown
# <Subsystem> Milestone NNN — Closure Status

Status: closed | conditionally closed | corrective pass required | blocked

Source implementation plan:

- `plans/implementation/<subsystem>/NNN-...md`

Source subsystem roadmap:

- `plans/subsystems/<subsystem>-roadmap.md#...`

Repository baseline reviewed: `<SHA>`

Implementation commits or pull requests:

- `<SHA or link>` — summary

## 1. Executive finding

State whether the actual capability or infrastructure boundary is complete and why.

## 2. Requirement-to-evidence matrix

| Requirement | Evidence | Result | Notes |
|---|---|---|---|
| ... | test, code, migration, docs, runtime output | pass/fail/partial/not run | ... |

## 3. Production implementation evidence

Distinguish implemented behavior from planned or absent behavior.

## 4. Verification executed

### Commands run

```bash
# exact commands
```

### Results

Record pass, fail, timeout, environmental block, skipped scope, and relevant counts without concealing partial execution.

## 5. Invariant review

Record evidence for each source-plan invariant.

## 6. Failure and recovery review

Cover applicable duplicate requests, cancellation races, restart, partial persistence, stale state, contention, resource release, malformed input, unauthorized input, and bounded result behavior.

## 7. Migration and compatibility review

Record schema migration, backward compatibility, protocol behavior, configuration behavior, rollback limits, and legacy-path status.

## 8. Security review

Record authentication/authorization, secret handling, path validation, privilege boundaries, denial-of-service limits, redaction, and audit behavior.

## 9. Documentation and operations

List updated architecture documents, commands, diagnostics, static guards, and recovery instructions.

## 10. Unresolved findings

| Severity | Finding | Impact | Required action |
|---|---|---|---|
| ... | ... | ... | ... |

Severity:

- critical — unsafe to merge or operate;
- high — core correctness, security, or public contract incomplete;
- medium — important behavior incomplete but bounded;
- low — polish, maintainability, or optional evidence gap.

## 11. Roadmap disposition

State one:

- milestone closed and next dependency may proceed;
- milestone conditionally closed with named evidence outstanding;
- corrective implementation plan required;
- milestone blocked and reason;
- subsystem roadmap must be revised.

## 12. Registry updates

List required changes in `plans/registry.md` and the source roadmap.
```

## Closure rules

A milestone MUST NOT be marked closed when:

- only compilation or formatting was verified;
- required tests were not run without justified substitute evidence;
- externally visible capability has only internal infrastructure;
- a security, persistence, or compatibility requirement is unimplemented;
- a Proposal 170 field, type, action, or selector is missing from its owning milestone;
- unsupported runtime behavior is represented as success;
- a known high- or medium-severity correctness defect remains;
- closure depends on unrecorded assumptions.

A milestone MAY be conditionally closed when production implementation is complete but named external or operational evidence cannot be obtained. The condition, risk, and exact future evidence must be explicit.

## Corrective follow-up

When closure requires correction:

1. preserve the closure record except for factual corrections;
2. create a new implementation plan under the same subsystem;
3. reference every unclosed finding;
4. add regression tests or guards that prevent recurrence;
5. do not reopen unrelated closed scope.
