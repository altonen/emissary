# Proposal 170 Implementation Handoffs

This directory contains bounded implementation and closure handoffs for the I2PControl Proposal 170 subsystem.

Authoritative direction:

- `plans/003-planning-process.md`
- `plans/adrs/ADR-0001-proposal-170-contract-and-stub-boundary.md`
- `plans/subsystems/i2pcontrol-proposal-170-roadmap.md`

## Current milestones

| Milestone | Status | Plan | Activation dependency |
|---|---|---|---|
| 014 — Spec-constrained truthfulness and local hardening | corrective pass required | `014-spec-constrained-truthfulness-and-local-hardening.md` | remaining SAM truthfulness finding owned by M016 |
| 015 — Focused independent reclosure | strict closure invalidated | `015-focused-independent-reclosure.md` | historical record; superseded by M017 |
| 016 — Bounded SAM session observation corrective pass | ready | `016-sam-fencing-and-connection-proof-corrective-pass.md` | one bounded read-only SAM observation handle explicitly authorized |
| 017 — Final-head independent reclosure | blocked | `017-final-head-independent-reclosure.md` | completed frozen M016 head and auditable distinct reviewer |

Earlier milestones remain historical or superseded as recorded in the subsystem roadmap.

## Current execution order

```text
M016 bounded SAM observation implementation
    |
    v
M017 independent final-head closure
```

M016 is the only ready implementation handoff.

## M016 current scope

The fencing race and TLS saturation-evidence defect were resolved at `9047feecde046dac8e0208bbf1acf2e3883f97ae`.

M016 now owns only truthful current SAM session reporting.

Authorized design:

- one fixed-capacity observation state owned by canonical SAM runtime composition;
- one private publisher used only at existing session/socket lifecycle transitions;
- one clonable read-only handle captured before `SamServer` moves into the router runtime;
- on-demand bounded snapshots passed through existing composition to I2PControl;
- exact pinned i2pd session fields only;
- no lifecycle authority or sensitive material.

Primary files are limited to the smallest SAM core files, the router composition seam, and `emissary-cli/src/i2pcontrol/client_services.rs` plus the smallest required DTO/composition support.

M016 must not add:

- Proposal 170 extensions;
- generic observer, event, cache, registry, polling, persistence, or supervisor infrastructure;
- router, transport, NetDB, tunnel, frontend, resolver, cryptographic, or broad security changes;
- repository-wide formatting;
- CI, release, publishing, platform, coverage, or generated-evidence machinery.

## M017 closure rule

M017 reviews the actual final head after M016. The reviewer must be distinct from the final M016 implementation executor and identify the separate agent/run or equivalent evidence.

Any high/medium finding rejects closure and returns work to the amended M016 boundary when possible.

Only an accepted `plans/closure/i2pcontrol-proposal-170/017-closure.md` may return the subsystem to `closed`.

## Verification rule

Use targeted local checks for touched core and CLI packages. Do not add them to GitHub Actions.

```bash
cargo check -p emissary-core --features std,events
cargo test -p emissary-core
cargo clippy -p emissary-core --features std,events --all-targets -- -D warnings
cargo check -p emissary-cli --no-default-features --features i2pcontrol
cargo test -p emissary-cli --no-default-features --features i2pcontrol
cargo clippy -p emissary-cli --no-default-features --features i2pcontrol --all-targets -- -D warnings
```

Run formatting checks only on the intended scope when unrelated repository baseline differences prevent the full workspace command. Do not reformat unrelated files.