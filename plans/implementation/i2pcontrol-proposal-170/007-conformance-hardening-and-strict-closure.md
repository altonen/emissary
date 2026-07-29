# I2PControl Proposal 170 Milestone 007 — Conformance, Hardening, and Strict Closure

Status: closed

Planning baseline: `ec289c77183d4f1010829ff255d8dbe90a941ad8` (`master`)

Production-code baseline described by the planning system: `9b43484a21d5a1291c4881cdae62a36c527f8c0f`

Activation rule:

- M003, M004, M005, and M006 must each have a closure record with status `closed`.
- M001 and M002 must remain closed with no later finding that invalidates their contract, authentication, domain, or persistence boundaries.
- Before implementation begins, the agent MUST replace the baseline above with the reviewed integration head, inspect every Proposal 170 implementation and closure record, reconcile the conformance matrix against production code, and enumerate all residual findings.
- This prewritten plan is the final release gate and remains blocked until every capability milestone closes.

Source roadmap:

- `plans/subsystems/i2pcontrol-proposal-170-roadmap.md#milestone-007--conformance-hardening-and-strict-closure`

Canonical requirements:

- `plans/000-long-term-specification.md` in full, especially `#12-completion-definition`
- `plans/001-terminology-and-domain-model.md`
- `plans/002-long-term-roadmap.md#milestone-m007--conformance-hardening-and-strict-closure`
- `plans/003-planning-process.md`

Applicable ADRs:

- `plans/adrs/ADR-0001-proposal-170-contract-and-stub-boundary.md`
- any later accepted ADR explicitly linked by M001–M006 closure records

Primary class: invariant / polish

## 1. Objective

Demonstrate, harden, and independently verify complete Proposal 170 API conformance without adding new product scope.

M007 is not a feature-development phase. It must:

- reconcile the canonical conformance matrix with actual production code;
- close every method, selector, action, tunnel type, field, type, validation, error, persistence, and backend row with executable evidence;
- exercise the real HTTPS endpoint, authentication, handlers, production stores, production inspection adapters, and passive service registry;
- test negative, adversarial, concurrent, cancellation, restart, corruption, platform, and resource-bound behavior;
- add static guards preventing future protocol expansion, frontend coupling, fabricated state, and stub-runtime leakage;
- complete operator and support documentation;
- produce an independent closure record that permits the exact completion statement in the long-term specification.

No incomplete capability may be hidden by changing the matrix, weakening acceptance criteria, or reclassifying a correctness gap as polish.

## 2. Why this milestone is blocked

Hard dependencies:

- M003 closes AddressBook and address-book RouterInfo selectors.
- M004 closes TunnelManager and explicit stubs.
- M005 closes all RouterInfo selectors and read-only inspection.
- M006 closes ClientServicesInfo.

M007 cannot serve as a substitute implementation milestone. If activation review finds a missing feature, incorrect semantics, unsafe ownership, or fabricated value, create a new corrective implementation plan under the owning milestone/subsystem and keep M007 blocked.

M007 may fix narrow test infrastructure, documentation, static guards, deterministic formatting, bounded diagnostics, and low-risk integration defects. It must stop when correction requires architecture or capability implementation.

## 3. Current implementation evidence expected at activation

The activation review must verify, not assume, that production contains:

### Foundation

- independent `i2pcontrol` feature/dependency ownership;
- frontend-independent HTTPS listener;
- exact base I2PControl authentication/version/token behavior;
- bounded JSON-RPC request handling;
- exact error/result envelopes;
- typed method and selector registries;
- secure configuration and lifecycle integration.

### Domain and persistence

- exact tunnel/action/option domain;
- exhaustive backend registry;
- unsupported and fake backends;
- four administrative address-book stores;
- versioned deterministic generation persistence;
- validation-before-activation;
- corruption fallback and all-corrupt failure;
- path confinement, redaction, and concurrency control.

### AddressBook

- exact operations and presence semantics;
- durable four-book CRUD;
- SetSubscriptions and SetConfig;
- destination validation;
- RouterInfo administrative selectors;
- no runtime resolver integration.

### TunnelManager

- all exact types/actions/options;
- durable CRUD for all types;
- lifecycle dispatch and fencing;
- exact permitted `All` behavior;
- explicit unsupported start/restart;
- inactive unsupported status;
- truthful startup-managed ownership;
- no missing data-plane implementation.

### RouterInfo

- every exact selector;
- startup/current identity and RouterInfo;
- bounded logs/clear;
- shared cumulative and rolling metrics;
- read-only core inspection;
- tunnel/queue/network/peer/ban/limit/session data;
- M003/M004 integrations;
- only-requested fields and no fabrication.

### ClientServicesInfo

- exact six selectors;
- passive proxy registry;
- I2CP/SAM listener/session snapshots;
- exact BOB unavailable result;
- inactive stubs;
- no lifecycle authority.

Any absent production boundary is a blocking implementation gap.

## 4. Invariants that must not regress

- Proposal 170 is implemented exactly, not extended.
- Base I2PControl authentication/version/JSON-RPC compatibility remains exact.
- Every declared tunnel type has a real or explicit unsupported backend.
- Unsupported tunnel configuration CRUD is functional.
- Unsupported tunnel execution is explicit, deterministic, and resource-free.
- Stubbed tunnels never report running or traffic capability.
- Existing startup-managed task ownership remains truthful.
- RouterInfo and ClientServicesInfo never fabricate state.
- All administrative state is versioned, bounded, durable, and path-confined.
- Administrative books do not change runtime resolution.
- No frontend is required and no frontend state is authoritative.
- Core inspection is bounded and read-only.
- I2PControl does not consume frontend events.
- No HTTP/JSON-RPC server dependency enters core.
- No secrets/private material enter responses, logs, errors, fixtures, or closure artifacts.
- Complete results are not silently truncated.
- Resource use is bounded before expensive work.
- Closure evidence records failures, skipped commands, and environmental limits honestly.

## 5. Scope

### In scope

- Executable conformance manifest and coverage checker.
- Golden request/response/error fixtures.
- Real-listener end-to-end protocol tests.
- Cross-method state consistency tests.
- Authentication/TLS/JSON-RPC adversarial tests.
- Persistence failure/corruption/restart/platform tests.
- Mutation/lifecycle/query contention and cancellation tests.
- Request/result/memory/task/queue resource-bound tests.
- Static ownership, protocol, dependency, and secret guards.
- Reference-client/reference-implementation compatibility fixtures where available.
- Documentation, support matrix, configuration examples, recovery guidance, and limitation disclosures.
- Independent final review and closure record.
- Narrow corrections that do not alter architecture or add missing capability.

### Explicitly out of scope

- Implementing a missing method/selector/action/backend.
- Implementing missing tunnel data planes.
- Migrating existing startup task ownership.
- Runtime address-book adoption.
- Frontend controls.
- New I2PControl fields, methods, aliases, pagination, capabilities, or richer statuses.
- Router behavior or algorithm changes.
- Broad refactoring for style/performance without a measured release blocker.
- Weakening the conformance matrix to match production defects.
- Marking the subsystem closed while a high/medium finding remains.

## 6. Required production and verification changes

### Canonical executable conformance manifest

Promote the M001 matrix into a machine-checkable manifest or equivalent generated test source. It must enumerate:

- all I2PControl methods required for Proposal 170 exposure;
- all Proposal 170 RouterInfo selectors;
- AddressBook operation modes and fields;
- TunnelManager actions, types, fields, applicability, statuses, and `All` rules;
- ClientServicesInfo selectors;
- exact request/response keys;
- JSON types and nullability;
- selector/presence rules;
- authentication/version requirements;
- error/result classification;
- implementation owner/source;
- bounds and secret classification;
- golden fixture/test identifiers;
- production support classification for tunnel runtime backends.

The checker must fail for:

- manifest row without handler/source/test;
- registered protocol item absent from manifest;
- duplicate key/type/action registration;
- missing tunnel backend;
- fixture with extra or missing result fields;
- undocumented null/error behavior;
- implementation support status inconsistent with backend registry.

Canonical external names must have one source. Do not duplicate hand-maintained lists across tests and production without a consistency guard.

### Golden fixture corpus

Create checked-in sanitized fixtures for:

- successful Authenticate and token use;
- every Proposal 170 selector alone;
- representative multi-selector requests;
- all AddressBook valid operation modes;
- AddressBook invalid/conflicting modes;
- all twelve TunnelManager types with minimum valid create/get/edit/delete;
- each TunnelManager action;
- unsupported start/restart/stop;
- permitted/rejected `All` cases;
- startup-managed ownership failures;
- every ClientServicesInfo selector;
- all standard JSON-RPC/auth/version/invalid-param errors;
- null/unavailable and response-too-large behavior;
- BOB exact value;
- log clear behavior.

Fixtures must:

- use no real secrets, private keys, credentials, or personal destinations;
- be deterministic across platforms where the protocol permits;
- separate variable runtime values from exact structural assertions;
- assert exact absence of extension fields;
- record the contract source/reference and manifest row IDs.

### End-to-end production-path harness

Build one test harness that starts the real Emissary application/control service with:

- isolated temporary base path;
- deterministic keys/router fixtures where safe;
- ephemeral control/proxy/protocol ports;
- generated test TLS material confined to the test directory;
- production authentication, dispatcher, handlers, stores, inspection adapters, and service registry;
- fake runtime backends only where M004 intentionally uses unsupported/fake behavior for deterministic mechanism tests;
- no external network dependency.

The harness must support:

- cold start;
- authenticated requests;
- service/router fixture injection through supported test seams;
- process/service reconstruction against retained state;
- persistence failpoints;
- port conflicts;
- cancellation and shutdown;
- multi-client concurrency;
- corruption injection;
- headless and UI-feature compilation/execution variants.

Do not create a test-only code path that bypasses production parsing/authentication/persistence.

### Authentication, TLS, and JSON-RPC hardening

Test and harden:

- HTTPS-only listener; plaintext rejection;
- secure loopback/default binding;
- invalid/expired/unknown tokens;
- token restart invalidation/persistence semantics exactly defined by M001;
- token and credential redaction;
- version negotiation boundaries;
- duplicate/missing request IDs;
- null/string/number IDs according to JSON-RPC support;
- named parameters only where required;
- malformed JSON;
- invalid request objects;
- unknown methods;
- wrong/missing/extra params;
- duplicate JSON keys according to parser policy;
- excessive nesting, strings, arrays, maps, and body size;
- slow/incomplete bodies and request deadlines;
- concurrent authentication attempts and token-store bounds;
- listener port conflict and unexpected task failure;
- graceful shutdown and immediate restart.

Any change to auth/version behavior requires compatibility review, not opportunistic hardening.

### Cross-method consistency tests

Prove the same canonical state appears consistently:

- AddressBook mutation -> AddressBook read/list -> RouterInfo address-book selector;
- SetSubscriptions/SetConfig -> RouterInfo selectors;
- TunnelManager create/edit/delete -> TunnelManager get -> RouterInfo I2PTunnel selector -> ClientServicesInfo I2PTunnel section where applicable;
- unsupported start/restart -> inactive TunnelManager get/status -> inactive ClientServicesInfo;
- startup-managed inventory -> RouterInfo/ClientServicesInfo consistent ownership/status;
- log-producing operation -> RouterInfo logs -> logs.clear -> empty/updated logs;
- router metrics/core fixture -> RouterInfo selectors;
- SAM/I2CP/proxy fixture -> ClientServicesInfo and related RouterInfo fields where applicable.

The test must fail if one method reads a persistence file while another reads the canonical service and produces divergent state.

### Persistence and recovery hardening

Run production-path tests for every store and combined state root:

- first initialization;
- deterministic round trip;
- concurrent mutations;
- cancellation before/after publication;
- failure at write/flush/sync/rename/activation/cleanup;
- partial temporary files;
- corrupt newest generation with valid fallback;
- all generations corrupt;
- unsupported schema version;
- oversize state;
- excessive generation count;
- cleanup/retention failure;
- symlink/path traversal attempts;
- restrictive permission behavior;
- disk-full/permission-denied simulation where practical;
- abrupt process termination at deterministic failpoints;
- Windows, macOS, and Linux publication/reload behavior.

Verify that current `router.toml`, runtime address-book files, startup tunnel key files, and unrelated router storage remain unchanged.

### Concurrency, cancellation, and lifecycle hardening

Test interleavings including:

- concurrent AddressBook upsert/delete/list;
- SetSubscriptions racing SetConfig;
- TunnelManager create/edit/rename/delete races;
- start/stop/restart races;
- rename/delete racing backend completion;
- concurrent and cancelled `All`;
- RouterInfo polling during store mutation and core state changes;
- log read/clear/write races;
- service-registry producer replacement and stale updates;
- shutdown during persistence, core inspection, and lifecycle requests;
- immediate restart after shutdown;
- request timeout versus operation completion;
- backend panic/error containment using fakes;
- bounded channel saturation and busy behavior.

Use deterministic barriers, paused time, and failpoints. Avoid flaky sleep-based tests where possible.

### Resource and denial-of-service hardening

Measure and assert bounds for:

- HTTP connections and in-flight requests;
- authentication tokens;
- request body/nesting/collection sizes;
- AddressBook entries and result bytes;
- tunnel definitions, options, access lists, custom options, and `All` concurrency;
- log entries/bytes;
- peer identities, RouterInfos, active stats, bans, tunnel records, and queues;
- SAM sessions and I2PTunnel records;
- core inspection channels and work per query;
- persistence generations and scan work;
- background tasks created by I2PControl;
- memory growth under repeated polling/mutation;
- shutdown time with saturated clients.

Tests must prove bounds are enforced before unbounded allocation or fan-out. A response exceeding the exact complete-result budget must fail, not truncate or add pagination.

### Protocol exactness static guards

Add source/generated guards that fail when:

- a Proposal 170 method/selector/type/action is missing;
- an extra public method/selector/type/action/status is added under this workstream;
- a handler returns fields not in its manifest result shape;
- selector handlers return unrequested keys;
- unsupported state is serialized as a new public status;
- `All` is accepted for a forbidden action;
- Delete presence semantics regress;
- unknown top-level tunnel fields are accepted as extensions;
- BOB returns a non-exact value;
- response truncation/cursors/capabilities are introduced.

Prefer generated exact-set comparisons over fragile text grep alone.

### Ownership and dependency static guards

Fail builds/tests when:

- I2PControl imports frontend/UI modules;
- I2PControl consumes `EventSubscriber`;
- handlers directly use filesystem writes or persistence paths;
- handlers write `router.toml` or runtime address-book paths;
- handlers spawn/bind/create SAM/I2CP/tunnel resources;
- unsupported backends import or call resource constructors;
- core depends on Axum, JSON-RPC server, TLS listener, or application persistence dependencies;
- mutable core manager handles appear in public inspection types;
- current runtime address-book resolution imports Proposal 170 administrative stores;
- startup managers import Proposal 170 mutation/lifecycle authority.

A dependency diff and reviewed architecture boundary must be included in closure evidence.

### Secret and sensitive-data guards

Scan/test production code, fixtures, logs, errors, Debug output, and closure artifacts for:

- authentication credentials/tokens;
- generated private keys;
- destination private material;
- proxy passwords;
- full sensitive tunnel definitions;
- private session/tunnel keys;
- arbitrary filesystem paths;
- raw request bodies.

Use synthetic canary secrets and assert absence from all sinks. Do not commit real generated certificates/private keys; generate them during tests or commit clearly non-secret fixture material where appropriate.

### Compatibility/reference testing

Where practical and legally/technically available:

- replay official Proposal 170 examples after correcting only known malformed envelope presentation according to M001;
- replay sanitized fixtures derived from the Java reference implementation;
- run an existing I2PControl client against Emissary;
- compare exact method/error/key/type behavior;
- record discrepancies and their governing contract decision.

Reference implementations clarify ambiguity but do not authorize protocol expansion. Tests must remain network-independent and checked in.

### Platform and feature matrix

Required build/test matrix:

- Linux headless I2PControl;
- Linux UI plus I2PControl;
- macOS headless and relevant UI build;
- Windows headless and relevant UI build;
- workspace default features;
- workspace all features;
- `emissary-core` no-default/no-std combinations already supported;
- i2pcontrol disabled, proving no behavior/dependency regression;
- supported Rust MSRV/current stable policy if defined by repository.

Platform-specific persistence/TLS/listener failures must be recorded rather than inferred from one OS.

### Documentation and support matrix

Complete:

- installation/build feature instructions;
- configuration and secure binding;
- authentication/token usage;
- exact method examples;
- AddressBook administrative-only semantics;
- TunnelManager API/runtime support table for all twelve types;
- exact stub start/restart behavior;
- startup-managed ownership behavior;
- RouterInfo selector source/null/bounds notes;
- ClientServicesInfo configured/listening/session semantics;
- persistence/recovery/corruption guidance;
- log retrieval/clear/redaction limits;
- security/resource limits;
- troubleshooting and diagnostics;
- explicit non-goals and deferred work;
- accurate completion statement.

Documentation must not imply that missing tunnels carry traffic or that frontend controls exist.

### Independent closure review

After implementation/hardening lands, a reviewer other than the implementation agent must create:

```text
plans/closure/i2pcontrol-proposal-170/007-status.md
```

The reviewer must:

- inspect production code rather than trust plan checkboxes;
- map every canonical requirement to evidence;
- run or independently verify required commands;
- examine unrun/environmental limitations;
- verify prior closure assumptions remain valid;
- classify every finding by severity;
- recommend closed, conditionally closed, corrective pass required, or blocked;
- update the roadmap/registry only according to evidence.

Strict closure requires no unresolved high or medium finding. An external interoperability environment alone may justify conditional closure only if all in-repo contract behavior is complete and the exact missing evidence is operational, not an implementation gap.

## 7. Ordered work packages

### Work package A — Activation audit and manifest reconciliation

Intent: determine whether the subsystem is actually ready for final hardening.

Required changes:

1. Update baseline to the integrated closed head.
2. Read all M001–M006 plans and closure records.
3. Generate current registered methods/selectors/types/actions/backends.
4. Reconcile against canonical matrix and specification.
5. Enumerate residual findings and ownership.
6. Stop and create corrective plans for any capability/architecture gap.

Acceptance evidence:

- signed-off activation report;
- no missing capability hidden in M007;
- manifest exact-set check passes.

### Work package B — Golden fixtures and production-path harness

Intent: make external behavior reproducible.

Required changes:

1. Build deterministic real-listener harness.
2. Create sanitized golden corpus.
3. Link fixtures to manifest rows.
4. Add structural exactness and no-extra-field assertions.
5. Remove/avoid test-only bypass paths.

Acceptance evidence:

- every row has executable fixture/evidence;
- fixtures run offline;
- no secrets/non-deterministic unmasked values.

### Work package C — Adversarial protocol/security/resource testing

Intent: close the exposed administrative service boundary.

Required changes:

1. Test TLS/auth/version/JSON-RPC negatives.
2. Test request/result bounds and saturation.
3. Add canary secret/redaction tests.
4. Add static protocol/dependency/ownership guards.
5. Measure task/memory/shutdown behavior.

Acceptance evidence:

- no unbounded path;
- plaintext/auth bypass impossible;
- no secret leakage;
- static guards fail on intentional mutation samples where practical.

### Work package D — Persistence, restart, and concurrency matrix

Intent: prove durable correctness under failure and contention.

Required changes:

1. Run all store failpoints through production handlers.
2. Add cross-store and cross-method consistency tests.
3. Add deterministic races/cancellation/shutdown.
4. Run supported-platform persistence matrix.
5. Record rollback/recovery diagnostics.

Acceptance evidence:

- no silent reset/torn state/stale lifecycle result;
- cross-method state remains canonical;
- platform evidence complete or explicitly blocked.

### Work package E — Compatibility, documentation, and operational closure

Intent: make the final support claim accurate and usable.

Required changes:

1. Run reference/client compatibility fixtures.
2. Complete support/security/recovery docs.
3. Verify i2pcontrol-disabled and frontend-independent builds.
4. Publish exact per-tunnel API/runtime table.
5. Prepare closure evidence bundle.

Acceptance evidence:

- docs match production tests;
- no runtime-complete claim for stubs;
- no frontend claim;
- completion statement is supportable.

### Work package F — Independent closure review

Intent: separate implementation from completion judgment.

Required changes:

1. Freeze implementation head.
2. Assign independent reviewer.
3. Create requirement-to-evidence matrix.
4. Re-run selected critical tests and inspect guards.
5. Classify residual findings.
6. Create closure record and update registry/roadmap.

Acceptance evidence:

- reviewer-authored closure record;
- no unresolved high/medium finding for strict closure;
- exact roadmap disposition.

## 8. Failure, cancellation, restart, and contention semantics

M007 must verify all prior milestone semantics together:

- malformed/unauthorized work fails before sensitive/expensive operations;
- durable mutation may commit immediately before response cancellation, and replay converges deterministically;
- failed persistence never exposes unpersisted success;
- corrupt newest state falls back only to a valid prior generation with diagnostics;
- no valid generation causes actionable failure, not silent reset;
- concurrent mutations are serialized/revision-safe;
- lifecycle generations prevent stale start/stop/restart completion;
- unsupported operations remain immediate and resource-free;
- `All` target snapshots/concurrency/cancellation are bounded;
- core inspection channels apply backpressure and never mutate router state;
- log read/clear/write is coherent;
- service-registry generations reject stale producer updates;
- shutdown cancels/finishes bounded work and releases listener ports;
- immediate restart loads committed administrative state and reconstructs volatile state truthfully;
- test failures/blocks are recorded rather than retried until hidden.

## 9. Compatibility and migration

M007 must verify:

- old configurations remain accepted;
- I2PControl remains disabled/defaulted exactly as M001 specifies;
- i2pcontrol-disabled builds preserve prior behavior/dependencies;
- `router.toml` remains startup configuration and is not mutated by Proposal 170 handlers;
- runtime address-book files and precedence remain unchanged;
- startup tunnel/proxy tasks remain owned by existing managers;
- Proposal 170 schema migrations remain versioned and validated;
- older versions ignore separate administrative state rather than corrupting it;
- future real backend replacement requires only backend-specific implementation/registration;
- no public API redesign is required by stored stub definitions;
- base I2PControl clients and reference fixtures remain compatible.

Any migration incompatibility with data loss, false runtime activation, or secret exposure is release-blocking.

## 10. Required tests

M007 owns the aggregate execution of all prior required tests plus the following final suites.

### Full conformance suite

- exact-set manifest checks;
- every method/selector/type/action/field fixture;
- only-requested fields;
- no extension fields/statuses;
- exact JSON types/nullability;
- exact protocol vs operation error classification;
- complete per-tunnel API/runtime matrix.

### Cross-method consistency suite

- AddressBook and RouterInfo;
- TunnelManager, RouterInfo, and ClientServicesInfo;
- logs and clear;
- metrics/core state and RouterInfo;
- listener/session state and ClientServicesInfo.

### Adversarial suite

- TLS/plaintext;
- auth/token/version;
- malformed/duplicate/deep/oversize JSON;
- invalid IDs/params/types/actions/selectors;
- unauthorized large requests;
- slow clients/timeouts;
- connection/request/token saturation;
- error/log redaction canaries.

### Persistence/recovery suite

- all failpoints;
- corruption/fallback/all-corrupt;
- concurrent writers;
- abrupt restart;
- cross-platform publication;
- path/symlink/permission/disk errors;
- retention and scan bounds;
- unrelated-file non-mutation.

### Lifecycle/contention suite

- AddressBook races;
- TunnelManager CRUD/lifecycle/rename/delete/All races;
- RouterInfo polling under mutation;
- logs clear/write/read;
- SAM/service updates;
- shutdown/restart under saturation;
- channel/full/busy behavior;
- fake backend panic/failure.

### Resource/performance suite

- near-limit valid requests/results;
- over-limit rejection before unbounded allocation;
- memory/task stability under repeated polling and mutations;
- bounded `All` fan-out;
- bounded peer/log/session/tunnel serialization;
- router progress under moderate polling;
- shutdown latency under saturated clients.

### Static architecture/security suite

- exact public vocabulary;
- no frontend/EventSubscriber coupling;
- no direct handler filesystem/runtime authority;
- no unsupported resource constructor path;
- no core server dependencies;
- no administrative store in runtime resolver;
- no startup manager mutation authority;
- no private-key/sensitive type in response snapshots;
- no silent truncation/pagination/capability extensions.

### Platform/feature suite

- supported Linux/macOS/Windows CI;
- default/all/headless/UI+i2pcontrol/i2pcontrol-disabled builds;
- core supported feature combinations;
- offline deterministic test run.

## 11. Required verification commands

The activation pass must derive exact commands from the implemented feature/test layout. Minimum expected commands include:

```bash
cargo fmt --all -- --check

cargo check --workspace
cargo check --workspace --all-features
cargo check -p emissary-cli --no-default-features
cargo check -p emissary-cli --no-default-features --features i2pcontrol
cargo check -p emissary-cli --features ui,i2pcontrol
cargo check -p emissary-core --no-default-features
cargo check -p emissary-core --all-features

cargo test -p emissary-cli --no-default-features --features i2pcontrol i2pcontrol
cargo test -p emissary-core
cargo test --workspace
cargo test --workspace --all-features

cargo clippy --workspace --all-targets --all-features -- -D warnings

# Project-provided commands added by implementation:
# - conformance manifest exact-set validator
# - golden fixture runner
# - static architecture/security guards
# - persistence failpoint/restart suite
# - contention/resource harness
```

CI must execute the relevant matrix on Linux, macOS, and Windows. The closure record must list exact commands, commits, platforms, pass/fail/skipped results, and artifacts.

## 12. Documentation updates

Complete or verify:

- `docs/i2pcontrol/README.md`;
- `docs/i2pcontrol/configuration.md`;
- `docs/i2pcontrol/authentication.md`;
- `docs/i2pcontrol/address-book.md`;
- `docs/i2pcontrol/tunnel-manager.md`;
- `docs/i2pcontrol/tunnel-backends.md`;
- `docs/i2pcontrol/router-info.md`;
- `docs/i2pcontrol/client-services.md`;
- `docs/i2pcontrol/administrative-state.md`;
- `docs/i2pcontrol/inspection-architecture.md`;
- `docs/i2pcontrol/security.md`;
- `docs/i2pcontrol/proposal-170-support.md`;
- project README/config examples as appropriate.

Exact filenames may differ if prior milestones establish equivalents, but content coverage must be complete and non-duplicative.

## 13. Acceptance criteria

1. M003, M004, M005, and M006 are strictly closed before M007 activation.
2. Activation audit finds no missing capability hidden inside M007.
3. One machine-checkable manifest enumerates every Proposal 170 contract row.
4. Production registries contain no missing or extra method, selector, action, type, or public status.
5. Every manifest row links to production source and executable evidence.
6. Every method/selector/action/type has a real HTTPS golden fixture.
7. Fixtures assert exact keys, types, nullability, envelopes, and absence of extensions.
8. Authentication, API version, request IDs, and JSON-RPC errors remain exact.
9. Plaintext, invalid token, expired token, and auth bypass attempts fail safely.
10. Request bodies, nesting, strings, collections, concurrency, and deadlines are bounded before expensive work.
11. AddressBook operations and six RouterInfo administrative selectors are exact and cross-consistent.
12. Administrative books remain separate from runtime resolution and downloader behavior.
13. TunnelManager supports complete CRUD for all twelve types.
14. Every type has exactly one real or unsupported backend.
15. Unsupported start/restart are deterministic and resource-free.
16. Unsupported definitions never report active in any method.
17. `All` behavior is exact, bounded, and cancellation-safe.
18. Startup-managed ownership remains truthful and unmodified.
19. Every RouterInfo selector returns truthful exact data or permitted unavailable/error behavior.
20. Only requested RouterInfo keys appear.
21. Logs are bounded/redacted/independently clearable.
22. Cumulative/rolling metrics and rates are exact.
23. Core inspection is bounded, read-only, backpressured, and secret-free.
24. Complete peer/tunnel/log/session results are not silently truncated.
25. ClientServicesInfo returns only requested exact sections.
26. HTTP/SOCKS/I2CP/SAM state is based on actual passive observation.
27. BOB exact unavailable value is returned and no BOB implementation exists.
28. Persistence is versioned, deterministic, path-confined, and restart-safe on supported platforms.
29. Interrupted/corrupt state never causes silent reset or torn activation.
30. Concurrent mutation/lifecycle/query interleavings preserve invariants.
31. Shutdown and immediate restart release resources and recover correctly.
32. Canary secrets do not appear in responses, logs, errors, Debug output, fixtures, or artifacts.
33. Static guards prevent frontend/EventSubscriber coupling and direct handler authority.
34. Core contains no HTTP/JSON-RPC server or administrative persistence dependency.
35. I2PControl-disabled builds preserve prior behavior and dependency shape.
36. Existing runtime address-book, startup tunnel/proxy, router, NetDB, transport, SAM, and I2CP behavior remains compatible.
37. Moderate polling/mutation load remains bounded and does not materially impair router progress.
38. Documentation accurately distinguishes contract completeness from runtime completeness.
39. No missing tunnel data plane, frontend work, runtime resolver adoption, protocol extension, or router behavioral change entered scope.
40. An independent closure record finds no unresolved high or medium defect and supports the canonical completion statement.

## 14. Stop conditions

The implementation/hardening agent must stop and create or request a corrective implementation plan when:

- any capability method/selector/action/type is missing or incorrect;
- any prior closure assumption is invalidated;
- a high/medium protocol, security, durability, ownership, truthfulness, or concurrency defect is found;
- a fix requires architecture or feature implementation rather than narrow hardening;
- a stub allocates runtime resources or reports active;
- a complete response needs a protocol extension to fit bounds;
- core inspection requires mutable authority;
- startup-managed tasks would need migration;
- runtime resolver integration is proposed;
- frontend work is proposed;
- platform durability cannot be established safely;
- test evidence would require external network access with no deterministic substitute;
- the matrix would need weakening to mark production complete.

The stop report must identify owning milestone, severity, violated invariant/acceptance criteria, reproduction/evidence, and the smallest corrective plan required.

## 15. Closure evidence required

The independent closure record must include:

- all dependency closure references;
- implementation/hardening commits and frozen reviewed head;
- complete canonical requirement-to-evidence matrix;
- machine-readable manifest output;
- exact fixture coverage report;
- exact commands/platforms/results;
- authentication/TLS/JSON-RPC evidence;
- AddressBook and cross-method consistency evidence;
- per-type TunnelManager CRUD/backend/stub/All evidence;
- full RouterInfo selector evidence;
- ClientServicesInfo evidence;
- persistence/failpoint/corruption/platform evidence;
- concurrency/cancellation/shutdown/restart evidence;
- resource/performance measurements;
- secret/redaction scan evidence;
- static protocol/ownership/dependency guard evidence;
- compatibility/reference-client evidence;
- documentation/support-matrix review;
- source review proving explicit non-goals remained absent;
- all skipped/unrun/environmental limitations;
- unresolved findings by severity;
- exact final recommendation and registry/roadmap updates.

Strict closure is prohibited if:

- any manifest row lacks production evidence;
- any high/medium finding remains;
- only compilation/unit tests were run without end-to-end evidence;
- a stub is reported active or allocates runtime resources;
- state is fabricated or silently truncated;
- persistence can silently reset/lose committed data;
- authentication/TLS/resource bounds are incomplete;
- frontend/core ownership boundaries are violated;
- documentation overclaims runtime support;
- required platform evidence is absent without a narrowly justified operational condition.

## 16. Handoff notes

- M007 is a release gate, not a dumping ground for unfinished implementation.
- Preserve every historical closure record; do not rewrite prior findings.
- Use one authoritative manifest and generated exact-set checks.
- Prefer deterministic barriers/failpoints/paused time over sleeps.
- Run production paths; avoid handler-only tests as final evidence.
- Keep fixtures sanitized and offline.
- Record failed/skipped commands exactly.
- Measure resource behavior rather than asserting it from code inspection alone.
- An independent reviewer owns final closure.
- The accurate final statement is exactly the one in `plans/000-long-term-specification.md`; do not strengthen it into runtime completeness for missing tunnels.
