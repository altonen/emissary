# I2PControl Proposal 170 Milestone 009 — RouterInfo Availability and Truthfulness

Status: blocked

Planning baseline: `b35d9ad7295f6d7d8795a741c7942a5ff7a97f52` (`master` before corrective planning commits)

Activation rule:

- M008 must have a closure record with disposition `closed`.
- The implementation agent must rebase the evidence inventory onto the reviewed M008 head before editing.

Source roadmap:

- `plans/subsystems/i2pcontrol-proposal-170-roadmap.md#milestone-009--routerinfo-availability-and-truthfulness`

Corrects:

- `plans/implementation/i2pcontrol-proposal-170/005-router-info-inspection.md`
- `plans/implementation/i2pcontrol-proposal-170/005-recovery.md`
- `plans/closure/i2pcontrol-proposal-170/005-closure.md`
- RouterInfo portions of M007 and its closure record

Canonical requirements:

- `plans/000-long-term-specification.md`
- `plans/001-terminology-and-domain-model.md`
- `plans/002-long-term-roadmap.md`
- `plans/003-planning-process.md`
- `plans/adrs/ADR-0001-proposal-170-contract-and-stub-boundary.md`
- `docs/i2pcontrol/proposal-170-conformance.md`

Primary class: invariant / capability corrective pass

## 1. Objective

Make RouterInfo distinguish actual zero/empty/false state from unavailable inspection state, eliminate every fabricated production default, and establish an exact selector-to-source map that M010 can use to add bounded real inspection without changing JSON-RPC handlers again.

This milestone repairs truthfulness and error semantics. It does not claim that all currently unavailable selectors have been populated; M010 owns the bounded core inspection implementation.

## 2. Defects being corrected

At the planning baseline, `ProductionRouterInfoControl` returns bare DTOs whose defaults are indistinguishable from real state. Examples include:

- zero exploratory/client tunnel counts and queue depth;
- a default NetDB snapshot;
- UDP marked active with multiple hard-coded fields;
- a default TCP snapshot;
- empty known-peer, active-peer, banned-peer, and active-peer-stat collections;
- `None` for every peer RouterInfo lookup;
- default peer limits;
- active I2PTunnel count fixed at zero;
- `unwrap_or(0)` when the shared tunnel service fails.

The current `RouterInfoControl` trait generally returns `T` or `Vec<T>`, so implementations cannot express the difference between:

- source available and legitimately zero/empty;
- source not wired;
- source temporarily unavailable;
- invalid selector input;
- source query failure;
- result exceeding a protocol/resource bound.

The handler therefore serializes placeholders as successful facts.

## 3. Why prior verification missed the defects

Prior tests asserted:

- selector registration and count;
- response key/type shape;
- default DTO serialization;
- production adapter construction;
- absence of panics and mutable handles.

They did not inject unavailable and failing sources or assert that the response differs from a legitimate empty snapshot. The closure record treated typed DTO existence as proof that canonical data sources were wired.

This milestone must add tests where zero/empty is a valid available result and compare it with unavailable/error behavior.

## 4. Invariants

- A successful RouterInfo value is derived from a named canonical source or a protocol-defined retained constant.
- Unavailable state never becomes a successful zero, empty collection, false boolean, empty string, or default struct unless that exact value is independently known to be true.
- Only requested selector keys appear on success.
- No partial success object is returned when a required requested selector fails and the protocol has no per-field error representation.
- Protocol-defined nullable fields use `null` only where the conformance matrix permits it.
- Non-null selectors fail with an existing JSON-RPC error envelope when their source is unavailable.
- No new public availability/status field is introduced.
- Core inspection remains bounded, immutable, read-only, and secret-free.
- No single-owner event subscriber is consumed.
- No router, NetDB, transport, tunnel, peer-selection, or congestion behavior changes.
- No missing tunnel data plane is implemented.
- No frontend or runtime address-book ownership change occurs.

## 5. Explicit non-goals

- Implementing the core data sources identified as missing; M010 owns that work.
- Changing Proposal 170 selector names, response fields, JSON types, or nullability to accommodate Emissary.
- Returning implementation-specific status objects.
- Adding pagination or silently truncating peer/profile/session data.
- Treating `Default::default()` as a generic unavailable sentinel.
- Broad core refactoring.

## 6. Required source and availability model

### 6.1 Selector source map

Create `docs/i2pcontrol/router-info-source-map.md` or an equivalent machine-readable source owned by the conformance manifest.

Every RouterInfo selector must have exactly one row with:

- exact wire key;
- output JSON type;
- protocol nullability;
- semantic definition;
- canonical Emissary owner/source;
- snapshot group;
- maximum collection/result bound;
- current availability at the reviewed M008 head;
- unavailable/error behavior;
- fixture/test ID;
- M010 work-package owner if real inspection is still missing.

Use these source classes:

1. `retained` — identity, serialized local RouterInfo, version, configured values, startup time.
2. `event-metric` — existing atomic counters or cached statuses.
3. `administrative-store` — shared AddressBook or TunnelManager state.
4. `core-inspection` — bounded runtime/NetDB/transport/tunnel snapshot required from M010.
5. `protocol-defined-empty` — only where the protocol semantics explicitly define absence as an empty value, not merely because Emissary lacks a source.
6. `nullable-unavailable` — only where the matrix permits null.
7. `unsupported-inspection` — selector implemented and validated, but no truthful source exists yet; request fails explicitly until M010 or a documented architecture decision supplies one.

Do not classify a selector as `protocol-defined-empty` without a cited contract rationale.

### 6.2 Internal error vocabulary

Add a private typed error, for example:

```rust
pub enum InspectionError {
    Unavailable { group: InspectionGroup },
    TemporarilyUnavailable { group: InspectionGroup },
    QueryFailed { group: InspectionGroup },
    ResultTooLarge { group: InspectionGroup, limit: usize },
    InvalidPeerId,
    InternalInvariant,
}
```

Names may differ. Requirements:

- no secrets or arbitrary internal paths in `Display` output used for clients;
- source errors may be retained for internal logs;
- no string parsing to identify error classes;
- exact mapping to existing JSON-RPC error codes/envelopes;
- no new wire fields.

### 6.3 Change `RouterInfoControl` to preserve availability

Bare-return methods must become fallible or explicitly nullable.

Examples:

```rust
async fn tunnel_summary(&self) -> Result<TunnelSummary, InspectionError>;
async fn known_peers(&self) -> Result<Vec<PeerIdentity>, InspectionError>;
async fn peer_router_info(
    &self,
    peer_id: &str,
) -> Result<Option<String>, InspectionError>;
```

`Option` must retain its semantic meaning. For peer RouterInfo, `Ok(None)` means the source was queried successfully and that peer is not present. It must not mean the source is not wired.

For clock skew, retain the protocol-permitted `Option<i64>` distinction if the conformance matrix confirms it.

### 6.4 Grouped request consistency

Map selectors to bounded snapshot groups so multiple fields from the same subsystem come from one coherent query per request:

- retained/router identity group;
- network/reachability group;
- UDP transport group;
- TCP transport group;
- traffic/build metrics group;
- tunnel summary group;
- NetDB summary group;
- known/active peer group;
- peer RouterInfo lookup group;
- ban/limit/stat group;
- administrative I2PTunnel group;
- log group;
- address-book group.

The handler should not query the same group independently for every individual key. It may lazily fetch a group only when at least one selector in that group is requested.

### 6.5 Wire behavior

For requested selectors:

- all source groups succeed: return exact success result with only requested keys;
- protocol-nullable value unavailable: return the permitted null value;
- non-null group unavailable/failing/oversize: return one sanitized JSON-RPC error response and no partial `result`;
- invalid peer ID: return existing invalid-params behavior;
- peer source available but peer absent: return the exact contract-specific absent/null behavior.

Do not return an empty object as a generic fallback.

## 7. Ordered work packages

### WP1 — Reconcile the conformance matrix

- Inventory all RouterInfo constants and handler match arms.
- Compare them to `docs/i2pcontrol/proposal-170-conformance.md` and the executable manifest.
- Add the source/availability fields from section 6.1.
- Identify duplicate or contradictory rows.
- Correct documentation errors only when supported by the Proposal 170/base I2PControl contract; do not weaken requirements to match current code.
- Record every selector currently backed by a fabricated/default production value.

Expected output: a complete selector source map with no `unknown` source row.

### WP2 — Add typed inspection errors

- Define the internal error and snapshot-group vocabulary.
- Add sanitized handler mapping.
- Add source-preserving internal logging.
- Ensure errors contain no token, password, private key, RouterInfo private material, arbitrary file contents, or backtrace.

### WP3 — Refactor the trait and fake adapters

- Change `RouterInfoControl` methods to preserve errors and semantic absence.
- Update `FakeRouterInfoControl` so tests must explicitly configure each requested snapshot group.
- A fake must default to `Unavailable`, not a successful default DTO.
- Provide concise fixture builders for available-zero, available-nonzero, unavailable, failure, and oversize states.

Example fixture intent:

```rust
let control = FakeRouterInfoControl::builder()
    .tunnel_summary(Ok(TunnelSummary {
        active_participating: 0,
        configured: 0,
        exploratory_inbound: 0,
        exploratory_outbound: 0,
        client_inbound: 0,
        client_outbound: 0,
        queue_depth: 0,
    }))
    .build();
```

The explicit fixture proves that returned zeros are known facts rather than constructor defaults.

### WP4 — Remove production placeholders

In `ProductionRouterInfoControl`:

- remove default NetDB/TCP snapshots;
- remove hard-coded UDP active/cookie/hidden/peer values;
- remove empty peer/ban/stat vectors used for unavailable sources;
- remove default peer limits;
- remove zero tunnel fields not backed by a source;
- remove `unwrap_or(0)` and other error suppression;
- retain only values backed by M008-shared services, retained configuration, logs, and existing real event metrics.

For source groups awaiting M010, return typed `Unavailable` rather than success.

A production method may return a real empty/zero only after its source query succeeds and reports empty/zero.

### WP5 — Handler grouped dispatch

- Add lazy group loading keyed by requested selectors.
- Query each group at most once per request.
- Abort without partial result on non-null source failure.
- Preserve only-requested-key behavior.
- Preserve existing auth/version checks before inspection.
- Preserve response-size enforcement before serialization.

### WP6 — Tests, guards, and documentation

Add focused unit and listener-level tests described below. Update:

- `docs/i2pcontrol/router-info.md`;
- `docs/i2pcontrol/inspection-architecture.md`;
- `docs/i2pcontrol/proposal-170-conformance.md`;
- `docs/i2pcontrol/proposal-170-support.md`.

Do not describe M005 as closed until M010 supplies and verifies the required real sources.

## 8. Failure, cancellation, restart, and contention semantics

### Failure

- Source unavailable is distinguishable from source query failure internally.
- Both map to sanitized protocol errors for non-null selectors.
- A failure in one requested group prevents a partial success object.
- A failure in an unrequested group has no effect and must not trigger work.

### Cancellation

- Cancelling a RouterInfo request cancels or drops only its bounded snapshot work.
- No shared runtime state is mutated.
- No permit, lock, or query task remains orphaned.

### Restart

- Availability derives from production dependencies reconstructed at startup.
- Restart must not change unavailable state into zero merely because a cache is initially empty.
- Retained identity/configuration values remain stable according to existing semantics.

### Contention

- Do not hold one subsystem lock while awaiting another subsystem query.
- Group snapshots copy bounded immutable data and release source locks before JSON assembly.
- Multiple selectors in one group use one snapshot.
- Continuous polling remains bounded by the existing request semaphore plus per-group limits.

## 9. Required tests

### Available-zero versus unavailable

For each scalar/list snapshot category, prove both cases:

- available source reports zero/empty and success contains zero/empty;
- unavailable source returns a JSON-RPC error and no `result`.

At minimum cover:

- tunnel summary;
- NetDB summary;
- UDP/TCP snapshot;
- known peers;
- active peers;
- banned peers;
- peer limits;
- active peer stats;
- I2PTunnel stats.

### Failure and absence distinction

- shared TunnelManager query error does not become configured count zero;
- peer source failure does not become peer not found;
- successful peer lookup with no record uses the exact absent behavior;
- oversize list fails explicitly and is not truncated;
- an unavailable unrequested group does not affect successful requested groups.

### Group consistency

Use a fake source that increments a generation on each query:

- request several selectors in one group;
- prove all fields use one generation;
- prove the group is queried exactly once;
- prove unrelated groups are not queried.

### Exact wire tests

- success contains only requested keys;
- failure contains `error`, not `result`;
- no implementation-specific status field appears;
- nullable clock skew or other permitted nulls serialize exactly;
- error messages are sanitized and stable.

### Static guards

Target production RouterInfo implementation and fail on unjustified uses of:

- `NetDbSnapshot::default()`;
- `TcpSnapshot::default()`;
- `Vec::new()` as an unavailable source response;
- `PeerLimits::default()`;
- `active_count: 0` without a source;
- `unwrap_or(0)`;
- comments claiming defaults are truthful absence.

Use an allowlist for explicit, independently sourced real-zero construction if needed. A broad text scan that can be bypassed by renaming is insufficient; combine it with behavioral tests.

## 10. Verification commands

```bash
cargo fmt --all -- --check
cargo check -p emissary-core --features std,events
cargo check -p emissary-cli --no-default-features --features i2pcontrol
cargo test -p emissary-cli --no-default-features --features i2pcontrol
cargo test -p emissary-cli --no-default-features --features i2pcontrol --test production_adapter
cargo test -p emissary-cli --no-default-features --features i2pcontrol --test static_guards
cargo test -p emissary-cli --no-default-features --features i2pcontrol --test router_info_truthfulness
```

Run clippy on touched packages/targets:

```bash
cargo clippy -p emissary-cli --no-default-features --features i2pcontrol --all-targets -- -D warnings
```

Record exact command outcomes. Do not count tautological assertions, compilation, or static source presence as behavioral closure evidence.

## 11. Acceptance criteria

1. Every RouterInfo selector has one exact source-map row.
2. Every row defines type, nullability, source group, bound, and unavailable behavior.
3. `RouterInfoControl` can distinguish unavailable, failed, absent, and available-zero/empty states.
4. Fake controls default to unavailable rather than successful defaults.
5. Production RouterInfo contains no fabricated default source group.
6. Existing real metrics, retained values, shared stores, and logs remain correctly exposed.
7. A requested non-null unavailable selector returns a sanitized JSON-RPC error with no partial result.
8. A protocol-nullable unavailable selector uses null only where explicitly permitted.
9. Legitimate zero/empty state still returns successful zero/empty values.
10. Peer source failure is distinct from peer absence.
11. Shared TunnelManager failure is distinct from configured count zero.
12. Each requested snapshot group is queried at most once per request.
13. Unrequested unavailable groups perform no work and do not fail the request.
14. Only requested keys appear on success.
15. No new wire fields, aliases, statuses, methods, or error objects are introduced.
16. Snapshot work remains bounded, cancellation-safe, read-only, and frontend-independent.
17. Tests would fail on every placeholder behavior identified in section 2.
18. Documentation explicitly marks source groups awaiting M010 as unavailable rather than implemented.
19. No router/core behavior or missing tunnel data plane is added.
20. Closure reports no unresolved high/medium defect within the availability/error-semantics boundary.

## 12. Stop conditions

Stop and record a blocker if:

- the external contract requires a success value where no truthful Emissary semantic exists and no existing error/null behavior is compatible;
- source-map reconciliation reveals Proposal 170 ambiguity that needs an ADR;
- changing trait errors requires public protocol expansion;
- the implementation starts inventing counters or classifications solely to satisfy field names;
- core mutation or algorithm changes would be required.

Do not reintroduce defaults to avoid a blocker.

## 13. Closure evidence required

The closure record must include:

- implementation commits;
- complete selector source map;
- list of all removed placeholder paths;
- available-zero versus unavailable test output;
- grouped-query consistency evidence;
- exact listener-level error/success fixtures;
- static-guard output;
- verification commands and outcomes;
- protocol compatibility review;
- unresolved source groups assigned to M010;
- findings by severity and disposition.

M010 and M011 may activate only after this milestone is strictly closed and the source/availability interfaces are stable.