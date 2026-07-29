# Proposal 170 Support Status

Status: M008 implemented (production composition and durable-state integrity closed)

> **Note:** Strict Proposal 170 closure is reopened until M008–M012 close. Prior M005–M007
> closure records are not accepted as current strict closure after the corrective review.

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

Real backend implementations will be added in future milestones.

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

121 selectors registered and dispatched. See [router-info.md](router-info.md) for the full selector catalog.

| Selector group | Status | Notes |
|---|---|---|
| Identity/static | Implemented | Startup-retained values |
| Router news | Implemented | Empty string (no news subsystem) |
| Clock skew | Fake | Requires core integration |
| Network status | Fake | Requires core integration |
| Share ratio | Fake | Requires config integration |
| Configured BW | Fake | Requires config integration |
| UDP/TCP transport | Fake | Requires core integration |
| NetDB | Fake | Requires core integration |
| Bandwidth (all intervals) | Implemented | MetricsSnapshot + RollingWindow |
| Tunnels | Fake | Requires core integration |
| I2PTunnel | Fake | Requires M004 adapter |
| Peers | Fake | Requires core integration |
| Logs | Implemented | LogRing with redaction |
| Address book | Implemented | M003 adapter |

## ClientServicesInfo selectors

Implemented in M006. The `ClientServicesInfo` method accepts a `Selector` map
with boolean values for the six exact Proposal 170 keys:

| Selector | Response shape | Source |
|---|---|---|
| `I2PTunnel` | `{client: {…}, server: {…}}` | `ProductionTunnelManagerControl::list()` |
| `HTTPProxy` | `{enabled, address, port}` | HTTP proxy `Listening`/`Stopped` |
| `SOCKS` | `{enabled, address, port}` | SOCKS proxy `Listening`/`Stopped` |
| `SAM` | `{enabled, sessions}` | core `SamServer::tcp_local_address()` |
| `BOB` | `false` (boolean) | exact Proposal 170 value (not implemented) |
| `I2CP` | `{enabled}` | core `ProtocolAddressInfo::i2cp` |

See [`docs/i2pcontrol/client-services.md`](client-services.md) for the
full method documentation, including configured-vs-listening semantics
and integration evidence.

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
| M005 | Corrective pass required | RouterInfo inspection (121 selectors, bounded metrics, logs) |
| M006 | Corrective pass required | ClientServicesInfo (composition wiring complete) |
| M007 | Corrective pass required | Conformance and strict closure |
| M008 | Closed | Production composition and durable-state integrity |
