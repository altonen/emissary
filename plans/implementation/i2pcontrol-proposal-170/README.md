# Proposal 170 Implementation Handoffs

This directory contains bounded implementation and closure handoffs for the I2PControl Proposal 170 subsystem.

Authoritative direction:

- `plans/003-planning-process.md`
- `plans/adrs/ADR-0001-proposal-170-contract-and-stub-boundary.md`
- `plans/subsystems/i2pcontrol-proposal-170-roadmap.md`
- `plans/closure/i2pcontrol-proposal-170/017-closure-invalidation.md`

## Current milestones

| Milestone | Status | Plan | Activation dependency |
|---|---|---|---|
| 014 — Spec-constrained truthfulness and local hardening | implementation retained | `014-spec-constrained-truthfulness-and-local-hardening.md` | broad final acceptance reopened by M018/M019 |
| 015 — Focused independent reclosure | invalid historical closure | `015-focused-independent-reclosure.md` | superseded |
| 016 — Bounded SAM session observation corrective pass | implementation retained | `016-sam-fencing-and-connection-proof-corrective-pass.md` | bounded SAM component accepted |
| 017 — Final-head independent reclosure | corrective pass required | `017-final-head-independent-reclosure.md` | closure invalidated by exact Proposal 170 contract findings |
| 018 — Exact wire-contract reconciliation | ready | `018-exact-wire-contract-reconciliation.md` | sole implementation handoff |
| 019 — Pinned-revision independent reclosure | blocked | `019-pinned-revision-independent-reclosure.md` | frozen complete M018 head and distinct auditable reviewer |

Earlier milestones remain historical or superseded as recorded in the subsystem roadmap.

## Current execution order

```text
M018 exact wire-contract reconciliation
    |
    v
M019 pinned-revision independent reclosure
```

Execute only M018. Do not begin M019 until the M018 implementation/test head is frozen and the registry marks M018 `closing`.

## Why M017 is invalidated

M017 accepted bounded SAM observation, atomic service fencing, TLS connection bounds, live metrics/logs, and truthful unavailable behavior. Those components remain retained.

Its broad Proposal 170 closure was invalid because exact public contract comparison was incomplete:

- RouterInfo canonical keys are missing or renamed;
- the 121-key legacy/base catalog is mislabeled as Proposal 170 instead of the exact 43 additions;
- AddressBook uses a different parameter model;
- TunnelManager uses different action casing and result shapes;
- ClientServicesInfo uses a nested selector envelope rather than direct parameter presence.

See:

- `plans/closure/i2pcontrol-proposal-170/017-closure-invalidation.md`.

## M018 handoff rule

M018 owns only exact wire reconciliation and directly affected evidence/documentation.

Required areas:

- exact 43-key RouterInfo manifest and direct parameter presence;
- exact AddressBook canonical modes;
- exact lowercase TunnelManager actions and structured results;
- exact direct-parameter ClientServicesInfo requests;
- compatibility aliases clearly separated from canonical behavior;
- literal official-example fixtures;
- strongest feasible production-composition SAM lifecycle evidence;
- separate wire/source/runtime support claims.

Compatibility forms may remain, but cannot substitute for canonical Proposal 170 forms or count toward canonical coverage.

M018 must not add:

- missing tunnel data planes;
- broad router, transport, NetDB, peer, tunnel, cryptographic, resolver, frontend, SAM, or I2CP architecture;
- generic protocol/schema/fixture/inspection frameworks;
- repository-wide formatting;
- CI, release, publishing, platform, coverage, or generated-evidence machinery;
- fabricated values for unavailable sources.

## M019 closure rule

M019 must independently refetch the still-open Proposal 170 source and verify that the implementation matches the pinned revision.

The reviewer must be distinct from the final M018 implementation executor and identify the separate agent/run.

M019 independently checks:

- exact 43 RouterInfo strings and types;
- AddressBook primary-source response adjudication;
- all seven lowercase TunnelManager actions and structured results;
- direct ClientServicesInfo selection with any value;
- compatibility extension isolation;
- truthful unavailable and unsupported behavior;
- SAM current-session/removal evidence;
- final changed-file scope and targeted command outcomes.

Any unresolved high/medium finding rejects closure and returns work to M018.

Final status must be `closed against pinned revision`, because Proposal 170 remains Open.

## Verification rule

Use targeted local checks for touched packages. Do not add them to GitHub Actions.

```bash
cargo fmt --all -- --check
cargo check -p emissary-cli --no-default-features --features i2pcontrol
cargo test -p emissary-cli --no-default-features --features i2pcontrol
cargo clippy -p emissary-cli --no-default-features --features i2pcontrol --all-targets -- -D warnings
```

Run core commands only when M018 uses its narrow core exception or existing core integration targets.

Known unrelated formatting differences may be replaced by touched-file nightly rustfmt checks with the baseline limitation recorded. Do not reformat unrelated files.

Remote CI, release verification, platform matrices, coverage gates, fuzz campaigns, network farms, and generated evidence bundles are not required.