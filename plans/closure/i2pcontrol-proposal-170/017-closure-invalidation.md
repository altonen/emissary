# M017 Closure Invalidation — Exact Proposal 170 Contract Defects

Status: corrective pass required

Invalidated closure record:

- `plans/closure/i2pcontrol-proposal-170/017-closure.md`

Reviewed repository head:

- `2816857633a927b629c051e07e7efa5baa8d6e07`

Pinned normative source used for invalidation:

- I2P Proposal 170, `I2PControl Expansion`
- status: `Open`
- created: `2026-05-20`
- last updated: `2026-05-20`
- `https://i2p.net/en/proposals/170-i2pcontrol-expansion/`

## 1. Disposition

M017's acceptance of the bounded SAM observation work remains useful evidence for that component. Its broader conclusion that the Proposal 170 workstream had zero unresolved high/medium findings is invalid.

The closure review concentrated on M014/M016 truthfulness, SAM observation, service fencing, TLS bounds, and final-head scope. It did not independently compare the repository's public request and response vocabulary against the exact current Proposal 170 text.

A subsequent exact-contract comparison found material protocol mismatches across all four Proposal 170 method areas. M017 is therefore retained as historical evidence but is no longer closure authority.

## 2. High contract findings

### M018-F1 — RouterInfo canonical keys are absent or renamed

Examples:

- Proposal 170: `i2p.router.id`; repository: `i2p.router.identity`.
- Proposal 170: `i2p.router.clockskew`; repository: `i2p.router.clock.skew`.
- Proposal 170: `i2p.router.info`; no exact canonical key registered.
- Proposal 170: `i2p.router.logs`; repository: `i2p.router.log`.
- Proposal 170: `i2p.router.logs.clear`; repository: `i2p.router.log.clear`.

The repository also reports a 121-key legacy/base catalog as `Proposal 170 selectors`, while the pinned proposal adds exactly 43 RouterInfo keys.

### M018-F3 — AddressBook canonical request shape is not implemented

Proposal 170 defines:

- `Type`;
- `Hostname`;
- `Destination`;
- optional presence-selected `Delete`;
- `SetSubscriptions` inside the `AddressBook` method;
- `SetConfig` inside the `AddressBook` method.

The repository instead requires action-style `book`, `request`, `name`, and `value` parameters and treats `SetSubscriptions` and `SetConfig` as separate JSON-RPC methods.

### M018-F4 — TunnelManager canonical action and result shape is not implemented

Proposal 170 defines lowercase actions:

```text
create
edit
get
start
stop
restart
delete
```

The repository accepts capitalized actions, adds `List`, and may return bare `"ok"` strings instead of canonical structured `result.status`, `result.results`, and `result.info` objects.

### M018-F5 — ClientServicesInfo canonical request shape is not implemented

Proposal 170 selects services by direct key presence in `params`, with any value.

The repository requires a nested boolean `Selector` object and therefore rejects the official example request shape.

## 3. Medium findings

- The support and conformance documents conflate exact wire recognition, source availability, and runtime backend implementation.
- All twelve TunnelManager data-plane backends remain unsupported, but current closure language can be read as complete operational support.
- SAM publisher lifecycle and serializer tests are strong, but no true real-session-to-production-ClientServicesInfo integration test is recorded.
- Proposal 170 remains Open, so closure must be pinned to an exact revision and rechecked for source changes during final review.

## 4. What remains accepted

This invalidation does not reject the following landed components absent a direct regression:

- bounded current SAM session observation from M016;
- atomic per-category service fencing;
- pre-spawn TLS connection bounds and saturation/restoration evidence;
- live metric and log composition;
- explicit unavailable results instead of fabricated RouterInfo values;
- durable administrative tunnel definitions and address-book state as Emissary functionality.

The corrective work is exact protocol reconciliation around those components, not a rewrite of them.

## 5. Corrective ownership

- M018 owns exact Proposal 170 wire-contract reconciliation and official-example fixtures.
- M019 owns independent closure against the actual pinned proposal revision and final implementation head.

M018 must preserve safe compatibility aliases where feasible, but aliases may not substitute for canonical Proposal 170 names or shapes.

## 6. Closure state

Current status:

- M014: implementation retained; broader final acceptance reopened by M018/M019.
- M016: bounded SAM implementation retained.
- M017: closure invalidated; historical evidence only.
- M018: ready.
- M019: blocked on a frozen complete M018 head and auditable independent review.

No current document may describe the Proposal 170 workstream as closed until M019 is accepted with zero unresolved high/medium findings.