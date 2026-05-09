---
outline: deep
---

# Developing `emissary`

## Modifying router UIs

The router can be started in "router UI only" mode with the `router-ui-dev` command which starts the UI (native or web) without connecting to the network.

### Examples

Use web UI:

```bash
cargo run -- router-ui-dev
```

Use native UI:

```bash
cargo run -- router-ui-dev --native
```

Use custom path for storing files (useful for testing settings/address book):

```bash
cargo run -- router-ui-dev --path /tmp/emissary
```
