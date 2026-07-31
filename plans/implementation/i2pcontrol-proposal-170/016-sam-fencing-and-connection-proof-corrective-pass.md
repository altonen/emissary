# I2PControl Proposal 170 Milestone 016 — SAM, Fencing, and Connection-Proof Corrective Pass

Status: ready

Planning baseline: `43088a42881a76b3936c76f6e7eb8a51262504c4`

Source roadmap:

- `plans/subsystems/i2pcontrol-proposal-170-roadmap.md`

Canonical requirements:

- `plans/000-long-term-specification.md`
- `plans/003-planning-process.md`
- `plans/adrs/ADR-0001-proposal-170-contract-and-stub-boundary.md`
- `docs/i2pcontrol/proposal-170-conformance.md`
- I2P Proposal 170, especially `ClientServicesInfo`
- the existing I2PControl JSON-RPC error and response contract

Primary class: tightly bounded correctness and evidence corrective pass

## 1. Objective

Correct the three remaining Proposal 170 closure defects without broadening the API, changing router behavior, reopening already validated security architecture, or adding CI/release machinery:

1. resolve the SAM `sessions` response against the actual Proposal 170/i2pd contract instead of treating an unavailable source as automatically equivalent to an empty active-session map;
2. make same-category service-generation validation and state replacement atomic;
3. replace the current below-limit TLS concurrency test with a deterministic test that actually saturates and releases the existing pre-spawn connection bound.

This milestone is not a general I2PControl reimplementation. M014’s accepted fixes for RouterInfo source truthfulness, production metrics/log wiring, explicit unsupported selectors, and the local connection semaphore remain intact unless a direct regression is discovered while touching one of the three owned defects.

## 2. Governing constraints

Apply these constraints in order:

1. Proposal 170 and the adopted i2pd `ClientServicesInfo` behavior are authoritative for SAM wire semantics.
2. Do not invent session fields, identifiers, unavailable envelopes, or extension statuses.
3. Do not report unavailable current session state as successful empty state unless the adopted contract or pinned upstream implementation explicitly defines that behavior.
4. Prefer a minimal bounded read-only accessor over a new registry, event stream, cache, observer task, or lifecycle API.
5. Keep service-registry correction entirely inside the existing I2PControl registry implementation.
6. Keep connection-limit correction local to the existing I2PControl server and existing adversarial test suite.
7. Do not touch unrelated hardened code merely because the repository contains later formatting churn.
8. Verification remains local and package-scoped. Do not add or modify GitHub Actions, release jobs, matrices, coverage rules, or generated evidence systems.
9. If the SAM contract cannot be resolved precisely, stop with a named blocker; do not redefine the contract to obtain closure.

## 3. Exact findings owned by M016

M016 owns only these findings:

| ID | Finding | Severity | Required disposition |
|---|---|---|---|
| M016-F1 | Active SAM can return `sessions: {}` when sessions are unobservable, while Proposal 170 describes active-session information and the example alone does not define an unavailable-source fallback | medium, contract-dependent | pin contract; implement exact bounded behavior or retain explicit blocker |
| M016-F2 | `ServiceRegistryInner::update_service` checks generation and writes the entry under separate locks, allowing a same-category replacement to race between validation and write | medium | one atomic category ownership/state transaction |
| M016-F3 | `tls_connection_bound_enforced` opens 10 connections against a production limit of 128 and therefore does not prove saturation, rejection, or restoration | medium evidence defect | deterministic over-limit behavioral regression |
| M016-F4 | M015 closure claims zero findings and complete evidence despite F1–F3 | closure defect | superseded by M017; do not edit M015 into passing |

No other historical or architectural finding is activated by this plan.

## 4. Scope boundary

### 4.1 Primary allowed production files

Production changes should be confined to:

- `emissary-cli/src/i2pcontrol/client_services.rs`
- `emissary-cli/src/i2pcontrol/service_registry.rs`
- `emissary-cli/src/i2pcontrol/server.rs`
- the smallest existing I2PControl DTO/control trait file needed by an exact SAM mapping

### 4.2 Allowed tests and documentation

- `emissary-cli/tests/client_services_live.rs`
- `emissary-cli/tests/client_services_integration.rs`
- `emissary-cli/tests/adversarial.rs`
- directly relevant unit tests in the three production files above
- `docs/i2pcontrol/client-services.md`
- `docs/i2pcontrol/proposal-170-conformance.md`
- `docs/i2pcontrol/proposal-170-support.md`
- `plans/**/i2pcontrol-proposal-170/**`
- `plans/registry.md`

### 4.3 Narrow core exception for SAM only

A core edit is allowed only if the contract reconciliation proves that active SAM session information is required and the exact safe output can be populated from Emissary’s canonical SAM owner.

Permitted core change:

- one bounded, read-only, secret-free snapshot method on the existing canonical SAM owner;
- one small DTO containing only fields explicitly required by the adopted contract;
- no mutation, lifecycle authority, listener ownership, persistence, event subscription, or background task.

The implementation commit that touches core must record:

- pinned Proposal 170 revision/date;
- pinned upstream i2pd commit and source location used to determine the adopted response;
- exact output fields and their source;
- collection bound;
- proof that destinations, private keys, payloads, authentication data, and mutable session handles are excluded.

If the exact response shape is not determinable, or safe exposure requires more than one small owner accessor, stop. Do not alter SAM internals broadly.

### 4.4 Explicitly forbidden

M016 must not:

- modify `.github/workflows/**`;
- add CI jobs, nightly checks, platform matrices, required checks, coverage gates, or evidence archives;
- change release, publishing, packaging, or version policy;
- reformat the repository or touch unrelated files to satisfy `cargo fmt`;
- redesign SAM, I2CP, HTTP proxy, SOCKS, router, transport, NetDB, tunnel pools, logging, metrics, events, or runtime scheduling;
- add a generic service registry, task supervisor, connection manager, metrics collector, logger, cache, polling task, or observer framework;
- change Proposal 170 method names, selectors, request keys, response keys, statuses, or error envelopes;
- implement missing tunnel data planes or BOB;
- add frontend controls or runtime address-book integration;
- reopen unrelated cryptographic, protocol, transport, or security-hardening code;
- add a dependency unless the exact SAM wire contract cannot be tested with existing dependencies. A dependency proposal is a stop-and-review event, not presumed approval.

## 5. Ordered implementation work

### WP1 — Pin the adopted SAM contract before editing code

Do not begin SAM implementation from the current Emissary documentation.

Perform a compact source reconciliation:

1. Pin the current official Proposal 170 revision/date.
2. Record that Proposal 170 describes `SAM` as returning bridge enabled state and active-session information.
3. Pin one upstream i2pd commit that implements `ClientServicesInfo`.
4. Locate the exact i2pd `ClientServicesInfo` SAM serialization path.
5. Record the concrete `sessions` object keys and per-session value shape, if any.
6. Record behavior for:
   - SAM disabled;
   - SAM listening with zero active sessions;
   - SAM listening with one or more active sessions;
   - session source failure or unavailability, if upstream defines it.
7. Compare this against the existing Emissary conformance matrix and JSON-RPC error vocabulary.

Create a short contract note inside the M016 implementation commit or closure draft. Do not create a new project-wide matrix.

Decision table:

| Upstream/spec result | Required implementation |
|---|---|
| Active sessions have a defined safe map shape | add the smallest bounded canonical read and serialize exactly that shape |
| Empty object is explicitly defined for all active SAM states, not merely shown as a zero-session example | retain empty object and add a source-backed regression proving the interpretation |
| Contract permits an existing I2PControl error when session state is unavailable | use that existing error; do not add a new status |
| Contract remains ambiguous or upstream behavior cannot be pinned | stop, mark M016 blocked on contract clarification, keep M017 blocked |

Do not use the current comment “empty by contract” as evidence. The purpose of WP1 is to validate or replace that claim.

### WP2 — Implement only the selected SAM path

#### Path A — Exact bounded session snapshot

Use this path only if WP1 identifies a concrete adopted session map.

Requirements:

- snapshot is requested only when the `SAM` selector is requested and SAM is listening;
- snapshot is collected on demand; no polling or cache;
- fixed maximum count, with failure before unbounded response construction;
- stable map key and value shape exactly matching the adopted contract;
- no private destination, private key, payload, socket, stream, authentication, or mutable handle;
- session removal is reflected on the next request;
- zero live sessions returns the contract’s genuine empty object;
- source failure does not become successful empty state unless WP1 proves that behavior.

#### Path B — Existing unavailable/error behavior

Use this path only if WP1 proves the adopted contract permits it.

Requirements:

- use an existing compatible JSON-RPC error path;
- do not add an Emissary-specific `available`, `partial`, `unsupported`, or `reason` field;
- disabled SAM behavior remains exactly as specified;
- listening-but-unobservable is distinct from listening-with-zero-sessions where the contract distinguishes them.

#### Path C — Contract blocker

If neither path is supported by authoritative evidence:

- do not change production behavior merely to obtain passing tests;
- update registry status to `blocked` with the exact missing contract fact;
- keep support documentation qualified;
- do not activate M017.

### WP3 — Make service generation and entry update atomic

Replace the split-lock transaction with one small state lock.

Preferred implementation:

```rust
struct ServiceRegistryState {
    entries: HashMap<ServiceCategory, ServiceEntry>,
    generations: HashMap<ServiceCategory, u64>,
}

struct ServiceRegistryInner {
    state: RwLock<ServiceRegistryState>,
}
```

Required behavior:

- `allocate_handle(category)` increments only that category while holding the state write lock;
- `update_service(category, handle_generation, ...)` validates the current category generation and replaces the entry while holding the same state write lock;
- an older same-category handle cannot write after a newer handle is allocated;
- allocating or updating a different category does not invalidate the handle;
- `snapshot()` copies the fixed six-category state under one read lock;
- registry remains passive and contains no task handles or lifecycle authority;
- failure remains internal and sanitized;
- do not introduce lock-free atomics, per-entry mutexes, epochs, channels, or a generic transactional store.

If the existing public snapshot generation is retained, document its meaning. It must not be used as a global stale-owner fence.

### WP4 — Add deterministic generation regressions

Add the smallest tests needed:

1. **Cross-category isolation:** allocate HTTP and SOCKS handles; allocate a newer SOCKS handle; the original HTTP handle still updates successfully.
2. **Same-category replacement:** allocate HTTP old/new handles; old update fails; new update succeeds.
3. **No stale overwrite:** set state with the new handle, then attempt old-handle update; snapshot remains the new state.
4. **Atomic implementation guard:** inspect or unit-test that generation validation and entry replacement use the same state write lock. Do not build a scheduler/fault-injection framework.

A deterministic sequential stale-overwrite test plus the single-lock implementation is sufficient. Do not add probabilistic stress loops merely to manufacture “concurrency evidence.”

### WP5 — Make the connection limit testable without changing public configuration

The production default remains 128. Do not add a user-facing setting.

Preferred implementation:

- store a connection-task limit or semaphore inside `ServerInstance`;
- `init_server` always installs `MAX_CONNECTION_TASKS`;
- add one `#[doc(hidden)]` or test-oriented constructor/helper that installs a small limit for integration tests;
- `serve()` uses the instance-owned limit/semaphore;
- no process-wide state and no generic connection manager.

An equivalent local design is acceptable if it keeps the production interface and default unchanged.

### WP6 — Replace the false connection-limit regression

Rewrite `tls_connection_bound_enforced` so it proves the actual property.

Recommended deterministic test with test limit `2`:

1. start the real TLS I2PControl listener with connection-task limit 2;
2. open two TCP connections and hold them in an incomplete TLS handshake, proving both permits remain occupied;
3. open a third connection;
4. assert the third socket is closed/reset promptly and cannot reach TLS or JSON-RPC;
5. close one held connection;
6. wait only for deterministic task/permit release, with a bounded timeout;
7. open a new TLS connection, authenticate, and complete one protected JSON-RPC request;
8. shut down the listener cleanly.

Required assertions:

- the test exceeds the configured test limit;
- saturation behavior is observable, not inferred from source text;
- capacity restoration is observed after disconnect;
- handler semaphore behavior remains independent;
- plaintext rejection and normal TLS authentication tests remain intact;
- no 30-second sleep is required for the normal passing path;
- all waits have explicit timeouts.

Do not count 10 successful requests below a 128 limit as saturation evidence.

### WP7 — Correct documentation and planning state

Update only claims directly affected by M016:

- replace unsupported “empty by contract when unobservable” language with the WP1-established behavior;
- retain explicit descriptions of unsupported tunnel data planes;
- state that the connection implementation existed in M014 but saturation proof is completed by M016;
- mark M015 closure invalidated rather than silently editing it into a passing record;
- move M016 to `closing` and M017 to `ready` only after implementation and required regressions land on a frozen head.

Do not rewrite unrelated I2PControl documentation.

## 6. Regression test inventory

Required cases:

| Case | Preferred location |
|---|---|
| SAM disabled exact response | existing `client_services_live.rs` or unit tests |
| SAM listening with zero sessions | existing client-services suite |
| SAM listening with one current session, if contract defines entries | existing client-services integration suite plus minimal core unit test |
| SAM session removal reflected on next request, if entries are implemented | existing client-services integration suite |
| SAM unavailable/source-failure behavior | existing client-services suite |
| cross-category generation isolation | `service_registry.rs` unit tests |
| same-category stale rejection and no overwrite | `service_registry.rs` unit tests |
| actual connection saturation over configured test limit | `adversarial.rs` |
| permit restoration after disconnect | `adversarial.rs` |
| real TLS authentication and plaintext rejection remain passing | existing `adversarial.rs` tests |

Do not add a new test framework, generated fixture system, evidence bundle, network interoperability farm, or CI-only test mode.

## 7. Verification commands

Run locally:

```bash
cargo fmt --all -- --check
cargo check -p emissary-cli --no-default-features --features i2pcontrol
cargo test -p emissary-cli --no-default-features --features i2pcontrol
cargo clippy -p emissary-cli --no-default-features --features i2pcontrol --all-targets -- -D warnings
```

Focused development reruns:

```bash
cargo test -p emissary-cli --no-default-features --features i2pcontrol --test client_services_live
cargo test -p emissary-cli --no-default-features --features i2pcontrol --test client_services_integration
cargo test -p emissary-cli --no-default-features --features i2pcontrol --test adversarial
```

Only if the permitted SAM core accessor is added:

```bash
cargo check -p emissary-core --features std,events
cargo test -p emissary-core
cargo clippy -p emissary-core --features std,events --all-targets -- -D warnings
```

Do not require or add:

- full workspace tests unrelated to touched code;
- desktop/UI builds;
- multi-platform CI;
- nightly jobs;
- coverage thresholds;
- fuzz campaigns;
- release dry runs;
- generated evidence archives;
- repeated verification loops after an unchanged frozen head.

## 8. Acceptance criteria

M016 implementation is complete only when all applicable criteria pass:

1. Work remains limited to Proposal 170 `ClientServicesInfo`, existing service fencing, and existing I2PControl connection bounds.
2. No Proposal 170 method, selector, request key, response key, status, or extension is added.
3. Official Proposal 170 and one pinned upstream i2pd implementation are recorded for the adopted SAM behavior.
4. The SAM result for disabled, zero-session, active-session, and unavailable-source states follows that pinned behavior exactly.
5. An example empty object is not treated as proof of an unavailable-source fallback.
6. If active session entries are required, they come from one minimal bounded canonical read-only source.
7. No sensitive SAM material or mutable authority is exposed.
8. If the exact contract cannot be established safely, M016 records a blocker and does not claim completion.
9. Service generation validation and entry replacement occur atomically under one state write lock or an equally simple atomic transaction.
10. Cross-category handle allocation never invalidates another category.
11. Same-category replacement always prevents older handles from overwriting current state.
12. Registry remains fixed-size, passive, secret-free, and lifecycle-free.
13. Production connection-task limit remains 128 and is not added to user configuration.
14. A test-only small limit allows deterministic saturation testing without a generic framework.
15. The saturation test opens more simultaneous connection work than the configured test limit.
16. The over-limit connection is observably rejected before JSON-RPC.
17. Capacity is observably restored after disconnect or task completion.
18. Existing real TLS Authenticate/protected dispatch and plaintext rejection tests remain passing.
19. No unrelated core, UI, proxy, transport, NetDB, tunnel, cryptographic, or security code is changed.
20. No `.github/workflows/**`, CI policy, release, publishing, matrix, coverage, or evidence-generation file is changed.
21. No repository-wide formatting pass is included.
22. Targeted format, check, tests, and clippy pass for touched packages.
23. Documentation states the actual SAM and connection-proof semantics.
24. M015 remains an invalid historical closure record; it is not rewritten to conceal the defects.
25. Implementation freezes a head, moves M016 to `closing`, and activates only M017.

## 9. Stop conditions

Stop and record a blocker rather than expanding scope when:

- Proposal 170/i2pd does not define enough SAM session structure to serialize exact active-session information;
- safe session output requires exposing private destinations, keys, payloads, authentication, sockets, or mutable session handles;
- SAM observation requires a new background task, event stream, registry, persistence store, or lifecycle redesign;
- registry correction expands beyond one local state lock and fixed category map;
- connection proof requires a production configuration surface, process-wide supervisor, or CI-only environment;
- implementation starts modifying unrelated core/security code or reformatting the workspace;
- a new dependency or CI workflow is proposed.

In these cases, preserve the current safe boundary, leave M017 blocked, and record the precise unresolved contract or implementation fact.

## 10. Handoff sequence for reliable execution

1. Pin Proposal 170 and upstream i2pd SAM serialization.
2. Write the compact SAM decision note.
3. Implement only the selected SAM path or stop with a blocker.
4. Consolidate service registry entries/generations into one atomic state lock.
5. Add deterministic registry regressions.
6. Add the smallest test-only connection-limit seam.
7. Replace the below-limit test with a real saturation/restoration test.
8. Run only the targeted commands.
9. Update directly affected docs.
10. Freeze the implementation head and move M016 to `closing`; activate M017.

Do not combine M016 implementation and M017 closure in the same pass.