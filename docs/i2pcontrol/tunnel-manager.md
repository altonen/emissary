# I2PControl TunnelManager

Status: M018 canonical wire implementation; M019 independent closure pending

This document describes the Proposal 170 TunnelManager API handler in Emissary.

## Overview

The TunnelManager handler implements the `TunnelManager` JSON-RPC method for all declared tunnel types. It provides:

- Canonical CRUD operations (create, edit, get, delete) for tunnel definitions
- Compatibility `List` and capitalized action values
- Lifecycle dispatch (start, stop, restart) through the backend registry
- Ownership enforcement for startup-managed tunnels
- Deterministic unsupported backend behavior for all 12 tunnel types

## Actions

The seven canonical actions are lowercase:

| Action | Description |
|---|---|
| `create` | Create a new tunnel definition |
| `edit` | Update an existing tunnel definition |
| `get` | Retrieve a tunnel definition by name |
| `delete` | Remove a tunnel definition |
| `start` | Start a tunnel through its backend |
| `stop` | Stop a tunnel through its backend |
| `restart` | Restart a tunnel (stop then start) |

Capitalized values and `List` are compatibility extensions, not Proposal 170
actions. `List` is excluded from the canonical action manifest.

### Action restrictions

`All` is accepted only for `Start`, `Stop`, and `Restart`. It is rejected with an error for `Create`, `Edit`, `Get`, and `Delete`.

## Tunnel types

All 12 Proposal 170 tunnel types are accepted:

| Type | Category |
|---|---|
| `client` | Client |
| `httpclient` | Client |
| `ircclient` | Client |
| `socks` | Client |
| `socksirc` | Client |
| `connectclient` | Client |
| `streamrclient` | Client |
| `server` | Server |
| `httpserver` | Server |
| `httpbidirserver` | Server |
| `ircserver` | Server |
| `streamrserver` | Server |

## Request format

All requests follow the JSON-RPC 2.0 envelope:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "TunnelManager",
  "params": {
    "Action": "<action>",
    "Type": "<type>",
    "Name": "<name>",
    ...
  }
}
```

### Required fields

- `Action` - One of the seven lowercase canonical action strings
- `Name` - Required for all canonical actions
- `Type` - Required for `create`

### Optional fields

- `All` - Boolean, valid only for `Start`, `Stop`, `Restart`
- `NewName` - String, valid only for `Edit` (atomic rename)
- Tunnel option fields (see Tunnel options)

## CRUD operations

### Create

Creates a new tunnel definition with the specified type and name.

```json
{
  "Action": "create",
  "Type": "socks",
  "Name": "my-proxy",
  "i2p.tunnel.listenPort": 1080
}
```

- `Type` and `Name` are required
- Duplicate names return a Proposal 170 status error
- Control-plane ownership is assigned automatically
- `StartOnLoad` is stored but does not start the tunnel

### Edit

Updates an existing tunnel definition. Preserves omitted fields.

```json
{
  "Action": "edit",
  "Name": "my-proxy",
  "NewName": "renamed-proxy",
  "i2p.tunnel.listenPort": 2080
}
```

- `Name` is required
- `NewName` performs an atomic rename with collision detection
- Startup-managed definitions are rejected

### Get

Retrieves a tunnel definition by name.

```json
{
  "Action": "get",
  "Name": "my-proxy"
}
```

Canonical `get` returns `{status, info}`. Canonical `create`, `edit`, lifecycle,
and `delete` return a structured `{status}` result, with `results` where the
operation returns a result list. Compatibility requests retain their historical
response shapes.

### Delete

Removes a tunnel definition.

```json
{
  "Action": "delete",
  "Name": "my-proxy"
}
```

- Startup-managed definitions are rejected
- Delete of absent name is a successful no-op

### Compatibility List

Returns all tunnel definitions as an array.

```json
{
  "Action": "List"
}
```

## Lifecycle operations

### Start

```json
{
  "Action": "start",
  "Name": "my-proxy"
}
```

Dispatches through the backend registry. Returns:
- `ok - <type> started` for supported backends
- `error - <type> not implemented` for unsupported backends
- Ownership error for startup-managed definitions

### Stop

```json
{
  "Action": "stop",
  "Name": "my-proxy"
}
```

For unsupported backends, stop is safe and idempotent.

### Restart

```json
{
  "Action": "Restart",
  "Name": "my-proxy"
}
```

Composed as stop then start through the backend registry.

### All behavior

```json
{
  "Action": "Start",
  "All": true
}
```

- Bounded serial dispatch over all definitions
- Startup-managed definitions are skipped
- Maximum 1000 targets
- Returns aggregated status

## Ownership model

| Ownership | Mutations | Lifecycle | Source |
|---|---|---|---|
| `ControlPlane` | Allowed | Allowed | Created via `Create` action |
| `StartupManaged` | Rejected | Rejected | Existing startup configuration |

## Tunnel options

The following matrix is the canonical Proposal 170 option inventory. Fields in
the second row are parsed into typed Emissary options; fields in the third row
are accepted, validated where the proposal gives a bound/type, and retained in
`rawConfig` for lossless round-trip. Retention is wire/CRUD support, not runtime
data-plane support.

| Disposition | Proposal 170 fields |
|---|---|
| Parsed and round-tripped | `Description`, `StartOnLoad`, `TargetDestination`, `Destination`, `TargetPort`, `ReachableBy`, `Port`, `TargetHost`, `Host` |
| Validated and retained in raw configuration | `TunnelLength` (0–3), `TunnelVariance` (−2–2), `TunnelQuantity` (1–6), `TunnelBackupQuantity` (0–3), `Shared`, `UseSSL`, `UseOutproxyPlugin`, `ProxyAuth`, `OutproxyAuth`, `DelayOpen`, `Reduce`, `Close`, `NewDest`, `PersistentClientKey`, `AllowInternalSSL`, `BlockAccessInProxies`, `UniqueLocalAddressPerClient`, `MultiHoming`, `CustomOptions`, `LeaseSetClientAuths` |
| Accepted and retained in raw configuration | `SigType`, `EncType`, `ProxyList`, `ProxyUsername`, `ProxyPassword`, `OutproxyUsername`, `OutproxyPassword`, `OutproxyType`, `SSLProxies`, `JumpList`, `ConnectDelay`, `Profile`, `ReduceCount`, `ReduceTime`, `CloseTime`, `PrivKeyFile`, `AllowUserAgent`, `AllowReferer`, `AllowAccept`, `WebsiteHostname`, `SpoofedHost`, `BlockUserAgents`, `UserAgents`, `BlockReferers`, `AccessOption`, `AccessList`, `FilterFilePath`, `MaxConcurrentConns`, `ClientPerMinute`, `ClientPerHour`, `ClientPerDay`, `TotalInPerMinute`, `TotalInPerHour`, `TotalInPerDay`, `PostLimit`, `PostLimitTime`, `PerClientPeriod`, `TotalPeriod`, `TotalBanTime`, `OptionalLookup`, `EncryptLeaseSet` |

No field in this matrix starts a tunnel, creates a data-plane session, or
reports a fabricated runtime state. Unsupported backends return an explicit
status/error while preserving durable definitions.

### General

| Field | Type | Description |
|---|---|---|
| `description` | string | Tunnel description |
| `i2p.tunnel.startOnLoad` | boolean | Start on router load (stored, not executed) |

### Client options

| Field | Type | Description |
|---|---|---|
| `i2p.tunnel.clientDest` | string | Target destination |
| `i2p.tunnel.clientDestPort` | integer | Target port (0-65535) |
| `i2p.tunnel.listenInterface` | string | Listen interface |
| `i2p.tunnel.listenPort` | integer | Listen port (0-65535) |
| `i2p.tunnel.accessList` | string | Comma-separated destinations |
| `i2p.tunnel.allowplaintext` | boolean | Allow plaintext to I2P |

### Server options

| Field | Type | Description |
|---|---|---|
| `i2p.tunnel.serverHostingDestination` | string | Base64 RouterInfo |
| `i2p.tunnel.isPrivate` | boolean | Hidden service |
| `i2p.tunnel.hashcashProofsRequired` | integer | Hash cash level |
| `i2p.tunnel.signatureType` | string | Signature type |
| `i2p.tunnel.consumer` | string | Consumer |

### HTTP options

| Field | Type | Description |
|---|---|---|
| `i2p.tunnel.sslCertificate` | string | SSL certificate path |
| `i2p.tunnel.sslKey` | string (redacted) | SSL key path |
| `i2p.tunnel.httpHost` | string | HTTP host |

### Proxy options

| Field | Type | Description |
|---|---|---|
| `i2p.tunnel.proxyUsername` | string | Proxy username |
| `i2p.tunnel.proxyPassword` | string (redacted) | Proxy password |

### IRC options

| Field | Type | Description |
|---|---|---|
| `i2p.tunnel.ircServer` | string | IRC server address |
| `i2p.tunnel.ircPort` | integer | IRC port (0-65535) |
| `i2p.tunnel.ircNick` | string | IRC nick |
| `i2p.tunnel.ircPassword` | string (redacted) | IRC password |
| `i2p.tunnel.ircChannels` | string | Comma-separated channels |

### Streamr options

| Field | Type | Description |
|---|---|---|
| `i2p.tunnel.streamrTarget` | string | Streamr target |

### I2CP and custom options

- `i2cp` - Object with string key-value pairs
- `i2p.tunnel.customOptions` - Object with string key-value pairs

## Security

- Secret fields (`ssl_key`, `proxy_password`, `irc_password`) are redacted in Debug/Display
- Names are validated for length (max 1024) and control characters
- Descriptions are validated for length (max 4096)
- Port values are validated for u16 range
- Error messages do not expose Rust type names or internal state

## Static guards

- No `std::fs`, `tokio::fs`, or `std::net` imports in handler code
- No `tokio::spawn` calls
- No `dioxus` or UI module imports
- No `router.toml` mutations
- Unsupported backends allocate zero network/session/task resources

## Tests

The handler includes 68 tests covering:

- Action and type parsing validation
- CRUD operations for all 12 tunnel types
- Edit with rename, field preservation, and collision detection
- Lifecycle operations through unsupported and fake backends
- `All` enforcement and mixed-target behavior
- Ownership rejection for startup-managed definitions
- Concurrent and race condition determinism
- Secret redaction in Debug/Display
- Static guards for no resource allocation
