# I2PControl Proposal 170 Roadmap

Status: active exact-contract corrective work

Current planning baseline: `2816857633a927b629c051e07e7efa5baa8d6e07`

Pinned source:

- I2P Proposal 170, `I2PControl Expansion`
- status: `Open`
- created and last updated: `2026-05-20`
- `https://i2p.net/en/proposals/170-i2pcontrol-expansion/`

Canonical references:

- `plans/003-planning-process.md`
- `plans/adrs/ADR-0001-proposal-170-contract-and-stub-boundary.md`
- `plans/closure/i2pcontrol-proposal-170/017-closure-invalidation.md`
- `plans/implementation/i2pcontrol-proposal-170/018-exact-wire-contract-reconciliation.md`
- `plans/implementation/i2pcontrol-proposal-170/019-pinned-revision-independent-reclosure.md`

## 1. Purpose and boundary

This subsystem owns exact Proposal 170 I2PControl wire compatibility:

- exact method names;
- exact parameter names and casing;
- selection by direct parameter presence where specified;
- exact action vocabulary;
- exact response fields and JSON types;
- truthful unavailable behavior;
- compatibility with already-shipped Emissary extensions;
- accurate documentation of wire, source, and runtime support.

It does not own:

- router algorithms;
- transport or NetDB inspection architecture;
- peer selection;
- missing tunnel data planes;
- frontend behavior;
- runtime address-book precedence;
- release or CI policy;
- broad security or formatting work.

## 2. Why the workstream is reopened

M016 correctly landed a bounded read-only SAM session observation handle. M017 accepted that component and several earlier truthfulness/resource corrections, but it did not independently compare the complete public wire vocabulary against the current official Proposal 170 text.

The later comparison found high-severity contract defects:

1. exact RouterInfo keys are missing or renamed;
2. the 121-key legacy/base registry is mislabeled as Proposal 170, while the pinned proposal adds exactly 43 RouterInfo keys;
3. AddressBook uses a different request model;
4. TunnelManager uses different action casing and result envelopes;
5. ClientServicesInfo uses a nested selector envelope instead of direct parameter presence.

The current implementation is useful, but the broad `closed` status is invalid. M017 is retained as historical component evidence and superseded as final closure authority.

## 3. Retained accepted implementation

The following remain accepted absent a direct regression:

- bounded SAM session observation and explicit overflow failure;
- atomic service-generation fencing;
- pre-spawn TLS connection bounds and saturation/restoration evidence;
- live metrics and tracing-backed log observation;
- truthful unavailable RouterInfo behavior instead of fabricated values;
- durable administrative tunnel and address-book stores;
- explicit unsupported tunnel backends.

M018 wraps exact protocol compatibility around these components. It must not rewrite them without a direct need.

## 4. Current findings

| ID | Finding | Severity | Owner |
|---|---|---|---|
| M018-F1 | Exact Proposal 170 RouterInfo keys absent or renamed | high | M018 |
| M018-F2 | 121-key legacy/base catalog mislabeled as Proposal 170 instead of exact 43-key addition set | medium | M018 |
| M018-F3 | AddressBook canonical operation modes missing | high | M018 |
| M018-F4 | TunnelManager lowercase actions and structured results missing | high | M018 |
| M018-F5 | ClientServicesInfo direct parameter-by-presence request missing | high | M018 |
| M018-F6 | No true real-session-to-production-ClientServicesInfo SAM evidence | medium evidence | M018 |
| M018-F7 | M017 broad closure claim invalid | closure defect | M019 |

No additional router/runtime finding is activated.

## 5. Corrective sequence

```text
M018 Exact wire-contract reconciliation
    |
    v
M019 Pinned-revision independent reclosure
```

This is the complete current sequence.

If M019 identifies a defect within M018's boundary, return it to M018 rather than creating another milestone.

## 6. M018 — Exact wire-contract reconciliation

Plan:

- `plans/implementation/i2pcontrol-proposal-170/018-exact-wire-contract-reconciliation.md`

Status: ready

### Objective

Implement and test the exact pinned public contract while retaining safe compatibility extensions.

### Work areas

#### RouterInfo

- create an exact machine-checkable manifest of the 43 Proposal 170 additions;
- recognize exact official keys such as `i2p.router.id`, `i2p.router.clockskew`, `i2p.router.info`, `i2p.router.logs`, and `i2p.router.logs.clear`;
- use direct parameter-by-presence semantics;
- preserve exact response keys and JSON types;
- retain genuinely equivalent old names only as compatibility aliases;
- classify unavailable selectors truthfully.

#### AddressBook

Implement the three canonical modes inside `AddressBook`:

- `Type` + `Hostname` + `Destination`, with optional presence-selected `Delete`;
- `SetSubscriptions`;
- `SetConfig`.

The open proposal contains inconsistent response-envelope examples. M018 must pin the linked reference implementation or record an explicit architecture-owner adjudication before closure.

Existing `book`/`request`/`name`/`value` and separate method forms may remain compatibility extensions.

#### TunnelManager

- accept lowercase `create`, `edit`, `get`, `start`, `stop`, `restart`, and `delete`;
- return structured canonical `status`, `results`, and `info` fields;
- preserve capitalized actions and `List` only as documented extensions;
- inventory every proposal option and range;
- keep unsupported runtime backends explicit rather than implementing data planes.

#### ClientServicesInfo

- select `I2PTunnel`, `HTTPProxy`, `SOCKS`, `SAM`, `BOB`, and `I2CP` by direct parameter presence with any value;
- preserve nested boolean `Selector` only as a compatibility extension;
- reject ambiguous mixed forms;
- retain bounded current SAM observations;
- add the strongest feasible production-composition SAM session lifecycle test.

#### Documentation

Separate:

- wire implemented;
- source available;
- runtime implemented.

No compatibility extension or unavailable source counts as canonical operational support.

### Exit conditions

- all applicable M018 acceptance criteria pass;
- exact source decisions and ambiguities are recorded;
- literal official-example fixtures pass;
- M017 is consistently marked invalidated;
- final implementation head is frozen;
- M018 moves to `closing`;
- M019 becomes `ready`.

## 7. M019 — Pinned-revision independent reclosure

Plan:

- `plans/implementation/i2pcontrol-proposal-170/019-pinned-revision-independent-reclosure.md`

Status: blocked

### Activation

M019 activates only after M018 lands on a frozen head with a complete implementation disposition.

### Review duties

- refetch Proposal 170 and verify revision metadata;
- independently compare the exact 43 RouterInfo manifest;
- compare literal AddressBook, TunnelManager, and ClientServicesInfo fixtures field-for-field;
- verify compatibility forms are secondary and non-ambiguous;
- verify unsupported sources/runtimes remain truthful;
- verify the SAM integration evidence;
- classify all changed files;
- run targeted local commands;
- record a distinct auditable reviewer.

### Exit conditions

- source revision unchanged or implementation rebased;
- zero unresolved high/medium findings;
- exact final reviewed head recorded;
- reviewer independence auditable;
- no scope expansion;
- accepted `plans/closure/i2pcontrol-proposal-170/019-closure.md`;
- registry and roadmap become `closed against pinned revision`.

## 8. Exact contract policy

### Canonical versus compatibility

Canonical Proposal 170 forms:

- must use exact official spelling and casing;
- must pass literal official-example fixtures;
- must not depend on an alias or extension path.

Compatibility forms:

- may remain to avoid breaking existing Emissary clients;
- must be documented separately;
- must not alter canonical output keys/types;
- must not be counted as Proposal 170 coverage;
- must reject ambiguous mixing with canonical forms.

### Unavailable versus unsupported

- `wire implemented` means the exact request is recognized and typed correctly;
- `source unavailable` means Emissary cannot truthfully supply current data;
- `runtime unsupported` means the administrative wire exists but the operational backend does not.

Unavailable or unsupported behavior must not be described as full runtime support.

## 9. Verification policy

Use targeted local verification only:

```bash
cargo fmt --all -- --check
cargo check -p emissary-cli --no-default-features --features i2pcontrol
cargo test -p emissary-cli --no-default-features --features i2pcontrol
cargo clippy -p emissary-cli --no-default-features --features i2pcontrol --all-targets -- -D warnings
```

Run core commands only if M018 uses its narrow core exception or requires existing core integration targets:

```bash
cargo check -p emissary-core --features std,events
cargo test -p emissary-core
cargo clippy -p emissary-core --features std,events --all-targets -- -D warnings
```

Known unrelated formatting differences may be recorded and replaced by touched-file nightly rustfmt checks. Do not format unrelated files.

Not required:

- remote CI;
- platform matrices;
- coverage gates;
- fuzzing;
- network farms;
- release dry runs;
- generated evidence bundles;
- full unrelated workspace audits.

## 10. Scope guard

Allowed production scope is existing I2PControl dispatch, handlers, DTOs, control traits, and smallest required composition seams.

One narrow core read-only accessor is allowed only when an exact official field already has a safe canonical source and no existing handle exposes it. This exception may not be used to build broad transport, NetDB, peer, or tunnel inspection.

Prohibited:

- `.github/workflows/**`;
- CI, release, publishing, or version automation;
- tunnel data-plane implementation;
- broad router/transport/NetDB/peer/tunnel/crypto/resolver/frontend redesign;
- generic protocol, schema, fixture, inspection, or compatibility frameworks;
- repository-wide formatting;
- fabricated defaults.

## 11. Milestone status

| Milestone | Status | Current disposition |
|---|---|---|
| 001–004 | historical implementation foundations | retained |
| 005–007 | superseded | retained as history |
| 008–014 | implementation evidence retained | final broad closure reopened |
| 015 | invalid historical closure | superseded |
| 016 | bounded SAM implementation retained | accepted component |
| 017 | corrective pass required | closure invalidated by exact-contract findings |
| 018 | ready | sole implementation handoff |
| 019 | blocked | final pinned-revision closure gate |

## 12. Completion definition

The Proposal 170 workstream may be marked `closed against pinned revision` only when M019 confirms:

- exact current source revision;
- exact 43 RouterInfo additions;
- exact AddressBook canonical modes;
- exact lowercase TunnelManager actions and structured results;
- exact direct ClientServicesInfo request form;
- literal official-example fixtures;
- safe compatibility preservation;
- truthful unavailable and unsupported classifications;
- accepted SAM current-session evidence;
- zero unresolved high/medium findings;
- no scope expansion.

Any future change to the Open proposal invalidates only the revision-bound closure claim and requires a new source comparison, not an automatic broad implementation rewrite.