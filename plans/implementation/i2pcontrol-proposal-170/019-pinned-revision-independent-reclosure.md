# I2PControl Proposal 170 Milestone 019 — Pinned-Revision Independent Reclosure

Status: blocked

Planning baseline: `2816857633a927b629c051e07e7efa5baa8d6e07`

Activation dependency:

- M018 implementation and evidence complete on a frozen head;
- `plans/closure/i2pcontrol-proposal-170/018-implementation-disposition.md` records exact contract decisions and verification outcomes;
- M018 is moved to `closing` in `plans/registry.md`;
- the M019 reviewer is distinct from the final M018 implementation executor and identifies the separate review run.

Pinned normative source:

- I2P Proposal 170, status `Open`
- created and last updated `2026-05-20`
- canonical page: `https://i2p.net/en/proposals/170-i2pcontrol-expansion/`

Primary class: independent protocol conformance closure

## 1. Objective

Independently determine whether the actual final implementation head conforms to the pinned Proposal 170 revision without overstating unavailable data sources or unsupported tunnel runtimes.

M019 is not another implementation pass. It must reject closure when an exact method, key, casing rule, request shape, response field, JSON type, or presence semantic remains wrong.

The accepted closure statement must be bounded:

> Emissary implements the exact Proposal 170 wire contract as pinned to the 2026-05-20 open revision, with separately documented unavailable data sources, unsupported tunnel runtimes, and compatibility extensions.

It must not claim permanent conformance to future revisions of the still-open proposal.

## 2. Review boundary

Review only:

- the exact M018 implementation diff;
- the current I2PControl request dispatch and serialization paths;
- the exact Proposal 170 contract manifest and literal fixtures;
- compatibility alias behavior directly adjacent to canonical forms;
- current Proposal 170 support/conformance documentation;
- the retained bounded SAM observation behavior;
- final changed-file scope and targeted command outcomes.

Do not reopen:

- router algorithms;
- transport, NetDB, peer-selection, tunnel construction, cryptography, resolver, or frontend architecture;
- missing tunnel data planes;
- CI, release, packaging, or publishing policy;
- unrelated repository formatting or historical code quality.

## 3. Required independent contract audit

### 3.1 Source freshness and pin verification

At review time:

1. fetch the official Proposal 170 page;
2. verify its status, created date, and last-updated date;
3. compare the fetched revision against M018's pinned source record;
4. if the proposal changed after `2026-05-20`, stop and determine whether M018 must be rebased before closure;
5. record the exact review date and source metadata in the M019 closure record.

Because the proposal is Open, a changed revision is a contract blocker, not an informational note.

### 3.2 RouterInfo manifest audit

Independently compare the repository manifest against the 43 exact Proposal 170 additions.

Required checks:

- exactly 43 canonical strings;
- no spelling, case, dot-placement, or suffix mismatch;
- no legacy/base key counted as a Proposal 170 addition;
- direct parameter-by-presence behavior accepts any value;
- canonical responses preserve the exact requested key;
- declared return JSON types match the proposal;
- unavailable canonical keys use truthful whole-request error behavior rather than renamed aliases, zeros, empty arrays, or defaults;
- nullable official fields use `null` only where permitted;
- `i2p.router.logs.clear` returns `"success"` when successful.

Spot-check at minimum:

```text
i2p.router.id
i2p.router.clockskew
i2p.router.info
i2p.router.logs
i2p.router.logs.clear
i2p.router.net.total.received.bytes
i2p.router.net.tunnels.shareratio
i2p.router.netdb.peers
i2p.router.addressbook.private.list
```

A missing or renamed canonical key is a high finding and rejects closure.

### 3.3 AddressBook audit

Review the three canonical modes:

1. `Type` + `Hostname` + `Destination`, with optional presence-selected `Delete`;
2. `SetSubscriptions` inside `AddressBook`;
3. `SetConfig` inside `AddressBook`.

Required checks:

- exact parameter capitalization;
- correct operation selection by presence;
- mixed modes rejected;
- exact result envelope follows the M018 primary-source adjudication;
- fixtures contain literal canonical requests and results;
- old `book`/`request`/`name`/`value` and separate method aliases are clearly compatibility extensions;
- canonical and compatibility forms cannot be mixed ambiguously;
- persistence, bounds, validation, and redaction remain intact.

If the M018 AddressBook adjudication lacks a cited primary source or explicit architecture-owner decision, reject closure.

### 3.4 TunnelManager audit

Required canonical actions:

```text
create
edit
get
start
stop
restart
delete
```

Required checks:

- exact lowercase actions accepted;
- `List` absent from the canonical action manifest;
- capitalized actions classified as compatibility aliases only;
- canonical result objects contain exact `status`, `results`, and `info` fields where required;
- no canonical success path returns a bare `"ok"` string;
- `All` is accepted only for start/stop/restart;
- tunnel option matrix covers every proposal-listed field and range;
- unsupported runtime backends return structured error status and never report running;
- CRUD support is not described as runtime support.

Any missing lowercase action or wrong canonical result envelope is a high finding.

### 3.5 ClientServicesInfo audit

Required checks:

- direct service keys appear inside `params`;
- any value selects the service;
- only requested services are returned;
- nested boolean `Selector` is compatibility-only;
- mixed direct/nested requests are rejected;
- exact service result shapes remain correct;
- bounded SAM snapshot is current and fails explicitly on missing/overflowed source;
- one active SAM session can be observed and later removed through the strongest available production-composition evidence.

If only split publisher/serializer tests exist and no production-composition evidence is supplied, record at least a medium evidence finding and reject strict closure unless the M018 disposition documents an accepted environmental limitation with a convincing closest-production substitute.

## 4. Compatibility review

Proposal 170 requires compatibility with existing I2PControl applications. The repository also has already-shipped Emissary-specific forms.

Verify:

- canonical Proposal 170 forms do not depend on compatibility aliases;
- compatibility aliases do not change canonical output keys or types;
- ambiguous mixed forms fail deterministically;
- existing supported Emissary clients retain their old request path unless the M018 plan records an explicit migration decision;
- documentation labels every extension visibly and does not include it in canonical coverage counts.

Compatibility preservation does not authorize accepting the wrong canonical form.

## 5. Coverage-claim audit

The final documentation must use three separate dimensions:

| Dimension | Closure requirement |
|---|---|
| Wire implemented | exact method, parameters, key, action, casing, result fields, JSON type, and presence semantics |
| Source available | truthful current Emissary source exists |
| Runtime implemented | real operation/backend exists |

Verify that:

- unavailable RouterInfo selectors are not marked operationally implemented;
- unsupported TunnelManager start/restart backends are not marked runtime implemented;
- all twelve tunnel types may be marked wire/CRUD implemented only when exact canonical serialization is proven;
- M016 bounded SAM observation remains wire/source implemented;
- the broader legacy key catalog is not labeled `Proposal 170 selectors`;
- the workstream is described as closed against a pinned open revision.

## 6. Changed-file and scope review

Compare the M018 planning baseline to the frozen implementation head.

Classify every changed file as:

- canonical contract implementation;
- compatibility preservation;
- focused test/fixture evidence;
- directly affected documentation/planning;
- out of scope.

Reject closure if the implementation introduces:

- workflow/CI changes;
- release or publishing automation;
- broad core inspection architecture;
- missing tunnel data planes;
- generic schema or fixture frameworks;
- unrelated repository formatting;
- frontend, resolver, transport, NetDB, tunnel, cryptographic, or broad security changes unrelated to exact wire serialization.

A small pre-declared core accessor is reviewable only under M018's explicit exception.

## 7. Required verification

Run against the frozen final implementation/test head:

```bash
cargo fmt --all -- --check
cargo check -p emissary-cli --no-default-features --features i2pcontrol
cargo test -p emissary-cli --no-default-features --features i2pcontrol
cargo clippy -p emissary-cli --no-default-features --features i2pcontrol --all-targets -- -D warnings
```

Run core commands only if M018 touched core or the SAM integration evidence requires them:

```bash
cargo check -p emissary-core --features std,events
cargo test -p emissary-core
cargo clippy -p emissary-core --features std,events --all-targets -- -D warnings
```

If full formatting is blocked by pre-existing untouched-file differences, run the configured formatter on all M018-touched Rust files and record the baseline failure. Do not fix unrelated formatting.

The closure record must list exact command outcomes. Static source inspection alone is insufficient for canonical protocol fixtures.

## 8. Mandatory fixture review

Execute and inspect literal fixtures for:

- the official RouterInfo `i2p.router.id` example;
- all 43 canonical RouterInfo keys;
- AddressBook add and delete-by-presence;
- AddressBook `SetSubscriptions` and `SetConfig`;
- TunnelManager lowercase create/edit/get/start/stop/restart/delete;
- structured TunnelManager results;
- ClientServicesInfo direct `I2PTunnel` and `SAM` example;
- direct selection using non-boolean values;
- compatibility aliases separately;
- mixed-form rejection;
- unavailable-source behavior;
- SAM active session and removal evidence.

The reviewer must compare fixture JSON field-for-field with the pinned source/adjudication, not merely confirm tests pass.

## 9. Finding policy

Severity:

- High: missing/renamed canonical key, wrong method shape, wrong action casing, wrong canonical response envelope, false operational success, or security-sensitive exposure.
- Medium: incomplete option matrix, ambiguous compatibility mixing, missing production-composition evidence, incorrect coverage claim, or unrecorded proposal ambiguity.
- Low: localized documentation or naming defect that cannot mislead a client or closure decision.

Closure requires:

- zero unresolved high findings;
- zero unresolved medium findings;
- exact final head recorded;
- reviewer independence recorded;
- all pinned-source decisions cited in the closure artifact.

Any high/medium finding rejects closure and returns the issue to M018 when it fits that scope. Do not create M020 merely to rename the same corrective work.

## 10. Required closure artifact

Create:

- `plans/closure/i2pcontrol-proposal-170/019-closure.md`

It must contain:

1. status;
2. pinned proposal metadata and review date;
3. frozen M018 implementation head;
4. final reviewed test head;
5. implementation executor identity/run;
6. independent reviewer identity/run;
7. changed-file classification;
8. exact 43-key manifest result;
9. AddressBook adjudication result;
10. TunnelManager canonical action/result result;
11. ClientServicesInfo request-shape and SAM evidence result;
12. compatibility-extension review;
13. three-dimensional coverage table;
14. verification commands and outcomes;
15. unresolved findings by severity;
16. final bounded closure statement.

Do not overwrite M017 into a passing record. Preserve M017 and its invalidation as historical evidence.

## 11. Acceptance criteria

M019 may close only when:

1. the official source still matches the pinned 2026-05-20 revision or M018 has been rebased to a newer revision;
2. the exact 43-key RouterInfo manifest passes independent comparison;
3. every canonical request uses exact direct parameter presence and exact casing;
4. every canonical response uses exact key names and JSON types;
5. AddressBook's response ambiguity has primary-source or explicit owner adjudication;
6. all three canonical AddressBook modes pass literal fixtures;
7. all seven lowercase TunnelManager actions pass literal fixtures;
8. canonical TunnelManager results are structured and no success returns bare `"ok"`;
9. every proposal tunnel option has an honest disposition;
10. ClientServicesInfo direct parameter requests pass with any values;
11. nested selectors and other legacy forms are compatibility-only;
12. SAM current-session and removal evidence is accepted;
13. unavailable sources and unsupported runtimes remain truthful;
14. documentation separates wire, source, and runtime support;
15. no document claims the broader legacy registry is the Proposal 170 addition set;
16. no scope expansion entered;
17. targeted verification passes or exact pre-existing environmental limitations are documented;
18. reviewer independence is auditable;
19. zero unresolved high/medium findings remain;
20. final status states closure against the pinned Open proposal revision, not permanent future conformance.

## 12. Activation and handoff rule

M019 remains blocked until M018 lands and freezes a complete implementation head.

Once activated:

1. do not modify production code during the review;
2. record findings before documentation closure edits;
3. reject and return high/medium defects to M018;
4. create `019-closure.md` only after the actual final head passes;
5. update registry and roadmap to `closed against pinned revision` only after the closure record is accepted.