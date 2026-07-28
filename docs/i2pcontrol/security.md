# I2PControl Security

Status: M002 infrastructure implemented

This document describes the security properties and considerations for the I2PControl administrative state in Emissary.

## Authentication

I2PControl uses JSON-RPC authentication with opaque tokens:

- Passwords are compared timing-resistantly
- Tokens are cryptographically random (32 bytes, hex-encoded)
- Tokens are stored in-memory only; no persistence
- Maximum concurrent tokens bounded at 1024
- Credentials are never logged or included in Debug output

See [README.md](README.md) for authentication details.

## Secret handling

### Redacted values

Sensitive tunnel options (passwords, keys) use the `OptionRedacted` wrapper:

```rust
pub struct OptionRedacted(Option<String>);
```

- `Debug` output shows `OptionRedacted(***)` instead of the value
- `Display` output shows `***` instead of the value
- The actual value is stored in memory for persistence but never logged

### Affected fields

The following `TunnelOptions` fields are redacted:

- `ssl_key` - SSL private key path
- `proxy_password` - SOCKS/HTTP proxy password
- `irc_password` - IRC server password

### Logging policy

- Complete tunnel definitions are never logged wholesale
- Individual option values are logged only at debug level with redaction
- Error messages from backends contain tunnel type but not secrets

## File system security

### Path confinement

The generation store enforces path confinement:

- Store directories must be real directories (not symlinks)
- All resolved paths must remain within the configured base path
- Symlinks in the generation directory are rejected during load
- User-provided identifiers (tunnel names) are used as BTreeMap keys, never as filesystem paths

### File permissions

On Unix systems, generation files are created with restrictive permissions:

- Mode `0o600` (owner read/write only)
- Applies to both temporary and final generation files
- Non-Unix platforms rely on OS file permissions

### Atomic publication

State updates use atomic rename to prevent corruption:

1. Write to a temporary file (`.tmp-gen-NNNNNN.json`)
2. Flush and sync the file
3. Rename to the final path (`gen-NNNNNN.json`)

If the process crashes during publication:
- Temporary files are detected and skipped on next load
- The most recent valid generation is loaded
- Corrupt generations fall back to prior valid ones

### Symlink rejection

Symlinks in the generation directory are:
- Detected during directory scanning
- Rejected with a warning log message
- Never followed or loaded

## State integrity

### Corruption detection

Each generation file includes:
- Schema identifier (`emissary-i2pcontrol`)
- Schema version (currently 1)
- Revision number

Files with unknown schema or version are rejected.

### Corruption recovery

On load, the store:
1. Scans generation files newest-first
2. Tries each file
3. Falls back to the previous valid generation on failure
4. Returns an actionable error if all generations are corrupt

A diagnostic is emitted for each failed generation without exposing payload secrets.

### Oversized state rejection

Each store has a configurable maximum size limit. State that exceeds this limit is rejected before any files are written.

## Startup safety

### No automatic task launch

- `StartOnLoad` is stored in tunnel definitions
- The persistence layer does not execute StartOnLoad
- No tasks are launched from persisted state
- Runtime adoption happens only through explicit handler calls

### No runtime resolver integration

Administrative address books are independent from the runtime resolver:
- Private, local, router, and published books are stored separately
- No book entry affects destination resolution
- Existing runtime address book files are untouched

### Configuration isolation

Proposal 170 state is stored separately from:
- `router.toml` - Router configuration
- Runtime address book (`<base>/addressbook/`)
- Server private key paths
- Frontend configuration files

## Compilation features

### Feature gating

All I2PControl code is gated behind the `i2pcontrol` Cargo feature:

```bash
# Build without I2PControl (default)
cargo build -p emissary-cli

# Build with I2PControl
cargo build -p emissary-cli --no-default-features --features i2pcontrol
```

### Core crate isolation

The `emissary-core` crate contains:
- No I2PControl or administrative persistence dependencies
- No JSON-RPC handling
- No HTTP server code

This ensures the core router remains independent of administrative concerns.

## Known limitations

### fsync not yet implemented

The generation store does not call `fsync` before rename. The atomic rename provides sufficient durability for most use cases, but `fsync` can be added later for stronger guarantees.

### Platform-specific permissions

File permission enforcement is Unix-only. On other platforms:
- Permission bits are not set
- OS-level file permissions apply
- The same filesystem confinement rules still apply

### No encryption at rest

Tunnel definitions containing proxy passwords are stored as plaintext JSON files. Operators should ensure the state directory has appropriate file system permissions.

## Testing

### Security tests

The following security properties are verified by tests:

- `symlink_in_directory_is_rejected` - Symlinks in generation directory are skipped
- `generation_files_have_restrictive_permissions` - Files are created with 0o600 on Unix
- `validate_confined_path_rejects_escape` - Paths escaping base are rejected
- `validate_confined_path_accepts_within_base` - Paths within base are accepted
- `option_redacted_debug_redacts` - Debug output redacts secrets
- `option_redacted_display_redacts` - Display output redacts secrets

### Static guards

Compile-time guards ensure:
- All 12 tunnel types are registered in `ALL_TUNNEL_TYPES`
- All 8 tunnel actions are registered in `ALL_TUNNEL_ACTIONS`
- No tunnel type is accidentally omitted

### No-side-effect tests

Tests verify that unsupported backends:
- Do not call `tokio::spawn`
- Do not allocate listeners or sockets
- Report `Unsupported` state consistently
