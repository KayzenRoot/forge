# FGE-003 Supplemental Architecture Decisions

## ADR-F003-06 — Sovereign Standalone Mode
Status: PROPOSED FOR FGE-003 ACCEPTANCE
Decision: HIVE, Core/Hades and IRIS enhance Forge but are not mandatory for the universal engineering path. CORE capabilities require native/fallback implementations. Integrations degrade explicitly and safely.

## ADR-F003-07 — One Primary Construction Prompt Per Module
Status: PROPOSED FOR FGE-003 ACCEPTANCE
Decision: After full module planning, one executor prompt constructs the complete module. Internal phases are allowed without user re-prompting. Later prompts for that module are correction deltas only.

## ADR-F003-08 — Proof-Carrying Green Test Reuse
Status: PROPOSED FOR FGE-003 ACCEPTANCE
Decision: Green tests are cached by source/input dependency, environment/toolchain, config and fixture fingerprints. Iteration uses sound impact-selected tests; uncertainty escalates scope. Full certification occurs at defined gates rather than after every edit.
