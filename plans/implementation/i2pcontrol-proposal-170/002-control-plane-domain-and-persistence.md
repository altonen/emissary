# I2PControl Proposal 170 Milestone 002 — Control-Plane Domain and Restart-Safe Persistence

Status: closed

Planning baseline: `6c92a71` (`master`)

Production-code baseline described by the planning system: `6c92a71`

Activation rule:

- M001 must have a closure record with status `closed`.
- Before implementation begins, the agent MUST replace the baseline above with the reviewed M001 closure head, inspect all M001 production changes, and record any material plan reconciliation.
- This plan is intentionally checked in before it is dependency-ready. Its existence does not authorize implementation while M001 is open.

Source roadmap:

- `plans/subsystems/i2pcontrol-proposal-170-roadmap.md#milestone-002--control-plane-domain-and-persistence`

Canonical requirements:

- `plans/000-long-term-specification.md#4-architectural-invariants`
- `plans/000-long-term-specification.md#6-tunnel-contract-and-stub-semantics`
- `plans/000-long-term-specification.md#7-existing-runtime-ownership`
- `plans/000-long-term-specification.md#8-address-book-boundary`
- `plans/000-long-term-specification.md#10-persistence-and-recovery`
- `plans/000-long-term-specification.md#11-security-and-resource-bounds`
- `plans/001-terminology-and-domain-model.md#2-control-plane-terms`
- `plans/001-terminology-and-domain-model.md#3-tunnel-terms`
- `plans/001-terminology-and-domain-model.md#4-address-book-terms`
- `plans/002-long-term-roadmap.md#milestone-m002--control-plane-domain-and-persistence`

Applicable ADRs:

- `plans/adrs/ADR-0001-proposal-170-contract-and-stub-boundary.md`

Primary class: invariant / infrastructure

## 1. Objective

Establish the canonical Proposal 170 administrative domain and persistence boundary consumed by AddressBook, TunnelManager, RouterInfo, and ClientServicesInfo without implementing any feature method or tunnel data plane.

At completion, Emissary must have:

- exhaustive, exact tunnel type and action domain types;
- a lossless canonical tunnel definition covering every Proposal 170 option frozen by M001;
- explicit tunnel ownership and internal runtime-state models;
- a backend trait and exhaustive registry with an explicit unsupported backend;
- four administrative address-book domain stores plus subscription and configuration models;
- versioned, deterministic, path-confined, restart-safe persistence;
- typed store/control interfaces and fakes for later handler tests;
- no automatic runtime adoption, listener creation, tunnel startup, address resolution, or frontend behavior.

This milestone establishes internal infrastructure only. It must not claim Proposal 170 feature capability until a later method handler consumes the infrastructure.

## 2. Why this milestone is blocked

Hard dependency:

- M001 must close with the exact conformance matrix, DTO conventions, method registry, authentication boundary, and typed control-plane seam.

The implementation must not begin against guessed request keys or response semantics. M001 may materially determine:

- exact Rust-to-wire field mappings;
- which Proposal 170 option keys are legal for each action and tunnel type;
- nullability and omission rules;
- operation-error representation;
- configuration map compatibility behavior;
- persistence-facing canonical forms;
- whether a compatibility ambiguity requires another ADR.

Once M001 closes, this milestone is otherwise self-contained. It does not depend on router inspection or real tunnel execution.

## 3. Current implementation evidence

At the production baseline:

### Existing application configuration

- `emissary-cli/src/config.rs` defines relatively narrow startup configurations for HTTP proxy, SOCKS proxy, generic client tunnels, generic server tunnels, and one runtime address-book configuration.
- `ClientTunnelConfig` contains only name, local address/port, target destination, and optional destination port.
- `ServerTunnelConfig` contains only name, local target port, destination path, and I2CP options.
- These structures cannot represent the complete Proposal 170 option matrix and MUST NOT become the canonical Proposal 170 persistence schema.

### Existing address-book storage

- `emissary-cli/src/address_book.rs` owns one runtime-oriented store under `<base>/addressbook`.
- It stores base32 mappings in one file and base64 destinations in hostname-derived files.
- Its synchronous handle mutators write directly to the runtime store and are not an atomic multi-book administrative persistence layer.
- It does not model `private`, `local`, `router`, and `published` as independent stores.

### Existing tunnel ownership

- `ClientTunnelManager` and `ServerTunnelManager` consume vectors at startup and spawn long-lived tasks.
- They do not expose a complete Proposal 170 lifecycle command interface.
- Existing proxy and tunnel tasks are therefore startup-managed, not control-plane-owned.

### Existing persistence primitives

- Emissary already has a configured base path and Tokio filesystem use.
- No Proposal 170 state root, schema version, generation/revision model, atomic replacement helper, corruption fallback, or administrative-state lock exists.

### M001 expected seam

M001 is expected to establish a typed control-plane interface and test double. M002 must implement domain/store adapters behind that seam rather than bypassing it from handlers.

## 4. Invariants that must not regress

- Exact external tunnel type strings remain:
  - `client`
  - `httpclient`
  - `ircclient`
  - `socks`
  - `socksirc`
  - `connectclient`
  - `streamrclient`
  - `server`
  - `httpserver`
  - `httpbidirserver`
  - `ircserver`
  - `streamrserver`
- Exact actions remain `List`, `create`, `edit`, `get`, `start`, `stop`, `restart`, and `delete`.
- Unknown external type or action values are rejected; aliases are not introduced.
- Every declared tunnel type resolves to exactly one registered backend.
- No persisted tunnel definition proves or implies that a runtime service exists.
- Unsupported backends allocate no listener, destination, LeaseSet, session, task, or traffic path.
- Existing startup-managed configuration remains authoritative for existing startup tasks.
- Proposal 170 administrative state remains separate from `router.toml`.
- Four administrative books remain separate from the runtime resolver.
- Persistence paths remain confined below the configured Emissary base path.
- State activation occurs only after complete validation.
- Concurrent mutations have deterministic serialization and revision behavior.
- Invalid or partially written state never silently replaces the newest valid generation.
- Core router crates do not acquire JSON-RPC, HTTP server, or administrative persistence ownership.
- No frontend dependency or state is introduced.

## 5. Scope

### In scope

- Exact domain enums and validated identifiers.
- Complete tunnel option model derived from the M001 conformance matrix.
- Canonical round-trip representation for TunnelManager `get`.
- Internal tunnel ownership and lifecycle state.
- Backend trait, unsupported backend, fake backend, and exhaustive registry.
- Administrative address-book types, entries, subscriptions, and string configuration.
- Versioned persistent envelopes.
- Generic generation-based store machinery.
- Atomic publication and bounded generation retention.
- Corruption detection, fallback, and actionable diagnostics.
- Deterministic concurrent mutation.
- Store/control traits and in-memory fakes.
- Focused domain and persistence documentation.
- Static guards proving exact type/backend coverage.

### Explicitly out of scope

- Registering Proposal 170 handlers as successful.
- AddressBook request parsing or responses.
- TunnelManager request parsing or responses.
- RouterInfo or ClientServicesInfo selectors.
- Starting any tunnel from persisted `StartOnLoad`.
- Adapting current client/server/proxy managers into runtime backends.
- Importing or migrating `router.toml` tunnels into Proposal 170 state.
- Making administrative address books affect destination resolution.
- Fetching subscriptions.
- Applying address-book path/config values to existing runtime components.
- Adding frontend controls or display.
- Adding a database or network service.
- Changing router, NetDB, transport, tunnel, SAM, or I2CP behavior.

## 6. Required production changes

### Core/domain

Create a dedicated administrative domain beneath the M001 I2PControl module. Exact file placement may follow M001, but ownership should remain equivalent to:

```text
emissary-cli/src/i2pcontrol/
    domain/
        mod.rs
        tunnel.rs
        address_book.rs
        revision.rs
    backends/
        mod.rs
        unsupported.rs
        fake.rs
    stores/
        mod.rs
        generation_store.rs
        tunnel_store.rs
        address_book_store.rs
```

Required types include:

- `TunnelType`: exhaustive exact external values with explicit parsing and serialization.
- `TunnelAction`: exhaustive exact action values.
- `TunnelName`: validated non-empty identifier preserving exact user spelling.
- `TunnelDefinition`: name, type, description/start intent, typed known options, and canonical raw configuration required by the M001 contract.
- `TunnelOptions`: complete known Proposal 170 option model, grouped internally only where grouping cannot alter wire names.
- `TunnelOwnership`: at least `ControlPlane`, `StartupManaged`, and `Unsupported`.
- `TunnelRuntimeState`: internal-only state such as `Stopped`, `Starting`, `Running`, `Stopping`, `Failed`, `Unsupported`, and `ExternallyManaged`.
- `AdministrativeAddressBookType`: exact `private`, `local`, `router`, and `published`.
- `AddressBookEntry`: hostname plus validated/canonical destination representation.
- `SubscriptionSet`: ordered persistent strings.
- `AddressBookConfiguration`: string-keyed map with deterministic ordering.
- `StateRevision`: monotonically increasing per-store revision used for serialized mutation and test evidence.

Do not use externally tagged Serde defaults if they can silently alter exact spelling. Add explicit tests for every wire string.

### Tunnel option representation

The model must satisfy both strict parsing and lossless `get` behavior:

1. The M001 matrix is the sole list of accepted top-level Proposal 170 tunnel parameters.
2. Known fields receive typed parsing and range validation in the later TunnelManager layer.
3. M002 defines storage-capable types for all fields without applying action-specific validation prematurely.
4. Canonical raw configuration is deterministic and uses exact external field names.
5. Arbitrary unknown top-level API keys are not retained as an extension mechanism.
6. Protocol-defined extensibility containers such as `CustomOptions` are retained exactly according to their specified type.
7. Secrets such as proxy passwords remain serializable only because the API contract requires configuration management, but Debug/Display/logging implementations must redact them.

A future backend replacement must be able to consume an existing `TunnelDefinition` without a schema redesign.

### Backend contract

Define a backend interface that is independent from JSON-RPC and persistence policy. It should support, at minimum:

- declared tunnel type;
- start;
- stop;
- inspect/status;
- explicit capability or implementation classification used internally only.

The exact signatures may use async traits, boxed futures, or command actors depending on M001 dependencies. They must carry:

- immutable tunnel definition input;
- cancellation/deadline context where execution is possible;
- typed backend errors;
- a runtime handle only when a real task exists.

`UnsupportedTunnelBackend` must:

- be constructible for any declared type;
- return typed `NotImplemented { tunnel_type }` from start;
- return typed `NotImplemented { tunnel_type }` from restart composition later;
- treat stop of an inactive definition as safe and resource-free;
- inspect as internal `Unsupported`;
- never spawn or bind anything.

`FakeTunnelBackend` must support deterministic success/failure/state scripting for M004 handler tests without network activity.

### Exhaustive backend registry

Implement a registry whose construction fails or whose compile/test guard fails if any `TunnelType` lacks a backend.

At M002 closure:

- every type may map to `UnsupportedTunnelBackend`;
- no real backend is required;
- duplicate registration is rejected;
- registry lookups are total for valid types;
- backend identity is not exposed as a new public Proposal 170 field.

Prefer an explicit exhaustive constructor over a mutable best-effort map assembled from unrelated call sites.

### Administrative state layout

Use a dedicated root beneath the configured base path:

```text
<base>/i2pcontrol/
    state/
        tunnels/
        address-books/
        address-book-config/
        subscriptions/
```

The exact directory names may change, but they must be fixed, documented, and path-confined.

Do not write Proposal 170 administrative data into:

- `router.toml`;
- the current runtime `addressbook/` directory;
- server private-key paths;
- frontend configuration files.

### Versioned generation-store design

Implement a reusable `GenerationStore<T>` or equivalent with these semantics:

- each committed state is a complete versioned envelope;
- files use unique, monotonically ordered generation names;
- publication writes a new file rather than overwriting the active file;
- the temporary file is created in the target directory;
- content is serialized deterministically;
- content is flushed and synced before publication where the platform supports it;
- publication uses a same-filesystem rename to a previously unused final generation path;
- loaders enumerate bounded candidate generations newest-first;
- only a fully parsed, schema-supported, validated generation becomes active;
- the newest corrupt/incomplete generation may fall back to the previous valid generation with a high-visibility diagnostic;
- if no valid generation exists but files are present, startup returns an actionable state error rather than silently resetting;
- retention keeps a bounded number of known-good prior generations;
- cleanup never deletes the active or only fallback-valid generation;
- filenames are generated internally and never derived from hostnames, tunnel names, URLs, or API-supplied paths.

Using unique generation filenames avoids non-portable overwrite-rename behavior and gives deterministic interrupted-write recovery on Unix and Windows.

### Persistent envelopes

Each store envelope must include:

- schema identifier;
- schema version;
- generation/revision;
- deterministic payload;
- optional integrity checksum if selected by implementation;
- no transient runtime handles or task state;
- no authentication tokens;
- no server destination private keys unless a later contract explicitly requires a confined key reference.

The schema must distinguish internal unsupported and startup-managed metadata without introducing external protocol fields.

### Concurrency and mutation

Each store must have one canonical async mutation boundary:

- load current immutable state;
- validate proposed state;
- increment revision;
- persist a complete new generation;
- publish the new in-memory snapshot only after durable publication succeeds;
- return the new revision.

Concurrent callers must be serialized per store or use optimistic revision checks. They must not interleave file writes or expose unpersisted state as committed.

Filesystem work that can block must use appropriate Tokio facilities or `spawn_blocking`. No unbounded directory scan may run on an async worker.

### Control-plane adapters and fakes

Implement store-backed control components behind M001 interfaces, but do not wire feature handlers to success yet.

Provide in-memory fakes with the same mutation/revision semantics for:

- tunnel definitions;
- address books;
- subscriptions;
- address-book configuration;
- backend lookup/state.

Fakes must not weaken validation compared with production stores.

### Security and secret handling

- Redact password-like and key-like fields in Debug and error output.
- Do not log serialized definitions wholesale.
- Enforce collection, string, and serialized-state size limits from M001 configuration.
- Reject symlinks or resolved paths that escape the configured state root.
- Open generation files with safe create-new semantics.
- Use restrictive file permissions where supported, especially because tunnel definitions may contain proxy credentials.
- Document platform behavior when permission bits are unavailable.
- Never interpolate user-provided identifiers into a filesystem path.

### Documentation and static guards

Add documentation covering:

- administrative state ownership;
- schema/generation layout;
- recovery algorithm;
- unsupported backend semantics;
- secret storage implications;
- why existing runtime config is not migrated.

Add guards/tests for:

- exact tunnel type set;
- exact action set;
- backend registry exhaustiveness;
- absence of `router.toml` writes from the Proposal 170 stores;
- absence of runtime address-book path reuse;
- no `tokio::spawn`, listener bind, SAM/I2CP session creation, or router mutation from unsupported backend code.

## 7. Ordered work packages

### Work package A — Reconcile M001 contracts and freeze M002 domain inputs

Intent: make M002 consume, not reinterpret, the closed protocol matrix.

Required changes:

1. Update this plan's baseline to the M001 closure head.
2. Enumerate every M001 tunnel field, type, action, nullability rule, and sensitive field.
3. Record the mapping from M001 DTO fields to M002 domain fields.
4. Resolve any remaining domain-level ambiguity through an ADR rather than ad hoc serialization.
5. Add a machine-readable exact type/action/options inventory if M001 did not already produce one.

Acceptance evidence:

- review table with no unmapped M001 tunnel or address-book field;
- no duplicate competing field-name source;
- tests consume the same inventory where practical.

### Work package B — Exact tunnel and address-book domain

Intent: provide stable, typed administrative models.

Required changes:

1. Implement exact enums and validated identifiers.
2. Implement the complete tunnel option model.
3. Implement ownership and internal runtime state.
4. Implement administrative address-book entries, subscription set, and configuration map.
5. Implement deterministic canonical serialization.
6. Implement redacted Debug/error formatting.

Acceptance evidence:

- round-trip tests for every type/action/option;
- range-independent storage tests;
- secret redaction tests;
- deterministic serialization fixtures.

### Work package C — Backend interface and exhaustive registry

Intent: wire future runtime adoption without implementing it.

Required changes:

1. Define typed backend request/result/error contracts.
2. Implement unsupported and fake backends.
3. Construct a total registry for all twelve types.
4. Reject duplicate/missing registrations.
5. Add no-op/resource-free stop and inspect behavior for unsupported definitions.

Acceptance evidence:

- table-driven registry test over every exact type;
- start/inspect/stop tests proving unsupported backends allocate no runtime resources;
- compile/source guard proving handlers are not part of the backend.

### Work package D — Generic versioned generation store

Intent: make persistence durable before feature stores depend on it.

Required changes:

1. Define versioned envelope and revision types.
2. Implement deterministic serialization and validation hooks.
3. Implement unique generation publication.
4. Implement newest-valid loading and bounded fallback.
5. Implement retention and cleanup.
6. Implement path confinement, safe file creation, and size limits.
7. Expose test failpoints around write, flush, sync, rename, activation, and cleanup.

Acceptance evidence:

- restart tests at every failpoint;
- newest-corrupt fallback test;
- all-corrupt startup-failure test;
- concurrent writer test;
- Windows-compatible unique-final-name behavior test.

### Work package E — Concrete administrative stores

Intent: instantiate the generic machinery without implementing methods.

Required changes:

1. Add tunnel-definition store.
2. Add four-book aggregate or separate address-book store with exact type separation.
3. Add subscription store.
4. Add address-book configuration store.
5. Add in-memory adapters/fakes.
6. Ensure stores start empty only when no prior state exists.

Acceptance evidence:

- complete store round trips;
- independent address-book isolation;
- unsupported tunnel persistence;
- no startup side effects;
- no runtime resolver changes.

### Work package F — Control-plane integration seam and documentation

Intent: make later milestones consume one authoritative infrastructure path.

Required changes:

1. Attach store adapters and backend registry to the M001 control-plane composition root.
2. Keep feature methods explicitly unavailable until their milestone handlers land.
3. Add operator and architecture documentation.
4. Add static ownership and no-side-effect guards.
5. Update the conformance matrix data-source column for M002-owned state.

Acceptance evidence:

- fake and production control-plane construction tests;
- listener can start with empty M002 state but methods remain correctly unavailable;
- feature builds remain frontend-independent;
- documentation names deferred behavior precisely.

## 8. Failure, cancellation, restart, and contention semantics

- Failure before a generation is published leaves the prior active snapshot unchanged.
- Failure after final generation publication but before in-memory activation is recovered by loading the new valid generation on restart; the current process returns a persistence error and may keep the old in-memory snapshot.
- Cleanup failure is non-fatal and must not invalidate a committed generation.
- A corrupt newest generation falls back only to a validated prior generation and emits a diagnostic that identifies the store and generation without exposing payload secrets.
- Presence of state files with no valid generation is a startup/configuration error, not an empty-state condition.
- Concurrent mutations are serialized per store. A failed mutation does not consume a visible revision.
- Cancellation before publication leaves no final generation; temporary files are cleaned eventually.
- Cancellation after publication is treated as committed even if the caller does not receive the response; later method plans must account for this ambiguity through idempotent name-based behavior.
- Unsupported backend calls do not need background cancellation because they perform no runtime work, but must honor caller deadlines without blocking.
- Store load and mutation have bounded file count, bytes, and validation work.
- No runtime task is reconstructed from administrative state after restart in M002.

## 9. Compatibility and migration

- Existing configurations without `<base>/i2pcontrol/state` continue to start unchanged.
- First use creates the state root lazily or during enabled I2PControl initialization.
- `router.toml` remains source-compatible and behavior-compatible.
- Existing runtime address-book files are neither moved nor imported.
- Existing client/server/proxy startup definitions are neither moved nor imported.
- M002 schema version 1 must be additive and self-describing.
- Future schema migrations must read older versions into a validated domain object before writing a new generation; in-place mutation is prohibited.
- Downgrade behavior must be documented. At minimum, older binaries must ignore the separate state root rather than corrupt it.
- Any newly added dependency must remain in `emissary-cli` unless it is a generic no-std-compatible primitive independently justified for core.

## 10. Required tests

### Focused unit tests

- exact `TunnelType` parse/serialize for all twelve values;
- exact `TunnelAction` parse/serialize for all eight values;
- rejection of case variants, aliases, empty values, and unknown values;
- tunnel option canonical serialization;
- deterministic map ordering;
- ownership/internal-state serialization boundaries;
- secret redaction;
- backend error classification;
- address-book type isolation;
- revision monotonicity.

### Persistence tests

- empty-store initialization;
- complete round trip for each store;
- deterministic bytes for equal logical state;
- unique generation publication;
- write/flush/sync/rename failure injection;
- newest-corrupt fallback;
- unsupported-version rejection;
- all-generations-corrupt startup failure;
- oversized state rejection;
- excessive-generation scan bound;
- retention safety;
- symlink/path-escape rejection;
- restrictive-permission behavior where testable.

### Restart and recovery tests

- restart after each publication failpoint;
- restart with unsupported tunnel definitions;
- restart with all four populated books;
- restart after cleanup failure;
- restart with stale temporary files;
- restart with a valid older and partial newer generation.

### Contention and cancellation tests

- concurrent creates against one store;
- concurrent rename/collision attempts at the domain-store layer;
- cancellation before and after durable publication;
- concurrent independent stores do not corrupt each other;
- no async runtime starvation under bounded large-state writes.

### Security and negative tests

- user identifiers never affect paths;
- password/key values absent from logs and Debug output;
- malformed JSON state rejected;
- deeply nested/oversized payload rejected before unbounded allocation;
- unknown schema and invalid checksum rejected;
- unsupported backend cannot bind a socket or spawn a task in test instrumentation.

### Compatibility tests

- old `router.toml` starts without state directory;
- current runtime address-book files remain byte-identical;
- startup-managed tunnels still start exactly through existing paths;
- headless and UI feature combinations compile;
- `emissary-core` dependency graph contains no administrative persistence or JSON-RPC dependency.

## 11. Required verification commands

The activation pass must update commands if M001 establishes different features or test targets. Expected minimum:

```bash
cargo fmt --all -- --check

cargo check -p emissary-cli --no-default-features
cargo check -p emissary-cli --no-default-features --features i2pcontrol
cargo check -p emissary-cli --features ui,i2pcontrol

cargo test -p emissary-cli --no-default-features --features i2pcontrol i2pcontrol::domain
cargo test -p emissary-cli --no-default-features --features i2pcontrol i2pcontrol::stores
cargo test -p emissary-cli --no-default-features --features i2pcontrol i2pcontrol::backends

cargo test -p emissary-core
cargo test --workspace --all-features

cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Run platform-specific persistence tests on every supported CI OS. Do not claim Windows/macOS atomicity from Linux-only evidence.

## 12. Documentation updates

- Add or update `docs/i2pcontrol/administrative-state.md`.
- Add or update `docs/i2pcontrol/tunnel-backends.md`.
- Add or update `docs/i2pcontrol/security.md` with at-rest secret implications.
- Document schema root, versions, generation recovery, retention, and corruption response.
- Document that `StartOnLoad` is stored but not executed in M002.
- Document that administrative books do not feed the runtime resolver.
- Update the M001 conformance matrix data-source and milestone-owner columns.
- Update the subsystem roadmap and registry when this plan activates and closes.

## 13. Acceptance criteria

1. M001 is closed and this plan is reconciled to its reviewed head before implementation.
2. Every exact Proposal 170 tunnel type has one parse/serialize representation.
3. Every exact TunnelManager action has one parse/serialize representation.
4. Every tunnel option in the M001 matrix has a storage-capable canonical representation.
5. Equal logical definitions serialize deterministically.
6. Sensitive option values are redacted from Debug, Display, errors, and logs.
7. Every tunnel type resolves to exactly one real or unsupported backend.
8. The baseline registry uses unsupported backends without starting any runtime service.
9. Unsupported backend start returns typed not-implemented without resource allocation.
10. Unsupported backend stop is safe and idempotent for inactive state.
11. Four administrative address-book types are represented independently.
12. Subscription ordering and address-book configuration maps round-trip deterministically.
13. Proposal 170 state is stored only under a dedicated confined state root.
14. No API-supplied name or path determines a filesystem location.
15. Persistence uses versioned complete generations and same-filesystem publication.
16. An interrupted write cannot replace the newest valid active state.
17. A corrupt newest generation falls back to a prior valid generation with a diagnostic.
18. State files with no valid generation cause an actionable error rather than silent reset.
19. Concurrent mutations cannot interleave or expose unpersisted state.
20. Existing `router.toml`, runtime address book, proxies, and startup tunnels retain prior behavior.
21. No persisted `StartOnLoad` value launches a task.
22. No AddressBook administrative entry affects runtime resolution.
23. Production and fake control-plane adapters expose equivalent domain validation.
24. Headless and UI-enabled builds compile without frontend ownership of the stores.
25. No administrative HTTP/JSON-RPC or persistence dependency is added to `emissary-core`.
26. All required tests and platform evidence are recorded in the closure record.

## 14. Stop conditions

The agent must stop and report rather than improvise when:

- M001 is not strictly closed;
- the M001 matrix lacks a tunnel field/type/action needed for canonical storage;
- exact external behavior would require retaining arbitrary unknown top-level protocol keys;
- safe cross-platform durable publication cannot be demonstrated;
- implementation would need to modify `router.toml` or migrate existing startup state;
- implementation would make an administrative book affect runtime resolution;
- a backend would need to create a real tunnel, proxy, listener, destination, session, or LeaseSet;
- a core mutable handle would need to cross into `emissary-cli`;
- a new public Proposal 170 field/status is proposed;
- secret persistence requirements cannot be bounded or documented safely;
- the work expands into M003, M004, M005, M006, or frontend behavior.

The stop report must identify the conflicting requirement, affected acceptance criteria, smallest decision needed, and whether an ADR or roadmap revision is required.

## 15. Closure evidence required

The later closure record must include:

- M001 closure reference and reconciled implementation baseline;
- implementation commit(s) and reviewed head;
- requirement-to-evidence mapping for all acceptance criteria;
- exact tunnel/action/option inventory coverage;
- backend registry exhaustiveness evidence;
- unsupported backend no-resource evidence;
- deterministic serialization fixtures;
- persistence failpoint and restart evidence;
- corrupt-generation fallback and all-corrupt failure evidence;
- concurrent mutation and cancellation evidence;
- path confinement, permission, and redaction evidence;
- old-configuration compatibility evidence;
- proof that current runtime address-book and startup tunnel behavior did not change;
- dependency/source review proving no core JSON-RPC or administrative persistence ownership;
- platform-specific atomic publication evidence;
- exact commands run and unrun limitations;
- unresolved findings by severity;
- roadmap and registry disposition.

Closure must be `corrective pass required` if any of these remains:

- missing type/action/option representation;
- non-exhaustive backend registration;
- unsupported backend runtime allocation;
- direct overwrite that can destroy the only valid state;
- silent reset after corruption;
- unconstrained user-derived filesystem path;
- secret leakage;
- automatic `StartOnLoad` execution;
- migration of existing runtime configuration;
- runtime resolver integration;
- frontend ownership;
- unresolved high/medium compatibility or durability finding.

## 16. Handoff notes

- This is a prewritten blocked plan. Reconcile it rather than executing it blindly after M001.
- Preserve unrelated upstream changes; Emissary is active.
- Keep M002 infrastructure narrow. Do not implement feature methods for visible progress.
- Prefer explicit domain types and exhaustive matches over stringly typed maps.
- Preserve exact external spellings in one authoritative inventory.
- Use deterministic `BTreeMap`-style ordering where order is not externally semantic.
- Use unique generation files to avoid platform-specific replace-overwrite assumptions.
- Keep failpoints test-only and deterministic.
- Do not log complete state payloads.
- Do not add real backends opportunistically.
- The implementation pass updates the registry to `closing`, not `closed`; independent closure remains required.
