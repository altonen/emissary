# I2PControl Proposal 170 Milestone 003 — AddressBook Administrative API

Status: blocked

Planning baseline: `ec289c77183d4f1010829ff255d8dbe90a941ad8` (`master`)

Production-code baseline described by the planning system: `9b43484a21d5a1291c4881cdae62a36c527f8c0f`

Activation rule:

- M002 must have a closure record with status `closed`.
- Before implementation begins, the agent MUST replace the baseline above with the reviewed M002 closure head, inspect all M001/M002 production changes, and reconcile this plan against the closed conformance matrix, DTOs, control-plane interfaces, and persistence schema.
- This prewritten plan is not dependency-ready while M002 is open.

Source roadmap:

- `plans/subsystems/i2pcontrol-proposal-170-roadmap.md#milestone-003--addressbook`

Canonical requirements:

- `plans/000-long-term-specification.md#4-architectural-invariants`
- `plans/000-long-term-specification.md#5-protocol-exactness`
- `plans/000-long-term-specification.md#8-address-book-boundary`
- `plans/000-long-term-specification.md#9-truthful-state-and-observability`
- `plans/000-long-term-specification.md#10-persistence-and-recovery`
- `plans/000-long-term-specification.md#11-security-and-resource-bounds`
- `plans/001-terminology-and-domain-model.md#4-address-book-terms`
- `plans/002-long-term-roadmap.md#milestone-m003--addressbook`

Applicable ADRs:

- `plans/adrs/ADR-0001-proposal-170-contract-and-stub-boundary.md`

Primary class: capability

## 1. Objective

Implement the complete Proposal 170 `AddressBook` method and the Proposal 170 RouterInfo address-book selectors using M002's four persistent administrative stores, subscription set, and address-book configuration.

At completion, an authenticated I2PControl caller must be able to:

- address the exact `private`, `local`, `router`, and `published` administrative books;
- list or inspect stored entries according to the exact M001 request contract;
- add or update a hostname/destination entry;
- delete an entry using Proposal 170's parameter-presence semantics;
- replace the administrative subscription set;
- update the administrative address-book configuration map;
- retrieve all four books, subscriptions, and configuration through the exact Proposal 170 RouterInfo selectors;
- restart Emissary and observe the same committed administrative state.

This milestone must not make these books authoritative for runtime destination resolution, trigger subscription downloads, mutate the current runtime address book, or add frontend behavior.

## 2. Why this milestone is blocked

Hard dependency:

- M002 must close with stable administrative address-book domain types, store interfaces, versioned persistence, deterministic revisions, and fake adapters.

M003 must not create a second persistence model inside handlers. It consumes:

- the exact request/response and error conventions closed by M001;
- the exact four-book domain and generation stores closed by M002;
- M002's control-plane mutation and snapshot boundaries;
- M002's size, revision, path-confinement, and recovery semantics.

If M002 closes with an interface materially different from this plan's assumptions, the activation pass must update the file-level mechanics while preserving the capability and non-goal boundaries.

## 3. Current implementation evidence

At the production baseline:

### Existing runtime address book

- `emissary-cli/src/address_book.rs` implements one runtime-oriented address-book manager under `<base>/addressbook`.
- The manager downloads a default hosts file and configured subscriptions through the HTTP proxy.
- It keeps hostname-to-base32 mappings in memory, stores an `addresses` file, and writes base64 destinations under `destinations/<hostname>.txt`.
- `AddressBookHandle` exposes synchronous `add_base32`, `add_base64`, and `remove` functions that write directly to that runtime store.
- The runtime resolver reads from this single store through the `emissary_core::runtime::AddressBook` trait.

That implementation is useful only for destination parsing and as evidence of current runtime ownership. It is not the Proposal 170 four-book administrative store and must not be repurposed by changing its precedence or file layout.

### Existing configuration

- `AddressBookConfig` currently contains only `default` and optional `subscriptions`.
- UI code may persist runtime subscription configuration to `router.toml`.
- Proposal 170 configuration is broader and string-keyed; M003 must use M002's dedicated administrative state rather than mutating the startup configuration.

### Missing API behavior

- No `AddressBook` JSON-RPC handler exists at the planning baseline.
- No RouterInfo address-book selectors exist.
- No exact operation-mode parser exists.
- No administrative four-book listing, mutation, subscription, or configuration service exists.

## 4. Invariants that must not regress

- External book type values remain exactly `private`, `local`, `router`, and `published`.
- AddressBook request and response key names, JSON types, nullability, and envelopes follow the M001 contract exactly.
- JSON-RPC errors remain distinct from valid method results.
- `Delete` is selected by parameter presence, not by requiring a boolean `true` value, unless M001's authoritative compatibility matrix proves a different exact rule.
- Entry mutation, `SetSubscriptions`, and `SetConfig` are explicit mutually exclusive operation modes.
- Only authenticated requests reach administrative state inspection or mutation.
- Destination values are parsed and validated using Emissary's existing I2P destination primitives, not accepted as arbitrary strings.
- The four administrative books remain logically and persistently independent.
- Subscription storage does not start download work.
- Configuration storage does not authorize arbitrary filesystem paths or mutate runtime paths.
- The current runtime address book and resolver remain behaviorally unchanged.
- `router.toml` is not mutated by AddressBook handlers.
- No frontend state, UI callback, or frontend-owned persistence is used.
- A successful result means the mutation was durably committed according to M002 semantics.
- Failed persistence cannot leave an unreported in-memory-only success.
- RouterInfo selectors return only requested fields and use immutable snapshots.
- API responses are bounded and never silently truncated.

## 5. Scope

### In scope

- Exact AddressBook method request parsing and operation selection.
- Authentication/version enforcement through the M001 server boundary.
- Exact administrative book type parsing.
- Hostname and destination validation.
- List/lookup behavior defined by the M001 contract.
- Add/update behavior.
- Delete-by-presence behavior.
- `SetSubscriptions` replacement behavior.
- `SetConfig` update behavior.
- M002 store-backed control-plane implementation.
- RouterInfo selectors for:
  - private address book;
  - local address book;
  - router address book;
  - published address book;
  - subscriptions;
  - address-book configuration.
- Revision-safe concurrent mutations.
- Restart, corruption, negative, authorization, and size-limit tests.
- Protocol fixtures and support documentation.

### Explicitly out of scope

- Runtime resolver integration or precedence.
- Importing the current runtime address book into any administrative book.
- Exporting administrative books to the current runtime address book.
- Fetching or refreshing subscriptions.
- Validating subscription URLs by performing network requests.
- Reconfiguring the existing address-book downloader.
- Treating Proposal 170 path-like configuration values as permission to read or write arbitrary paths.
- Updating `router.toml`.
- UI changes.
- TunnelManager, ClientServicesInfo, or unrelated RouterInfo selectors.
- Router, NetDB, transport, proxy, SAM, I2CP, or tunnel behavior changes.
- Additional address-book types, merge operations, pagination, import/export methods, or capability fields.

## 6. Required production changes

### Protocol and DTO integration

Implement the `AddressBook` handler behind M001's exact method registry and protocol DTO conventions.

The handler must:

1. authenticate and validate the negotiated API version before accessing data;
2. parse named parameters through exact DTOs;
3. select one and only one operation mode;
4. perform operation-specific validation;
5. call a typed M002 control-plane interface;
6. serialize the exact result or JSON-RPC error envelope;
7. avoid direct file access, runtime address-book calls, or configuration edits.

Do not deserialize into an unconstrained `HashMap<String, Value>` and infer semantics throughout the handler. A narrow raw-parameter boundary may be used only to preserve presence semantics before conversion into typed operation requests.

### Operation-mode parser

Create an explicit internal operation enum, for example:

```rust
pub enum AddressBookOperation {
    EntryMutation(EntryMutationRequest),
    SetSubscriptions(SetSubscriptionsRequest),
    SetConfig(SetConfigRequest),
}
```

The exact external request shape remains authoritative. The parser must reject:

- simultaneous entry mutation and `SetSubscriptions`;
- simultaneous entry mutation and `SetConfig`;
- simultaneous `SetSubscriptions` and `SetConfig`;
- incomplete required parameter sets;
- unknown book types;
- wrong JSON types;
- conflicting duplicate semantics hidden by generic maps;
- unsupported extension fields when M001 marks them invalid.

If the M001 contract defines read/list behavior as the absence of mutation markers, model that explicitly rather than relying on handler fall-through.

### Entry validation

Implement one canonical validation path:

- validate the book type through M002's exact enum;
- enforce the M001 hostname length and syntax bounds;
- preserve hostname spelling exactly where the protocol requires it;
- reject path separators, NULs, control characters, and other invalid forms even though names do not become filesystem paths;
- decode the I2P Base64 destination with the existing Emissary decoder;
- parse it through `emissary_core::primitives::Destination`;
- reject trailing garbage or structurally invalid destinations;
- derive canonical destination data needed for equality tests without changing the returned Proposal 170 form;
- enforce request and decoded-size limits before expensive parsing.

Do not accept a base32 address where the contract requires a full destination. Do not resolve hostnames over the runtime address book as validation.

### Add and update semantics

For a valid entry-mutation request without `Delete` presence:

- create the entry when the hostname is absent;
- update/replace it when the hostname exists, according to the exact compatibility behavior frozen by M001;
- perform the mutation against exactly one selected administrative book;
- preserve all other books and state;
- commit through M002's atomic revision boundary;
- return success only after durable publication;
- return the exact method result fields and values.

If Proposal 170 distinguishes add from replace based on destination or hostname presence, preserve that exact behavior. Do not invent conflict responses or merge semantics.

### Delete-by-presence semantics

The raw parameter parser must retain whether `Delete` was present independently of its decoded value.

Unless M001 resolves otherwise, any syntactically accepted presence of `Delete` selects deletion. The implementation must not use:

```rust
if params.delete == Some(true) { ... }
```

when that would make `Delete: false` perform an add/update contrary to the proposal's presence rule.

Deletion must:

- target exactly one book and hostname/destination identity according to the contract;
- be deterministic when the entry does not exist;
- commit through M002 persistence;
- not call `AddressBookHandle::remove`;
- not delete current runtime destination files.

### Listing and lookup

Implement the exact M001 result form for reading book contents. The control plane must expose immutable snapshots with deterministic ordering.

Requirements:

- only the selected book is read;
- no cross-book merge occurs;
- entries are ordered deterministically when the wire contract does not define order;
- result construction checks configured entry and byte limits;
- oversize results fail with an explicit bounded error rather than truncating or returning a partial book;
- no secret or private-key material is present in entries;
- concurrent mutations produce either the before or after committed revision, never a torn mixture.

### SetSubscriptions

Implement exact replacement semantics through M002's `SubscriptionSet`:

- validate the exact JSON shape and item types;
- preserve ordering when the contract treats order as meaningful;
- apply documented duplicate handling exactly;
- enforce per-item and total limits;
- reject embedded control characters and malformed encodings;
- do not perform DNS, HTTP, I2P, or proxy requests;
- do not update the current runtime downloader;
- do not alter `router.toml`;
- commit as one atomic revision;
- return only after durable publication.

URL syntax validation may be performed only if required by M001. It must be syntactic, bounded, and side-effect free.

### SetConfig

Implement Proposal 170's string-keyed configuration update semantics:

- accept only the exact parameter shape frozen by M001;
- keep values as strings when the protocol defines strings;
- validate known keys without converting this method into runtime reconfiguration;
- retain compatibility-permitted unknown keys only if the M001 matrix explicitly allows them;
- reject arbitrary extension keys otherwise;
- use deterministic key ordering;
- enforce key/value/total size limits;
- redact sensitive values in logs and errors;
- treat path-like values as inert administrative strings;
- never open, create, or remove those paths;
- commit atomically through the M002 configuration store.

### RouterInfo address-book selectors

Register the six Proposal 170 address-book-related selectors through M001's RouterInfo selector registry.

Each selector must:

- be activated solely by exact selector presence;
- return only its corresponding field;
- read a single immutable M002 snapshot;
- use the exact key and JSON type;
- preserve deterministic ordering;
- obey response-size limits;
- return an explicit protocol-permitted error if the full requested field cannot be constructed;
- not read current runtime address-book files or frontend state.

When multiple address-book selectors are requested in one call, use a revision-consistent aggregate snapshot if the M002 interface supports one. Otherwise, define and test a lock order that prevents deadlock and document that independently committed stores may have different revisions.

### Control-plane ownership

Likely modules after M001/M002 reconciliation:

```text
emissary-cli/src/i2pcontrol/
    address_book.rs
    router_info/address_books.rs
    control_plane/address_books.rs
```

Handlers may depend on traits such as:

```rust
trait AddressBookControl {
    async fn snapshot_book(...);
    async fn upsert_entry(...);
    async fn delete_entry(...);
    async fn replace_subscriptions(...);
    async fn update_configuration(...);
    async fn snapshot_administrative_state(...);
}
```

Exact signatures may vary, but no handler should know state filenames or hold mutable store internals.

### Error mapping

Create a reviewed mapping from domain/store failures to M001 JSON-RPC errors or exact method results:

- invalid params;
- unknown book type;
- invalid hostname;
- invalid destination encoding;
- invalid destination structure;
- invalid operation-mode combination;
- state unavailable/corrupt;
- persistence failure;
- response too large;
- revision conflict if externally observable;
- internal invariant failure.

Do not leak filesystem paths, serialized state, destination bytes, credentials, or internal Rust error chains to callers.

### Security and authorization

- Authentication must precede parsing of expensive destination payloads where the server architecture permits.
- Request-body limits from M001 apply before full JSON allocation.
- Destination decode and parse limits must be explicit.
- Mutations must be serialized through M002's store boundary.
- No API input becomes a file path.
- Logs must include operation class and book type at most; they must not log full destinations, subscriptions, configuration values, tokens, or request bodies.
- Unauthorized requests must not reveal whether a hostname or state file exists.

### Documentation and static guards

Add support documentation that clearly states:

- these are Proposal 170 administrative books;
- they do not change current Emissary resolution;
- subscriptions are stored but not fetched by this API implementation;
- configuration is stored but not automatically applied to runtime downloader policy;
- the exact four book types and mutation behavior;
- persistence and corruption behavior;
- response-size and validation limits.

Add guards proving:

- handlers do not import or call `AddressBookHandle` mutation methods;
- handlers do not write `router.toml`;
- handlers do not use the current `<base>/addressbook` path;
- no network client is reachable from the Proposal 170 AddressBook operation path;
- no frontend module is imported;
- only exact address-book selectors are registered.

## 7. Ordered work packages

### Work package A — Reconcile exact AddressBook contract

Intent: derive implementation from the closed conformance matrix.

Required changes:

1. Update this plan to the M002 closure baseline.
2. Extract exact request modes, required fields, key-presence rules, response fields, and error conventions from M001.
3. Map every operation to one M002 control-plane method.
4. Resolve any ambiguity about Delete, no-op deletion, list form, duplicate subscriptions, and configuration replacement/update behavior before coding.
5. Add table-driven fixtures for all valid and invalid operation modes.

Acceptance evidence:

- reviewed operation matrix with no ambiguous row;
- fixture inventory linked to the M001 conformance row identifiers;
- no handler implementation begins from proposal prose alone when M001 already resolved it.

### Work package B — Typed parser and validation

Intent: make exact semantics explicit before persistence is invoked.

Required changes:

1. Implement raw presence-aware DTO conversion.
2. Implement mutually exclusive operation selection.
3. Implement exact book type parsing.
4. Implement hostname bounds and syntax validation.
5. Implement Base64 decode plus `Destination::parse` validation.
6. Implement subscription and configuration bounds.
7. Implement sanitized error mapping.

Acceptance evidence:

- unit tests for every operation-mode combination;
- Delete-presence regression test including false-like values accepted by the exact DTO;
- malformed/oversized destination tests;
- no persistence call on invalid input.

### Work package C — Store-backed mutations and reads

Intent: establish real administrative capability.

Required changes:

1. Wire upsert, delete, list/lookup, subscriptions, and configuration through M002 interfaces.
2. Preserve deterministic ordering and revisions.
3. Ensure success follows durable commit.
4. Enforce whole-response bounds without truncation.
5. Handle persistence and corruption failures without fallback to runtime state.

Acceptance evidence:

- handler tests against fake and production stores;
- revision and durable-success tests;
- per-book isolation tests;
- restart tests for every mutation class.

### Work package D — RouterInfo selector integration

Intent: expose the same canonical administrative state through RouterInfo.

Required changes:

1. Register all six exact selectors.
2. Implement selector-by-presence filtering.
3. Use immutable snapshots from M002 stores.
4. Add multi-selector consistency tests.
5. Add size-bound behavior.

Acceptance evidence:

- one fixture per selector alone;
- combined-selector fixture;
- unrelated-key absence assertions;
- result matches AddressBook method state after restart.

### Work package E — Scope guards and compatibility proof

Intent: prove that administrative capability did not alter runtime behavior.

Required changes:

1. Add source guards against runtime address-book mutation, `router.toml`, network fetch, and frontend imports.
2. Run the current runtime address-book test suite unchanged.
3. Add an integration test with conflicting runtime and administrative entries proving runtime resolution remains unchanged.
4. Document stored-but-not-fetched subscriptions and inert configuration.
5. Update conformance matrix and support documentation.

Acceptance evidence:

- runtime resolution compatibility test;
- byte/path evidence that existing runtime files were untouched;
- static guards pass;
- documentation uses no language implying runtime adoption.

## 8. Failure, cancellation, restart, and contention semantics

- Invalid requests perform no state mutation and consume no visible revision.
- Persistence failure returns an error and leaves the prior committed snapshot active.
- A mutation published durably immediately before request cancellation remains committed even if the caller receives no response; repeating the same upsert/delete must converge deterministically.
- Upsert of the same canonical hostname/destination is idempotent at the logical state level, even if an implementation chooses whether to consume a new revision.
- Delete of an absent entry follows the exact M001 result semantics and must not affect other entries or books.
- Concurrent mutations to the same book are serialized through M002.
- Concurrent mutations to different books may proceed independently only if the aggregate snapshot and lock ordering remain deadlock-free.
- `SetSubscriptions` and `SetConfig` are whole-operation atomic; partial lists/maps never become visible.
- Restart loads the newest valid committed generation according to M002 rules.
- Corrupt administrative state never causes fallback to the current runtime address book.
- Response cancellation releases snapshot references and request permits promptly.
- No AddressBook request starts a long-lived task.
- Shutdown waits only for bounded in-flight persistence according to M001/M002 cancellation policy; it does not begin network cleanup.

## 9. Compatibility and migration

- Existing Emissary configurations remain valid when no administrative state exists.
- Existing runtime address-book files and downloader behavior remain unchanged.
- No automatic import occurs from runtime state to administrative state.
- No automatic export occurs from administrative state to runtime state.
- The M002 schema remains the canonical persistence format; M003 does not introduce a second schema.
- M003 may add additive schema fields only if required by the already-frozen contract and accompanied by an explicit M002 schema migration review.
- Existing M001 clients continue using the same endpoint/authentication/version behavior.
- Proposal 170 malformed examples do not override M001's established JSON-RPC envelope.
- Unknown configuration-key compatibility follows the M001 decision exactly.
- Downgrade behavior remains that older Emissary versions ignore the separate administrative state rather than consuming it.

## 10. Required tests

### Focused unit tests

- exact book type parse/serialize;
- valid operation-mode selection;
- every conflicting operation-mode combination;
- required-field absence;
- Delete-by-presence behavior;
- hostname syntax/length boundaries;
- destination Base64 and structural parsing;
- subscription item/list bounds and ordering;
- configuration key/value/total bounds;
- error sanitization;
- selector presence and exact response key/type.

### Handler tests with fake control plane

- list/lookup for every book;
- create and update for every book;
- delete existing and absent entries;
- `SetSubscriptions` success/failure;
- `SetConfig` success/failure;
- control-plane persistence error mapping;
- response-too-large behavior;
- unauthorized requests never call the fake control plane;
- only requested RouterInfo keys appear.

### Integration tests with production stores

- full CRUD for all four books through the real HTTPS JSON-RPC listener;
- persistence across process/service reconstruction;
- subscriptions/configuration across restart;
- AddressBook method state equals RouterInfo selector state;
- concurrent client mutations;
- mutation cancellation before/after publication;
- corruption fallback inherited from M002;
- no runtime address-book file changes;
- no `router.toml` changes.

### Runtime compatibility tests

- runtime resolution with administrative store disabled/enabled is identical;
- conflicting administrative hostname does not change runtime resolution;
- current downloader still uses only existing runtime configuration;
- current UI compilation and address-book behavior remain unchanged;
- current proxy resolution tests pass.

### Security and negative tests

- unauthenticated large destination rejected before expensive work;
- oversized/deep JSON rejected at M001 boundary;
- control characters and path separators rejected in hostnames;
- destination/subscription/configuration values absent from logs;
- path-like SetConfig values cause no filesystem access;
- subscription values cause no network access;
- malformed administrative state produces sanitized errors;
- high-concurrency mutation remains bounded.

### Feature/platform tests

- headless I2PControl build;
- UI plus I2PControl build;
- all-features workspace;
- supported CI operating systems for restart and persistence behavior.

## 11. Required verification commands

The activation pass must reconcile exact feature and test names. Expected minimum:

```bash
cargo fmt --all -- --check

cargo check -p emissary-cli --no-default-features --features i2pcontrol
cargo check -p emissary-cli --features ui,i2pcontrol

cargo test -p emissary-cli --no-default-features --features i2pcontrol i2pcontrol::address_book
cargo test -p emissary-cli --no-default-features --features i2pcontrol i2pcontrol::router_info::address_books
cargo test -p emissary-cli --no-default-features --features i2pcontrol address_book

cargo test -p emissary-core
cargo test --workspace --all-features

cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Also run any M001 protocol fixture validator and M002 persistence/failpoint suite required by their closure records.

## 12. Documentation updates

- Add or update `docs/i2pcontrol/address-book.md`.
- Update `docs/i2pcontrol/proposal-170-support.md`.
- Update the machine-readable conformance matrix for all AddressBook and address-book RouterInfo rows.
- Document all four exact book names.
- Document Delete-by-presence semantics.
- Document destination validation.
- Document persistence/restart/corruption behavior.
- State prominently that administrative books do not affect runtime resolution.
- State that subscriptions are stored but not fetched.
- State that administrative configuration is not automatically applied to the runtime downloader.
- Do not document any frontend control.
- Update roadmap and registry only when dependency state changes.

## 13. Acceptance criteria

1. M002 is strictly closed and this plan is reconciled to its reviewed head.
2. `AddressBook` is registered through the M001 method registry with exact authentication/version behavior.
3. Request parsing preserves parameter-presence semantics.
4. Entry mutation, SetSubscriptions, and SetConfig are mutually exclusive explicit modes.
5. Book type accepts exactly `private`, `local`, `router`, and `published`.
6. Invalid or aliased book types are rejected.
7. Required fields and JSON types follow the M001 matrix exactly.
8. `Delete` selects deletion by exact presence semantics.
9. Hostnames are bounded and validated before persistence.
10. Destinations are decoded and structurally parsed through existing Emissary primitives.
11. Invalid destinations never reach the store.
12. Add/update is durable before success is returned.
13. Delete is durable before success is returned.
14. Each mutation affects exactly one administrative book.
15. All four books remain independent across restart.
16. Listing/lookup follows the exact result shape and deterministic ordering.
17. Oversize results fail explicitly and are never truncated.
18. SetSubscriptions persists an exact bounded ordered set/list according to the contract.
19. SetSubscriptions performs no network fetch and changes no runtime downloader configuration.
20. SetConfig persists exact bounded string data according to the contract.
21. Path-like configuration values perform no filesystem operation.
22. No AddressBook handler writes `router.toml`.
23. No AddressBook handler calls current runtime `AddressBookHandle` mutators.
24. No administrative state changes runtime destination resolution.
25. All six RouterInfo address-book selectors return exact keys and JSON types.
26. RouterInfo returns only requested address-book fields.
27. AddressBook method and RouterInfo selectors observe the same committed state.
28. Unauthorized requests reveal no administrative state and perform no mutation.
29. Concurrent mutations cannot expose torn or unpersisted state.
30. Restart and corruption behavior follows M002 without silent reset.
31. Logs and errors contain no full destination, subscription value, configuration value, token, or state path.
32. Headless and UI-enabled builds operate without frontend ownership.
33. No router/core behavior or dependency ownership changes.
34. Required protocol, persistence, compatibility, security, and concurrency tests pass.

## 14. Stop conditions

The agent must stop and report rather than improvise when:

- M002 is not strictly closed;
- the M001 contract leaves AddressBook operation modes or result envelopes unresolved;
- M002 lacks a complete four-book store or atomic mutation interface;
- exact behavior would require changing the runtime resolver;
- implementation would need to reuse or mutate `<base>/addressbook`;
- implementation would need to update `router.toml` or frontend configuration;
- SetSubscriptions would need to perform network fetches;
- SetConfig would need to honor arbitrary filesystem paths;
- a requested destination form cannot be validated with existing primitives without changing core parsing behavior;
- a response would need silent truncation to fit limits;
- arbitrary protocol extensions, extra fields, pagination, or new book types are proposed;
- work expands into TunnelManager, general RouterInfo inspection, ClientServicesInfo, or frontend behavior.

The stop report must identify the exact conflict, affected matrix rows and acceptance criteria, smallest decision required, and whether an ADR or roadmap update is necessary.

## 15. Closure evidence required

The later closure record must include:

- M002 closure reference and reconciled baseline;
- implementation commits and reviewed head;
- requirement-to-evidence mapping for all acceptance criteria;
- exact operation-mode and DTO fixture coverage;
- Delete-by-presence evidence;
- destination decode/parse evidence;
- four-book CRUD and isolation evidence;
- subscriptions/configuration persistence and restart evidence;
- AddressBook-to-RouterInfo consistency evidence;
- concurrent mutation and cancellation evidence;
- corruption/recovery evidence;
- unauthorized and oversized-request evidence;
- source/static guard evidence for no runtime handle, no runtime path, no `router.toml`, no network fetch, and no frontend imports;
- runtime resolver compatibility evidence;
- unchanged current address-book/proxy tests;
- exact verification commands and outcomes;
- unrun/platform limitations;
- unresolved findings by severity;
- roadmap and registry disposition.

Closure must be `corrective pass required` if any of these remains:

- wrong envelope/key/type/presence behavior;
- Delete requires `true` contrary to the frozen contract;
- any book is missing or merged;
- invalid destination accepted;
- non-durable success;
- partial subscriptions/configuration publication;
- runtime address-book mutation or resolver integration;
- network fetch or runtime reconfiguration;
- response truncation;
- secret or destination leakage;
- frontend coupling;
- unresolved high/medium protocol, persistence, security, or compatibility finding.

## 16. Handoff notes

- Reconcile this blocked plan after M002; do not execute against the planning baseline blindly.
- Treat the M001 conformance matrix as authoritative over examples in prose.
- Keep raw parameter handling limited to exact presence detection.
- Do not copy current runtime address-book persistence mechanics into the administrative store.
- Reuse existing destination parsing, not existing runtime mutation.
- Prefer deterministic whole-state snapshots over iterating mutable maps during serialization.
- Never log request bodies while debugging validation.
- Use ephemeral TLS ports and isolated state roots in tests.
- Keep subscription/network tests explicit: the expected network-call count is zero.
- Do not opportunistically integrate runtime resolution.
- The implementation pass updates registry status to `closing`, not `closed`; closure remains independent.
