# I2PControl Proposal 170 Milestone 011 — ClientServicesInfo Live State

Status: blocked

Planning baseline: `b35d9ad7295f6d7d8795a741c7942a5ff7a97f52` (`master` before corrective planning commits)

Activation rule:

- M008 and M009 must each have a closure record with disposition `closed`.
- M011 may execute in parallel with M010 after M009 closes, but final integration must be reconciled against the reviewed M010 head before M012 activates.
- Before implementation, replace this baseline with the reviewed activation head and re-run the service-source inventory.

Source roadmap:

- `plans/subsystems/i2pcontrol-proposal-170-roadmap.md#milestone-011--clientservicesinfo-live-state`

Corrects:

- `plans/implementation/i2pcontrol-proposal-170/006-client-services-info.md`
- `plans/closure/i2pcontrol-proposal-170/006-closure.md`
- ClientServicesInfo portions of M007 and its closure record

Canonical requirements:

- `plans/000-long-term-specification.md`
- `plans/001-terminology-and-domain-model.md`
- `plans/002-long-term-roadmap.md`
- `plans/003-planning-process.md`
- `plans/adrs/ADR-0001-proposal-170-contract-and-stub-boundary.md`
- `docs/i2pcontrol/proposal-170-conformance.md`
- M008 shared production-controls boundary
- M009 typed availability/error semantics where reused

Primary class: capability corrective pass

## 1. Objective

Make every ClientServicesInfo section reflect current canonical service state at request time, including live TunnelManager mutations and bounded SAM sessions, while preserving passive observation, exact Proposal 170 response shapes, explicit unsupported BOB behavior, inactive tunnel stubs, and frontend independence.

## 2. Defects being corrected

At the planning baseline:

1. I2PTunnel inventory is copied into `ServiceRegistry` once during startup and is not updated after TunnelManager Create/Edit/Delete operations.
2. `resolve_sam()` always serializes `"sessions": {}` even when a session count is present.
3. The M006 closure record acknowledges that SAM session count is fixed at zero because core exposes no bounded session snapshot.
4. HTTPProxy and SOCKS configured/starting states may serialize `enabled: true` before a listener has actually bound.
5. Missing registry entries serialize as disabled/empty state, so observer wiring failure is indistinguishable from a genuinely disabled service.
6. I2CP/SAM listener state is initially sampled, but the architecture does not prove that later listener failure or replacement updates the same canonical snapshot.
7. Existing tests mainly drive the registry directly and do not prove cross-method consistency through the production HTTPS path.

These defects make ClientServicesInfo structurally complete but not a truthful live inspection method.

## 3. Why prior verification missed the defects

Prior verification proved selector parsing, registry generation fencing, response keys, and passive observer helper behavior. It accepted startup-time inventory and a hard-coded empty SAM session object as sufficient because the response shape was stable.

The missing evidence was temporal:

- state before bind versus after bind;
- state after listener failure;
- TunnelManager mutations after server startup;
- SAM session creation/removal;
- observer/source absence versus real disabled state;
- restart reconstruction from current owners.

M011 tests must exercise these transitions through canonical producers and the real handler.

## 4. Invariants

- ClientServicesInfo is observation-only and has no start/stop/restart/rebind/reconfigure authority.
- `enabled: true` means an actual listener/service is currently active according to the selector's contract, not merely configured.
- Observer/source absence is not reported as disabled unless canonical configuration proves the service is disabled.
- I2PTunnel inventory is read from the same M008 shared TunnelManager service used by TunnelManager handlers.
- Unsupported tunnel definitions may appear as configured inventory but never as active/running/listening.
- SAM session output is bounded and contains no private keys, destination private material, authentication data, payloads, or mutable handles.
- BOB remains the exact Proposal 170 unavailable value and no BOB implementation is added.
- Only requested sections appear.
- No frontend state is consulted.
- No router, transport, SAM, I2CP, proxy, or tunnel lifecycle behavior changes except narrowly exposing passive snapshots or emitting observation updates at existing lifecycle transitions.
- No missing tunnel data plane is implemented.
- No runtime address-book integration is added.
- No new wire fields, status vocabulary, aliases, methods, or pagination are introduced.

## 5. Explicit non-goals

- Adding lifecycle commands to ClientServicesInfo.
- Migrating startup-managed tunnel ownership.
- Starting proxy, SAM, I2CP, or tunnel tasks from I2PControl.
- Exposing SAM private session context.
- Implementing BOB.
- Implementing missing tunnel types.
- Adding frontend controls.
- Broadly refactoring proxy/SAM/I2CP task supervision.
- Returning stale cached inventory merely to avoid a live bounded query.

## 6. Required source model

Create or extend a machine-readable ClientServicesInfo source map with one row per exact selector:

| Selector | Canonical source | Current-state rule | Unavailable rule |
|---|---|---|---|
| `I2PTunnel` | M008 shared `TunnelManagerControl` | Query current durable definitions at request time or use a coherently updated shared snapshot | Sanitized JSON-RPC error; never startup-stale empty success |
| `HTTPProxy` | Actual proxy listener/task observation | `enabled: true` only after successful bind and while task is active | Error if observer source is missing; `enabled: false` for known disabled/stopped/failed state |
| `SOCKS` | Actual SOCKS listener/task observation | Same as HTTPProxy | Same as HTTPProxy |
| `SAM` | Actual SAM listener plus bounded current session snapshot | Listener and sessions captured coherently enough to avoid invented sessions | Error if active listener source cannot provide required session shape |
| `BOB` | Contract-defined unsupported service | Exact unavailable value | Not applicable |
| `I2CP` | Actual I2CP listener ownership | `enabled: true` only while bound/active | Error if expected source is missing; false for known disabled/stopped state |

The exact response schema must be reconciled against Proposal 170 and the accepted M001 contract matrix before implementation. If the existing conformance document incorrectly says `array` while production/docs use an object, correct the normative inventory with cited contract evidence rather than preserving an internal inconsistency.

## 7. Required architecture

### 7.1 Composite read-only service control

Prefer one internal read-only interface used by the handler, for example:

```rust
#[async_trait]
pub trait ClientServicesControl: Send + Sync {
    async fn i2p_tunnel_inventory(&self) -> Result<TunnelInventory, ServiceInspectionError>;
    fn http_proxy_snapshot(&self) -> Result<ListenerSnapshot, ServiceInspectionError>;
    fn socks_snapshot(&self) -> Result<ListenerSnapshot, ServiceInspectionError>;
    async fn sam_snapshot(&self) -> Result<SamSnapshot, ServiceInspectionError>;
    fn i2cp_snapshot(&self) -> Result<ListenerSnapshot, ServiceInspectionError>;
}
```

Equivalent decomposition is acceptable. Requirements:

- it consumes the shared M008 TunnelManager object, not files;
- listener registry/snapshot ownership is explicit;
- source absence is representable;
- errors are typed and sanitized;
- no lifecycle handles are exposed;
- test doubles require explicit state.

### 7.2 Live I2PTunnel inventory

Preferred correction: query the shared `TunnelManagerControl::list()` only when `I2PTunnel` is requested.

This avoids maintaining a second mutable inventory cache. If performance requires a cache, it must be updated transactionally after every successful create/edit/rename/delete and rebuilt from the shared service on restart. The closure record must prove no stale window after a successful mutation response.

Inventory mapping must:

- use current definitions;
- preserve exact client/server classification rules;
- include unsupported definitions only as configured inventory;
- exclude invented runtime-active fields;
- propagate store failure rather than return empty maps;
- enforce the existing complete-result bound without truncation.

### 7.3 Listener state semantics

Define internal states precisely:

- `Disabled`: configuration says service is disabled and no task/listener is active.
- `Starting`: task exists but bind has not succeeded.
- `Listening`: bind succeeded and the serving task remains active.
- `Failed`: task/bind failed.
- `Stopping`: shutdown initiated; no longer report enabled unless the external contract explicitly defines otherwise.
- `Stopped`: task exited and listener is closed.
- `Unavailable`: observer/source wiring is absent or owner is gone unexpectedly; distinct from disabled.

Public mapping:

- `enabled: true` only for `Listening` or another contract-proven active state;
- `Starting` and `Configured` do not report active merely because configuration requested enablement;
- `Disabled`, `Failed`, `Stopping`, and `Stopped` report the exact inactive value;
- `Unavailable` produces a sanitized error rather than inactive success.

### 7.4 Bounded SAM session inspection

Add the smallest read-only snapshot at the canonical SAM session owner.

Requirements:

- count and/or session entries follow the exact Proposal 170 schema;
- session identifiers are public administrative identifiers only;
- no destination private keys, signing keys, encryption keys, authentication tokens, payload data, remote peer secrets, or mutable session contexts;
- check collection size before copying/serializing;
- fail explicitly if the complete result exceeds the bound;
- session add/remove/close transitions update current state;
- listener state and session snapshot do not require one global lock held through serialization.

If Proposal 170 requires detail that cannot be exposed safely or does not exist in Emissary, stop and record a contract blocker. Do not return `{}` as a substitute.

### 7.5 Observer lifecycle

Existing generation fencing may remain, but production producers must emit transitions at actual lifecycle points:

- before bind attempt: `Starting`;
- after successful bind: `Listening` with actual local address;
- bind/serve failure: `Failed` with sanitized internal detail;
- normal task exit: `Stopped`;
- shutdown initiation: `Stopping` where observable;
- replacement/restart: new generation invalidates stale producers.

Drop guards must not report stopped while a replacement task is already active under a newer generation.

## 8. Ordered work packages

### WP1 — Reconcile exact contract and current producers

- Re-read Proposal 170 and the accepted conformance matrix for all six selectors.
- Resolve the existing response-type inconsistency in documentation/tests.
- Inventory all production producers and consumers:
  - HTTP proxy construction/spawn/exit;
  - SOCKS construction/spawn/exit;
  - SAM listener and session context;
  - I2CP listener;
  - TunnelManager handler and shared production control;
  - `ServiceRegistry` construction and replacement.
- Record which lifecycle transitions are currently observable and which are missing.

Stop if current code has materially changed; update the plan baseline before editing.

### WP2 — Add explicit service inspection errors/control

- Add `ClientServicesControl` or equivalent.
- Distinguish disabled, stopped, unavailable, failed, and active state internally.
- Update fake/test controls to require explicit selector state.
- Map errors to existing sanitized JSON-RPC envelopes.

### WP3 — Make I2PTunnel current

- Replace startup-only registry inventory with a request-time shared TunnelManager query, or implement transactional cache updates if justified.
- Remove direct construction of a second `ProductionTunnelManagerControl` in `main.rs` solely to populate inventory.
- Add cross-method consistency tests for create/edit/rename/delete.
- Verify unsupported definitions remain inactive.

### WP4 — Correct HTTP/SOCKS/I2CP listener truthfulness

- Emit actual lifecycle transitions from existing task boundaries.
- Map starting/configured to inactive public state until bind succeeds.
- Map unexpected source absence to error.
- Ensure listener failure/exit is reflected without waiting for restart.
- Do not add task ownership or control to the registry.

### WP5 — Add SAM session snapshot

- Add bounded read-only session inspection at the canonical SAM owner.
- Wire session creation/removal/closure to current snapshot state.
- Map exact safe session fields to Proposal 170.
- Add oversize and secret-redaction tests.
- Preserve actual listener enabled state independently from session count.

### WP6 — Handler consistency and bounds

- Fetch only requested sections.
- Take one snapshot/query per requested category.
- Abort without partial result on a required category failure if the protocol has no per-section error representation.
- Enforce exact collection/response limits.
- Preserve exact BOB value.
- Preserve authentication/version checks before service inspection.

### WP7 — Documentation and static guards

Update:

- `docs/i2pcontrol/client-services.md`;
- `docs/i2pcontrol/proposal-170-conformance.md`;
- `docs/i2pcontrol/proposal-170-support.md`;
- `docs/i2pcontrol/inspection-architecture.md` if the shared control boundary changes.

Add static/behavioral guards preventing startup-only inventory and hard-coded empty SAM sessions from returning.

## 9. Failure, cancellation, restart, and contention semantics

### Failure

- Listener bind/serve failure becomes known inactive/failed state, not stale enabled state.
- Observer/source disappearance becomes unavailable error, not disabled.
- Tunnel store query failure becomes JSON-RPC error, not empty inventory.
- SAM session snapshot failure becomes error when SAM is requested.
- BOB remains exact unavailable value and never errors due to missing implementation.

### Cancellation

- Cancelling a ClientServicesInfo request drops only snapshot/query work.
- No proxy/SAM/I2CP/tunnel task is cancelled by inspection.
- No shared lock remains held after cancellation.

### Restart

- Listener/session observers allocate new generations.
- Stale previous-generation updates are rejected.
- Tunnel inventory reconstructs from the shared durable store.
- SAM sessions reconstruct from the new SAM owner and start empty only if the owner confirms no sessions.
- No process-global cached service state survives.

### Contention

- Concurrent TunnelManager mutations and ClientServicesInfo queries produce a coherent before-or-after inventory.
- A successful mutation response must be visible to subsequent queries.
- SAM session snapshots use bounded copy/read operations.
- No registry lock is held while awaiting TunnelManager or SAM queries.
- Continuous polling does not materially affect proxy/SAM/I2CP serving tasks.

## 10. Required tests

### I2PTunnel cross-method tests

Through the real authenticated HTTPS endpoint:

1. request I2PTunnel before creation and observe real empty inventory;
2. create a supported-or-stubbed definition through TunnelManager;
3. immediately request ClientServicesInfo and observe it;
4. edit/rename and observe exact current name/data;
5. delete and observe removal;
6. force the shared store query to fail and observe JSON-RPC error, not empty inventory;
7. start an unsupported definition and prove it never appears active.

### Listener lifecycle tests

For HTTPProxy and SOCKS:

- configured before bind does not report enabled;
- successful bind reports enabled with actual address/port;
- bind failure reports inactive/failed result according to exact wire shape;
- task exit reports inactive;
- source removal/unwired production control reports error, not disabled;
- stale generation cannot overwrite replacement listener state.

For I2CP and SAM listener:

- actual bound listener reports enabled;
- disabled configuration reports inactive;
- failed/closed listener updates current state;
- source unavailable is distinct from disabled.

### SAM session tests

- listener active with zero sessions returns exact active/empty shape;
- create one or more sessions through supported SAM test seams and observe exact safe entries/count;
- close sessions and observe removal;
- oversize session collection fails explicitly without truncation;
- response/log/error contains no private/session key material;
- concurrent create/close/query returns coherent snapshots without panic/deadlock.

### Selector and failure tests

- only requested sections appear;
- failure in unrequested category has no effect;
- failure in requested category returns error and no partial result;
- BOB exact value remains unchanged;
- response-size bound is based on actual requested/current collections, not a tautological estimate alone.

### Restart tests

- create durable tunnel definitions;
- run active listener/session fixtures;
- stop and reconstruct application state;
- prove durable inventory returns after restart;
- prove volatile listener/session state reflects only the new process;
- prove stale generation updates are rejected.

### Static guards

Fail if production code contains:

- startup-only I2PTunnel population without mutation/query integration;
- `session_count.unwrap_or(0)` used to justify an always-empty SAM response;
- unconditional `"sessions": {}` for an active source that has sessions;
- configured/starting proxy mapped to `enabled: true`;
- missing registry/source mapped directly to disabled in production;
- direct persistence-file reads from ClientServicesInfo;
- lifecycle task handles or cancellation authority in service inspection DTOs.

## 11. Verification commands

```bash
cargo fmt --all -- --check
cargo check -p emissary-core --features std,events
cargo check -p emissary-cli --no-default-features --features i2pcontrol
cargo test -p emissary-core
cargo test -p emissary-cli --no-default-features --features i2pcontrol
cargo test -p emissary-cli --no-default-features --features i2pcontrol --test client_services_integration
cargo test -p emissary-cli --no-default-features --features i2pcontrol --test client_services_live
cargo test -p emissary-cli --no-default-features --features i2pcontrol --test i2pcontrol
cargo test -p emissary-cli --no-default-features --features i2pcontrol --test static_guards
```

Run clippy on touched packages:

```bash
cargo clippy -p emissary-core --features std,events --all-targets -- -D warnings
cargo clippy -p emissary-cli --no-default-features --features i2pcontrol --all-targets -- -D warnings
```

Record exact outcomes and environmental skips. Do not treat direct registry unit tests as a substitute for production-path temporal evidence.

## 12. Acceptance criteria

1. Exact ClientServicesInfo response schemas are reconciled with Proposal 170 and the normative matrix.
2. Every selector has one canonical source and unavailable rule.
3. I2PTunnel uses the same M008 shared TunnelManager service as TunnelManager handlers.
4. Successful Create/Edit/Rename/Delete is visible to the next ClientServicesInfo query without restart.
5. Tunnel store failure does not become empty inventory.
6. Unsupported definitions appear only as configured inventory and never active/running/listening.
7. HTTPProxy reports enabled only after actual successful bind and while active.
8. SOCKS reports enabled only after actual successful bind and while active.
9. I2CP enabled state reflects actual current listener state.
10. SAM enabled state reflects actual current listener state.
11. SAM sessions reflect actual bounded current sessions using the exact safe wire shape.
12. Active SAM sessions are not reported as an empty object solely because inspection was missing.
13. No SAM private/session-sensitive material is exposed.
14. Missing observer/source state is distinct from known disabled state.
15. Listener failure/exit and replacement update current state with generation fencing.
16. Only requested sections appear.
17. Requested-source failure returns a sanitized error with no partial result.
18. Unrequested-source failure performs no work and does not fail the request.
19. BOB remains the exact unsupported value and no BOB runtime/configuration is added.
20. Snapshot/query work is bounded, cancellation-safe, contention-safe, and frontend-independent.
21. No service lifecycle authority, router behavior, runtime resolver change, or missing tunnel data plane is introduced.
22. Production HTTPS tests cover temporal transitions and cross-method consistency.
23. Documentation no longer claims startup-stale or empty-placeholder behavior as complete.
24. M006 may be reconsidered for strict closure only after independent review finds no high/medium defect.

## 13. Stop conditions

Stop and record a blocker if:

- Proposal 170 requires SAM data that cannot be exposed safely;
- exact response shape cannot be reconciled without an ADR;
- current service owners cannot expose passive state without transferring lifecycle authority;
- a complete result cannot be bounded without protocol pagination/extension;
- live inventory would require direct file watching instead of the shared service;
- implementation begins adding BOB or missing tunnel data planes.

Do not restore stale caches or empty placeholders to avoid a blocker.

## 14. Closure evidence required

The closure record must include:

- implementation commits;
- final selector/source/schema table;
- before/after temporal behavior table;
- production HTTPS cross-method test output;
- listener bind/failure/exit transition evidence;
- SAM session create/remove/oversize/redaction evidence;
- restart and stale-generation evidence;
- static-guard output;
- verification command outcomes;
- compatibility/security/lifecycle review;
- unresolved findings by severity and disposition.

M012 remains blocked until M010 and M011 are both strictly closed.