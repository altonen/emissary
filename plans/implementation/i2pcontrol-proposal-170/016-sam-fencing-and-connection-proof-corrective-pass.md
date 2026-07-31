# I2PControl Proposal 170 Milestone 016 — Bounded SAM Session Observation Corrective Pass

Status: ready

Amended planning baseline: `93f96fef0e97447c77051922cf5a22495148b456`

Source roadmap:

- `plans/subsystems/i2pcontrol-proposal-170-roadmap.md`

Canonical references:

- `plans/adrs/ADR-0001-proposal-170-contract-and-stub-boundary.md`
- `docs/i2pcontrol/proposal-170-conformance.md`
- I2P Proposal 170 `ClientServicesInfo`
- pinned i2pd commit `7866f644d3d3dea3d1adf5374a6ea378c8efd536`, `daemon/I2PControl.cpp`, `SAMInfoHandler`
- `plans/closure/i2pcontrol-proposal-170/016-closure.md` as the superseded pre-amendment blocker record

Primary class: narrow remaining correctness implementation

## 1. Amendment decision

The architecture owner explicitly authorizes one bounded SAM session-observation handle.

This amendment resolves the ownership stop condition recorded by the earlier M016 blocker. It does not authorize a general event system, session registry, polling task, lifecycle API, or broader SAM redesign.

The two non-SAM M016 findings are already complete at `9047feecde046dac8e0208bbf1acf2e3883f97ae`:

- same-category service generation validation and entry replacement are atomic;
- the TLS connection-bound regression now proves saturation, rejection, and permit restoration.

M016 now owns only the active SAM session-map correction and its directly required composition and tests.

## 2. Objective

Expose the exact adopted Proposal 170/i2pd SAM session map through one fixed-capacity, read-only observation handle owned by the canonical `SamServer` runtime.

The completed implementation must ensure:

- `sessions: {}` means there are genuinely zero active observable sessions;
- active sessions appear on the next `ClientServicesInfo` request;
- removed sessions disappear on the next request;
- the handle exposes no private key material, payloads, mutable session handles, authentication data, or lifecycle authority;
- the response remains bounded and fails rather than truncating or fabricating state;
- no other Proposal 170 method or subsystem changes.

## 3. Exact remaining finding

| ID | Finding | Severity | Required disposition |
|---|---|---|---|
| M016-F1 | A listening SAM bridge can return successful `sessions: {}` even when active sessions exist because I2PControl has no canonical bounded read source | medium | implement the bounded observation handle and exact adopted session map |

M016-F2 and M016-F3 are resolved and must not be reimplemented.

## 4. Scope boundary

### 4.1 Allowed core files

Use only the smallest files required to publish current SAM observation state:

- `emissary-core/src/sam/mod.rs`
- `emissary-core/src/sam/session.rs`
- one existing SAM stream/socket module only if required to derive the adopted non-sensitive socket summary
- the smallest router/composition module that constructs and moves `SamServer` into the runtime

### 4.2 Allowed CLI files

- `emissary-cli/src/i2pcontrol/client_services.rs`
- the smallest existing I2PControl DTO/control trait file required by the session map
- `emissary-cli/src/main.rs` or the existing composition file that passes the handle into I2PControl

### 4.3 Allowed tests and documentation

- existing SAM core unit tests
- `emissary-cli/tests/client_services_live.rs`
- `emissary-cli/tests/client_services_integration.rs`
- directly affected I2PControl documentation
- Proposal 170 planning, registry, and closure records

### 4.4 Explicitly forbidden

Do not:

- modify `.github/workflows/**` or add CI, nightly, matrix, coverage, release, publishing, or evidence machinery;
- reformat unrelated files;
- change SAM protocol behavior, command parsing, tunnel creation, routing, stream semantics, listener ownership, or session lifecycle;
- add a generic observer framework, global registry, event bus, polling task, persistence store, or process-wide cache;
- expose private destinations, private keys, lease-set material, payloads, command channels, sockets, stream objects, or mutable handles;
- add Proposal 170 fields, aliases, partial-result envelopes, pagination, or Emissary-specific statuses;
- touch router, transport, NetDB, tunnel-pool, frontend, resolver, cryptographic, or general security architecture;
- revisit the completed service-fencing or TLS connection-limit design absent a direct regression.

## 5. Required architecture

### 5.1 Ownership model

Create one SAM-specific observation pair:

```rust
pub struct SamSessionObservationHandle {
    inner: Arc<RwLock<SamSessionObservationState>>,
}

struct SamSessionObservationPublisher {
    inner: Arc<RwLock<SamSessionObservationState>>,
}
```

Equivalent naming is acceptable.

Rules:

- the read handle is clonable and exposes snapshot reads only;
- the publisher remains private to the SAM implementation;
- neither side can start, stop, modify, message, or otherwise control a session;
- the observation state is created with `SamServer`;
- clone the read handle before `SamServer` is moved into the router runtime;
- pass that clone through the existing composition root into I2PControl;
- do not make `SamServer` itself shared or lock the server future.

A constructor may return `(SamServer<R>, SamSessionObservationHandle)` or `SamServer` may expose a one-time cloneable `observation_handle()` before it is moved. Choose the smaller change for current composition.

### 5.2 Fixed bounded state

Use a fixed maximum equal to the existing I2PControl SAM session limit, or establish one shared constant used by both core observation and CLI serialization.

The state must contain only the adopted output metadata:

```rust
struct SamSessionObservationState {
    sessions: BTreeMap<Arc<str>, SamObservedSession>,
    overflowed: bool,
    generation: u64,
}
```

`HashMap` plus deterministic sorting during a bounded snapshot is acceptable. Do not sort or clone an unbounded collection.

When the session or socket bound would be exceeded:

- do not silently omit entries;
- do not return a partial map;
- set a bounded overflow/error state or reject the observation update in a way that makes the next snapshot fail explicitly;
- map that failure through the existing I2PControl application/internal error path, without adding a wire field.

### 5.3 Exact observed DTO

The serialized shape must match the pinned i2pd `SAMInfoHandler` behavior exactly:

- map key: the adopted session identifier;
- per-session fields: only the pinned `name`, `address`, and `sockets` fields;
- each socket entry: only the pinned `type` and `peer` fields.

Before coding, record the exact i2pd source expressions used for each field. Do not infer or substitute a different Emissary concept merely because the field name is similar.

If an adopted field has no safe Emissary equivalent:

- stop and record that exact field-level blocker;
- do not invent a placeholder, empty string, zero value, or alternate identifier;
- do not broaden the handle to expose sensitive internal objects.

### 5.4 Lifecycle updates

Update the observation state only at existing canonical lifecycle transitions.

Required transitions:

1. Insert a session only when it becomes active, not while pending.
2. Remove it when the active session future completes or is otherwise removed from `active_sessions`.
3. Add or update socket summaries at the existing point where the corresponding adopted socket becomes active.
4. Remove socket summaries at the existing close/removal transition.
5. Sub-session behavior must follow the pinned i2pd representation exactly.

Do not add a background reconciler. The writer calls must be small synchronous metadata updates adjacent to existing lifecycle mutations.

### 5.5 Locking and runtime behavior

- never hold the observation lock across `.await`;
- never hold it while performing network, tunnel, storage, or cryptographic work;
- snapshot under a read lock, clone only the bounded DTO, release the lock, then serialize in I2PControl;
- writer failure must not crash or stall the SAM data plane;
- observation code must not alter ordering or success/failure of SAM protocol operations;
- no lock is exposed outside the observation module.

## 6. Ordered work packages

### WP1 — Freeze field semantics

Using the already pinned Proposal 170 and i2pd commit:

1. Record the exact session map key.
2. Record exact sources and meanings for `name` and `address`.
3. Record socket `type` vocabulary and `peer` representation.
4. Record zero-session and disabled-SAM behavior.
5. Record sub-session treatment.
6. Define session and per-session socket bounds.

This note belongs in the implementation commit or directly affected documentation, not a new project-wide specification.

### WP2 — Add the bounded core observation pair

Implement the read handle, private publisher, bounded DTO, overflow/error behavior, and focused core tests.

The read API should be one method such as:

```rust
pub fn snapshot(&self) -> Result<SamSessionSnapshot, SamObservationError>;
```

Do not expose individual mutable lookup or update methods on the read handle.

### WP3 — Wire existing lifecycle transitions

Add publisher updates only where `SamServer` and `SamSession` already create, activate, update, and remove the relevant state.

Do not duplicate session ownership. `active_sessions` and the existing session futures remain authoritative; the observation state is a bounded administrative projection.

### WP4 — Pass the handle through composition

Clone the observation handle before the server is moved into the router runtime. Pass it through the existing router/CLI composition path into the I2PControl production state.

Requirements:

- production startup fails clearly if I2PControl is enabled but the required handle is unexpectedly absent;
- tests may use a bounded fake handle;
- do not create a second empty production handle inside I2PControl;
- do not add global state.

### WP5 — Serialize exact `ClientServicesInfo.SAM`

When `SAM` is requested:

- preserve the existing listener-derived `enabled` value;
- if disabled, return the exact disabled response defined by the contract;
- if listening, snapshot the observation handle on demand;
- serialize every bounded current session exactly;
- return `{}` only for a genuine zero-session snapshot;
- convert observation overflow/failure to the existing JSON-RPC application/internal error path.

Do not query or clone session state when the `SAM` selector is absent.

### WP6 — Add falsifiable regressions

Required tests:

1. SAM disabled returns the exact disabled response.
2. SAM listening with zero sessions returns a genuine empty map.
3. One active session appears with the exact adopted fields.
4. Session removal is visible on the next request.
5. Socket addition/removal is visible when the adopted map includes sockets.
6. Sub-session mapping matches pinned i2pd behavior.
7. Overflow produces an error, never truncation or empty success.
8. Snapshot contains no prohibited material.
9. A request without the `SAM` selector does not read the observation handle.
10. Existing atomic fencing and connection saturation tests remain passing.

Prefer deterministic unit tests around the publisher/handle plus one production-composition or integration test. Do not build a network interoperability farm.

### WP7 — Update direct documentation and freeze the head

Update only SAM capability/source claims and planning state.

After implementation and required tests pass:

- freeze the implementation head;
- change M016 from `ready` to `closing`;
- change M017 from `blocked` to `ready`;
- create a new M016 closure/evidence record or clearly supersede the old blocker record;
- do not rewrite M015 into passing evidence.

## 7. Verification

Run locally:

```bash
cargo fmt --all -- --check
cargo check -p emissary-core --features std,events
cargo test -p emissary-core
cargo clippy -p emissary-core --features std,events --all-targets -- -D warnings
cargo check -p emissary-cli --no-default-features --features i2pcontrol
cargo test -p emissary-cli --no-default-features --features i2pcontrol
cargo clippy -p emissary-cli --no-default-features --features i2pcontrol --all-targets -- -D warnings
```

If repository-wide formatting remains blocked by unrelated baseline files, record that honestly and run `rustfmt +nightly --check` only on touched Rust files. Do not reformat the repository.

No remote CI evidence is required.

## 8. Acceptance criteria

M016 is complete only when:

1. The exact pinned i2pd session key and field meanings are documented.
2. One bounded read-only handle is owned by canonical SAM runtime composition.
3. The publisher remains private and carries no lifecycle authority.
4. The handle is cloned before `SamServer` is moved into the router runtime.
5. Active sessions appear only after activation and disappear after removal.
6. Socket summaries track the exact adopted lifecycle and vocabulary.
7. `sessions: {}` represents a genuine zero-session snapshot only.
8. Overflow or source failure returns an existing error rather than partial, stale, or empty success.
9. Session and socket collections are bounded before response construction.
10. No private keys, private destination material, payloads, authentication data, command channels, sockets, stream objects, or mutable handles are exposed.
11. No SAM protocol, routing, tunnel, listener, or session-lifecycle behavior changes.
12. No Proposal 170 wire extension is added.
13. No generic registry, cache, event bus, polling task, supervisor, or persistence layer is introduced.
14. No unrelated core, CLI, UI, router, transport, NetDB, tunnel, cryptographic, security, CI, release, or formatting changes enter the diff.
15. Required core and CLI tests pass.
16. Previously completed atomic fencing and connection-bound regressions remain passing.
17. Documentation no longer describes unobservable active sessions as empty-by-contract.
18. The implementation freezes a head, moves M016 to `closing`, and activates only M017.

## 9. Stop conditions

Stop rather than broaden scope if:

- a pinned output field cannot be mapped without exposing sensitive material;
- exact socket summaries require sharing live socket or stream objects rather than sanitized metadata;
- implementation requires a background task, generic event system, global registry, persistence, or SAM lifecycle redesign;
- the observation lock would need to span async or network work;
- more than the narrow SAM files and composition seams above are required;
- a new dependency, workflow, or release mechanism is proposed.

Record the precise field or ownership blocker and leave M017 blocked.

## 10. Reliable handoff sequence

1. Freeze the exact i2pd field mapping.
2. Add the bounded DTO and read/publisher pair.
3. Wire session activation/removal.
4. Wire only the adopted socket transitions.
5. Clone and pass the read handle through composition.
6. Replace empty compatibility output with current bounded serialization.
7. Add focused unit and one integration regression set.
8. Run targeted core and CLI verification.
9. Update direct docs and freeze the M016 implementation head.
10. Move M016 to `closing` and M017 to `ready`.

Do not combine M016 implementation and M017 independent closure.