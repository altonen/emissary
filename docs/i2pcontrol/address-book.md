# Proposal 170 AddressBook Administrative API

Status: M003 complete — AddressBook, SetSubscriptions, and SetConfig methods implemented

This document describes the Proposal 170 AddressBook administrative API for Emissary's I2PControl service.

## Overview

The AddressBook API provides administrative management of four independent address books, a subscription set, and a configuration map. These are **administrative stores only** — they do not affect runtime destination resolution.

## Methods

### AddressBook

The `AddressBook` method performs CRUD operations on one of four administrative address books.

**Parameters:**

| Parameter | Type | Required | Description |
|---|---|---|---|
| `book` | string | yes | One of: `private`, `local`, `router`, `published` |
| `request` | string | yes | One of: `List`, `Lookup`, `Add`, `Update`, `Delete` |
| `name` | string | for Lookup/Add/Update/Delete | Hostname (e.g., `example.i2p`) |
| `value` | string | for Add/Update | I2P destination (base64) |

**Operations:**

- **List**: Returns all entries in the specified book as `[{name, value}, ...]`
- **Lookup**: Returns a single entry or `null` if not found
- **Add**: Creates a new entry; fails if hostname already exists
- **Update**: Updates an existing entry; fails if hostname not found
- **Delete**: Deletes an entry by `name` presence; without `name`, deletes all entries in the book

**Delete-by-presence semantics:**

The `Delete` operation uses parameter presence, not boolean value. If `name` is present in the request (regardless of value), it selects deletion of that specific entry. If `name` is absent, it deletes all entries in the book.

### SetSubscriptions

The `SetSubscriptions` method atomically replaces the subscription set.

**Parameters:**

| Parameter | Type | Required | Description |
|---|---|---|---|
| `subscriptions` | array of strings | yes | Ordered list of subscription URLs |

Subscriptions are stored but **not fetched** by this API. Maximum 1000 subscriptions, 2048 bytes each.

### SetConfig

The `SetConfig` method atomically replaces the address book configuration.

**Parameters:**

| Parameter | Type | Required | Description |
|---|---|---|---|
| `config` | object | yes | String-keyed configuration map |

Configuration values are stored as inert strings. Path-like values perform no filesystem operations. Maximum 1000 entries, 256-byte keys, 4096-byte values.

## Address Books

| Book | Description |
|---|---|
| `private` | Private administrative book |
| `local` | Local administrative book |
| `router` | Router administrative book |
| `published` | Published administrative book |

Each book is independently persistent and isolated from the others.

## RouterInfo Selectors

The following selectors expose address-book state through the RouterInfo method:

| Selector | Type | Description |
|---|---|---|
| `i2p.router.addressbook.private` | array | Private book entries |
| `i2p.router.addressbook.local` | array | Local book entries |
| `i2p.router.addressbook.router` | array | Router book entries |
| `i2p.router.addressbook.published` | array | Published book entries |
| `i2p.router.addressbook.subscriptions` | array | Subscription URLs |
| `i2p.router.addressbook.config` | object | Configuration key-value pairs |

## Persistence

All administrative state persists through M002's `GenerationStore` with:

- Versioned envelopes
- Atomic publication (write-to-temp, rename)
- Corruption fallback to prior valid generation
- Bounded retention

## Security

- Authentication required for all operations
- No full destinations logged
- No subscription values logged
- No configuration values logged
- No filesystem paths derived from input
- Path-like configuration values are inert

## Runtime Independence

**Administrative books do not affect runtime destination resolution.** The current runtime address book, downloader, and resolver remain unchanged. No automatic import or export occurs between administrative and runtime state.
