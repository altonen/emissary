# I2PControl Proposal 170 Roadmap

Status: active corrective work

Current corrective planning baseline: `b35d9ad7295f6d7d8795a741c7942a5ff7a97f52` (`master` before corrective planning commits)

Canonical references:

- `plans/000-long-term-specification.md`
- `plans/001-terminology-and-domain-model.md`
- `plans/002-long-term-roadmap.md`
- `plans/003-planning-process.md`

Related ADRs:

- `plans/adrs/ADR-0001-proposal-170-contract-and-stub-boundary.md`

External contract:

- I2P Proposal 170, I2PControl API Expansion
- existing I2PControl JSON-RPC API

## 1. Purpose and ownership boundary

This subsystem owns the exact Proposal 170 administrative API contract for Emissary:

- I2PControl listener, TLS, authentication, JSON-RPC dispatch, and protocol DTOs;
- Proposal 170 RouterInfo selectors;
- AddressBook administrative stores and operations;
- TunnelManager configuration and lifecycle dispatch;
- explicit unsupported tunnel backends;
- ClientServicesInfo;
- bounded inspection adapters and persistence required by those methods;
- protocol, security, and closure evidence.

The subsystem consumes existing router identity, metrics, protocol listener state, address parsing, configured proxy/tunnel state, and narrowly scoped read-only core inspection.

It does not own router algorithms, I2P wire protocols, missing tunnel data planes, frontend behavior, startup-managed tunnel lifecycle migration, or runtime address-resolution policy.

## 2. Canonical invariants

- Proposal 170 wire names, JSON types, methods, actions, selectors, and tunnel types remain exact.
- Existing I2PControl authentication/version/JSON-RPC behavior remains compatible.
- The service operates independently of every frontend.
- Enabled production state contains no fake control implementation.
- Production store failures are explicit and never become transient fake success.
- Shared handlers and inspection adapters observe one canonical loaded service object.
- Missing or failing values are not fabricated as zero, false, empty, absent, or default success.
- Unsupported tunnel execution is explicit and never simulated.
- Every declared tunnel type has one registered real or unsupported backend.
- Unsupported definitions never report active runtime capability.
- Administrative address books do not silently change runtime resolution.
- I2PControl does not consume a single-owner event stream required elsewhere.
- Core inspection is bounded, read-only, neutral, and secret-free.
- I2PControl is actually served over TLS; plaintext does not reach JSON-RPC.
- Request, connection, collection, task, persistence, and response resource use is bounded.
- No router, NetDB, transport, tunnel-selection, peer-selection, congestion, or service-lifecycle behavior is changed to satisfy inspection.

## 3. Capabilities and infrastructure

### Capabilities

- Authenticate and call Proposal 170 through an independent TLS I2PControl listener.
- Query all Proposal 170 RouterInfo selectors from truthful sources or explicit protocol-compatible unavailable behavior.
- Manage all four Proposal 170 administrative address books.
- Create, edit, inspect, delete, and dispatch lifecycle actions for every declared tunnel type.
- Receive deterministic not-implemented lifecycle results for stubbed tunnel types.
- Query all ClientServicesInfo selectors from current service state.

### Infrastructure

- Exact request/response and error DTOs.
- Method registry and typed control-plane interfaces.
- Explicit production/test composition boundaries.
- Versioned persistent administrative state.
- Exhaustive tunnel backend registry.
- Bounded router/service inspection snapshots.
- Shared metrics and tracing-backed log snapshots.
- Matrix-driven conformance fixtures.
- Real TLS listener and pre-buffer request limits.

## 4. Non-goals

- Implementing missing client or server tunnel data planes.
- Migrating existing startup managers into Proposal 170 lifecycle ownership.
- Adding frontend controls or replacing frontend state paths.
- Changing address-book runtime precedence.
- Extending Proposal 170 with capability discovery, aliases, pagination, partial results, or richer status fields.
- Creating Java-I2P-specific internal classifications that have no meaningful Emissary equivalent.
- Refactoring core subsystems beyond bounded read-only inspection.
- Claiming runtime completeness for stubbed tunnel types.
- Adding release or publishing automation.

## 5. Historical implementation sequence

The original implementation was decomposed as:

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

M001–M004 remain historically closed unless later corrective evidence invalidates them.

M005–M007 were marked closed, but an independent post-implementation review at baseline `b35d9ad7295f6d7d8795a741c7942a5ff7a97f52` found material defects that invalidate strict closure.

## 6. Post-closure findings requiring corrective work

### Production composition and persistence

- Production state is constructed fake-first.
- Store creation/load failures can log a warning and continue with fake controls.
- RouterInfo constructs a second, unloaded TunnelManager control instead of sharing the loaded handler service.
- Temporary fallback storage may be used in nominal production inspection.
- State helpers suppress errors into empty/absent values.
- `ProductionControlPlane` contains unconditional tunnel placeholders.

### RouterInfo

- Several production selector groups return hard-coded zero, false, empty, `None`, or default DTO state.
- The original M005 closure explicitly documented medium-severity missing NetDB/peer sources and nevertheless declared the milestone closed.
- The control trait cannot distinguish unavailable, failed, absent, and real zero/empty states.

### ClientServicesInfo

- I2PTunnel inventory is captured at startup and is stale after live TunnelManager mutations.
- SAM session output is always empty.
- Configured/starting proxies can report enabled before actual bind.
- Missing observer/source state is indistinguishable from known disabled state.

### TLS and request hardening

- TLS configuration is built but discarded.
- The raw TCP listener is served directly, so documentation and logs claiming HTTPS are unsupported.
- Body size is checked after extraction into a complete `String`.
- Connection/handshake/pre-handler buffering is not bounded by the handler semaphore.
- Several adversarial tests assert only `is_ok() || is_err()` and therefore prove nothing.

### Closure process

- M007 implementation and “independent review” were attributed to the same agent.
- Static/manifest counts were treated as capability evidence.
- The registry was marked closed despite medium-severity findings and missing production evidence.

## 7. Corrective dependency graph

```text
M008 Production composition and durable-state integrity
    |
    v
M009 RouterInfo availability and truthfulness
    |
    +--------------------+--------------------+
    |                    |                    |
    v                    v                    v
M010 Bounded core    M011 ClientServices  M012 Real TLS and
router inspection    live state           request hardening
    |                    |                    |
    +--------------------+--------------------+
                         |
                         v
              M013 Independent reclosure
```

Dependency classes:

- M008 -> M009: hard.
- M009 -> M010: hard.
- M009 -> M011: hard, with M008 shared-control boundary also required.
- M009 -> M012: interface; M012 must reconcile current state construction but may execute in parallel with M010/M011.
- M010/M011/M012 -> M013: hard.
- M001–M004 -> M013: historical closure revalidation.

Only M008 is dependency-ready at registration time. Later plans remain blocked until their activation rules are satisfied.

## 8. Original milestone summaries

### Milestone 001 — Contract matrix and I2PControl foundation

Class: invariant / infrastructure

Historical objective: create the exact contract inventory and frontend-independent authenticated JSON-RPC foundation.

Current disposition: historically closed; TLS-serving and request-bound defects are owned by M012 and must be reconciled before final closure.

### Milestone 002 — Control-plane domain and persistence

Class: invariant / infrastructure

Historical objective: canonical administrative models, backend traits, and restart-safe generation storage.

Current disposition: historically closed; fail-closed production composition and shared service ownership are owned by M008.

### Milestone 003 — AddressBook

Class: capability

Historical objective: four persistent administrative books, operations, subscriptions, configuration, and RouterInfo selectors without runtime resolver adoption.

Current disposition: historically closed; M008/M013 must verify failures are not converted to fake/empty success.

### Milestone 004 — TunnelManager and explicit stubs

Class: capability / infrastructure

Historical objective: complete configuration/lifecycle API for every declared type with explicit unsupported backends.

Current disposition: historically closed; M008/M011/M013 must verify one shared store, live cross-method visibility, and inactive stubs.

### Milestone 005 — RouterInfo inspection

Class: capability / infrastructure

Historical objective: every selector from truthful bounded snapshots.

Current disposition: corrective pass required. M009 and M010 own closure.

### Milestone 006 — ClientServicesInfo

Class: capability

Historical objective: exact service selectors from actual listener/session/registry state.

Current disposition: corrective pass required. M011 owns closure.

### Milestone 007 — Conformance and strict closure

Class: invariant / polish

Historical objective: independent proof of complete production conformance.

Current disposition: corrective pass required. M013 supersedes this closure gate after M008–M012.

## 9. Corrective milestones

### Milestone 008 — Production composition and durable-state integrity

Class: invariant / infrastructure corrective pass

Objective:

- separate production and test construction;
- construct/load production stores once;
- share one canonical service object across handlers/inspection;
- fail closed on production initialization/load errors;
- propagate query failures instead of suppressing them;
- remove legacy production placeholders and temporary fallback stores.

Hard dependencies: none beyond stable M001–M004 interfaces.

Exit conditions:

- enabled production state cannot contain fakes;
- no production fallback-to-fake path;
- shared tunnel object identity is proven;
- store/query errors remain explicit;
- restart and fail-closed production tests pass;
- no high/medium finding remains in this boundary.

Implementation plan:

- `plans/implementation/i2pcontrol-proposal-170/008-production-composition-and-durable-state-integrity.md`

### Milestone 009 — RouterInfo availability and truthfulness

Class: invariant / capability corrective pass

Objective:

- create an exact selector source/availability map;
- distinguish unavailable, failure, absence, and real zero/empty state;
- make control interfaces fallible/nullable as required;
- eliminate all fabricated production defaults;
- establish stable grouped snapshot interfaces for M010.

Hard dependency: M008 closed.

Exit conditions:

- every selector has one source and unavailable rule;
- production defaults are removed;
- requested unavailable non-null selectors return explicit compatible errors;
- real zero/empty remains successful;
- only requested keys appear;
- no high/medium finding remains in the truthfulness boundary.

Implementation plan:

- `plans/implementation/i2pcontrol-proposal-170/009-router-info-availability-and-truthfulness.md`

### Milestone 010 — Bounded core router inspection

Class: capability / infrastructure corrective pass

Objective:

- add neutral bounded read-only core snapshots for transport, tunnels, NetDB, peers, bans, limits, and peer RouterInfo where Emissary has canonical state;
- map them through M009 without changing protocol handlers;
- retain explicit unavailable behavior for nonexistent/unsafe semantics rather than inventing values.

Hard dependency: M009 closed.

Exit conditions:

- real nonzero/current core state is proven through production adapter and listener tests;
- source loss produces errors rather than defaults;
- lists are bounded and not truncated;
- no private material or mutable authority escapes;
- no router behavior changes;
- M005 can pass independent reconsideration.

Implementation plan:

- `plans/implementation/i2pcontrol-proposal-170/010-bounded-core-router-inspection.md`

### Milestone 011 — ClientServicesInfo live state

Class: capability corrective pass

Objective:

- make I2PTunnel inventory current after every successful mutation;
- report proxy/I2CP/SAM enabled only from actual active listener state;
- add bounded current SAM session inspection;
- distinguish unavailable source from disabled service;
- preserve passive observation and exact BOB/stub behavior.

Hard dependencies: M008 and M009 closed.

Exit conditions:

- cross-method tunnel mutations are immediately visible;
- actual listener/session transitions are proven;
- active SAM sessions are not hard-coded empty;
- missing observers do not become disabled success;
- no lifecycle authority or scope creep;
- M006 can pass independent reconsideration.

Implementation plan:

- `plans/implementation/i2pcontrol-proposal-170/011-client-services-live-state.md`

### Milestone 012 — Real TLS and request resource hardening

Class: security / invariant corrective pass

Objective:

- use the TLS acceptor in the actual serving path;
- reject plaintext before JSON-RPC;
- enforce body/connection/handshake/request limits before unbounded work;
- define parser policy;
- replace tautological adversarial tests with exact production-listener evidence.

Dependencies: M008/M009 interfaces stable; may run in parallel with M010/M011 after M009.

Exit conditions:

- real TLS client succeeds and plaintext fails;
- body limits apply before full buffering;
- slow handshake/body and concurrency are bounded;
- permits/resources restore on every path;
- no tautological security/resource test remains;
- no high/medium security finding remains.

Implementation plan:

- `plans/implementation/i2pcontrol-proposal-170/012-real-tls-and-request-resource-hardening.md`

### Milestone 013 — Production conformance and independent reclosure

Class: invariant / verification / closure

Objective:

Independently review the frozen integration head and determine whether exact Proposal 170 completion is supportable through real TLS, production composition, durable stores, real/explicitly unavailable inspection, live service state, restart, concurrency, cancellation, corruption, and resource evidence.

Hard dependencies: M010, M011, and M012 closed; M001–M004 revalidated.

Exit conditions:

- complete requirement-to-evidence matrix;
- actual production-path tests for every method family;
- all original post-closure defects have regression evidence;
- reviewer is distinct from final implementation agent;
- no unresolved high/medium finding;
- roadmap/registry closure occurs only after the M013 closure record is accepted.

Implementation plan:

- `plans/implementation/i2pcontrol-proposal-170/013-production-conformance-and-independent-reclosure.md`

## 10. Cross-cutting requirements

### Storage and migration

- additive, versioned administrative state;
- atomic same-filesystem writes;
- deterministic serialization;
- invalid new state cannot replace valid old state;
- unsupported and externally managed state remains explicit;
- no unreviewed migration of existing startup configuration;
- no production fallback to fake/transient storage.

### Protocol and compatibility

- exact Proposal 170 and base I2PControl behavior;
- no aliases, extension fields, extra statuses, or alternate envelopes;
- request IDs and selector filtering preserved;
- unavailable/error semantics use existing compatible envelopes;
- fixtures record compatibility assumptions.

### Security and authorization

- actual TLS serving;
- secure tokens and credential comparison;
- no token/private-key leakage;
- loopback-safe defaults;
- connection, body, collection, nesting, task, and work limits;
- confined persistence paths;
- unauthorized requests fail before protected inspection or mutation.

### Concurrency, cancellation, and recovery

- listener and request tasks shut down through application cancellation;
- concurrent mutation is serialized or conflict-safe;
- partial writes recover deterministically;
- snapshot collection is bounded and does not deadlock router tasks;
- no cross-subsystem locks held together;
- unsupported backends allocate no runtime resource;
- stale service observers cannot overwrite current generations.

### Observability and evidence

- bounded, sanitized log retrieval;
- useful startup/listener diagnostics that describe actual behavior;
- no frontend event consumption;
- closure records contain actual command output, failures, skipped commands, and limitations;
- static guards supplement rather than replace behavioral tests;
- no tautological assertions count as evidence.

## 11. Verification strategy

Verification proceeds at six levels:

1. unit tests for DTOs, exact keys/types, auth, validation, availability, status mapping, and persistence;
2. handler tests against explicitly configured test controls;
3. production adapter tests against real/sentinel core owners and shared stores;
4. real TLS listener integration tests with authenticated JSON-RPC requests;
5. restart, corruption, concurrency, cancellation, source-loss, and resource-limit tests;
6. matrix-driven end-to-end conformance and independent closure review.

Every closure must distinguish tests run from tests planned, skipped, failed, or unavailable.

## 12. Risks and decision points

- Some Proposal RouterInfo fields may describe Java-I2P-specific semantics with no meaningful Emissary equivalent. They must remain explicit compatible unavailable/error behavior unless an accepted ADR defines a truthful mapping.
- Full peer/session collections may exceed safe bounds. Do not silently truncate or invent pagination.
- Core inspection must not expose mutable subsystem authority or private material.
- Actual TLS serving may require one narrowly feature-gated server dependency or lower-level Hyper integration; the choice must remain confined to `emissary-cli`.
- ClientServicesInfo SAM schema must be reconciled against the external contract before exposing session details.
- Existing startup-managed tunnels remain observable but not silently controllable.

## 13. Completion definition

The subsystem closes only when M013 independently confirms the exact completion definition in `plans/000-long-term-specification.md` and verifies that:

- unsupported tunnel data planes remain explicit deferred work;
- no frontend/runtime-resolver/router-behavior scope entered the workstream;
- production state is truthful and fail-closed;
- actual TLS and resource bounds are proven;
- no high/medium defect remains.

## 14. Milestone status

| Milestone | Status | Implementation plan | Closure record / corrective owner | Blockers |
|---|---|---|---|---|
| 001 | closed (historical; revalidate at M013) | `plans/implementation/i2pcontrol-proposal-170/001-contract-matrix-and-i2pcontrol-foundation.md` | `plans/closure/i2pcontrol-proposal-170/001-closure.md`; TLS correction M012 | — |
| 002 | closed (historical; revalidate at M013) | `plans/implementation/i2pcontrol-proposal-170/002-control-plane-domain-and-persistence.md` | `plans/closure/i2pcontrol-proposal-170/002-closure.md`; composition correction M008 | — |
| 003 | closed (historical; revalidate at M013) | `plans/implementation/i2pcontrol-proposal-170/003-address-book-administrative-api.md` | `plans/closure/i2pcontrol-proposal-170/003-closure.md` | — |
| 004 | closed (historical; revalidate at M013) | `plans/implementation/i2pcontrol-proposal-170/004-tunnel-manager-contract-and-stubs.md` | `plans/closure/i2pcontrol-proposal-170/004-closure.md` | — |
| 005 | corrective pass required | `plans/implementation/i2pcontrol-proposal-170/005-router-info-inspection.md` | M008–M010 | Fabricated/default production state and missing core inspection |
| 006 | corrective pass required | `plans/implementation/i2pcontrol-proposal-170/006-client-services-info.md` | M011 | Startup-stale tunnel inventory and empty SAM sessions |
| 007 | corrective pass required / superseded | `plans/implementation/i2pcontrol-proposal-170/007-conformance-hardening-and-strict-closure.md` | M012–M013 | Invalid security evidence and non-independent closure |
| 008 | closed | `plans/implementation/i2pcontrol-proposal-170/008-production-composition-and-durable-state-integrity.md` | `plans/closure/i2pcontrol-proposal-170/008-closure.md` | — |
| 009 | closed | `plans/implementation/i2pcontrol-proposal-170/009-router-info-availability-and-truthfulness.md` | `plans/closure/i2pcontrol-proposal-170/009-closure.md` | — |
| 010 | closed | `plans/implementation/i2pcontrol-proposal-170/010-bounded-core-router-inspection.md` | `plans/closure/i2pcontrol-proposal-170/010-closure.md` | — |
| 011 | ready | `plans/implementation/i2pcontrol-proposal-170/011-client-services-live-state.md` | pending | M010 closed |
| 012 | ready | `plans/implementation/i2pcontrol-proposal-170/012-real-tls-and-request-resource-hardening.md` | pending | M008/M009/M010 interfaces stable |
| 013 | blocked | `plans/implementation/i2pcontrol-proposal-170/013-production-conformance-and-independent-reclosure.md` | pending | M010, M011, M012 strict closure and M001–M004 revalidation |