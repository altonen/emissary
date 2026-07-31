# M016 Implementation Disposition — Bounded SAM Session Observation

Status: closing

Frozen implementation head: `355e243` (`fix(i2pcontrol): expose bounded SAM session observations`)

This is the amended M016 implementation evidence record. The earlier
`016-closure.md` remains the superseded pre-amendment blocker record and is
not acceptance evidence.

## Executor and handoff

Implementation executor: Codex implementation run for M016, recorded in the
repository session that produced `355e243`.

The implementation head is frozen for the independent M017 review. M017 is
now ready and must be performed by a distinct, auditable reviewer. This
record does not independently close the Proposal 170 workstream.

## Pinned field semantics

The adopted source is pinned i2pd commit
`7866f644d3d3dea3d1adf5374a6ea378c8efd536`, `daemon/I2PControl.cpp`,
`I2PControlService::SAMInfoHandler`:

| Wire field | Pinned source expression | Emissary mapping |
|---|---|---|
| session map key | `it.first` while iterating `sam->GetSessions()` | SAM session identifier (`Arc<str>`) |
| `name` | `it.second->localDestination->GetNickname()` | `inbound.nickname`, then `outbound.nickname`, then the first four characters of the destination identity's I2P Base64 form, matching i2pd's default abbreviation |
| `address` | `context.GetAddressBook().ToAddress(ident)` | destination identity encoded as lowercase I2P Base32 with `.b32.i2p` suffix |
| socket `type` | `socket->GetSocketType()` | active i2pd values `1` session, `2` stream, `3` acceptor |
| socket `peer` | `socket->GetSocket().remote_endpoint()` | accepted TCP peer rendered as `SocketAddr` text |

Only active primary sessions are represented, matching i2pd's
`GetSessions()` map. SAM sub-sessions are routed through their primary
session and do not create separate entries. The disabled listener response
remains `enabled: false` with an empty `sessions` object; a listening
zero-session response is an actual empty snapshot.

## Requirement and evidence matrix

| Requirement | Evidence | Result |
|---|---|---|
| One bounded read-only handle with private publisher | `SamSessionObservationHandle`, private `SamSessionObservationPublisher`, `SamServer::observation_handle()` | Pass |
| Fixed bounds and explicit overflow | `SAM_SESSION_OBSERVATION_LIMIT = 1000`, `SAM_SOCKET_OBSERVATION_LIMIT = 8`, overflow snapshots return an existing internal error path | Pass |
| Exact session metadata only | `SamObservedSession` contains only `name`, `address`, `sockets`; socket DTO contains only `type`, `peer` | Pass |
| Active-session truthfulness | publication occurs when a pending session becomes active; removal follows active future completion | Pass |
| Socket lifecycle | session socket is published at activation; stream/acceptor sockets are published at existing command adoption and removed on failure/close/session removal | Pass |
| Composition | handle is cloned before `SamServer` is spawned and passed through `Router` into production I2PControl state | Pass |
| No secrets or authority | read handle exposes only `snapshot()`; no keys, destinations, payloads, sockets, streams, command channels, or lifecycle methods cross the boundary | Pass |
| No lock across async work | snapshot and writer updates are synchronous bounded metadata copies | Pass |
| Existing fencing and connection proof preserved | prior accepted evidence at `9047fee`; no changes to those paths | Pass |

## Verification outcomes

Passed:

```text
cargo check -p emissary-core --features std,events
cargo test -p emissary-core
cargo clippy -p emissary-core --features std,events --all-targets -- -D warnings
cargo check -p emissary-cli --no-default-features --features i2pcontrol
cargo test -p emissary-cli --no-default-features --features i2pcontrol
cargo clippy -p emissary-cli --no-default-features --features i2pcontrol --all-targets -- -D warnings
nightly rustfmt --check on every touched Rust file
```

The repository-wide stable `cargo fmt --all -- --check` remains blocked by
pre-existing formatting differences in untouched files and by unstable
rustfmt options configured by this repository. No unrelated files were
reformatted; all touched Rust files pass the nightly formatter configured for
the project.

## Scope and unresolved findings

Changed production scope is limited to the SAM observation DTO/publisher,
existing SAM socket peer propagation, the existing session/router composition
seams, the I2PControl serializer/state seam, directly affected tests, and
Proposal 170 documentation/planning records. No CI, release, generic
observer, polling, persistence, protocol, router, transport, NetDB, tunnel,
frontend, or cryptographic work was added.

M016-F1 is implemented and has no remaining high or medium defect identified
in this implementation pass. M016-F2 and M016-F3 remain resolved at
`9047fee`. Independent M017 review is still required before the subsystem can
be marked closed.

## Disposition

`closing`: implementation is complete at the frozen head and the downstream
independent final-head reclosure plan is unblocked. M017 owns final acceptance
or rejection and must not repair production defects while closing the plan.
