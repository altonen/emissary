# RouterInfo Selector Source Map

Status: M009 implemented (RouterInfo availability and truthfulness)

This document is the single source of truth for every Proposal 170 RouterInfo
selector's wire key, output type, nullability, semantic definition, canonical
Emissary source, snapshot group, bounds, availability, and M010 work-package.

## Source classes

| Class | Description |
|---|---|
| `retained` | Identity, serialized local RouterInfo, version, configured values, startup time |
| `event-metric` | Existing atomic counters or cached statuses from `EventMetrics` |
| `administrative-store` | Shared `AddressBook` or `TunnelManager` state |
| `core-inspection` | Bounded runtime/NetDB/transport/tunnel snapshot required from M010 |
| `protocol-defined-empty` | Protocol semantics explicitly define absence as empty |
| `nullable-unavailable` | Protocol permits null where value is unknown |
| `unsupported-inspection` | Selector validated but no truthful source exists; fails explicitly until M010 |

## Selector source map

### Retained group

| Wire key | JSON type | Nullability | Source | Bound | Current availability | Unavailable behavior | M010 owner |
|---|---|---|---|---|---|---|---|
| `i2p.router.identity` | string | non-null | `retained` | — | implemented | error | — |
| `i2p.router.version` | string | non-null | `retained` | — | implemented | error | — |
| `i2p.router.uptime` | integer | non-null | `retained` | — | implemented | error | — |
| `i2p.router.news` | string | non-null | `retained` | — | implemented (empty string) | error | — |
| `i2p.router.shareRatio` | number | non-null | `retained` | — | implemented | error | — |
| `i2p.router.configuredBwInbound` | integer | non-null | `retained` | — | implemented | error | — |
| `i2p.router.configuredBwOutbound` | integer | non-null | `retained` | — | implemented | error | — |

### Network group

| Wire key | JSON type | Nullability | Source | Bound | Current availability | Unavailable behavior | M010 owner |
|---|---|---|---|---|---|---|---|
| `i2p.router.net.bw.inbound` | string | non-null | `event-metric` | — | implemented | error | — |
| `i2p.router.net.bw.outbound` | string | non-null | `event-metric` | — | implemented | error | — |
| `i2p.router.clock.skew` | integer/null | nullable | `unsupported-inspection` | — | unavailable | null (protocol-permitted) | M010 |

### UDP transport group

| Wire key | JSON type | Nullability | Source | Bound | Current availability | Unavailable behavior | M010 owner |
|---|---|---|---|---|---|---|---|
| `i2p.router.udp.active` | boolean | non-null | `event-metric` | — | implemented (connected-routers-derived) | error | — |
| `i2p.router.udp.cookie.active` | boolean | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.udp.integratedPeers` | integer | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.udp.firewalled` | boolean | non-null | `event-metric` | — | implemented (firewall-derived) | error | — |
| `i2p.router.udp.hidden` | boolean | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.udp.coinficientPeers` | integer | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.udp.criticalPeers` | integer | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.udp.fastPeers` | integer | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.udp.highCapacityPeers` | integer | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.udp.interleavedPeers` | integer | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.udp.litPeers` | integer | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.udp.lowCapacityPeers` | integer | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.udp.onDemandPeers` | integer | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.udp.peerStats` | object | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.udp.standardPeers` | integer | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.udp.unreachablePeers` | integer | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.udp.totalPeers` | integer | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.udp.currentPeers` | integer | non-null | `unsupported-inspection` | — | unavailable | error | M010 |

### TCP transport group

| Wire key | JSON type | Nullability | Source | Bound | Current availability | Unavailable behavior | M010 owner |
|---|---|---|---|---|---|---|---|
| `i2p.router.tcp.active` | boolean | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.tcp.integratedPeers` | integer | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.tcp.firewalled` | boolean | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.tcp.hosts` | string | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.tcp.status` | string | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.tcp.version` | string | non-null | `unsupported-inspection` | — | unavailable | error | M010 |

### NetDB group

| Wire key | JSON type | Nullability | Source | Bound | Current availability | Unavailable behavior | M010 owner |
|---|---|---|---|---|---|---|---|
| `i2p.router.netdb.active` | boolean | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.netdb.activeProfiles` | integer | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.netdb.highestVersion` | integer | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.netdb.knownProfiles` | integer | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.netdb.newProfiles` | integer | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.netdb.activeRouters` | integer | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.netdb.alreadyExperiencedPeers` | integer | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.netdb.banlistSize` | integer | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.netdb.exploratoryPeers` | integer | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.netdb.fastPeers` | integer | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.netdb.highCapacityPeers` | integer | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.netdb.isBacklogged` | boolean | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.netdb.knownActive` | integer | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.netdb.knownIdle` | integer | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.netdb.knownUsed` | integer | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.netdb.knownVanilla` | integer | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.netdb.knownVolatile` | integer | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.netdb.lastExplored` | integer | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.netdb.lastProfileLookup` | integer | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.netdb.lastRouterLookup` | integer | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.netdb.lastUnsaved` | integer | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.netdb.leaseSets` | integer | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.netdb.newActive` | integer | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.netdb.newIdle` | integer | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.netdb.oldActive` | integer | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.netdb.oldIdle` | integer | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.netdb.peerProfiles` | integer | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.netdb.plaintextPeers` | integer | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.netdb.reserveActive` | integer | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.netdb.reserveActivePeers` | integer | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.netdb.reserveHighCapacity` | integer | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.netdb.reserveIntegrated` | integer | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.netdb.reserveKnown` | integer | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.netdb.reserveLookup` | integer | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.netdb.reservePending` | integer | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.netdb.reserveReserved` | integer | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.netdb.reserveStandard` | integer | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.netdb.reserveTier2` | integer | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.netdb.reserveUsed` | integer | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.netdb.reserveVolatile` | integer | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.netdb.standardPeers` | integer | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.netdb.lowCapacityPeers` | integer | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.netdb.tunnels` | integer | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.netdb.usedPeers` | integer | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.netdb.volatilePeers` | integer | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.netdb.addressBooks` | integer | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.netdb.addressBookEntries` | integer | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.netdb.addressBookSources` | integer | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.netdb.addressBookSubscriptions` | integer | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.netdb.addressBookUpdates` | integer | non-null | `unsupported-inspection` | — | unavailable | error | M010 |

### Traffic metrics group

| Wire key | JSON type | Nullability | Source | Bound | Current availability | Unavailable behavior | M010 owner |
|---|---|---|---|---|---|---|---|
| `i2p.router.bw.inbound.total` | integer | non-null | `event-metric` | — | implemented | — | — |
| `i2p.router.bw.outbound.total` | integer | non-null | `event-metric` | — | implemented | — | — |
| `i2p.router.bw.inbound.1s` | integer | non-null | `event-metric` | — | implemented | — | — |
| `i2p.router.bw.outbound.1s` | integer | non-null | `event-metric` | — | implemented | — | — |
| `i2p.router.bw.inbound.15s` | integer | non-null | `event-metric` | — | implemented | — | — |
| `i2p.router.bw.outbound.15s` | integer | non-null | `event-metric` | — | implemented | — | — |
| `i2p.router.bw.inbound.1m` | integer | non-null | `event-metric` | — | implemented | — | — |
| `i2p.router.bw.outbound.1m` | integer | non-null | `event-metric` | — | implemented | — | — |
| `i2p.router.bw.inbound.1h` | integer | non-null | `event-metric` | — | implemented | — | — |
| `i2p.router.bw.outbound.1h` | integer | non-null | `event-metric` | — | implemented | — | — |
| `i2p.router.bw.inbound.1d` | integer | non-null | `event-metric` | — | implemented | — | — |
| `i2p.router.bw.outbound.1d` | integer | non-null | `event-metric` | — | implemented | — | — |

### Tunnel summary group

| Wire key | JSON type | Nullability | Source | Bound | Current availability | Unavailable behavior | M010 owner |
|---|---|---|---|---|---|---|---|
| `i2p.router.tunnels.participating` | integer | non-null | `event-metric` | — | implemented | — | — |
| `i2p.router.tunnels.exploratoryIn` | integer | non-null | `unsupported-inspection` | — | unavailable (returns 0) | error | M010 |
| `i2p.router.tunnels.exploratoryOut` | integer | non-null | `unsupported-inspection` | — | unavailable (returns 0) | error | M010 |
| `i2p.router.tunnels.clientIn` | integer | non-null | `unsupported-inspection` | — | unavailable (returns 0) | error | M010 |
| `i2p.router.tunnels.clientOut` | integer | non-null | `unsupported-inspection` | — | unavailable (returns 0) | error | M010 |
| `i2p.router.tunnels.configured` | integer | non-null | `administrative-store` | — | implemented | error | — |
| `i2p.router.tunnels.queue` | integer | non-null | `unsupported-inspection` | — | unavailable (returns 0) | error | M010 |

### I2PTunnel group

| Wire key | JSON type | Nullability | Source | Bound | Current availability | Unavailable behavior | M010 owner |
|---|---|---|---|---|---|---|---|
| `i2p.router.net.i2ptunnels` | integer | non-null | `administrative-store` | — | implemented | error | — |

### Peer list group

| Wire key | JSON type | Nullability | Source | Bound | Current availability | Unavailable behavior | M010 owner |
|---|---|---|---|---|---|---|---|
| `i2p.router.peers.knownCount` | integer | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.peers.known` | array | non-null | `unsupported-inspection` | 10,000 | unavailable | error | M010 |
| `i2p.router.peers.activeCount` | integer | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.peers.active` | array | non-null | `unsupported-inspection` | 10,000 | unavailable | error | M010 |
| `i2p.router.peers.banned` | array | non-null | `unsupported-inspection` | 10,000 | unavailable | error | M010 |
| `i2p.router.peers.bannedCount` | integer | non-null | `unsupported-inspection` | — | unavailable | error | M010 |

### Peer lookup group

| Wire key | JSON type | Nullability | Source | Bound | Current availability | Unavailable behavior | M010 owner |
|---|---|---|---|---|---|---|---|
| `i2p.router.peers.routerInfo` | string/null | nullable | `unsupported-inspection` | 4 MB | unavailable | error | M010 |

### Peer stats group

| Wire key | JSON type | Nullability | Source | Bound | Current availability | Unavailable behavior | M010 owner |
|---|---|---|---|---|---|---|---|
| `i2p.router.peers.limits` | object | non-null | `unsupported-inspection` | — | unavailable | error | M010 |
| `i2p.router.peers.activeStats` | array | non-null | `unsupported-inspection` | 10,000 | unavailable | error | M010 |

### Log group

| Wire key | JSON type | Nullability | Source | Bound | Current availability | Unavailable behavior | M010 owner |
|---|---|---|---|---|---|---|---|
| `i2p.router.log` | array | non-null | `event-metric` | 10,000 | implemented | — | — |
| `i2p.router.log.clear` | boolean | non-null | `event-metric` | — | implemented | — | — |

### Address book group

| Wire key | JSON type | Nullability | Source | Bound | Current availability | Unavailable behavior | M010 owner |
|---|---|---|---|---|---|---|---|
| `i2p.router.addressbook.private` | array | non-null | `administrative-store` | — | implemented | error | — |
| `i2p.router.addressbook.local` | array | non-null | `administrative-store` | — | implemented | error | — |
| `i2p.router.addressbook.router` | array | non-null | `administrative-store` | — | implemented | error | — |
| `i2p.router.addressbook.published` | array | non-null | `administrative-store` | — | implemented | error | — |
| `i2p.router.addressbook.subscriptions` | array | non-null | `administrative-store` | — | implemented | error | — |
| `i2p.router.addressbook.config` | object | non-null | `administrative-store` | — | implemented | error | — |
