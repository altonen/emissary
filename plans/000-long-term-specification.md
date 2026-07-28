# Emissary Proposal 170 Long-Term Specification

Status: normative for the Proposal 170 workstream

This document defines the required end state for implementing I2P Proposal 170 in Emissary. The keywords MUST, MUST NOT, REQUIRED, SHOULD, SHOULD NOT, and MAY are normative.

## 1. Purpose

Emissary MUST expose the I2PControl API additions described by I2P Proposal 170 without expanding the proposal, redesigning router behavior, or prematurely implementing missing tunnel data planes.

The target is contract-complete Proposal 170 support:

- every specified method, selector, parameter, action, tunnel type, response key, JSON type, and validation rule exists;
- data already available from Emissary is returned truthfully;
- missing read-only data is exposed through bounded inspection interfaces;
- unsupported tunnel types are wired through explicit runtime stubs;
- configuration management remains functional for stubbed tunnel types;
- future real tunnel backends can replace stubs without changing the public API.

## 2. Normative external references

The implementation MUST preserve compatibility with:

- I2P Proposal 170, `I2PControl API Expansion`;
- the existing I2PControl JSON-RPC API and authentication/version behavior;
- JSON-RPC 2.0 response-envelope and request-ID semantics.

The proposal is the authority for Proposal 170 fields and actions. The established I2PControl API is the authority for shared transport, authentication, token, version, and JSON-RPC behavior. Reference implementations MAY clarify ambiguity but MUST NOT silently expand the protocol.

## 3. Scope boundary

### 3.1 Required capability

The workstream owns:

- an independently runnable I2PControl service;
- authentication and JSON-RPC dispatch required to expose Proposal 170;
- Proposal 170 additions to `RouterInfo`;
- `AddressBook`;
- `TunnelManager`;
- `ClientServicesInfo`;
- persistent Proposal 170 administrative state;
- read-only router inspection required by Proposal 170;
- explicit tunnel backend registration and unsupported backends;
- protocol, persistence, security, restart, and conformance tests.

### 3.2 Explicit non-goals

The workstream MUST NOT:

- add I2P wire messages or extend Proposal 170;
- alter routing, peer selection, NetDB behavior, tunnel construction, transport behavior, congestion control, or exploratory-tunnel policy;
- implement missing IRC, Streamr, CONNECT, HTTP-server, bidirectional HTTP-server, SOCKS-IRC, or other tunnel data planes;
- redesign existing proxy or tunnel managers merely to make lifecycle control convenient;
- change runtime address-book resolution precedence;
- add frontend controls, pages, views, or frontend-owned state;
- report a stubbed tunnel as active or traffic-capable;
- fabricate values to make selectors appear implemented.

## 4. Architectural invariants

### 4.1 Administrative ownership

I2PControl is an administrative control surface. Its HTTP listener, authentication, request parsing, persistence, and method handlers SHOULD be owned by `emissary-cli` or an equivalent application-layer component.

Core router crates MUST NOT acquire HTTP/JSON-RPC dependencies solely for I2PControl.

### 4.2 Handler purity

JSON-RPC handlers MUST perform only:

1. authentication and version checks;
2. parameter parsing and validation;
3. invocation of a typed control-plane operation;
4. exact response serialization.

Handlers MUST NOT directly own router tasks, mutate NetDB state, edit arbitrary files, or contain tunnel data-plane logic.

### 4.3 Router inspection

Core changes are permitted only when they expose bounded, read-only snapshots required by Proposal 170.

Inspection interfaces MUST NOT:

- expose mutable subsystem handles;
- transfer task ownership;
- modify peer profiles, bans, NetDB entries, tunnels, transports, or queues;
- consume an event stream needed by another frontend or subsystem;
- block router progress on unbounded serialization.

### 4.4 Frontend independence

I2PControl MUST operate in headless and frontend-enabled builds without depending on frontend state or lifecycle. No visual frontend work is part of this workstream.

## 5. Protocol exactness

The implementation MUST:

- use JSON-RPC 2.0 envelopes;
- preserve request IDs;
- use named parameters;
- require authentication tokens for all methods except authentication as defined by I2PControl;
- return only requested selector keys where the API uses selector-by-presence behavior;
- preserve exact Proposal 170 key names, value types, action names, and tunnel type names;
- distinguish JSON-RPC/protocol errors from TunnelManager operation status strings;
- avoid aliases, pagination, metadata wrappers, capability fields, or extension status values not defined by the protocol.

Malformed examples in explanatory text MUST NOT override the established JSON-RPC result envelope.

## 6. Tunnel contract and stub semantics

Every Proposal 170 tunnel type MUST be accepted by parsing, validation, persistence, and backend dispatch.

For a tunnel type without a runtime implementation:

- `create`, `edit`, `get`, and `delete` MUST operate on persistent administrative state;
- `start` and `restart` MUST reach an explicit unsupported backend and return a deterministic Proposal 170-compatible `error - ... not implemented` status;
- `stop` MUST be safe and idempotent for an inactive definition;
- status inspection MUST NOT report the tunnel as running;
- no listener, LeaseSet, session, destination, or traffic path may be simulated;
- unsupported state MUST remain internal and map to an existing wire-level state such as `stopped` when queried.

Future runtime implementation MUST require only backend registration and backend-specific code. It MUST NOT require public API, persistence-schema, or handler redesign.

## 7. Existing runtime ownership

Existing startup-managed Emissary tunnels MAY be exposed through read-only inspection. Proposal 170 MUST NOT claim lifecycle authority over tasks that do not expose a safe lifecycle control contract.

An operation against an externally managed tunnel MUST either be truthfully supported or return a deterministic operation error. It MUST NOT update a control-plane copy while leaving contrary runtime state unreported.

## 8. Address-book boundary

Proposal 170's `private`, `local`, `router`, and `published` books MUST exist as persistent administrative stores with exact CRUD, configuration, and subscription behavior.

This workstream MUST NOT change Emissary's runtime resolver precedence or automatically make the four administrative books authoritative for destination resolution. Runtime integration requires separate design and approval.

## 9. Truthful state and observability

RouterInfo and ClientServicesInfo MUST use real snapshots or explicitly permitted null/error behavior. Missing implementation MUST NOT be disguised as zero, false, an empty collection, or a successful operation.

I2PControl log retrieval MAY use a bounded tracing-backed memory buffer. Clearing that buffer MUST NOT clear or reconfigure unrelated log sinks.

## 10. Persistence and recovery

Proposal 170 administrative data MUST use:

- versioned schemas;
- deterministic serialization;
- validation before activation;
- atomic same-filesystem replacement;
- bounded recovery behavior after interrupted writes;
- explicit handling of unsupported or externally managed tunnel definitions.

Existing `router.toml` behavior MUST remain compatible unless a later accepted plan explicitly defines an additive configuration change.

## 11. Security and resource bounds

The implementation MUST provide:

- secure token generation and validation;
- loopback-safe default binding or explicit secure configuration;
- request-body and nesting limits;
- bounded peer, log, RouterInfo, and tunnel result construction;
- timeouts and cancellation for request work;
- redaction of credentials, tokens, private keys, and sensitive destination material;
- path confinement for persistence;
- deterministic concurrent-edit behavior.

## 12. Completion definition

This workstream is complete only when:

1. every Proposal 170 method and selector is implemented;
2. every declared tunnel type has a real or explicit unsupported backend;
3. stubbed types support complete configuration CRUD and truthful failed execution;
4. RouterInfo and ClientServicesInfo values are truthful and correctly typed;
5. all four address-book stores persist and round-trip;
6. the service runs without a frontend and does not interfere with frontend event consumers;
7. no protocol extension, router behavioral change, missing tunnel data plane, or frontend work has entered scope;
8. a closure record demonstrates protocol, security, persistence, restart, concurrency, and compatibility evidence.

The accurate completion statement is:

> Emissary implements the complete Proposal 170 I2PControl API contract. Unsupported tunnel data planes are wired through explicit stubs and remain separate implementation work.
