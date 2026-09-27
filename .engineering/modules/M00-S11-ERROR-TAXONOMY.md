# M00-S11 — Error Taxonomy & Failure Intelligence Foundation

Status: CANDIDATE IMPLEMENTATION PRESENT / NOT CERTIFIED
Module: M00 Forge Kernel & Contract Runtime
Depends on: S01-S10

## Mission
Define a stable, typed, safe and machine-actionable failure model for Forge so errors can drive retry, recovery, quarantine, self-healing, observability and evidence without parsing human strings.

## Core laws
1. Human message is never the machine contract.
2. Every public failure has a stable error code/class.
3. Retryability is explicit, not inferred from text/status alone.
4. Root cause and user-facing safe detail are separate.
5. Errors preserve causal chains without leaking secrets.
6. Unknown/unclassified failures are first-class and trigger conservative behavior.
7. Error handling cannot silently convert failure into success.

## Error envelope
- error_id: occurrence identity;
- error_code: stable semantic code;
- error_class;
- severity;
- domain/module owner;
- operation/command/capability;
- retryability;
- side-effect/outcome certainty;
- recoverability;
- user-safe message;
- diagnostic-safe metadata;
- causal chain references;
- correlation/causation/work-order IDs;
- config/state/capability/contract fingerprints;
- provider/adapter identity where applicable;
- first/last observed timestamps;
- failure fingerprint;
- evidence references.

## Error classes
VALIDATION
CONFIGURATION
CONTRACT
COMPATIBILITY
AUTHENTICATION
AUTHORIZATION
RESOURCE
CAPACITY
TIMEOUT
CANCELLED
DEPENDENCY
NETWORK
STORAGE
STATE_CONFLICT
MIGRATION
INTEGRITY
SECURITY
PROVIDER
TOOLCHAIN
BUILD
TEST
EXECUTION
UNAVAILABLE
UNKNOWN_OUTCOME
INTERNAL_BUG
CORRUPTION
POLICY

Subcodes refine the class without exploding top-level taxonomy.

## Severity
INFO/EXPECTED — modeled negative outcome, not operational fault.
WARNING — degraded but contained.
ERROR — requested operation failed.
CRITICAL — threatens module/project correctness or availability.
FATAL — kernel cannot safely become/remain READY.

Severity does not determine retryability.

## Retryability
NEVER
IMMEDIATE_SAFE
BACKOFF_SAFE
AFTER_DEPENDENCY_RECOVERY
AFTER_USER_ACTION
AFTER_STATE_RECONCILIATION
UNKNOWN

UNKNOWN is conservative and cannot trigger blind retries.

## Outcome certainty
NOT_STARTED
NO_EFFECT
COMMITTED
ROLLED_BACK
PARTIAL
UNKNOWN_OUTCOME
This field is critical for S10 side-effect safety.

## Proprietary technologies

### Forge Error Genome (FEG)
Canonical machine-readable failure identity combining stable semantic code/class with relevant contract/operation/provider dimensions. Occurrence-specific noise is excluded.

### Failure Fingerprint Engine (FFE)
Produces normalized fingerprints for grouping equivalent failures while removing paths, timestamps, random IDs, secrets and other noise. Multiple fingerprint resolutions support exact vs family grouping.

### Causal Failure Graph (CFG)
Links errors to upstream/downstream command, event, provider, resource and dependency failures. Helps distinguish one root failure from 500 secondary symptoms.

### Retry Safety Matrix (RSM)
Machine policy mapping error class + side-effect class + outcome certainty + idempotency + deadline/resource state to permitted retry/recovery actions.

### Error Knowledge Capsule (EKC)
Compact evidence packet for diagnostics/self-healing: fingerprint, minimal relevant causal graph, source/contract/config fingerprints, recent safe flight-recorder events and prior known resolutions. Designed to avoid dumping whole logs/context into an LLM.

### Failure Novelty Detector (FND)
Compares a normalized failure against known fingerprints/resolutions. Known failures can take deterministic repair paths; novel failures escalate to richer diagnostics/reasoning.

### Error Budget Fuse (EBF)
Tracks repeated failure families within bounded windows and can stop retries/restarts/provider use before cascading resource waste. Integrates with S02 MCI/S09 ESS.

### Resolution Evidence Link (REL)
A resolved failure can link to the exact patch/config/action/test evidence that fixed it. Later recurrence can reuse the proof only when relevant fingerprints remain compatible.

## Stable code convention
Codes are namespaced and semantic, e.g.:
FORGE.CONTRACT.VERSION_INCOMPATIBLE
FORGE.STATE.CANONICAL_INTEGRITY_FAILED
FORGE.COMMAND.UNKNOWN_OUTCOME
FORGE.CAPABILITY.NO_ELIGIBLE_PROVIDER
FORGE.RUNTIME.RESOURCE_LEASE_EXHAUSTED
Numeric compact IDs may be generated internally, but readable stable codes remain canonical.

## Error boundaries
- Internal Rust errors are mapped before crossing public module boundaries.
- External provider errors are preserved as safe source metadata but mapped into Forge taxonomy.
- Panics are INTERNAL_BUG unless a stronger integrity classification applies.
- Validation errors retain field/path info only when safe.
- Multiple errors may aggregate under a parent error without flattening causality.

## Rust implementation principles
Candidate approach:
- typed enums/domain errors internally;
- thiserror-like ergonomics may be evaluated;
- anyhow-like opaque context is acceptable only inside application/private boundaries, never as the public machine contract;
- source chains preserved;
- no panic/unwrap/expect on recoverable production paths unless statically/invariant justified and lint-reviewed.

Exact libraries are implementation-stage choices.

## Privacy/redaction
Fields are classified SAFE, SENSITIVE, SECRET. Secret data is never included. Sensitive values are tokenized/redacted/fingerprinted only where policy permits. Error fingerprints must remain stable after redaction.

## Failure-to-action mapping
An error can recommend machine actions only from an allowlisted action vocabulary:
RETRY, WAIT, REAUTHENTICATE, RECONFIGURE, RECONCILE_STATE, FALLBACK_PROVIDER, QUARANTINE_PROVIDER, RESTART_MODULE, RESTART_KERNEL, ROLLBACK, ESCALATE, STOP.
Recommendation is constrained by RSM and policy; provider text cannot inject actions.

## Self-healing boundary
S11 supplies classification/evidence, not unrestricted repair. M07 later decides whether a defect is SELF_HEALABLE or EXECUTOR_REQUIRED. High-risk/security/state-corruption failures default to escalation, not autonomous patching.

## Observability
Errors emit structured telemetry with stable low-cardinality dimensions. Raw error messages/paths are not metric labels. Fingerprints support grouping. Sampling may apply to repeated noncritical occurrences, but CRITICAL/FATAL/evidence-required failures retain mandatory records.

## Performance
- code/class lookup uses compact IDs;
- fingerprint normalization avoids expensive full-stack serialization on hot expected failures;
- full stack/backtrace captured by policy/severity/build profile;
- repeated failures deduplicated/aggregated;
- causal graph edges reference IDs rather than duplicating envelopes;
- benchmark creation/mapping/fingerprint/grouping overhead.

## Integration
S02 lifecycle consumes severity/recoverability.
S05 provider resolver consumes provider failure evidence.
S08 persists required failure/evidence records.
S09 event failures use retry/quarantine semantics.
S10 UOR/RSM protect side effects.
S18 telemetry consumes structured error envelopes.
M06 failure fingerprints guide targeted test reruns.
M07 self-healing consumes EKC/REL.
M20 reliability consumes CFG/error budgets.

## Test strategy
- stable code serialization/version tests;
- error mapping across module/provider boundaries;
- retry matrix property tests;
- outcome certainty/side-effect combinations;
- secret/sensitive redaction;
- fingerprint stability/noise normalization;
- fingerprint collision/adversarial cases;
- causal graph root-vs-symptom tests;
- repeated-failure fuse behavior;
- unknown error conservative behavior;
- panic boundary tests;
- provider malicious error payloads;
- Windows/Linux path normalization;
- performance benchmarks.

## Anti-pattern gates
Forbidden:
- matching error strings for control flow;
- blanket retry on 5xx/timeout;
- secret-bearing Debug dumps;
- catch-all converting failures to success/default values;
- losing source/correlation identity at boundaries;
- metric label cardinality from raw messages;
- infinite retry/restart loops;
- LLM-generated repair action bypassing deterministic policy.

## Acceptance criteria
S11 is accepted when every Forge failure can be represented with stable semantics, retry/outcome safety, safe causality, fingerprints, evidence and bounded machine actions, while preserving privacy and enabling later test intelligence/self-healing without requiring LLM diagnosis for known failures.
