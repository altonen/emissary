# I2PControl Proposal 170 Milestone 018 — Exact Wire-Contract Reconciliation

Status: closing

Planning baseline: `2816857633a927b629c051e07e7efa5baa8d6e07`

Pinned normative source:

- I2P Proposal 170, `I2PControl Expansion`
- status: `Open`
- created: `2026-05-20`
- last updated: `2026-05-20`
- canonical page: `https://i2p.net/en/proposals/170-i2pcontrol-expansion/`

Source roadmap:

- `plans/subsystems/i2pcontrol-proposal-170-roadmap.md`

Primary class: protocol-contract corrective pass

## 1. Why this milestone exists

M016 correctly implemented bounded current SAM observation. M017 nevertheless accepted a broader closure claim that is not supported by the current official Proposal 170 wire contract.

The repository currently contains useful administrative behavior, but several public request and response shapes differ materially from Proposal 170:

1. the RouterInfo registry labels 121 legacy/base/implementation-specific keys as Proposal 170 while several exact Proposal 170 keys are absent or renamed;
2. AddressBook uses `book`/`request`/`name`/`value` action-style parameters and separate `SetSubscriptions` and `SetConfig` methods instead of Proposal 170's `Type`/`Hostname`/`Destination`/optional `Delete` and in-method `SetSubscriptions`/`SetConfig` modes;
3. TunnelManager accepts capitalized actions, adds a `List` action, and returns bare strings or implementation-specific shapes instead of the proposal's lowercase actions and structured result objects;
4. ClientServicesInfo requires a nested boolean `Selector` object instead of selecting services by direct parameter presence with any value;
5. the closure and support documents conflate wire recognition, live source availability, and runtime backend implementation.

These are protocol defects. They are not solved by more router inspection infrastructure, more tunnel data planes, broader security hardening, or expanded CI.

## 2. Objective

Reconcile Emissary's I2PControl surface with the exact Proposal 170 revision pinned above while retaining safe backward compatibility for already-shipped Emissary extensions.

The completed pass must:

- recognize exact Proposal 170 method parameter names, selector names, casing, and presence semantics;
- return exact Proposal 170 response fields and JSON types where the proposal is unambiguous;
- explicitly adjudicate proposal ambiguities against the linked reference implementation or a recorded architecture-owner decision;
- preserve existing Emissary request forms only as documented compatibility extensions, never as the canonical Proposal 170 contract;
- keep unavailable router data and unsupported tunnel runtimes truthful rather than fabricating values;
- add official-example protocol fixtures without introducing a new test framework;
- update documentation so `wire implemented`, `source available`, and `runtime implemented` are separate claims.

## 3. Exact findings owned by M018

| ID | Finding | Severity | Required disposition |
|---|---|---|---|
| M018-F1 | Exact Proposal 170 RouterInfo keys such as `i2p.router.id`, `i2p.router.clockskew`, and `i2p.router.info` are absent or replaced by different names | high contract defect | add exact canonical keys and classify existing names as compatibility aliases or unrelated legacy keys |
| M018-F2 | The repository's `121 Proposal 170 selectors` claim combines legacy/base keys with Proposal 170 additions | medium documentation/coverage defect | create an exact 43-key Proposal 170 addition manifest and report legacy keys separately |
| M018-F3 | AddressBook request modes and parameter names do not match Proposal 170 | high contract defect | implement exact Proposal 170 modes inside `AddressBook`; retain old form only as compatibility extension |
| M018-F4 | TunnelManager action casing and result envelopes do not match Proposal 170; `List` is an extension | high contract defect | implement lowercase canonical actions and structured results; classify `List` and capitalized actions as extensions |
| M018-F5 | ClientServicesInfo requires nested boolean selectors instead of direct parameter presence with any value | high contract defect | implement exact direct-parameter mode; retain nested form only as compatibility extension |
| M018-F6 | No true end-to-end SAM session test reaches ClientServicesInfo through production composition | medium evidence defect | add one bounded real-session integration path or record a precise environmental blocker |
| M018-F7 | M017 claims zero unresolved high/medium findings and complete Proposal 170 closure | closure defect | preserve M017 as invalidated history; supersede it with M019 |

No router algorithm, transport, NetDB, tunnel data-plane, frontend, release, or CI finding is activated by this milestone.

## 4. Scope boundary

### 4.1 Primary allowed production files

Production edits should remain within existing I2PControl files:

- `emissary-cli/src/i2pcontrol/rpc.rs`
- `emissary-cli/src/i2pcontrol/server.rs`
- `emissary-cli/src/i2pcontrol/router_info.rs`
- `emissary-cli/src/i2pcontrol/router_info_handler.rs`
- `emissary-cli/src/i2pcontrol/address_book.rs`
- `emissary-cli/src/i2pcontrol/tunnel_manager.rs`
- `emissary-cli/src/i2pcontrol/client_services.rs`
- existing I2PControl domain/control files required to serialize exact results

Minimal composition changes are allowed only when an existing production source is already available but not passed into the exact handler.

### 4.2 Tests and documentation

Allowed:

- existing I2PControl unit and integration suites;
- one compact contract fixture module or static manifest inside the existing test structure;
- `docs/i2pcontrol/**`;
- Proposal 170 plans, registry, roadmap, and closure records.

### 4.3 Core exception

No new core observation seam is expected.

A core edit is permitted only when:

- the exact official key already has a safe canonical Emissary source;
- the source cannot be read through an existing handle;
- the change is one small bounded read-only accessor;
- M018 records the exact key and field requiring it before the edit.

Do not use this exception to implement currently unavailable transport, NetDB, peer, or tunnel inspection groups.

### 4.4 Explicitly forbidden

M018 must not:

- modify `.github/workflows/**`;
- add CI jobs, matrices, nightly checks, coverage gates, generated evidence bundles, or release automation;
- implement missing tunnel data planes;
- redesign router, transport, NetDB, tunnel-pool, peer-selection, cryptographic, resolver, frontend, SAM, or I2CP architecture;
- add generic inspection, observation, task-supervision, protocol-generation, schema-generation, or compatibility frameworks;
- remove existing Emissary compatibility forms without an explicit migration decision;
- fabricate unavailable selector values to make fixtures pass;
- perform repository-wide formatting;
- add dependencies unless an existing parser/serializer cannot express the exact JSON shape. A dependency proposal is a stop-and-review event.

## 5. Contract-freeze work package

### WP1 — Create the exact pinned contract inventory

Before changing handlers, add a compact machine-checkable inventory for the pinned revision.

The inventory must separate:

1. base I2PControl methods already implemented;
2. the exact 43 RouterInfo keys added by Proposal 170;
3. AddressBook operation modes and parameter names;
4. TunnelManager canonical actions, tunnel types, common option names, and result fields;
5. ClientServicesInfo direct parameter keys and response shapes;
6. Emissary compatibility aliases/extensions.

The exact 43 RouterInfo additions are:

```text
i2p.router.news
i2p.router.id
i2p.router.clockskew
i2p.router.info
i2p.router.logs
i2p.router.logs.clear
i2p.router.net.total.received.bytes
i2p.router.net.total.sent.bytes
i2p.router.net.total.transit.bytes
i2p.router.net.bw.transit.15s
i2p.router.net.tunnels.shareratio
i2p.router.net.tunnels.participating.info
i2p.router.net.tunnels.i2ptunnel
i2p.router.net.tunnels.exploratory.inbound
i2p.router.net.tunnels.exploratory.outbound
i2p.router.net.tunnels.exploratory.info.list
i2p.router.net.tunnels.client.inbound
i2p.router.net.tunnels.client.outbound
i2p.router.net.tunnels.client.info.list
i2p.router.net.status.v6
i2p.router.net.error
i2p.router.net.error.v6
i2p.router.net.testing
i2p.router.net.testing.v6
i2p.router.net.tunnels.successrate
i2p.router.net.tunnels.totalsuccessrate
i2p.router.net.tunnels.queue
i2p.router.net.tunnels.tbmqueue
i2p.router.netdb.peers
i2p.router.netdb.activepeers.info
i2p.router.netdb.ntcp.limit
i2p.router.netdb.ssu.limit
i2p.router.netdb.bannedpeers
i2p.router.netdb.activepeers.list
i2p.router.netdb.peers.list
i2p.router.netdb.peers.info
i2p.router.netdb.activepeers.stats
i2p.router.addressbook.private.list
i2p.router.addressbook.local.list
i2p.router.addressbook.router.list
i2p.router.addressbook.published.list
i2p.router.addressbook.subscriptions
i2p.router.addressbook.config
```

Acceptance for WP1:

- a static test asserts the manifest contains exactly these 43 unique strings;
- documentation no longer calls the broader legacy registry `Proposal 170 selectors`;
- every canonical key has a declared JSON type and source state: `available`, `unavailable`, or `protocol ambiguity`;
- no key is silently mapped to a semantically different legacy key.

## 6. RouterInfo reconciliation

### WP2 — Implement canonical direct-parameter semantics

Proposal 170 selects RouterInfo fields by key presence directly in `params`; values are not semantically significant.

Required behavior:

- exact canonical keys are recognized by presence;
- the response uses the exact requested canonical key;
- unknown keys follow the existing JSON-RPC invalid-parameter policy;
- unavailable non-null data returns the existing whole-request error behavior already adopted by Emissary; do not return false zero/empty success;
- nullable official fields such as `i2p.router.id`, `i2p.router.clockskew`, and `i2p.router.info` return their exact types or `null` where the proposal permits it;
- `i2p.router.logs.clear` returns the exact string `"success"` on success;
- exact official numeric types remain numeric JSON values;
- exact list/map selectors remain arrays/objects when available.

### WP3 — Preserve compatibility without corrupting the canonical contract

Existing names such as the following may be retained as Emissary compatibility aliases where their semantics genuinely match:

- `i2p.router.identity` -> `i2p.router.id` only if the returned value is the router hash required by the proposal;
- `i2p.router.clock.skew` -> `i2p.router.clockskew`;
- `i2p.router.log` -> `i2p.router.logs`;
- `i2p.router.log.clear` -> `i2p.router.logs.clear`.

Do not alias when values differ materially. For example, serialized RouterInfo and router hash are distinct outputs.

Compatibility rules:

- canonical official key remains the primary code path;
- aliases are documented separately;
- requesting canonical and alias forms for the same semantic field in one call must not duplicate source queries or create contradictory values;
- legacy-only keys outside Proposal 170 remain supported but are not counted toward Proposal 170 completion.

## 7. AddressBook reconciliation

### WP4 — Implement exact Proposal 170 AddressBook modes

Canonical entry mutation mode:

```text
Type
Hostname
Destination
Delete (optional; operation selected by presence)
```

Canonical subscription mode:

```text
SetSubscriptions: [String]
```

Canonical configuration mode:

```text
SetConfig: Object
```

Required behavior:

- `Type` accepts exactly `private`, `local`, `router`, and `published`;
- absence of `Delete` adds or replaces the named entry according to the adopted reference behavior;
- presence of `Delete` deletes the named entry, regardless of the parameter value;
- `SetSubscriptions` and `SetConfig` are handled inside the `AddressBook` method;
- one request must select exactly one canonical operation mode;
- mixed canonical modes are rejected as invalid parameters;
- persistence, validation, bounds, redaction, and administrative/runtime separation from prior milestones remain intact.

### WP5 — Adjudicate AddressBook response ambiguity explicitly

The open proposal contains inconsistent examples: one uses top-level `success`/`message`, another uses a JSON-RPC `result` object.

Before finalizing the wire response:

1. inspect the proposal-linked Java I2PControl reference implementation PR;
2. record the exact response structure used there for entry mutation, `SetSubscriptions`, and `SetConfig`;
3. compare it with JSON-RPC 2.0 envelope requirements and the proposal text;
4. write a compact adjudication note in the implementation disposition.

Decision rules:

- prefer the linked reference implementation when it clearly resolves the proposal's example inconsistency;
- do not silently invent a third response shape;
- if the reference implementation is unavailable or contradictory, stop only the affected AddressBook response-envelope item and obtain an explicit architecture-owner decision;
- the final canonical form must be covered by literal JSON fixtures.

Existing action-style `book`/`request`/`name`/`value` requests and separate method aliases may remain as documented compatibility extensions. Reject requests that mix canonical and compatibility forms.

## 8. TunnelManager reconciliation

### WP6 — Implement exact canonical actions and result envelopes

Canonical Proposal 170 actions are lowercase:

```text
create
edit
get
start
stop
restart
delete
```

Required behavior:

- lowercase values are accepted exactly;
- `Name` remains the identifier;
- `All` remains valid only for `start`, `stop`, and `restart`;
- `Type` remains required for `create`;
- proposal option names retain exact casing and JSON types;
- numeric ranges listed by the proposal are validated without changing unrelated backend behavior;
- `create` returns a structured result containing `status` and `results` where applicable;
- `edit`, lifecycle, and delete return a structured result containing `status`;
- `get` returns structured `status` and `info`;
- unsupported runtime backends return truthful `error - ... not implemented` status text inside the canonical result shape rather than a bare string or fabricated running state.

### WP7 — Preserve Emissary extensions explicitly

- capitalized action values may remain accepted as compatibility aliases;
- `List` may remain as an Emissary extension;
- neither is counted as Proposal 170 coverage;
- canonical lowercase actions and exact result envelopes must be used in official fixtures;
- no missing tunnel data plane is implemented by this milestone.

Create a field-by-field tunnel option matrix covering every Proposal 170 common and type-specific option. Each field must be classified as:

- parsed and round-tripped;
- accepted and retained in raw configuration;
- explicitly unsupported with deterministic status;
- blocked by proposal ambiguity.

Do not claim complete TunnelManager runtime support merely because CRUD is durable.

## 9. ClientServicesInfo reconciliation

### WP8 — Implement direct parameter-by-presence requests

Canonical request form:

```json
{
  "params": {
    "I2PTunnel": "",
    "SAM": ""
  }
}
```

Required behavior:

- `I2PTunnel`, `HTTPProxy`, `SOCKS`, `SAM`, `BOB`, and `I2CP` are selected by direct key presence;
- any JSON value selects the key;
- only requested keys appear in the response;
- unknown direct keys follow the established invalid-parameter policy;
- the nested boolean `Selector` object may remain as a documented compatibility extension;
- mixed direct and nested forms are rejected to avoid ambiguous selection;
- current live service sources, bounded SAM observation, and explicit failure behavior remain unchanged.

### WP9 — Add one real SAM-to-I2PControl integration path

The existing tests separately prove publisher lifecycle behavior and CLI serialization. Add one focused integration that:

1. starts or constructs the smallest real SAM server/router composition supported by the existing test environment;
2. creates one active SAM session through the normal SAM protocol path;
3. queries `ClientServicesInfo` using the exact direct Proposal 170 request form;
4. verifies the current session identifier, `name`, `address`, and session socket entry;
5. closes the session;
6. verifies the next query no longer includes it.

Use existing test harness components. Do not build a network farm or generic integration framework.

If the current environment cannot complete a real SAM tunnel-pool activation deterministically, record the exact blocker and use the closest production-composition test that still exercises the real shared observation handle. M019 must decide whether that evidence is sufficient; do not claim a true end-to-end test when it is not.

## 10. Documentation reconciliation

### WP10 — Separate three coverage dimensions

Update the conformance and support documents so every canonical item is classified separately:

| Dimension | Meaning |
|---|---|
| Wire implemented | Exact method, key, casing, presence semantics, request fields, result fields, and JSON types are recognized |
| Source available | Emissary has a truthful current canonical source for the requested data |
| Runtime implemented | The requested operation has a real runtime backend rather than an explicit unsupported stub |

Required statements:

- Proposal 170 is Open and closure is pinned to the 2026-05-20 revision;
- the exact Proposal 170 RouterInfo addition set contains 43 keys;
- legacy/base selector keys are documented separately;
- unavailable selectors are not described as operationally implemented;
- all twelve TunnelManager types may have wire/CRUD coverage while runtime start/restart remains unsupported;
- M016's bounded SAM observation remains accepted unless a direct regression is found;
- M017 is invalidated by M018-F1 through F7 and remains historical evidence only.

## 11. Required fixtures and tests

Use literal protocol fixtures derived from the official examples and exact inventory.

Minimum cases:

### RouterInfo

- exact `i2p.router.id` request recognized;
- exact `i2p.router.clockskew` request recognized;
- exact `i2p.router.info` request recognized;
- exact logs and clear keys recognized and typed correctly;
- all 43 canonical keys are unique and registered;
- direct parameter presence selects regardless of value;
- canonical response key is preserved;
- legacy aliases are not counted in the 43-key manifest.

### AddressBook

- canonical add request;
- canonical delete-by-presence request;
- canonical `SetSubscriptions` request;
- canonical `SetConfig` request;
- mixed operation modes rejected;
- compatibility request still works when retained;
- canonical response envelope fixture matches the recorded adjudication.

### TunnelManager

- lowercase `create`, `edit`, `get`, `start`, `stop`, `restart`, and `delete` accepted;
- capitalized aliases, when retained, are explicitly compatibility-only;
- canonical structured `status` response for each action;
- `get` contains structured `info`;
- unsupported backend reports a structured error status;
- `List` is not present in the canonical action manifest.

### ClientServicesInfo

- exact direct-parameter official example;
- any value selects a service;
- only requested services returned;
- nested selector remains compatibility-only;
- mixed forms rejected;
- real or closest production-composition SAM lifecycle evidence.

Do not create a new fixture generator, schema compiler, or CI-only mode.

## 12. Verification commands

Run locally and proportionally:

```bash
cargo fmt --all -- --check
cargo check -p emissary-cli --no-default-features --features i2pcontrol
cargo test -p emissary-cli --no-default-features --features i2pcontrol
cargo clippy -p emissary-cli --no-default-features --features i2pcontrol --all-targets -- -D warnings
```

Run focused suites during implementation:

```bash
cargo test -p emissary-cli --no-default-features --features i2pcontrol router_info
cargo test -p emissary-cli --no-default-features --features i2pcontrol address_book
cargo test -p emissary-cli --no-default-features --features i2pcontrol tunnel_manager
cargo test -p emissary-cli --no-default-features --features i2pcontrol client_services
```

Run core checks only if the narrow core exception is used or the SAM integration test requires existing core test targets:

```bash
cargo check -p emissary-core --features std,events
cargo test -p emissary-core
cargo clippy -p emissary-core --features std,events --all-targets -- -D warnings
```

If full workspace formatting remains blocked by known unrelated baseline differences, check only touched files with the repository's configured nightly rustfmt and record the limitation. Do not reformat unrelated files.

No remote CI, matrix, coverage, fuzzing, release, or generated evidence is required.

## 13. Acceptance criteria

M018 is complete only when all applicable criteria pass:

1. The implementation and documentation pin Proposal 170 status, created date, and last-updated date.
2. The exact 43-key RouterInfo addition manifest matches the pinned proposal with no omission, rename, or extra key.
3. Every canonical RouterInfo key is recognized by direct parameter presence and returned under the exact same key.
4. Legacy/base keys are classified separately and no longer counted as Proposal 170 additions.
5. Unavailable canonical selectors return the established truthful unavailable error rather than fabricated values.
6. AddressBook accepts exact canonical parameter names and all three official operation modes.
7. AddressBook response-envelope ambiguity is resolved by recorded primary-source evidence or explicit architecture-owner decision.
8. Existing AddressBook action-style requests, when retained, are explicitly compatibility-only and cannot be mixed with canonical mode.
9. TunnelManager accepts all seven lowercase canonical actions.
10. Canonical TunnelManager results use the required structured fields.
11. `List` and capitalized action values are classified as compatibility extensions, not Proposal 170 requirements.
12. Every Proposal 170 tunnel option is listed in the field matrix with an honest implementation disposition.
13. No unsupported tunnel backend reports a false running/successful runtime state.
14. ClientServicesInfo accepts the six direct service keys by presence with any value.
15. Nested `Selector` behavior, when retained, is compatibility-only and mixed forms are rejected.
16. Bounded current SAM observation behavior remains correct.
17. One real or explicitly qualified closest-production SAM-to-I2PControl integration test is recorded.
18. Official example fixtures execute against the exact canonical forms.
19. Documentation separates wire support, source availability, and runtime implementation.
20. M017 closure is marked invalidated and no document claims zero unresolved high/medium findings before M019.
21. Existing compatible clients are not broken without an explicit migration decision.
22. No CI, release, repository-wide formatting, broad core inspection, tunnel data-plane, frontend, or general security scope enters.
23. Targeted format, check, test, and clippy commands pass for touched packages or exact pre-existing environmental limitations are recorded.
24. A frozen M018 implementation head is recorded before M019 begins.
25. Registry moves M018 to `closing` and M019 to `ready` only after the implementation and required evidence land.

## 14. Stop conditions

Stop the affected item rather than guessing when:

- the open proposal and linked reference implementation materially disagree on a request or response shape;
- an exact official field has no safe Emissary equivalent and would require sensitive state exposure;
- a canonical selector requires a broad new transport, NetDB, peer, or tunnel inspection architecture;
- exact TunnelManager support would require implementing a deferred runtime data plane;
- compatibility preservation creates an ambiguous request that cannot be rejected cleanly;
- a new dependency, generic protocol framework, CI workflow, or broad repository change is proposed.

A stopped item remains a named blocker. It must not be converted into a default, empty, alias, or extension response merely to obtain closure.

## 15. Handoff sequence

1. Freeze the exact contract inventory and official examples.
2. Reconcile RouterInfo canonical keys and direct presence semantics.
3. Reconcile AddressBook canonical modes and adjudicate its response ambiguity.
4. Reconcile TunnelManager lowercase actions, option matrix, and structured results.
5. Reconcile ClientServicesInfo direct parameter presence.
6. Add focused literal fixtures and the SAM integration evidence.
7. Update support and conformance documentation with three-dimensional status.
8. Run targeted local verification.
9. Freeze the implementation head and write an M018 implementation disposition.
10. Move M018 to `closing`; activate M019 for independent pinned-revision closure.

Do not combine final M019 acceptance with the implementation pass.
