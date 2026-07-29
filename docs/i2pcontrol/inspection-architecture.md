# I2PControl Inspection Architecture

Status: M008 implemented (production composition closed)

This document describes the read-only inspection architecture for I2PControl Proposal 170 in Emissary.

## Design principles

1. **Read-only boundary**: Inspection requests never mutate router state
2. **Truthful state**: No fabricated values; unavailable data returns protocol-permitted null/error
3. **Bounded responses**: All collections and byte sizes have explicit limits
4. **No event consumption**: `EventSubscriber` is never consumed by I2PControl
5. **No core dependencies**: Core remains free of HTTP/JSON-RPC/Serde-JSON server dependencies
6. **Fail-closed startup**: Production store failures abort I2PControl initialization
7. **Shared identity**: All tunnel consumers share one loaded service object via `Arc`

## Architecture layers

```
┌─────────────────────────────────────────────┐
│  I2PControl HTTPS Server (axum)             │
│  ├── Authentication (token service)         │
│  ├── JSON-RPC dispatch                      │
│  └── Concurrency limiter (Semaphore)        │
├─────────────────────────────────────────────┤
│  RouterInfo Handler                         │
│  ├── Selector parsing (presence-only)       │
│  ├── Budget estimation (pre-query)          │
│  ├── Response assembly (only requested keys)│
│  └── Per-selector dispatch                  │
├─────────────────────────────────────────────┤
│  Data Sources                               │
│  ├── I2pControlState (startup values)       │
│  ├── MetricsSnapshot (cumulative counters)  │
│  ├── RollingWindow (1s/15s/1m/1h/1d)       │
│  ├── LogRing (bounded, redacted, clearable) │
│  ├── RouterInfoControl trait (fakes/adapters)│
│  └── AddressBookControl trait (M003)        │
├─────────────────────────────────────────────┤
│  Core (emissary-core)                       │
│  ├── EventHandle (atomic counters)          │
│  ├── Router (router identity, RI bytes)     │
│  └── Subsystem managers (tunnels, peers...) │
└─────────────────────────────────────────────┘
```

## Key components

### I2pControlState

Shared application state holding:
- Token service for authentication
- Router info control adapter (`Arc<dyn RouterInfoControl>`)
- Address book control adapter (`Arc<dyn AddressBookControl>`)
- Tunnel manager control adapter (`Arc<dyn TunnelManagerControl>`)
- Control plane adapter (`Arc<dyn ControlPlane>`)
- Startup-retained values (router ID, RI bytes, RI Base64)
- MetricsSnapshot for cumulative counters
- RollingWindow for recent traffic
- Concurrency semaphore

Production state is constructed via `I2pControlState::new_production()` with all required
dependencies supplied explicitly. Test state is constructed via `I2pControlState::new_test()`
which installs fake adapters. The production constructor cannot omit a dependency or default
to a fake.

All trait-object fields use `Arc` (not `Box`) to enable shared identity across consumers.
The tunnel manager, router info, and address book all reference the same underlying service
objects through their `Arc` clones.

### RouterInfoControl trait

Defines the read-only interface for router inspection:
- Identity/version/uptime
- Network status (IPv4/IPv6)
- UDP/TCP transport snapshots
- NetDB summary
- Bandwidth metrics
- Tunnel summaries
- Peer lists and statistics
- Log snapshot/clear
- Address book state

Production implementation requires core integration. Current implementation uses `FakeRouterInfoControl` for testing.

### MetricsSnapshot

Cloneable, thread-safe cumulative metrics source:
- Atomic counters for transport/transit bytes
- Atomic gauges for connected routers, participating tunnels
- Atomic counters for tunnel build successes/failures
- Non-destructive snapshot reads
- Process-lifetime monotonicity

### RollingWindow

Fixed-bucket rolling traffic accumulator:
- 1-second bucket granularity
- 86400 buckets (24-hour coverage)
- O(buckets) read, not O(events)
- Deterministic eviction of expired buckets
- Monotonic clock (not wall-clock)
- Read-only during queries

### LogRing

Bounded, redacted, independently clearable log buffer:
- Fixed maximum entries and total bytes
- Redaction of private keys, passwords, tokens
- Clear affects only this ring
- Concurrent readers receive coherent snapshot
- Wired as `tracing_subscriber::Layer`

## Data flow

### Startup values

```
Router::new() → (Router, EventSubscriber, serialized_RI)
    ↓
setup_router() → I2pControlState::set_startup_values(router_id, RI_bytes, RI_b64)
    ↓
I2pControlState retains values for handler reads
```

### Cumulative metrics

```
Transport sessions → EventHandle::transport_inbound_bandwidth(bytes)
                   → EventHandle::transport_outbound_bandwidth(bytes)
    ↓
MetricsSnapshot (separate atomics, fed by application layer)
    ↓
Handler reads MetricsSnapshot::snapshot()
```

### Rolling traffic

```
Transport sessions → EventHandle atomics
    ↓
Application layer feeds RollingWindow::record(inbound, outbound)
    ↓
Handler reads RollingWindow::read() for all intervals
```

### Log events

```
tracing::event!() → LogRingLayer::on_event()
    ↓
LogRing::push(entry) with redaction
    ↓
Handler reads LogRing::snapshot() for log queries
Handler calls LogRing::clear() for log clear
```

## Security properties

- Authentication before any inspection dispatch
- No private keys, session keys, or tokens in responses
- Log redaction before ring insertion
- Bounded response sizes prevent DoS
- No mutation of router state
- No consumption of frontend events
- No direct filesystem reads from handlers
- Error messages sanitized (no internal paths)

## Testing approach

- All 649 tests use `FakeRouterInfoControl`
- Unit tests per selector group
- `router_info_selectors_complete` test verifies selector count
- `unrelated_keys_absent` test verifies only-requested-key behavior
- LogRing tests: push/eviction/clear/redaction/concurrency
- MetricsSnapshot tests: cumulative/non-destructive/clone
- RollingWindow tests: empty/record/default intervals
- Budget enforcement tests: pre-query estimation

## Remaining work

- Production `RouterInfoControl` adapter (requires core `RouterInspectionHandle`)
- `RouterInspectionHandle` in `emissary-core` (typed bounded queries)
- Integration tests with real router state
- Contention/performance benchmarks
- Static compile-time guards
