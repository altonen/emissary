# I2PControl Proposal 170 Milestone 010 — Bounded Core Router Inspection

Status: blocked

Planning baseline: `b35d9ad7295f6d7d8795a741c7942a5ff7a97f52` (`master` before corrective planning commits)

Activation rule:

- M008 and M009 must each have a closure record with disposition `closed`.
- The M009 selector source map and internal availability/error interfaces must be treated as stable inputs.
- Before implementation, replace this baseline with the reviewed M009 head and reconcile every source-map row against current core ownership.

Source roadmap:

- `plans/subsystems/i2pcontrol-proposal-170-roadmap.md#milestone-010--bounded-core-router-inspection`

Corrects:

- unclosed production-data requirements from M005;
- medium-severity deferrals recorded in `plans/closure/i2pcontrol-proposal-170/005-closure.md`;
- RouterInfo portions of M007 strict closure.

Canonical requirements:

- `plans/000-long-term-specification.md`
- `plans/001-terminology-and-domain-model.md`
- `plans/002-long-term-roadmap.md`
- `plans/003-planning-process.md`
- `plans/adrs/ADR-0001-proposal-170-contract-and-stub-boundary.md`
- M009 selector source map and availability semantics

Primary class: capability / infrastructure corrective pass

## 1. Objective

Add the smallest bounded, read-only, neutral core inspection surfaces needed to back RouterInfo with actual Emissary state, then wire those snapshots through the M009 availability boundary without exposing subsystem authority, private material, unbounded collections, or frontend/event-stream dependencies.

The milestone closes the production inspection gap. It must not redesign router behavior merely to mimic Java I2P internal counters.

## 2. Scope boundary

### In scope

- Actual UDP and TCP transport availability/state.
- Actual connected/active peer identities and bounded transport statistics.
- Actual configured/effective transport or peer limits where Emissary owns equivalent values.
- Actual participating, exploratory, and client tunnel counts and queue depth where canonically observable.
- Actual NetDB/router/lease-set counts and bounded known-peer inventory where canonically observable.
- Exact bounded peer RouterInfo lookup and serialization.
- Actual ban-list snapshot where a canonical ban owner exists.
- Exact classification/source mapping for Proposal fields that do not have an Emissary semantic.
- Neutral core DTOs and read-only handles consumed by `emissary-cli` adapters.
- Production-path integration, contention, cancellation, and polling tests.

### Explicitly out of scope

- Changing peer selection or profile classification.
- Creating new peer tiers solely to populate Proposal fields.
- Changing NetDB storage, lookup, flooding, expiration, or persistence behavior.
- Changing transport session ownership or reconnect behavior.
- Changing tunnel construction, pool sizing, exploratory policy, or queue policy.
- Exposing private keys, session keys, lease-set private material, or mutable subsystem handles.
- Consuming the single-owner `EventSubscriber` or parsing logs for state.
- Implementing missing tunnel types or runtime services.
- Frontend changes.
- Runtime address-book integration.
- Adding Proposal 170 extensions, aliases, richer statuses, pagination, or partial-result envelopes.

## 3. Defects and missing sources

The M005 closure record acknowledged that several production sources were absent while still reporting the milestone closed. The baseline adapter currently fabricates successful state for at least:

- NetDB summary fields;
- known peers;
- active peers;
- peer RouterInfo lookup;
- banned peers;
- peer limits;
- active peer transport statistics;
- TCP transport snapshot;
- most UDP peer/category values;
- exploratory/client tunnel counts;
- tunnel queue depth;
- active I2PTunnel count.

M009 must already have changed those paths to explicit unavailable/error behavior. M010 supplies real snapshots where Emissary has a canonical equivalent and records precise unsupported semantics where it does not.

## 4. Why prior verification missed the gap

Prior work exposed a small set of event counters and treated a large DTO surface as if it implied a complete inspection implementation. Tests used fake DTOs and static source scans instead of starting canonical core owners with nonzero state and querying the real production adapter.

M010 evidence must begin at the core owner, mutate or construct real state through supported test seams, and prove the JSON-RPC response reflects that state without a second shadow store or fake adapter.

## 5. Invariants

- Core inspection types are neutral and contain no JSON-RPC or Proposal 170 wire names.
- Read-only inspection grants no mutation, cancellation, lifecycle, routing, or configuration authority.
- Snapshots are bounded before copying large collections.
- Counts may be complete even when an associated list is too large; a requested complete list must fail explicitly rather than truncate silently.
- Snapshot acquisition has explicit lock/queue/deadline behavior.
- No source lock is held during JSON serialization or while awaiting another subsystem.
- Private key and session-secret types cannot enter inspection DTOs.
- Peer RouterInfo serialization uses public RouterInfo bytes only.
- A source-reported empty collection is a real empty collection; absence of a source remains an M009 error.
- No event subscriber required by UI or another subsystem is consumed.
- No router behavior changes.
- No unsupported tunnel runtime becomes active.
- No frontend dependency enters core or CLI inspection.

## 6. Required architecture

### 6.1 Neutral inspection handle

Introduce a clonable read-only handle in `emissary-core`, or extend an existing neutral handle if one now exists.

Preferred conceptual shape:

```rust
pub struct RouterInspectionHandle<R: Runtime> {
    transport: TransportInspectionHandle<R>,
    tunnels: TunnelInspectionHandle<R>,
    netdb: NetDbInspectionHandle<R>,
    limits: RouterLimitSnapshot,
}
```

Exact internal fields must follow current ownership. The public surface should expose grouped bounded queries, not raw `Arc<Mutex<HashMap<...>>>` handles.

An equivalent trait-object design is acceptable if it is easier to test and does not leak runtime generics into `emissary-cli`.

### 6.2 Grouped neutral snapshots

Create only the groups required by the M009 source map.

#### Transport snapshot

May include, when canonically available:

- UDP enabled/active/bound status;
- TCP enabled/active/bound status;
- firewall/reachability status already cached by core;
- current connected peer count;
- bounded active peer IDs;
- bounded per-peer public transport statistics;
- public transport version/status strings if meaningful;
- configured/effective connection limits.

Do not invent UDP peer categories that Emissary does not track. Mark those source-map rows unsupported/unavailable instead of deriving them from unrelated counts.

#### Tunnel snapshot

May include:

- active participating count;
- exploratory inbound/outbound pool counts;
- client inbound/outbound pool counts;
- build queue depth;
- active locally owned client/server tunnel count if canonically observable;
- existing cumulative build success/failure counters.

A configured Proposal 170 definition count remains sourced from the shared M008 TunnelManager store. Do not merge configuration ownership with runtime pool ownership.

#### NetDB snapshot

May include:

- whether the NetDB subsystem is active;
- router-info count;
- lease-set count;
- bounded known router IDs;
- latest/highest router version if this can be computed from retained public RouterInfo data within bounds;
- public peer classifications already maintained by Emissary;
- public ban count/list if owned by NetDB or a shared ban service;
- exact serialized public RouterInfo lookup by validated peer ID.

Do not create Java-I2P-specific profile categories, reserve pools, or timestamps that have no Emissary equivalent. Such rows remain explicit unsupported inspection errors unless an accepted ADR defines a compatible semantic.

### 6.3 Bounded query contract

Each list-producing query must accept or internally enforce an exact maximum.

Preferred behavior:

```rust
pub enum BoundedSnapshotError {
    Unavailable,
    Busy,
    TimedOut,
    TooLarge { actual: usize, limit: usize },
    InvalidKey,
    Internal,
}
```

Requirements:

- count-only queries do not copy full collections;
- list queries check size before allocation where possible;
- no silent truncation;
- no unbounded sort;
- deterministic order where the wire contract or fixtures require it;
- bounded serialization of one peer RouterInfo;
- no blocking filesystem read on the async worker for each request.

### 6.4 Composition

The application creates one inspection handle from canonical router-owned components and injects it into the production RouterInfo adapter through the M008 production-controls object.

The handle must work identically in headless and UI-enabled modes.

Do not construct inspection state by reopening persistence files from `emissary-cli`.

## 7. Ordered work packages

### WP1 — Canonical owner inventory

Before adding APIs, map every M009 `core-inspection` row to current owners.

Inspect at minimum:

- `Router` and router context construction;
- `EventHandle` and existing atomic counters;
- transport manager/session ownership;
- UDP/SSU and TCP/NTCP listener/session ownership;
- inbound/outbound/exploratory/client tunnel pool ownership;
- tunnel build queue ownership;
- NetDB/router-info/lease-set ownership;
- ban-list ownership;
- configured/effective connection-limit ownership;
- SAM ownership only to avoid overlap with M011.

Produce a table with:

- selector group;
- owner type/module;
- current lock/channel/storage mechanism;
- safe snapshot strategy;
- bound;
- whether a truthful Emissary semantic exists;
- required production/test seam.

Stop if current repository changes have moved ownership materially; update the plan baseline/source map before coding.

### WP2 — Define neutral DTOs and errors

- Add core-neutral snapshots and bounded error types.
- Use public identifiers and public RouterInfo bytes only.
- Avoid serde wire renames tied to Proposal 170 in core.
- Add compile-time trait assertions for `Send + Sync` where required.
- Add negative type/static tests preventing private-key types in DTOs.

### WP3 — Transport inspection

- Add read-only current listener/session snapshot methods at the canonical transport owner.
- Collect active peer IDs and statistics without transferring session ownership.
- Expose actual enabled/active state, not configuration guesses.
- Expose limits only from active configuration/manager values.
- Ensure session removal cannot leave a stale peer permanently visible.
- Add deterministic fake/session fixtures at the transport layer.

### WP4 — Tunnel inspection

- Add read-only gauges for participating, exploratory, and client pools.
- Add queue-depth observation at the actual queue owner.
- Reuse existing build success/failure atomics.
- Define exact moment-in-time semantics for counts.
- Do not start or retain tunnels for inspection.
- Do not count unsupported Proposal 170 definitions as runtime active.

### WP5 — NetDB and peer inspection

- Add count-only snapshot operations.
- Add bounded known-peer identity listing.
- Add exact public RouterInfo lookup by validated ID.
- Add public classification fields only where already canonical.
- Add ban snapshot through the actual ban owner, if present.
- For rows with no meaningful Emissary source, keep M009 unsupported inspection behavior and document the rationale in the source map.

### WP6 — CLI adapter integration

- Extend the production event/inspection adapter or replace it with a composite neutral `CoreInspection` interface.
- Map neutral snapshots to existing Proposal 170 DTOs in `emissary-cli`.
- Preserve M009 error distinctions.
- Query each group once per request.
- Combine configured TunnelManager count from the shared M008 control with runtime tunnel gauges without hiding either failure.
- Remove any remaining hard-coded RouterInfo defaults.

### WP7 — Production-path tests and documentation

Add the tests below and update:

- `docs/i2pcontrol/router-info-source-map.md`;
- `docs/i2pcontrol/router-info.md`;
- `docs/i2pcontrol/inspection-architecture.md`;
- `docs/i2pcontrol/proposal-170-support.md`.

The source map must clearly distinguish:

- real Emissary value;
- protocol-defined nullable/unavailable value;
- unsupported Java-specific semantic returning an explicit error.

## 8. Failure, cancellation, restart, and contention semantics

### Source failure

- Closed/dropped core owners return `Unavailable`, not zero.
- Busy bounded query channels return `Busy`/sanitized error, not stale cached success unless the cache is explicitly canonical and freshness-bounded.
- Poisoned/invariant failures return internal errors and are logged without secret state.

### Cancellation

- A cancelled HTTP request drops its inspection future and releases permits.
- Inspection must not cancel router tasks or active sessions.
- No detached query task should continue allocating or sorting after cancellation.

### Restart

- Inspection handles are reconstructed from the new router instance.
- No process-global fallback directory or static state survives across instances.
- Empty startup state is reported as available-empty only after the owning subsystem is active and confirms it.

### Contention

- Use atomic gauges for hot scalar values when an existing canonical update point exists.
- For collection snapshots, copy under one bounded lock/read transaction and release promptly.
- Do not hold NetDB, transport, and tunnel locks simultaneously.
- Query deadlines must be shorter than the outer request deadline and produce explicit errors.
- Polling at moderate frequency must not materially affect router progress.

## 9. Required tests

### Core unit tests

For every new snapshot group:

- active nonzero state produces exact counts;
- active empty state produces exact zero/empty values;
- owner unavailable produces `Unavailable`;
- oversize list produces `TooLarge` without truncation;
- invalid peer ID is rejected before lookup;
- no private material appears in debug/serialized test representations;
- snapshot mutation after return cannot affect the owner.

### Lifecycle tests

- add/remove a transport session and observe active peers update;
- build/remove or install/remove tunnel pool state and observe gauges update;
- add/remove NetDB RouterInfo and lease-set entries and observe counts/list update;
- ban/unban a peer through supported internal test seams and observe snapshot update;
- restart owners and prove stale handles fail rather than report old state.

Do not add production mutation APIs solely for tests. Use existing constructors, mocks, or crate-private test fixtures.

### Production adapter tests

Use nonzero sentinel core state and prove exact mapping for:

- UDP/TCP active state;
- active peer count/list/stats;
- configured/effective limits;
- participating/exploratory/client tunnel counts;
- queue depth;
- NetDB/router/lease-set counts;
- known peer IDs;
- peer RouterInfo lookup;
- bans, where supported;
- shared configured I2PTunnel count plus runtime active count.

Also prove unavailable groups still return M009 errors.

### Real listener tests

Start the production-shaped I2PControl endpoint with a controlled core fixture:

1. authenticate through HTTPS;
2. request each corrected selector group;
3. verify only requested keys and exact nonzero values;
4. mutate supported fixture state;
5. request again and observe current state;
6. remove/stop the source and verify explicit error, not zero/default.

### Contention and polling tests

- concurrent RouterInfo polling while sessions/NetDB/tunnel gauges change;
- cancellation while a bounded collection snapshot is waiting;
- oversize response does not allocate or serialize an unbounded list;
- no deadlock under independently changing transport, tunnel, and NetDB state;
- UI/event subscriber tests continue receiving events while I2PControl polls.

### Static guards

- no `axum`, JSON-RPC, Proposal 170 key constants, or CLI DTO imports in `emissary-core`;
- no private-key/session-key fields in inspection DTOs;
- no mutable manager/task handle returned by inspection APIs;
- no `EventSubscriber` consumption in I2PControl inspection;
- no log parsing as a data source;
- no default-success fallback in production adapter.

## 10. Verification commands

```bash
cargo fmt --all -- --check
cargo check -p emissary-core --features std,events
cargo check -p emissary-cli --no-default-features --features i2pcontrol
cargo test -p emissary-core
cargo test -p emissary-cli --no-default-features --features i2pcontrol
cargo test -p emissary-cli --no-default-features --features i2pcontrol --test production_adapter
cargo test -p emissary-cli --no-default-features --features i2pcontrol --test router_info_truthfulness
cargo test -p emissary-cli --no-default-features --features i2pcontrol --test router_info_production
cargo test -p emissary-cli --no-default-features --features i2pcontrol --test static_guards
```

Run clippy for touched packages:

```bash
cargo clippy -p emissary-core --features std,events --all-targets -- -D warnings
cargo clippy -p emissary-cli --no-default-features --features i2pcontrol --all-targets -- -D warnings
```

If broader workspace/UI commands are unavailable, record the exact environmental blocker. Do not substitute fake-only tests for production-path evidence.

## 11. Acceptance criteria

1. Every M009 `core-inspection` source-map row is reconciled to a canonical owner or an explicit unsupported semantic.
2. Core exposes only neutral, bounded, read-only snapshot interfaces.
3. No JSON-RPC or Proposal 170 wire terminology enters core inspection types.
4. Actual UDP/TCP active state is reported from canonical transport ownership.
5. Actual active peer identities and bounded stats are reported where supported.
6. Actual configured/effective limits are reported from canonical configuration/manager state.
7. Actual participating, exploratory, and client tunnel gauges are reported where canonically observable.
8. Actual queue depth is reported from the queue owner where canonically observable.
9. Actual NetDB router/lease-set counts and bounded known-peer inventory are reported where supported.
10. Peer RouterInfo lookup distinguishes source failure, peer absence, and successful public serialization.
11. Actual bans are reported where a canonical ban owner exists; otherwise the selector remains explicit unsupported inspection, never empty success.
12. Java-specific or nonexistent Emissary classifications are not invented.
13. Configured Proposal 170 definitions and runtime-active tunnel state remain separate and truthful.
14. Unsupported tunnel definitions never count as runtime active.
15. List results are bounded and never silently truncated.
16. Snapshot queries release locks before JSON serialization and do not hold cross-subsystem locks.
17. Cancellation and owner shutdown release all inspection resources.
18. Moderate concurrent polling does not consume the UI/event subscriber or materially block router progress.
19. Production listener tests prove nonzero current state and explicit source-loss errors.
20. No hard-coded/default RouterInfo production source remains.
21. No router behavior, protocol, lifecycle ownership, frontend, runtime resolver, or missing tunnel data-plane change is introduced.
22. Documentation accurately distinguishes real values from explicit unsupported inspection semantics.
23. M005 may be reconsidered for strict closure only after an independent review of M008–M010 finds no high/medium defect.

## 12. Stop conditions

Stop and record a blocker rather than inventing semantics if:

- a Proposal selector describes Java-I2P state with no meaningful Emissary equivalent;
- obtaining a value would require changing routing, peer selection, profile classification, transport, tunnel, or NetDB behavior;
- obtaining a full list requires unbounded copying or pagination not permitted by the contract;
- the only available source is logs or a single-owner event receiver;
- public RouterInfo lookup would expose private material;
- the architecture requires lifecycle authority rather than inspection.

Keep the selector on M009 explicit unavailable/error behavior and document the reason.

## 13. Closure evidence required

The closure record must include:

- implementation commits;
- final core owner/source table;
- neutral snapshot type inventory;
- exact bounds and deadline table;
- before/after list of unavailable RouterInfo groups;
- nonzero production adapter and listener test output;
- source-loss and oversize test output;
- contention/cancellation/polling evidence;
- proof that UI/event subscribers remain unaffected;
- static-guard output;
- verification command outcomes;
- unsupported semantic rows with rationale;
- compatibility, security, and behavior review;
- unresolved findings by severity and disposition.

M012 remains blocked until this milestone and M011 are strictly closed.