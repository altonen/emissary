# I2PControl Proposal 170 Milestone 001 — Contract Matrix and I2PControl Foundation

Status: closing

Repository production baseline: `9b43484a21d5a1291c4881cdae62a36c527f8c0f` (`master`)

The planning-scaffold commit containing this plan changes documentation only. The implementation agent MUST inspect current `master` before editing and record any production-code drift from the baseline.

Source roadmap:

- `plans/subsystems/i2pcontrol-proposal-170-roadmap.md#milestone-001--contract-matrix-and-i2pcontrol-foundation`

Canonical requirements:

- `plans/000-long-term-specification.md#2-normative-external-references`
- `plans/000-long-term-specification.md#3-scope-boundary`
- `plans/000-long-term-specification.md#4-architectural-invariants`
- `plans/000-long-term-specification.md#5-protocol-exactness`
- `plans/000-long-term-specification.md#11-security-and-resource-bounds`
- `plans/001-terminology-and-domain-model.md#1-api-and-protocol-terms`
- `plans/002-long-term-roadmap.md#milestone-m001--contract-matrix-and-i2pcontrol-foundation`

Applicable ADRs:

- `plans/adrs/ADR-0001-proposal-170-contract-and-stub-boundary.md`

Primary class: invariant / infrastructure

## 1. Objective

Establish a production-shaped, frontend-independent I2PControl foundation and freeze the exact Proposal 170 contract into a reviewable conformance matrix and test fixtures.

At milestone completion, Emissary must have:

- an independently feature-gated HTTPS JSON-RPC listener;
- exact base I2PControl authentication and API-version behavior required to expose Proposal 170;
- bounded request parsing and exact JSON-RPC result/error envelopes;
- a typed method-dispatch boundary and fake control plane suitable for later method implementation;
- startup, shutdown, configuration, security, and integration tests;
- a complete inventory of every Proposal 170 method, selector, parameter, action, tunnel type, JSON type, nullability rule, validation rule, data source, and expected milestone owner.

This milestone does not implement AddressBook, TunnelManager, Proposal 170 RouterInfo selectors, ClientServicesInfo, persistent administrative state, router inspection, or tunnel backends. It creates the stable boundary those milestones consume.

## 2. Why this milestone is ready

- No production dependency must close before the server and contract boundary can be established.
- The maintainers have explicitly selected contract-complete Proposal 170 support with unsupported tunnel runtime backends, recorded in ADR-0001.
- Proposal 170 and the existing I2PControl API provide the external contract source.
- Emissary already has an async application lifecycle, CLI configuration, cancellation/shutdown flow, tracing, and optional Axum/Serde JSON dependencies that can be separated from frontend ownership.
- The milestone can be completed without changing router behavior, exposing core internals, or deciding missing tunnel runtime designs.
- Later milestones can proceed against a typed fake control plane after this milestone closes.

## 3. Current implementation evidence

At the production baseline:

### Workspace and dependency ownership

- `Cargo.toml` defines `emissary-cli` as the default workspace member and keeps router implementation in `emissary-core`.
- `emissary-cli/Cargo.toml` declares Axum as optional with WebSocket/macros/Tokio/HTTP1 features.
- `serde_json` is optional.
- the current `ui` feature enables Axum through Dioxus liveview integration.
- there is no independent `i2pcontrol` Cargo feature.

Risk: reusing the existing feature relationship without correction would make the administrative API frontend-dependent or make headless builds unexpectedly pull the full UI.

### Application lifecycle

- `emissary-cli/src/main.rs` owns application startup, constructs the router and supporting managers, builds `RouterContext`, and branches into UI or non-UI execution.
- there is no I2PControl service task or control-plane context.
- application shutdown already has a central cancellation path that the new listener should join rather than replace.

Risk: starting the listener inside the UI branch would violate frontend independence; starting it without structured cancellation could leak a bound socket or delay shutdown.

### Configuration

- `emissary-cli/src/config.rs` parses the existing Emissary configuration and preserves configured client/server tunnels, proxies, router settings, and address-book settings.
- there is no I2PControl configuration block, TLS material management, or authentication credential configuration.

Risk: insecure defaults, a plaintext listener, or implicit external binding would create a security regression.

### Router and event ownership

- `Router` exposes only a limited public management surface.
- `RouterContext` currently retains router identity, configuration, address-book handle, port mapper, and a frontend-oriented event subscriber.
- the event subscriber is a single receiver rather than an independent shared snapshot API.

This milestone must not broaden core inspection or consume frontend events. It should define interfaces only.

### Protocol implementation

- no I2PControl module, JSON-RPC DTOs, method registry, token service, or protocol conformance fixtures exist.
- no tests exercise an I2PControl HTTPS listener.

## 4. Invariants that must not regress

- No I2P wire protocol, router algorithm, NetDB behavior, transport behavior, tunnel behavior, or destination behavior changes.
- No frontend source file is required to implement, start, stop, authenticate, or test I2PControl.
- Headless builds can compile with I2PControl enabled and the UI disabled.
- UI builds can compile with I2PControl disabled.
- I2PControl does not own or drain the existing frontend event subscriber.
- The listener binds only to configured addresses and uses secure defaults.
- Plaintext HTTP is not silently exposed as I2PControl compatibility.
- Only `Authenticate` bypasses token authentication, according to the base I2PControl contract.
- Request IDs and JSON-RPC 2.0 envelopes are preserved exactly.
- Protocol errors are distinct from future method-defined operation statuses.
- Proposal 170 names and types are recorded exactly; no aliases or extensions are introduced.
- No future feature method returns a fabricated success or placeholder payload in this milestone.
- Request work, body size, token storage, connection handling, and response construction are bounded.
- Credentials, tokens, private keys, and TLS private-key material are never written to logs.
- Existing configuration continues to parse unchanged when the new block is absent.
- Existing UI and non-UI startup behavior remains unchanged when I2PControl is disabled.

## 5. Scope

### In scope

- Exact source-of-truth capture for Proposal 170 and required base I2PControl behavior.
- A field/method/action/tunnel-type conformance matrix.
- Machine-readable or compile-time fixture inventory derived from that matrix.
- Independent Cargo feature ownership for I2PControl.
- Additive I2PControl configuration.
- TLS listener infrastructure compatible with I2PControl's HTTPS endpoint expectations.
- Authentication request/result DTOs, API version validation, password verification, token issuance, token lookup, and restart behavior.
- JSON-RPC 2.0 request, success, and error DTOs.
- Exact parsing and dispatch for single requests.
- Bounded body handling and rejection of unsupported batch requests unless the base contract explicitly requires them.
- Typed method-name representation and a registry that can be extended by later milestones.
- A minimal control-plane trait or service boundary plus a fake implementation for tests.
- Application startup and shutdown integration independent of frontend mode.
- Focused protocol, TLS, authentication, configuration, lifecycle, and negative tests.
- Operator documentation for enabling the incomplete foundation in development, clearly stating that Proposal 170 feature methods are not yet complete.

### Explicitly out of scope

- Implementing Proposal 170 AddressBook behavior.
- Implementing Proposal 170 TunnelManager behavior.
- Defining full tunnel persistence or tunnel option models.
- Implementing unsupported tunnel backends; that belongs to M002/M004.
- Implementing Proposal 170 RouterInfo selectors.
- Adding read-only router inspection.
- Adding shared metrics snapshots or readable logs.
- Implementing ClientServicesInfo.
- Implementing missing tunnel data planes.
- Migrating existing startup-managed tunnel lifecycle ownership.
- Changing address-book runtime behavior.
- Adding frontend controls, screens, status indicators, or frontend state.
- Binding I2PControl to non-loopback by default.
- Adding non-standard health, capabilities, discovery, pagination, or debug methods.
- Treating M001 infrastructure as completed Proposal 170 capability.

## 6. Required production changes

### 6.1 Contract inventory and source discipline

Create a durable conformance document, expected at:

```text
docs/i2pcontrol/proposal-170-conformance.md
```

It MUST contain a row for every:

- Proposal 170 method;
- RouterInfo selector;
- AddressBook request mode and response field;
- TunnelManager action;
- TunnelManager `All` rule;
- declared tunnel type;
- ClientServicesInfo selector;
- base I2PControl authentication/version requirement used by the extension;
- JSON-RPC error or operation-status distinction relevant to implementation.

Recommended columns:

| Contract item | Request key/type | Required/optional/presence | Response key/type | Nullability | Validation/error behavior | Planned data source | Owner milestone | Fixture/test ID | Notes |
|---|---|---|---|---|---|---|---|---|---|

Rules:

- copy external field spelling exactly;
- do not normalize names in the canonical column;
- distinguish boolean value semantics from parameter-presence semantics;
- identify whether a result is a JSON-RPC error, normal result, or textual operation status;
- identify proposal ambiguities explicitly;
- resolve envelope behavior against the established JSON-RPC/I2PControl contract;
- do not use the unmerged Java implementation as normative authority, although it may be cited as compatibility evidence;
- assign every item to M003, M004, M005, or M006 after M001 foundation ownership.

Add a machine-checkable inventory. Acceptable approaches include:

- Rust constants/enums with a test that compares expected names;
- JSON fixtures under `emissary-cli/tests/fixtures/i2pcontrol/`;
- a small checked-in manifest consumed by tests.

Do not write a generator that fetches network content during tests.

### 6.2 Cargo feature and dependency separation

Introduce an `i2pcontrol` feature in `emissary-cli/Cargo.toml` that is independent of `ui`.

Required build relationships:

```text
--no-default-features
    -> builds headless Emissary without UI or I2PControl

--no-default-features --features i2pcontrol
    -> builds headless Emissary with I2PControl

--no-default-features --features ui
    -> builds UI without requiring I2PControl

--all-features
    -> builds UI and I2PControl together without duplicate/conflicting Axum ownership
```

Axum may be shared as an optional dependency by both features. UI-only dependencies must not become dependencies of `i2pcontrol`.

Add only dependencies required for exact HTTPS, JSON-RPC, authentication, and secure token behavior. Prefer Rustls-compatible, maintained primitives and avoid introducing a general web framework beyond the existing Axum boundary.

The implementation MUST justify any new dependency in the closure record, including feature flags and whether it enters `emissary-core` or only `emissary-cli`. No HTTP/TLS/JSON dependency may be added to `emissary-core` for M001.

Default-feature policy:

- preserve the current user-visible default build unless maintainers have explicitly directed otherwise;
- do not silently enable a new listener in existing installations;
- the new service MUST remain disabled by configuration unless explicitly enabled, even when compiled.

### 6.3 Additive configuration

Add a typed I2PControl configuration section using existing configuration conventions. A representative shape is:

```toml
[i2pcontrol]
enabled = false
bind = "127.0.0.1:7650"
password = "..."
# Optional explicit TLS certificate/key paths; otherwise use managed state.
# certificate = "/path/to/cert.pem"
# private_key = "/path/to/key.pem"
```

The exact field names must be documented and stable before code lands.

Required semantics:

- absence of the block preserves current startup behavior;
- default `enabled` is false;
- default bind is loopback only;
- wildcard or non-loopback binding is permitted only through explicit configuration and must produce a security warning without logging credentials;
- enabled operation requires a non-empty credential source;
- malformed bind addresses, unreadable TLS material, invalid credentials configuration, and conflicting TLS modes fail startup with actionable sanitized errors;
- configuration parsing does not open the listener;
- secrets are not included in Debug output or configuration diagnostics;
- paths are resolved using established base-path/config rules and not relative to attacker-controlled request data.

Do not invent token-lifetime, plaintext mode, alternate auth, or remote-access configuration that is not required by the base API or secure operation.

### 6.4 TLS lifecycle

I2PControl must be served over HTTPS according to the established API contract.

Support one secure, deterministic certificate path:

1. operator-provided certificate and private key; or
2. managed self-signed certificate and private key under the Emissary base path when explicit files are absent.

If managed generation is implemented:

- generate only when I2PControl is enabled;
- write atomically;
- do not regenerate on every start;
- restrict private-key permissions where the platform supports it;
- validate existing material before binding;
- fail closed on malformed, mismatched, or insecurely inaccessible material;
- never log private-key contents;
- document certificate location and rotation behavior;
- add restart tests proving certificate identity is stable.

Do not add plaintext fallback after TLS initialization failure.

If exact compatibility requires a certificate behavior not safely resolvable from normative sources, stop and record the ambiguity rather than shipping an insecure guess.

### 6.5 JSON-RPC domain

Add an application-layer module, expected to begin as:

```text
emissary-cli/src/i2pcontrol/
    mod.rs
    server.rs
    auth.rs
    rpc.rs
    errors.rs
    control_plane.rs
```

Equivalent decomposition is allowed if ownership remains clear.

Define typed DTOs for:

- JSON-RPC request with exact `jsonrpc`, `method`, named `params`, and `id` handling;
- success response;
- error response;
- error object with exact code/message/data policy;
- authentication parameters and result;
- API-version representation;
- opaque authentication token.

Requirements:

- require `jsonrpc` to be exactly `2.0` where the base contract requires it;
- preserve string or numeric request IDs supported by JSON-RPC;
- decide and test null-ID/notification behavior against the base I2PControl contract;
- reject positional parameters if I2PControl requires named parameters;
- reject malformed top-level values and unsupported batches with the correct protocol error;
- distinguish absent params from an empty object;
- avoid deserializing arbitrary method params into unbounded nested values before body/nesting controls apply;
- sanitize internal errors;
- serialize no unrelated fields.

Do not model future Proposal 170 results as generic success placeholders. Later milestones should add exact result DTOs or validated serializers.

### 6.6 Authentication and API version

Implement the exact `Authenticate` method required by I2PControl.

The contract matrix must freeze:

- exact method name;
- parameter names and types;
- accepted API version behavior;
- password failure behavior;
- result keys and types;
- token representation;
- relevant error codes.

Security requirements:

- compare configured credentials using a timing-resistant mechanism appropriate to the chosen representation;
- issue tokens using a cryptographically secure random source;
- store only the minimum token state required;
- tokens are opaque and never logged;
- token lookup is bounded;
- tokens are invalidated on process restart unless normative compatibility requires durable tokens;
- no method other than Authenticate can execute before token verification;
- authentication failure responses do not reveal whether version, password, or token storage details differ beyond the protocol-required error;
- concurrent authentication requests cannot create unbounded retained token state.

Do not add refresh tokens, bearer headers, cookies, alternate passwords, or public expiration metadata.

If the base API does not define token expiry, do not invent a wire-visible expiry contract. Internal cleanup may be bounded only if it preserves expected client behavior and is documented.

### 6.7 Method registry and control-plane boundary

Define a typed method registry or dispatcher that:

- has one canonical external string per method;
- routes Authenticate before token validation only as allowed;
- validates tokens before future protected handlers;
- can accept exact per-method parameter/result types later;
- does not require changes to server/TLS code when methods are added;
- returns the correct unknown/unavailable method error for methods not yet implemented;
- records no sensitive params in logs.

Define a minimal control-plane boundary used by future handlers. It should expose no mutable router handles in M001.

An acceptable initial shape is a trait or service object with deliberately empty/future-facing typed subinterfaces, plus a fake implementation used to test dispatcher ownership. Avoid designing complete RouterInfo, AddressBook, or TunnelManager domain types before M002.

Do not register placeholder handlers that return zero-filled or successful Proposal 170 payloads.

The contract inventory may define exact method constants now. Runtime dispatch should expose only methods with real milestone-owned behavior, returning the established method-not-found/unavailable behavior for the rest until implemented.

### 6.8 Server lifecycle and application integration

Integrate the server into `emissary-cli` outside the UI/no-UI branch.

Required lifecycle:

1. parse and validate configuration;
2. initialize TLS/auth state only when enabled;
3. bind the configured listener before reporting startup success;
4. spawn the server under structured application ownership;
5. run UI or headless application behavior independently;
6. receive the existing shutdown signal/cancellation;
7. stop accepting new requests;
8. allow a bounded graceful shutdown window for active requests;
9. release the socket and join the task;
10. surface startup or unexpected server failure to the application rather than silently disabling control.

Do not detach the server task.

Do not store it in frontend state.

Do not move the existing event subscriber into the I2PControl context.

If the server fails after successful startup, define whether the application fails closed or reports a fatal service error. The behavior must be deterministic and tested; silently losing the administrative listener is not acceptable when it was explicitly enabled.

### 6.9 Request and resource bounds

Implement explicit limits for:

- HTTP request body size;
- header size through server configuration where available;
- request read timeout;
- handler timeout;
- maximum concurrent in-flight requests;
- authentication token store size or cleanup policy;
- nesting/recursion depth if generic JSON values remain in the parser;
- error message length;
- graceful shutdown duration.

Use conservative documented defaults. Limits are implementation safeguards, not new protocol fields.

Return protocol-compatible errors or HTTP rejection behavior without panicking or allocating unbounded memory.

### 6.10 Documentation and static guards

Create or update:

```text
docs/i2pcontrol/README.md
docs/i2pcontrol/proposal-170-conformance.md
```

The README must explain:

- compile feature and runtime enablement;
- HTTPS certificate behavior;
- authentication setup;
- loopback default;
- current milestone status;
- that M001 does not complete Proposal 170 feature methods;
- that no frontend controls exist;
- how later support status will be represented.

Add static or source-level guards where practical to ensure:

- `emissary-core` does not gain Axum, HTTP server, TLS server, or JSON-RPC dependencies;
- `i2pcontrol` does not import UI modules;
- UI feature disablement does not disable I2PControl compilation;
- future protected handlers cannot bypass the common token gate;
- no placeholder Proposal 170 success response is registered.

## 7. Ordered work packages

### Work package A — Freeze external contracts

Intent: remove ambiguity before server behavior is implemented.

Required changes:

1. Read Proposal 170 and the base I2PControl API from normative sources.
2. Create the complete conformance matrix.
3. Record every exact external name and JSON type.
4. Classify envelope, authentication, validation, selector-presence, and operation-status semantics.
5. Identify explicit proposal ambiguities and their compatibility resolution.
6. Create a machine-checkable inventory and fixture naming convention.
7. Cross-check every Proposal 170 tunnel type against ADR-0001.

Acceptance evidence:

- review script or test proves all expected methods/selectors/actions/types exist once;
- no row has an empty owner milestone or data-source classification;
- AddressBook Delete presence semantics and TunnelManager `All` restrictions are explicit;
- malformed proposal examples are documented without altering the JSON-RPC envelope.

### Work package B — Separate feature and configuration ownership

Intent: make I2PControl independently buildable and disabled by secure runtime default.

Required changes:

1. Add the independent Cargo feature.
2. Separate shared Axum dependency activation from UI-only dependencies.
3. Add typed additive configuration.
4. Redact secrets from Debug/diagnostics.
5. Validate bind and credential settings.
6. Add compile/configuration tests for every feature combination.

Acceptance evidence:

- the four build combinations listed in section 6.2 compile;
- old configuration fixtures parse unchanged;
- enabled configuration without credentials fails before binding;
- non-loopback configuration is explicit and warned;
- no UI crate/module is required by I2PControl.

### Work package C — Build TLS and structured server lifecycle

Intent: create a real HTTPS service owned by the application lifecycle.

Required changes:

1. Implement operator-provided and/or managed TLS material according to section 6.4.
2. Bind only after validation.
3. Start the listener outside frontend branches.
4. Add structured cancellation and bounded graceful shutdown.
5. Surface bind, TLS, and unexpected server failures deterministically.
6. Add startup/shutdown diagnostics without secrets.

Acceptance evidence:

- an HTTPS client completes a request against the test listener;
- plaintext connection cannot execute JSON-RPC;
- restart reuses valid managed certificate material;
- port conflict produces startup failure;
- shutdown releases the port and leaves no detached task;
- identical behavior occurs with UI disabled and enabled test configurations.

### Work package D — Implement exact JSON-RPC core

Intent: make request and response behavior independently testable before feature methods land.

Required changes:

1. Define request, ID, params, success, error, and error-code DTOs.
2. Add bounded parsing and supported top-level shape checks.
3. Implement dispatcher and unknown-method behavior.
4. Preserve request IDs.
5. Sanitize internal failures.
6. Add table-driven protocol fixtures.

Acceptance evidence:

- valid requests receive exact envelopes;
- malformed JSON, invalid request objects, wrong version, positional params, unknown methods, and unsupported batches produce expected errors;
- fuzz/property-style malformed values do not panic;
- unrelated response keys are absent.

### Work package E — Implement authentication and common authorization gate

Intent: guarantee every future method inherits one correct authentication boundary.

Required changes:

1. Implement exact Authenticate params/results and API-version validation.
2. Add secure token issuance and bounded storage.
3. Add common token extraction/validation for all protected dispatch.
4. Ensure logs and errors redact credentials/tokens.
5. Define restart invalidation and concurrency behavior.

Acceptance evidence:

- Authenticate succeeds with exact valid inputs;
- wrong password/version/type/missing fields fail exactly;
- every protected test method fails without a token and succeeds with a valid token;
- tokens from a prior server instance fail after restart;
- concurrent authentication respects token-store bounds;
- log capture contains no password or token bytes.

### Work package F — Establish control-plane and future method boundary

Intent: ensure later milestones add domain behavior without replacing transport/auth/server code.

Required changes:

1. Define typed method identifiers and handler registration.
2. Define minimal control-plane interfaces without mutable core handles.
3. Add fake/test control plane.
4. Prove token gating occurs before handler invocation.
5. Define how later handlers provide exact method-specific DTOs.
6. Keep unimplemented Proposal 170 methods explicitly unavailable.

Acceptance evidence:

- a fake protected method proves auth-before-handler ordering;
- a handler panic/error is sanitized and does not terminate the server;
- adding a test method requires no TLS/server changes;
- future method names are inventoried but no placeholder success response exists.

### Work package G — Harden, document, and prepare closure

Intent: leave a reviewable foundation rather than a prototype.

Required changes:

1. Add request, concurrency, timeout, token, and shutdown bounds.
2. Add negative and lifecycle tests.
3. Add dependency and frontend-coupling guards.
4. Complete operator and conformance docs.
5. Update the roadmap and registry to `closing` only after production work lands.
6. Create no closure record during the implementation pass unless the repository workflow explicitly assigns closure to the same agent; independent review is preferred.

Acceptance evidence:

- all required commands are recorded with actual outcomes;
- security-sensitive logs are reviewed;
- docs state incomplete feature-method status accurately;
- no production file outside the necessary CLI/config/test/docs boundary changed without justification.

## 8. Failure, cancellation, restart, and contention semantics

### Configuration and startup

- Disabled I2PControl performs no TLS generation, credential initialization, bind, or background task creation.
- Invalid enabled configuration fails before the router is reported fully started.
- TLS material failure fails closed; no plaintext fallback is allowed.
- Bind failure is surfaced as startup failure.
- Partial managed-certificate generation must leave either the prior valid pair or recoverable temporary files, not a half-active credential pair.

### Request failure

- Malformed HTTP or oversized bodies are rejected before JSON-RPC handler execution.
- Malformed JSON and invalid JSON-RPC requests return exact bounded errors where a response is permitted.
- Internal handler errors are sanitized and do not terminate the listener.
- Client disconnect cancels or abandons request work without retaining permits indefinitely.
- Handler timeout releases concurrency permits and does not invalidate unrelated tokens.

### Authentication contention

- Concurrent valid Authenticate calls may produce distinct tokens but cannot grow memory without a configured bound or safe cleanup policy.
- Concurrent invalid authentication performs bounded work.
- Credential comparison does not hold a global async lock across expensive I/O.
- Token lookup must not expose a race where a rejected token invokes a handler.

### Shutdown

- Cancellation stops admission before waiting for active requests.
- Graceful shutdown is bounded.
- Requests exceeding the bound are cancelled or terminated without leaving tasks or sockets.
- TLS and token state are dropped after listener completion.
- A restarted process does not accept prior-process tokens.

### Unexpected listener failure

- If I2PControl is enabled and the listener exits unexpectedly, the application must surface a fatal or explicitly supervised failure; it must not silently continue while claiming the service is enabled.
- Rebinding loops or automatic retries are out of scope unless existing application supervision provides a standard bounded mechanism.

### Duplicate requests

- Authenticate is intentionally non-idempotent with respect to token identity but semantically repeatable.
- M001 defines no mutating Proposal 170 methods, so request idempotency for AddressBook/TunnelManager is deferred.
- Request IDs are correlation values and must not be treated as global deduplication keys.

## 9. Compatibility and migration

- Existing configuration files without `[i2pcontrol]` remain valid and preserve prior behavior.
- Existing default builds retain their current UI behavior.
- No existing port is rebound or repurposed.
- No core storage format changes.
- No existing tunnel/address-book configuration migrates.
- The service is disabled by runtime default, preventing an unrequested new listener.
- The endpoint uses the established I2PControl method and JSON-RPC conventions; no Emissary-specific wrapper is added.
- Authentication tokens are not persisted unless normative compatibility explicitly requires it; process-restart invalidation is the default secure interpretation.
- If later milestones require DTO evolution, they must add method-specific exact types behind the stable dispatcher rather than altering the base envelope.
- Removal criteria for any temporary test method: test-only registration must be gated to tests and absent from production method inventory.

## 10. Required tests

### 10.1 Focused unit tests

- exact JSON-RPC request/response serialization;
- request-ID variants;
- named-params validation;
- error-code and message mapping;
- Authenticate parameter/result serialization;
- API-version acceptance and rejection;
- password validation and redaction;
- token generation, equality/lookup, invalidation, and bounds;
- configuration defaults, valid cases, and invalid cases;
- bind-address classification;
- TLS path validation and managed-material state transitions;
- method registry uniqueness;
- conformance inventory uniqueness and completeness.

### 10.2 Integration tests

- real HTTPS listener Authenticate round trip;
- protected fake method without token, bad token, and valid token;
- wrong JSON-RPC version;
- malformed JSON;
- invalid top-level shape;
- positional params rejection;
- unknown method;
- oversized body;
- plaintext connection rejection;
- port-conflict startup failure;
- explicit non-loopback configuration behavior;
- UI-disabled I2PControl build and listener test;
- UI-enabled and I2PControl-enabled coexistence test where feasible without launching a graphical frontend.

### 10.3 Restart and recovery tests

- managed TLS material remains stable across two server instances;
- interrupted certificate/key write recovery;
- prior-process token rejected after restart;
- shutdown releases the port for immediate restart;
- malformed existing TLS material fails closed;
- disabled startup leaves no state files.

### 10.4 Contention and cancellation tests

- concurrent Authenticate requests respect bounds;
- concurrent protected requests respect in-flight limits;
- slow request plus shutdown completes within the graceful bound;
- client disconnect releases handler resources;
- handler timeout releases permit;
- listener task failure is surfaced.

### 10.5 Security and negative tests

- password and token absent from tracing capture;
- private-key contents absent from errors and logs;
- empty password rejected when enabled;
- wildcard binding requires explicit configuration and warning;
- invalid certificate/key mismatch rejected;
- oversized/nested JSON fails boundedly;
- malformed UTF-8/body behavior is controlled;
- unknown method cannot bypass token validation if protected dispatch order requires authentication before lookup;
- Authenticate is the only unauthenticated production method.

### 10.6 Migration and compatibility tests

- representative old configuration fixture parses unchanged;
- default runtime behavior creates no listener;
- feature combinations compile;
- exact base envelope fixtures remain stable;
- no `emissary-core` dependency graph change introduces HTTP/TLS/JSON server dependencies.

## 11. Required verification commands

The implementation agent must adjust exact test filters to landed module names but preserve the intent.

```bash
# Formatting
cargo fmt --all -- --check

# Feature-boundary compilation
cargo check -p emissary-cli --no-default-features
cargo check -p emissary-cli --no-default-features --features i2pcontrol
cargo check -p emissary-cli --no-default-features --features ui
cargo check -p emissary-cli --all-features

# Focused protocol/auth/config tests
cargo test -p emissary-cli --no-default-features --features i2pcontrol i2pcontrol

# Default and all-feature CLI tests
cargo test -p emissary-cli
cargo test -p emissary-cli --all-features

# Lint focused changed targets
cargo clippy -p emissary-cli --no-default-features --features i2pcontrol --all-targets -- -D warnings
cargo clippy -p emissary-cli --all-features --all-targets -- -D warnings

# Broad workspace regression
cargo test --workspace
```

If the repository baseline contains unrelated pre-existing failures, the closure record must show:

- the exact failing command and output summary;
- a narrower command proving the changed surface;
- evidence that the failure predates the implementation;
- no false claim that the broad suite passed.

Recommended static checks:

```bash
# No I2PControl dependency on UI modules
rg -n "crate::ui|mod ui|dioxus" emissary-cli/src/i2pcontrol emissary-cli/tests

# No server-stack dependency introduced into core manifests
rg -n "axum|hyper|rustls|tokio-rustls|serde_json" emissary-core/Cargo.toml emissary-core/src

# Review all I2PControl external names for accidental extensions
rg -n "I2PControl|Authenticate|RouterInfo|AddressBook|TunnelManager|ClientServicesInfo" emissary-cli/src docs/i2pcontrol
```

The first command may find intentional test prose; every hit must be reviewed rather than blindly requiring zero output.

## 12. Documentation updates

Required:

- `docs/i2pcontrol/README.md`;
- `docs/i2pcontrol/proposal-170-conformance.md`;
- configuration reference for `[i2pcontrol]`;
- TLS certificate generation/provisioning and rotation behavior;
- authentication and secure binding guidance;
- explicit M001 support statement;
- roadmap status and registry transition after implementation;
- dependency rationale for new crates/features.

The support statement after M001 must say, in substance:

> The I2PControl transport, authentication, and JSON-RPC foundation is implemented. Proposal 170 feature methods remain under staged implementation and must not yet be described as complete.

Do not publish a tunnel support matrix as complete in M001.

## 13. Acceptance criteria

M001 is implementation-complete only when all of the following are true:

1. `docs/i2pcontrol/proposal-170-conformance.md` contains every Proposal 170 method, selector, action, tunnel type, key, type, nullability/validation rule, data source, owner milestone, and fixture ID.
2. A machine-checkable inventory fails if an expected external item is missing or duplicated.
3. `emissary-cli` has an `i2pcontrol` feature independent from `ui`.
4. All required feature combinations compile.
5. Existing configuration without an I2PControl block preserves prior behavior.
6. Runtime default is disabled and opens no listener or state.
7. Explicitly enabled configuration uses HTTPS and fails closed if TLS or credentials are invalid.
8. The listener defaults to loopback and non-loopback binding requires explicit configuration.
9. Authenticate matches the exact base I2PControl parameter, result, version, and error contract.
10. Every production method except Authenticate is protected by the common token gate.
11. Tokens are cryptographically random, opaque, bounded, redacted, and invalid after restart unless the normative API requires otherwise.
12. JSON-RPC requests preserve IDs and exact result/error envelopes.
13. Malformed, oversized, unsupported, or unauthorized requests fail boundedly without panic.
14. The server starts outside frontend branches and runs in headless mode.
15. UI and I2PControl can compile together without ownership conflict.
16. Shutdown is structured, bounded, releases the port, and leaves no detached task.
17. Unexpected enabled-listener failure is surfaced deterministically.
18. No Proposal 170 feature method returns fabricated success or placeholder data.
19. No core router behavior or dependency boundary changes.
20. No frontend behavior or source dependency is added.
21. Required focused tests pass and broad-suite results are recorded honestly.
22. Documentation states the exact incomplete capability status after M001.

## 14. Stop conditions

The implementation agent must stop and report rather than improvise when:

- normative I2PControl sources conflict materially on Authenticate, API version, token, TLS, or JSON-RPC behavior and the conflict cannot be resolved by established compatibility rules;
- exact HTTPS compatibility requires a certificate model that would add insecure defaults or unbounded key management;
- implementing the foundation would require an HTTP/JSON/TLS server dependency in `emissary-core`;
- startup integration would require moving or duplicating frontend-owned event consumption;
- the plan would require implementing RouterInfo, AddressBook, TunnelManager, ClientServicesInfo, tunnel data planes, or runtime address-book behavior;
- existing application cancellation cannot safely own the server without a broader lifecycle redesign;
- a new public configuration or protocol extension appears necessary;
- current production code has materially diverged from the baseline such that the roadmap ownership boundary is no longer accurate;
- required external compatibility evidence is unavailable and would otherwise be guessed.

A stop report must identify the exact conflict, affected acceptance criteria, smallest decision needed, and whether an ADR or roadmap revision is required.

## 15. Closure evidence required

The later closure record must include:

- implementation commit(s) and reviewed head SHA;
- requirement-to-evidence mapping for all 22 acceptance criteria;
- completed conformance matrix review and machine-check output;
- exact feature-combination build commands and outcomes;
- exact focused and broad test commands and outcomes;
- HTTPS packet/client evidence showing plaintext is not accepted;
- authentication success/failure/token-restart evidence;
- log-redaction evidence;
- startup, port-conflict, unexpected-failure, graceful-shutdown, and immediate-restart evidence;
- old-configuration compatibility evidence;
- dependency diff proving no server stack entered `emissary-core`;
- source review proving no frontend coupling and no placeholder Proposal 170 success handlers;
- documented unrun tests or environmental limitations;
- unresolved findings by severity;
- roadmap and registry disposition.

Closure must be `corrective pass required` if any of the following remains:

- plaintext fallback;
- unprotected production method other than Authenticate;
- listener coupled to UI mode;
- token or credential leakage;
- unbounded request/token/task behavior;
- fabricated Proposal 170 result;
- core server-stack dependency;
- missing conformance row;
- unresolved high/medium compatibility ambiguity.

## 16. Handoff notes

- Preserve unrelated upstream work; Emissary is an active repository.
- Re-check `master` before edits and after implementation.
- Keep production changes concentrated in `emissary-cli`, configuration, tests, and documentation.
- Prefer small typed modules over one monolithic server file.
- Do not over-design M002 domain types in M001.
- Do not register fake Proposal 170 feature success handlers to make integration tests convenient.
- Test listeners should use ephemeral ports except tests that explicitly verify configured port behavior.
- TLS and port tests may need serial execution when they share managed state directories; use isolated temporary directories wherever possible.
- Never include real credentials or generated private keys in fixtures.
- Treat official external documentation as contract input and checked-in fixtures as test input; tests must not require network access.
- The implementation pass should update the registry to `closing`, not `closed`; closure requires evidence review.
