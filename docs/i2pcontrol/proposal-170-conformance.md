# Proposal 170 Conformance Matrix

Status: normative inventory for the pinned-revision closure

Proposal 170 remains Open and this inventory is pinned to the revision created
and last updated on 2026-05-20. M017's broad closure is invalidated historical
evidence; M019 independently accepted the final head against that revision.

Coverage is reported in three dimensions: **wire implemented** (exact request
and response contract), **source available** (truthful current data source),
and **runtime implemented** (real operation backend). An unavailable source or
unsupported runtime is never counted as operational support.

The exact 43 RouterInfo additions are:

```text
i2p.router.news
i2p.router.id
i2p.router.clockskew
i2p.router.info
i2p.router.logs
i2p.router.logs.clear
i2p.router.net.total.received.bytes
i2p.router.net.total.sent.bytes
i2p.router.net.total.transit.bytes
i2p.router.net.bw.transit.15s
i2p.router.net.tunnels.shareratio
i2p.router.net.tunnels.participating.info
i2p.router.net.tunnels.i2ptunnel
i2p.router.net.tunnels.exploratory.inbound
i2p.router.net.tunnels.exploratory.outbound
i2p.router.net.tunnels.exploratory.info.list
i2p.router.net.tunnels.client.inbound
i2p.router.net.tunnels.client.outbound
i2p.router.net.tunnels.client.info.list
i2p.router.net.status.v6
i2p.router.net.error
i2p.router.net.error.v6
i2p.router.net.testing
i2p.router.net.testing.v6
i2p.router.net.tunnels.successrate
i2p.router.net.tunnels.totalsuccessrate
i2p.router.net.tunnels.queue
i2p.router.net.tunnels.tbmqueue
i2p.router.netdb.peers
i2p.router.netdb.activepeers.info
i2p.router.netdb.ntcp.limit
i2p.router.netdb.ssu.limit
i2p.router.netdb.bannedpeers
i2p.router.netdb.activepeers.list
i2p.router.netdb.peers.list
i2p.router.netdb.peers.info
i2p.router.netdb.activepeers.stats
i2p.router.addressbook.private.list
i2p.router.addressbook.local.list
i2p.router.addressbook.router.list
i2p.router.addressbook.published.list
i2p.router.addressbook.subscriptions
i2p.router.addressbook.config
```

The 121 legacy/base selectors remain documented separately and are not part of
this addition count. The exact machine-readable types/source states live in
`rpc::router_info_keys::PROPOSAL_170_CONTRACT`.

This document records every Proposal 170 method, selector, parameter, action, tunnel type,
JSON type, nullability rule, validation rule, data source, expected milestone owner, and
fixture/test ID. It is the single source of truth for contract completeness.

## 1. Base I2PControl Methods

### Authenticate

| Contract item | Request key/type | Required/optional/presence | Response key/type | Nullability | Validation/error behavior | Planned data source | Owner milestone | Fixture/test ID | Notes |
|---|---|---|---|---|---|---|---|---|---|
| Authenticate method name | `method` = `"Authenticate"` | required | — | — | — | — | M001 | `fixture_authenticate` | Exact I2PControl method name |
| `api` parameter | `params.API` | required | — | — | Must be `1` or `2`; `1` accepted for backward compat | — | M001 | `fixture_authenticate` | API version negotiation |
| `username` parameter | `params.Username` | required | — | — | Must be `"i2pcontrol"` (exact) | — | M001 | `fixture_authenticate` | Only accepted username per base API |
| `password` parameter | `params.Password` | required | — | — | Non-empty string; compared timing-resistantly | — | M001 | `fixture_authenticate` | Password from configuration |
| `result` success | `result.Token` | present on success | string (opaque hex) | non-null on success | Cryptographically random; bounded store | Token service | M001 | `fixture_authenticate` | Opaque token for subsequent calls |
| `result` success | `result.API` | present on success | string | non-null on success | Echoed API version | — | M001 | `fixture_authenticate` | Returned for client verification |
| Wrong password error | `error` | on auth failure | object | — | JSON-RPC error code `-1` (or `-32600` depending on base impl) | — | M001 | `fixture_auth_error_password` | Do not reveal password vs version mismatch |
| Missing fields error | `error` | on missing params | object | — | JSON-RPC standard error | — | M001 | `fixture_auth_error_missing` | Reject incomplete Authenticate |

### GetKeys

| Contract item | Request key/type | Required/optional/presence | Response key/type | Nullability | Validation/error behavior | Planned data source | Owner milestone | Fixture/test ID | Notes |
|---|---|---|---|---|---|---|---|---|---|
| GetKeys method name | `method` = `"GetKeys"` | required | — | — | — | — | M002+ | `fixture_get_keys` | Returns all available selector keys |

## 2. Proposal 170 Methods

### RouterInfo

| Contract item | Request key/type | Required/optional/presence | Response key/type | Nullability | Validation/error behavior | Planned data source | Owner milestone | Fixture/test ID | Notes |
|---|---|---|---|---|---|---|---|---|---|
| RouterInfo method | `method` = `"RouterInfo"` | required | — | — | — | — | M005 | `fixture_router_info` | Base method for router inspection |
| `i2p.router.udp.active` | param presence | selector | boolean | non-null when returned | — | Router transport state | M005 | `fixture_ri_udp_active` | Whether UDP transport is active |
| `i2p.router.udp.cookie.active` | param presence | selector | boolean | non-null when returned | — | Router transport state | M005 | `fixture_ri_udp_cookie` | — |
| `i2p.router.udp.integrated Peers` | param presence | selector | integer | non-null when returned | — | Router transport state | M005 | `fixture_ri_udp_integrated` | — |
| `i2p.router.udp.firewalled` | param presence | selector | boolean | non-null when returned | — | Router transport state | M005 | `fixture_ri_udp_firewalled` | — |
| `i2p.router.udp.hidden` | param presence | selector | boolean | non-null when returned | — | Router transport state | M005 | `fixture_ri_udp_hidden` | — |
| `i2p.router.udp.coinficientPeers` | param presence | selector | integer | non-null when returned | — | Router transport state | M005 | `fixture_ri_udp_coinficient` | Note: proposal spelling preserved |
| `i2p.router.udp.criticalPeers` | param presence | selector | integer | non-null when returned | — | Router transport state | M005 | `fixture_ri_udp_critical` | — |
| `i2p.router.udp.fastPeers` | param presence | selector | integer | non-null when returned | — | Router transport state | M005 | `fixture_ri_udp_fast` | — |
| `i2p.router.udp.highCapacityPeers` | param presence | selector | integer | non-null when returned | — | Router transport state | M005 | `fixture_ri_udp_highcapacity` | — |
| `i2p.router.udp.interleavedPeers` | param presence | selector | integer | non-null when returned | — | Router transport state | M005 | `fixture_ri_udp_interleaved` | — |
| `i2p.router.udp.litPeers` | param presence | selector | integer | non-null when returned | — | Router transport state | M005 | `fixture_ri_udp_lit` | — |
| `i2p.router.udp.lowCapacityPeers` | param presence | selector | integer | non-null when returned | — | Router transport state | M005 | `fixture_ri_udp_lowcapacity` | — |
| `i2p.router.udp.onDemandPeers` | param presence | selector | integer | non-null when returned | — | Router transport state | M005 | `fixture_ri_udp_ondemand` | — |
| `i2p.router.udp.peerStats` | param presence | selector | object | non-null when returned | — | Router transport state | M005 | `fixture_ri_udp_peerstats` | — |
| `i2p.router.udp.standardPeers` | param presence | selector | integer | non-null when returned | — | Router transport state | M005 | `fixture_ri_udp_standard` | — |
| `i2p.router.udp.unreachablePeers` | param presence | selector | integer | non-null when returned | — | Router transport state | M005 | `fixture_ri_udp_unreachable` | — |
| `i2p.router.udp.totalPeers` | param presence | selector | integer | non-null when returned | — | Router transport state | M005 | `fixture_ri_udp_total` | — |
| `i2p.router.udp.currentPeers` | param presence | selector | integer | non-null when returned | — | Router transport state | M005 | `fixture_ri_udp_current` | — |
| `i2p.router.version` | param presence | selector | string | non-null when returned | — | Router version info | M005 | `fixture_ri_version` | — |
| `i2p.router.uptime` | param presence | selector | integer | non-null when returned | — | Router uptime | M005 | `fixture_ri_uptime` | — |
| `i2p.router.netdb.active` | param presence | selector | boolean | non-null when returned | — | NetDB state | M005 | `fixture_ri_netdb_active` | — |
| `i2p.router.netdb.activeProfiles` | param presence | selector | integer | non-null when returned | — | NetDB state | M005 | `fixture_ri_netdb_activeprofiles` | — |
| `i2p.router.netdb.highestVersion` | param presence | selector | integer | non-null when returned | — | NetDB state | M005 | `fixture_ri_netdb_highestversion` | — |
| `i2p.router.netdb.knownProfiles` | param presence | selector | integer | non-null when returned | — | NetDB state | M005 | `fixture_ri_netdb_knownprofiles` | — |
| `i2p.router.netdb.newProfiles` | param presence | selector | integer | non-null when returned | — | NetDB state | M005 | `fixture_ri_netdb_newprofiles` | — |
| `i2p.router.netdb.activeRouters` | param presence | selector | integer | non-null when returned | — | NetDB state | M005 | `fixture_ri_netdb_activerouters` | — |
| `i2p.router.netdb.alreadyExperiencedPeers` | param presence | selector | integer | non-null when returned | — | NetDB state | M005 | `fixture_ri_netdb_alreadyexperienced` | — |
| `i2p.router.netdb.banlistSize` | param presence | selector | integer | non-null when returned | — | NetDB state | M005 | `fixture_ri_netdb_banlistsize` | — |
| `i2p.router.netdb.exploratoryPeers` | param presence | selector | integer | non-null when returned | — | NetDB state | M005 | `fixture_ri_netdb_exploratorypeers` | — |
| `i2p.router.netdb.fastPeers` | param presence | selector | integer | non-null when returned | — | NetDB state | M005 | `fixture_ri_netdb_fastpeers` | — |
| `i2p.router.netdb.highCapacityPeers` | param presence | selector | integer | non-null when returned | — | NetDB state | M005 | `fixture_ri_netdb_highcapacitypeers` | — |
| `i2p.router.netdb.isBacklogged` | param presence | selector | boolean | non-null when returned | — | NetDB state | M005 | `fixture_ri_netdb_isbacklogged` | — |
| `i2p.router.netdb.knownActive` | param presence | selector | integer | non-null when returned | — | NetDB state | M005 | `fixture_ri_netdb_knownactive` | — |
| `i2p.router.netdb.knownIdle` | param presence | selector | integer | non-null when returned | — | NetDB state | M005 | `fixture_ri_netdb_knownidle` | — |
| `i2p.router.netdb.knownUsed` | param presence | selector | integer | non-null when returned | — | NetDB state | M005 | `fixture_ri_netdb_knownused` | — |
| `i2p.router.netdb.knownVanilla` | param presence | selector | integer | non-null when returned | — | NetDB state | M005 | `fixture_ri_netdb_knownvanilla` | — |
| `i2p.router.netdb.knownVolatile` | param presence | selector | integer | non-null when returned | — | NetDB state | M005 | `fixture_ri_netdb_knownvolatile` | — |
| `i2p.router.netdb.lastExplored` | param presence | selector | integer | non-null when returned | — | NetDB state | M005 | `fixture_ri_netdb_lastexplored` | — |
| `i2p.router.netdb.lastProfileLookup` | param presence | selector | integer | non-null when returned | — | NetDB state | M005 | `fixture_ri_netdb_lastprofilelookup` | — |
| `i2p.router.netdb.lastRouterLookup` | param presence | selector | integer | non-null when returned | — | NetDB state | M005 | `fixture_ri_netdb_lastrouterlookup` | — |
| `i2p.router.netdb.lastUnsaved` | param presence | selector | integer | non-null when returned | — | NetDB state | M005 | `fixture_ri_netdb_lastunsaved` | — |
| `i2p.router.netdb.leaseSets` | param presence | selector | integer | non-null when returned | — | NetDB state | M005 | `fixture_ri_netdb_leasesets` | — |
| `i2p.router.netdb.newActive` | param presence | selector | integer | non-null when returned | — | NetDB state | M005 | `fixture_ri_netdb_newactive` | — |
| `i2p.router.netdb.newIdle` | param presence | selector | integer | non-null when returned | — | NetDB state | M005 | `fixture_ri_netdb_newidle` | — |
| `i2p.router.netdb.oldActive` | param presence | selector | integer | non-null when returned | — | NetDB state | M005 | `fixture_ri_netdb_oldactive` | — |
| `i2p.router.netdb.oldIdle` | param presence | selector | integer | non-null when returned | — | NetDB state | M005 | `fixture_ri_netdb_oldidle` | — |
| `i2p.router.netdb.peerProfiles` | param presence | selector | integer | non-null when returned | — | NetDB state | M005 | `fixture_ri_netdb_peerprofiles` | — |
| `i2p.router.netdb.plaintextPeers` | param presence | selector | integer | non-null when returned | — | NetDB state | M005 | `fixture_ri_netdb_plaintextpeers` | — |
| `i2p.router.netdb.reserveActive` | param presence | selector | integer | non-null when returned | — | NetDB state | M005 | `fixture_ri_netdb_reserveactive` | — |
| `i2p.router.netdb.reserveActivePeers` | param presence | selector | integer | non-null when returned | — | NetDB state | M005 | `fixture_ri_netdb_reserveactivepeers` | — |
| `i2p.router.netdb.reserveHighCapacity` | param presence | selector | integer | non-null when returned | — | NetDB state | M005 | `fixture_ri_netdb_reservehighcapacity` | — |
| `i2p.router.netdb.reserveIntegrated` | param presence | selector | integer | non-null when returned | — | NetDB state | M005 | `fixture_ri_netdb_reserveintegrated` | — |
| `i2p.router.netdb.reserveKnown` | param presence | selector | integer | non-null when returned | — | NetDB state | M005 | `fixture_ri_netdb_reserveknown` | — |
| `i2p.router.netdb.reserveLookup` | param presence | selector | integer | non-null when returned | — | NetDB state | M005 | `fixture_ri_netdb_reservelookup` | — |
| `i2p.router.netdb.reservePending` | param presence | selector | integer | non-null when returned | — | NetDB state | M005 | `fixture_ri_netdb_reservepending` | — |
| `i2p.router.netdb.reserveReserved` | param presence | selector | integer | non-null when returned | — | NetDB state | M005 | `fixture_ri_netdb_reservereserved` | — |
| `i2p.router.netdb.reserveStandard` | param presence | selector | integer | non-null when returned | — | NetDB state | M005 | `fixture_ri_netdb_reservestandard` | — |
| `i2p.router.netdb.reserveTier2` | param presence | selector | integer | non-null when returned | — | NetDB state | M005 | `fixture_ri_netdb_reservetier2` | — |
| `i2p.router.netdb.reserveUsed` | param presence | selector | integer | non-null when returned | — | NetDB state | M005 | `fixture_ri_netdb_reserveused` | — |
| `i2p.router.netdb.reserveVolatile` | param presence | selector | integer | non-null when returned | — | NetDB state | M005 | `fixture_ri_netdb_reservevolatile` | — |
| `i2p.router.netdb.standardPeers` | param presence | selector | integer | non-null when returned | — | NetDB state | M005 | `fixture_ri_netdb_standardpeers` | — |
| `i2p.router.netdb.tunnels` | param presence | selector | integer | non-null when returned | — | NetDB state | M005 | `fixture_ri_netdb_tunnels` | — |
| `i2p.router.netdb.usedPeers` | param presence | selector | integer | non-null when returned | — | NetDB state | M005 | `fixture_ri_netdb_usedpeers` | — |
| `i2p.router.netdb.volatilePeers` | param presence | selector | integer | non-null when returned | — | NetDB state | M005 | `fixture_ri_netdb_volatilepeers` | — |
| `i2p.router.netdb.addressBooks` | param presence | selector | integer | non-null when returned | — | NetDB state | M005 | `fixture_ri_netdb_addressbooks` | — |
| `i2p.router.netdb.addressBook Entries` | param presence | selector | integer | non-null when returned | — | NetDB state | M005 | `fixture_ri_netdb_addressbookentries` | Note: proposal spelling preserved |
| `i2p.router.netdb.addressBookSources` | param presence | selector | integer | non-null when returned | — | NetDB state | M005 | `fixture_ri_netdb_addressbooksources` | — |
| `i2p.router.netdb.addressBookSubscriptions` | param presence | selector | integer | non-null when returned | — | NetDB state | M005 | `fixture_ri_netdb_addressbooksubscriptions` | — |
| `i2p.router.netdb.addressBookUpdates` | param presence | selector | integer | non-null when returned | — | NetDB state | M005 | `fixture_ri_netdb_addressbookupdates` | — |
| `i2p.router.bw.inbound.1s` | param presence | selector | integer | non-null when returned | — | Bandwidth metrics | M005 | `fixture_ri_bw_inbound_1s` | — |
| `i2p.router.bw.inbound.15s` | param presence | selector | integer | non-null when returned | — | Bandwidth metrics | M005 | `fixture_ri_bw_inbound_15s` | — |
| `i2p.router.bw.inbound.1m` | param presence | selector | integer | non-null when returned | — | Bandwidth metrics | M005 | `fixture_ri_bw_inbound_1m` | — |
| `i2p.router.bw.inbound.1h` | param presence | selector | integer | non-null when returned | — | Bandwidth metrics | M005 | `fixture_ri_bw_inbound_1h` | — |
| `i2p.router.bw.inbound.1d` | param presence | selector | integer | non-null when returned | — | Bandwidth metrics | M005 | `fixture_ri_bw_inbound_1d` | — |
| `i2p.router.bw.inbound.total` | param presence | selector | integer | non-null when returned | — | Bandwidth metrics | M005 | `fixture_ri_bw_inbound_total` | — |
| `i2p.router.bw.outbound.1s` | param presence | selector | integer | non-null when returned | — | Bandwidth metrics | M005 | `fixture_ri_bw_outbound_1s` | — |
| `i2p.router.bw.outbound.15s` | param presence | selector | integer | non-null when returned | — | Bandwidth metrics | M005 | `fixture_ri_bw_outbound_15s` | — |
| `i2p.router.bw.outbound.1m` | param presence | selector | integer | non-null when returned | — | Bandwidth metrics | M005 | `fixture_ri_bw_outbound_1m` | — |
| `i2p.router.bw.outbound.1h` | param presence | selector | integer | non-null when returned | — | Bandwidth metrics | M005 | `fixture_ri_bw_outbound_1h` | — |
| `i2p.router.bw.outbound.1d` | param presence | selector | integer | non-null when returned | — | Bandwidth metrics | M005 | `fixture_ri_bw_outbound_1d` | — |
| `i2p.router.bw.outbound.total` | param presence | selector | integer | non-null when returned | — | Bandwidth metrics | M005 | `fixture_ri_bw_outbound_total` | — |
| `i2p.router.tcp.active` | param presence | selector | boolean | non-null when returned | — | Router transport state | M005 | `fixture_ri_tcp_active` | — |
| `i2p.router.tcp.integratedPeers` | param presence | selector | integer | non-null when returned | — | Router transport state | M005 | `fixture_ri_tcp_integrated` | — |
| `i2p.router.tcp.firewalled` | param presence | selector | boolean | non-null when returned | — | Router transport state | M005 | `fixture_ri_tcp_firewalled` | — |
| `i2p.router.tcp.hosts` | param presence | selector | string | non-null when returned | — | Router transport state | M005 | `fixture_ri_tcp_hosts` | — |
| `i2p.router.tcp.status` | param presence | selector | string | non-null when returned | — | Router transport state | M005 | `fixture_ri_tcp_status` | — |
| `i2p.router.tcp.version` | param presence | selector | string | non-null when returned | — | Router transport state | M005 | `fixture_ri_tcp_version` | — |
| `i2p.router.identity` | param presence | selector | string | non-null when returned | — | Router identity | M005 | `fixture_ri_identity` | Base64 RouterInfo |
| `i2p.router.net.bw.inbound` | param presence | selector | integer | non-null when returned | — | Bandwidth metrics | M005 | `fixture_ri_net_bw_inbound` | — |
| `i2p.router.net.bw.outbound` | param presence | selector | integer | non-null when returned | — | Bandwidth metrics | M005 | `fixture_ri_net_bw_outbound` | — |

### AddressBook

| Contract item | Request key/type | Required/optional/presence | Response key/type | Nullability | Validation/error behavior | Planned data source | Owner milestone | Fixture/test ID | Notes |
|---|---|---|---|---|---|---|---|---|---|
| AddressBook method | `method` = `"AddressBook"` | required | — | — | — | — | M003 | `fixture_address_book` | Implemented |
| `Type` / `Hostname` / `Destination` | direct params | entry mode | — | — | Exact book type and bounded hostname/destination validation | Administrative store | M018 | `canonical_wire_fixture_mutates_entry_and_uses_result_object` | Wire implemented; source available |
| `Delete` | direct param presence | optional | — | — | Presence selects delete regardless of value | Administrative store | M018 | `canonical_wire_fixture_mutates_entry_and_uses_result_object` | Wire implemented; source available |
| `signature` parameter | `params.signature` | optional | — | — | Valid signature if present | — | M003 | `fixture_ab_signature` | Accepted but not validated (no signature verification in M003) |
| List result | `result` | on List | array of objects | non-null | — | Administrative store | M003 | `fixture_ab_list` | Each entry has `name` and `value`; implemented |
| Lookup result | `result` | on Lookup | object or null | null if not found | — | Administrative store | M003 | `fixture_ab_lookup` | Implemented |
| Delete presence semantics | `name` param presence | presence-based | string | — | Presence of `name` param = delete specific entry; absence = delete all in book | — | M003 | `fixture_ab_delete` | Implemented |
| SetConfig | `params.SetConfig` inside `AddressBook` | canonical mode | `result.success`, `result.message` | non-null on success | Exactly one canonical mode; bounded string map | Administrative store | M018 | `canonical_wire_fixture_supports_subscription_and_config_modes` | Wire/source implemented; standalone method is compatibility-only |
| SetSubscriptions | `params.SetSubscriptions` inside `AddressBook` | canonical mode | `result.success`, `result.message` | non-null on success | Exactly one canonical mode; bounded string list | Administrative store | M018 | `canonical_wire_fixture_supports_subscription_and_config_modes` | Wire/source implemented; standalone method is compatibility-only |
| `i2p.router.addressbook.private` | param presence | selector | array | non-null when returned | — | Administrative store | M003 | `fixture_ri_ab_private` | Implemented |
| `i2p.router.addressbook.local` | param presence | selector | array | non-null when returned | — | Administrative store | M003 | `fixture_ri_ab_local` | Implemented |
| `i2p.router.addressbook.router` | param presence | selector | array | non-null when returned | — | Administrative store | M003 | `fixture_ri_ab_router` | Implemented |
| `i2p.router.addressbook.published` | param presence | selector | array | non-null when returned | — | Administrative store | M003 | `fixture_ri_ab_published` | Implemented |
| `i2p.router.addressbook.subscriptions` | param presence | selector | array | non-null when returned | — | Administrative store | M003 | `fixture_ri_ab_subscriptions` | Implemented |
| `i2p.router.addressbook.config` | param presence | selector | object | non-null when returned | — | Administrative store | M003 | `fixture_ri_ab_config` | Implemented |

### TunnelManager

| Contract item | Request key/type | Required/optional/presence | Response key/type | Nullability | Validation/error behavior | Planned data source | Owner milestone | Fixture/test ID | Notes |
|---|---|---|---|---|---|---|---|---|---|
| TunnelManager method | `method` = `"TunnelManager"` | required | — | — | — | — | M004 | `fixture_tunnel_manager` | — |
| `Action` parameter | `params.Action` | required | — | — | Canonical values: `create`, `edit`, `get`, `start`, `stop`, `restart`, `delete` | — | M018 | `canonical_wire_fixture_covers_all_seven_actions` | Lowercase is canonical; capitalized values and `List` are compatibility-only |
| `type` parameter | `params.type` | required for Create/Edit | — | — | Must be one of declared tunnel types | — | M004 | `fixture_tm_type` | Exact type names |
| `name` parameter | `params.name` | required for most actions | — | — | Non-empty string | — | M004 | `fixture_tm_name` | — |
| List result | `result` | compatibility `List` only | array | non-null | Historical extension | — | M004 | `fixture_tm_list` | Not in canonical action manifest |
| Create result | `result.status`, `result.results` | on canonical `create` | structured object | non-null | Operation status text and result list | Durable store | M018 | `canonical_wire_fixture_covers_all_seven_actions` | Wire/source implemented |
| Get result | `result.status`, `result.info` | on canonical `get` | structured object | non-null | Tunnel definition in `info` | Durable store | M018 | `canonical_wire_fixture_covers_all_seven_actions` | Runtime lifecycle may remain unsupported |
| Edit result | `result.status` | on Edit | string | non-null | Operation status text | — | M004 | `fixture_tm_edit` | — |
| Delete result | `result.status` | on Delete | string | non-null | Operation status text | — | M004 | `fixture_tm_delete` | — |
| Start result | `result.status` | on Start | string | non-null | Operation status text | — | M004 | `fixture_tm_start` | — |
| Stop result | `result.status` | on Stop | string | non-null | Operation status text | — | M004 | `fixture_tm_stop` | — |
| Restart result | `result.status` | on Restart | string | non-null | Operation status text | — | M004 | `fixture_tm_restart` | — |
| Unsupported start/restart | `result.status` | on unsupported Start/Restart | string | non-null | `"error - ... not implemented"` | Unsupported backend | M004 | `fixture_tm_unsupported_start` | Deterministic error per ADR-0001 |

### TunnelManager All Rule

| Contract item | Request key/type | Required/optional/presence | Response key/type | Nullability | Validation/error behavior | Planned data source | Owner milestone | Fixture/test ID | Notes |
|---|---|---|---|---|---|---|---|---|---|
| All tunnel name | `params.name` = `"All"` | reserved | — | — | Must not be used for Create; used only with Start/Stop/Restart | — | M004 | `fixture_tm_all` | Exact spelling: `All` (capital A) |
| All Start/Stop/Restart | canonical lowercase action + `All: true` | — | structured status object | non-null | Dispatches all definitions; unsupported backends remain explicit | Backend registry | M018 | `canonical_wire_fixture_covers_all_seven_actions` | Runtime support is per backend |

### Tunnel Types

| Contract item | Request key/type | Required/optional/presence | Response key/type | Nullability | Validation/error behavior | Planned data source | Owner milestone | Fixture/test ID | Notes |
|---|---|---|---|---|---|---|---|---|---|
| `client` | type value | — | — | — | Parses; real or unsupported backend | — | M004 | `fixture_type_client` | I2P HTTP client proxy |
| `httpclient` | type value | — | — | — | Parses; real or unsupported backend | — | M004 | `fixture_type_httpclient` | HTTP proxy |
| `ircclient` | type value | — | — | — | Parses; real or unsupported backend | — | M004 | `fixture_type_ircclient` | IRC client proxy |
| `socks` | type value | — | — | — | Parses; real or unsupported backend | — | M004 | `fixture_type_socks` | SOCKS proxy |
| `socksirc` | type value | — | — | — | Parses; real or unsupported backend | — | M004 | `fixture_type_socksirc` | SOCKS-IRC |
| `connectclient` | type value | — | — | — | Parses; real or unsupported backend | — | M004 | `fixture_type_connectclient` | CONNECT proxy |
| `streamrclient` | type value | — | — | — | Parses; real or unsupported backend | — | M004 | `fixture_type_streamrclient` | Streamr client |
| `server` | type value | — | — | — | Parses; real or unsupported backend | — | M004 | `fixture_type_server` | Basic server |
| `httpserver` | type value | — | — | — | Parses; real or unsupported backend | — | M004 | `fixture_type_httpserver` | HTTP server |
| `httpbidirserver` | type value | — | — | — | Parses; real or unsupported backend | — | M004 | `fixture_type_httpbidirserver` | Bidirectional HTTP server |
| `ircserver` | type value | — | — | — | Parses; real or unsupported backend | — | M004 | `fixture_type_ircserver` | IRC server |
| `streamrserver` | type value | — | — | — | Parses; real or unsupported backend | — | M004 | `fixture_type_streamrserver` | Streamr server |

### ClientServicesInfo

| Contract item | Request key/type | Required/optional/presence | Response key/type | Nullability | Validation/error behavior | Planned data source | Owner milestone | Fixture/test ID | Notes |
|---|---|---|---|---|---|---|---|---|---|
| ClientServicesInfo method | `method` = `"ClientServicesInfo"` | required | — | — | — | — | M006 | `fixture_client_services_info` | — |
| I2PTunnel selector | direct `I2PTunnel` presence | selector | object (`{client: {}, server: {}}`) | non-null when returned | Any value selects; nested `Selector` is compatibility-only | Live `TunnelManagerControl::list()` | M018 | `canonical_direct_wire_fixture_selects_by_presence` | Wire/source implemented |
| HTTPProxy selector | `HTTPProxy` | selector | object (`{enabled, address, port}`) | non-null when returned | Only returned if requested; `enabled: true` only after bind | Service registry listener observation | M011 | `fixture_csi_httpproxy` | `enabled: true` only for `Listening` state (M011) |
| SOCKS selector | `SOCKS` | selector | object (`{enabled, address, port}`) | non-null when returned | Only returned if requested; `enabled: true` only after bind | Service registry listener observation | M011 | `fixture_csi_socks` | `enabled: true` only for `Listening` state (M011) |
| SAM selector | `SAM` | selector | object (`{enabled, sessions}`) | non-null when returned | Only returned if requested | Service registry listener plus canonical bounded `SamServer` snapshot | M016 | `fixture_csi_sam` | Active primary sessions and sanitized sockets are current; overflow is an explicit internal error |
| BOB selector | `BOB` | selector | boolean (`false`) | non-null when returned | Only returned if requested | Exact Proposal 170 value | M006 | `fixture_csi_bob` | Not implemented in Emissary |
| I2CP selector | `I2CP` | selector | object (`{enabled}`) | non-null when returned | Only returned if requested | Service registry I2CP listener observation | M011 | `fixture_csi_i2cp` | `enabled: true` only while bound (M011) |

## 3. JSON-RPC Envelope Rules

| Contract item | Key/Type | Behavior | Notes |
|---|---|---|---|
| `jsonrpc` version | `"2.0"` | Required exactly `"2.0"` in request | Per JSON-RPC 2.0 spec |
| Request ID | string or integer | Preserved exactly in response | Null ID (notification) has no response |
| Named params | `params` = object | Required; positional params rejected | Per I2PControl convention |
| Success response | `{"jsonrpc":"2.0","id":...,"result":...}` | Exact envelope | No extra keys |
| Error response | `{"jsonrpc":"2.0","id":...,"error":{"code":...,"message":"..."}}` | Exact envelope | `data` field optional |
| Error code `-1` | Parse error / invalid request | Malformed JSON or invalid JSON-RPC structure | — |
| Error code `-32600` | Invalid request | Valid JSON but not a valid JSON-RPC request | — |
| Error code `-32601` | Method not found | Unknown method name | — |
| Error code `-32602` | Invalid params | Method exists but params are wrong | — |
| Error code `-32603` | Internal error | Server-side failure | Sanitized message |
| Batch requests | Array of requests | Rejected unless base contract requires | I2PControl does not require batch |
| Notification | Null ID | No response sent | Per JSON-RPC 2.0 |

## 4. Proposal 170 Ambiguities and Resolutions

| Ambiguity | Resolution | Source |
|---|---|---|
| Authenticate API version `1` vs `2` | Accept both; return negotiated version | Base I2PControl backward compatibility |
| Error code for auth failure | Use JSON-RPC standard error codes; authentication failure returns code `-1` with descriptive message | Base I2PControl convention |
| AddressBook Delete presence semantics | Presence of `name` param = delete specific entry; absence = delete all entries in book | Proposal 170 specification |
| TunnelManager `All` reserved name | `All` is a reserved tunnel name; cannot be used for Create; used with Start/Stop/Restart | Proposal 170 specification |
| Selector-based response filtering | Only requested selector keys appear in response; absent selectors produce no response keys | Proposal 170 specification |
| Batch JSON-RPC | Not required by I2PControl; rejected with standard error | Base I2PControl convention |
