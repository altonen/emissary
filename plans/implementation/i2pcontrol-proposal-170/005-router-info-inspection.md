# I2PControl Proposal 170 Milestone 005 — RouterInfo Inspection and Exact Selectors

Status: blocked

Planning baseline: `ec289c77183d4f1010829ff255d8dbe90a941ad8` (`master`)

Production-code baseline described by the planning system: `9b43484a21d5a1291c4881cdae62a36c527f8c0f`

Activation rule:

- M002 must have a closure record with status `closed` before the core inspection and common selector work begins.
- M003 and M004 are soft integration dependencies for their address-book and I2PTunnel selector data. Their selector adapters may be completed after those milestones close, but M005 cannot close while any M005-owned selector lacks its final truthful data source.
- Before implementation begins, the agent MUST replace the baseline above with the reviewed dependency head, inspect all M001/M002 production changes, and reconcile this plan against the exact conformance matrix, selector DTOs, control-plane interfaces, persistence models, and current core structure.
- This prewritten plan is not dependency-ready while M002 is open.

Source roadmap:

- `plans/subsystems/i2pcontrol-proposal-170-roadmap.md#milestone-005--routerinfo-proposal-170-inspection`

Canonical requirements:

- `plans/000-long-term-specification.md#4-architectural-invariants`
- `plans/000-long-term-specification.md#5-protocol-exactness`
- `plans/000-long-term-specification.md#7-existing-runtime-ownership`
- `plans/000-long-term-specification.md#9-truthful-state-and-observability`
- `plans/000-long-term-specification.md#11-security-and-resource-bounds`
- `plans/001-terminology-and-domain-model.md#2-control-plane-terms`
- `plans/001-terminology-and-domain-model.md#5-router-and-service-inspection-terms`
- `plans/002-long-term-roadmap.md#milestone-m005--routerinfo-inspection`

Applicable ADRs:

- `plans/adrs/ADR-0001-proposal-170-contract-and-stub-boundary.md`

Primary class: capability / infrastructure

## 1. Objective

Implement every Proposal 170 RouterInfo selector using exact selector-by-presence behavior and truthful, bounded, read-only state sources.

At completion, authenticated callers must be able to request any Proposal 170 RouterInfo selector individually or in a valid combination and receive:

- only the requested exact response keys;
- the exact M001 JSON types and nullability behavior;
- retained local router identity and serialized RouterInfo;
- bounded router logs with an independently clearable I2PControl buffer;
- cumulative and rolling bandwidth/tunnel metrics;
- truthful configured/active tunnel summaries;
- exploratory, client, and participating tunnel inspection;
- network reachability/testing information;
- success rates and queue sizes;
- known, active, banned, and limited peer information;
- bounded serialized RouterInfo and active-peer statistics;
- M003 administrative address-book state;
- M004 I2PTunnel administrative/runtime state.

The inspection path must not mutate router behavior, consume the frontend's event receiver, expose mutable core subsystem handles, or fabricate missing values.

## 2. Why this milestone is blocked

Hard/interface dependency:

- M002 must close with a stable control-plane composition boundary and bounded snapshot conventions.

Soft integration dependencies:

- M003 owns the four administrative address-book selectors, subscriptions, and configuration data source.
- M004 owns the configured I2PTunnel definition/status data source.

M005 may establish selector registration, startup-retained values, log snapshots, shared metrics, and read-only core inspection after M002. It must keep M003/M004 adapters behind interfaces or fakes until those milestones close.

M005 must not close with placeholder empty collections for selectors whose real source is merely not integrated yet.

## 3. Current implementation evidence

At the production baseline:

### Router construction and retained values

- `Router::new` constructs the local `RouterInfo`, serializes it, computes the local router ID, creates core managers, and returns `(Router, EventSubscriber, Vec<u8>)`.
- `emissary-cli::setup_router` writes the returned serialized RouterInfo to `router.info` and retains the `RouterId` in `RouterContext`.
- The serialized RouterInfo itself is not retained in the application control context after the file write.
- `RouterContext` currently owns the router, base path, router ID, one EventSubscriber, configuration, optional runtime address-book handle, port mapper, and UI configuration.

### Public Router surface

- `Router` publicly exposes shutdown, protocol listener addresses, router ID, and external-address updates.
- NetDB, tunnel manager, profile storage, tunnel pools, transport sessions, clock-skew estimators, queues, peer bans, and SAM session details are internal.
- A complete Proposal 170 implementation therefore requires narrow additive read-only snapshot interfaces.

### Event and metrics state

- `EventHandle` already tracks cumulative transport inbound/outbound bytes, transit inbound/outbound bytes, connected routers, transit tunnel count, tunnel build successes, and tunnel build failures with atomics.
- `EventManager` periodically produces aggregate `RouterStatus` events.
- `EventSubscriber` owns one receiver and is passed to the UI when enabled.
- Consuming that receiver from I2PControl would steal events and violate frontend independence.
- Existing counters do not directly provide all rolling-window, queue, peer, or detailed tunnel data required by Proposal 170.

### Logging

- `emissary-cli/src/logger.rs` installs a formatted tracing layer and reloadable target filter.
- It does not maintain a bounded readable ring buffer.
- There is no I2PControl-specific clear operation.

### Core data availability

- Core managers possess most required information, but it is distributed across NetDB/profile storage, transports, tunnel managers/pools, SAM/I2CP servers, and reachability logic.
- Existing code was not designed to expose one mutable management handle, which is desirable; M005 should add bounded immutable snapshots instead.

### Administrative integrations

- M003 will own address-book snapshots.
- M004 will own configured I2PTunnel inventory and internal status.
- M005 must consume those interfaces rather than reading their persistence files.

## 4. Invariants that must not regress

- Every selector key, JSON type, nullability rule, and selector-presence rule comes from the closed M001 matrix.
- Only requested selector keys appear in the result.
- Existing non-Proposal-170 RouterInfo behavior remains compatible.
- Missing or unavailable data is represented only by a protocol-permitted null/error outcome.
- Zero, false, empty string, empty list, or empty object is not used as a generic placeholder.
- Snapshot collection is read-only.
- No mutable NetDB, transport, tunnel, profile, SAM, I2CP, queue, or ban handle crosses into the JSON-RPC layer.
- Snapshot requests cannot change peer selection, tunnel construction, bans, queue ordering, routing, reachability tests, or transport sessions.
- I2PControl does not consume `EventSubscriber` or otherwise interfere with UI events.
- Logs are bounded, redacted, and independently clearable without clearing terminal/file/other tracing sinks.
- Peer, RouterInfo, tunnel, log, and queue responses are bounded before serialization.
- Oversize complete results fail explicitly rather than truncating unless the exact protocol defines truncation.
- Core remains free of HTTP/JSON-RPC/Serde-JSON server dependencies.
- Core inspection code remains usable without a frontend.
- Polling cannot block router progress on unbounded work.
- No private keys, authentication tokens, destination private material, or sensitive proxy credentials enter snapshots.
- M003 administrative books remain separate from runtime resolution.
- M004 unsupported tunnels remain inactive.

## 5. Scope

### In scope

- Exact Proposal 170 RouterInfo selector registry and dispatch.
- Selector-by-presence parsing and only-requested-field serialization.
- Retaining startup router ID and serialized local RouterInfo.
- Truthful handling of router news according to M001's exact source/empty semantics.
- Clock-skew inspection.
- Bounded log ring buffer and clear operation.
- Shared cumulative metrics snapshots.
- Fixed rolling-window metrics required by Proposal 170.
- Share-ratio and configured limit reporting.
- Participating, exploratory, and client tunnel snapshots.
- I2PTunnel quick statistics from M004.
- Network reachability, IPv4/IPv6, testing, and error state.
- Recent and total tunnel success rates.
- Tunnel queue sizes.
- Known/active peer counts and identity lists.
- Serialized peer RouterInfo lookup/list responses.
- Active peer transport/session statistics.
- Configured and effective peer/transport limits.
- Banned peer snapshots.
- Address-book selectors from M003.
- Read-only core inspection contracts and bounded query channels.
- Handler, core, restart, contention, security, and resource tests.

### Explicitly out of scope

- Mutating any inspected state.
- Adding RouterInfo setters or management actions.
- Changing tunnel pools, routing, peer selection, bans, transport sessions, NetDB storage, or queue policy.
- Changing reachability testing behavior.
- Starting new metrics servers.
- Reusing the UI event receiver.
- Exposing full internal manager handles.
- Returning unbounded full-NetDB dumps.
- Pagination or continuation tokens not defined by Proposal 170.
- Silent list truncation.
- Inventing news retrieval, update checks, or network fetches.
- Implementing missing tunnel types.
- Runtime address-book integration.
- ClientServicesInfo method handling, except shared inspection infrastructure used by M006.
- Frontend changes.

## 6. Required production changes

### Selector inventory and source manifest

M001's conformance matrix must be converted into one authoritative RouterInfo selector manifest. Every Proposal 170 selector row must declare:

- exact external request key;
- exact response key;
- exact JSON type;
- nullability/omission rule;
- selector-presence behavior;
- required authentication/version;
- semantic data source;
- owning milestone;
- configured item/byte/work bound;
- fixture identifier;
- error behavior when a complete truthful value cannot be produced.

Group the implementation internally without changing the external names:

1. Identity and static router data.
2. Logs and log clearing.
3. Cumulative and rolling traffic metrics.
4. Participating/configured/exploratory/client tunnel state.
5. Network reachability/testing/error state.
6. Success rates and queues.
7. NetDB/peer identities, RouterInfos, bans, limits, and active-session statistics.
8. Address-book administrative data.

Add a test that fails when a selector exists in the M001 matrix but has no source adapter and exact fixture.

### Handler and response assembly

Implement RouterInfo extension handling behind M001's method registry.

The handler must:

1. authenticate/version-check before expensive inspection;
2. parse exact selector presence into a compact requested-selector set;
3. calculate an aggregate response budget before issuing expensive queries;
4. request only the necessary snapshots;
5. enforce per-selector and aggregate deadlines;
6. assemble only requested exact fields;
7. validate the final JSON type/shape against the manifest in tests;
8. avoid returning partial results after one selected field fails unless M001 explicitly permits per-field null/error behavior.

Do not eagerly collect all router state for every request.

### Startup-retained identity and RouterInfo

Extend the application control context to retain immutable startup values:

- local `RouterId` in exact Base64 form required by M001;
- serialized local RouterInfo bytes returned by `Router::new`;
- exact Base64 encoding of the serialized RouterInfo;
- startup time or other stable context only if a selector requires it;
- effective configuration values needed by selectors, such as share ratio and configured connection limits.

Requirements:

- do not reread `router.info` for each request;
- do not expose signing/static/private keys;
- retain one bounded immutable byte buffer;
- preserve exact null behavior if local RouterInfo is legitimately unavailable in a test configuration;
- if RouterInfo changes after external address discovery and the proposal expects current serialized RI, define a read-only current-RI snapshot source rather than falsely returning the startup value. Resolve this during activation from M001 semantics.

### Router news

Proposal 170 defines a router-news selector. Emissary has no news subsystem.

M005 may return an empty string only if M001's compatibility analysis establishes that:

- the field is a non-null string;
- an empty string truthfully means no current router news;
- no network fetch or update subsystem is required.

Otherwise, use the exact permitted unavailable/error behavior. Do not add a news downloader, hard-coded announcement, timestamp, or extension field.

### Clock-skew inspection

Identify the canonical existing clock-skew estimate used by transports/router logic. Add a read-only snapshot that:

- returns the exact unit and signedness required by M001;
- does not reset or influence the estimator;
- distinguishes not-yet-known from zero skew;
- avoids averaging or converting independently in the handler;
- is bounded and lock-free or uses a short read lock.

If Emissary has no canonical estimate, do not invent one from wall-clock samples. Stop and record the selector as requiring a narrowly scoped core measurement design/ADR.

### Bounded log buffer

Add a dedicated tracing layer or writer that captures sanitized formatted events into an in-memory bounded ring.

Required properties:

- fixed maximum entries and total bytes;
- deterministic eviction of oldest complete entries;
- no blocking disk/network I/O;
- no recursive tracing from buffer operations;
- redaction of tokens, passwords, private keys, and sensitive destination material;
- clear operation affects only this ring and increments an internal generation;
- concurrent readers receive an immutable snapshot;
- clear racing read returns a coherent before-or-after generation;
- existing terminal/file formatting and filter reload remain unchanged;
- disabled I2PControl builds need not retain the ring unless a shared diagnostics feature already justifies it;
- no request body or complete configuration is logged.

The selector that clears logs must use the exact M001 presence/action semantics. Do not expose extra cursor/generation metadata.

### Shared metrics snapshot

Replace dependence on the single `EventSubscriber` with a cloneable/passive metrics snapshot source.

A suitable design is:

```rust
pub struct RouterMetricsSnapshot {
    total_transport_received: u64,
    total_transport_sent: u64,
    total_transit_received: u64,
    total_transit_sent: u64,
    connected_routers: usize,
    participating_tunnels: usize,
    tunnel_build_successes: u64,
    tunnel_build_failures: u64,
    // fixed rolling-window values required by Proposal 170
}
```

Exact fields/types must follow M001.

Implementation constraints:

- reuse canonical event/metrics counters where semantics match;
- do not read and drain `EventSubscriber`;
- use saturating/widened counters to avoid architecture-dependent `usize` overflow on the API boundary;
- define process-lifetime reset semantics clearly;
- counters remain monotonic except process restart;
- snapshot reads are non-destructive;
- the UI continues receiving its existing events.

If changing event counters from `usize` to `u64` is necessary for exact long-running semantics, scope the change to representation correctness and prove no router behavioral effect.

### Rolling 15-second transit traffic

Proposal 170 requires recent transit traffic over a fixed interval. Implement a bounded rolling-window accumulator owned by the metrics layer, not by each request.

Required semantics:

- fixed buckets or a ring covering at least the exact 15-second interval;
- monotonic clock, not wall-clock;
- precise documented boundary inclusion;
- bounded constant memory;
- handles timer delay and counter wrap/reset safely;
- read is O(number of fixed buckets), not O(events);
- no per-request background task;
- tests use paused/deterministic time;
- process restart begins with an empty recent window rather than fabricated historical values.

### Recent and total tunnel success rates

Use canonical tunnel build success/failure counters.

- Total success rate derives from cumulative counters using the exact M001 formula/type/rounding.
- Recent success rate uses a fixed rolling window defined by the proposal/reference compatibility analysis.
- Zero-attempt behavior follows exact M001 null/zero semantics.
- Failed/rejected/timed-out classifications match current tunnel manager accounting; do not redefine build outcomes.
- Reads are passive and do not reset counters.
- Floating-point formatting/precision is deterministic if the wire type is numeric.

### Read-only core inspection handle

Add one additive public inspection boundary to `emissary-core`, for example:

```rust
pub struct RouterInspectionHandle<R: Runtime> {
    request_tx: ...
}
```

or a collection of smaller handles owned by a top-level facade.

The handle must expose only typed bounded queries and immutable snapshots. Candidate queries:

- tunnel summary snapshot;
- exploratory inbound/outbound tunnel snapshot;
- client destination tunnel snapshot;
- participating tunnel snapshot;
- tunnel queue-depth snapshot;
- NetDB summary;
- known peer identity list;
- active peer identity list;
- selected peer RouterInfo serialization;
- banned peer snapshot;
- configured/effective peer and transport limits;
- selected active-peer transport statistics;
- network reachability/testing/error snapshot;
- clock-skew snapshot;
- SAM session summary shared later with M006.

Each query must include:

- explicit maximum items/bytes;
- deadline/cancellation context or bounded oneshot response;
- typed unavailable/busy/too-large errors;
- no mutation variant;
- no arbitrary closure/function supplied by the caller;
- no direct references into mutable core collections;
- no private key or sensitive handshake material.

### Query placement and ownership

Expose snapshots from the subsystem that owns the data:

- NetDB/profile storage owns known peers, stored RouterInfos, bans, and participation/profile summaries.
- Transport manager/subsystem manager owns active connection identities, addresses, direction/state, byte counters, and configured/effective connection limits.
- Tunnel manager/pools own exploratory/client/participating tunnel summaries and queue sizes.
- Reachability logic owns IPv4/IPv6 network status, errors, and testing state.
- SAM server owns session summaries required by M006.

The top-level Router may retain cloneable inspection request handles. It must not become a giant lock over every manager.

### Snapshot construction rules

- Build snapshots inside the owning subsystem while its state is coherent.
- Copy only protocol-required primitive data.
- Sort deterministically outside critical locks where possible.
- Enforce item and byte estimates before serializing large RouterInfos.
- Never hold transport/NetDB/tunnel locks while JSON serializes.
- Avoid nested cross-subsystem lock acquisition.
- Use request channels when state is actor-owned rather than adding shared mutable locks.
- Apply backpressure with bounded channels; do not spawn a task per item.
- Return busy/timeout rather than blocking router progress.

### Tunnel snapshots

Implement exact Proposal 170 semantics for:

- participating tunnels;
- configured I2PTunnel quick statistics from M004;
- exploratory inbound and outbound tunnel lists/info;
- client inbound and outbound tunnel lists/info;
- tunnel queues.

The M001 matrix must define each record field and type. Do not expose internal Rust structs directly.

Requirements:

- tunnel IDs/hops/expiration/build state are copied read-only;
- no hop private/session keys;
- no control handle;
- unsupported M004 definitions appear only in the configured I2PTunnel view and remain inactive;
- startup-managed configured tunnels are represented only according to M004's truthful inventory;
- exploratory/client pool inspection does not trigger building or refresh;
- queue counts are instantaneous bounded gauges with exact meaning documented;
- an expired/stale item is included or excluded according to the owner's canonical current-state rule, not handler timing guesswork.

### NetDB and peer snapshots

Implement exact Proposal 170 selectors for semantic groups including:

- number/list of known peers;
- number/list of active peers;
- serialized RouterInfo data requested by exact ID/list semantics;
- peer and connection limits;
- banned peers;
- active-peer statistics.

Requirements:

- known peers use the canonical NetDB/profile set, not filesystem enumeration;
- active peers use live transport sessions, not recently seen timestamps;
- IDs use exact Base64 format;
- requested RouterInfo serialization comes from validated stored data and is Base64 encoded exactly once;
- absent peer behavior follows M001 null/error semantics;
- bans expose only protocol-required peer/reason/expiration data and no mutable ban operation;
- active statistics exclude handshake/session secrets;
- IP/socket information is exposed only if the proposal requires it and security review approves the exact field;
- result limits are checked before constructing the full list;
- when a selector requests the complete list and it exceeds configured safe response bounds, fail the complete field rather than truncate silently.

### Network status and testing

Map current reachability/firewall state into the exact Proposal 170 IPv4/IPv6/error/testing codes.

- Centralize mapping from internal states to wire values.
- Preserve unknown/testing/firewalled/OK/symmetric-NAT distinctions only where exact codes exist.
- Do not parse UI display strings back into protocol codes.
- Retain the latest passive state in a cloneable snapshot source rather than consuming events.
- Distinguish never-tested from currently testing and last-known result if required.
- Error text/codes are bounded and sanitized.
- No API request starts a reachability test.

### Share ratio and limits

Use retained effective configuration values:

- report exact share ratio type/units/rounding;
- distinguish configured limit from effective/current count;
- use current NTCP2/SSU2 limits where selectors require them;
- do not infer unlimited values from `None` without following exact M001 mapping;
- do not mutate configuration.

### Address-book and I2PTunnel adapters

Define narrow interfaces so M005 can compile/test before M003/M004 close:

- fake administrative address-book snapshot provider;
- fake I2PTunnel inventory provider.

Final closure requires production adapters to the closed M003/M004 control-plane services. No direct store-file reads are permitted.

### Response budgets and no truncation

Define configurable or fixed safe limits for:

- total RouterInfo response bytes;
- log entries/bytes;
- peer identities;
- serialized peer RouterInfos;
- active peer stats;
- tunnel records;
- address-book records through M003;
- per-query core work and deadline.

The handler should estimate worst-case output before issuing all expensive queries. If a requested complete selector cannot fit, return the exact bounded error. Do not add pagination or truncation flags.

### Security and redaction

- Authenticate before core query dispatch.
- Never include private keys, tunnel layer/session keys, authentication tokens, proxy passwords, destination private material, or raw handshakes.
- Sanitize internal error strings and paths.
- Redact logs before they enter the ring where possible.
- Avoid IP/address disclosure beyond exact Proposal 170 fields.
- Bound Base64 encoding allocation.
- Use constant-time token validation inherited from M001.
- Rate/concurrency-limit expensive RouterInfo calls.
- A caller cannot select arbitrary filesystem paths or internal object IDs outside exact parser constraints.

### Documentation and static guards

Document:

- every selector and its semantic source;
- null/unavailable behavior;
- counter reset/process-lifetime semantics;
- rolling-window semantics;
- log ring bounds and clear scope;
- peer/tunnel response bounds;
- read-only inspection architecture;
- no frontend event consumption;
- no router behavior mutation.

Add guards proving:

- selector registry equals the M001 inventory exactly;
- handlers return only requested keys;
- no `EventSubscriber` import/use in I2PControl;
- no frontend/UI import in inspection code;
- no HTTP/JSON-RPC dependency in core;
- no mutable command variants in `RouterInspectionHandle`;
- no private-key types in public snapshot DTOs;
- no direct persistence-file reads from RouterInfo handlers;
- no silent truncation helper;
- no unbounded collection conversion in selector paths.

## 7. Ordered work packages

### Work package A — Reconcile selector matrix and source ownership

Intent: make every selector's implementation source explicit before adding core APIs.

Required changes:

1. Update the plan baseline after M002.
2. Enumerate every exact M001 RouterInfo selector.
3. Record key/type/nullability/source/bounds/error/fixture.
4. Classify selectors as CLI-retained, log, metrics, core inspection, M003, or M004.
5. Identify any selector with no truthful source and stop for a narrow decision before coding it.

Acceptance evidence:

- manifest has no unowned selector;
- no source is described as placeholder or TODO;
- every core query is justified by at least one exact selector.

### Work package B — Startup values, selector registry, and response assembly

Intent: implement the non-core exact framework.

Required changes:

1. Retain router ID and serialized local RouterInfo.
2. Register all exact selectors.
3. Implement presence-only requested-set parsing.
4. Implement exact only-requested response assembly.
5. Add aggregate response budgeting and deadline propagation.
6. Resolve router-news behavior.

Acceptance evidence:

- fixture per selector with fake sources;
- combined selector fixtures;
- unrelated key absence;
- no eager all-state collection.

### Work package C — Bounded logs and shared metrics

Intent: provide multi-consumer observability without stealing events.

Required changes:

1. Add bounded redacted tracing ring and independent clear.
2. Add cloneable passive cumulative metrics snapshot.
3. Add rolling 15-second transit counters.
4. Add recent and total tunnel success rates.
5. Retain network state passively without consuming UI events.
6. Add deterministic time and concurrency tests.

Acceptance evidence:

- UI event tests unchanged;
- log clear leaves other sinks active;
- exact rolling-window boundary fixtures;
- process restart/reset semantics documented and tested.

### Work package D — Core inspection contracts

Intent: expose only the immutable data Proposal 170 requires.

Required changes:

1. Define bounded snapshot DTOs and typed errors.
2. Add actor/query handles to NetDB, transport, tunnel, reachability, and SAM owners as required.
3. Retain a top-level Router inspection facade.
4. Enforce per-query limits/deadlines/backpressure.
5. Avoid cross-subsystem locks and mutable handles.
6. Add no-mutation and no-secret static guards.

Acceptance evidence:

- core unit tests for every query;
- busy/timeout/oversize behavior;
- snapshot coherence under mutation;
- router progress/contention benchmarks.

### Work package E — Tunnel, peer, queue, and network selector adapters

Intent: map snapshots into exact protocol values.

Required changes:

1. Implement participating/exploratory/client tunnel mappings.
2. Implement queue mappings.
3. Implement known/active/RouterInfo/banned/limits/active-stat peer mappings.
4. Implement clock skew and network-status mappings.
5. Implement share ratio and configured/effective limit mappings.
6. Add exact Base64/type/range tests.

Acceptance evidence:

- per-selector production-source integration tests;
- no private/sensitive material;
- no fabricated values;
- complete-result oversize failure tests.

### Work package F — M003/M004 integration and end-to-end closure readiness

Intent: replace fakes with the canonical administrative services.

Required changes:

1. Integrate M003 address-book snapshots after M003 closes.
2. Integrate M004 I2PTunnel inventory after M004 closes.
3. Run real-listener combined selector tests.
4. Add static scope guards and documentation.
5. Update matrix evidence links and registry status.

Acceptance evidence:

- production adapters use interfaces, not persistence files;
- unsupported tunnels remain inactive;
- address-book selectors equal M003 committed state;
- all exact selector rows have production evidence.

## 8. Failure, cancellation, restart, and contention semantics

- Selector parsing failure issues no core query.
- Authentication failure issues no log, metric, store, or core query.
- A multi-selector request uses coherent immutable snapshots per source and never serializes while holding core mutable locks.
- If one required selected field fails and M001 does not permit per-field null, the entire request fails without returning a misleading partial result.
- Core query channels are bounded; full channels return busy/timeout rather than spawning unbounded work.
- Cancellation drops response receivers and stops scheduling additional queries. Owning actors complete or abandon bounded copy work without changing state.
- Snapshot requests never cancel or interrupt router operations.
- Log clear racing read yields a coherent generation before or after clear.
- Counter snapshots are non-destructive and safe under concurrent increments.
- Rolling-window timers tolerate delayed polling without unbounded catch-up work.
- Restart resets process-lifetime and recent metrics as documented while preserving persistent administrative M003/M004 state.
- Core inspection handles become unavailable cleanly during shutdown; requests receive a sanitized unavailable error.
- Shutdown does not wait for unbounded list serialization.
- Response construction aborts before exceeding configured byte/work limits.
- No stale snapshot is represented as current when the exact selector requires live state; timestamping may remain internal and cannot become an extension field.

## 9. Compatibility and migration

- Existing RouterInfo method behavior remains unchanged for pre-Proposal-170 selectors.
- Existing `Router::new` callers may require an additive return/context change or builder-accessible inspection handle; preserve source compatibility where practical and update all workspace callers explicitly.
- Core feature combinations, including no-std/event-disabled configurations where supported, must remain buildable. Inspection may be feature-gated but cannot force HTTP/Serde-JSON into core.
- Existing UI `EventSubscriber` ownership remains unchanged.
- Existing metrics server behavior remains unchanged.
- Existing tracing output/filter behavior remains unchanged.
- M003/M004 stores and schemas remain authoritative; M005 adds no duplicate persistence.
- Process-lifetime metric reset is documented; no persistence migration is needed for volatile metrics.
- Any widening from `usize` to `u64` is additive representation hardening and must preserve internal semantics.
- No protocol extensions are introduced to expose truncation, cursor, snapshot time, backend type, or internal generation.

## 10. Required tests

### Selector contract tests

For every exact selector:

- selector alone returns exactly one expected key;
- key spelling and JSON type match M001;
- omitted selector yields no key;
- invalid selector value/type follows exact behavior;
- null/unavailable behavior is exact;
- source error mapping is exact;
- response bound behavior is exact.

Add pairwise/representative multi-selector tests and one all-selector test within safe fixture bounds.

### Identity/static tests

- router ID exact Base64;
- serialized RouterInfo exact Base64 and no double encoding;
- startup/current RI semantics;
- share ratio type/rounding;
- configured limit mapping;
- router-news empty/unavailable semantics.

### Log tests

- bounded entries and bytes;
- deterministic oldest eviction;
- concurrent write/read/clear;
- clear does not affect terminal/other sink;
- no recursion;
- redaction of tokens/passwords/private keys/destinations;
- oversized single event behavior;
- disabled-feature behavior.

### Metrics/window tests

- cumulative bytes and counts;
- non-destructive concurrent reads;
- 15-second exact window boundaries with paused time;
- timer delay/catch-up;
- restart reset;
- recent/total success with zero and mixed attempts;
- overflow/widening behavior;
- UI EventSubscriber still receives events.

### Core inspection unit tests

- each query success path;
- empty/unknown state;
- configured item/byte limit;
- channel backpressure;
- deadline/cancellation;
- actor shutdown;
- deterministic ordering;
- no mutable references escape;
- no private material in snapshots;
- concurrent subsystem mutation produces coherent snapshots;
- no deadlock under mixed query load.

### Tunnel selector tests

- participating counts/info;
- exploratory inbound/outbound/info;
- client inbound/outbound/info;
- queue sizes;
- M004 configured I2PTunnel quick stats;
- unsupported inactive state;
- expired/current filtering;
- no build/refresh side effect.

### Peer/NetDB tests

- known vs active semantics;
- exact IDs;
- selected RouterInfo serialization;
- absent peer behavior;
- bans and expiration;
- configured/effective limits;
- active peer stats;
- complete-result oversize rejection;
- no arbitrary peer lookup outside exact ID validation;
- no private transport material.

### Network/clock tests

- IPv4/IPv6 exact code mappings;
- unknown/testing/firewalled/OK/symmetric-NAT mappings where defined;
- network error sanitization;
- clock skew known/unknown/zero/positive/negative;
- request does not trigger a test or mutate estimator.

### Integration, contention, and performance tests

- real HTTPS listener with production sources;
- headless and UI-enabled operation;
- continuous RouterInfo polling while router/event loop progresses;
- concurrent expensive peer/log/tunnel requests bounded by permits;
- shutdown during queries;
- immediate restart;
- large safe fixture near configured response limit;
- no unbounded memory growth under repeated polling;
- baseline router throughput/progress remains within a documented tolerance under moderate polling.

### Static/security tests

- no `EventSubscriber` use in I2PControl;
- no UI imports;
- no JSON-RPC/server dependency in core;
- no mutation variants in inspection APIs;
- no private-key types in public snapshot definitions;
- no direct state-file reads from handlers;
- no truncation helper/extension fields;
- logs/errors redact secrets;
- unauthorized requests issue no query.

## 11. Required verification commands

The activation pass must reconcile exact feature/test names. Expected minimum:

```bash
cargo fmt --all -- --check

cargo check -p emissary-core --no-default-features
cargo check -p emissary-core --all-features
cargo check -p emissary-cli --no-default-features --features i2pcontrol
cargo check -p emissary-cli --features ui,i2pcontrol

cargo test -p emissary-core events
cargo test -p emissary-core router
cargo test -p emissary-core netdb
cargo test -p emissary-core tunnel
cargo test -p emissary-core transport

cargo test -p emissary-cli --no-default-features --features i2pcontrol i2pcontrol::router_info
cargo test -p emissary-cli --no-default-features --features i2pcontrol i2pcontrol::logs
cargo test -p emissary-cli --no-default-features --features i2pcontrol i2pcontrol::metrics

cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Also run M001 fixture validation, M002 persistence tests, M003/M004 production adapter tests once those milestones close, and any dedicated contention/performance harness added by this plan.

## 12. Documentation updates

- Add or update `docs/i2pcontrol/router-info.md`.
- Add or update `docs/i2pcontrol/inspection-architecture.md`.
- Update `docs/i2pcontrol/security.md`.
- Update `docs/i2pcontrol/proposal-170-support.md`.
- Update the conformance matrix with source and evidence for every selector.
- Document cumulative and rolling metric semantics.
- Document process-restart reset behavior.
- Document log bounds, redaction, and clear scope.
- Document peer/tunnel response bounds and no-truncation behavior.
- Document every null/unavailable case.
- Document read-only core query ownership and backpressure.
- State that no API query mutates router state or triggers tests/builds.
- State that frontend events are not consumed.

## 13. Acceptance criteria

1. M002 is strictly closed and this plan is reconciled to its reviewed head.
2. Every M001 Proposal 170 RouterInfo selector has one authoritative manifest row.
3. Every selector has an exact key, JSON type, nullability rule, source, limit, and fixture.
4. The selector registry contains no missing or extra Proposal 170 selector.
5. Only requested selector keys appear in responses.
6. Authentication/version validation precedes expensive inspection.
7. Router ID is returned in the exact required encoding.
8. Serialized local RouterInfo is returned in the exact required encoding and semantics.
9. Router-news behavior is truthful and requires no new network subsystem.
10. Clock skew uses a canonical passive estimate and distinguishes unknown from zero.
11. Logs are bounded by entries and bytes.
12. Logs are redacted before exposure.
13. Log clear affects only the I2PControl ring.
14. Log reads/clear are coherent under concurrency.
15. Cumulative transport/transit byte counters are non-destructive and correctly typed.
16. Recent transit traffic uses the exact fixed rolling interval.
17. Recent and total tunnel success rates use exact semantics and zero-attempt behavior.
18. Share ratio and configured/effective limits use truthful retained configuration.
19. I2PControl does not consume or interfere with `EventSubscriber`.
20. Core exposes only bounded read-only inspection snapshots.
21. No mutable subsystem handle or private key material escapes core.
22. Core query channels/locks have explicit bounds and deadlines.
23. Snapshot construction does not block router progress on unbounded work.
24. Participating, exploratory, and client tunnel selectors are truthful and exact.
25. Tunnel queue selectors report canonical instantaneous gauges without mutation.
26. M004 unsupported definitions appear inactive and never runtime-capable.
27. Known peers use canonical stored peer state.
28. Active peers use live transport state.
29. Peer RouterInfo serialization is exact, bounded, and validated.
30. Ban, limit, and active-peer-stat selectors expose only protocol-required data.
31. IPv4/IPv6/network error/testing mappings are centralized and exact.
32. No RouterInfo request starts reachability testing, builds tunnels, changes bans, or mutates queues.
33. Complete results that exceed safe bounds fail explicitly and are never silently truncated.
34. M003 address-book selectors use the production administrative service, not files or runtime resolver state.
35. M004 I2PTunnel selectors use the production tunnel control service, not persistence files.
36. Handler/core errors are sanitized and contain no secrets or internal paths.
37. Headless and UI-enabled builds return equivalent results for the same router state.
38. Core remains free of HTTP/JSON-RPC server dependencies.
39. Continuous polling remains bounded and does not materially impair router progress.
40. Required protocol, core, restart, concurrency, security, and performance tests pass.

## 14. Stop conditions

The agent must stop and report rather than improvise when:

- M002 is not strictly closed;
- a selector in M001 lacks exact key/type/nullability semantics;
- a selector has no truthful existing or narrowly inspectable data source;
- implementing a selector would require changing router behavior rather than exposing state;
- clock skew would need to be invented from an unrelated measurement;
- complete peer/tunnel data cannot be returned within bounds and the proposal has no error path;
- a mutable NetDB/transport/tunnel/profile/SAM/I2CP handle would need to cross the boundary;
- an inspection query would need to consume EventSubscriber;
- a query would require a new router test/build/refresh action;
- private/session/key material would need to be exposed;
- a protocol extension such as pagination, truncation metadata, timestamps, or capabilities is proposed;
- M003/M004 production state is unavailable at closure time;
- work expands into missing tunnel implementation, runtime address-book adoption, frontend behavior, or broad core redesign.

The stop report must identify the selector, missing source, affected acceptance criteria, smallest read-only interface or ADR required, and why a placeholder is prohibited.

## 15. Closure evidence required

The later closure record must include:

- dependency closure references and reconciled baseline;
- implementation commits and reviewed head;
- requirement-to-evidence mapping for all acceptance criteria;
- complete selector manifest with no missing/extra rows;
- per-selector exact fixture evidence;
- only-requested-key evidence;
- startup identity/RI encoding evidence;
- router-news and clock-skew truthfulness evidence;
- log bounds/redaction/clear evidence;
- cumulative/rolling metrics and success-rate evidence;
- EventSubscriber non-interference evidence;
- core inspection API/source review and no-mutation evidence;
- tunnel/queue/network selector evidence;
- known/active/RI/ban/limit/peer-stat evidence;
- oversize no-truncation evidence;
- M003/M004 production adapter evidence;
- contention, cancellation, shutdown, and performance evidence;
- dependency review proving no server stack in core;
- secret/static guard evidence;
- exact commands and platform results;
- unrun limitations;
- unresolved findings by severity;
- roadmap and registry disposition.

Closure must be `corrective pass required` if any of these remains:

- missing or extra selector;
- wrong key/type/nullability;
- unrelated fields returned;
- fabricated zero/false/empty placeholder;
- unbounded or silently truncated result;
- EventSubscriber consumption;
- mutable core authority exposure;
- inspection side effect;
- secret/private material exposure;
- incorrect rolling-window or success-rate semantics;
- M003/M004 fake still used in production;
- frontend coupling;
- material polling regression;
- unresolved high/medium protocol, truthfulness, security, or concurrency finding.

## 16. Handoff notes

- This is the most core-sensitive milestone. Preserve the read-only boundary rigorously.
- Reconcile against actual M001/M002 code before naming modules or channel types.
- Prefer actor-owned bounded queries to new global locks.
- Copy protocol-required primitives, not internal structs.
- Keep Base64 encoding outside critical sections.
- Use deterministic time in rolling-window tests.
- Never solve an oversize response by silently returning fewer peers/logs/tunnels.
- Do not infer active from configured.
- Do not parse UI display strings into protocol codes.
- Do not make the log ring a second unbounded logging sink.
- Keep SAM session inspection sufficiently generic for M006 without implementing M006 here.
- The implementation pass moves registry status to `closing`, not `closed`; closure remains independent.
