# Forge Construction Model

## Version strategy
Forge is designed as one complete product version. There is no MVP product track. Internal increments, module checkpoints and releases exist to make construction safe; they do not redefine the approved full-product scope.

## Sovereign standalone principle
Forge MUST remain operational when HIVE, Core/Hades or IRIS are unavailable. Ecosystem systems are accelerators and capability providers, never mandatory runtime dependencies for the universal engineering path.
- Native Forge fallback exists for every capability classified CORE.
- Integrations use ports/adapters, timeouts, circuit breakers and capability negotiation.
- No boot-time hard dependency on HIVE/Core/IRIS.
- Integration loss produces an explicit DEGRADED capability state, not fabricated success.
- Forge may reuse ecosystem technologies through versioned libraries/packages/protocols when their contracts permit it, without requiring the originating service to be online.
- Integration-specific features may become unavailable, but repository intake, planning, source engineering, targeted tests, review, build evidence and local delivery remain usable.
- Standalone and integrated modes must have contract tests and failure-injection tests.

## One-prompt module execution law
After a module's complete planning package is approved, generate ONE primary executor prompt for the ENTIRE module. It may be long. It must include all sections, implementation, migrations, docs, tests, benchmarks, evidence and STOP CONDITION needed to construct that module.
The primary prompt may internally sequence phases/checkpoints for safety, but it must not require a new user prompt between normal phases. Subsequent executor prompts are CORRECTION DELTAS only. The next primary construction prompt is for the next module after current-module certification.

## Vertical module loop
1. Discover the whole module and its sections.
2. Identify upstream/downstream contracts and freeze the public boundary.
3. Record ADRs, threats, performance budgets, test strategy and migration needs.
4. Admit the complete-module Work Order and compile one primary executor prompt.
5. Build the complete module through the executor, with internal safe phases.
6. Use impact-targeted validation while iterating; collect deterministic evidence.
7. Run the module certification suite at the defined certification gate.
8. Review. Fix SELF_HEALABLE defects directly with available ChatGPT/GitHub tools.
9. Send only EXECUTOR_REQUIRED correction deltas to Codex/Coder/CLI/IDE, automatically as PDF.
10. Re-audit exact head.
11. Promote the module checkpoint and freeze its public contract.
12. Only then begin deep planning of the next module.

## Anti-rework architecture
Contract-first module boundaries; dependency direction enforced by graph; no hidden cross-module imports; stable IDs; compatibility tests; migration paths; ports/adapters; capability negotiation; modular monolith first; replay-safe versioned events; single-module persistent-state ownership; health/telemetry/evidence hooks.

## Test economics: green-test reuse
Forge MUST NOT rerun the complete test estate after every edit.
- Every test result is keyed by test identity/version, source/input dependency fingerprint, environment/toolchain fingerprint, relevant configuration and fixture fingerprint.
- A green result is reusable only while all dependencies in its proof key remain unchanged.
- Changed files/symbols/contracts select the smallest sound impacted test set using dependency/coverage history and risk.
- Newly failing tests trigger the failure neighborhood first, not the whole suite.
- Fix iterations rerun failed tests plus directly impacted tests.
- Escalate to broader suites when public contracts, dependency graph, migrations, security boundaries, shared fixtures/toolchain/configuration change, impact analysis confidence is low, or risk policy requires it.
- Full/module certification suites run at module completion, release/security gates and scheduled confidence calibration, not on every patch.
- Cache invalidation is fail-safe: uncertainty invalidates reuse.
- Flaky tests are quarantined only by explicit policy/evidence; they are never silently treated as green.
- Persist evidence explaining why tests were executed or reused.

Planned proprietary technologies: Predictive Test Selection, Green Proof Cache, Test Impact Graph, Failure Fingerprints, Test Budget Optimizer, Confidence Escalation Gate.

## Hybrid interface model
The kernel is protocol-neutral. MCP is the preferred interoperable agent/tool surface; Skills package reusable workflows/capabilities; typed APIs/events provide durable integration; CLI/IDE adapters support local executors. No single transport or peer system becomes the product architecture.

## Review and continuation
Every review automatically inspects exact state; classifies SELF_HEALABLE/EXECUTOR_REQUIRED; fixes safe SELF_HEALABLE defects here; rechecks available evidence; emits a correction PDF only when executor work remains; otherwise emits the next execution PDF when external execution is required. The user does not need to repeat this workflow.

## Innovation rule
Every module planning cycle investigates proprietary opportunities in productivity, quality, testing, security, performance, caching, token economy and automation. Each proposal requires purpose, measurable hypothesis, fallback and risk. Novelty never overrides correctness.

## STOP CONDITION
Do not implement a module until its complete planning package and complete-module Work Order are admitted. Do not deeply plan the following module until the current module is completed/promoted, except narrow interface stubs required to freeze current contracts.
