# Backlog

## NECESSARY
- FGE-001 Bootstrap GEF + HIVE foundation. COMPLETE.
- FGE-002 GitHub governance and operational validation. COMPLETE.
- FGE-003 Master product decomposition and construction model. IN REVIEW.
- M00 Forge Kernel & Contract Runtime. NEXT after FGE-003 promotion.
- M01-M25 follow the canonical order in MASTER-MODULE-INDEX.md, subject only to governed dependency ADRs.

## MODULE EXECUTION LAW
Every module follows PLAN COMPLETE MODULE → FREEZE CONTRACTS → BUILD COMPLETE MODULE → TEST/EVIDENCE → REVIEW/SELF-HEAL → PROMOTE → NEXT MODULE.

Deep planning of M(n+1) waits for completion of M(n), except narrow interface stubs explicitly required to freeze M(n) contracts.

## PRODUCT SCOPE
One complete Forge product. No MVP track. Internal increments/checkpoints exist for engineering safety, evidence and rollback.

See MASTER-MODULE-INDEX.md for M00-M25 and CONSTRUCTION-MODEL.md for the canonical execution loop.

## FGE-004 — M00 Work Order Admission

**Status:** ACTIVE / PLANNING GATE

Admit the complete implementation Work Order for M00 — Forge Kernel & Contract Runtime from the promoted S01-S22 consolidated architecture.

Required outcomes:
- bind the promoted M00 consolidated architecture and canonical source hierarchy;
- freeze implementation scope, out-of-scope, contracts, constraints and build waves;
- map all M00 Definition of Done and proof obligations into acceptance criteria;
- require Sovereign Standalone Mode and ZDRP certification;
- require exact-head evidence and governed review;
- compile one primary executor prompt for the entire M00 module after admission;
- keep product implementation unauthorized until this Work Order is admitted.

STOP CONDITION: M00 implementation Work Order admitted and execution gate explicitly opened.
