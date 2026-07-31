# I2PControl Proposal 170 Milestone 006 — ClientServicesInfo

Status: closed

Planning baseline: `95a37f029cd37b8b00fbebddbdc178e3f168fbdc` (`master`)

Production-code baseline described by the planning system: `95a37f029cd37b8b00fbebddbdc178e3f168fbdc`

Activation rule:

- M004 and M005 each have a closure record with status `closed`.
- This plan is dependency-ready and authorized for execution.

Source roadmap:

- `plans/subsystems/i2pcontrol-proposal-170-roadmap.md#milestone-006--clientservicesinfo`

Canonical requirements:

- `plans/000-long-term-specification.md#4-architectural-invariants`
- `plans/000-long-term-specification.md#5-protocol-exactness`
- `plans/000-long-term-specification.md#7-existing-runtime-ownership`
- `plans/000-long-term-specification.md#9-truthful-state-and-observability`
- `plans/000-long-term-specification.md#11-security-and-resource-bounds`
- `plans/001-terminology-and-domain-model.md#5-router-and-service-inspection-terms`
- `plans/002-long-term-roadmap.md#milestone-m006--clientservicesinfo`

Applicable ADRs:

- `plans/adrs/ADR-0001-proposal-170-contract-and-stub-boundary.md`

Primary class: capability

## 1. Objective

Implement the complete Proposal 170 `ClientServicesInfo` method for the exact service selectors:

- `I2PTunnel`;
- `HTTPProxy`;
- `SOCKS`;
- `SAM`;
- `BOB`;
- `I2CP`.

At completion, authenticated callers must receive only requested service sections, populated from truthful passive listener/session/registry state. Stubbed tunnel definitions must remain inactive, frontend presence must not affect results, and the method must not become a service lifecycle controller.

## 2. Why this milestone is blocked

Hard dependencies:

- M004 owns the canonical I2PTunnel definition, ownership, backend, and status inventory.
- M005 owns bounded read-only router/service inspection, protocol listener addresses, SAM session snapshots, network-safe snapshot patterns, response budgeting, and selector-by-presence infrastructure.

M006 must consume those closed interfaces. It must not create duplicate tunnel status logic or mutable SAM/I2CP manager handles.

## 3. Current implementation evidence

At the production baseline:

- `Router::protocol_address_info()` exposes bound I2CP, SAM TCP/UDP, NTCP2, and SSU2 listener information after startup.
- `main.rs` conditionally spawns HTTP and SOCKS proxies from startup configuration when SAM is available.
- HTTP proxy startup has a one-shot readiness signal used by the runtime address-book downloader.
- SOCKS startup has no equivalent externally retained readiness snapshot.
- Client and server tunnel managers are startup-owned and do not expose a complete status/control registry.
- SAM sessions are internal to the core SAM server.
- BOB is not implemented by Emissary.
- No ClientServicesInfo method or passive fixed-size client-service registry exists.

## 4. Invariants that must not regress

- Selector names, response keys, JSON types, and nullability follow the M001 matrix exactly.
- Only requested service sections appear.
- Configuration does not automatically mean listening or active.
- A bound listener does not automatically mean a successful active session where the protocol distinguishes them.
- Unsupported M004 tunnels never appear active.
- BOB is represented exactly as unavailable/false according to M001; no BOB implementation is added.
- HTTP/SOCKS observation does not take task ownership or alter retry/startup behavior.
- SAM/I2CP inspection is read-only and bounded.
- No method call starts, stops, restarts, rebinds, or reconfigures a service.
- No frontend event/state is consulted.
- No mutable service handle enters the JSON-RPC layer.
- Listener/session failures are reported truthfully and sanitized.
- Responses are bounded and not silently truncated.
- Existing application startup/shutdown behavior remains unchanged.

## 5. Scope

### In scope

- Exact ClientServicesInfo request parsing and selector presence.
- Exact result serialization.
- Passive application-level service registry.
- HTTP proxy configured/starting/listening/failed/stopped observation.
- SOCKS proxy configured/starting/listening/failed/stopped observation.
- I2CP listener state from actual bound address information.
- SAM TCP/UDP listener state and bounded session snapshot from M005.
- I2PTunnel view from M004.
- Exact BOB unavailable value.
- Startup, failure, shutdown, restart, concurrency, authorization, and resource tests.
- Documentation and static scope guards.

### Explicitly out of scope

- Lifecycle controls for any client service.
- Restarting or supervising HTTP/SOCKS beyond current behavior.
- Dynamic proxy configuration.
- Creating SAM/I2CP sessions.
- Implementing BOB.
- Implementing missing tunnel types.
- Moving current service tasks into a new manager.
- UI changes.
- New service categories, aliases, health metadata, timestamps, capabilities, or diagnostics fields.
- Router behavior changes.

## 6. Required production changes

### Exact method and selector handling

Register `ClientServicesInfo` through the M001 method registry.

The handler must:

1. authenticate/version-check first;
2. parse exact selector presence;
3. request only selected passive snapshots;
4. enforce per-section and aggregate response budgets;
5. return only selected exact keys;
6. map unavailable/disabled/failed state exactly;
7. perform no service mutation.

Reuse M005's selector-set and response-budget patterns where possible without generalizing the public protocol.

### Passive client-service registry

Add a frontend-independent application-layer registry with a fixed set of service categories. Internal state may include:

```rust
pub enum ObservedServiceState {
    Disabled,
    Configured,
    Starting,
    Listening,
    Failed(SanitizedFailure),
    Stopping,
    Stopped,
}
```

This vocabulary is internal only. The serializer maps it to exact M001 values and must not expose new public statuses.

Registry requirements:

- fixed-size entries; no unbounded dynamic service keys;
- cloneable immutable snapshots;
- monotonic startup generation to reject stale updates;
- no task handles or cancellation authority;
- no frontend ownership;
- no secrets in failure details;
- shutdown-safe updates;
- coherent before-or-after snapshots under concurrent changes.

### HTTP proxy observation

Instrument current HTTP proxy startup passively:

- record Disabled when no config exists;
- record Configured/Starting before task construction;
- record Listening only after the actual listener and required tunnel/session startup condition represented by M001 is satisfied;
- record Failed with sanitized bounded reason on constructor/bind/runtime failure;
- record Stopped when the task exits or application shuts down;
- preserve the existing address-book one-shot readiness signal without consuming or replacing it;
- if the current one-shot occurs at the exact required readiness point, fan out the observation at its producer rather than adding a second consumer;
- retain actual bound address/port only if the exact response requires it;
- do not expose outproxy credentials or configuration internals beyond exact fields.

If current proxy code cannot distinguish bind readiness from SAM tunnel readiness, activation review must define the exact M001 semantic and add the smallest passive notification at the owning task.

### SOCKS proxy observation

Add equivalent passive observation at the actual bind/start/error/exit boundaries.

- Do not treat successful task spawn as listening.
- Do not redesign the SOCKS proxy into a supervisor.
- Do not add a command channel.
- Do not expose outproxy or sensitive configuration beyond exact response fields.
- Use the same application registry/generation semantics as HTTP.

### I2CP observation

Use the actual bound listener information produced by core router startup and M005's passive inspection.

- Disabled/unavailable when no listener is configured/bound.
- Active/listening only when the server successfully initialized and returned a local address.
- Preserve exact host/port/address representation from M001.
- Do not infer session count unless the protocol requires it and M005 exposes a truthful bounded session snapshot.
- No I2CP connection/session mutation.

### SAM observation

Use M005's core inspection handle for:

- actual TCP listener state/address;
- actual UDP listener state/address if required;
- bounded active session count/list/info exactly required by M001;
- sanitized server failure/unavailable state.

Requirements:

- no direct access to SAM mutable session maps;
- no session keys, destination private material, authentication data, or raw protocol payloads;
- session result ordering deterministic;
- full result bounded without silent truncation;
- no query starts/ends a SAM session;
- distinguish listener enabled from active sessions.

### BOB

Return the exact M001 value representing that Emissary does not provide BOB. Expected baseline is `false`, but the closed matrix is authoritative.

- Do not add a BOB listener, stub server, port, or configuration.
- Do not add explanatory extension text to the response.
- Add a fixture proving the exact type/value.

### I2PTunnel

Consume M004's production inventory interface.

The exact response may include configured definitions and/or quick runtime status according to M001. Requirements:

- control-plane-owned unsupported definitions appear configured but inactive only where the exact shape represents configuration;
- unsupported definitions never appear listening/running;
- startup-managed inventory uses M004's truthful ownership/status mapping;
- no direct persistence read;
- no backend dispatch;
- no capability/stub metadata extension;
- enforce item/byte limits without truncation.

### Composition and startup wiring

The service registry must be created in the application composition root and passed to producers/inspection adapters through narrow update handles.

Likely ownership:

```text
emissary-cli/src/i2pcontrol/
    client_services.rs
    control_plane/client_services.rs
    service_registry.rs
```

Exact placement may follow closed M001–M005 architecture.

The registry must exist independently of UI build/mode. When I2PControl is disabled, instrumentation should compile away or remain negligible passive state if shared by diagnostics.

### State mapping manifest

For each selector, record:

- exact request key;
- exact result key/type;
- internal source;
- internal-to-wire state mapping;
- disabled/configured/starting/listening/failed/stopping/stopped behavior;
- null/false/empty behavior;
- item/byte bounds;
- fixture identifier.

No state should be mapped by parsing log messages or UI strings.

### Failure and sanitization

Service failures may contain socket addresses and sanitized OS error kinds only where appropriate. They must not contain:

- tokens;
- private keys;
- destination private material;
- proxy credentials;
- complete configuration;
- arbitrary filesystem paths;
- Rust backtraces.

If the exact protocol has no failure-detail field, retain failure detail internally and map only to the defined unavailable/inactive value.

### Documentation and static guards

Document:

- exact six selectors;
- configured versus listening semantics;
- SAM listener versus session semantics;
- BOB unavailable behavior;
- I2PTunnel stub inactivity;
- response limits;
- no lifecycle control.

Add guards proving:

- no handler invokes proxy/tunnel/SAM/I2CP start/stop APIs;
- no frontend/UI imports;
- no log-message parsing for state;
- no direct M004 state-file reads;
- no mutable SAM/I2CP session handles;
- no extra service categories or public state fields;
- BOB has no implementation dependency or listener.

## 7. Ordered work packages

### Work package A — Reconcile exact service contract

Intent: freeze each service section's semantic meaning before instrumentation.

Required changes:

1. Update this plan to the M004/M005 closure head.
2. Extract exact section shapes and state meanings from M001.
3. Map each field to M004/M005 or an application service-registry producer.
4. Resolve whether configured, bound, tunnel-ready, or session-active is required per service.
5. Add exact fixtures and state mapping tables.

Acceptance evidence:

- no field uses a guessed source;
- configured/listening/session semantics are explicit;
- all six selectors have fixtures.

### Work package B — Passive fixed service registry

Intent: observe application-owned proxies without taking control.

Required changes:

1. Implement fixed internal states and generation-fenced updates.
2. Add immutable snapshots.
3. Integrate registry into application composition.
4. Instrument HTTP and SOCKS producer boundaries.
5. Add shutdown/exit/failure updates.

Acceptance evidence:

- actual bind/readiness transitions in tests;
- stale producer update rejected;
- no task handle or command authority exposed;
- current proxy behavior unchanged.

### Work package C — Core and tunnel adapters

Intent: consume closed M004/M005 state.

Required changes:

1. Adapt I2CP listener state.
2. Adapt SAM listener/session snapshots.
3. Adapt I2PTunnel inventory/status.
4. Implement exact BOB value.
5. Apply per-section bounds and sanitization.

Acceptance evidence:

- production adapter tests;
- stubs inactive;
- no direct mutable handle/store-file access;
- complete-result oversize behavior.

### Work package D — Exact handler and integration

Intent: complete the public method.

Required changes:

1. Register handler.
2. Implement selector-by-presence and only-requested sections.
3. Implement exact state serialization.
4. Add real HTTPS integration tests.
5. Add authorization, cancellation, shutdown, and frontend-independence tests.

Acceptance evidence:

- one exact fixture per selector;
- combined selector fixtures;
- no unrelated section;
- no mutation side effect.

### Work package E — Scope guards and documentation

Intent: prove observation did not become management.

Required changes:

1. Add static guards against start/stop/control calls.
2. Add no-frontend, no-log-parsing, no-direct-store guards.
3. Update support matrix and operator docs.
4. Run existing proxy/SAM/I2CP/tunnel tests.
5. Update conformance evidence and planning status.

Acceptance evidence:

- dependency/source review;
- unchanged existing behavior;
- documentation distinguishes listener/session/configuration accurately.

## 8. Failure, cancellation, restart, and contention semantics

- Invalid or unauthorized requests perform no service snapshot query beyond fixed M001 authentication handling.
- Snapshot reads are immutable and do not hold task locks during JSON serialization.
- Registry producers use startup generations so a failed/old task cannot overwrite a newer task's state.
- Task spawn success alone does not mark Listening.
- Bind/start failure becomes internal Failed and exact public unavailable/inactive behavior.
- Task exit updates state without restarting or supervising it.
- Shutdown transitions are bounded and do not delay router shutdown for API observation.
- Request cancellation drops snapshot work and releases permits; it does not affect services.
- SAM session snapshots use M005 bounded query channels and respect deadlines.
- A complete session/tunnel section exceeding limits fails explicitly; it is not truncated.
- Restart reconstructs service state from fresh startup and actual bound listeners, not persisted volatile state.
- M004 administrative definitions persist, but unsupported runtime state restarts inactive.
- Concurrent state updates and reads produce coherent before-or-after snapshots.
- Frontend startup/shutdown has no effect on service registry ownership.

## 9. Compatibility and migration

- Existing proxy, SAM, and I2CP startup configuration remains unchanged.
- Existing startup ordering remains unchanged except for passive state notifications.
- Existing HTTP address-book readiness signaling remains functional.
- Existing task error/retry/exit behavior remains unchanged.
- No persisted service-registry schema is introduced.
- M004 remains authoritative for I2PTunnel persistence and ownership.
- M005 remains authoritative for core inspection.
- Older configurations without I2PControl continue unchanged.
- Existing UI and headless modes share the same application service state source.
- BOB remains unavailable; no compatibility promise beyond exact API reporting is made.
- No public response extensions are added for internal Failed/Starting/Stopping states.

## 10. Required tests

### Focused unit tests

- exact selector parse/presence;
- exact response key/type per service;
- internal-to-wire state mapping for every internal state;
- BOB exact value;
- only-requested sections;
- deterministic ordering;
- error sanitization;
- response budgeting.

### Service registry tests

- disabled/configured/starting/listening/failed/stopping/stopped transitions;
- generation fencing;
- concurrent readers/writers;
- task exit and restart generation;
- shutdown behavior;
- no unbounded dynamic entries.

### HTTP/SOCKS integration tests

- configured but pre-bind state;
- actual bind/listening state;
- port-conflict failure;
- runtime task exit;
- address-book readiness still receives signal;
- no control action from ClientServicesInfo;
- no credential/config leakage.

### SAM/I2CP tests

- disabled listener;
- actual bound address;
- SAM TCP/UDP exact representation;
- zero and multiple SAM sessions;
- oversize session section;
- session open/close races;
- query timeout/shutdown;
- no mutable session action.

### I2PTunnel tests

- unsupported control-plane definitions configured but inactive;
- startup-managed inventory mapping;
- no backend dispatch;
- bounded list;
- exact only-requested section;
- M004 state changes reflected passively.

### End-to-end tests

- each selector alone through real HTTPS listener;
- all selectors combined within safe bounds;
- unauthorized requests;
- cancellation during SAM snapshot;
- shutdown/restart;
- headless and UI-enabled identical results;
- no frontend events consumed;
- existing proxy/SAM/I2CP/tunnel suites unchanged.

### Static/security tests

- no start/stop/restart calls from handler;
- no UI imports;
- no log parsing;
- no direct M004 persistence reads;
- no mutable SAM/I2CP handles;
- no BOB listener/dependency;
- secrets absent from snapshots/logs/errors;
- no extra service keys/statuses.

## 11. Required verification commands

The activation pass must reconcile exact targets. Expected minimum:

```bash
cargo fmt --all -- --check

cargo check -p emissary-cli --no-default-features --features i2pcontrol
cargo check -p emissary-cli --features ui,i2pcontrol

cargo test -p emissary-cli --no-default-features --features i2pcontrol i2pcontrol::client_services
cargo test -p emissary-cli --no-default-features --features i2pcontrol proxy
cargo test -p emissary-core sam
cargo test -p emissary-core i2cp

cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Also run M004 inventory/backend tests, M005 inspection/contention tests, M001 fixture validation, and supported-platform integration tests.

## 12. Documentation updates

- Add or update `docs/i2pcontrol/client-services.md`.
- Update `docs/i2pcontrol/proposal-170-support.md`.
- Update the exact conformance matrix and evidence links.
- Document configured/listening/session semantics for each service.
- Document BOB false/unavailable behavior.
- Document that unsupported I2PTunnel definitions remain inactive.
- Document response bounds and unavailable behavior.
- State that ClientServicesInfo is observation-only.
- State that no frontend control exists.

## 13. Acceptance criteria

1. M004 and M005 are strictly closed and this plan is reconciled to their reviewed head.
2. ClientServicesInfo is registered through exact M001 authentication/version handling.
3. Exactly six selector categories are accepted.
4. Only requested service sections appear.
5. Every response key/type/nullability follows M001 exactly.
6. Internal service states do not create new public status vocabulary.
7. HTTP configured state is distinguished from actual listening/readiness.
8. HTTP observation does not consume or break the existing address-book readiness signal.
9. SOCKS configured state is distinguished from actual listening.
10. Proxy task failure/exit is observed passively and sanitized.
11. No proxy lifecycle command or ownership transfer is added.
12. I2CP uses actual bound listener information.
13. SAM uses actual bound listener and bounded session information.
14. SAM snapshots expose no private/session-sensitive material.
15. BOB returns the exact unavailable/false value and no BOB implementation is added.
16. I2PTunnel consumes M004's production inventory.
17. Unsupported tunnel definitions never appear active/listening/running.
18. Startup-managed tunnel state is represented only where truthful.
19. No direct M004 persistence-file read occurs.
20. No service query starts, stops, restarts, rebinds, or reconfigures a service.
21. Registry updates are generation-fenced and stale tasks cannot overwrite current state.
22. Concurrent reads/updates produce coherent snapshots.
23. Complete oversize sections fail explicitly and are never silently truncated.
24. Errors/logs contain no credentials, private keys, complete configs, or internal paths.
25. Request cancellation and application shutdown release resources without affecting services.
26. Restart reconstructs volatile state from actual startup/listeners.
27. Headless and UI-enabled modes report equivalent service state.
28. Existing proxy/SAM/I2CP/tunnel behavior and tests remain unchanged.
29. No frontend work, BOB implementation, missing tunnel implementation, or router behavioral change is included.
30. Required protocol, integration, concurrency, security, and compatibility tests pass.

## 14. Stop conditions

The agent must stop and report rather than improvise when:

- M004 or M005 is not strictly closed;
- M001 does not define whether a field means configured, bound, tunnel-ready, or session-active;
- truthful HTTP/SOCKS readiness would require redesigning service supervision rather than a passive notification;
- SAM/I2CP state requires mutable manager ownership;
- complete results cannot fit safe bounds and no exact error behavior exists;
- a new public status/capability/diagnostic field is proposed;
- BOB implementation or stub listener is proposed;
- a missing tunnel backend would need implementation;
- service observation would need to parse logs or consume frontend events;
- implementation would change startup ordering/behavior materially;
- work expands into lifecycle control, frontend work, or router redesign.

The stop report must name the selector/field, missing semantic source, affected acceptance criteria, and smallest passive interface or ADR required.

## 15. Closure evidence required

The later closure record must include:

- dependency closure references and reconciled baseline;
- implementation commits and reviewed head;
- requirement-to-evidence mapping for all acceptance criteria;
- exact six-selector fixture evidence;
- only-requested-section evidence;
- service-state mapping review;
- HTTP/SOCKS actual readiness/failure/exit evidence;
- existing address-book readiness compatibility evidence;
- I2CP listener evidence;
- SAM listener/session/bounds/no-secret evidence;
- BOB exact value and no-implementation evidence;
- M004 I2PTunnel inactive-stub evidence;
- no-control static/source guards;
- cancellation/shutdown/restart/concurrency evidence;
- headless/UI equivalence evidence;
- exact commands/platform outcomes;
- unrun limitations;
- unresolved findings by severity;
- roadmap and registry disposition.

Closure must be `corrective pass required` if any of these remains:

- missing/extra selector or field;
- configured state reported as active without evidence;
- stubbed tunnel reported active;
- mutable service ownership/control added;
- broken HTTP readiness signal;
- direct store read or log parsing;
- BOB implementation added;
- secret/session-sensitive data exposed;
- silent truncation;
- frontend coupling;
- unresolved high/medium truthfulness, protocol, security, or lifecycle finding.

## 16. Handoff notes

- Reconcile this blocked plan only after M004/M005 strict closure.
- Observe producer boundaries; do not infer state from task spawn or logs.
- Keep the service registry fixed-size and passive.
- Do not duplicate M004/M005 data models.
- Do not add management actions for convenience.
- Treat BOB as an exact constant result, not a stub project.
- Use ephemeral ports and isolated runtime state in integration tests.
- Preserve the HTTP address-book readiness path exactly.
- The implementation pass moves registry status to `closing`, not `closed`; independent closure remains required.
