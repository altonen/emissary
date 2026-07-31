# Proposal 170 Implementation Handoffs

This directory contains bounded implementation and closure handoffs for the I2PControl Proposal 170 subsystem.

Authoritative direction:

- `plans/000-long-term-specification.md`
- `plans/001-terminology-and-domain-model.md`
- `plans/002-long-term-roadmap.md`
- `plans/003-planning-process.md`
- `plans/adrs/ADR-0001-proposal-170-contract-and-stub-boundary.md`
- `plans/subsystems/i2pcontrol-proposal-170-roadmap.md`

## Milestone plans

| Milestone | Current status | Plan | Activation dependency |
|---|---|---|---|
| 001 — Contract matrix and I2PControl foundation | historical closed | `001-contract-matrix-and-i2pcontrol-foundation.md` | none |
| 002 — Control-plane domain and persistence | historical closed | `002-control-plane-domain-and-persistence.md` | M001 |
| 003 — AddressBook administrative API | historical closed | `003-address-book-administrative-api.md` | M002 |
| 004 — TunnelManager contract and explicit stubs | historical closed | `004-tunnel-manager-contract-and-stubs.md` | M002 |
| 005 — RouterInfo inspection and exact selectors | superseded | `005-router-info-inspection.md` | later corrective plans |
| 006 — ClientServicesInfo | superseded | `006-client-services-info.md` | later corrective plans |
| 007 — Conformance, hardening, and strict closure | superseded | `007-conformance-hardening-and-strict-closure.md` | later corrective plans |
| 008 — Production composition and durable-state integrity | historical closed | `008-production-composition-and-durable-state-integrity.md` | retained |
| 009 — RouterInfo availability and truthfulness | historical closed | `009-router-info-availability-and-truthfulness.md` | retained |
| 010 — Bounded core router inspection | historical corrective record | `010-bounded-core-router-inspection.md` | residual behavior materially corrected by M014 |
| 011 — ClientServicesInfo live state | corrective pass required | `011-client-services-live-state.md` | remaining SAM/fencing defects owned by M016 |
| 012 — Real TLS and request resource hardening | corrective pass required | `012-real-tls-and-request-resource-hardening.md` | remaining saturation-proof defect owned by M016 |
| 013 — Production conformance and independent reclosure | superseded | `013-production-conformance-and-independent-reclosure.md` | superseded by later gates |
| 014 — Spec-constrained truthfulness and local hardening | corrective pass required | `014-spec-constrained-truthfulness-and-local-hardening.md` | materially landed; M016 owns three residual findings |
| 015 — Focused independent reclosure | strict closure invalidated | `015-focused-independent-reclosure.md` | historical record; superseded by M017 |
| 016 — SAM, fencing, and connection-proof corrective pass | ready | `016-sam-fencing-and-connection-proof-corrective-pass.md` | baseline `43088a42881a76b3936c76f6e7eb8a51262504c4` |
| 017 — Final-head independent reclosure | blocked | `017-final-head-independent-reclosure.md` | frozen M016 head and auditable independent reviewer |

## Current execution order

```text
M016 implementation
    |
    v
M017 independent final-head closure
```

M016 is the only ready implementation handoff. Do not split its three findings into additional milestones.

## M016 scope rule

M016 owns only:

- exact SAM `ClientServicesInfo` session semantics;
- atomic same-category service-generation fencing;
- deterministic saturation and restoration proof for the existing local TLS connection bound.

Primary production files:

```text
emissary-cli/src/i2pcontrol/client_services.rs
emissary-cli/src/i2pcontrol/service_registry.rs
emissary-cli/src/i2pcontrol/server.rs
```

A core edit is permitted only for one small bounded read-only session accessor on the canonical SAM owner when pinned Proposal 170/i2pd behavior requires actual session entries.

M016 must not add:

- Proposal 170 extensions;
- SAM lifecycle control or a generic session registry;
- router, transport, NetDB, tunnel, frontend, resolver, cryptographic, or broad security work;
- repository-wide formatting;
- CI, nightly, platform, coverage, release, publishing, or generated-evidence machinery;
- a generic task supervisor or connection-management framework.

If the exact SAM contract cannot be pinned or safely exposed, stop with a named blocker and keep M017 blocked.

## M017 closure rule

M017 reviews the actual final head, not the older M015 frozen head.

The closure reviewer must be distinct from the final M016 implementation executor and identify the distinct agent/run or equivalent evidence in the closure record.

M017 must reconcile, without broad re-audit:

- post-M015 formatting commit `19370671053b534751328a1e761d717696e55761`;
- feature-gating commit `43088a42881a76b3936c76f6e7eb8a51262504c4`;
- M016’s exact SAM, fencing, and saturation changes;
- the final changed-file scope and targeted local command outcomes.

Any high/medium finding rejects closure and returns work to M016 when it fits the same narrow boundary.

## Verification rule

Use targeted local verification:

```bash
cargo fmt --all -- --check
cargo check -p emissary-cli --no-default-features --features i2pcontrol
cargo test -p emissary-cli --no-default-features --features i2pcontrol
cargo clippy -p emissary-cli --no-default-features --features i2pcontrol --all-targets -- -D warnings
```

Run core commands only if the permitted SAM accessor is added.

Do not add these commands to GitHub Actions. Remote CI evidence is optional and absence of it is not a blocker.

Not required:

- full unrelated workspace testing;
- UI builds;
- multi-platform or nightly CI;
- fuzzing;
- network farms;
- coverage thresholds;
- release dry runs;
- generated evidence archives;
- repeated verification on an unchanged frozen head.

## Activation rule

1. Execute only M016.
2. Resolve SAM from official Proposal 170 plus pinned adopted i2pd source before changing its wire behavior.
3. Land M016 production changes and required existing-suite regressions.
4. Freeze the implementation head.
5. Move M016 to `closing` and M017 to `ready` in `plans/registry.md`.
6. A distinct reviewer executes M017 against the actual final head.
7. Only an accepted `plans/closure/i2pcontrol-proposal-170/017-closure.md` may return the subsystem to `closed`.
8. Preserve `015-closure.md` as an invalid historical record; do not rewrite it into passing evidence.
