# Proposal 170 Implementation Handoffs

This directory contains the bounded implementation plans for the I2PControl Proposal 170 subsystem.

Authoritative direction:

- `plans/000-long-term-specification.md`
- `plans/001-terminology-and-domain-model.md`
- `plans/002-long-term-roadmap.md`
- `plans/003-planning-process.md`
- `plans/adrs/ADR-0001-proposal-170-contract-and-stub-boundary.md`
- `plans/subsystems/i2pcontrol-proposal-170-roadmap.md`

## Milestone plans

| Milestone | Status at this planning commit | Plan | Activation dependency |
|---|---|---|---|
| 001 — Contract matrix and I2PControl foundation | closed | `001-contract-matrix-and-i2pcontrol-foundation.md` | none |
| 002 — Control-plane domain and persistence | closed | `002-control-plane-domain-and-persistence.md` | M001 strict closure |
| 003 — AddressBook administrative API | closed | `003-address-book-administrative-api.md` | M002 strict closure |
| 004 — TunnelManager contract and explicit stubs | closed | `004-tunnel-manager-contract-and-stubs.md` | M002 strict closure |
| 005 — RouterInfo inspection and exact selectors | superseded | `005-router-info-inspection.md` | Superseded by M009/M010 |
| 006 — ClientServicesInfo | superseded | `006-client-services-info.md` | Superseded by M011 |
| 007 — Conformance, hardening, and strict closure | superseded | `007-conformance-hardening-and-strict-closure.md` | Superseded by M012/M013 |
| 008 — Production composition and durable-state integrity | closed | `008-production-composition-and-durable-state-integrity.md` | M001–M004 interfaces |
| 009 — RouterInfo availability and truthfulness | closed | `009-router-info-availability-and-truthfulness.md` | M008 closed |
| 010 — Bounded core router inspection | closed | `010-bounded-core-router-inspection.md` | M009 closed |
| 011 — ClientServicesInfo live state | closed | `011-client-services-live-state.md` | M008, M009 closed |
| 012 — Real TLS and request resource hardening | closed | `012-real-tls-and-request-resource-hardening.md` | M008/M009 interfaces stable |
| 013 — Production conformance and independent reclosure | closed | `013-production-conformance-and-independent-reclosure.md` | M010, M011, M012 closed |

## Activation rule

A prewritten plan is not automatically ready merely because it exists.

Before activating a blocked plan:

1. Confirm every hard dependency has an accepted closure record.
2. Replace the planning baseline with the reviewed dependency head.
3. Inspect current production code and all dependency closure findings.
4. Reconcile file-level mechanics, interfaces, tests, and commands without weakening canonical invariants.
5. Record material deviations in the plan or an ADR.
6. Mark only the next dependency-ready plan `ready` in `plans/registry.md`.
7. Preserve later plans as blocked until their dependencies close.

M003, M004, and the interface-safe portions of M005 may proceed in parallel only after M002 closes and the registry explicitly activates them.

## Scope rule

Every plan remains confined to Proposal 170 API implementation:

- no protocol expansion;
- no frontend work;
- no router behavioral changes;
- no runtime resolver adoption;
- no migration of existing startup task ownership;
- no implementation of missing tunnel data planes.

Missing tunnel types receive complete API configuration behavior and explicit unsupported runtime backends. A later real tunnel project replaces a backend registration; it does not redesign Proposal 170 handlers or persistence.

## Closure rule

Implementation landing changes a plan to `closing`, not `closed`.

A separate closure record under:

```text
plans/closure/i2pcontrol-proposal-170/NNN-status.md
```

must review the plan's acceptance criteria, actual code, commands, tests, compatibility, security, persistence, failure semantics, and unresolved findings. Only an accepted closure record may unblock the next hard-dependent milestone.
