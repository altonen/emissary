# Proposal 170 Long-Term Roadmap

Status: active

This roadmap orders the work required to implement I2P Proposal 170 without expanding into missing tunnel data planes, router behavioral changes, or frontend work.

Normative references:

- `plans/000-long-term-specification.md`
- `plans/001-terminology-and-domain-model.md`
- `plans/adrs/ADR-0001-proposal-170-contract-and-stub-boundary.md`

The detailed workstream roadmap is `plans/subsystems/i2pcontrol-proposal-170-roadmap.md`.

## Dependency graph

```text
M001 Contract matrix and I2PControl foundation
    |
    v
M002 Control-plane domain and persistence
    |
    +------------------+-------------------+
    |                  |                   |
    v                  v                   v
M003 AddressBook   M004 TunnelManager   M005 RouterInfo inspection
                       and stubs             |
                         |                   |
                         +---------+---------+
                                   |
                                   v
                         M006 ClientServicesInfo
                                   |
                                   v
                         M007 Conformance and closure
```

M003, M004, and the non-core portions of M005 may proceed in parallel after M002 closes. M006 depends on stable tunnel and router/service inspection interfaces. M007 is the release gate.

## Milestone M001 — Contract matrix and I2PControl foundation

Primary class: invariant / infrastructure

Establish:

- an exact Proposal 170 conformance matrix;
- base I2PControl authentication and version behavior required by the extension;
- a frontend-independent JSON-RPC listener and dispatcher;
- exact protocol/error DTOs;
- bounded request handling and security defaults;
- a typed method registry and control-plane interface boundary;
- contract fixtures for all later milestones.

Exit condition: the service foundation is production-shaped and tested, but Proposal 170 feature methods may remain explicitly unavailable until their owning milestone lands.

## Milestone M002 — Control-plane domain and persistence

Primary class: invariant / infrastructure

Establish:

- canonical tunnel definitions covering every Proposal 170 option;
- exhaustive tunnel type and action enums;
- backend registry contracts;
- administrative address-book models;
- versioned, atomic, restart-safe persistence;
- explicit ownership and internal state models;
- fake control-plane implementations for method tests.

Exit condition: later handlers can implement the exact contract without reshaping storage or runtime ownership.

## Milestone M003 — AddressBook

Primary class: capability

Implement:

- all four administrative books;
- list, lookup, add/update, and delete semantics;
- `SetConfig` and `SetSubscriptions`;
- exact validation and response behavior;
- persistence and restart recovery;
- RouterInfo address-book selectors backed by the administrative stores.

Exit condition: the complete Proposal 170 AddressBook API works without changing runtime resolver precedence.

## Milestone M004 — TunnelManager and explicit stubs

Primary class: capability / infrastructure

Implement:

- exact parsing for every declared tunnel type and option;
- create, edit, get, delete, start, stop, restart, and permitted `All` behavior;
- exhaustive backend registration;
- real adapters only where current runtime ownership is safe and already available;
- explicit unsupported backends for missing data planes;
- truthful handling of startup-managed tunnels;
- persistent definitions and deterministic status/error mapping.

Exit condition: the public TunnelManager contract is complete, unsupported types have functional configuration CRUD, and no stub can report or simulate active service.

## Milestone M005 — RouterInfo inspection

Primary class: capability / infrastructure

Implement all Proposal 170 RouterInfo selectors using:

- retained startup identity and serialized RouterInfo;
- shared metric snapshots;
- bounded tracing-backed log retrieval;
- bounded read-only core inspection;
- Proposal 170 tunnel and address-book registries;
- exact selector-by-presence response filtering.

Exit condition: every selector is implemented with truthful data, permitted null/error behavior, and no router mutation or frontend event interference.

## Milestone M006 — ClientServicesInfo

Primary class: capability

Implement exact service selectors for:

- I2PTunnel;
- HTTPProxy;
- SOCKS;
- SAM;
- BOB;
- I2CP.

Use actual listener/session state where available, explicit unavailable state where specified, and inactive representation for stubbed tunnels.

Exit condition: all selectors return only requested sections and never report unsupported services as active.

## Milestone M007 — Conformance, hardening, and strict closure

Primary class: invariant / polish

Complete:

- generated or matrix-driven protocol tests;
- reference fixture compatibility;
- authentication, negative-input, and denial-of-service tests;
- concurrent-edit, cancellation, restart, and persistence-failure tests;
- static guards against protocol expansion and frontend coupling;
- support documentation distinguishing API and runtime completeness;
- independent closure review.

Exit condition: every conformance-matrix row has evidence, no high- or medium-severity correctness finding remains, and the completion statement in the long-term specification is accurate.

## Deferred work outside this roadmap

The following require separate roadmaps or explicit later plans:

- runtime lifecycle migration of existing startup-managed tunnels;
- missing client/server tunnel data planes;
- runtime use and precedence of the four Proposal 170 address books;
- frontend management of I2PControl resources;
- new I2PControl methods or fields;
- cross-router interoperability certification beyond Proposal 170 contract conformance.

A deferred real tunnel backend should replace one stub through the backend registry. It must not reopen Proposal 170 parsing, persistence, or handler design.

## Roadmap completion rule

The roadmap is not complete when only the server foundation or internal models exist. It closes only after M007 demonstrates exact external behavior and the explicit non-goals remain intact.
