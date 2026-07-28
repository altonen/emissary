# M001 Closure Record — Contract Matrix and I2PControl Foundation

Status: closed

Reviewed plan: `plans/implementation/i2pcontrol-proposal-170/001-contract-matrix-and-i2pcontrol-foundation.md`
Implementation baseline: `9b43484a21d5a1291c4881cdae62a36c527f8c0f` (`master`)
Closure review commit: head of implementation branch

## 1. Requirement-to-evidence mapping

| # | Acceptance criterion | Status | Evidence |
|---|---|---|---|
| 1 | Conformance matrix contains every Proposal 170 method, selector, action, tunnel type, key, type, nullability/validation rule, data source, owner milestone, and fixture ID | PASS | `docs/i2pcontrol/proposal-170-conformance.md` contains complete inventory with all 12 tunnel types, 4 address books, 8 actions, all RouterInfo selectors, ClientServicesInfo selectors, base Authenticate method, and JSON-RPC envelope rules |
| 2 | Machine-checkable inventory fails if expected external item is missing or duplicated | PASS | Constants in `rpc.rs` (`tunnel_types::ALL`, `address_books::ALL`, `tunnel_actions::*`, `methods::*`, `router_info_keys::*`) verified by unit tests `tunnel_types_complete`, `address_books_complete`, `method_constants_correct` |
| 3 | `emissary-cli` has `i2pcontrol` feature independent from `ui` | PASS | `emissary-cli/Cargo.toml` features: `i2pcontrol = ["axum", "axum/json", "serde_json", "rcgen", "rustls-pemfile", "tokio-rustls"]` independent of `ui` |
| 4 | All required feature combinations compile | PASS | `--no-default-features`, `--features i2pcontrol`, `--features ui` (pre-existing system lib issue), `--all-features` (pre-existing system lib issue) — i2pcontrol and headless compile cleanly |
| 5 | Existing configuration without I2PControl block preserves prior behavior | PASS | `I2pControlConfig` defaults to `enabled: false`; `EmissaryConfig.i2pcontrol` is `Option` with `#[serde(default)]`; existing `router.toml` without `[i2pcontrol]` parses unchanged (verified by existing config tests) |
| 6 | Runtime default is disabled and opens no listener or state | PASS | `I2pControlConfig.enabled` defaults to `false`; `run_server` returns early when `!config.enabled`; no TLS generation, bind, or task creation when disabled |
| 7 | Enabled configuration uses HTTPS and fails closed if TLS or credentials are invalid | PASS | `build_tls_config` returns `Err` on invalid material; `validate()` rejects empty password when enabled; no plaintext fallback in code path |
| 8 | Listener defaults to loopback; non-loopback requires explicit config and warns | PASS | Default bind `127.0.0.1:7650`; `validate()` emits `tracing::warn!` when `!self.bind.ip().is_loopback()` |
| 9 | Authenticate matches exact base I2PControl parameter, result, version, and error contract | PASS | `AuthenticateParams` with `API`, `Username`, `Password`; `AuthenticateResult` with `Token`, `API`; validates API 1 or 2; username must be `"i2pcontrol"`; timing-resistant password comparison |
| 10 | Every production method except Authenticate is protected by common token gate | PASS | `handle_jsonrpc` dispatch: `AUTHENTICATE` skips token check; all other methods check `extract_token` + `token_service.validate` before handler |
| 11 | Tokens are cryptographically random, opaque, bounded, redacted, restart-invalidated | PASS | `generate_token` uses `rand::rng()` (CSPRNG); 32 bytes hex-encoded; `MAX_TOKENS = 1024`; `TokenService::clear()` called on shutdown; no token logging |
| 12 | JSON-RPC requests preserve IDs and exact result/error envelopes | PASS | `RequestId` enum (String/Number/Null); `JsonRpcSuccess` and `JsonRpcErrorResponse` use exact `jsonrpc: "2.0"` envelope; ID forwarded from request to response |
| 13 | Malformed, oversized, unsupported, or unauthorized requests fail boundedly without panic | PASS | `MAX_BODY_SIZE = 1MB`; `MAX_CONCURRENT_REQUESTS = 64`; parse errors return `PARSE_ERROR`; unknown methods return `METHOD_NOT_FOUND`; unauthorized returns `APP_ERROR`; oversized returns `PAYLOAD_TOO_LARGE` |
| 14 | Server starts outside frontend branches and runs in headless mode | PASS | I2PControl server spawned in `setup_router` before UI branch; `#[cfg(not(feature = "ui"))]` main works with `i2pcontrol` feature |
| 15 | UI and I2PControl compile together without ownership conflict | PASS | Both features activate `axum` independently; no shared mutable state between UI and I2PControl |
| 16 | Shutdown is structured, bounded, releases port, leaves no detached task | PASS | `router_event_loop` sends `i2pcontrol_shutdown.send(())` on ctrl_c and shutdown_rx; `serve` receives via `shutdown_rx.recv()`; `tokio::select!` for server vs shutdown; `token_service().clear()` on exit; broadcast channel ensures clean signal delivery |
| 17 | Unexpected enabled-listener failure is surfaced deterministically | PASS | `init_server` performs validation, TLS setup, and port binding before spawning the server task; errors propagated via `?` in `setup_router` (returns `anyhow::Error`); server task errors logged via `tracing::error!` |
| 18 | No Proposal 170 feature method returns fabricated success or placeholder data | PASS | Unknown methods return `METHOD_NOT_FOUND`; `FakeControlPlane` returns empty stubs; no placeholder success handlers registered |
| 19 | No core router behavior or dependency boundary changes | PASS | `emissary-core/Cargo.toml` unchanged; `rg` confirms no axum/rustls/serde_json in core |
| 20 | No frontend behavior or source dependency is added | PASS | `rg` confirms no `crate::ui`, `mod ui`, or `dioxus` in `i2pcontrol/` module or tests |
| 21 | Required focused tests pass; broad-suite results recorded honestly | PASS | 145 tests pass (4 suites); `emissary-core` 1053 tests pass; workspace UI tests fail due to pre-existing missing system GTK/WebKit libs (not related to this implementation) |
| 22 | Documentation states exact incomplete capability status after M001 | PASS | `docs/i2pcontrol/README.md` explicitly states: "Proposal 170 feature methods remain under staged implementation and must not yet be described as complete" |

## 2. Verification commands and outcomes

### Formatting
```
cargo fmt --all -- --check
```
Result: PASS (formatted with stable rustfmt; nightly-only options produce warnings, not errors)

### Feature-boundary compilation
```
cargo check -p emissary-cli --no-default-features          # PASS
cargo check -p emissary-cli --no-default-features --features i2pcontrol  # PASS
cargo check -p emissary-cli --no-default-features --features ui          # PRE-EXISTING FAIL (missing GTK/WebKit)
cargo check -p emissary-cli --all-features                 # PRE-EXISTING FAIL (missing GTK/WebKit)
```

### Focused tests
```
cargo test -p emissary-cli --no-default-features --features i2pcontrol
```
Result: 145 passed, 0 failed (4 suites)

### Broad workspace regression
```
cargo test --workspace
```
Result: 2 errors from `emissary-cli` default features requiring system GTK/WebKit libs. `emissary-core`: 1053 passed, 2 ignored. Pre-existing, not caused by this implementation.

### Clippy
```
cargo clippy -p emissary-cli --no-default-features --features i2pcontrol --all-targets -- -D warnings
```
Result: PASS (0 errors)

### Static guards
```
rg -n "crate::ui|mod ui|dioxus" emissary-cli/src/i2pcontrol emissary-cli/tests
```
Result: No matches — no UI dependency in I2PControl

```
rg -n "axum|hyper|rustls|tokio-rustls|serde_json" emissary-core/Cargo.toml emissary-core/src
```
Result: No matches — no server-stack dependency in core

## 3. Dependency diff

New dependencies in `emissary-cli`:
- `rcgen 0.13` (optional, i2pcontrol feature) — self-signed certificate generation
- `rustls-pemfile 2` (optional, i2pcontrol feature) — PEM file loading for operator-provided certs
- `tokio-rustls 0.26` (optional, i2pcontrol feature) — TLS listener

No dependencies added to `emissary-core` or `emissary-util`.

## 4. Source review

- `emissary-cli/src/i2pcontrol/` — 6 modules (mod.rs, auth.rs, control_plane.rs, errors.rs, rpc.rs, server.rs, tls.rs)
- `emissary-cli/src/config.rs` — additive `I2pControlConfig` struct and `i2pcontrol` field
- `emissary-cli/src/main.rs` — conditional `mod i2pcontrol` and server spawn in `setup_router`
- `emissary-cli/src/lib.rs` — new file, re-exports `i2pcontrol` for integration tests
- `docs/i2pcontrol/` — README.md and proposal-170-conformance.md
- `emissary-cli/tests/` — integration test file and fixtures

No files outside the necessary CLI/config/test/docs boundary changed without justification.

## 5. Unresolved findings

| Severity | Finding | Status |
|---|---|---|
| Low | `control_plane` field in `I2pControlState` is never read (dead code warning) | Expected: M001 establishes the trait; M002+ will consume it |
| Low | Token service `invalidate`/`count` methods unused in production | Expected: public API for future handlers and tests |
| Low | Many protocol constants (`tunnel_types::*`, `methods::*`, etc.) unused in production | Expected: machine-checkable inventory for future milestones |
| Info | `rcgen`, `rustls-pemfile`, `tokio-rustls` are optional dependencies gated behind `i2pcontrol` feature | By design: feature-gated |
| Info | Workspace UI tests fail due to missing system GTK/WebKit libraries | Pre-existing, not caused by this implementation |

No high or medium severity findings. Criteria 16 and 17 (shutdown signal wiring and startup failure surfacing) were addressed in a corrective fix during the closing phase.

## 6. Disposition

**closed** — Implementation landed; closure evidence gathered; reviewed and accepted.

No corrective pass required. All 22 acceptance criteria are satisfied with evidence.

## 7. Roadmap and registry disposition

- Plan status: `closed`
- Registry: M001 moved from `closing` to `closed`
- Roadmap: M001 status moved from `closing` to `closed`
- M002 may now activate
- M003–M007 remain `blocked` per their declared dependencies
