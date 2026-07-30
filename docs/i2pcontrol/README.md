# I2PControl for Emissary

Status: Proposal 170 complete (M013 closed)

This document describes the I2PControl HTTPS JSON-RPC service foundation in Emissary.

## Compile feature

I2PControl is an independent Cargo feature in `emissary-cli`. It is **not** enabled by default.

```bash
# Build without I2PControl (default)
cargo build -p emissary-cli

# Build with I2PControl enabled
cargo build -p emissary-cli --no-default-features --features i2pcontrol

# Build with both UI and I2PControl
cargo build -p emissary-cli --all-features
```

## Runtime enablement

Even when compiled with the `i2pcontrol` feature, the service is **disabled by default**.
It only starts when explicitly enabled in the configuration.

Add an `[i2pcontrol]` section to `router.toml`:

```toml
[i2pcontrol]
enabled = true
bind = "127.0.0.1:7650"
password = "your-secure-password"
```

## Configuration

| Field | Type | Default | Description |
|---|---|---|---|
| `enabled` | boolean | `false` | Enable I2PControl listener |
| `bind` | string | `"127.0.0.1:7650"` | Bind address |
| `password` | string | `""` | Authentication password |
| `certificate` | string | (managed) | Optional TLS certificate path |
| `private_key` | string | (managed) | Optional TLS private key path |

### Security notes

- **Default binding is loopback only** (127.0.0.1:7650). Non-loopback binding requires explicit configuration and produces a security warning.
- **Empty password is rejected** when I2PControl is enabled.
- **Credentials are never logged** or included in Debug output.
- **Existing configurations without `[i2pcontrol]`** parse unchanged and preserve prior behavior.

## HTTPS certificate behavior

I2PControl is served over HTTPS. Certificate behavior:

1. **Operator-provided**: If `certificate` and `private_key` paths are configured, those files are loaded.
2. **Managed self-signed**: If no paths are configured, a self-signed certificate is generated under `<base_path>/i2pcontrol-certs/`.
   - Generated only when I2PControl is enabled.
   - Written atomically; not regenerated on every start.
   - Certificate identity is stable across restarts.
   - Invalid existing material triggers regeneration.

**No plaintext HTTP fallback is supported.**

## Authentication

I2PControl uses JSON-RPC authentication:

```json
{
  "jsonrpc": "2.0",
  "method": "Authenticate",
  "params": {
    "API": 2,
    "Username": "i2pcontrol",
    "Password": "your-password"
  },
  "id": 1
}
```

Success returns an opaque token:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "Token": "hex-encoded-token",
    "API": "2"
  }
}
```

Subsequent requests must include the token via the `X-I2PControl-Token` header.

### Token behavior

- Tokens are cryptographically random (32 bytes, hex-encoded).
- Tokens are stored in-memory only; no persistence.
- Tokens are invalidated on process restart.
- Maximum concurrent tokens bounded at 1024.

## M001 support status

**The I2PControl transport, authentication, and JSON-RPC foundation is implemented.**

## M002 support status

**The Proposal 170 control-plane domain types, backend interface, and restart-safe persistence are implemented.**

M002 provides the administrative infrastructure consumed by later milestones:

- **Domain types**: `TunnelType` (12 variants), `TunnelAction` (8 variants), `TunnelName`, `TunnelDefinition`, `TunnelOptions`, `TunnelOwnership`, `TunnelRuntimeState`, `StartIntent`, `OptionRedacted`
- **Address book types**: `AdministrativeAddressBookType` (4 books), `AddressBookEntry`, `SubscriptionSet`, `AddressBookConfiguration`
- **Backend interface**: `TunnelBackend` trait, `UnsupportedTunnelBackend`, `FakeTunnelBackend`, exhaustive registry
- **Persistence**: `GenerationStore<T>` with versioned envelopes, atomic publication, corruption fallback, bounded retention
- **Stores**: `TunnelStore`, `AddressBookStore`, `SubscriptionStore`
- **Fakes**: In-memory fake stores for handler tests

See [administrative-state.md](administrative-state.md), [tunnel-backends.md](tunnel-backends.md), and [security.md](security.md) for details.

Proposal 170 is fully implemented. All methods are operational:

- `RouterInfo` selectors (M009/M010: truthful sources, bounded inspection)
- `TunnelManager` operations (M004: CRUD + lifecycle for all 12 types)
- `ClientServicesInfo` selectors (M011: live tunnel/listener/session state)
- `AddressBook` operations (M003: four persistent stores)
- Real TLS serving (M012: TlsAcceptor retained and consumed)
- Production composition (M008: no fakes, fail-closed, shared state)

## No frontend controls

M001 does not add any frontend controls, screens, views, or state. I2PControl runs independently of the UI. The UI and I2PControl features are independent and do not conflict.
