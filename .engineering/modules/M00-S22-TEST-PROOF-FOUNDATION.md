# M00-S22 — Test-Proof Foundation

Status: CANDIDATE IMPLEMENTATION PRESENT / NOT CERTIFIED
Module: M00 Forge Kernel & Contract Runtime
Depends on: S01-S21

## Mission
Establish the proof system that lets Forge demonstrate correctness proportionally to change/risk, preserve still-valid evidence, select the smallest sound verification set and escalate automatically when dependency knowledge or confidence is insufficient.

## Core laws
1. Tests are evidence producers, not the definition of correctness.
2. Every governed change has proof obligations derived from contracts, risk and impacted semantics.
3. A green test is reusable only while its proof dependencies remain compatible.
4. Unknown impact broadens verification.
5. High-assurance work requires stronger independent proof.
6. Passing tests cannot override violated contracts/security/integrity invariants.
7. Full suites remain certification tools, not mandatory after every small edit.
8. Evidence must identify the exact source/config/toolchain/environment it proves.

## Proof classes
STATIC
CONTRACT
UNIT
PROPERTY
INTEGRATION
STATE/MIGRATION
CONCURRENCY
FAULT/RECOVERY
SECURITY
COMPATIBILITY
PERFORMANCE
REPLAY/DETERMINISM
OFFLINE/SOVEREIGN
END_TO_END
MANUAL/POLICY

A proof obligation may require several classes.

## Proprietary technologies

### Forge Proof Graph (FPG)
Typed graph linking requirements/invariants/contracts/risks to source, capabilities, tests, benchmarks, simulations and evidence. A green node proves only the obligations/edges declared and validated for it.

### Proof Obligation Compiler (POC)
Compiles a change + S06 Change Cone + contract/risk/side-effect metadata into machine-readable proof obligations before validation begins.

### Minimal Sound Proof Set (MSPS)
Computes the smallest known evidence/test set that covers all current obligations under confidence/risk policy. "Minimal" is subordinate to "sound"; uncertainty expands the set.

### Proof Preservation Engine (PPE)
Determines which existing proofs survive a semantic change by comparing dependency fingerprints, proof assumptions and environment/toolchain state.

### Green Proof Cache (GPC)
S14-backed store for preserved successful proofs. It stores proof metadata/evidence, not merely a boolean test result.

### Proof Confidence Lattice (PCL)
Classifies evidence confidence from DIRECT_STRONG through DERIVED/CONDITIONAL to UNKNOWN, considering coverage quality, provenance, freshness and independence. It is not a product-quality score.

### Counterexample Vault (CEV)
Content-addressed corpus of minimized failing inputs, race schedules, fuzz/property counterexamples, crash points and regression fixtures. Every confirmed defect should try to leave behind a reusable counterexample.

### Failure Neighborhood Explorer (FNE)
When a validation fails, expands around the failure using causal/source/test graph relationships and executes a bounded neighborhood before jumping immediately to the entire suite.

### Proof Gap Detector (PGD)
Detects requirements/contracts/risks with no adequate evidence producer. A module cannot be certified while mandatory proof gaps remain.

### Adversarial Proof Synthesizer (APS)
Generates boundary/property/fault/concurrency/security scenarios from FCF contracts and invariants. Deterministic generation is preferred; LLMs may propose candidates but generated proofs must be executable and independently evaluated.

### Verification Economics Planner (VEP)
Optimizes validation order by expected information gain, runtime, resource cost and risk: cheap/high-signal proofs first, expensive broad suites later when necessary. It cannot omit mandatory proof classes.

### Proof Freshness Horizon (PFH)
Defines when evidence becomes stale because of source, environment, dependency, threat model, benchmark baseline or time-sensitive external changes.

### Certification Evidence Capsule (CEC)
Compact immutable bundle of obligations, executed/preserved proofs, fingerprints, gaps, failures, benchmarks and verdict used by module/release gates.

## Proof obligation sources
- Requirements/acceptance criteria;
- FCF contracts/invariants;
- architecture rules;
- side-effect/risk class;
- security/threat model;
- state/migration ownership;
- compatibility dimensions;
- performance/resource budgets;
- deterministic/replay contracts;
- sovereign/offline requirements;
- defect counterexamples;
- Definition of Done.

## Change-to-proof pipeline
CHANGE
-> semantic diff / Change Cone
-> identify affected obligations
-> compile POC
-> query FPG/GPC
-> preserve compatible proofs via PPE
-> detect gaps
-> compute MSPS
-> order with VEP
-> execute
-> on failure use FNE/CEV
-> update proofs/fingerprints
-> PGD
-> CEC
-> verdict.

## Risk profiles
LOW:
targeted static/unit/contract proofs where applicable.

STANDARD:
targeted + integration/property/state/security as impacted.

ELEVATED:
broader independent verification, fault/recovery/performance where relevant.

HIGH_ASSURANCE:
strongest applicable proof set, explicit negative/adversarial tests, side-effect simulation/reconciliation, security/integrity checks and certification gate. Proof reuse requires higher confidence/independence.

Risk never reduces mandatory proof obligations.

## Test selection
Selection uses S06 graph edges, semantic diffs, contract ownership, historical failures, runtime coverage where trustworthy and counterexamples. File proximity alone is insufficient. Unknown/low-confidence graph edges broaden selection.

## Property/invariant testing
Core invariants become executable properties where practical:
- budget conservation;
- no permission amplification;
- no duplicate unsafe effect;
- bounded queues;
- lifecycle legality;
- compatibility directionality;
- cancellation lineage;
- state crash consistency;
- cache correctness with empty cache equivalence.

## Concurrency/model testing
Use deterministic/model schedulers where practical for state machines, cancellation, queues and idempotency. Production concurrency need not be globally serialized. Race counterexamples enter CEV.

## Fault injection
Inject crashes/timeouts/provider loss/disk pressure/corrupt cache/partial writes/network isolation/adapter hangs at declared fault points. Fault tests prove recovery/degradation contracts rather than merely generating chaos.

## Security proof
Security checks include static/dependency/secret/permission/trust-boundary/adversarial contract tests as relevant. M08 later expands this; M00 establishes proof identities and gates.

## Performance proof
Benchmarks bind hardware/workload/toolchain profile and statistical method. A functional pass cannot prove performance. Baseline/regression thresholds are separate evidence obligations.

## Sovereign proof
S21 ZDRP scenarios are mandatory M00 certification obligations. They execute with enforced network isolation and ecosystem providers unavailable.

## Deterministic replay proof
S19 DEE/RCAP allows exact/semantic replay assertions. Replays never bypass SERF.

## Test hermeticity
Tests declare external dependencies, time/RNG/filesystem/environment needs and fixture fingerprints. Hidden network/current-time/shared-state dependencies are proof defects.

## Flaky tests
Flakiness is not solved by infinite retry. Repetition can characterize nondeterminism, but a flaky mandatory proof remains a gap until isolated/fixed or explicitly replaced by stronger evidence.

## Mutation testing
Selective mutation testing may validate whether important tests actually detect contract/invariant violations. It is targeted by risk/hotspot, not necessarily run repository-wide each iteration.

## LLM role
LLMs may:
- propose tests/properties;
- summarize failures;
- suggest likely proof gaps;
- generate candidate fixtures.
LLMs do NOT decide that unexecuted code is proven correct. Final machine-verifiable proof/evidence remains deterministic or independently evaluated.

## Evidence identity
Every proof records:
- proof/test ID + version;
- obligation IDs;
- source/contract/config/state/graph/toolchain/environment fingerprints;
- inputs/fixture/counterexample IDs;
- execution/replay envelope;
- result;
- duration/resources;
- coverage/assumption metadata;
- evidence artifact fingerprints.

## Module certification
M00 cannot be COMPLETE until mandatory obligations have a CEC with no unresolved required PGD gaps, exact-head evidence and the module DoD. Later modules inherit the same mechanism.

## Test economics
The goal is minimum total engineering cost at required confidence:
- preserve valid green proofs;
- run cheap/high-signal checks first;
- rerun failure neighborhood;
- avoid unrelated full suites during iteration;
- broaden when graph confidence is weak;
- full/module certification at gates;
- record time/resources/tests avoided and any later regression cost.

## Integration
S03 contracts define invariants.
S06 supplies causal impact.
S08 stores evidence/counterexamples.
S11 fingerprints failures.
S12/S13/S20 expose concurrency/resource invariants.
S14 stores proofs.
S16 compatibility produces obligations.
S18 attributes validation cost/hotspots.
S19 enables replay.
S21 supplies sovereign certification.

M06 later builds the full Test Intelligence & Verification Engine on this foundation; M07 consumes proof gaps/evidence for review/self-healing.

## Test strategy for the proof system itself
- obligation compiler golden/property tests;
- graph proof-preservation soundness;
- hidden dependency conservative invalidation;
- MSPS coverage correctness;
- risk escalation;
- counterexample minimization/reuse;
- FNE bounded expansion;
- flaky proof handling;
- mutation sensitivity;
- evidence fingerprint stability;
- proof cache poisoning/corruption;
- environment/toolchain invalidation;
- offline ZDRP integration;
- high-assurance reuse restrictions;
- large proof graph scalability;
- compare selected proof set against seeded regression corpus to estimate false-negative risk.

## Metrics
- proof obligations covered/gapped;
- proofs preserved vs rerun;
- validation time/resource/token cost;
- counterexamples reused;
- regressions caught by proof class;
- false-preservation incidents target zero;
- graph confidence/invalidation breadth;
- flaky proof rate;
- certification duration;
- tests/time avoided with later regression cost.

## Anti-pattern gates
Forbidden:
- "tests passed" as sole correctness claim;
- reusing a green result without dependency fingerprints;
- file-name-only test selection;
- infinite flaky-test retry until green;
- LLM assertion as proof;
- performance/security inferred from unit tests;
- skipping mandatory proof because it is expensive;
- full suite after every edit by default;
- declaring module complete with unresolved mandatory proof gaps.

## Acceptance criteria
S22 is accepted when Forge can compile change/risk into explicit proof obligations, preserve still-valid evidence, compute a minimal sound proof set, escalate under uncertainty, retain counterexamples, expose proof gaps and produce a certification evidence capsule proving the exact M00 head against its Definition of Done.

## C03 exact-change and backend receipt gate

`ChangeAssessment::from_git` obtains `git rev-parse --show-object-format`, resolves base and HEAD
as real commit objects, requires the supplied exact head to be current HEAD and the base to be its
ancestor, and derives paths from the exact Git diff. The subprocess has fixed arguments, no shell,
no lazy fetch, a 15-second timeout, bounded output, a 4,096-path limit, and strict path validation.
SHA-1 repositories require 40-hex commit IDs and SHA-256 repositories require 64-hex IDs.
Callers cannot construct an assessment with an omitted path list. An empty range, missing source
fingerprint, absent graph, incomplete graph, missing seed, or low-confidence edge broadens the
obligation set.

A `Passed` node can be inserted only with a trusted backend receipt that binds proof ID/kind,
artifact fingerprint, Git object format and exact head, change-set fingerprint, dependencies,
obligations, input/environment fingerprints, backend run, and executor/reviewer session IDs.
Review nodes require distinct session IDs. Token syntax is only structural validation: the host
backend must resolve those IDs to real read-only dispatch sessions before signing. A role label,
directory, path, or agent display name is not a session ID. Rejected or stale bindings never certify.
