# Proposal 170 Support Status

Status: M016 implementation complete; M017 independent final-head reclosure pending

This document tracks the implementation status of Proposal 170 I2PControl expansion in Emissary.

## Method support

| Method | Status | Milestone |
|---|---|---|
| `Authenticate` | Implemented | M001 |
| `GetKeys` | Not started | — |
| `RouterInfo` | Implemented (corrective pass) | M005 |
| `AddressBook` | Implemented | M003 |
| `TunnelManager` | Implemented | M004 |
| `ClientServicesInfo` | Implemented | M006 |

## TunnelManager action support

| Action | Status | Notes |
|---|---|---|
| `List` | Implemented | Returns all definitions |
| `Create` | Implemented | All 12 types |
| `Edit` | Implemented | Atomic rename, field preservation |
| `Get` | Implemented | Lossless round-trip |
| `Delete` | Implemented | Startup-managed rejected |
| `Start` | Implemented | Backend dispatch |
| `Stop` | Implemented | Idempotent for unsupported |
| `Restart` | Implemented | Stop then start |

## Tunnel type runtime support

| Type | CRUD | Start | Stop | Restart |
|---|---|---|---|---|
| `client` | Implemented | Not implemented | Safe no-op | Not implemented |
| `httpclient` | Implemented | Not implemented | Safe no-op | Not implemented |
| `ircclient` | Implemented | Not implemented | Safe no-op | Not implemented |
| `socks` | Implemented | Not implemented | Safe no-op | Not implemented |
| `socksirc` | Implemented | Not implemented | Safe no-op | Not implemented |
| `connectclient` | Implemented | Not implemented | Safe no-op | Not implemented |
| `streamrclient` | Implemented | Not implemented | Safe no-op | Not implemented |
| `server` | Implemented | Not implemented | Safe no-op | Not implemented |
| `httpserver` | Implemented | Not implemented | Safe no-op | Not implemented |
| `httpbidirserver` | Implemented | Not implemented | Safe no-op | Not implemented |
| `ircserver` | Implemented | Not implemented | Safe no-op | Not implemented |
| `streamrserver` | Implemented | Not implemented | Safe no-op | Not implemented |

### CRUD

All 12 tunnel types support durable create, lossless get, edit with field preservation, and delete. Startup-managed definitions are read-only.

### Lifecycle

All tunnel types currently resolve to `UnsupportedTunnelBackend`. Start and restart return `error - <type> not implemented`. Stop is a safe idempotent no-op.

Real tunnel data-plane implementations are deferred outside the I2PControl scope. The Proposal 170 contract is satisfied by explicit unsupported stubs.

## AddressBook support

| Action | Status | Notes |
|---|---|---|
| `List` | Implemented | Per-book listing |
| `Lookup` | Implemented | Hostname lookup |
| `Add` | Implemented | Entry insertion |
| `Update` | Implemented | Entry update |
| `Delete` | Implemented | Per-entry or per-book |

### Books

| Book | Status |
|---|---|
| `private` | Implemented |
| `local` | Implemented |
| `router` | Implemented |
| `published` | Implemented |

## RouterInfo selectors

121 selectors registered and dispatched. See [router-info.md](router-info.md) for the full selector catalog and [router-info-source-map.md](router-info-source-map.md) for the complete source map.

| Selector group | Status | Notes |
|---|---|---|
| Identity/static | Implemented | Startup-retained values |
| Router news | Implemented | Empty string (no news subsystem) |
| Clock skew | Implemented | Protocol-permitted null (no clock skew estimate) |
| Network status | Implemented | EventMetrics firewall status |
| Share ratio | Implemented | Retained configuration |
| Configured BW | Implemented | Retained configuration |
| UDP transport | Unavailable | No transport-specific canonical source; aggregate connection counts are not used |
| UDP transport (peers, stats, cookie, hidden) | Unavailable | No Emissary equivalent for Java-I2P peer categories |
| TCP transport | Unavailable | No transport-specific canonical source |
| TCP transport (hosts, status, version, firewalled, peers) | Unavailable | No Emissary equivalent |
| NetDB | Unavailable | NetDB task is spawned; no inspection interface |
| Bandwidth totals | Implemented | Live EventMetrics adapter |
| Bandwidth recent windows | Unavailable | No canonical production rolling-window source |
| Tunnels (participating, configured) | Implemented | Live EventMetrics + TunnelManager |
| Tunnels (exploratory, client, queue) | Unavailable | Tunnel pool tasks are spawned; no inspection interface |
| I2PTunnel | Implemented | TunnelManager administrative store |
| Peers (known, active, RouterInfo lookup) | Unavailable | No bounded live source exposed by Emissary core |
| Peers (banned, limits, activeStats) | Unavailable | No canonical ban owner or per-peer transport stats |
| Logs | Implemented | LogRing with redaction |
| Address book | Implemented | M003 adapter |

## ClientServicesInfo selectors

Implemented in M006, corrected in M011 for live state truthfulness.
The `ClientServicesInfo` method accepts a `Selector` map
with boolean values for the six exact Proposal 170 keys:

| Selector | Response shape | Source | M011 status |
|---|---|---|---|
| `I2PTunnel` | `{client: {…}, server: {…}}` | `TunnelManagerControl::list()` (live query) | Live query at request time |
| `HTTPProxy` | `{enabled, address, port}` | HTTP proxy `Listening`/`Stopped` | `enabled: true` only after bind |
| `SOCKS` | `{enabled, address, port}` | SOCKS proxy `Listening`/`Stopped` | `enabled: true` only after bind |
| `SAM` | `{enabled, sessions}` | core listener plus bounded `SamServer` observation handle | Implemented; overflow fails explicitly and never returns a partial session map |
| `BOB` | `false` (boolean) | exact Proposal 170 value (not implemented) | Unchanged |
| `I2CP` | `{enabled}` | core `ProtocolAddressInfo::i2cp` | `enabled: true` only while bound |

See [`docs/i2pcontrol/client-services.md`](client-services.md) for the
full method documentation, including live query semantics,
configured-vs-listening semantics, and integration evidence.

## Security

- Timing-resistant password comparison
- Token-based authentication
- Request body and string length limits
- Secret redaction in logs and Debug output
- Log ring redaction of private keys, passwords, tokens
- No file system mutations outside persistence store
- No network activity in handler code
- Pre-query budget estimation prevents oversized responses
- Per-selector item bounds enforce collection limits
- No `EventSubscriber` consumption (frontend events preserved)
- No private keys or session material in responses

## Roadmap

| Milestone | Status | Description |
|---|---|---|
| M001 | Closed | Base protocol, auth, JSON-RPC |
| M002 | Closed | Tunnel domain, persistence, backend trait |
| M003 | Closed | AddressBook handler |
| M004 | Closed | TunnelManager contract and stubs |
| M005 | Superseded | RouterInfo inspection (superseded by M009/M010) |
| M006 | Superseded | ClientServicesInfo (superseded by M011) |
| M007 | Superseded | Conformance and strict closure (superseded by M012/M013) |
| M008 | Closed | Production composition and durable-state integrity |
| M009 | Closed | RouterInfo availability and truthfulness |
| M010 | Closed | Bounded core router inspection |
| M011 | Closed | ClientServicesInfo live state |
| M012 | Closed | Real TLS and request resource hardening |
| M013 | Closed | Production conformance and independent reclosure |
