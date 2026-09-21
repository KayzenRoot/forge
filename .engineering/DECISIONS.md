# Decisions Ledger

## ADR-0001 — GEF governance from inception
Status: ACCEPTED  
Decision: Forge adopts GEF Bootstrap v1.0.0 semantics as its engineering governance baseline.

## ADR-0002 — HIVE as context, Git as truth
Status: ACCEPTED  
Decision: HIVE provides memory/context/retrieval. Git, canonical sources, code, tests and evidence remain authoritative.

## ADR-0003 — No premature microservices
Status: ACCEPTED  
Decision: Prefer modular boundaries and explicit ports/adapters. Service extraction requires measured operational or scaling justification.

## ADR-0004 — Bootstrap before implementation
Status: ACCEPTED  
Decision: Product implementation waits until foundation bootstrap is audited and accepted.

## ADR-0005 — Solo-maintainer review policy
Status: ACCEPTED  
Decision: The `main` ruleset requires zero approving reviews because GitHub rejects a self-approval on
a single-maintainer repository (HTTP 422). Merge is gated by the objective `governance` status check in
strict mode, mandatory conversation resolution, stale-review dismissal and the deletion/non-fast-forward
blocks. Protection was never bypassed and `bypass_actors` stays empty.

## ADR-0006 — Foundation completion is necessary, not sufficient
Status: ACCEPTED  
Decision: Auditing and merging FGE-001/FGE-002 promotes the checkpoint to `FOUNDATION_READY` only.
`productImplementationAuthorized` remains `false` until the FGE-003 Work Order is admitted, because the
canonical process requires capabilities to be admitted by Work Orders and proven by evidence.
