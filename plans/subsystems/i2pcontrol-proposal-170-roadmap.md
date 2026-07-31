# I2PControl Proposal 170 Roadmap

Status: active narrow corrective work

Current corrective planning baseline: `43088a42881a76b3936c76f6e7eb8a51262504c4`

Canonical references:

- `plans/000-long-term-specification.md`
- `plans/001-terminology-and-domain-model.md`
- `plans/002-long-term-roadmap.md`
- `plans/003-planning-process.md`
- `plans/adrs/ADR-0001-proposal-170-contract-and-stub-boundary.md`
- `docs/i2pcontrol/proposal-170-conformance.md`
- I2P Proposal 170 and the existing I2PControl JSON-RPC contract

## 1. Purpose and boundary

This subsystem owns the exact Proposal 170 I2PControl API contract for Emissary:

- authenticated JSON-RPC over the independent TLS listener;
- RouterInfo selectors;
- AddressBook administrative operations;
- TunnelManager configuration and explicit unsupported backends;
- ClientServicesInfo;
- only the bounded read-only observation and local resource controls required by those methods.

It does not own router algorithms, transport behavior, NetDB behavior, tunnel construction, peer selection, frontend behavior, runtime address resolution, missing tunnel data planes, release automation, or general CI/security policy.

## 2. Canonical invariants

- Proposal 170 wire names, methods, selectors, actions, tunnel types, JSON types, and existing error envelopes remain exact.
- A value described as current comes from current canonical state.
- Missing or failing state is not disguised as zero, false, empty, absent, stale, or default success unless the adopted contract explicitly defines that result.
- Existing Emissary sources are reused; shadow metrics, logs, caches, stores, and lifecycle registries are not introduced.
- Core observation, if unavoidable, is one bounded read-only secret-free seam that does not change runtime behavior.
- Unsupported tunnel runtimes remain explicit inactive backends.
- TLS is required before JSON-RPC.
- I2PControl connection and request work remains locally bounded.
- Existing hardened code outside the I2PControl boundary is not broadly reopened.
- No CI, release, publishing, platform-matrix, coverage, nightly, or generated-evidence machinery is added.
- Repository-wide formatting is not part of Proposal 170 corrective work.

## 3. Non-goals

- implementing missing client/server tunnel data planes;
- adding BOB;
- migrating startup-managed tunnels into Proposal lifecycle ownership;
- adopting Proposal administrative address books for runtime resolution;
- frontend management surfaces;
- new Proposal extensions, aliases, pagination, statuses, or partial-result envelopes;
- router, transport, NetDB, tunnel-pool, peer-selection, congestion, cryptographic, or general security redesign;
- a generic SAM observer, service registry, task supervisor, connection manager, metrics system, logging system, or verification framework;
- broad revalidation of unrelated code.

## 4. Historical disposition

M001–M004 established the contract and administrative foundations.

M005–M007 were superseded after production truthfulness and closure defects were found.

M008–M013 implemented the first corrective sequence. M008’s production composition and the principal TLS-serving correction materially improved the implementation, but M013 closure was later invalidated.

M014 then corrected stale RouterInfo state, live metrics/log wiring, cross-category generation ownership, and the pre-spawn connection implementation. M015 declared strict closure at frozen head `e59fa239eb03ea608d3b8123a36fc2fa72fcf675`.

A subsequent review found three remaining issues and a final-head closure defect:

1. active SAM session state was treated as successful empty output without pinned adopted-contract authority;
2. same-category generation validation and entry replacement were separated by independently released locks;
3. the connection-limit test opened 10 requests against a limit of 128 and did not prove saturation;
4. M015 reviewed an older head and did not reconcile later broad formatting and test-gating commits.

M015 remains historical evidence but is not the current closure authority.

## 5. Current corrective findings

| Finding | Severity | Owner | Required correction |
|---|---|---|---|
| SAM unavailable state can appear as successful empty active-session map | medium, contract-dependent | M016 | pin official Proposal and adopted i2pd behavior; exact bounded map, permitted existing error, or blocker |
| same-category service handle can race with replacement between generation check and entry write | medium | M016 | one atomic fixed-registry state transaction |
| connection saturation test remains below production limit | medium evidence defect | M016 | small test-only limit and real over-limit rejection/restoration regression |
| M015 names an older reviewed head and independence/scope evidence is insufficient | closure defect | M017 | auditable independent final-head review |
| broad formatting commit after M015 was outside the frozen review | scope reconciliation | M017 | whitespace/semantic classification without absorbing unrelated code into Proposal 170 |

No unrelated finding is activated by this table.

## 6. Corrective sequence

```text
M016 SAM, fencing, and connection-proof corrective pass
    |
    v
M017 Final-head independent reclosure
```

Only M016 is ready. M017 remains blocked until M016 lands on a frozen implementation head and the registry records an auditable independent reviewer.

This is the complete corrective sequence. If M017 finds a defect that fits M016, return it to M016 rather than creating another milestone.

## 7. Milestone 016 — SAM, fencing, and connection-proof corrective pass

Plan:

- `plans/implementation/i2pcontrol-proposal-170/016-sam-fencing-and-connection-proof-corrective-pass.md`

Objective:

- establish exact adopted SAM session semantics from official Proposal 170 and pinned i2pd source;
- implement only the exact safe bounded SAM path or stop with a contract blocker;
- make same-category registry fencing atomic using one small state lock;
- prove the existing pre-spawn connection bound with deterministic over-limit behavior;
- update only directly affected docs and planning state.

Primary production files:

- `emissary-cli/src/i2pcontrol/client_services.rs`
- `emissary-cli/src/i2pcontrol/service_registry.rs`
- `emissary-cli/src/i2pcontrol/server.rs`

Permitted exception:

- one minimal bounded read-only accessor on the canonical SAM owner only if the pinned adopted contract requires exact active-session entries.

Exit conditions:

- all M016 acceptance criteria pass;
- exact SAM disposition is recorded;
- registry check/write is atomic;
- test exceeds its configured connection limit and observes rejection/restoration;
- targeted local checks pass;
- no unrelated formatting, security, CI, release, or architectural change;
- frozen implementation head recorded;
- M016 moves to `closing` and M017 becomes `ready`.

## 8. Milestone 017 — Final-head independent reclosure

Plan:

- `plans/implementation/i2pcontrol-proposal-170/017-final-head-independent-reclosure.md`

Objective:

Independently review the actual final head, the M016 diff, and the limited post-M015 scope/evidence issues without reopening the rest of the repository.

Required review:

- pinned SAM contract and implementation;
- atomic same-category fencing;
- real connection saturation and restoration;
- M014 live metric/log and unavailable-path spot checks;
- feature-gated required tests under `--features i2pcontrol`;
- mechanical reconciliation of the post-M015 broad formatting commit;
- auditable reviewer independence;
- final-head registry/documentation claims.

Exit conditions:

- reviewer distinct from the M016 implementation executor and identified by a distinct agent/run or equivalent auditable record;
- final reviewed head explicitly named;
- zero unresolved high/medium finding;
- no required evidence replaced by a static or tautological check;
- no CI/release/broad security scope;
- accepted `plans/closure/i2pcontrol-proposal-170/017-closure.md` before roadmap/registry return to `closed`.

## 9. File-scope policy

### Primary M016 production boundary

- `emissary-cli/src/i2pcontrol/client_services.rs`
- `emissary-cli/src/i2pcontrol/service_registry.rs`
- `emissary-cli/src/i2pcontrol/server.rs`
- smallest existing I2PControl trait/DTO file required by the exact SAM map

### Tests and docs

- existing client-services and adversarial suites;
- directly affected I2PControl docs;
- Proposal 170 plans, registry, and closure record.

### Core exception

Only one small bounded read-only canonical SAM session snapshot may touch core. It must not change SAM protocol behavior, listener ownership, session lifecycle, persistence, task ownership, or security policy.

If exact SAM support requires more, stop with a blocker.

### Prohibited systems

- `.github/workflows/**`;
- CI/nightly/platform/coverage/evidence machinery;
- release or publishing automation;
- frontend code;
- unrelated core/router/transport/NetDB/tunnel/security code;
- project-wide formatting;
- general supervision or observation frameworks.

## 10. Verification policy

Verification is local, targeted, and proportional:

```bash
cargo fmt --all -- --check
cargo check -p emissary-cli --no-default-features --features i2pcontrol
cargo test -p emissary-cli --no-default-features --features i2pcontrol
cargo clippy -p emissary-cli --no-default-features --features i2pcontrol --all-targets -- -D warnings
```

Run equivalent core commands only if the permitted SAM accessor is added.

Do not add these commands to GitHub Actions as part of this workstream.

Not required:

- full unrelated workspace verification;
- UI builds;
- multi-platform or nightly CI;
- fuzz campaigns;
- network farms;
- coverage thresholds;
- release dry runs;
- generated evidence bundles;
- repeated verification on an unchanged frozen head.

## 11. Completion definition

The workstream returns to `closed` only after M017 confirms:

- exact Proposal 170 and adopted ClientServicesInfo behavior;
- truthful bounded SAM session output or an authoritatively permitted existing error;
- no unavailable SAM source disguised as empty success;
- atomic same-category service ownership fencing;
- actual over-limit connection rejection and capacity restoration;
- live metrics/log wiring and explicit unavailable RouterInfo paths remain intact;
- required tests execute under the I2PControl feature;
- final reviewed head is current;
- reviewer independence is auditable;
- no router behavior, frontend, resolver, CI, release, formatting, or broad security scope entered;
- zero unresolved high/medium findings.

The completion statement must continue to distinguish the implemented Proposal 170 administrative API from explicit unsupported tunnel data-plane backends.

## 12. Milestone status

| Milestone | Status | Plan | Current disposition |
|---|---|---|---|
| 001–004 | historical closed | existing plans | retained foundations |
| 005–007 | superseded | existing plans | superseded by corrective sequence |
| 008–009 | historical closed | existing plans | retained |
| 010–012 | historical corrective records | existing plans | M014 materially corrected residual behavior; M016 owns only listed remaining defects |
| 013 | superseded | `013-production-conformance-and-independent-reclosure.md` | superseded by later gates |
| 014 | corrective pass required | `014-spec-constrained-truthfulness-and-local-hardening.md` | materially landed; M016 owns three residual findings |
| 015 | strict closure invalidated | `015-focused-independent-reclosure.md` | historical closure record; superseded by M017 |
| 016 | ready | `016-sam-fencing-and-connection-proof-corrective-pass.md` | only active implementation handoff |
| 017 | blocked | `017-final-head-independent-reclosure.md` | blocked on frozen M016 head and auditable independent reviewer |
