# RouterInfo Method

Status: M009 implemented (availability and truthfulness)

This document describes the Proposal 170 `RouterInfo` JSON-RPC method implementation in Emissary.

## Overview

The `RouterInfo` method allows authenticated callers to request specific router state data using exact selector-by-presence behavior. Only requested selector keys appear in the response.

## Selector registry

All 121 Proposal 170 RouterInfo selectors are registered in `rpc.rs` and verified by the `router_info_selectors_complete` test.

### Selector groups

| Group | Prefix | Count | Source |
|---|---|---|---|
| Identity/static | `i2p.router.identity`, `i2p.router.version`, `i2p.router.uptime` | 3 | Startup-retained values |
| Router news | `i2p.router.news` | 1 | Empty string (no news subsystem) |
| Clock skew | `i2p.router.clock.skew` | 1 | Fake (requires core integration) |
| Network status | `i2p.router.net.bw.*` | 2 | Fake (requires core integration) |
| Share ratio | `i2p.router.shareRatio` | 1 | Fake (requires config integration) |
| Configured BW | `i2p.router.configuredbw.*` | 2 | Fake (requires config integration) |
| UDP transport | `i2p.router.udp.*` | 7 | Fake (requires core integration) |
| TCP transport | `i2p.router.tcp.*` | 7 | Fake (requires core integration) |
| NetDB | `i2p.router.netdb.*` | 10 | Fake (requires core integration) |
| Bandwidth | `i2p.router.bw.*` | 14 | MetricsSnapshot + RollingWindow |
| Tunnels | `i2p.router.tunnels.*` | 7 | Fake (requires core integration) |
| I2PTunnel | `i2p.router.iptunnels` | 1 | Fake (requires M004 integration) |
| Peers | `i2p.router.peers.*` | 10 | Fake (requires core integration) |
| Logs | `i2p.router.log.*` | 2 | LogRing (tracing layer) |
| Address book | `i2p.router.addressbook.*` | 6 | M003 adapter |

### Selector behavior

- Selector-by-presence: only keys with `true` value in the `Selector` object are included
- Keys with `false` or missing are omitted from the response
- Unknown selector keys return an error (`INVALID_PARAMS`)
- Only requested keys appear in the response (no unrelated keys)

## Startup-retained values

At startup, the following values are retained and never re-read from disk:

- `router_id`: Local router identity in Base64
- `router_info_bytes`: Serialized local RouterInfo bytes
- `router_info_b64`: Base64 encoding of serialized RouterInfo
- `startup_time`: Server startup instant for uptime calculation

These are set via `I2pControlState::set_startup_values()` during router initialization.

## Bounded metrics

### Cumulative counters (MetricsSnapshot)

- `total_transport_received`: Cumulative inbound transport bytes
- `total_transport_sent`: Cumulative outbound transport bytes
- `total_transit_received`: Cumulative inbound transit bytes
- `total_transit_sent`: Cumulative outbound transit bytes
- `connected_routers`: Number of connected routers
- `participating_tunnels`: Number of transit tunnels
- `tunnel_build_successes`: Cumulative tunnel build successes
- `tunnel_build_failures`: Cumulative tunnel build failures

Counters are monotonic except process restart. Reads are non-destructive.

### Rolling window (RollingWindow)

1-second buckets covering multiple intervals:

| Interval | Buckets | Memory |
|---|---|---|
| 1 second | 1 | ~24 bytes |
| 15 seconds | 15 | ~360 bytes |
| 1 minute | 60 | ~1.4 KB |
| 1 hour | 3600 | ~86 KB |
| 1 day | 86400 | ~2 MB |

Rolling window resets on process restart. No historical data is fabricated.

## Bounded log buffer (LogRing)

- Fixed maximum entries (default: 1000) and total bytes (default: 512 KB)
- Deterministic oldest-entry eviction
- Redaction of Base64 private keys (>=40 chars), `password=`, and `token=` patterns
- Clear affects only the I2PControl ring; terminal/file sinks unchanged
- Concurrent readers receive coherent snapshot
- Wired as `tracing_subscriber::Layer` for automatic event capture

## Response budgets

Pre-query budget estimation prevents oversized responses:

| Selector | Limit |
|---|---|
| Peer identities (known/active) | 10,000 |
| Peer RouterInfo bytes | 4 MB |
| Active peer stats | 10,000 |
| Banned peers | 10,000 |
| Log entries | 10,000 |
| Total response | 10 MB |

If estimated response exceeds bounds, the request fails with an explicit error before any expensive queries are issued.

Per-selector item bounds enforce limits on returned collections.

## Null/unavailable behavior

- Clock skew: `null` when not yet determined (protocol-permitted nullable)
- Router news: empty string (no news subsystem; retained constant)
- Peer RouterInfo: `null` when no peer ID specified
- Network status: exact string codes ("OK", "Firewalled", "Testing", etc.)
- Share ratio: from retained configuration
- Unavailable non-null selectors: return JSON-RPC error with no partial result
- Available-zero selectors: return successful zero/empty values
- Source failure: distinguished from unavailable and from empty

## Read-only architecture

- No mutation of router state from inspection requests
- No consumption of `EventSubscriber` (frontend events preserved)
- No mutable core handles exposed
- No private keys, session keys, or authentication tokens in responses
- Log redaction applied before ring insertion
- Core remains free of HTTP/JSON-RPC dependencies

## Limitations (deferred to M010)

- NetDB, TCP transport, peer list/lookup/stats: `unsupported-inspection`
- UDP transport peers (integrated, coinficient, critical, etc.): `unsupported-inspection`
- Tunnel exploratory/client inbound/outbound, queue depth: `unsupported-inspection`
- Clock skew estimator: not yet implemented
