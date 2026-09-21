# Hive Forge — Master Module Index

Status: FGE-003 planning baseline
Product strategy: one complete product version; no MVP track.

## Construction law
Forge is developed vertically, one module at a time:
DISCOVER MODULE → FREEZE CONTRACTS → WORK ORDER → BUILD COMPLETE MODULE → TEST/EVIDENCE → REVIEW/SELF-HEAL → PROMOTE MODULE CHECKPOINT → NEXT MODULE.

A later module may consume only frozen public contracts of earlier modules. Cross-module shortcuts and hidden imports are forbidden. Contract changes require an ADR, impact analysis and compatibility/migration plan.

## Product operating model
Forge is NextLabs' internal AI-native software factory. HIVE supplies durable context/retrieval, Core/Hades supplies intelligence/orchestration/policy, IRIS supplies media/creative generation, and GEF supplies governed engineering lifecycle. Forge turns governed intent into tested, secure, reproducible, deployable and observable software.

Primary interface model: hybrid.
- Local modular kernel for deterministic execution and policy enforcement.
- MCP servers/clients for interoperable tools and context.
- Skills for versioned reusable capabilities.
- typed APIs/events for durable system-to-system integration.
- CLI/IDE adapters for Codex, Coder and future executors.
- Git/GitHub as canonical engineering truth/evidence.

## Module map

### M00 — Forge Kernel & Contract Runtime
Sections: capability registry; module lifecycle; dependency graph; contract registry; configuration; feature/capability negotiation; event bus; command bus; error taxonomy; idempotency; concurrency; plugin/adapter SDK; compatibility gates; health/readiness; deterministic execution envelope.
Own tech: Forge Contract Fabric (FCF), Capability DNA, Deterministic Execution Envelope.

### M01 — Project Intake, Discovery & Specification Compiler
Sections: project intake; interviewer/discovery; requirements; scope classifier; constraint extraction; ambiguity detector; acceptance-criteria compiler; architecture brief; risk classifier; dependency discovery; change-request intake; specification versioning.
Own tech: Intent-to-Spec Compiler (ISC), Ambiguity Heatmap, Scope Gravity.

### M02 — Context & Knowledge Bridge
Sections: HIVE project binding; context retrieval; context capsules; source hierarchy resolver; semantic/code retrieval; delta context; cache; freshness; provenance; token budgets; offline degradation; memory write-back proposals.
Own tech: Deterministic Context Capsules, Context Delta Mesh, Token Budget Governor.

### M03 — Planning, Work Orders & Build Graph
Sections: module planner; Work Order compiler; task DAG; dependency scheduling; critical path; Context Lock; preflight; STOP CONDITION compiler; checkpoint deltas; change-impact planning; resumability.
Own tech: Work Order Compiler, Build Graph Brain, Context Lock Fingerprints.

### M04 — Agent & Executor Fabric
Sections: Core/Hades orchestration bridge; executor registry; Codex/Coder/CLI/IDE adapters; model routing; role agents; capability negotiation; sandbox policy; parallelism; retries; handoff; tool permissions; cost/token routing.
Own tech: Executor Exchange Protocol, Model Utility Router, Agent Lease/Capability Tokens.

### M05 — Source Engineering & Code Intelligence
Sections: repo mapper; AST/symbol graph; dependency graph; semantic diff; refactoring engine; code generation; patch engine; coding standards; dead-code proof; duplication detection; API compatibility; migrations.
Own tech: Semantic Change Graph, Patch Minimizer, Code Provenance Spine.

### M06 — Test Intelligence & Verification Engine
Sections: unit/integration/e2e/contract/property/fuzz/mutation tests; test generation; test-impact selection; flaky-test detection; coverage risk; deterministic fixtures; parallel scheduling; regression corpus; test evidence.
Own tech: Predictive Test Selection, Failure Fingerprints, Coverage Risk Map, Test Budget Optimizer.

### M07 — Quality, Review & Self-Healing
Sections: automated review; defect classification; self-healable repairs; correction deltas; architecture conformance; complexity; maintainability; regression review; evidence audit; exact-head review; quality gates.
Own tech: Self-Healing PR Loop, Architecture Conformance Engine, Defect Repair Confidence Gate.

### M08 — Security, Supply Chain & Trust
Sections: SAST; SCA; secrets; SBOM; provenance; signing/attestation; dependency policy; container/IaC scanning; threat modeling; Web3 security hooks; least privilege; artifact trust; vulnerability gates.
Own tech: Trust Graph, Risk-Adaptive Gate, Provenance Chain.

### M09 — Build, Packaging & Reproducibility
Sections: language/toolchain detection; build graph; hermetic builds; cache; remote/local execution; containers; artifacts; cross-platform packaging; deterministic/reproducible builds; incremental builds; artifact registry adapters.
Own tech: Reproducible Build Capsules, Build Cache Intelligence, Artifact DNA.

### M10 — Environment, Infrastructure & Deployment
Sections: environment model; IaC; local/dev/preview/staging/prod; container orchestration; secrets injection; deploy strategies; migrations; rollback; drift detection; ephemeral previews; edge/serverless/VPS/cloud adapters.
Own tech: Deployment Intent Compiler, Environment Twin, Safe Rollout Controller.

### M11 — Runtime Observability & Feedback
Sections: logs; metrics; traces; profiles; errors; SLO/SLA; cost telemetry; release health; anomaly detection; incident evidence; runtime-to-code correlation; rollback triggers; production feedback.
Own tech: Runtime Feedback Loop, Telemetry-to-Change Graph, Release Health Sentinel.

### M12 — Release, Versioning & GitHub Factory
Sections: branch/PR automation; semantic versioning; changelog; release notes; tags; release trains; rulesets; evidence bundles; merge automation; release provenance; hotfix flow; rollback records.
Own tech: Evidence Graph, Release Confidence Ledger, Governed Merge Engine.

### M13 — Web & Application Factory
Sections: frontend; backend; API; database; auth; realtime; queues; search; storage; accessibility; SEO; PWA; desktop/mobile wrappers; design-system adapters; performance budgets.
Own tech: App Blueprint Compiler, Full-Stack Contract Mesh.

### M14 — Web3 & Smart Contract Factory
Sections: EVM/chain adapters; contracts; ABI/type generation; local chains; simulations; invariant/fuzz testing; static analysis; upgradeability; wallets/signing boundaries; indexers; transaction simulation; gas optimization; deployment/verification.
Own tech: Transaction Safety Simulator, Contract Invariant Forge, Chain Deployment Ledger.

### M15 — Game & Interactive Software Factory
Sections: Isoryn/Godot adapters; project generation; gameplay code; ECS/domain systems; asset pipelines; scene validation; deterministic simulation tests; performance budgets; builds; patching; multiplayer/backend integration.
Own tech: Game Build Graph, Scene Contract Validator, Simulation Regression Lab.

### M16 — AI/Agent Application Factory
Sections: LLM provider abstraction; prompt/version management; RAG integration; tool/MCP generation; agent graphs; structured outputs; evals; guardrails; caching; model fallback; token/cost controls; AI observability.
Own tech: Prompt ABI, Model Behavior Contract, Eval-Driven Build Gate.

### M17 — IRIS Creative Asset Bridge
Sections: asset requests; generation jobs; character/brand consistency; image/video/audio/3D metadata; licensing/provenance; asset validation; optimization; variants; project binding; delivery into apps/games/web.
Own tech: Creative Asset Contract, Asset Provenance Spine, Consistency Lock.

### M18 — Data, Database & Migration Engineering
Sections: schema design; migrations; fixtures; data contracts; ORM/query adapters; indexing; backup/restore tests; data quality; privacy classes; migration rehearsal; rollback; performance.
Own tech: Schema Evolution Simulator, Data Contract Graph.

### M19 — Performance & Resource Optimization
Sections: profiling; benchmark harness; CPU/GPU/RAM/network/storage budgets; web performance; DB/query optimization; build/test acceleration; LLM token/cost optimization; regression gates; hardware-aware execution.
Own tech: Resource Budget Compiler, Adaptive Execution Planner, Performance Regression Radar.

### M20 — Reliability, Resilience & Recovery
Sections: fault injection; retries/timeouts/circuit breakers; disaster recovery; backup verification; chaos tests; graceful degradation; dependency outage simulation; recovery runbooks; state reconciliation.
Own tech: Failure Scenario Compiler, Recovery Proof Engine.

### M21 — Developer Experience & Unified Control Surface
Sections: CLI; dashboard; project/module status; Work Orders; executor control; logs/evidence; local environment; configuration; approvals; visual dependency graphs; search; notifications; accessibility.
Own tech: Forge Cockpit, Command Palette Protocol.

### M22 — Skills, MCP & Integration Marketplace
Sections: skill format; MCP server/client registry; tool schemas; permission manifests; connector SDK; versioning; compatibility; discovery; installation; sandboxing; internal catalog; third-party adapters.
Own tech: Capability Package Format, Tool Trust Manifest.

### M23 — Enterprise/Internal Governance & Policy
Sections: organization policy; roles; project policies; secret boundaries; audit trail; compliance profiles; retention; licensing; cost policy; environment permissions; exception workflow.
Own tech: Policy-as-Contract, Decision Provenance Ledger.

### M24 — Learning, Pattern Distillation & Continuous Improvement
Sections: successful-pattern mining; failure mining; reusable templates; benchmark history; code/test pattern library; HIVE write-back; Core recommendations; technology scouting; controlled experimentation; deprecation.
Own tech: Cross-Project Pattern Distillation, Engineering Memory Flywheel, Innovation Radar.

### M25 — System Integration, Certification & Long-Term Evolution
Sections: Hive/Core/IRIS end-to-end contracts; compatibility matrix; upgrade protocol; migration tooling; disaster drills; performance certification; security certification; full-system regression; dogfooding; release certification; long-term module evolution.
Own tech: Ecosystem Compatibility Matrix, Forge Certification Protocol.

## Cross-cutting requirements for every module
Security by default; typed/versioned contracts; deterministic evidence; observability; error handling; tests proportional to risk; benchmarks; token/cost accounting where LLMs are involved; cache strategy; Windows/Linux compatibility where applicable; no secret leakage; migration/rollback for persistent state; documentation; HIVE context integration; GEF Work Order/DoD; Core policy/orchestration hooks; IRIS hooks when media is relevant.

## Module completion gate
A module is COMPLETE only when its scope, contracts, implementation, tests, security review, performance baseline, observability, documentation, Evidence Bundle, exact-head audit and checkpoint are approved. The next module may then be planned. No partial module is called complete.

## Ordering principle
M00-M12 establish the universal factory. M13-M18 add production-domain factories. M19-M24 harden and compound the factory. M25 certifies the ecosystem as a whole. Dependencies may be specified early, but implementation proceeds module-by-module unless an ADR proves a narrow prerequisite slice is unavoidable.
