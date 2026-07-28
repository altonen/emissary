# ADR-0001: Proposal 170 Contract Completeness with Explicit Tunnel Stubs

Status: accepted

Date: 2026-07-28

Decision owners: project maintainers

Related canonical sections:

- `plans/000-long-term-specification.md#3-scope-boundary`
- `plans/000-long-term-specification.md#4-architectural-invariants`
- `plans/000-long-term-specification.md#6-tunnel-contract-and-stub-semantics`
- `plans/001-terminology-and-domain-model.md#3-tunnel-terms`

Affected subsystem roadmaps:

- `plans/subsystems/i2pcontrol-proposal-170-roadmap.md`

## Context

I2P Proposal 170 defines an expanded I2PControl surface that represents numerous router statistics, address-book operations, client services, and I2PTunnel tunnel types.

Emissary currently implements some relevant router and proxy capabilities but does not implement every declared tunnel data plane and does not expose the required runtime lifecycle authority for all existing startup-managed services.

Implementing all missing tunnel types inside the Proposal 170 effort would substantially expand scope, force premature service and lifecycle design, and risk changes to router behavior unrelated to the API contract.

Omitting unsupported tunnel types from parsing or method dispatch would make the Proposal 170 API incomplete and force later public API redesign.

Returning fake success or fake running state would violate truthful administration and create interoperability and operational hazards.

## Decision drivers

- Implement Proposal 170 exactly without adding fields, methods, aliases, or statuses.
- Keep the workstream limited to API implementation.
- Avoid router protocol and behavioral changes.
- Avoid frontend work.
- Preserve complete configuration round-tripping for future tunnel implementations.
- Prevent clients from believing unavailable services are active.
- Make later tunnel implementations local backend additions rather than API redesigns.
- Keep current startup-managed tunnel ownership truthful.

## Considered options

### Option A — Implement only currently supported tunnel types

The API would reject or omit declared tunnel types that Emissary cannot run.

Benefits:

- smallest immediate runtime surface;
- no stub abstraction.

Costs and failure modes:

- not contract-complete Proposal 170;
- clients cannot create or preserve future-compatible definitions;
- later implementation requires parser, persistence, and test changes;
- declared protocol types become implementation-dependent extensions.

### Option B — Implement every missing tunnel data plane now

The Proposal 170 effort would include all tunnel runtime designs and services.

Benefits:

- contract and runtime completeness arrive together.

Costs and failure modes:

- scope expands far beyond API implementation;
- lifecycle, LeaseSet, listener, proxy, and protocol decisions become rushed;
- router and service behavior changes become difficult to isolate;
- review and closure boundaries become too broad.

### Option C — Accept all types and simulate successful execution

Configuration and lifecycle operations would report success without real services.

Benefits:

- superficial client compatibility.

Costs and failure modes:

- false operational state;
- clients may publish unusable configuration or assume listeners and LeaseSets exist;
- difficult migration from simulated to real state;
- violates truthful administration.

### Option D — Contract-complete API with explicit unsupported backends

Every declared type is parsed, validated, stored, retrieved, edited, deleted, and dispatched through an exhaustive backend registry. Missing data planes use an explicit unsupported backend. Runtime execution fails deterministically without adding public protocol fields or statuses.

Benefits:

- complete and stable public contract;
- strict scope containment;
- truthful runtime state;
- future backends replace stubs without handler or storage redesign;
- complete API fixtures can be written now.

Costs and failure modes:

- contract completeness and runtime completeness must be documented separately;
- internal state is richer than public Proposal 170 state;
- clients receive operation failures for declared but unavailable runtime types;
- persistence must be designed for options not yet consumed by Emissary.

## Decision

Adopt Option D.

Every Proposal 170 tunnel type MUST have a registered backend. A backend is either real or explicitly unsupported.

For unsupported tunnel types:

- `create`, `edit`, `get`, and `delete` operate on durable administrative definitions;
- `start` and `restart` return a deterministic Proposal 170-compatible textual status beginning with `error -` and identifying that the tunnel type is not implemented;
- `stop` is safe and idempotent when no runtime instance exists;
- inspection never reports active service;
- no listener, destination, session, LeaseSet, or traffic path is simulated;
- internal unsupported state maps to an existing public inactive state when queried;
- no new `supported`, `stubbed`, `implemented`, `capabilities`, or equivalent field is added.

JSON-RPC handlers depend on a typed backend registry and remain independent of backend-specific execution.

Existing startup-managed tunnels are represented as externally managed unless and until a separate roadmap establishes safe lifecycle authority. Unsupported mutations return deterministic operation errors rather than creating contradictory administrative/runtime state.

The Proposal 170 workstream may add bounded read-only core inspection required for truthful selectors. It may not alter router behavior.

## Consequences

### Positive

- The Proposal 170 contract can be implemented and tested in full now.
- Runtime tunnel design remains a separate, reviewable concern.
- Future backend work does not change the external API.
- Unsupported services cannot masquerade as running.
- Persistence becomes forward-compatible with later implementations.
- Scope remains narrow enough for milestone planning and independent closure.

### Negative

- Some valid Proposal 170 lifecycle requests return not-implemented operation statuses.
- Documentation must distinguish API and runtime support precisely.
- The internal model must preserve options that current runtime code does not understand.
- Existing startup-managed tunnels cannot be fully controlled until separate lifecycle work lands.

### Neutral or deferred

- Which currently available Emissary proxies or tunnels can safely receive real adapters is decided in the TunnelManager milestone after ownership inspection.
- Missing tunnel data planes remain separate future projects.
- Runtime resolver integration for the four Proposal 170 address books remains separate.
- Frontend use of I2PControl remains separate.

## Compatibility and migration

- Public JSON-RPC names, fields, types, and status channels remain Proposal 170-compatible.
- No new capability-discovery extension is introduced.
- Tunnel persistence must be versioned and lossless for all declared options.
- Replacing an unsupported backend with a real backend must not require migration of the public request model or handler routing.
- Existing startup configuration remains authoritative for existing startup-managed tasks until separately migrated.

## Security and reliability implications

- Stub start/restart paths must fail before binding sockets, creating destinations, publishing LeaseSets, or starting tasks.
- Stop must be idempotent and must not target unrelated startup-managed tasks.
- Concurrent create/edit/delete operations require deterministic serialization and atomic persistence.
- Unsupported backend errors must not disclose secrets or private destination material.
- Backend registration must be exhaustive and tested so unknown types cannot reach an implicit fallback.
- Request and persistence data remain bounded even when options are preserved losslessly.

## Verification

Conforming implementation evidence must prove:

1. every declared tunnel type parses;
2. every type resolves to a registered backend;
3. stubbed types round-trip through create, get, edit, restart recovery, and delete;
4. stubbed start/restart return deterministic operation errors;
5. stubbed stop is safe and inactive status is truthful;
6. no stubbed type reports running or opens a runtime resource;
7. future test backends can replace stubs without handler changes;
8. startup-managed tunnel mutation does not create contradictory runtime state;
9. no protocol extension fields or statuses were added.

## Supersession

None.
