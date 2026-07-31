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
| 005 — RouterInfo inspection and exact selectors | superseded | `005-router-info-inspection.md` | superseded by M009/M010/M014 |
| 006 — ClientServicesInfo | superseded | `006-client-services-info.md` | superseded by M011/M014 |
| 007 — Conformance, hardening, and strict closure | superseded | `007-conformance-hardening-and-strict-closure.md` | superseded by M012/M013/M015 |
| 008 — Production composition and durable-state integrity | historical closed | `008-production-composition-and-durable-state-integrity.md` | retained |
| 009 — RouterInfo availability and truthfulness | historical closed | `009-router-info-availability-and-truthfulness.md` | retained with M014 residual corrections |
| 010 — Bounded core router inspection | corrective pass required | `010-bounded-core-router-inspection.md` | residual defects owned by M014 |
| 011 — ClientServicesInfo live state | corrective pass required | `011-client-services-live-state.md` | residual defects owned by M014 |
| 012 — Real TLS and request resource hardening | corrective pass required | `012-real-tls-and-request-resource-hardening.md` | residual defects owned by M014 |
| 013 — Production conformance and independent reclosure | superseded as final gate | `013-production-conformance-and-independent-reclosure.md` | superseded by M015 |
| 014 — Spec-constrained truthfulness and local hardening | closed | `014-spec-constrained-truthfulness-and-local-hardening.md` | accepted by M015 |
| 015 — Focused independent reclosure | closed | `015-focused-independent-reclosure.md` | `plans/closure/i2pcontrol-proposal-170/015-closure.md` |

## Current execution order

```text
M014 implementation
    |
    v
M015 independent closure
```

Do not split M014 into additional subsystem milestones unless M015 identifies a defect that cannot fit the existing narrow boundary.

## Scope rule

M014/M015 remain confined to exact Proposal 170 correctness:

- no protocol expansion;
- no frontend work;
- no router behavioral change;
- no runtime resolver adoption;
- no migration of existing startup task ownership;
- no missing tunnel data-plane implementation;
- no BOB implementation;
- no broad re-hardening of already validated code;
- no CI, release, publishing, platform-matrix, coverage, or generated-evidence work.

Primary production changes belong in:

```text
emissary-cli/src/i2pcontrol/**
```

Only minimal handle wiring may touch `emissary-cli/src/main.rs` or `emissary-cli/src/logger.rs`.

A core edit is permitted only for one small bounded read-only observation seam required by the exact Proposal contract. If broader core redesign is required, preserve explicit unavailable behavior and stop.

## Verification rule

Use targeted local verification for the touched package boundary:

```bash
cargo fmt --all -- --check
cargo check -p emissary-cli --no-default-features --features i2pcontrol
cargo test -p emissary-cli --no-default-features --features i2pcontrol
cargo clippy -p emissary-cli --no-default-features --features i2pcontrol --all-targets -- -D warnings
```

Run core commands only when core is touched.

Do not add these commands to GitHub Actions as part of this workstream. Remote CI evidence is optional; required local regressions are not.

## Activation rule

1. M014 is the only ready implementation plan.
2. When M014 code and required regressions land, freeze the implementation head.
3. Move M014 to `closing` and M015 to `ready` in `plans/registry.md`.
4. A reviewer distinct from the final M014 implementation agent executes M015.
5. Any high/medium finding rejects closure and returns work to a narrowly amended M014 boundary.
6. Only an accepted `plans/closure/i2pcontrol-proposal-170/015-closure.md` may return the subsystem to `closed`.
