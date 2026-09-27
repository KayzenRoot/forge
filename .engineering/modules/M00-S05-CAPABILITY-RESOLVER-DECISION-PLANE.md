# M00-S05 — Capability Resolver & Forge Decision Plane

Status: CANDIDATE IMPLEMENTATION PRESENT / NOT CERTIFIED
Module: M00 Forge Kernel & Contract Runtime
Depends on: S01-S04

## Mission
Select the safest eligible execution path for a requested capability, then optimize among eligible paths for quality, latency, resource use, monetary/token cost, locality and reliability without making probabilistic AI a hidden authority over hard invariants.

## Resolution pipeline
REQUEST
-> NORMALIZE INTENT/CONSTRAINTS
-> HARD POLICY FILTER
-> CONTRACT/COMPATIBILITY FILTER
-> HEALTH/FRESHNESS FILTER
-> RESOURCE/PRIVACY/SIDE-EFFECT FILTER
-> BUILD ELIGIBLE PROVIDER SET
-> DETERMINISTIC FAST-PATH/CACHE
-> SCORE/PARETO RANK
-> OPTIONAL TYPED DECISION
-> CONFIDENCE/RISK GATE
-> SELECT / ABSTAIN / ESCALATE
-> BIND IMMUTABLE CAPABILITY SNAPSHOT
-> EXECUTE
-> RECORD OUTCOME/FEEDBACK

Hard constraints are never traded for score.

## Resolution request
Contains:
- capability contract/version/features;
- correctness/quality floor;
- determinism requirement;
- privacy/data-egress ceiling;
- side-effect ceiling;
- latency/deadline;
- monetary/token/resource budget;
- locality preference/requirement;
- minimum evidence/freshness;
- risk class;
- fallback/escalation policy;
- reproducibility requirement.

## Proprietary technologies

### Forge Decision Plane (FDP)
Provider-neutral decision architecture combining deterministic rules, cached proof, local typed-decision models, remote typed-decision services and generative reasoning escalation. FDP is a decision substrate, not a single model.

### Constraint-First Resolver (CFR)
Eliminates ineligible providers before optimization. Security, contract compatibility, privacy and correctness floors cannot be outweighed by low latency/cost.

### Multi-Objective Provider Frontier (MOPF)
Maintains the Pareto frontier of eligible providers across quality, latency, reliability, cost/tokens, resource pressure and locality instead of collapsing everything into one fragile magic score too early.

### Decision Escalation Ladder (DEL)
Default order:
L0 deterministic rule/lookup
L1 validated cache/proof reuse
L2 local typed System-1 decision
L3 remote typed System-1 decision if policy allows
L4 compact generative model
L5 strong reasoning model
L6 human/policy approval where required
A level may be skipped when incapable or disallowed. Escalation is driven by uncertainty/risk, not prestige.

### Confidence-Calibrated Abstention (CCA)
Decision providers must be allowed to abstain. Low confidence, out-of-distribution input, calibration drift or policy ambiguity escalates instead of forcing a guess.

### Decision Batch Fusion (DBF)
Multiple independent typed questions over the same immutable state/context can be batched to amortize encoding/network/token overhead. Results retain per-question confidence/provenance.

### Decision Memoization Graph (DMG)
Caches deterministic and probabilistic decisions by semantic request, relevant state/context fingerprint, provider/model/calibration fingerprint and policy version. Any semantic dependency change invalidates the decision.

### Adaptive Utility Router (AUR)
Learns measured provider utility from outcome evidence while staying inside deterministic policy envelopes. It can update preferences, never silently weaken hard constraints.

### Shadow Decision Arena (SDA)
Candidate/new routing strategies or models run in shadow against real/synthetic requests without controlling execution. Compare quality, calibration, latency and cost before promotion.

### Regret & Escalation Ledger (REL)
Records whether cheap decisions later caused retries, failures or expensive escalation. Optimizes end-to-end cost of correctness rather than cheapest first call.

## Jev/Laya strategy
Jev-compatible remote and Laya-compatible local providers are adapters behind a Typed Decision Contract.
Required metadata/evidence:
- supported question/output shapes and cardinality;
- provider/model/version fingerprint;
- confidence semantics and calibration dataset/version;
- latency/resource/cost class;
- privacy/egress;
- maximum input/context characteristics;
- known limitations;
- abstention behavior.

No Jev/Laya-specific API leaks into consumers. They compete with deterministic and other decision providers in benchmarks.

## Decision classes
1. HARD_DETERMINISTIC: security/contract/invariant decisions; AI cannot override.
2. BOUNDED_SEMANTIC: classification/routing/ranking suitable for typed System-1 when calibrated.
3. OPEN_REASONING: architecture/debugging/planning requiring generative reasoning.
4. HIGH_ASSURANCE: money/signing/privileged/irreversible/security-critical; deterministic gates plus explicit high-assurance policy.

## Quality model
Provider quality is capability/task-class specific. A single global model ranking is forbidden. Measurements include accuracy/task success, calibration, retry rate, downstream defects, latency percentiles, tokens/cost, resource use and fallback/escalation frequency.

## Cost model
Optimize expected total cost:
initial execution + expected retry + escalation + defect/rework risk + resource opportunity cost.
A cheap provider that creates repeated failures can rank below a more expensive first-pass provider.

## Token economy
- never invoke an LLM for deterministic work;
- use compact typed outputs where possible;
- reuse decision proofs by fingerprints;
- batch questions sharing state/context;
- progressive disclosure from HIVE rather than full context;
- choose smallest sufficient reasoning tier;
- cache stable prefixes/provider artifacts when supported;
- record tokens avoided as well as tokens spent.

## Resource-aware routing
Resolver consumes S01 resource leases and S04 hardware capabilities. Under local CPU/GPU/RAM pressure it may select a remote provider only if privacy/cost/network policy permits; under network outage it shifts to native/local providers. Resource pressure never relaxes correctness/security floors.

## Failure/fallback
Fallback chains are explicit and bounded. Timeout/contract violation/quarantine may trigger next eligible provider. Non-idempotent side effects require special retry policy and cannot blindly fail over. Circuit state and negative capability cache prevent retry storms.

## Determinism/replay
Every selection emits a Resolution Proof:
- request fingerprint;
- capability snapshot fingerprint;
- policy version;
- eligible/rejected provider IDs with machine reason codes;
- metrics/evidence inputs;
- selected provider;
- decision/escalation provenance;
- confidence where applicable;
- fallback chain;
- resolver version.
Replay mode can reconstruct why a provider was selected from preserved inputs.

## Privacy/security
Prompts/data are classified before remote routing. Secret/regulated/private payloads cannot egress unless policy explicitly allows. Prompt injection or provider text cannot mutate resolver policy. Provider-returned confidence is untrusted until calibrated/validated.

## Performance
- immutable S04 snapshot reads;
- precompiled policy predicates;
- cached eligibility sets;
- incremental Pareto frontier updates;
- single-flight provider probes;
- no network call for deterministic registry resolution;
- typed-decision calls only after cheap deterministic pruning;
- benchmarks for 10/100/1k/10k provider inventories and concurrent requests.

## Test strategy
- property tests: hard constraints never violated;
- deterministic tie-break/replay tests;
- Pareto/frontier correctness;
- privacy/egress rejection;
- cost/deadline/resource constraints;
- stale/quarantined provider exclusion;
- fallback and non-idempotent retry safety;
- calibration/abstention/escalation tests;
- Jev/Laya provider equivalence contract tests;
- decision cache invalidation;
- batch fusion equivalence;
- shadow-routing comparison;
- adversarial provider metadata/output tests;
- benchmark and simulated workload tests.

## Success metrics
Measure rather than promise:
- task success/correctness;
- p50/p95/p99 decision latency;
- end-to-end latency;
- tokens and monetary cost per successful outcome;
- escalation rate;
- retry/rework rate;
- calibration error;
- cache/proof hit rate;
- provider outage survival;
- policy violation count (target zero).

## Acceptance criteria
S05 is accepted when provider selection is constraint-first, reproducible, risk-aware, resource-aware and provider-neutral; Jev/Laya are optional measured adapters; probabilistic decisions can abstain/escalate; and Forge optimizes end-to-end successful work rather than raw token or call cost.
