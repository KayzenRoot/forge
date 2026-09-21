# Forge Construction Model

## Version strategy
Forge is designed as one complete product version. There is no MVP product track. Internal increments, module checkpoints and releases exist to make construction safe; they do not redefine the approved full-product scope.

## Vertical module loop
For each module:
1. Discover the whole module and its sections.
2. Identify upstream/downstream contracts and freeze the public boundary.
3. Record ADRs, threats, performance budgets, test strategy and migration needs.
4. Admit one or more bounded Work Orders that together implement the complete module.
5. Build through an executor branch.
6. Collect deterministic tests, security, benchmark and Evidence Bundle.
7. Review. Fix SELF_HEALABLE defects directly with available ChatGPT/GitHub tools.
8. Send only EXECUTOR_REQUIRED deltas to Codex/Coder/CLI/IDE, automatically as PDF.
9. Re-audit exact head.
10. Promote the module checkpoint and freeze its public contract.
11. Only then begin deep planning of the next module.

## Anti-rework architecture
- Contract-first module boundaries.
- Dependency direction enforced by module graph.
- No hidden cross-module imports.
- Stable IDs for contracts, Work Orders, evidence and decisions.
- Compatibility tests for every public contract.
- Migration path for breaking changes.
- Ports/adapters around HIVE, Core/Hades, IRIS, GitHub, models, IDEs and infrastructure.
- Feature/capability negotiation instead of version guessing.
- Modular monolith first; process/service extraction only from measured need.
- Event schemas versioned and replay-safe.
- Persistent state owned by exactly one module.
- Every module exposes health, telemetry and evidence hooks.

## Hybrid interface model
The kernel is protocol-neutral. MCP is the preferred interoperable agent/tool surface; Skills package reusable workflows/capabilities; typed APIs/events are used for durable service integration; CLI/IDE adapters support local executors. No single transport is allowed to become the product architecture.

## Review and continuation
Every requested review executes the following automatically:
- inspect exact repository/PR state;
- classify defects SELF_HEALABLE or EXECUTOR_REQUIRED;
- correct SELF_HEALABLE defects here when safe and objectively verifiable;
- re-run/inspect available checks;
- if EXECUTOR_REQUIRED remains, produce only the correction prompt as downloadable PDF;
- if no executor correction remains, produce the next project execution prompt as downloadable PDF when the next step requires an external executor;
- never ask the user to repeat this workflow.

## Innovation rule
Each module planning cycle must explicitly investigate proprietary technology opportunities in productivity, quality, testing, security, performance, caching, token economy and automation. Proposed innovation must include purpose, measurable hypothesis, fallback and risk. Novelty never overrides correctness.

## STOP CONDITION
Do not begin implementation of a module until its complete planning package and Work Order(s) are admitted. Do not deeply plan the following module until the current module is completed and promoted, except for boundary/interface stubs needed to avoid rework.
