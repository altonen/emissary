# Interim Planning Archive

This directory retains completed, superseded, rejected, or abandoned interim planning for traceability.

The archive is not an active work queue. Agents MUST use `plans/registry.md` and current subsystem roadmaps to identify executable work.

## What belongs here

- completed milestone implementation plans after closure;
- superseded subsystem roadmaps;
- closed corrective plans;
- rejected interim proposals worth retaining;
- status documents no longer needed in active directories.

## What does not belong here

- canonical specification, terminology, roadmap, or planning governance;
- accepted ADRs;
- active subsystem roadmaps;
- ready or active implementation plans;
- unresolved closure records.

## Archive layout

Preserve original category and subsystem where practical:

```text
archive/
    subsystems/<subsystem>-roadmap.md
    implementation/<subsystem>/NNN-short-title.md
    closure/<subsystem>/NNN-status.md
```

When archiving:

1. update links from `plans/registry.md` and the active subsystem roadmap;
2. add a short archival note stating final status and replacement, if any;
3. preserve Git history through a move where practical;
4. do not rewrite historical conclusions to match later implementation;
5. ensure active documents link to the replacement or later milestone.

Archived plans may provide evidence, but they are not authoritative over current canonical documents, accepted ADRs, active roadmaps, or current repository behavior.
