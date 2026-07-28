# I2PControl Proposal 170 Roadmap

Status: active

Canonical references:

- `plans/000-long-term-specification.md`
- `plans/001-terminology-and-domain-model.md`
- `plans/002-long-term-roadmap.md`

Related ADRs:

- `plans/adrs/ADR-0001-proposal-170-contract-and-stub-boundary.md`

External contract:

- I2P Proposal 170, I2PControl API Expansion
- existing I2PControl JSON-RPC API

## 1. Purpose and ownership boundary

This subsystem owns the exact Proposal 170 administrative API contract for Emissary:

- I2PControl listener, authentication, JSON-RPC dispatch, and protocol DTOs;
- Proposal 170 RouterInfo selectors;
- AddressBook administrative stores and operations;
- TunnelManager configuration and lifecycle dispatch;
- explicit unsupported tunnel backends;
- ClientServicesInfo;
- bounded inspection adapters and persistence required by those methods;
- protocol and closure evidence.

The subsystem consumes existing router identity, metrics, protocol listener state, address parsing, configured proxy/tunnel state, and narrowly scoped read-only core inspection.

It does not own router algorithms, I2P wire protocols, missing tunnel data planes, frontend behavior, or runtime address-resolution policy.

## 2. Work classification

### Invariants

- Proposal 170 wire names, types, methods, actions, and tunnel types remain exact.
- Existing I2PControl JSON-RPC and authentication behavior remains compatible.
- No frontend is required to run or observe the service.
- No router, NetDB, transport, tunnel-selection, or congestion behavior changes.
- Unsupported tunnel execution is explicit and never simulated.
- Every declared tunnel type has a registered real or unsupported backend.
- Missing values are not fabricated.
- Administrative address books do not silently change runtime resolution.
- I2PControl does not consume a single-owner event stream required elsewhere.

### Capabilities

- Authenticate and call Proposal 170 through an independent I2PControl listener.
- Query all Proposal 170 RouterInfo selectors.
- Manage all four Proposal 170 administrative address books.
- Create, edit, inspect, delete, and dispatch lifecycle actions for every declared tunnel type.
- Receive deterministic not-implemented lifecycle results for stubbed tunnel types.
- Query all ClientServicesInfo selectors.

### Infrastructure

- Exact request/response and error DTOs.
- Method registry and control-plane interfaces.
- Versioned persistent administrative state.
- Exhaustive tunnel backend registry.
- Bounded router/service inspection snapshots.
- Shared metrics and tracing-backed log snapshots.
- Matrix-driven conformance fixtures.

### Polish

- Operator documentation and configuration examples.
- Diagnostics and redaction.
- Test-fixture generation and static protocol guards.
- Resource-limit tuning after correctness closure.

## 3. Non-goals

- Implementing missing client or server tunnel data planes.
- Migrating existing startup managers into Proposal 170 lifecycle ownership.
- Adding frontend controls or replacing frontend state paths.
- Changing address-book runtime precedence.
- Extending Proposal 170 with capability discovery, aliases, pagination, or richer status fields.
- Refactoring core subsystems beyond bounded read-only inspection.
- Claiming runtime completeness for stubbed tunnel types.

## 4. Current state

At repository baseline `9b43484a21d5a1291c4881cdae62a36c527f8c0f` on `master`:

- the workspace contains `emissary-cli`, `emissary-core`, and `emissary-util`;
- `emissary-cli` has no I2PControl module or dedicated control-plane service;
- Axum and Serde JSON are optional dependencies, with Axum currently associated with UI/liveview functionality;
- the application constructs router, address-book, proxy, protocol, and tunnel managers in `emissary-cli/src/main.rs`;
- existing client and server tunnel managers create configured tasks at startup and do not expose a complete external create/edit/start/stop/restart/delete command interface;
- existing address-book code exposes one logical runtime store rather than four Proposal 170 administrative books;
- `Router` exposes only a limited public management surface relative to Proposal 170 inspection requirements;
- event/status infrastructure contains several useful counters but the current event subscriber is not an independent multi-consumer Proposal 170 snapshot service;
- tracing output does not provide a dedicated bounded readable/clearable I2PControl log buffer;
- no Proposal 170 persistence schema or tunnel backend registry exists.

These gaps justify a staged implementation. They do not justify broad router redesign.

## 5. Target architecture

The primary application-layer module is expected under:

```text
emissary-cli/src/i2pcontrol/
    mod.rs
    server.rs
    auth.rs
    rpc.rs
    errors.rs
    control_plane.rs
    router_info.rs
    address_book.rs
    tunnel_manager.rs
    client_services.rs
    persistence.rs
```

Exact file placement MAY change with repository evidence, but ownership must remain equivalent.

### 5.1 Request path

```text
HTTP listener
    -> bounded JSON-RPC parser
    -> authentication/version gate
    -> exact method registry
    -> typed Proposal 170 handler
    -> ControlPlane interface
    -> inspection/store/backend adapter
    -> exact result or error serializer
```

### 5.2 Control-plane boundary

The control plane provides typed interfaces for:

- immutable router inspection;
- shared metrics snapshots;
- bounded logs and clear operation;
- four address-book stores and configuration;
- tunnel definitions, ownership, state, backend dispatch, and persistence;
- client-service inspection.

Handlers do not directly manipulate runtime task internals or files.

### 5.3 Tunnel architecture

A canonical tunnel definition preserves all Proposal 170 options. An exhaustive backend registry maps each exact tunnel type to:

- a real adapter where existing runtime behavior can be safely and truthfully controlled; or
- `UnsupportedTunnelBackend` where no real data plane exists.

Unsupported runtime state remains internal. Public queries map it to an existing inactive state, while start/restart return deterministic `error - ... not implemented` operation statuses.

### 5.4 Persistence

Proposal 170 administrative state uses a dedicated versioned state area under the configured Emissary base path. It is validated, atomically replaced, restart-readable, and independent from existing startup manager configuration unless a later milestone defines a safe additive bridge.

### 5.5 Core inspection

Core additions are limited to bounded, read-only snapshot interfaces required for RouterInfo and ClientServicesInfo. They expose data, not subsystem authority.

## 6. Dependency graph

```text
M001 Contract matrix and I2PControl foundation
    |
    v
M002 Domain model and persistence
    |
    +--> M003 AddressBook ------------------+
    |                                       |
    +--> M004 TunnelManager and stubs -------+--> M006 ClientServicesInfo
    |                                       |
    `--> M005 RouterInfo inspection ---------+
                                                |
                                                v
                                      M007 Conformance and closure
```

- M001 -> M002: hard.
- M002 -> M003: hard.
- M002 -> M004: hard.
- M002 -> M005: interface for DTOs/control-plane contracts; core inspection can begin after interfaces stabilize.
- M004/M005 -> M006: hard for accurate service state.
- M003/M004/M005/M006 -> M007: hard.

## 7. Milestones

### Milestone 001 — Contract matrix and I2PControl foundation

Class: invariant / infrastructure

Objective:

Create the exact conformance inventory and a frontend-independent, authenticated, bounded JSON-RPC service foundation.

Dependencies:

- no hard dependency;
- ADR-0001 and canonical documents are stable interfaces.

Deliverable boundary:

- conformance matrix and fixtures;
- independent Cargo feature/dependency ownership;
- additive configuration;
- authentication/token/version service;
- JSON-RPC parser, result/error DTOs, dispatcher, and method registry;
- typed control-plane interface and test double;
- startup/shutdown integration and protocol/security tests.

User or operator value:

A production-shaped I2PControl endpoint and stable implementation boundary exist for Proposal 170 without frontend coupling.

Exit conditions:

- base protocol and auth tests pass through the real listener;
- every Proposal 170 method/selector/tunnel type is present in the matrix and fixture inventory;
- no feature method falsely reports completion;
- later milestones can add handlers without replacing the server foundation.

Deferred work:

- AddressBook, TunnelManager, RouterInfo extension, and ClientServicesInfo semantics.

### Milestone 002 — Control-plane domain and persistence

Class: invariant / infrastructure

Objective:

Establish canonical administrative models and restart-safe storage before feature handlers depend on them.

Dependencies:

- M001 hard.

Deliverable boundary:

- exact tunnel/action/type/option domain;
- ownership and internal runtime state;
- backend traits and exhaustive registry;
- administrative address-book domain;
- versioned state schemas, atomic persistence, corruption handling, and fake adapters.

Exit conditions:

- all declared options round-trip without loss;
- interrupted writes do not replace valid state;
- every tunnel type resolves to a backend in tests;
- no persisted record starts a missing runtime service.

### Milestone 003 — AddressBook

Class: capability

Objective:

Implement all Proposal 170 AddressBook operations and RouterInfo address-book selectors as persistent administrative state.

Dependencies:

- M002 hard.

Exit conditions:

- all four books support exact list/lookup/add/update/delete behavior;
- Delete uses parameter presence semantics;
- SetConfig and SetSubscriptions persist and recover;
- destination validation is deterministic;
- runtime resolver behavior is unchanged.

### Milestone 004 — TunnelManager and explicit stubs

Class: capability / infrastructure

Objective:

Complete the TunnelManager public contract for every declared type while keeping missing data-plane work deferred.

Dependencies:

- M002 hard.

Exit conditions:

- every type supports parsing and configuration CRUD;
- every type has a real or unsupported backend;
- start/stop/restart and permitted `All` behavior are exact;
- unsupported start/restart fail deterministically;
- unsupported records never report active;
- existing startup-managed runtime ownership is represented truthfully.

### Milestone 005 — RouterInfo Proposal 170 inspection

Class: capability / infrastructure

Objective:

Implement every Proposal 170 RouterInfo selector from truthful bounded snapshots.

Dependencies:

- M002 interface;
- M003/M004 soft for their selector data.

Exit conditions:

- only requested keys are returned;
- exact types and nullability are enforced;
- logs are bounded and independently clearable;
- metrics are multi-consumer safe;
- core inspection is read-only and bounded;
- no placeholder state is fabricated.

### Milestone 006 — ClientServicesInfo

Class: capability

Objective:

Implement all service selectors from actual listener/session/registry state.

Dependencies:

- M004 and M005 hard;
- M003 soft only if shared administrative reporting is required.

Exit conditions:

- I2PTunnel, HTTPProxy, SOCKS, SAM, BOB, and I2CP selectors are exact;
- only requested sections appear;
- stubbed tunnels do not appear active;
- frontend presence does not alter results.

### Milestone 007 — Conformance, hardening, and strict closure

Class: invariant / polish

Objective:

Close every conformance row and prove scope, compatibility, persistence, security, and lifecycle correctness.

Dependencies:

- M003, M004, M005, and M006 hard.

Exit conditions:

- matrix-driven protocol evidence covers every field and type;
- negative, concurrent, restart, corruption, cancellation, and resource-bound tests pass;
- static guards prevent extension and frontend coupling;
- documentation distinguishes contract and runtime completeness;
- independent closure finds no unresolved high/medium defect.

## 8. Cross-cutting requirements

### Storage and migration

- additive, versioned administrative state;
- atomic same-filesystem writes;
- deterministic serialization;
- invalid new state cannot replace valid old state;
- unsupported and externally managed state remains explicit;
- no unreviewed migration of existing startup configuration.

### Protocol and compatibility

- exact Proposal 170 and base I2PControl behavior;
- no aliases, extension fields, extra statuses, or alternate envelopes;
- request IDs and selector filtering preserved;
- fixtures record compatibility assumptions.

### Security and authorization

- secure tokens and credential comparison;
- no token/private-key leakage;
- loopback-safe defaults;
- body, collection, nesting, and work limits;
- confined persistence paths;
- unauthorized requests fail before sensitive inspection or mutation.

### Concurrency, cancellation, and recovery

- listener and request tasks shut down through application cancellation;
- concurrent mutation is serialized or conflict-safe;
- partial writes recover deterministically;
- snapshot collection is bounded and does not deadlock router tasks;
- backend start/stop races cannot create duplicate or orphaned tasks;
- unsupported backends allocate no runtime resource.

### Observability and audit

- bounded log retrieval;
- sanitized operation failures;
- useful startup/listener diagnostics;
- no frontend event consumption;
- closure records contain actual command output and limitations.

### Performance and resource use

- request limits precede expensive deserialization or snapshot construction;
- peer/RouterInfo/log result construction is bounded;
- polling does not materially affect router progress;
- persistence does not block async runtime workers with unbounded filesystem work.

### Documentation and operations

- configuration and authentication instructions;
- exact support matrix;
- explicit runtime-stub disclosure;
- recovery behavior for invalid state;
- no frontend documentation implying controls exist.

## 9. Verification strategy

Verification proceeds at five levels:

1. unit tests for DTOs, exact keys/types, auth, validation, status mapping, and persistence;
2. handler tests against fake control-plane implementations;
3. HTTP listener integration tests with real JSON-RPC requests;
4. restart, corruption, concurrency, cancellation, and resource-limit tests;
5. matrix-driven end-to-end conformance and independent closure review.

Every milestone closure must distinguish tests run from tests planned or unavailable.

## 10. Risks and decision points

- Base I2PControl version/auth details may expose ambiguities requiring a compatibility-focused ADR.
- Exact Proposal 170 RouterInfo fields may require several core inspection adapters; each must remain read-only and bounded.
- Existing startup-managed tunnel state may not be fully observable; unobservable control must fail rather than fabricate.
- Preserving all future tunnel options can tempt untyped storage. The domain should type known fields while retaining exact lossless representation.
- Proposal 170 examples may conflict with established JSON-RPC envelopes; base protocol compatibility takes precedence and must be documented.
- Binding/TLS defaults require careful compatibility and security review during M001.

## 11. Completion definition

The subsystem closes only when the exact completion definition in `plans/000-long-term-specification.md` is demonstrably true and the closure record confirms no deferred tunnel implementation, frontend work, or router behavior change entered the workstream.

## 12. Milestone status

| Milestone | Status | Implementation plan | Closure record | Blockers |
|---|---|---|---|---|
| 001 | closed | `plans/implementation/i2pcontrol-proposal-170/001-contract-matrix-and-i2pcontrol-foundation.md` | `plans/closure/i2pcontrol-proposal-170/001-closure.md` | — |
| 002 | closing | `plans/implementation/i2pcontrol-proposal-170/002-control-plane-domain-and-persistence.md` | `plans/closure/i2pcontrol-proposal-170/002-closure.md` | — |
| 003 | closed | `plans/implementation/i2pcontrol-proposal-170/003-address-book-administrative-api.md` | `plans/closure/i2pcontrol-proposal-170/003-closure.md` | — |
| 004 | closed | `plans/implementation/i2pcontrol-proposal-170/004-tunnel-manager-contract-and-stubs.md` | `plans/closure/i2pcontrol-proposal-170/004-closure.md` | — |
| 005 | not started | — | — | M002 interface; later integration with M003/M004 |
| 006 | not started | — | — | M004 and M005 closure |
| 007 | not started | — | — | M003–M006 closure |
