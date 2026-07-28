# I2PControl Tunnel Backends

Status: M002 infrastructure implemented

This document describes the tunnel backend interface and registry in Emissary.

## Overview

The tunnel backend system provides a clean separation between the Proposal 170 control plane and the actual tunnel runtime. Each tunnel type resolves to exactly one backend implementation.

## Backend trait

The `TunnelBackend` trait defines the interface for tunnel runtime backends:

```rust
#[async_trait]
pub trait TunnelBackend: Send + Sync {
    fn tunnel_type(&self) -> TunnelType;
    async fn start(&self, definition: &TunnelDefinition) -> BackendResult<()>;
    async fn stop(&self, definition: &TunnelDefinition) -> BackendResult<()>;
    fn inspect(&self, definition: &TunnelDefinition) -> BackendStatus;
}
```

### Contract

- `start` must not allocate listeners, destinations, sessions, tasks, or traffic paths for unsupported backends
- `stop` of an inactive definition must be safe and resource-free
- `inspect` must return the current state without side effects
- All methods must honor caller deadlines without blocking

### Error types

```rust
pub enum BackendError {
    NotImplemented { tunnel_type: TunnelType },
    InvalidState {
        tunnel_type: TunnelType,
        current_state: TunnelRuntimeState,
        attempted_action: &'static str,
    },
    Internal { message: String },
}
```

## Backend registry

The `TunnelBackendRegistry` provides exhaustive registration of backends for all 12 tunnel types.

### Construction

```rust
let registry = TunnelBackendRegistry::new(backends)?;
```

Construction fails if:
- A tunnel type is registered more than once (`DuplicateRegistration`)
- A tunnel type is missing from the registration (`MissingRegistration`)

### Lookup

```rust
let backend = registry.get(tunnel_type);
```

Lookup is total for valid tunnel types. The registry is constructed once at startup and not modified thereafter.

### Default registry

`create_default_registry()` maps all 12 tunnel types to `UnsupportedTunnelBackend`:

```rust
pub fn create_default_registry() -> Result<TunnelBackendRegistry, RegistryError> {
    let backends: Vec<Arc<dyn TunnelBackend>> = ALL_TUNNEL_TYPES
        .iter()
        .map(|&tt| Arc::new(UnsupportedTunnelBackend::new(tt)) as Arc<dyn TunnelBackend>)
        .collect();
    TunnelBackendRegistry::new(backends)
}
```

## Unsupported backend

`UnsupportedTunnelBackend` is the baseline backend for all tunnel types at M002:

- Constructible for any declared tunnel type
- Returns typed `NotImplemented` from `start`
- Treats `stop` of an inactive definition as safe and resource-free
- Inspects as internal `Unsupported`
- Never spawns or binds anything

### Behavior

| Operation | Behavior |
|---|---|
| `start()` | Returns `Err(BackendError::NotImplemented { tunnel_type })` |
| `stop()` | Returns `Ok(())` unconditionally |
| `inspect()` | Returns `BackendStatus { runtime_state: Unsupported, message: "..." }` |

## Fake backend

`FakeTunnelBackend` supports deterministic success/failure/state scripting for handler tests:

```rust
let script = FakeBackendScript {
    start_action: FakeAction::Success,
    stop_action: FakeAction::Success,
    inspect_state: TunnelRuntimeState::Running,
    inspect_message: "running in test".to_string(),
};
let backend = FakeTunnelBackend::with_script(TunnelType::Socks, script);
```

### Scripted behavior

- `FakeAction::Success` - Operation succeeds
- `FakeAction::Error(BackendError)` - Operation fails with the given error

Scripts can be updated at runtime via `set_script()`.

### Fake registry

`FakeBackendRegistry` provides an in-memory registry for tests:

```rust
let mut registry = FakeBackendRegistry::new();
registry.register(Arc::new(FakeTunnelBackend::new(TunnelType::Socks)));
let backend = registry.get(TunnelType::Socks);
```

## Tunnel types

All 12 tunnel types are mapped to backends:

| Type | Category | Backend |
|---|---|---|
| `client` | Client | Unsupported |
| `httpclient` | Client | Unsupported |
| `ircclient` | Client | Unsupported |
| `socks` | Client | Unsupported |
| `socksirc` | Client | Unsupported |
| `connectclient` | Client | Unsupported |
| `streamrclient` | Client | Unsupported |
| `server` | Server | Unsupported |
| `httpserver` | Server | Unsupported |
| `httpbidirserver` | Server | Unsupported |
| `ircserver` | Server | Unsupported |
| `streamrserver` | Server | Unsupported |

Real backend implementations will be added in future milestones (M004+).

## Design rationale

### Why a registry?

The registry ensures:
- No tunnel type is accidentally left without a backend
- Duplicate registrations are caught at startup
- Backend selection is O(1) via HashMap lookup
- The full set of backends is visible in one place

### Why unsupported backends?

Unsupported backends allow the control plane to:
- Accept and persist tunnel definitions for all types
- Return typed errors when attempting to start unsupported tunnels
- Avoid resource allocation for types without runtime support
- Provide a clean migration path as real backends are implemented

### Why not a mutable registry?

The registry is immutable after construction because:
- Backend assignments are fixed at compile time
- Runtime modification would require complex synchronization
- The exhaustive check is simpler with a fixed set
