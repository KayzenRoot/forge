# FGE-003 Architecture Decisions

## ADR-F003-01 — Hybrid integration surface
Status: PROPOSED FOR FGE-003 ACCEPTANCE
Decision: Use a protocol-neutral modular kernel, MCP for interoperable agent/tool access, Skills for reusable capability packaging, typed APIs/events for durable system integration and adapters for CLI/IDE executors.

## ADR-F003-02 — Vertical module construction
Status: PROPOSED FOR FGE-003 ACCEPTANCE
Decision: Plan a module completely, freeze its public contracts, construct and certify it, then plan the next module deeply.

## ADR-F003-03 — One complete product scope
Status: PROPOSED FOR FGE-003 ACCEPTANCE
Decision: Forge has no MVP product scope. Engineering increments and module checkpoints remain mandatory for safety and evidence.

## ADR-F003-04 — Contract-first anti-rework boundary
Status: PROPOSED FOR FGE-003 ACCEPTANCE
Decision: Later modules consume frozen public contracts. Breaking changes require ADR, impact analysis, compatibility/migration plan and contract tests.

## ADR-F003-05 — Internal ecosystem role
Status: PROPOSED FOR FGE-003 ACCEPTANCE
Decision: Forge is an internal NextLabs engineering factory synchronized with HIVE, Core/Hades and IRIS; it is not designed around public SaaS multi-tenancy or billing unless later explicitly promoted.
