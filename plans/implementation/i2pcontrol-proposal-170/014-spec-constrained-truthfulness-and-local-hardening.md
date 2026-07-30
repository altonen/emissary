# I2PControl Proposal 170 Milestone 014 — Spec-Constrained Truthfulness and Local Hardening

Status: ready

Planning baseline: `2f0508dc73b8d8e5d7429effcbe4dbee8797833c`

Source roadmap:

- `plans/subsystems/i2pcontrol-proposal-170-roadmap.md`

Canonical requirements:

- `plans/000-long-term-specification.md`
- `plans/003-planning-process.md`
- `plans/adrs/ADR-0001-proposal-170-contract-and-stub-boundary.md`
- `docs/i2pcontrol/proposal-170-conformance.md`
- I2P Proposal 170 and the existing I2PControl JSON-RPC contract

Primary class: narrow correctness corrective pass

## 1. Objective

Correct the remaining Proposal 170 truthfulness, composition, and local resource-bound defects without expanding the API, redesigning Emissary core, reopening unrelated security work, or adding CI/release machinery.

This milestone is intentionally one implementation pass. It replaces any temptation to create separate RouterInfo, ClientServicesInfo, metrics, logging, TLS, or verification subprojects.

The implementation must make the smallest change that causes the existing Proposal 170 endpoint to report current truthful state or an existing protocol-compatible unavailable/error result.

## 2. Governing rules

Apply these rules in order:

1. Proposal 170 and the existing I2PControl contract are authoritative.
2. Use an existing canonical Emissary source when one already exists.
3. If the protocol permits null or an existing error for an unavailable source, use that behavior.
4. Do not create router behavior, counters, classifications, lifecycle authority, or data planes solely to populate an API field.
5. Do not report zero, false, empty, or stale state as current success when the source is unavailable.
6. Keep implementation inside `emissary-cli/src/i2pcontrol/` wherever possible.
7. Any core change must be a minimal bounded read-only observation seam and must not change runtime behavior.
8. Existing security-hardening architecture outside I2PControl is presumed valid and is not reopened.
9. Verification is local and targeted. Do not add GitHub Actions, release gates, matrices, generated evidence bundles, or new process policy.

When these rules conflict, stop and preserve explicit unavailable behavior rather than broadening the implementation.

## 3. Exact defects owned by M014

M014 owns only the following defects found after M013:

1. `CoreSnapshot` is captured once during startup and then presented as current RouterInfo state.
2. UDP and TCP active state are derived from one aggregate connected-router count rather than transport-specific truth.
3. bandwidth and recent-traffic selectors read I2PControl-local stores that are not wired to the production event source.
4. the RouterInfo log adapter receives a fresh ring instead of the tracing-backed application ring.
5. service observer generations are global across categories, allowing an unrelated service handle to invalidate another category.
6. active SAM state can return an unconditional empty sessions object because no bounded source is connected.
7. accepted TLS connections spawn without a count bound before the handler semaphore.
8. M010–M013 documentation and registry state overstate closure while these defects remain.

Do not pull unrelated historical, architectural, or security findings into this milestone.

## 4. Scope boundary

### 4.1 Primary allowed files

Production changes should be confined to:

- `emissary-cli/src/i2pcontrol/**`
- existing I2PControl tests under `emissary-cli/tests/`
- `docs/i2pcontrol/**`
- M014/M015 planning and closure files

### 4.2 Narrow composition exceptions

The following files may be touched only to pass an already-existing handle into I2PControl or to remove stale one-time composition:

- `emissary-cli/src/main.rs`
- `emissary-cli/src/logger.rs`

These files must not receive new general-purpose frameworks, lifecycle ownership, or unrelated refactors.

### 4.3 Narrow core exception

A core edit is allowed only when all of the following are true:

- Proposal 170 requires current state that cannot be read from an existing public handle;
- existing protocol-compatible unavailable behavior is insufficient for the required selector;
- the seam is read-only, bounded, secret-free, and does not change router decisions or task ownership;
- the change is limited to `emissary-core/src/inspection.rs` and the smallest existing canonical owner module needed to expose the read;
- no new background task, persistence store, event subscriber, global registry, or protocol dependency is introduced.

Before editing core, record in the implementation commit message:

- exact Proposal 170 field;
- existing canonical owner;
- why the current handle is insufficient;
- why explicit unavailable behavior would not satisfy the contract;
- exact read bound.

If more than one small owner seam or a cross-core redesign is required, stop and leave the selector explicitly unavailable. Do not weaken the rest of the router to complete M014.

### 4.4 Explicitly forbidden

M014 must not:

- modify `.github/workflows/**`;
- add CI jobs, platform matrices, required checks, release automation, or publishing logic;
- change crates.io, GitHub release, package, or version policy;
- add frontend controls;
- change runtime address-book precedence;
- implement missing tunnel data planes;
- migrate startup-managed tunnel ownership;
- add BOB support;
- add Proposal 170 extensions, aliases, pagination, richer status fields, or partial-result envelopes;
- add Java-I2P-specific peer tiers, NetDB counters, transport categories, or tunnel-pool concepts that Emissary does not already own;
- redesign the router, transport manager, NetDB, tunnel pools, SAM server, logging subsystem, event system, or task runtime;
- perform a broad security-hardening pass outside the I2PControl listener and adapters;
- introduce a generic task supervisor, metrics framework, logging framework, cache, polling daemon, or shadow state store;
- add dependencies unless the existing dependency set cannot implement the local fix. A new dependency requires an explicit stop-and-review note and is not presumed acceptable.

## 5. Required contract reconciliation

Before changing code, create a compact implementation note in the M014 commit or closure draft covering only these affected surfaces:

| Surface | Required decision |
|---|---|
| UDP active | identify a transport-specific canonical source or return existing unavailable/error behavior |
| TCP active | identify a transport-specific canonical source or return existing unavailable/error behavior |
| known/active peers and peer RouterInfo | current bounded read or explicit unavailable; never startup-cached success |
| participating tunnels | current existing event gauge or explicit unavailable |
| configured tunnel count | continue using the shared live TunnelManager service |
| bandwidth totals | use the actual production event-metrics source |
| recent traffic windows | use an already-fed canonical source or return unavailable; do not build a new sampler |
| RouterInfo logs | use the existing tracing-backed bounded ring |
| HTTP/SOCKS/I2CP/SAM listener state | preserve exact current listener semantics with per-category fencing |
| SAM sessions | exact Proposal 170-required safe shape from a bounded source, or explicit unavailable if permitted |
| TLS connection work | local count bound before spawning per-connection work |

Do not expand this table into a new project-wide conformance matrix. The existing normative matrix remains authoritative.

## 6. Ordered implementation work

### WP1 — Remove stale RouterInfo success

Replace the startup-owned `Option<CoreSnapshot>` behavior with one of these, in preference order:

1. existing live event/owner handles queried on demand;
2. one narrow clonable read-only inspection handle that obtains a bounded current snapshot;
3. explicit existing unavailable/error behavior for fields without a safe canonical live source.

Requirements:

- query only groups requested by the current RouterInfo call;
- do not add a periodic refresher or background cache;
- do not keep an immutable startup snapshot and call it live;
- do not copy or sort unbounded collections;
- do not hold a core lock across JSON serialization or another subsystem query;
- preserve exact selector filtering and existing error envelopes;
- distinguish real empty state from unavailable source.

Transport-specific state must not be inferred from a cross-transport aggregate. A connection on one transport must not make another transport report active.

For fields with no Emissary semantic, keep the M009 explicit unsupported/unavailable result. Do not populate zero-valued fields merely because they exist in a neutral DTO.

### WP2 — Wire existing metrics and logs

Use the actual production objects already created by the application:

- cumulative transport/transit/build values must come from the `EventHandleMetrics` or equivalent existing live source supplied at composition;
- remove or bypass I2PControl-local zeroed metrics objects that are not fed by production;
- recent-window values may be returned only if an existing production path actually records them;
- otherwise map requested recent-window selectors to existing unavailable/error behavior;
- pass the tracing-backed bounded `LogRing` returned by logger initialization into `ServerInitContext` and then into `ProductionRouterInfoControl`;
- do not create a second production log ring;
- preserve redaction and independent `clear` semantics for the I2PControl ring only.

Do not introduce a new metrics collector, event consumer, periodic sampling task, or logging layer.

### WP3 — Correct service observation ownership

Change generation fencing from one global generation to per-`ServiceCategory` ownership.

Requirements:

- allocating a SOCKS handle must not invalidate an HTTP handle;
- replacement of an HTTP producer must invalidate only older HTTP handles;
- the fixed category set remains fixed;
- no service task handle or lifecycle authority enters the registry;
- stale-update errors remain internal and sanitized;
- observer call sites may remain passive and minimal.

Review absent-entry behavior. Because the registry initializes every fixed category, a genuinely missing/unwired production source must not silently become known disabled success. Preserve the exact distinction already defined by M009/M011 or return the existing compatible error.

### WP4 — Resolve SAM sessions strictly to the specification

First verify the exact Proposal 170 SAM response requirement in the normative matrix and external contract.

Then choose exactly one path:

- **Required and safely observable:** add the smallest bounded read-only session summary accessor at the canonical SAM owner and map only non-sensitive fields required by Proposal 170.
- **Protocol permits unavailable:** return the existing compatible unavailable/error behavior when session state cannot be observed.
- **Contract ambiguity:** stop and record the blocker. Do not call an unconditional empty object complete.

Restrictions:

- no SAM lifecycle control;
- no session keys, destinations, private material, payloads, or authentication data;
- no generic SAM registry or new background monitor;
- no session-count placeholder used to justify an empty sessions object;
- oversize collections must fail before unbounded construction.

### WP5 — Add one local TLS connection bound

Bound accepted I2PControl connection work before `tokio::spawn`.

Preferred implementation:

- a local `Arc<Semaphore>` owned by `serve()`;
- acquire or try-acquire one owned permit before spawning TLS work;
- close/drop the accepted socket deterministically when saturated;
- move the permit into the spawned task so every exit path releases it;
- retain the existing TLS handshake and overall connection deadlines;
- retain the existing independent handler semaphore.

Do not add a process-wide supervisor or alter Emissary runtime scheduling. A local `JoinSet` may be used only if needed to implement bounded shutdown with existing APIs; it must not become a general task-management framework.

### WP6 — Correct documentation and planning state

Update only documentation made inaccurate by M014:

- `docs/i2pcontrol/router-info-source-map.md`
- `docs/i2pcontrol/router-info.md`
- `docs/i2pcontrol/client-services.md`
- `docs/i2pcontrol/security.md` if present and directly relevant
- `docs/i2pcontrol/proposal-170-support.md`
- roadmap, registry, and closure records

Required wording:

- distinguish current real values from explicit unavailable selectors;
- do not claim runtime completeness for unsupported tunnel data planes;
- do not call SAM session support complete unless actual required current sessions are exposed;
- do not call connection work bounded unless a count bound exists before spawn;
- do not claim the workstream closed until M015 accepts a frozen head.

## 7. Regression tests

Add the smallest useful tests to existing suites. Do not create a new test framework or evidence generator.

Required behavioral cases:

1. a live supported RouterInfo source changes after startup and the next request observes the new value;
2. source loss/unavailability produces the existing error rather than stale or zero success;
3. UDP activity does not become true solely because only a non-UDP transport is active, and the reciprocal case is also covered where test seams exist;
4. nonzero production event metrics reach the corresponding RouterInfo selectors;
5. a tracing event inserted through the existing application ring appears in RouterInfo log retrieval, and clear affects that ring without changing unrelated sinks;
6. allocating/updating different service categories does not invalidate their handles; replacement invalidates only the same category;
7. SAM sessions follow the selected spec-compliant path: exact bounded entries or explicit unavailable behavior;
8. opening more than the configured I2PControl connection limit cannot create more simultaneous TLS/connection tasks, and capacity is restored after disconnect/timeout;
9. a real TLS client can authenticate and make one protected request; plaintext does not reach JSON-RPC;
10. existing TunnelManager/ClientServicesInfo live inventory behavior remains unchanged.

Tests should reuse:

- `router_info_truthfulness.rs`
- `production_adapter.rs`
- `production_composition.rs`
- `client_services_live.rs`
- `adversarial.rs`
- existing core unit-test modules only when a minimal core seam is added

A new test file is allowed only if the production TLS connection-bound case cannot fit coherently in an existing suite. Do not split tests by milestone merely for documentation symmetry.

## 8. Verification commands

Run locally. Do not add these commands to GitHub Actions.

```bash
cargo fmt --all -- --check
cargo check -p emissary-cli --no-default-features --features i2pcontrol
cargo test -p emissary-cli --no-default-features --features i2pcontrol
cargo clippy -p emissary-cli --no-default-features --features i2pcontrol --all-targets -- -D warnings
```

Only when core is touched:

```bash
cargo check -p emissary-core --features std,events
cargo test -p emissary-core
cargo clippy -p emissary-core --features std,events --all-targets -- -D warnings
```

Optional focused reruns during development:

```bash
cargo test -p emissary-cli --no-default-features --features i2pcontrol --test router_info_truthfulness
cargo test -p emissary-cli --no-default-features --features i2pcontrol --test production_composition
cargo test -p emissary-cli --no-default-features --features i2pcontrol --test client_services_live
cargo test -p emissary-cli --no-default-features --features i2pcontrol --test adversarial
```

Do not require:

- full workspace tests unrelated to changed code;
- UI-feature builds requiring unavailable desktop libraries;
- multi-platform matrices;
- network interoperability farms;
- coverage thresholds;
- generated evidence archives;
- release dry runs.

Record environmental skips honestly. A skipped unrelated UI build does not block M014. A skipped required I2PControl regression does block M014.

## 9. Acceptance criteria

M014 implementation is complete only when all applicable criteria pass:

1. The implementation remains limited to Proposal 170 behavior.
2. No Proposal 170 method, selector, action, type, key, status, or error extension is added.
3. No unrelated security-hardening code is changed.
4. No CI, release, publishing, or required-check configuration is changed.
5. Production RouterInfo does not present a startup-owned snapshot as current state.
6. Supported live RouterInfo values change when their canonical source changes.
7. Unsupported or unsafe sources return existing protocol-compatible unavailable/error behavior.
8. UDP and TCP activity are not inferred from one cross-transport aggregate.
9. Cumulative bandwidth/build selectors use the actual production source.
10. Recent-window selectors are either fed by an existing real source or explicitly unavailable.
11. RouterInfo log retrieval uses the existing tracing-backed bounded ring.
12. No second production log ring or metrics shadow store remains on the response path.
13. Service generations are independent per category.
14. Same-category replacement still rejects stale updates.
15. SAM session output follows the exact Proposal 170 contract and is not an unconditional successful empty placeholder for an unavailable source.
16. No sensitive SAM or router material is exposed.
17. Accepted TLS connection tasks are count-bounded before spawn.
18. Connection and handler limits remain independent and release capacity on every tested exit path.
19. Real TLS authentication/protected dispatch and plaintext rejection are behaviorally tested.
20. TunnelManager, AddressBook, and unsupported tunnel-stub behavior remain unchanged.
21. No router algorithm, transport behavior, NetDB behavior, tunnel policy, frontend, or runtime resolver behavior changes.
22. Any core edit is the minimal read-only bounded seam permitted by section 4.3.
23. Targeted local format, check, tests, and clippy pass for touched packages.
24. Documentation states actual support and limitations without claiming M015 closure.
25. The implementation commit freezes a head for M015 review and the registry moves M014 to `closing`, not `closed`.

## 10. Stop conditions

Stop implementation and record a blocker rather than expanding scope when:

- exact Proposal 170 behavior cannot be determined from the normative matrix and external specification;
- truthful support requires changing router decisions, protocol behavior, tunnel construction, peer selection, NetDB ownership, or service lifecycle;
- current state would require a broad core refactor or more than a small read-only owner seam;
- a complete collection cannot be bounded without adding pagination or a protocol extension;
- the only source is a single-consumer event stream needed elsewhere;
- SAM session data cannot be exposed safely in the required shape;
- a new dependency or framework appears necessary;
- an implementation proposal includes new CI, release, security-policy, or project-wide verification machinery.

In each case, preserve explicit unavailable behavior and leave M015 blocked. Do not redefine the contract or claim closure.

## 11. Handoff guidance

Execution order for a smaller implementation model:

1. Freeze the affected selector/source decisions.
2. Wire the existing production log and metrics objects.
3. Correct per-category service generation fencing.
4. Replace stale RouterInfo snapshot success with live source/error behavior.
5. Resolve SAM strictly to the specification.
6. Add the local pre-spawn connection semaphore.
7. Add only the listed regressions.
8. Run the targeted commands.
9. Update docs and move the registry to `closing`.

Do not combine implementation and closure. M015 must review the frozen M014 head separately.