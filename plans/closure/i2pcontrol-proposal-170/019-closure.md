# I2PControl Proposal 170 Milestone 019 — Pinned-Revision Independent Reclosure

Status: closed against pinned revision

Source implementation plan:

- `plans/implementation/i2pcontrol-proposal-170/019-pinned-revision-independent-reclosure.md`

Source subsystem roadmap:

- `plans/subsystems/i2pcontrol-proposal-170-roadmap.md`

Planning baseline reviewed: `2816857633a927b629c051e07e7efa5baa8d6e07`

Frozen M018 implementation head: `ea35de9`

Final reviewed implementation/test head: `db5e067`

Review date: `2026-07-31`

## 1. Executive finding

M019 independently reviewed the frozen M018 implementation and the actual
final test head against the pinned 2026-05-20 revision of I2P Proposal 170.
The source remains Open, with `created: 2026-05-20` and
`last updated: 2026-05-20`. The exact 43-key RouterInfo addition set,
AddressBook modes, lowercase TunnelManager actions and structured results,
direct ClientServicesInfo selectors, compatibility boundaries, and truthful
unavailable/unsupported behavior all conform to that revision.

The bounded SAM observation limitation is accepted as an environmental
evidence boundary, not as a claim of live data-plane coverage. The strongest
available production-composition evidence reaches the exact serializer through
the shared observation handle, while M016 supplies deterministic publisher,
active-session, socket, and removal lifecycle evidence. A live SAM destination
activation is not deterministic in the repository's ordinary test environment.
This does not leave an unresolved high- or medium-severity finding because the
M018 disposition explicitly records the limitation and supplies the closest
production composition substitute.

Bounded closure statement:

> Emissary implements the exact Proposal 170 wire contract as pinned to the
> 2026-05-20 open revision, with separately documented unavailable data
> sources, unsupported tunnel runtimes, and compatibility extensions.

This statement is not a claim of permanent conformance to future revisions of
the still-open proposal.

## 2. Independence and auditability

The M018 implementation executor is the distinct prior Codex implementation
run recorded by `plans/closure/i2pcontrol-proposal-170/018-implementation-disposition.md`,
which produced `ea35de9`. The M019 reviewer is the current Codex independent
review run recorded by this closure, performed after the implementation head
was frozen and after the M018 disposition was available. The review run
independently refetched the normative source, rechecked the reference
adjudication, inspected the code and literal fixtures, and reran the required
commands.

## 3. Pinned source and adjudications

Normative source:

- I2P Proposal 170, `I2PControl Expansion`
- status: `Open`
- created: `2026-05-20`
- last updated: `2026-05-20`
- `https://i2p.net/en/proposals/170-i2pcontrol-expansion/`

The fetched page still matches the M018 pin exactly. Because the proposal is
still Open, this closure is revision-bound.

AddressBook response ambiguity remains resolved against the linked Java
reference implementation PR #6 and its raw handler:

- `https://github.com/i2p/i2p.plugins.i2pcontrol/pull/6`
- `https://raw.githubusercontent.com/Nick2k4L/i2p.plugins.i2pcontrol/enhancement/src/java/net/i2p/i2pcontrol/servlets/jsonrpc2handlers/AddressBookHandler.java`

The reference handler constructs `success` and `message` as JSON-RPC result
parameters. Emissary therefore correctly uses
`result: {"success": boolean, "message": string}` and sanitizes messages
without exposing reference filesystem paths.

Independent manifest result: exactly these 43 unique canonical additions were
present, with no legacy/base key counted as an addition:

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

## 4. Requirement-to-evidence matrix

| Requirement | Evidence | Result |
|---|---|---|
| Exact 43 RouterInfo additions | `rpc::router_info_keys::PROPOSAL_170_ADDITIONS`, `PROPOSAL_170_CONTRACT`, conformance manifest, and independent field-for-field comparison with Proposal 170 lines 79–153 | Pass; exact strings, uniqueness, declared JSON types, and source states match |
| Canonical RouterInfo presence and response keys | `router_info_handler.rs` direct-parameter path and canonical fixtures for id, info, clockskew, metrics, share ratio, tunnels, news, logs, and logs.clear | Pass; values are selected by presence and response keys are preserved exactly |
| RouterInfo unavailable behavior | `canonical_unavailable_field_is_explicit_error` and source-state manifest | Pass; unavailable fields fail whole-request without fabricated defaults |
| RouterInfo nullable fields and log clear | canonical fixture and handler serialization | Pass; nullable fields preserve permitted values/null and logs.clear returns the exact string `success` |
| AddressBook entry add/delete | `canonical_wire_fixture_mutates_entry_and_uses_result_object` | Pass; exact `Type`/`Hostname`/`Destination` casing and Delete-by-presence are implemented |
| AddressBook subscriptions/config | `canonical_wire_fixture_supports_subscription_and_config_modes` | Pass; `SetSubscriptions` and `SetConfig` are in-method canonical modes |
| AddressBook mode isolation | `canonical_and_compatibility_address_book_forms_cannot_mix` plus canonical mode counting | Pass; mixed and multi-mode requests reject deterministically |
| AddressBook response envelope | M018 primary-source adjudication independently rechecked against the Java handler | Pass; canonical result object is exact and sanitized |
| TunnelManager action vocabulary | `TunnelAction::from_str_exact`, canonical action manifest, and `canonical_wire_fixture_covers_all_seven_actions` | Pass; exactly lowercase `create`, `edit`, `get`, `start`, `stop`, `restart`, `delete`; `List` is absent from canonical actions |
| TunnelManager result envelopes | canonical seven-action fixture and `operation_response` | Pass; canonical success paths are structured and never bare `ok`; get carries `status` and `info`, create/All carry `results` where required |
| TunnelManager options and runtime truthfulness | documented option matrix, range validation, raw round-trip, backend registry, and unsupported-backend tests | Pass for wire/CRUD; unsupported lifecycle backends return explicit error status and never report running |
| ClientServicesInfo direct selection | `canonical_direct_wire_fixture_selects_by_presence` with non-boolean values | Pass; only requested direct keys appear and values are not interpreted |
| ClientServicesInfo compatibility isolation | `canonical_and_compatibility_selectors_cannot_be_mixed` and nested `Selector` compatibility path | Pass; nested boolean selection is secondary and mixed forms reject |
| ClientServicesInfo service shapes | selector-specific serializer tests and official direct `I2PTunnel`/`SAM` fixture shape | Pass; exact keys and JSON types are preserved |
| SAM observation bounds and lifecycle | M016 accepted publisher/socket/session-removal tests plus M018 production-composition serializer test | Pass with documented environmental limitation; overflow/missing source fails explicitly |
| Three-dimensional support claims | `proposal-170-support.md`, `router-info-source-map.md`, method docs, and tunnel-backend docs | Pass; wire, source, and runtime are stated separately |
| Compatibility preservation | existing action-style AddressBook, nested selectors, and capitalized/List TunnelManager tests/docs | Pass; extensions remain visible, secondary, and outside canonical counts |
| Scope boundary | baseline-to-head changed-file review and `git diff --check` | Pass; no CI, release, frontend, broad core, transport, NetDB, resolver, crypto, or tunnel data-plane changes |

## 5. Changed-file and scope review

Compared with the M018 planning baseline, all changed files fall within the
declared boundary:

| Classification | Files |
|---|---|
| Canonical contract implementation | `emissary-cli/src/i2pcontrol/rpc.rs`, `router_info_handler.rs`, `address_book.rs`, `tunnel_manager.rs`, `client_services.rs`, `domain/tunnel.rs`, `backends/registry.rs` |
| Compatibility preservation | Existing compatibility paths in `router_info_handler.rs`, `address_book.rs`, `tunnel_manager.rs`, and `client_services.rs`, with corresponding documentation |
| Focused test/fixture evidence | `emissary-cli/tests/conformance_manifest.rs`, `golden_fixtures.rs`, `production_composition.rs`, `static_guards.rs`, and in-module canonical fixtures |
| Directly affected documentation/planning | `docs/i2pcontrol/**`, Proposal 170 roadmap/registry/readme, M018 disposition, M019 plan, and this closure record |
| Out of scope | None identified |

No workflow, release, publishing, generic framework, broad inspection,
frontend, resolver, transport, NetDB, cryptographic, or missing tunnel
data-plane change was introduced.

## 6. Coverage-claim audit

| Dimension | Accepted claim |
|---|---|
| Wire implemented | Exact Proposal 170 methods, parameters, selector/action casing, presence semantics, response keys, fields, and JSON types are implemented and fixture-tested |
| Source available | Only truthful current Emissary sources are marked available; unavailable RouterInfo sources and missing SAM observation sources fail explicitly |
| Runtime implemented | AddressBook and tunnel CRUD administrative persistence are implemented; unsupported lifecycle backends remain explicit and are not represented as runtime support |

The 121-key legacy/base catalog is documented separately from the exact 43
Proposal 170 additions. All twelve tunnel types have canonical wire/CRUD
registration, not twelve claimed data planes. M016 bounded SAM observation is
wire/source implemented within its documented bounds; it does not claim a new
SAM runtime or lifecycle authority.

## 7. Verification executed

### Commands

```text
rtk cargo fmt --all -- --check
  FAIL: repository baseline formatting differences and nightly-only rustfmt
        options; no semantic failure. The limitation includes untouched files
        and stable rustfmt's inability to apply configured unstable options.

rtk rustup run nightly rustfmt --edition 2021 --check <all 11 M018-touched Rust files>
  PASS

rtk cargo check -p emissary-cli --no-default-features --features i2pcontrol
  PASS

rtk cargo test -p emissary-cli --no-default-features --features i2pcontrol
  PASS: 1130 tests across 15 suites

rtk cargo clippy -p emissary-cli --no-default-features --features i2pcontrol --all-targets -- -D warnings
  PASS

rtk git diff --check
  PASS
```

Core commands were not required: M018's implementation diff did not touch
core and the retained SAM evidence was already accepted in M016/M017. No
remote CI or unrelated workspace audit was required by the plan.

## 8. Invariant, failure, recovery, and security review

- Canonical and compatibility forms remain separate; unknown or mixed forms
  fail according to the existing JSON-RPC invalid-parameter policy.
- AddressBook mutations remain bounded, validated, durable, redacted, and
  isolated from runtime name resolution. Delete uses parameter presence.
- Tunnel definitions remain durably CRUD-managed, startup-owned definitions
  remain protected, and unsupported lifecycle backends do not allocate runtime
  resources or report running state.
- RouterInfo unavailable sources fail closed rather than returning zeros,
  empty collections, aliases, or defaults. Exact nullable fields use null only
  where the pinned contract permits it.
- ClientServicesInfo reads bounded snapshots without taking lifecycle
  authority; SAM overflow and missing active observation sources fail
  explicitly. Publisher and removal behavior remains covered by retained M016
  lifecycle tests.
- No credentials, private keys, complete destinations, session authority,
  filesystem paths, or sensitive configuration values are exposed by the
  canonical responses or sanitized errors.
- No migration, restart, cancellation, or contention regression was found in
  the reviewed M018 scope; the focused persistence and bounded-result tests
  remain part of the passing feature-gated suite.

## 9. Unresolved findings

| Severity | Finding | Impact | Disposition |
|---|---|---|---|
| High | None | — | Closure gate satisfied |
| Medium | None | — | Closure gate satisfied |
| Low | None | — | No closure-impacting defect |
| Informational | A deterministic live SAM destination/session activation is not available in the ordinary repository test environment | No change to wire correctness; live protocol activation remains an operational/environment-specific test | Accepted bounded limitation documented in M018; closest production composition and retained publisher/removal lifecycle evidence are present |

## 10. Roadmap and registry disposition

M018 is formally closed as the exact-wire implementation handoff. M019 is
formally closed against the pinned open revision. The Proposal 170 subsystem
roadmap is therefore closed against that revision.

There are no future implementation plans after M019 in this subsystem. No
future plan can be newly unblocked by this closure; the dependency-ready,
active-closure, and blocked-plan tables are reconciled to empty. Earlier
historical plans with accepted closure records are not reactivated. The stale
M006 implementation-plan status is reconciled to `closed` to match its
accepted closure record and the already-closed downstream milestones.

## 11. Final disposition

`closed against pinned revision`: zero unresolved high or medium findings;
exact final head, source metadata, independent review run, compatibility
review, fixture review, verification outcomes, and bounded SAM limitation are
recorded above. Future changes to the still-open Proposal 170 require a new
source comparison and may invalidate this revision-bound statement without
rewriting this historical record.
