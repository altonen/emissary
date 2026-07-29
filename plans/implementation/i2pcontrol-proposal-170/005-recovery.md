# M005 Recovery Plan — RouterInfo Inspection Gaps

Status: active

## Scope

Address all gaps identified in the M005 gap analysis. This recovery plan covers:

1. **Missing selector constants** (tunnels, peers, clock, I2PTunnel, share ratio, router news, logs)
2. **RouterInfoControl trait alignment** (add missing trait methods for missing selectors)
3. **Handler dispatch** (wire all selectors to handler with adapter functions)
4. **Observability integration** (LogRing as tracing layer, MetricsSnapshot/RollingWindow feed)
5. **Budget enforcement** (use MAX_* constants in handler)
6. **Startup value retention** (RouterId + serialized RI bytes in I2pControlState)
7. **Log redaction** (sanitize sensitive content before ring insert)
8. **Documentation** (router-info.md, inspection-architecture.md)

Out of scope (deferred):
- Core inspection API (`RouterInspectionHandle`) — requires emissary-core changes, separate milestone
- Production `RouterInfoControl` adapter — requires core integration
- Integration/contention/performance tests — requires full router
- Static compile-time guards — requires broader codebase analysis

## Work Packages

### WP1: Missing Selector Constants and Trait Methods

Add to `rpc.rs`:
- `ROUTER_NEWS`, `CLOCK_SKEW`, `SHARE_RATIO`, `CONFIGURED_BW_INBOUND`, `CONFIGURED_BW_OUTBOUND`
- `TUNNELS_PARTICIPATING`, `TUNNELS_EXPLORATORY_IN`, `TUNNELS_EXPLORATORY_OUT`
- `TUNNELS_CLIENT_IN`, `TUNNELS_CLIENT_OUT`, `TUNNELS_CONFIGURED`, `TUNNELS_QUEUE`
- `PEERS_KNOWN_COUNT`, `PEERS_KNOWN`, `PEERS_ACTIVE_COUNT`, `PEERS_ACTIVE`
- `PEERS_ROUTER_INFO`, `PEERS_BANNED`, `PEERS_BANNED_COUNT`, `PEERS_LIMITS`
- `PEERS_ACTIVE_STATS`, `NET_IPTUNNELS`
- `LOG_SNAPSHOT`, `LOG_CLEAR`

Add to `RouterInfoControl` trait:
- `router_news() -> String`
- `share_ratio() -> f64` (already exists)
- `configured_bw_limits() -> (u64, u64)` (already exists)
- `clock_skew() -> ClockSkew` (already exists)
- `i2ptunnel_stats() -> I2PTunnelStats` (already exists)
- `log_snapshot() -> LogSnapshot` (already exists)
- `log_clear()` (already exists)

### WP2: Handler Dispatch for All Selectors

Wire every selector to its adapter in `router_info_handler.rs`:
- Identity/version/uptime
- Network status (IPv4/IPv6)
- UDP/TCP transport
- NetDB
- Bandwidth (all rolling windows)
- Router news (empty string)
- Clock skew
- Tunnels (participating, exploratory, client, queue)
- Peers (known, active, RouterInfo, banned, limits, stats)
- I2PTunnel quick stats
- Share ratio / configured BW
- Address book
- Log snapshot / clear
- Budget enforcement (check MAX_* before expensive queries)

### WP3: Log Redaction

Add redaction logic to `LogRing::push()`:
- Redact patterns: `[A-Za-z0-9+/]{40,}=*` (Base64 private keys), `password=\S+`, `token=\S+`
- Redact with `[REDACTED]`

### WP4: Observability Integration

Wire `LogRing` into tracing subscriber in `logger.rs`:
- Add a `Layer` impl for `LogRing`
- Push formatted events into the ring
- Clear only the ring on log clear

Wire `MetricsSnapshot` and `RollingWindow` into `EventHandle` or transport:
- Record transport bytes on EventHandle
- Record transit bytes on EventHandle

### WP5: Startup Value Retention

Add to `I2pControlState`:
- `router_id: String` — local router identity Base64
- `router_info_bytes: Vec<u8>` — serialized RouterInfo
- `router_info_b64: String` — Base64 of serialized RI
- `startup_time: Instant`
- `share_ratio: f64`
- `configured_bw: (u64, u64)`

### WP6: Documentation

Create `docs/i2pcontrol/router-info.md`:
- Selector catalog
- Null/unavailable behavior
- Bounds and limits
- Read-only architecture

## Verification

```bash
cargo test -p emissary-cli --no-default-features --features i2pcontrol
cargo clippy -p emissary-cli --features i2pcontrol -- -D warnings
cargo fmt
```
