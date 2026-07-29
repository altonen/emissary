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

When the proxy is configured but not yet listening, `address` and
`port` may be absent. When `Failed` or `Stopped`, the response is
`{enabled: false}`.

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

`enabled` reflects the actual bound TCP listener state. `sessions` is
present and an object (currently always empty — bounded session
snapshot integration is tracked in the closure record).

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
during startup. `enabled: false` means I2CP was not configured or the
bind failed.

## Lifecycle states

Each proxy transition is recorded through a generation-fenced
[`ServiceUpdateHandle`]. The internal state vocabulary
(`Disabled`, `Configured`, `Starting`, `Listening`, `Failed`,
`Stopping`, `Stopped`) is private; the JSON-RPC layer maps these to the
exact Proposal 170 response shape.

### Configured vs. listening semantics

- `Configured` means the service exists in configuration and is being
  initialized, but is not yet listening on a bound address.
- `Listening` means the service has successfully bound a local
  address and is actively serving requests.
- For HTTP/SOCKS/I2CP/SAM, `Listening` requires a successful
  `bind()` (or equivalent) and is recorded from the actual
  `local_addr()` returned by the core listener.
- For I2PTunnel, the response does not distinguish configured from
  listening — it returns the inventory regardless. Unsupported
  tunnel definitions never appear in a running state.

### SAM listener vs. session semantics

`SAM` reports two distinct things:

- Listener enabled: a bound TCP/UDP address on the configured port.
  This corresponds to `SamServer::tcp_local_address()` returning
  `Some(_)`.
- Active sessions: bounded count of SAM sessions currently in the
  core `SessionContext`. This is recorded as 0 because the core does
  not yet expose a bounded session snapshot. The response shape is
  stable; the count will populate when core exposes a public
  bounded snapshot accessor.

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
- SAM listener snapshot from `Router::protocol_address_info()`,
- I2PTunnel inventory from `ProductionTunnelManagerControl::list()`.

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
  read via `TunnelManagerControl::list()`).
- Returns SAM session private keys, destination material, or
  authentication data.

## Tests

Unit tests live alongside the implementation:

- `service_registry.rs` — 18 tests for registry semantics.
- `client_services.rs` — 44 tests for handler and selector dispatch.
- `observers.rs` — 14 tests for passive observer helpers.

Integration tests:

- `tests/client_services_integration.rs` — 15 tests covering
  selector parsing, response shape, lifecycle observations, and
  concurrent registry updates.

## References

- Plan: `plans/implementation/i2pcontrol-proposal-170/006-client-services-info.md`
- Closure: `plans/closure/i2pcontrol-proposal-170/006-closure.md`
- M001 conformance matrix: `docs/i2pcontrol/proposal-170-conformance.md`
- Proposal 170 support status: `docs/i2pcontrol/proposal-170-support.md`
