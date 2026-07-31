# M016 Pre-Amendment Blocker Record — SAM, Fencing, and Connection Proof

Status: superseded as current disposition

Historical frozen implementation head: `9047feecde046dac8e0208bbf1acf2e3883f97ae`

Current implementation authority:

- `plans/implementation/i2pcontrol-proposal-170/016-sam-fencing-and-connection-proof-corrective-pass.md`

## Historical result

This record captured the state before the architecture owner authorized a bounded SAM session-observation handle.

At the historical frozen head:

- atomic same-category service generation fencing passed;
- real TLS connection saturation, rejection, and permit restoration passed;
- the adopted Proposal 170/i2pd active SAM session map remained blocked because the then-current M016 scope prohibited the shared observation seam required to expose it.

Pinned sources used for that conclusion:

- official Proposal 170, `ClientServicesInfo`;
- i2pd commit `7866f644d3d3dea3d1adf5374a6ea378c8efd536`, `daemon/I2PControl.cpp`, `SAMInfoHandler`.

The pinned i2pd behavior serializes active sessions under their session identifiers with the adopted `name`, `address`, and `sockets` fields; each socket contains the adopted `type` and `peer` fields.

## Amendment

The architecture owner subsequently authorized one fixed-capacity, read-only SAM session-observation handle with a private SAM-owned publisher.

The amended plan permits:

- sanitized metadata updates at existing session/socket lifecycle transitions;
- a cloneable read-only handle captured before `SamServer` is moved into the router runtime;
- bounded on-demand snapshots through existing composition into I2PControl;
- exact pinned i2pd fields only;
- explicit failure on overflow rather than partial or empty success.

It still prohibits:

- SAM protocol or lifecycle redesign;
- mutable session, socket, stream, key, destination, payload, authentication, or command-channel exposure;
- generic observer, event, cache, registry, polling, persistence, or supervisor infrastructure;
- unrelated core/security work;
- CI, release, or repository-formatting expansion.

## Current disposition

This file remains historical evidence of why implementation stopped under the earlier scope. It is not an accepted M016 closure and no longer blocks execution.

Current status is:

- M016: `ready` under the amended bounded observation-handle plan;
- M017: `blocked` until amended M016 lands on a frozen head and moves to `closing`;
- M014: `corrective pass required` until the SAM truthfulness finding is resolved and independently reclosed.

After amended M016 lands, create a new closure/evidence record or explicitly append a final implementation disposition. Do not convert this historical blocker into acceptance evidence.