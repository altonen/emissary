# ClientServicesInfo

Proposal 170 `ClientServicesInfo` method implementation in Emissary.

## Overview

`ClientServicesInfo` is an observation-only JSON-RPC method that returns
the runtime state of application-owned client services. The method is
registered in the M001 method registry and dispatched by
`emissary-cli/src/i2pcontrol/server.rs`.

The implementation is strictly passive: it observes proxies, listeners,
and session state without taking ownership, supervision, or control of
any service. No method invocation starts, stops, restarts, rebinds, or
reconfigures a service.

## Request shape

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "ClientServicesInfo",
  "params": {
    "Selector": {
      "I2PTunnel": true,
      "HTTPProxy": true,
      "SOCKS": true,
      "SAM": true,
      "BOB": true,
      "I2CP": true
    }
  }
}
```

The `Selector` parameter is a JSON object whose keys are the six
Proposal 170 client-service categories. Only entries whose value is
`true` produce a response section; absent keys or `false` values cause
the corresponding section to be omitted.

Only the six exact selector keys listed below are accepted. Any other
key returns `INVALID_PARAMS`.

## Response shape

The response contains one entry per requested selector. Omitted
selectors do not appear in the response.

### `I2PTunnel`

Always returns an object with `client` and `server` keys. Each value is
a map of tunnel name to entry object (currently `{address}`, optionally
`{port}` for server tunnels). Empty when no definitions exist.

```json
{
  "I2PTunnel": {
    "client": {
      "my-client-tunnel": { "address": "abcd.b32.i2p" }
    },
    "server": {
      "my-server-tunnel": { "address": "host.b32.i2p", "port": 80 }
    }
  }
}
```

**Live query (M011):** I2PTunnel inventory is queried from the shared
`TunnelManagerControl` at request time, not from a startup-only
registry snapshot. Successful Create/Edit/Rename/Delete mutations are
visible to the next ClientServicesInfo query without restart. Store
failures propagate as JSON-RPC errors rather than empty inventory.

Unsupported definitions appear in the map but always with `Configured`
state — they never report active/listening/running.

### `HTTPProxy`

Reports the HTTP proxy lifecycle.

```json
{
  "HTTPProxy": {
    "enabled": true,
    "address": "127.0.0.1",
    "port": 4444
  }
}
```

`enabled: true` only after a successful bind (`Listening` state).
`Configured` and `Starting` states report `enabled: false` because no
listener has actually bound yet. When `Failed` or `Stopped`, the
response is `{enabled: false}`.

### `SOCKS`

Reports the SOCKS proxy lifecycle, same shape as `HTTPProxy`.

```json
{
  "SOCKS": {
    "enabled": true,
    "address": "127.0.0.1",
    "port": 1080
  }
}
```

Same `enabled` semantics as HTTPProxy: only `Listening` reports
`enabled: true`.

### `SAM`

Reports the SAM bridge listener and session state.

```json
{
  "SAM": {
    "enabled": true,
    "sessions": {}
  }
}
```

`enabled` reflects the actual bound TCP listener state. `Configured`
and `Starting` report `enabled: false`.

`sessions` is always an empty object. Proposal 170 requires the
`sessions` field. The core `SamServer` tracks active sessions
internally but does not yet expose a public bounded session accessor.
Without a safe canonical read source, the sessions object is the
protocol-compatible response when session state cannot be observed.
This is not a placeholder for missing inspection.

### `BOB`

Always returns the exact boolean `false` because Emissary does not
implement BOB. No BOB listener, stub server, port, or configuration
exists.

```json
{
  "BOB": false
}
```

### `I2CP`

Reports whether the core I2CP listener is bound.

```json
{
  "I2CP": { "enabled": true }
}
```

`enabled: true` means the core router bound a local I2CP listener
during startup and it remains active. `Configured` and `Starting`
report `enabled: false`. `enabled: false` also means I2CP was not
configured or the bind failed.

## Lifecycle states

Each proxy transition is recorded through a generation-fenced
[`ServiceUpdateHandle`]. The internal state vocabulary
(`Disabled`, `Configured`, `Starting`, `Listening`, `Failed`,
`Stopping`, `Stopped`) is private; the JSON-RPC layer maps these to the
exact Proposal 170 response shape.

### Configured vs. listening semantics

- `Configured` means the service exists in configuration and is being
  initialized, but is not yet listening on a bound address.
- `Starting` means the task has been spawned but `bind()` has not
  succeeded yet.
- `Listening` means the service has successfully bound a local
  address and is actively serving requests.
- Only `Listening` maps to `enabled: true` in the public response.
  `Configured` and `Starting` always map to `enabled: false`.
- For I2PTunnel, the response does not distinguish configured from
  listening — it returns the inventory regardless. Unsupported
  tunnel definitions never appear in a running state.

### SAM listener vs. session semantics

`SAM` reports two distinct things:

- Listener enabled: a bound TCP/UDP address on the configured port.
  This corresponds to `SamServer::tcp_local_address()` returning
  `Some(_)`.
- Active sessions: The core `SamServer` tracks active sessions via
  `SessionContext<R, Arc<str>>` but does not expose a public bounded
  session snapshot accessor. The sessions object is the
  protocol-compatible empty object when session state cannot be
  observed. This is not a placeholder for missing inspection.

### I2PTunnel live query

I2PTunnel inventory is no longer populated at startup and stored in
the registry. Instead, the handler queries `TunnelManagerControl::list()`
at request time. This ensures:

- Successful Create/Edit/Rename/Delete mutations are visible immediately
- Store failures propagate as JSON-RPC errors, not empty inventory
- No stale startup-only inventory can persist across mutations
- Unsupported definitions appear in the inventory but never as
  active/running/listening

### I2PTunnel stub inactivity

Unsupported M004 backends (e.g., IRC, Streamr) report definitions but
never report runtime state. Their entries appear in the
`client`/`server` maps but never with active/running fields.

## Composition and wiring

The passive client-service registry lives at the heart of the
implementation. It is created in the application composition root
(`emissary-cli/src/main.rs`) and shared by `Arc` between:

- The I2PControl state (`I2pControlState::service_registry`),
- HTTP proxy spawn sites,
- SOCKS proxy spawn sites,
- I2CP listener snapshot from `Router::protocol_address_info()`,
- SAM listener snapshot from `Router::protocol_address_info()`.

The handler queries `TunnelManagerControl::list()` at request time for
I2PTunnel inventory rather than reading from the registry.

Producers in the composition root use `i2pcontrol::observers::*`
helpers to emit transition events through generation-fenced
`ServiceUpdateHandle` instances. Stale handles from previous startup
generations cannot overwrite current state.

The registry is independent of the UI layer. When I2PControl is
disabled, the registry and observers are not compiled. When the UI is
enabled, the registry behaves identically — no UI event/state is
consulted.

## Response bounds

Response size is bounded before dispatch:

- `MAX_RESPONSE_BYTES` = 1 MiB (estimated)
- `MAX_TUNNEL_DEFINITIONS` = 1000 per I2PTunnel map
- `MAX_SAM_SESSIONS` = 1000 per SAM snapshot

Complete results that exceed safe bounds fail explicitly with
`INTERNAL_ERROR` and are never silently truncated.

## Failure and sanitization

Proxy failures are recorded as `Failed(SanitizedFailure)`. The
`SanitizedFailure` shape contains only:

- `error_kind` — short OS error kind name (e.g.,
  `"ConnectionRefused"`).
- `address` — optional socket address.

No credentials, private keys, complete configuration, internal paths,
or Rust backtraces are included. When the exact proposal response
shape has no failure-detail field (HTTPProxy/SOCKS/I2CP), the failure
is mapped to `{enabled: false}` and the sanitized detail is retained
internally only.

## No lifecycle control

The method never:

- Starts, stops, restarts, rebinds, or reconfigures a service.
- Consumes frontend or UI events.
- Parses log messages.
- Performs direct M004 persistence-file reads (the inventory is
  read via `TunnelManagerControl::list()` at request time).
- Returns SAM session private keys, destination material, or
  authentication data.

## Tests

Unit tests live alongside the implementation:

- `service_registry.rs` — registry semantics.
- `client_services.rs` — handler, selector dispatch, live tunnel
  query, and proxy enabled-state correctness.
- `observers.rs` — passive observer helpers.

Integration tests:

- `tests/client_services_integration.rs` — selector parsing,
  response shape, lifecycle observations, and concurrent registry
  updates.

Static guards:

- `tests/static_guards.rs` — M011 guards verifying no startup-only
  I2PTunnel population, no unconditional SAM sessions placeholder,
  Configured/Starting not reported as enabled, and handler uses
  live tunnel manager.

## References

- Plan: `plans/implementation/i2pcontrol-proposal-170/011-client-services-live-state.md`
- M006 plan: `plans/implementation/i2pcontrol-proposal-170/006-client-services-info.md`
- M006 closure: `plans/closure/i2pcontrol-proposal-170/006-closure.md`
- M011 closure: `plans/closure/i2pcontrol-proposal-170/011-closure.md`
- M001 conformance matrix: `docs/i2pcontrol/proposal-170-conformance.md`
- Proposal 170 support status: `docs/i2pcontrol/proposal-170-support.md`
