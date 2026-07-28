# I2PControl Proposal 170 Milestone 004 — TunnelManager Contract and Explicit Stubs

Status: closed

Planning baseline: `ec289c77183d4f1010829ff255d8dbe90a941ad8` (`master`)

Production-code baseline described by the planning system: `9b43484a21d5a1291c4881cdae62a36c527f8c0f`

Activation rule:

- M002 must have a closure record with status `closed`.
- Before implementation begins, the agent MUST replace the baseline above with the reviewed M002 closure head, inspect all M001/M002 production changes, and reconcile this plan against the closed method contract, tunnel domain, persistence schema, backend registry, and control-plane interfaces.
- This plan is prewritten for dependency visibility and remains blocked while M002 is open.

Source roadmap:

- `plans/subsystems/i2pcontrol-proposal-170-roadmap.md#milestone-004--tunnelmanager-and-explicit-stubs`

Canonical requirements:

- `plans/000-long-term-specification.md#4-architectural-invariants`
- `plans/000-long-term-specification.md#5-protocol-exactness`
- `plans/000-long-term-specification.md#6-tunnel-contract-and-stub-semantics`
- `plans/000-long-term-specification.md#7-existing-runtime-ownership`
- `plans/000-long-term-specification.md#9-truthful-state-and-observability`
- `plans/000-long-term-specification.md#10-persistence-and-recovery`
- `plans/000-long-term-specification.md#11-security-and-resource-bounds`
- `plans/001-terminology-and-domain-model.md#3-tunnel-terms`
- `plans/002-long-term-roadmap.md#milestone-m004--tunnelmanager-and-explicit-stubs`

Applicable ADRs:

- `plans/adrs/ADR-0001-proposal-170-contract-and-stub-boundary.md`

Primary class: capability / infrastructure

## 1. Objective

Implement the complete Proposal 170 `TunnelManager` API contract for every declared tunnel type while deliberately leaving missing tunnel data planes unimplemented behind explicit, deterministic unsupported backends.

At completion, an authenticated caller must be able to:

- parse every exact Proposal 170 tunnel type and option;
- create a durable administrative definition for every declared type;
- edit, rename, inspect, and delete every control-plane-owned definition;
- request start, stop, and restart through one typed backend-dispatch path;
- receive a deterministic Proposal 170-compatible `error - ... not implemented` status when a missing tunnel type is started or restarted;
- stop an inactive unsupported definition safely and idempotently;
- apply `All` only to the actions permitted by Proposal 170;
- observe existing startup-managed Emissary tunnels truthfully without granting the new control plane authority it does not possess;
- restart Emissary and retain all administrative definitions without automatically starting missing services.

This milestone completes the API contract. It does not implement IRC, Streamr, CONNECT, HTTP-server, bidirectional HTTP-server, SOCKS-IRC, or any other missing tunnel data plane, and it does not redesign current startup managers.

## 2. Why this milestone is blocked

Hard dependency:

- M002 must close with exact tunnel domain types, complete option storage, ownership and internal-state models, versioned persistence, typed backend contracts, an exhaustive registry, and fake/unsupported backends.

M004 must not invent a second tunnel model in handlers. It consumes:

- M001's exact request parameters, actions, validation matrix, response/status strings, and `All` behavior;
- M002's canonical `TunnelDefinition`, store, revision semantics, backend trait, registry, ownership model, and unsupported backend;
- M001's authentication, JSON-RPC, request limits, and cancellation context.

M004 is otherwise independent from RouterInfo core inspection. Its inventory/status adapter becomes an input to M005 and M006.

## 3. Current implementation evidence

At the production baseline:

### Existing client tunnels

- `emissary-cli/src/tunnel/client.rs` consumes a startup vector of `ClientTunnelConfig`.
- It creates one SAM stream session and spawns per-tunnel event loops.
- Each event loop binds a local TCP listener, accepts one connection, connects to a destination, copies traffic, and is restarted by a `JoinSet` loop.
- The manager has no command channel for create/edit/get/start/stop/restart/delete.
- Runtime task identity, cancellation, per-definition status, and safe external mutation are not exposed.

### Existing server tunnels

- `emissary-cli/src/tunnel/server.rs` loads or creates persistent destinations during manager construction.
- It starts SAM sessions and forwarding loops for startup configurations.
- It has no Proposal 170 lifecycle command channel or external ownership transfer.
- Destination private-key paths and runtime tasks remain startup-manager concerns.

### Existing proxies

- HTTP and SOCKS proxies are configured and spawned directly from `main.rs`.
- Their tasks are not owned by a Proposal 170 lifecycle registry.
- They must not be relabeled as dynamically controllable `httpclient` or `socks` backends without a separate runtime design.

### Existing configuration

- `ClientTunnelConfig` and `ServerTunnelConfig` represent only a small subset of Proposal 170 options.
- `router.toml` is startup configuration, not the canonical Proposal 170 administrative state.

### Missing control API

- No `TunnelManager` JSON-RPC handler exists.
- No exact action/type parser, backend dispatch, administrative CRUD, ownership error, or permitted-`All` implementation exists.

## 4. Invariants that must not regress

- Exact tunnel types remain the twelve values defined in the canonical terminology document.
- Exact actions remain `list`, `create`, `edit`, `get`, `start`, `stop`, `restart`, and `delete`.
- Required `Name` and `Action` semantics follow the M001 matrix exactly.
- `All` is accepted only for `start`, `stop`, and `restart`.
- JSON-RPC/protocol errors remain distinct from valid TunnelManager textual operation statuses.
- Every declared type is accepted by parsing, validation, persistence, and backend lookup.
- Every declared type has exactly one registered real or unsupported backend.
- Missing runtime capability is explicit and never simulated.
- Unsupported start and restart allocate no listener, task, session, destination, LeaseSet, or traffic path.
- Unsupported definitions never report `running`.
- Configuration CRUD remains functional for unsupported types.
- `StartOnLoad` is stored but does not start missing runtime services in this milestone.
- Existing startup-managed tunnels and proxies retain their current task ownership.
- M004 never edits a control-plane copy while implying contrary startup runtime state changed.
- Existing startup configuration and `router.toml` are not mutated.
- Secret values are redacted from logs, Debug output, and operation failures.
- Success follows durable persistence for configuration mutations.
- Handler code does not contain type-specific data-plane logic.
- No frontend behavior is introduced.
- No core router, SAM, I2CP, NetDB, transport, or tunnel algorithm changes are introduced.

## 5. Scope

### In scope

- Exact TunnelManager request parsing.
- Exact type/action/option validation from the M001 matrix.
- Name lookup and canonical identity.
- Create, edit, rename, get, and delete for control-plane-owned definitions.
- Start, stop, and restart dispatch through M002 backend contracts.
- Exact `All` validation and aggregation.
- Unsupported backend status/error mapping.
- Persistent control-plane definitions.
- Read-only inventory of existing startup-managed generic client/server tunnels and configured proxies where the M001 contract requires them to appear.
- Explicit ownership errors for unsupported mutations or lifecycle actions against startup-managed objects.
- Deterministic concurrency, revision, generation, and cancellation behavior.
- Backend instrumentation and no-resource tests.
- Handler, listener, restart, security, and compatibility tests.
- Documentation distinguishing API completeness from runtime completeness.

### Explicitly out of scope

- Implementing any missing tunnel data plane.
- Adding dynamic lifecycle command channels to existing client/server managers.
- Taking ownership of current HTTP or SOCKS proxy tasks.
- Creating real adapters merely because a similarly named startup service exists.
- Migrating startup definitions into Proposal 170 persistence.
- Starting persisted definitions automatically.
- Generating destination private keys for Proposal 170 definitions.
- Binding listeners for control-plane definitions.
- Creating SAM or I2CP sessions for control-plane definitions.
- Changing streaming, LeaseSet, tunnel-pool, router, NetDB, or transport behavior.
- Adding public capability discovery or implementation-status fields.
- Adding aliases, extra tunnel types, extra actions, pagination, batch extensions, or richer statuses.
- Frontend controls or display.

## 6. Required production changes

### Protocol and DTO integration

Register the exact `TunnelManager` method through M001's method registry.

The request path must remain:

```text
HTTPS JSON-RPC request
    -> authentication/version gate
    -> bounded exact DTO parsing
    -> action-specific validation
    -> typed TunnelControl operation
    -> M002 store/backend/ownership adapter
    -> exact Proposal 170 result or JSON-RPC error
```

Handlers must not:

- access state files directly;
- inspect or spawn Tokio tasks directly;
- call SAM/I2CP constructors;
- bind sockets;
- edit `router.toml`;
- branch into data-plane code by tunnel type.

### Presence-aware request model

The parser must preserve:

- presence and exact value of `Name`;
- presence and exact value of `Action`;
- presence/value of `All`;
- presence/value of `NewName` where specified;
- every option field and whether it was omitted;
- the exact declared tunnel type.

Convert into action-specific internal requests:

```rust
pub enum TunnelManagerOperation {
    Create(CreateTunnelRequest),
    Edit(EditTunnelRequest),
    Get(GetTunnelRequest),
    Start(LifecycleRequest),
    Stop(LifecycleRequest),
    Restart(LifecycleRequest),
    Delete(DeleteTunnelRequest),
}
```

Do not carry one giant optional-field DTO deep into persistence and infer action semantics there.

### Action validation

Create a generated or table-driven validator from the M001 conformance inventory.

For each action, define:

- required fields;
- permitted optional fields;
- forbidden fields;
- valid tunnel types;
- field JSON types;
- range and format constraints;
- cross-field constraints;
- secret fields;
- whether `All` is permitted;
- whether the operation targets one definition or an aggregate.

Examples of cross-field validation that must be captured exactly when present in M001:

- type required for create and immutable except where edit explicitly permits it;
- ports bounded to protocol-valid ranges;
- quantities, lengths, variances, backups, and timeouts bounded;
- host/listen fields syntactically valid without binding;
- destination/key fields structurally validated only to the level required by configuration storage;
- access lists parsed and bounded without changing runtime policy;
- `CustomOptions` parsed as the exact protocol-defined type;
- proxy credentials accepted only for applicable types and redacted;
- server-only fields rejected for client-only types and vice versa;
- `NewName` valid only for edit when the proposal allows it.

No field may be accepted merely because M002 can store it.

### Definition identity and names

Names are the primary administrative identity unless M001 specifies otherwise.

Required behavior:

- bounded non-empty names;
- exact case sensitivity according to the compatibility matrix;
- no path derivation from names;
- deterministic duplicate-name handling;
- no collision between control-plane-owned and startup-managed inventory without an explicit namespace/precedence rule frozen before implementation;
- rename through one atomic store transaction;
- old name absent and new name present after commit;
- no intermediate duplicate or missing state visible;
- rename collision returns the exact error/status class.

If M001 does not specify collision behavior with startup-managed names, stop and resolve it before implementation.

### Create

Create must:

1. validate all exact fields for the selected type;
2. construct the M002 canonical definition without dropping options;
3. reject duplicate identity according to the frozen rule;
4. assign control-plane ownership;
5. assign internal inactive/unsupported state based on backend classification;
6. persist before reporting success;
7. not call backend start even when `StartOnLoad` is true;
8. not create destinations, files outside the M002 store, listeners, sessions, or tasks;
9. return the exact Proposal 170 status string/result form.

### Edit

Edit must:

- load one committed definition;
- reject startup-managed ownership unless the exact operation is truthfully supported without changing runtime state;
- apply only fields permitted for edit;
- preserve omitted fields;
- validate the complete resulting definition, not only changed values;
- preserve unknown protocol-defined extensible container values as required;
- perform rename and option changes atomically;
- reject edits that would imply a running service was modified when no safe runtime update path exists;
- persist before success;
- not trigger start/restart implicitly.

Because M004's control-plane definitions use unsupported backends by default, edit can normally proceed while inactive. Future real backends must define explicit running-edit semantics in a separate plan.

### Get

Get must:

- retrieve the exact named definition or exact `All` behavior only if M001 permits it;
- return all required stored configuration fields using exact external keys and JSON types;
- distinguish stored configuration from runtime state according to the proposal's result form;
- return a deterministic inactive state for unsupported definitions;
- return truthful observed state for startup-managed inventory only where it is actually observable;
- never include backend implementation metadata or internal ownership fields unless the protocol already defines a corresponding value;
- enforce whole-response limits without truncation.

The canonical raw option representation from M002 must make create -> get and edit -> get lossless for all protocol fields.

### Delete

Delete must:

- reject unsupported ownership changes to startup-managed definitions;
- reject or stop a running future real backend according to the exact frozen policy rather than improvising;
- for M004 unsupported definitions, remove only administrative state;
- persist the removal before success;
- never remove startup configuration, destination private-key files, runtime address-book data, or unrelated state;
- be deterministic for absent names according to M001.

### Lifecycle dispatch

All lifecycle actions must pass through a typed `TunnelControl` actor/service. The handler cannot call backend methods while holding store locks.

A safe sequence for a single definition is:

1. resolve committed definition and ownership;
2. validate action and expected generation;
3. acquire a per-definition operation permit or actor command slot;
4. re-check generation/ownership;
5. invoke the registered backend with cancellation/deadline context;
6. map backend result to internal state and exact operation status;
7. publish any internal runtime-state update required for observation;
8. release resources on every path.

At M004 baseline, control-plane-owned definitions may all resolve to unsupported backends. Do not create a real backend solely to demonstrate success.

### Unsupported backend mapping

Exact public behavior must follow M001, with the canonical expected shape:

- start: `error - tunnel type <type> is not implemented` or the exact frozen string;
- restart: same deterministic not-implemented class;
- stop of inactive unsupported definition: exact successful/no-op status if the proposal permits idempotent stop;
- inspection: map internal `Unsupported` to an existing inactive state such as `stopped`;
- no new `unsupported`, `stubbed`, `implemented`, or capability field on the wire.

Centralize this mapping. Do not let each handler/type produce different text.

### Startup-managed inventory

Build a read-only inventory adapter from retained startup configuration and observable service state.

Candidate inventory sources:

- existing `ClientTunnelConfig` list retained before manager ownership moves;
- existing `ServerTunnelConfig` list retained before manager construction;
- configured HTTP proxy;
- configured SOCKS proxy.

Requirements:

- inventory creation does not change startup behavior;
- names/types/options are mapped only where the Proposal 170 representation is truthful;
- unrepresentable fields use exact permitted omission/null behavior, not guesses;
- no private destination key is exposed;
- no task handle is transferred;
- mutations and lifecycle actions return a deterministic ownership error such as the exact frozen equivalent of `error - tunnel is managed by the startup configuration`;
- do not write an administrative shadow copy and report success;
- if active state is not observable, report permitted unavailable/inactive/error behavior rather than assuming running because configured.

Do not map HTTP/SOCKS startup services to tunnel types until the exact semantics and truthful field mapping are reviewed during activation. Configuration-name similarity alone is insufficient.

### `All` behavior

Implement `All` only for start, stop, and restart.

The exact M001 result shape and atomicity are authoritative. The implementation plan requires:

- deterministic target snapshot and ordering;
- no inclusion of definitions created after snapshot acquisition;
- per-target backend/ownership dispatch;
- bounded target count and total work;
- no unbounded parallel task fan-out;
- cancellation stops scheduling new targets and cleans up in-flight operations;
- complete aggregate result according to the protocol;
- no rollback claim if operations are inherently independent;
- explicit treatment of mixed unsupported, startup-managed, and future real backends;
- no use of `All` for create, edit, get, or delete.

If Proposal 170 returns only one status and cannot represent partial results, M001 must define the compatibility rule before M004 activation. Do not invent a per-tunnel extension response.

### Runtime state and fencing

Even with unsupported backends, implement the lifecycle state boundary correctly for future replacement:

- per-definition generation/revision;
- at most one lifecycle operation in flight per definition;
- stale completion cannot update a newer renamed/edited/deleted definition;
- delete/rename and lifecycle operations have a documented lock/actor order;
- no backend call while a global store lock is held;
- future real runtime handles remain outside serialized state;
- process restart reconstructs definitions as inactive unless a future real-backend recovery plan says otherwise.

Unsupported operations should complete immediately without persisting transient starting/stopping state unless such state is externally observable and required.

### Error and status mapping

Create one reviewed mapping for:

- malformed/invalid parameters -> JSON-RPC error;
- unknown method/action/type -> exact JSON-RPC error class;
- missing definition -> exact protocol error/status;
- duplicate name -> exact protocol error/status;
- ownership rejection -> textual operation status where appropriate;
- backend not implemented -> textual operation status;
- backend failure -> sanitized textual operation status;
- persistence failure -> internal/operation error according to M001;
- cancellation/deadline -> sanitized error/status;
- response too large -> bounded protocol error.

Do not expose Rust type names, task IDs, state paths, credentials, destination keys, or internal generations.

### Security and resource bounds

- Authenticate before expensive option parsing or inventory construction.
- Enforce request-body, string, collection, custom-option, access-list, and result-size limits.
- Redact all proxy passwords, keys, and sensitive options.
- Never log full definitions or request bodies.
- Bound `All` targets and concurrency.
- Unsupported backend instrumentation must prove zero network/session/task allocation.
- A malicious name cannot influence paths or log structure.
- Backend errors are sanitized.
- Handler cancellation releases permits and does not leave false active state.

### Documentation and static guards

Document:

- exact supported API actions and types;
- distinction between configuration CRUD and runtime execution;
- which backends are explicit stubs;
- exact not-implemented behavior;
- startup-managed ownership behavior;
- `StartOnLoad` storage-only behavior;
- persistence and restart behavior;
- no frontend control surface.

Add guards proving:

- all twelve types appear in the registry;
- every type uses a real or unsupported backend;
- no unsupported backend imports socket, SAM, I2CP, destination-generation, LeaseSet, or router-task constructors;
- handlers do not call `tokio::spawn`, bind listeners, or edit files directly;
- handlers do not write `router.toml`;
- no new wire status/type/action constants exist outside the authoritative contract inventory;
- no frontend module dependency exists.

## 7. Ordered work packages

### Work package A — Reconcile exact TunnelManager contract

Intent: turn M001's matrix into executable validators and fixtures.

Required changes:

1. Update this plan to the M002 closure baseline.
2. Enumerate exact actions, types, fields, applicability, defaults, and response/status rules.
3. Freeze name collision, missing-definition, `All`, edit/rename, delete, and startup-managed behavior.
4. Map each valid request to one M002 domain/control operation.
5. Generate or table-drive option validation where practical.

Acceptance evidence:

- every M001 TunnelManager row mapped to validator, handler, test, and data source;
- no unresolved aggregate-status ambiguity;
- no accepted field absent from M002 storage.

### Work package B — Typed requests and exact validation

Intent: reject invalid requests before state or backend work.

Required changes:

1. Implement presence-aware DTO conversion.
2. Implement action-specific request types.
3. Implement exact type/field applicability.
4. Implement ranges, syntax, cross-field, and secret classification.
5. Implement `All` restrictions.
6. Implement sanitized error mapping.

Acceptance evidence:

- valid fixture per type/action combination that the protocol permits;
- invalid fixture for forbidden/missing/wrong-type fields;
- no store/backend call on invalid input.

### Work package C — Durable configuration CRUD

Intent: make every declared type administratively functional.

Required changes:

1. Implement create for all types.
2. Implement edit and atomic rename.
3. Implement exact get round trip.
4. Implement delete.
5. Preserve options losslessly.
6. Keep StartOnLoad inert.
7. Enforce name/ownership conflicts.

Acceptance evidence:

- table-driven create/edit/get/delete tests for all twelve types;
- restart round trips;
- secret redaction;
- no runtime allocation during CRUD.

### Work package D — Lifecycle actor and unsupported dispatch

Intent: wire the real future execution seam without implementing execution.

Required changes:

1. Implement per-definition lifecycle serialization/fencing.
2. Dispatch through exhaustive backend registry.
3. Map unsupported start/restart to exact deterministic errors.
4. Implement idempotent inactive stop.
5. Maintain truthful inactive inspection.
6. Add cancellation/deadline handling.

Acceptance evidence:

- all twelve types reach a backend;
- stub lifecycle tests prove zero resource allocation;
- stale-generation and concurrent lifecycle tests;
- no false running state.

### Work package E — Startup-managed inventory and ownership rejection

Intent: expose only what is truthful without taking control.

Required changes:

1. Retain immutable startup configuration snapshots needed for inventory.
2. Define exact mapping to Proposal 170 types/fields or omit entries that cannot be truthfully represented.
3. Add observable status only where a passive source exists.
4. Reject mutations/lifecycle operations with exact ownership status.
5. Keep private key material and task handles inaccessible.

Acceptance evidence:

- startup services continue unchanged;
- inventory fixtures match retained configuration;
- mutation/lifecycle requests do not affect runtime or files;
- unobservable state is not fabricated.

### Work package F — `All`, integration, and scope guards

Intent: complete aggregate behavior and prove the scope boundary.

Required changes:

1. Implement exact permitted `All` behavior with bounded concurrency.
2. Add real-listener integration tests.
3. Add static guards for no data-plane/resource calls.
4. Update conformance/support documentation.
5. Update registry/roadmap when implementation and closure states change.

Acceptance evidence:

- mixed target aggregate tests;
- cancellation and large-target bounds;
- no extensions in responses;
- source guards and dependency review pass.

## 8. Failure, cancellation, restart, and contention semantics

- Invalid requests perform no state/backend operation.
- Create/edit/delete success is returned only after durable M002 publication.
- Persistence failure leaves prior committed state and runtime state unchanged.
- Cancellation after durable CRUD publication may leave the mutation committed even if the response is lost; replay must converge through deterministic name semantics.
- One lifecycle operation is active per definition.
- Concurrent start requests for an unsupported definition both resolve deterministically without spawning resources; implementation may coalesce or serialize but cannot report running.
- Stop racing start/restart uses generation fencing and cannot apply stale completion.
- Rename/delete racing lifecycle operations either occur before dispatch or invalidate stale completion; no operation may target a different definition that reused the name.
- `All` operates on a bounded target snapshot. It does not lock the registry for the duration of backend work.
- Cancellation of `All` stops new scheduling, awaits/cleans bounded in-flight operations, and follows exact aggregate error behavior.
- Unsupported backend operations are immediate, deterministic, and restart-neutral.
- Restart reconstructs control-plane definitions as inactive/unsupported and retains no transient runtime handle.
- Startup-managed services continue under existing manager recovery/retry behavior; M004 does not intercept it.
- No CRUD/lifecycle operation holds a persistence lock across network or runtime work.
- Failure messages are sanitized and bounded.

## 9. Compatibility and migration

- Existing `router.toml` remains valid and unmodified.
- Existing client/server tunnel and proxy startup behavior remains unchanged.
- Existing startup definitions are not copied into Proposal 170 persistence.
- Proposal 170 definitions remain separate and inactive unless a later real-backend plan replaces a stub.
- M004 uses the M002 schema; any required schema addition must be additive, reviewed, and migrated through M002 mechanisms.
- Existing M001 endpoint/authentication/error behavior remains unchanged.
- A future real backend replaces one registry entry and implements runtime semantics without changing public parsing, persistence, handlers, or existing stored definitions.
- Downgrade leaves Proposal 170 administrative state ignored by older versions rather than translated into startup config.
- No compatibility claim is made that Emissary's current generic tunnels are drop-in equivalents for every Java I2PTunnel type.

## 10. Required tests

### Focused unit tests

- exact type/action parsing;
- action-specific required/permitted/forbidden fields;
- every option type/range/applicability rule;
- name validation and collision behavior;
- NewName atomic validation;
- `All` restrictions;
- error/status mapping;
- secret redaction;
- unsupported state-to-wire mapping.

### Per-type CRUD tests

For each of the twelve tunnel types:

1. create a minimum valid definition;
2. get and compare exact canonical fields;
3. edit each applicable option family;
4. rename where permitted;
5. restart store/control plane and get again;
6. delete;
7. prove no runtime resource was allocated.

Add maximal representative definitions covering every option at least once.

### Lifecycle tests

For every type backed by unsupported implementation:

- start returns exact not-implemented status;
- restart returns exact not-implemented status;
- stop is safe/idempotent according to the contract;
- inspect/get never reports running;
- repeated and concurrent requests remain deterministic;
- cancellation/deadline releases permits;
- zero socket, task, SAM, I2CP, destination, LeaseSet, or file-side-effect count.

For fake backends:

- success/failure/starting/running/stopping transitions;
- stale completion fencing;
- cancel/start, stop/start, rename/start, delete/start races;
- backend panic/error containment.

### `All` tests

- valid start/stop/restart;
- rejection for create/edit/get/delete;
- empty registry;
- mixed unsupported/startup-managed/fake targets;
- deterministic target ordering;
- bounded concurrency;
- partial failure behavior exactly matching M001;
- cancellation mid-operation;
- definitions created/deleted during aggregate execution.

### Startup-managed compatibility tests

- current startup client/server managers still receive the same definitions;
- current HTTP/SOCKS startup behavior unchanged;
- inventory mapping is read-only;
- mutations/lifecycle requests return ownership errors;
- no `router.toml` or destination-key file changes;
- no control-plane shadow success;
- unobservable status is not guessed.

### Integration and restart tests

- full CRUD/lifecycle through real HTTPS listener;
- persistence across process/service reconstruction;
- corrupt store behavior inherited from M002;
- unauthorized requests never access store/backend;
- oversized request/result handling;
- shutdown with active fake lifecycle operations;
- immediate restart/rebind;
- headless and UI-enabled feature combinations.

### Security and static tests

- passwords/keys absent from logs/errors/Debug;
- no user name becomes path;
- no unsupported backend imports/calls resource constructors;
- no handler file writes/spawns/binds;
- no frontend imports;
- exact registry/type/action constants only;
- dependency review shows no server/admin stack in core.

## 11. Required verification commands

The activation pass must reconcile exact targets. Expected minimum:

```bash
cargo fmt --all -- --check

cargo check -p emissary-cli --no-default-features --features i2pcontrol
cargo check -p emissary-cli --features ui,i2pcontrol

cargo test -p emissary-cli --no-default-features --features i2pcontrol i2pcontrol::tunnel_manager
cargo test -p emissary-cli --no-default-features --features i2pcontrol i2pcontrol::backends
cargo test -p emissary-cli --no-default-features --features i2pcontrol tunnel

cargo test -p emissary-core
cargo test --workspace --all-features

cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Also run M001 fixture validation, M002 persistence/failpoint tests, and any static ownership/resource guard commands established by prior closures.

## 12. Documentation updates

- Add or update `docs/i2pcontrol/tunnel-manager.md`.
- Update `docs/i2pcontrol/tunnel-backends.md`.
- Update `docs/i2pcontrol/proposal-170-support.md` with per-type API/runtime status.
- Update the conformance matrix for every TunnelManager row.
- Document exact create/edit/get/delete and lifecycle behavior.
- Document `All` restrictions and aggregate semantics.
- Document unsupported start/restart status strings.
- Document safe inactive stop behavior.
- Document startup-managed ownership errors.
- Document stored-but-not-executed StartOnLoad.
- State that no missing tunnel type has been implemented.
- Do not document frontend controls.

## 13. Acceptance criteria

1. M002 is strictly closed and this plan is reconciled to its reviewed head.
2. TunnelManager is registered through the M001 method/auth/version boundary.
3. Name and Action requirements follow the exact M001 contract.
4. Exactly eight actions are accepted.
5. Exactly twelve tunnel types are accepted.
6. Aliases, case variants, and extension types/actions are rejected.
7. Every M001 tunnel field has exact JSON-type and applicability validation.
8. Invalid requests perform no store or backend work.
9. Every valid type supports durable create.
10. Every valid type supports lossless get.
11. Every valid type supports durable edit for permitted fields.
12. Rename is atomic, collision-safe, and leaves no torn identity.
13. Every valid type supports durable delete while control-plane-owned and inactive.
14. CRUD success is never returned before durable publication.
15. StartOnLoad is stored but launches no missing service.
16. Every type resolves to exactly one backend.
17. Unsupported start returns the exact deterministic not-implemented operation status.
18. Unsupported restart returns the exact deterministic not-implemented operation status.
19. Unsupported inactive stop is safe and exact.
20. Unsupported definitions never report active/running.
21. Unsupported operations allocate no listener, task, session, destination, LeaseSet, key file, or traffic path.
22. Lifecycle operations are serialized/fenced per definition.
23. Stale completion cannot update a renamed, edited, deleted, or recreated definition.
24. `All` is accepted only for start, stop, and restart.
25. `All` target selection, ordering, concurrency, and aggregate result follow M001 exactly.
26. `All` does not create unbounded tasks or hold store locks across backend work.
27. Startup-managed inventory is read-only and truthful.
28. Mutation/lifecycle requests against startup-managed objects return exact ownership errors.
29. No startup task, proxy, destination file, or `router.toml` entry is changed by those errors.
30. No private destination key or sensitive option is exposed.
31. Handler errors/statuses are sanitized and bounded.
32. Request and response size/work limits are enforced without truncation.
33. Restart preserves administrative definitions and reconstructs them inactive/unsupported.
34. Existing startup tunnel/proxy behavior and tests remain unchanged.
35. No missing data-plane implementation, dynamic manager redesign, frontend work, or router/core behavioral change is included.
36. A future real backend can replace a registry entry without public API, handler, or persistence redesign.
37. Required protocol, persistence, concurrency, cancellation, security, and compatibility tests pass.

## 14. Stop conditions

The agent must stop and report rather than improvise when:

- M002 is not strictly closed;
- M001 leaves any action/type/field/All/status behavior unresolved;
- M002 cannot store a field losslessly;
- name collisions between startup-managed and control-plane-owned definitions lack a frozen rule;
- exact `All` partial-failure behavior cannot be represented without a protocol extension;
- a lifecycle operation would require implementing or redesigning a tunnel/proxy manager;
- a proposed real backend would bind a listener, create a destination/session/LeaseSet, or carry traffic;
- implementation would edit `router.toml` or existing destination-key files;
- implementation would claim ownership over an existing startup task;
- a response requires a new public status/capability field;
- a complete result would require silent truncation;
- work expands into RouterInfo core inspection, ClientServicesInfo, frontend behavior, or missing tunnel data planes.

The stop report must name the exact contract row, ownership conflict, affected acceptance criteria, and smallest ADR/roadmap decision required.

## 15. Closure evidence required

The later closure record must include:

- M002 closure and reconciled baseline;
- implementation commits and reviewed head;
- requirement-to-evidence mapping for all acceptance criteria;
- exact type/action/field validator coverage;
- per-type CRUD and restart matrix;
- get round-trip/losslessness evidence;
- rename/collision evidence;
- exhaustive backend registry evidence;
- per-type unsupported start/restart/stop evidence;
- zero-resource instrumentation evidence;
- state/generation fencing and race evidence;
- exact `All` fixtures, mixed-result, cancellation, and bounds evidence;
- startup-managed inventory and ownership-rejection evidence;
- unchanged startup task/config/key-file evidence;
- secret redaction and authorization evidence;
- source/static guards for no resource calls, no file writes, no frontend, and no core server dependency;
- exact verification commands and outcomes;
- unrun/platform limitations;
- unresolved findings by severity;
- roadmap and registry disposition.

Closure must be `corrective pass required` if any of these remains:

- missing type/action/field support;
- lossy CRUD;
- non-durable success;
- unsupported backend resource allocation;
- unsupported tunnel reported active;
- missing backend registration;
- incorrect `All` acceptance or extension response;
- stale lifecycle race;
- startup-manager mutation or false ownership;
- `router.toml`/key-file mutation;
- secret leakage;
- real missing tunnel implementation added;
- frontend or router behavior changes;
- unresolved high/medium protocol, ownership, persistence, or security finding.

## 16. Handoff notes

- This is a prewritten blocked plan; reconcile it after M002 closure.
- Contract completeness is the goal. Runtime completeness is explicitly not the goal.
- The safest initial real-backend count is zero unless an existing runtime interface is already independently controllable and truthfully mappable.
- Similar service names do not prove backend equivalence.
- Keep all not-implemented text in one tested mapping.
- Treat `StartOnLoad` as configuration intent only in this milestone.
- Use fake backends for lifecycle mechanism tests; do not build a network demo backend.
- Preserve existing task ownership and retry behavior.
- Never include full definitions in tracing fields.
- Test zero resource allocation with instrumentation, not source inspection alone.
- Use bounded serial tests for port/config compatibility where needed.
- The implementation pass moves the registry to `closing`, not `closed`; closure remains independent.
