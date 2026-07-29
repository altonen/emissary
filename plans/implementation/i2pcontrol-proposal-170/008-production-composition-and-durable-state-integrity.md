# I2PControl Proposal 170 Milestone 008 — Production Composition and Durable-State Integrity

Status: ready

Planning baseline: `b35d9ad7295f6d7d8795a741c7942a5ff7a97f52` (`master`)

Source roadmap:

- `plans/subsystems/i2pcontrol-proposal-170-roadmap.md#milestone-008--production-composition-and-durable-state-integrity`

Corrects:

- `plans/implementation/i2pcontrol-proposal-170/005-router-info-inspection.md`
- `plans/implementation/i2pcontrol-proposal-170/005-recovery.md`
- `plans/implementation/i2pcontrol-proposal-170/006-client-services-info.md`
- `plans/implementation/i2pcontrol-proposal-170/007-conformance-hardening-and-strict-closure.md`
- `plans/closure/i2pcontrol-proposal-170/005-closure.md`
- `plans/closure/i2pcontrol-proposal-170/006-closure.md`
- `plans/closure/i2pcontrol-proposal-170/007-closure.md`

Canonical requirements:

- `plans/000-long-term-specification.md`
- `plans/001-terminology-and-domain-model.md`
- `plans/002-long-term-roadmap.md`
- `plans/003-planning-process.md`, especially corrective-pass and Proposal 170 guard requirements
- `plans/adrs/ADR-0001-proposal-170-contract-and-stub-boundary.md`

Primary class: invariant / infrastructure corrective pass

## 1. Objective

Make the enabled production I2PControl path use one coherent set of loaded production services, propagate durable-state failures explicitly, and make it impossible for production startup or handlers to silently substitute fake, empty, zeroed, or separately initialized state.

This milestone does not add new RouterInfo data sources or new ClientServicesInfo observations. It repairs the composition and error-propagation foundation those later corrective milestones require.

## 2. Defects being corrected

At the planning baseline:

1. `I2pControlState::new()` installs `FakeControlPlane`, `FakeAddressBookControl`, `FakeTunnelManagerControl`, and `FakeRouterInfoControl` before production replacement.
2. `ServerInitContext` treats production dependencies as optional booleans and optional metrics.
3. `init_server()` logs warnings and continues with fake stores when production address-book or tunnel-store creation/loading fails.
4. `init_server()` constructs a second `ProductionTunnelManagerControl` for `ProductionRouterInfoControl`; that second instance is not loaded and is not the same object used by TunnelManager handlers.
5. A fixed temporary fallback directory can be used by a nominally production RouterInfo adapter.
6. `I2pControlState` helper methods suppress control-plane errors with `unwrap_or_default()`, `.ok()`, or `.flatten()`, turning failures into empty lists, missing entries, or zero-like state.
7. `ProductionControlPlane` implements tunnel queries with unconditional empty/unsupported results even though a production tunnel service exists elsewhere.
8. Existing tests build state through the same fake-first constructor used by production, so they do not prove that production can never retain a fake adapter.

These defects invalidate strict closure even when individual stores and handlers work in isolation.

## 3. Why prior verification missed the defects

Prior verification emphasized:

- type coverage and manifest counts;
- construction of production adapter types;
- isolated store round trips;
- static absence of frontend/core HTTP coupling;
- handler tests against fake control planes.

It did not assert object identity across handlers, fail-closed startup behavior, production adapter provenance, or propagation of store-load/query failures. Static tests accepted the presence of production constructors without proving that every enabled request path consumed the same successfully loaded object.

Regression evidence in this milestone must directly exercise those missed properties.

## 4. Invariants

- An enabled production server contains no fake control-plane implementation.
- A production store initialization or load failure is visible and cannot become transient in-memory success.
- AddressBook, TunnelManager, RouterInfo, and ClientServicesInfo read the same canonical loaded service objects where they share state.
- Success from a mutating handler still means durable commit.
- Query failures remain errors; they are not converted to empty collections, `None`, zero, or `false`.
- Test doubles remain available through explicit test-only constructors or dependency injection.
- No missing tunnel data plane is implemented.
- No startup-managed tunnel ownership is migrated.
- No administrative address book becomes authoritative for runtime resolution.
- No frontend state becomes required or authoritative.
- No router algorithm, transport protocol, NetDB behavior, peer selection, tunnel construction, or congestion behavior changes.
- No Proposal 170 field, method, selector, action, type, alias, or status is added.

## 5. Explicit non-goals

- Populating currently fabricated RouterInfo selectors; M009 and M010 own that work.
- Populating SAM sessions or live I2PTunnel inventory; M011 owns that work.
- Reworking persistence schemas unless required to preserve existing durable semantics.
- Replacing unsupported tunnel backends with real implementations.
- Adding a frontend management surface.
- Broadly refactoring application startup outside the I2PControl composition boundary.

## 6. Required target architecture

### 6.1 Separate production and test construction

Production state must be constructed from required, already validated dependencies. Tests may construct fake state explicitly.

Preferred shape:

```rust
pub struct ProductionControls {
    pub address_books: Arc<dyn AddressBookControl>,
    pub tunnels: Arc<dyn TunnelManagerControl>,
    pub router_info: Arc<dyn RouterInfoControl>,
    pub control_plane: Arc<dyn ControlPlane>,
    pub service_registry: ServiceRegistry,
}

impl I2pControlState {
    pub fn new_production(password: String, controls: ProductionControls) -> Self;

    #[cfg(test)]
    pub fn new_test(password: String) -> Self;
}
```

Equivalent naming is acceptable. The required property is that production construction cannot omit a production dependency and cannot default to a fake.

If integration tests outside the crate need fake state, expose a clearly named test-support constructor behind an appropriate feature or test-support module. Do not retain a generic `new()` whose behavior is ambiguous.

### 6.2 Build and load stores once

The production composition root must:

1. resolve confined paths under the configured base path;
2. create required directories;
3. construct each production store adapter once;
4. load each adapter once;
5. fail startup on any required construction/load error;
6. clone the same `Arc` into every consumer.

Example intended flow:

```rust
let address_books = Arc::new(ProductionAddressBookControl::new(address_book_dir));
address_books.load().await.map_err(I2pControlError::Persistence)?;

let tunnels = Arc::new(ProductionTunnelManagerControl::new(tunnel_dir)
    .map_err(I2pControlError::Persistence)?);
tunnels.load().await.map_err(I2pControlError::Persistence)?;

let router_info = Arc::new(ProductionRouterInfoControl::new(
    /* retained router state */
    Arc::clone(&tunnels),
));
```

Do not reopen the same path through another store instance. Do not use `std::env::temp_dir()` in enabled production composition.

### 6.3 Preserve typed errors

Introduce or reuse a sanitized internal error type that distinguishes at least:

- configuration/path creation failure;
- persistence initialization/load failure;
- persistence query/mutation failure;
- unavailable inspection source;
- internal invariant failure.

Internal errors may retain source details for logs, but JSON-RPC responses must remain sanitized and protocol-compatible.

Do not use string matching to classify errors.

### 6.4 Remove error-suppressing state helpers

Change helper methods such as tunnel list/get and address-book list/lookup to return their underlying `Result`.

Incorrect:

```rust
pub async fn tunnel_list(&self) -> Vec<TunnelDefinition> {
    self.tunnel_manager.list().await.unwrap_or_default()
}
```

Required direction:

```rust
pub async fn tunnel_list(&self) -> Result<Vec<TunnelDefinition>, ControlError> {
    self.tunnel_manager.list().await.map_err(ControlError::from)
}
```

Handlers must explicitly map the error to the established JSON-RPC error envelope. They must not interpret a query failure as an empty store or missing object.

### 6.5 Resolve the legacy `ControlPlane` overlap

Inspect every use of `ControlPlane::tunnel_list`, `tunnel_get`, and `is_tunnel_type_supported`.

Preferred correction if production handlers no longer consume them:

- narrow `ControlPlane` to retained router identity/version/uptime responsibilities;
- keep tunnel operations solely on `TunnelManagerControl`;
- update fake implementations and tests accordingly.

If a real production consumer remains, change the overlapping methods to delegate to the same shared tunnel service. Do not leave unconditional empty methods in `ProductionControlPlane`.

Record the selected option in the implementation commit and closure record. Do not create a second tunnel abstraction.

## 7. Ordered work packages

### WP1 — Baseline and consumer inventory

Before editing:

1. confirm `master` still descends from the planning baseline;
2. enumerate every construction of:
   - `I2pControlState`;
   - `ServerInitContext`;
   - `ProductionAddressBookControl`;
   - `ProductionTunnelManagerControl`;
   - `ProductionRouterInfoControl`;
   - fake control-plane implementations;
3. enumerate every `unwrap_or_default`, `unwrap_or(0)`, `.ok().flatten()`, and empty fallback in I2PControl production code;
4. classify each occurrence as test-only, protocol-defined empty state, or defect;
5. record the inventory in the implementation notes or closure evidence.

Stop if unrelated work has already replaced the composition architecture; reconcile this plan before continuing.

### WP2 — Explicit constructors and shared ownership

- Add explicit production/test state constructors.
- Change state trait-object fields to `Arc<dyn ...>` where shared identity is required.
- Remove production `set_*_production` mutation after construction unless a narrowly justified builder requires it.
- Keep test dependency injection explicit.
- Ensure `ProductionRouterInfoControl` accepts the shared tunnel service as an interface or shared concrete adapter, not a newly opened path.
- Ensure ClientServices corrective work can later access the same tunnel service without direct filesystem reads.

### WP3 — Fail-closed startup

- Remove `use_production_address_book` and `use_production_tunnel_manager` boolean fallback semantics from enabled production startup.
- Remove warnings that continue with fake adapters.
- Remove temporary fallback tunnel directories.
- Convert required adapter creation/load failures into `init_server()` errors.
- Ensure the application does not spawn the I2PControl task after failed initialization.
- Preserve the behavior that disabled I2PControl does not initialize these stores.

### WP4 — Error propagation through handlers

- Change state helpers to return typed results.
- Update AddressBook, SetSubscriptions, SetConfig, TunnelManager, RouterInfo, and ClientServicesInfo call sites as applicable.
- Use existing JSON-RPC error codes and envelopes; do not add protocol fields.
- Log sanitized context once at the ownership boundary.
- Verify that failed writes do not return success and failed reads do not return empty state.

### WP5 — Remove overlapping placeholder control methods

- Apply the selected `ControlPlane` correction from section 6.5.
- Remove unconditional empty/false production implementations.
- Add a static test preventing production implementations from containing hard-coded empty tunnel results.

### WP6 — Tests and documentation

Add focused tests described below and update:

- `docs/i2pcontrol/inspection-architecture.md`;
- `docs/i2pcontrol/administrative-state.md`;
- `docs/i2pcontrol/security.md` if startup failure behavior affects operator guidance;
- `docs/i2pcontrol/proposal-170-support.md` to state that strict closure is reopened until M008–M012 close.

## 8. Failure, cancellation, restart, and contention semantics

### Startup failure

- Directory creation, adapter construction, schema validation, or load failure aborts I2PControl initialization.
- The error must identify the affected subsystem in logs without leaking credentials, private keys, or arbitrary persisted content.
- No partially constructed server state is returned.

### Query/mutation failure

- A store/query failure returns a sanitized JSON-RPC error.
- Prior valid durable state remains active after failed mutation.
- No handler retries a non-idempotent mutation implicitly.

### Cancellation

- Cancellation while loading a store returns initialization failure; it must not publish fake state.
- Cancellation during a mutation follows the generation-store atomicity contract from M002.

### Restart

- Restart loads the same durable state into one canonical service object.
- Restart after an all-corrupt or unsupported-schema state fails explicitly according to existing persistence policy.
- Tokens remain volatile as previously specified.

### Contention

- Shared `Arc` ownership must not create nested lock acquisition across address-book, tunnel, RouterInfo, or service-registry paths.
- RouterInfo must not hold its own lock while awaiting a tunnel-store lock.
- Existing per-store mutation serialization remains intact.

## 9. Required tests

### Constructor/provenance tests

- Production construction requires all production dependencies.
- Test construction is the only path that installs fake adapters.
- A production state exposes adapter provenance in test-only introspection or proves it through injected sentinel implementations.
- No enabled production path calls `Fake*::new()`.

### Shared identity tests

Use a sentinel tunnel control with a unique atomic counter or shared generation:

1. inject it into production state and RouterInfo;
2. mutate/list through TunnelManager;
3. query through RouterInfo;
4. prove both consumers observe the same generation/object.

A test that merely opens the same directory twice is insufficient.

### Fail-closed startup tests

Through `init_server()` or a production composition harness:

- address-book directory creation failure aborts startup;
- address-book all-corrupt state aborts startup;
- tunnel directory creation failure aborts startup;
- tunnel all-corrupt or unsupported-schema state aborts startup;
- no listener is returned;
- no fake state becomes queryable;
- no temporary fallback directory is consulted.

### Error-propagation tests

Inject failing controls and prove:

- tunnel list failure returns JSON-RPC error, not `[]`;
- tunnel get failure returns JSON-RPC error, not not-found;
- address-book list failure returns JSON-RPC error, not `[]`;
- lookup failure returns JSON-RPC error, not absent entry;
- RouterInfo shared tunnel query failure does not become count `0`;
- mutation failure never returns a successful `result.status`.

### Restart tests

- create address-book and tunnel records;
- stop the server cleanly;
- reconstruct production controls from the same base path;
- prove all handlers and RouterInfo see the same records after restart.

### Static guards

Fail the test suite if enabled production source contains:

- `falling back to fake`;
- `emissary-i2pcontrol-tunnels-fallback`;
- production construction of `FakeControlPlane`, `FakeAddressBookControl`, `FakeTunnelManagerControl`, or `FakeRouterInfoControl`;
- a second `ProductionTunnelManagerControl::new()` in RouterInfo wiring;
- error-suppressing `unwrap_or_default()` or `unwrap_or(0)` in production state query helpers.

Scope guards narrowly enough that legitimate test code remains usable.

## 10. Verification commands

Run from repository root:

```bash
cargo fmt --all -- --check
cargo check -p emissary-core --features std,events
cargo check -p emissary-cli --no-default-features --features i2pcontrol
cargo test -p emissary-cli --no-default-features --features i2pcontrol
cargo test -p emissary-cli --no-default-features --features i2pcontrol --test production_adapter
cargo test -p emissary-cli --no-default-features --features i2pcontrol --test i2pcontrol
cargo test -p emissary-cli --no-default-features --features i2pcontrol --test static_guards
```

Also run the narrow new integration suite directly, for example:

```bash
cargo test -p emissary-cli --no-default-features --features i2pcontrol --test production_composition
```

Run clippy against the touched feature boundary:

```bash
cargo clippy -p emissary-cli --no-default-features --features i2pcontrol --all-targets -- -D warnings
```

If pre-existing warnings prevent `-D warnings`, record the exact baseline and prove no new warning originates in touched files. Do not report an unrun command as passing.

## 11. Acceptance criteria

1. Production and test state construction are explicitly separate.
2. Enabled production state cannot contain a fake control-plane implementation.
3. Address-book and tunnel adapters are constructed and loaded exactly once per server instance.
4. TunnelManager, RouterInfo, and later ClientServices consumers share the same tunnel service object.
5. No enabled production path uses a temporary fallback store.
6. Required production-store initialization/load failure aborts I2PControl startup.
7. No store/query failure is converted to empty, absent, zero, or false state.
8. State helper methods preserve underlying errors.
9. Handler error mapping remains sanitized and uses existing JSON-RPC envelopes.
10. `ProductionControlPlane` contains no unconditional tunnel placeholders.
11. Disabled I2PControl does not initialize Proposal 170 administrative stores.
12. Existing durable state remains compatible and no schema migration is introduced without explicit evidence.
13. Existing M003/M004 AddressBook and TunnelManager success-path behavior remains intact.
14. Unsupported tunnel backends remain explicit, inactive, and resource-free.
15. No router, transport, NetDB, tunnel data-plane, frontend, or runtime resolver behavior changes.
16. Fail-closed, shared-identity, error-propagation, and restart tests pass through production-shaped construction.
17. Static guards would have failed on every defect enumerated in section 2.
18. Documentation and support status no longer claim strict Proposal 170 closure.
19. The closure record contains no unresolved high- or medium-severity finding.

## 12. Stop conditions

Stop and mark the milestone blocked rather than broadening scope if:

- correcting shared ownership requires migrating startup-managed tunnel lifecycle;
- persistence repair requires an unplanned incompatible schema migration;
- a handler requires a new Proposal 170 field or error shape;
- core router behavior must change rather than exposing read-only state;
- production and test construction cannot be separated without a repository-wide framework rewrite.

Record the exact blocker and smallest follow-up boundary.

## 13. Closure evidence required

The closure record must include:

- implementation commit(s);
- the baseline consumer/fallback inventory;
- a before/after production composition diagram;
- proof that one loaded tunnel object is shared by all current consumers;
- exact fail-closed startup test output;
- exact error-propagation test output;
- restart evidence against retained state;
- static-guard output;
- verification commands and actual outcomes;
- compatibility and persistence review;
- unresolved findings by severity;
- disposition using the planning-process vocabulary.

M009 may activate only after this milestone is strictly closed.