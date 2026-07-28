# Proposal 170 Support Status

Status: M004 implemented

This document tracks the implementation status of Proposal 170 I2PControl expansion in Emissary.

## Method support

| Method | Status | Milestone |
|---|---|---|
| `Authenticate` | Implemented | M001 |
| `GetKeys` | Not started | — |
| `RouterInfo` | Not started | M005 |
| `AddressBook` | Implemented | M003 |
| `TunnelManager` | Implemented | M004 |
| `ClientServicesInfo` | Not started | M006 |

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

Not yet implemented. Planned for M005.

## ClientServicesInfo selectors

Not yet implemented. Planned for M006.

## Security

- Timing-resistant password comparison
- Token-based authentication
- Request body and string length limits
- Secret redaction in logs and Debug output
- No file system mutations outside persistence store
- No network activity in handler code

## Roadmap

| Milestone | Status | Description |
|---|---|---|
| M001 | Closed | Base protocol, auth, JSON-RPC |
| M002 | Closed | Tunnel domain, persistence, backend trait |
| M003 | Closed | AddressBook handler |
| M004 | Closed | TunnelManager contract and stubs |
| M005 | Ready | RouterInfo inspection |
| M006 | Blocked | ClientServicesInfo |
| M007 | Blocked | Integration, restart, security hardening |
