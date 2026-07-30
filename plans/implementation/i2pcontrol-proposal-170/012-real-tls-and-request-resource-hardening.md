# I2PControl Proposal 170 Milestone 012 — Real TLS and Request Resource Hardening

Status: closed

Planning baseline: `b35d9ad7295f6d7d8795a741c7942a5ff7a97f52` (`master` before corrective planning commits)

Activation rule:

- M008 and M009 must have closure records with disposition `closed`.
- M010 and M011 may still be active, but M012 must reconcile its server-state constructor and test harness against their current reviewed interfaces before editing.
- M013 remains blocked until M010, M011, and M012 are all strictly closed.

Source roadmap:

- `plans/subsystems/i2pcontrol-proposal-170-roadmap.md#milestone-012--real-tls-and-request-resource-hardening`

Corrects:

- M001 listener/security implementation gaps;
- M007 adversarial/resource evidence gaps;
- `plans/closure/i2pcontrol-proposal-170/007-closure.md` claims regarding HTTPS-only serving and bounded request handling.

Canonical requirements:

- `plans/000-long-term-specification.md`
- `plans/001-terminology-and-domain-model.md`
- `plans/002-long-term-roadmap.md`
- `plans/003-planning-process.md`
- `plans/adrs/ADR-0001-proposal-170-contract-and-stub-boundary.md`
- base I2PControl authentication and JSON-RPC compatibility

Primary class: security / invariant corrective pass

## 1. Objective

Serve I2PControl over actual TLS, enforce request and connection resource limits before unbounded buffering or expensive parsing, replace tautological adversarial tests with concrete production-listener assertions, and produce credible security evidence for final closure.

## 2. Defects being corrected

At the planning baseline:

1. `init_server()` builds a `tokio_rustls::TlsAcceptor` and discards it in `_tls_acceptor`.
2. `ServerInstance` retains only a raw `TcpListener`.
3. `serve()` passes the raw listener directly to `axum::serve`, so the endpoint is plaintext HTTP despite HTTPS/TLS documentation and log messages.
4. The handler receives `body: String`; the custom `MAX_BODY_SIZE` check occurs only after the framework has buffered/extracted the body.
5. The request semaphore is acquired inside the handler and therefore does not bound TLS handshakes, accepted sockets, header parsing, or pre-handler body buffering.
6. No concrete slow/incomplete-body, handshake timeout, header timeout, or total request deadline evidence exists.
7. M007 tests for deeply nested JSON, large strings, and duplicate keys use assertions equivalent to `result.is_ok() || result.is_err()`, which are always true.
8. The closure record cites those tests as evidence of request/nesting/collection bounds.
9. Plaintext rejection and real TLS certificate validation were not exercised through the production serving path.

These are security and closure-evidence defects independent of Proposal 170 feature semantics.

## 3. Why prior verification missed the defects

Prior tests validated TLS configuration generation and parser functions separately. They did not connect a TLS client to the actual server loop or prove that plaintext requests fail. The production listener was inferred to be HTTPS because a TLS config existed and logs used the word HTTPS.

Resource tests called `rpc::parse_request()` directly after constructing a complete in-memory string, so they did not test network-layer buffering, connection limits, or timeouts.

This milestone must test the real listener from socket connection through TLS, HTTP, body limits, authentication, dispatch, response, and shutdown.

## 4. Invariants

- Enabled I2PControl accepts application requests only over TLS.
- Plaintext HTTP on the configured port never reaches JSON-RPC dispatch.
- TLS configuration/certificate/key errors fail startup.
- Private keys and passwords never appear in responses, logs, fixtures, or panic output.
- Request body limits are enforced while reading/buffering, before JSON parsing.
- Connection, handshake, body-read, request, and in-flight work are bounded.
- Unauthorized requests fail before protected inspection/mutation.
- JSON-RPC request IDs and exact success/error envelopes remain compatible.
- No core HTTP/TLS dependency is added to `emissary-core`.
- No UI dependency is required.
- No Proposal 170 field, method, selector, type, action, status, alias, or extension is added.
- No router, transport, NetDB, tunnel, service, frontend, or runtime resolver behavior changes.
- No missing tunnel data plane is implemented.

## 5. Explicit non-goals

- Internet-facing certificate automation or ACME.
- Mutual TLS unless already required by base I2PControl.
- HTTP/2 unless required by current compatibility evidence.
- A general-purpose web-server framework refactor.
- Moving server dependencies into core.
- Changing password/token protocol semantics without a separately documented compatibility requirement.
- Broad denial-of-service hardening outside the I2PControl listener.
- Release/publishing automation.

## 6. Required server architecture

### 6.1 Retain and use the TLS acceptor

`ServerInstance` must own the production TLS server configuration/acceptor required by `serve()`.

Conceptual shape:

```rust
pub struct ServerInstance {
    listener: TcpListener,
    tls_acceptor: TlsAcceptor,
    state: Arc<I2pControlState>,
    bind: SocketAddr,
}
```

The serving loop must accept TCP sockets, complete a bounded TLS handshake, and serve HTTP only on the resulting TLS stream.

Implementation options:

1. use existing `tokio-rustls` plus the minimal Hyper/Axum connection-serving APIs already transitively available; or
2. add one narrowly scoped optional TLS-serving dependency under the `i2pcontrol` feature.

Selection criteria:

- no dependency enters `emissary-core`;
- HTTP/1 support remains compatible;
- graceful shutdown and per-connection cancellation are explicit;
- handshake/connection limits are enforceable;
- tests can use ephemeral ports and generated test certificates;
- no UI feature is required.

Document the selected approach. Do not keep a dead TLS acceptor merely to satisfy tests.

### 6.2 Bound connection and request phases

Define explicit constants/configuration for at least:

- maximum simultaneously accepted/active connections;
- TLS handshake timeout;
- maximum request body bytes;
- maximum header bytes/count if supported by the selected server path;
- body/read timeout or total request deadline;
- maximum concurrent JSON-RPC handler executions;
- graceful shutdown drain deadline.

The exact values may remain conservative fixed constants for this private administrative service. They must be documented and tested.

Recommended phase order:

```text
TCP accept permit
    -> TLS handshake timeout
    -> HTTP connection
    -> body limit while reading
    -> JSON-RPC in-flight permit
    -> parse/validate
    -> authenticate/version gate
    -> bounded dispatch deadline
    -> response size check
```

Release every permit on failure, timeout, cancellation, or disconnect.

### 6.3 Enforce body limits before full extraction

Use Axum/HTTP body limiting at the router/layer/extractor boundary or a custom bounded reader.

Incorrect evidence:

```rust
async fn handle_jsonrpc(body: String) {
    if body.len() > MAX_BODY_SIZE { ... }
}
```

Required behavior:

- the server stops reading once the configured limit is exceeded;
- oversized input never becomes a full unbounded `String`;
- a stable HTTP/JSON-RPC error response is returned when possible;
- malformed or disconnected bodies release resources;
- test instrumentation can prove the parser/handler was not invoked for rejected bodies.

### 6.4 Explicit JSON parsing policy

Define and test:

- maximum nesting/depth;
- maximum string/body size through the body limit;
- array/batch rejection;
- named-params object requirement;
- duplicate JSON key policy;
- invalid UTF-8/body handling;
- request ID handling;
- notification behavior.

For duplicate keys, either reject them or document deterministic last-value behavior if compatibility requires it. Do not leave behavior as “may parse or may error.”

### 6.5 TLS material and logging

- Managed certificate/key generation remains confined under the configured base path.
- Existing user-provided certificate/key validation remains supported.
- Use restrictive key permissions where platform APIs permit; document platform limitations.
- Do not log certificate private key contents, passwords, tokens, raw request bodies, or complete error chains containing secrets.
- Log actual TLS listener startup only after the server is capable of accepting TLS.

## 7. Ordered work packages

### WP1 — Baseline listener audit

Before editing:

- trace `build_tls_config()` output into `init_server()` and `serve()`;
- enumerate all raw listener/serve paths;
- identify current Axum/Hyper versions and APIs available under `i2pcontrol`;
- inventory existing body limits, timeouts, semaphores, and response-size checks;
- inventory tests that claim TLS/resource coverage;
- list every tautological assertion and replace it in later work packages.

Record the selected serving approach before changing dependencies.

### WP2 — Actual TLS serving

- retain the TLS acceptor/config in `ServerInstance`;
- implement bounded TLS accept/serve loop;
- make plaintext HTTP fail before handler dispatch;
- preserve ephemeral bind reporting;
- preserve structured shutdown and token clearing;
- ensure connection tasks terminate on shutdown or drain deadline;
- surface accept/handshake/serve failures with sanitized diagnostics.

### WP3 — Pre-buffer request limits

- apply body limit before extraction;
- add connection and handler permits at the correct phases;
- add handshake and request deadlines;
- ensure oversized/slow/disconnected bodies release all permits;
- preserve exact JSON-RPC errors where a valid JSON-RPC response is possible;
- use HTTP-level rejection when parsing never safely begins.

### WP4 — Parser policy and adversarial behavior

- define duplicate-key policy;
- verify nesting/depth behavior;
- verify batch rejection;
- verify named params and ID handling;
- verify invalid/missing fields;
- ensure raw malformed input is not logged;
- keep auth/version compatibility unchanged.

If compatibility evidence requires behavior different from the current parser, document it before editing.

### WP5 — Replace weak tests

Delete or rewrite every test whose only assertion is equivalent to:

```rust
assert!(result.is_ok() || result.is_err());
```

Each adversarial test must assert one expected outcome and one or more side-effect/resource properties.

Examples:

- oversize body -> HTTP 413 or documented JSON-RPC error; handler invocation counter remains zero;
- nesting beyond limit -> exact parse/invalid-request error;
- duplicate key -> exact rejected or documented deterministic value;
- plaintext request -> TLS/connection failure and no JSON-RPC response;
- stalled handshake -> timeout and permit count restored;
- stalled body -> request timeout and permit count restored.

### WP6 — Real TLS client harness

Add a production-path harness that:

- generates isolated test certificate material;
- starts `init_server()` and `serve()` on an ephemeral loopback port;
- configures a client to trust the test certificate or explicitly uses the generated CA/certificate fixture;
- performs Authenticate and protected requests over HTTPS;
- supports raw plaintext and slow-socket adversarial probes;
- exposes deterministic handler/permit counters only through test instrumentation, not production wire fields;
- shuts down and immediately restarts on the same port where supported.

Avoid external network dependency and shelling out to platform-specific TLS clients.

### WP7 — Documentation and guards

Update:

- `docs/i2pcontrol/README.md`;
- `docs/i2pcontrol/security.md`;
- `docs/i2pcontrol/inspection-architecture.md` if server phase ownership is described;
- `docs/i2pcontrol/proposal-170-support.md`.

Add guards proving:

- the TLS acceptor is consumed by the serving path;
- raw `axum::serve(listener, ...)` is not used for I2PControl;
- production logs do not claim HTTPS before TLS serving is wired;
- body limit is applied before a `String`/complete-body extractor;
- tautological adversarial assertions do not return.

## 8. Failure, cancellation, restart, and contention semantics

### TLS/accept failure

- One malformed handshake does not terminate the listener.
- Repeated failures remain bounded by connection permits.
- Fatal listener failure surfaces to application supervision.
- Handshake error logs contain remote address and error class only where safe.

### Request failure

- Oversized, malformed, timed-out, unauthorized, and cancelled requests do not invoke protected handlers unnecessarily.
- No mutation is implicitly retried.
- Partial bodies are discarded within bounds.

### Cancellation/shutdown

- Shutdown stops accepting new sockets.
- Handshakes and requests receive cancellation or a bounded drain period.
- All connection/request permits are released.
- Tokens are cleared after serving tasks have stopped according to established semantics.
- No detached connection task survives server shutdown.

### Restart

- Immediate restart can bind the released port after shutdown.
- Managed TLS material is reused/validated according to existing policy.
- Old tokens are invalid.
- No process-global TLS or request state leaks across instances.

### Contention

- Connection permits bound pre-handler resource consumption.
- Handler permits bound JSON-RPC work.
- One slow client cannot monopolize the only permit indefinitely.
- Authentication floods remain bounded by connection/request limits and token-store capacity.

## 9. Required tests

### TLS correctness

- valid TLS client completes Authenticate and a protected method;
- plaintext HTTP cannot obtain a JSON-RPC response;
- invalid TLS handshake is rejected;
- client that does not trust the managed cert fails validation unless explicitly configured to trust it;
- wrong certificate/key pair fails startup;
- malformed managed material follows documented recovery/failure policy;
- TLS task shuts down and releases port.

### Body/resource limits

- body exactly at limit follows normal parse behavior;
- body one byte over limit is rejected before handler/parser invocation;
- very large declared/streamed body is cut off within the limit;
- slow/incomplete body times out;
- stalled TLS handshake times out;
- active connection and handler limits reject or queue according to documented behavior;
- permits return to full capacity after every failure/cancellation path.

### Parser/adversarial tests

- excessive nesting has one exact expected error;
- duplicate keys have one exact documented result;
- arrays/batches are rejected exactly;
- invalid UTF-8/body is rejected safely;
- missing/wrong JSON-RPC fields return exact errors;
- string/integer IDs are preserved;
- notifications return no content;
- extra/positional params follow exact compatibility rules;
- raw secrets/request bodies are absent from logs/errors.

### Authentication tests through TLS

- valid credentials issue a bounded opaque token;
- invalid username/password returns indistinguishable auth failure;
- invalid/unknown token fails before protected control calls;
- token capacity behavior remains deterministic;
- shutdown/restart invalidates old token;
- concurrent authentication remains bounded and thread-safe.

### Static evidence

- no tautological `is_ok() || is_err()` assertion in adversarial/resource suites;
- no discarded `_tls_acceptor`;
- no raw I2PControl `axum::serve` path;
- no post-buffer-only body-size check as the primary limit;
- no HTTP/TLS dependency in `emissary-core`.

## 10. Verification commands

```bash
cargo fmt --all -- --check
cargo check -p emissary-cli --no-default-features --features i2pcontrol
cargo test -p emissary-cli --no-default-features --features i2pcontrol
cargo test -p emissary-cli --no-default-features --features i2pcontrol --test i2pcontrol_tls
cargo test -p emissary-cli --no-default-features --features i2pcontrol --test adversarial
cargo test -p emissary-cli --no-default-features --features i2pcontrol --test request_resource_limits
cargo test -p emissary-cli --no-default-features --features i2pcontrol --test static_guards_m012
```

Run clippy:

```bash
cargo clippy -p emissary-cli --no-default-features --features i2pcontrol --all-targets -- -D warnings
```

Where practical, run the feature combination used by the full application:

```bash
cargo check -p emissary-cli --features ui,i2pcontrol
```

If UI system libraries block full tests, record the exact command/error and retain headless production-path TLS evidence. Do not call parser-only tests end-to-end evidence.

## 11. Acceptance criteria

1. The production I2PControl server actually serves HTTP over TLS streams.
2. TLS configuration is retained and consumed by the serving path.
3. Plaintext HTTP never reaches JSON-RPC dispatch.
4. TLS configuration/material failure prevents startup.
5. Connection and handshake work are bounded.
6. Request body size is enforced before full body/string buffering.
7. Slow handshake and slow/incomplete body have deterministic deadlines.
8. Handler concurrency remains bounded independently of connection count.
9. All permits/resources are released on success, failure, timeout, disconnect, cancellation, and shutdown.
10. Parser nesting and duplicate-key policies are explicit and tested.
11. Batch, params, ID, notification, and error-envelope behavior remain compatible.
12. Unauthorized requests fail before protected inspection/mutation.
13. Secrets and raw request bodies do not enter responses/logs/fixtures.
14. The real TLS client harness covers Authenticate and protected dispatch.
15. Immediate shutdown/restart invalidates tokens and releases the port.
16. Every tautological M007 adversarial assertion is removed or replaced with one exact expected outcome.
17. Resource tests assert side effects such as handler non-invocation and permit restoration.
18. No HTTP/TLS dependency enters core and no frontend dependency is required.
19. No Proposal 170 contract expansion, router behavior change, service lifecycle change, or missing tunnel data plane is introduced.
20. Documentation accurately describes actual TLS and exact limits.
21. Closure contains no unresolved high/medium security defect.

## 12. Stop conditions

Stop and record a blocker if:

- actual TLS serving requires a dependency/architecture change that cannot remain feature-gated to `emissary-cli`;
- base I2PControl compatibility conflicts with proposed parser/auth hardening;
- request limits cannot be enforced before buffering with the selected server stack;
- graceful shutdown cannot account for spawned connection tasks;
- implementation begins broad web-server or router refactoring.

Do not retain plaintext serving or tautological tests to avoid a blocker.

## 13. Closure evidence required

The closure record must include:

- implementation commits;
- selected TLS serving architecture and dependency review;
- real TLS client request/response transcript with secrets redacted;
- plaintext rejection evidence;
- body-limit pre-parser/handler evidence;
- slow handshake/body timeout evidence;
- permit restoration and shutdown/restart evidence;
- exact parser-policy table;
- list of removed/replaced tautological tests;
- static-guard output;
- verification command outcomes;
- compatibility/security/dependency review;
- unresolved findings by severity and disposition.

M013 remains blocked until M010, M011, and M012 are all strictly closed.