# M018 Implementation Disposition — Exact Wire-Contract Reconciliation

Status: closed

Frozen implementation head: `ea35de9` (`fix(i2pcontrol): reconcile Proposal 170 wire contract`)

Implementation executor: Codex implementation run recorded in the repository
session that produced `ea35de9`.

This is the M018 implementation handoff and disposition. It is not the final
independent Proposal 170 closure; M019 is now unblocked for that review.

## Pinned sources and adjudications

Normative source:

- I2P Proposal 170, `I2PControl Expansion`
- status: `Open`
- created: `2026-05-20`
- last updated: `2026-05-20`
- <https://i2p.net/en/proposals/170-i2pcontrol-expansion/>

AddressBook response ambiguity was resolved against the linked Java reference
implementation PR #6:

- <https://github.com/i2p/i2p.plugins.i2pcontrol/pull/6>
- <https://raw.githubusercontent.com/Nick2k4L/i2p.plugins.i2pcontrol/enhancement/src/java/net/i2p/i2pcontrol/servlets/jsonrpc2handlers/AddressBookHandler.java>

The reference handler builds `success`/`message` in the JSON-RPC response
parameters, so the canonical Emissary shape is a JSON-RPC `result` object:
`{"success": boolean, "message": string}`. The proposal's top-level
`success` example is treated as an inconsistent example. Emissary sanitizes
messages and does not expose the reference implementation's filesystem paths.

## Requirement-to-evidence matrix

| Requirement | Evidence | Disposition |
|---|---|---|
| Exact 43 RouterInfo additions | `rpc::router_info_keys::PROPOSAL_170_ADDITIONS` and `PROPOSAL_170_CONTRACT`; conformance/static manifest tests | Pass; legacy/base 121-key catalog is separate |
| Canonical RouterInfo direct presence and exact response keys | `router_info_handler.rs`; `canonical_direct_wire_fixture_returns_exact_fields`, logs, unavailable, and conformance tests | Pass |
| Canonical nullable fields and logs clear type | `i2p.router.id`, `clockskew`, `info`, `logs`, `logs.clear` fixture | Pass; clear returns exact `"success"` |
| Truthful unavailable/ambiguous RouterInfo fields | Contract source states plus whole-request unavailable path | Pass; no fabricated defaults |
| Canonical AddressBook entry, delete, subscriptions, config modes | AddressBook handler and three literal canonical unit fixtures | Pass |
| AddressBook response-envelope adjudication | This record's pinned Java PR/raw source decision and response-envelope fixture | Pass; M019 independently rechecks source |
| Compatibility AddressBook forms | Existing action-style/separate-method tests; canonical/compatibility mixing rejection | Pass; extensions remain explicitly non-canonical |
| Seven lowercase TunnelManager actions | `TunnelAction::from_str_exact`, canonical action manifest, all-seven literal fixture | Pass; `List` and capitals are compatibility-only |
| Structured TunnelManager results and honest runtime status | canonical create/edit/get/lifecycle/delete fixture; backend registry count and unsupported status paths | Pass; no tunnel data plane added |
| Tunnel option inventory | `tunnel-manager.md` matrix; typed extraction/range validation and raw round-trip | Pass for wire/CRUD; runtime remains per-backend |
| ClientServicesInfo direct presence | direct official example, any-value, and mixed-form fixtures | Pass |
| Bounded SAM observation retained | Existing M016 evidence plus production composition serializer test | Qualified; see SAM evidence below |
| Three-dimensional documentation | conformance, support, method, and source-map updates | Pass |
| Scope guard | `git diff --name-only`; no CI, release, core, frontend, or data-plane edits | Pass |

## SAM evidence and limitation

The repository does not expose the core SAM observation publisher outside
`SamServer`, and a deterministic real destination/session activation requires a
live SAM protocol/tunnel-pool environment. M018 therefore does not claim a
true end-to-end session lifecycle test.

`emissary-cli/tests/production_composition.rs::production_sam_observation_source_reaches_client_services_serializer`
constructs real production address-book/tunnel/router/control adapters, passes
the shared `SamSessionObservationHandle` through `I2pControlState`, and queries
the ClientServicesInfo serializer with a listening SAM registry entry. The
bounded empty snapshot is returned as an actual empty session map. M019 must
decide whether this closest-production evidence is sufficient or require an
environment-specific real SAM integration.

M016's accepted publisher lifecycle and serializer evidence remains retained;
no direct regression was found.

## Verification outcomes

Passed at frozen head:

```text
rtk cargo check -p emissary-cli --no-default-features --features i2pcontrol
rtk cargo test -p emissary-cli --no-default-features --features i2pcontrol
rtk cargo clippy -p emissary-cli --no-default-features --features i2pcontrol --all-targets -- -D warnings
rtk rustfmt +nightly --edition 2021 --check <all M018-touched Rust files>
rtk git diff --check
```

The feature-gated package suite passed all `1130` tests. Stable
`cargo fmt --all -- --check` remains blocked by pre-existing repository-wide
formatting differences and this repository's nightly-only rustfmt options;
only M018-touched Rust files were formatted and checked with configured
nightly rustfmt. No unrelated files were reformatted.

## Compatibility, security, and scope review

- Existing nested RouterInfo/ClientServicesInfo selectors and capitalized/List
  TunnelManager forms remain accepted only as compatibility extensions.
- Canonical and compatibility request forms reject ambiguous mixtures.
- AddressBook persistence, bounds, validation, redaction, and administrative /
  runtime separation remain intact.
- Unavailable inspection and unsupported tunnel runtime behavior remains
  explicit; no false running state or fabricated selector value was added.
- No credentials, private keys, full destinations, filesystem paths, or
  internal authority are exposed by the canonical responses.
- No CI, release, frontend, broad core, transport, NetDB, peer, or tunnel
  data-plane scope entered the change.

## Findings and handoff

M018-F1 through M018-F5 and M018-F7 are implemented or dispositioned by the
frozen head. M018-F6 remains a named evidence limitation, not a claim of true
end-to-end coverage. No unresolved implementation high/medium finding is
known within the M018 scope. M019 must independently recheck the pinned
source, this disposition, the final head, and whether the qualified SAM
evidence satisfies strict closure.

M018 is therefore `closed` as the exact-wire implementation handoff. M019
performed the distinct reviewer/final-head/source recheck and accepted the
subsystem against the pinned open revision in
`plans/closure/i2pcontrol-proposal-170/019-closure.md`.
