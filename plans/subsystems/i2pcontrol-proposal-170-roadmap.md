# I2PControl Proposal 170 Roadmap

Status: closed — M017 final-head review accepted

Current planning baseline: `b2f45de`

Canonical references:

- `plans/000-long-term-specification.md`
- `plans/003-planning-process.md`
- `plans/adrs/ADR-0001-proposal-170-contract-and-stub-boundary.md`
- `docs/i2pcontrol/proposal-170-conformance.md`
- I2P Proposal 170 and pinned i2pd `ClientServicesInfo` behavior

## 1. Purpose and boundary

This subsystem owns the exact Proposal 170 I2PControl administrative contract. It does not own router algorithms, transport behavior, NetDB behavior, tunnel construction, frontend behavior, runtime address resolution, release automation, or general CI/security policy.

Current work is limited to truthful SAM `ClientServicesInfo` session reporting through one bounded read-only observation handle.

## 2. Current state

M014 materially corrected RouterInfo truthfulness, live metric/log composition, service observation, and local TLS resource bounds. Its remaining SAM active-session finding was resolved by M016 and accepted by M017.

At `9047feecde046dac8e0208bbf1acf2e3883f97ae`:

- service generation validation and replacement became atomic;
- the connection-bound regression began proving real saturation, rejection, and permit restoration.

The earlier M016 blocked record correctly stopped because its original scope did not authorize the shared observation seam required by the adopted SAM contract.

The architecture owner authorized one fixed-capacity, read-only SAM session-observation handle. M016 implemented it at frozen head `355e243`; M017 accepted the final reviewed head `b2f45de`.

## 3. Remaining finding

| Finding | Severity | Owner | Required correction |
|---|---|---|---|
| A listening SAM bridge can return `sessions: {}` while active sessions exist because I2PControl has no canonical bounded read source | medium | M016 | resolved by exact pinned i2pd session map through one bounded read-only SAM observation handle at `355e243`; accepted by M017 |

No fencing or connection-limit finding remains active.

## 4. Corrective sequence

```text
M016 Bounded SAM session observation corrective pass
    |
    v
M017 Final-head independent reclosure
```

M016 and M017 are closed. The workstream is complete with explicit unavailable selectors and deferred tunnel data planes documented.

## 5. Milestone 016

Plan:

- `plans/implementation/i2pcontrol-proposal-170/016-sam-fencing-and-connection-proof-corrective-pass.md`

Objective:

- define exact pinned i2pd field mappings;
- create one fixed-capacity SAM observation state;
- keep the publisher private to SAM lifecycle code;
- clone a read-only handle before `SamServer` moves into the router runtime;
- pass the handle through existing composition into I2PControl;
- serialize current sessions on demand;
- ensure empty means genuinely zero sessions;
- fail rather than truncate, fabricate, or expose sensitive state.

Allowed core boundary:

- `emissary-core/src/sam/mod.rs`
- `emissary-core/src/sam/session.rs`
- one existing SAM stream/socket module only if required for the exact sanitized socket summary
- smallest router composition seam that creates and moves `SamServer`

Allowed CLI boundary:

- `emissary-cli/src/i2pcontrol/client_services.rs`
- smallest existing DTO/control trait and composition seam needed to carry the read handle

The handle is an administrative projection only. It must not change SAM behavior, share live socket/stream objects, or expose lifecycle authority.

Exit conditions:

- exact adopted map implemented and bounded;
- active session and removal behavior tested;
- socket summaries tested when required by the adopted shape;
- overflow produces an existing error, not partial or empty success;
- sensitive material excluded;
- targeted core and CLI verification passes;
- no unrelated changes;
- frozen implementation head recorded;
- M016 moves to `closing`; M017 becomes `ready`.

## 6. Milestone 017

Plan:

- `plans/implementation/i2pcontrol-proposal-170/017-final-head-independent-reclosure.md`

M017 independently reviews the actual final head after M016. The reviewer must be distinct from the final M016 implementation executor and identify the separate agent/run or equivalent auditable evidence.

Review remains focused on:

- exact SAM field semantics and bounds;
- session/socket lifecycle freshness;
- secret exclusion;
- preservation of M014 live sources and unavailable paths;
- atomic fencing and connection saturation spot checks;
- final changed-file scope;
- current documentation and registry claims.

M017 must not repair production defects and close them in the same pass.

## 7. Invariants

- No Proposal 170 method, selector, key, status, error envelope, or extension is added.
- `sessions: {}` represents a genuine zero-session snapshot only.
- Observation state is fixed-capacity and current.
- Overflow or snapshot failure is explicit; no partial or stale response.
- No private key, destination secret, payload, authentication material, command channel, socket, stream object, or mutable session handle is exposed.
- No lock spans async, network, storage, tunnel, or cryptographic work.
- No generic observer, event bus, cache, polling task, persistence layer, or lifecycle registry is introduced.
- No CI, release, repository-formatting, frontend, router, transport, NetDB, tunnel, cryptographic, or broad security scope enters.

## 8. Verification policy

Use targeted local verification:

```bash
cargo fmt --all -- --check
cargo check -p emissary-core --features std,events
cargo test -p emissary-core
cargo clippy -p emissary-core --features std,events --all-targets -- -D warnings
cargo check -p emissary-cli --no-default-features --features i2pcontrol
cargo test -p emissary-cli --no-default-features --features i2pcontrol
cargo clippy -p emissary-cli --no-default-features --features i2pcontrol --all-targets -- -D warnings
```

If unrelated repository formatting blocks the full format check, run nightly rustfmt on touched files and record the baseline limitation. Do not reformat unrelated code.

No remote CI evidence, full workspace campaign, platform matrix, coverage gate, fuzz campaign, release dry run, or evidence archive is required.

## 9. Completion definition

The workstream returns to `closed` only after M017 confirms:

- current bounded SAM sessions match the pinned adopted shape;
- zero, active, removal, socket, sub-session, overflow, and secret-exclusion behavior is correct;
- M014 live metrics/logs and explicit unavailable RouterInfo behavior remain intact;
- atomic fencing and connection saturation remain correct;
- final reviewed head is current;
- reviewer independence is auditable;
- zero unresolved high/medium findings;
- no scope expansion.

## 10. Milestone status

| Milestone | Status | Plan | Current disposition |
|---|---|---|---|
| 001–004 | historical closed | existing plans | retained foundations |
| 005–007 | superseded | existing plans | superseded |
| 008–009 | historical closed | existing plans | retained |
| 010–012 | historical corrective records | existing plans | residual behavior materially corrected by M014 and `9047fee` |
| 013 | superseded | `013-production-conformance-and-independent-reclosure.md` | superseded |
| 014 | closed | `014-spec-constrained-truthfulness-and-local-hardening.md` | corrective work accepted by M017 final-head review |
| 015 | strict closure invalidated | `015-focused-independent-reclosure.md` | historical; superseded by M017 |
| 016 | closed | `016-sam-fencing-and-connection-proof-corrective-pass.md` | implementation frozen at `355e243`; accepted by M017 |
| 017 | closed | `017-final-head-independent-reclosure.md` | final reviewed head `dbbd107`; zero unresolved high/medium findings |
