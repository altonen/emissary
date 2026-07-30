# I2PControl Proposal 170 Milestone 013 — Production Conformance and Independent Reclosure

Status: closed

Planning baseline: `b35d9ad7295f6d7d8795a741c7942a5ff7a97f52` (`master` before corrective planning commits)

Activation rule:

- M008, M009, M010, M011, and M012 must each have an accepted closure record with disposition `closed`.
- M001–M004 must be re-reviewed for later invalidating findings.
- The closure reviewer must not be the implementation agent responsible for the final production changes under review.
- Before execution, replace this baseline with the reviewed integration head and freeze that head in the closure record.

Source roadmap:

- `plans/subsystems/i2pcontrol-proposal-170-roadmap.md#milestone-013--production-conformance-and-independent-reclosure`

Supersedes as a strict closure gate:

- `plans/implementation/i2pcontrol-proposal-170/007-conformance-hardening-and-strict-closure.md`
- `plans/closure/i2pcontrol-proposal-170/007-closure.md`

Canonical requirements:

- `plans/000-long-term-specification.md`, especially the Proposal 170 completion definition
- `plans/001-terminology-and-domain-model.md`
- `plans/002-long-term-roadmap.md`
- `plans/003-planning-process.md`
- `plans/adrs/ADR-0001-proposal-170-contract-and-stub-boundary.md`
- all accepted M001–M012 implementation and closure records
- I2P Proposal 170 and base I2PControl contract

Primary class: invariant / verification / closure

## 1. Objective

Independently determine whether the production Emissary I2PControl implementation truthfully satisfies the exact Proposal 170 API contract and the project's scope boundary after M008–M012, using real TLS listener, production composition, durable stores, production inspection sources, live service state, adversarial limits, restart behavior, and cross-method consistency evidence.

M013 is not a feature implementation milestone. If activation review finds a capability, architecture, security, or truthfulness defect, stop and create a narrowly owned corrective plan. Do not repair material defects inside the closure gate and then self-approve them.

## 2. Why the prior closure is invalid

The prior M007 closure cannot be relied upon because it:

- accepted M005 despite documented medium-severity missing production sources;
- accepted successful zero/default/empty RouterInfo placeholders as truthful inspection;
- accepted startup-stale ClientServicesInfo tunnel inventory;
- accepted permanently empty SAM session output;
- did not prove one shared loaded production tunnel service across handlers;
- allowed production persistence failures to fall back to fake controls;
- described the endpoint as HTTPS while the TLS acceptor was discarded and the raw listener was served;
- cited tautological parser/resource tests as hardening evidence;
- combined implementation and “independent” review under the same agent;
- marked the registry closed despite unresolved findings that violated registry rules.

M013 must explicitly verify that every one of these failure modes is now prevented by production code and regression tests.

## 3. Invariants under review

- Exact Proposal 170 and base I2PControl wire names, types, methods, selectors, actions, tunnel types, validation, IDs, and envelopes.
- No public extensions, aliases, capability flags, richer statuses, pagination, or partial-result shapes.
- Actual TLS serving; plaintext does not reach JSON-RPC.
- Enabled production state contains no fake control implementation.
- Production stores fail closed and are constructed/loaded once.
- Shared administrative/runtime inspection consumers observe one canonical service object.
- No failed query becomes empty, absent, zero, false, or default success.
- RouterInfo distinguishes unavailable, absent, and real zero/empty state.
- Real core inspection is bounded, read-only, neutral, secret-free, and frontend-independent.
- ClientServicesInfo reflects current listener/session/tunnel state.
- Unsupported tunnel data planes remain explicit stubs and never report active.
- Existing startup-managed tunnel ownership remains truthful and is not silently migrated.
- Administrative address books remain separate from runtime resolution.
- No router, NetDB, transport, peer-selection, tunnel construction, congestion, or service lifecycle behavior change entered the workstream.
- No single-owner event subscriber is consumed.
- Persistence is versioned, deterministic, atomic, path-confined, and restart-safe.
- Request, connection, body, collection, task, and response resource use is bounded.
- Secrets/private material are absent from responses, logs, errors, fixtures, and closure artifacts.
- Closure evidence distinguishes run, skipped, failed, and unavailable commands.

## 4. Explicit non-goals

- Implementing any missing tunnel data plane.
- Migrating startup-managed tunnel ownership.
- Adopting Proposal 170 address books for runtime resolution.
- Adding frontend controls.
- Broad router/core refactoring.
- Release automation or publishing.
- Weakening the conformance matrix to match implementation.
- Reclassifying a correctness/security defect as documentation or polish.
- Self-review by the final implementation agent.

## 5. Required independent review process

### 5.1 Freeze the reviewed head

The closure reviewer must:

1. identify the exact integration head SHA;
2. verify the working tree/remote branch being reviewed;
3. list M008–M012 implementation and closure commits;
4. confirm no later commit invalidates their evidence;
5. record the frozen head at the top of the M013 closure record.

Any production change after the head is frozen invalidates closure until the reviewer reconciles and re-runs affected evidence.

### 5.2 Activation audit

Before running broad tests, inspect each dependency closure for:

- unresolved findings;
- skipped required evidence;
- implementation-agent assertions presented as proof;
- source maps with unsupported or unknown rows;
- environment substitutions that bypass production behavior;
- static guards without behavioral evidence;
- test-only seams not consumed by production.

If a high/medium defect exists, stop M013 and register a corrective plan under the owning milestone.

### 5.3 Requirement-to-evidence matrix

Create a machine-checkable or reviewable matrix covering at least:

- every method;
- every RouterInfo selector;
- every AddressBook operation and book;
- every TunnelManager action/type/field/applicability rule;
- every ClientServicesInfo selector;
- every standard/auth/version/error envelope;
- persistence and recovery rules;
- TLS and request bounds;
- ownership and scope invariants.

Each row must name:

- contract source;
- production implementation owner;
- production data source or explicit unsupported inspection behavior;
- exact fixture/test;
- test layer: unit, adapter, listener, restart, adversarial, static;
- current result;
- limitation/finding if any.

A row cannot pass solely because a constant or DTO exists.

## 6. Required production-path harness

Use one isolated harness that exercises the actual enabled production path:

- temporary base path;
- generated or fixture TLS material;
- actual TLS server loop;
- production authentication/token service;
- production state constructor with no fakes;
- production AddressBook and TunnelManager stores;
- production RouterInfo adapter and bounded core inspection;
- production ClientServicesInfo control/observers;
- ephemeral listener/proxy/protocol ports;
- deterministic core/service fixtures through supported seams;
- structured shutdown and restart;
- no external network dependency.

The harness must not:

- bypass TLS;
- call handlers directly for end-to-end claims;
- substitute fake stores for production persistence;
- reopen stores independently for different methods;
- use test-only default values as source state;
- rely on UI state;
- parse logs to infer router/service state.

## 7. Ordered review packages

### RP1 — Contract and manifest reconciliation

- Re-read Proposal 170/base I2PControl.
- Reconcile normative docs, executable manifests, production constants, handlers, DTOs, and fixtures.
- Verify exact counts and exact sets, but also verify every row has production behavior.
- Resolve documentation inconsistencies, especially ClientServicesInfo response types and RouterInfo unavailable semantics.
- Verify unsupported inspection rows are documented and return explicit compatible errors rather than fabricated values.

### RP2 — Production composition and persistence

Prove:

- no production fake controls;
- address-book/tunnel stores construct/load once;
- RouterInfo/ClientServices/TunnelManager share the same tunnel service;
- persistence failure aborts or errors explicitly;
- no temporary fallback path;
- read errors do not become empty/absent results;
- mutations return success only after durable commit;
- restart reconstructs coherent state.

### RP3 — TLS, auth, and JSON-RPC

Prove through the actual listener:

- real TLS handshake and protected request success;
- plaintext rejection;
- certificate/key failure handling;
- invalid credentials/token/version behavior;
- request ID preservation and notifications;
- exact errors and named params;
- body/nesting/duplicate-key/batch policy;
- connection/handshake/body/request/concurrency limits;
- shutdown and immediate restart;
- no secrets/raw body leakage.

### RP4 — AddressBook

For all four books:

- list, lookup, add, update, specific delete, delete-all;
- presence semantics;
- SetSubscriptions and SetConfig;
- isolation and restart;
- corruption/failure behavior;
- RouterInfo administrative selectors consume the same service;
- runtime resolver behavior remains unchanged.

### RP5 — TunnelManager and stubs

For all twelve exact tunnel types:

- parse and minimum valid Create;
- Get/Edit/Rename/Delete;
- Start/Stop/Restart;
- permitted/rejected `All` actions;
- ownership errors for startup-managed entries;
- backend registry exhaustiveness;
- unsupported Start/Restart deterministic errors;
- unsupported status remains inactive;
- no runtime resources allocated by stubs;
- persistence and cross-method visibility.

No closure claim may imply missing tunnel data-plane implementation.

### RP6 — RouterInfo truthfulness

For every selector/group:

- only requested keys;
- exact type/nullability;
- actual retained/event/store/core source;
- real nonzero/empty fixtures;
- explicit unavailable/error behavior where approved;
- source failure distinct from real zero/empty/absent;
- bounded lists and oversize errors;
- coherent group snapshots;
- no default production DTO fallback;
- no event subscriber interference;
- no behavior mutation.

### RP7 — ClientServicesInfo live state

Prove:

- live TunnelManager Create/Edit/Rename/Delete visibility without restart;
- HTTP/SOCKS enabled only after actual bind and while active;
- listener failure/exit updates state;
- I2CP/SAM actual listener state;
- bounded current SAM session state;
- source unavailable distinct from disabled;
- exact BOB value;
- unsupported tunnel definitions never active;
- only requested sections;
- no lifecycle authority.

### RP8 — Recovery, concurrency, cancellation, and resource evidence

Run deterministic tests for:

- corrupt newest generation with valid fallback;
- all generations corrupt;
- unsupported schema;
- permission/path/symlink failure;
- concurrent mutations and queries;
- cancellation during store/query/request work;
- shutdown during active requests;
- immediate restart;
- bounded polling under changing core/service state;
- oversize collections/responses;
- slow handshake/body;
- permit/channel/lock release;
- no deadlocks or orphaned tasks.

### RP9 — Static scope and secret review

Verify:

- no HTTP/JSON-RPC/TLS dependency in core;
- no frontend imports in I2PControl control/inspection;
- no private key/session key types in DTOs;
- no direct persistence reads in handlers;
- no fake fallback strings/paths;
- no hard-coded RouterInfo defaults;
- no startup-only ClientServices tunnel cache;
- no discarded TLS acceptor/raw server path;
- no tautological adversarial assertions;
- no new tunnel data-plane implementation;
- no runtime address-book integration;
- no router algorithm changes.

Static evidence supplements but does not replace behavioral tests.

## 8. Required test layers

### Unit

- exact DTO serialization/validation;
- typed errors and availability;
- persistence atomicity;
- backend/stub semantics;
- snapshot bounds;
- token and parser policy.

### Adapter

- production adapters against real/sentinel owners;
- nonzero state;
- unavailable and failing state;
- shared object identity;
- no default fallback.

### Listener

- real TLS Authenticate and every protected method family;
- exact response/error envelope;
- request/resource limits;
- plaintext rejection.

### Restart

- durable AddressBook/TunnelManager state;
- volatile tokens/listeners/sessions;
- source reconstruction;
- stale generation rejection.

### Concurrency/cancellation

- multi-client mutation/query;
- service/session changes during polling;
- shutdown and timeouts;
- resource restoration.

### Static

- architecture, dependency, vocabulary, secret, and scope guards.

No test may pass with an assertion equivalent to `x || !x`, `is_ok() || is_err()`, or “did not panic” when exact behavior is required.

## 9. Verification commands

Minimum commands, adjusted only for current feature names:

```bash
cargo fmt --all -- --check
cargo check -p emissary-core --features std,events
cargo check -p emissary-cli --no-default-features --features i2pcontrol
cargo test -p emissary-core
cargo test -p emissary-cli --no-default-features --features i2pcontrol
cargo test -p emissary-cli --no-default-features --features i2pcontrol --test production_composition
cargo test -p emissary-cli --no-default-features --features i2pcontrol --test router_info_truthfulness
cargo test -p emissary-cli --no-default-features --features i2pcontrol --test router_info_production
cargo test -p emissary-cli --no-default-features --features i2pcontrol --test client_services_live
cargo test -p emissary-cli --no-default-features --features i2pcontrol --test i2pcontrol_tls
cargo test -p emissary-cli --no-default-features --features i2pcontrol --test request_resource_limits
cargo test -p emissary-cli --no-default-features --features i2pcontrol --test persistence_concurrency
cargo test -p emissary-cli --no-default-features --features i2pcontrol --test golden_fixtures
cargo test -p emissary-cli --no-default-features --features i2pcontrol --test conformance_manifest
cargo test -p emissary-cli --no-default-features --features i2pcontrol --test static_guards_m013
```

Clippy:

```bash
cargo clippy -p emissary-core --features std,events --all-targets -- -D warnings
cargo clippy -p emissary-cli --no-default-features --features i2pcontrol --all-targets -- -D warnings
```

Feature compatibility where environment permits:

```bash
cargo check -p emissary-cli --features ui,i2pcontrol
cargo test --workspace --features emissary-cli/i2pcontrol,emissary-core/events --no-default-features
```

For every skipped or failed command, record:

- exact command;
- exact reason/output summary;
- whether it blocks closure;
- what narrower evidence was run;
- why that evidence is or is not equivalent.

Do not label environmental absence as passing.

## 10. Acceptance criteria

1. M008–M012 are strictly closed with reviewed evidence.
2. M001–M004 remain valid after corrective changes.
3. The reviewed head is frozen and recorded.
4. Final review is performed by a reviewer distinct from the final implementation agent.
5. Every contract row names a production source/behavior and executable evidence.
6. Exact method/selector/action/type/key sets match Proposal 170/base I2PControl with no extras.
7. Actual TLS listener and plaintext rejection are proven.
8. Enabled production state contains no fake controls or temporary fallback stores.
9. Shared store/control identity is proven across methods.
10. Persistence failures and query failures remain explicit.
11. AddressBook operations are durable, isolated, restart-safe, and runtime-resolution-neutral.
12. TunnelManager supports exact CRUD/actions for all types with explicit inactive stubs.
13. No missing tunnel runtime/data plane is implemented or implied.
14. RouterInfo contains no fabricated successful default.
15. Real zero/empty state is distinct from unavailable/failure.
16. Core inspection is bounded, read-only, neutral, secret-free, and event-subscriber-safe.
17. ClientServicesInfo reflects current tunnel/listener/session state.
18. SAM active session state is not hard-coded empty.
19. Request/connection/body/collection/task/response limits are concrete and tested.
20. Tautological tests have been removed.
21. Restart, cancellation, corruption, concurrency, and shutdown evidence passes.
22. Secrets/private material are absent from all outward and evidence surfaces.
23. Headless and UI-feature builds remain equivalent for I2PControl state where environment permits verification.
24. No router/network/frontend/runtime-resolver scope creep is present.
25. Documentation accurately distinguishes API contract completeness, explicit unsupported inspection semantics, and deferred tunnel runtime completeness.
26. No unresolved high or medium finding remains.
27. The exact completion statement is supportable without qualification beyond explicitly deferred real tunnel data planes and documented protocol-compatible unavailable inspection semantics.

## 11. Stop conditions

Stop and create/register a corrective plan if:

- any dependency closure contains an unresolved high/medium finding;
- any requested production method uses fake/default/stale state;
- actual TLS or request bounds are not proven;
- any RouterInfo/ClientServices result fabricates absence;
- any full result is silently truncated;
- a required test bypasses production composition/listener/persistence;
- compatibility requires an unresolved ADR;
- closure would need to weaken the matrix or completion definition;
- final implementation and review cannot be separated.

M013 must not absorb material production fixes.

## 12. Closure record requirements

Create `plans/closure/i2pcontrol-proposal-170/013-closure.md` containing:

- frozen reviewed head;
- reviewer identity/role and separation from implementation;
- implementation/closure commit list for M008–M012;
- dependency activation audit;
- complete requirement-to-evidence matrix;
- exact commands and outcomes;
- real TLS transcript and plaintext rejection evidence;
- production composition/shared-state evidence;
- persistence/restart/corruption evidence;
- RouterInfo source/truthfulness evidence;
- ClientServices temporal/live-state evidence;
- concurrency/cancellation/resource evidence;
- static scope/secret review;
- platform/environment limitations;
- unresolved findings by severity;
- one disposition: `closed`, `conditionally closed`, `corrective pass required`, or `blocked`.

Strict closure is permitted only with zero unresolved high or medium findings. If closed, update the roadmap and registry in a separate planning commit after the closure record is accepted.