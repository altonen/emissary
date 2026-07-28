# Proposal 170 Terminology and Domain Model

Status: normative for the Proposal 170 workstream

This document defines stable language for plans, implementation, tests, and closure records. Terms from Proposal 170 retain their exact external spelling on the wire even when Rust identifiers use conventional casing.

## 1. API and protocol terms

### I2PControl service

The administrative HTTP JSON-RPC service that authenticates callers and dispatches existing I2PControl methods plus Proposal 170 methods and selectors.

It is not an I2P transport, router protocol, frontend, or tunnel data plane.

### JSON-RPC protocol error

A failure in request parsing, method lookup, authentication, version negotiation, parameter validation, or internal request processing that belongs in a JSON-RPC `error` envelope.

### Operation status

A method-defined result, particularly TunnelManager's textual `status`, indicating whether a valid requested operation succeeded or failed. An unsupported tunnel backend is normally an operation failure, not a new JSON-RPC error code.

### Selector

A Proposal 170 parameter whose presence requests one corresponding response field. Unless the protocol explicitly says otherwise, only requested selector fields are returned.

### Contract-complete

Every specified method, field, action, type, validation path, and response shape exists and is tested. Contract completeness does not imply that every represented tunnel data plane is implemented.

### Runtime-complete

The underlying service or tunnel performs its real network behavior. Runtime completeness is tracked separately from Proposal 170 contract completeness.

## 2. Control-plane terms

### Control plane

The typed internal boundary used by JSON-RPC handlers. It coordinates authentication-independent operations for router inspection, address books, tunnel definitions, service inspection, logs, and persistence.

### Administrative state

Persistent state owned for API configuration and reporting. Administrative state does not automatically become authoritative router runtime state.

### Inspection snapshot

A bounded, immutable representation of current router or service state produced without transferring mutable subsystem ownership or changing behavior.

### Externally managed

A service or tunnel whose task lifecycle is owned by an existing Emissary startup manager rather than the Proposal 170 control plane.

Externally managed objects may be inspected. Lifecycle or mutation operations must be supported truthfully or rejected explicitly.

## 3. Tunnel terms

### Tunnel type

One exact Proposal 170 tunnel type value:

- `client`
- `httpclient`
- `ircclient`
- `socks`
- `socksirc`
- `connectclient`
- `streamrclient`
- `server`
- `httpserver`
- `httpbidirserver`
- `ircserver`
- `streamrserver`

No aliases or additional type values are part of this workstream.

### Tunnel definition

A persistent, canonical representation of a Proposal 170 tunnel configuration, including its name, exact type, start-on-load intent, typed known options, and a lossless canonical option representation required for `get` and future backend adoption.

A tunnel definition is not proof that a runtime service exists.

### Tunnel backend

The runtime adapter registered for one tunnel type. It owns execution-specific start, stop, and inspection behavior but not JSON-RPC parsing or persistence policy.

### Real backend

A backend connected to an actual Emissary runtime implementation capable of performing the tunnel type's intended network behavior.

### Unsupported backend

An explicit backend registered for a declared tunnel type whose runtime data plane is not implemented.

It must:

- reject start and restart deterministically;
- remain safe under stop;
- report no active runtime capability;
- preserve full configuration CRUD through the control plane.

### Backend registry

The exhaustive mapping from every Proposal 170 tunnel type to either a real or unsupported backend. Missing registrations are implementation defects.

### Tunnel ownership

The authority responsible for runtime lifecycle:

- **control-plane owned** — created and managed through Proposal 170 infrastructure;
- **startup managed** — created by existing Emissary configuration and managers;
- **unsupported** — no runtime task can exist, although administrative state can exist.

### Internal tunnel state

A richer implementation state such as stopped, starting, running, stopping, failed, unsupported, or externally managed.

Internal states must map to existing Proposal 170 wire values and must not create new public status vocabulary.

## 4. Address-book terms

### Administrative address book

One of Proposal 170's `private`, `local`, `router`, or `published` persistent stores exposed by I2PControl.

### Runtime resolver

The existing Emissary destination resolution path. Administrative address-book implementation does not imply runtime resolver integration or precedence changes.

### Subscription set

The ordered, persistent Proposal 170 address-book subscription configuration.

### Address-book configuration

The persistent string-keyed configuration map accepted and returned by Proposal 170. Known fields may receive additional validation; compatibility-preserving unknown fields are retained where the contract permits.

## 5. Router and service inspection terms

### RouterInfo selector

One Proposal 170 extension key requested through `RouterInfo`, including identity, serialized RouterInfo, clock skew, logs, byte counters, tunnel state, peer state, address books, and network state.

### Client service

A service category exposed by `ClientServicesInfo`: I2PTunnel, HTTPProxy, SOCKS, SAM, BOB, or I2CP.

### Truthful unavailable value

A protocol-permitted null or explicit error used when Emissary cannot supply a value. A fabricated zero, false, empty collection, running state, or success response is not truthful unavailability.

## 6. Planning terms

### Canonical document

A stable normative document under `plans/000-*`, `plans/001-*`, `plans/002-*`, or `plans/003-*`.

### Subsystem roadmap

The dependency-ordered Proposal 170 workstream plan under `plans/subsystems/`.

### Implementation plan

A bounded agent handoff tied to a repository baseline and one roadmap milestone.

### Closure record

An evidence-based decision establishing whether a milestone is closed, conditionally closed, blocked, or requires correction.

### Corrective plan

A new implementation plan that owns unclosed findings from a closure record. It does not rewrite history or silently weaken the original milestone.
