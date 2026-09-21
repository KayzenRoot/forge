# M00-S07 — Configuration System

Status: PLANNED / NOT IMPLEMENTED
Module: M00 Forge Kernel & Contract Runtime
Depends on: S01-S06

## Mission
Provide typed, versioned, deterministic and auditable configuration for Forge while keeping secrets separate, supporting standalone operation and preventing configuration drift from silently changing active executions.

## Core laws
1. Configuration is data governed by contracts, not arbitrary environment-string lookup.
2. Secrets are references/handles, not ordinary configuration values.
3. Every active execution binds to an immutable configuration snapshot.
4. Precedence is explicit and explainable.
5. Invalid configuration fails before affected capability readiness.
6. Reload is transactional and scope-aware.
7. Semantic config changes participate in fingerprints, causality and test invalidation.

## Configuration layers
Lowest to highest precedence:
1. compiled safe defaults;
2. Forge installation/profile defaults;
3. project configuration;
4. environment profile;
5. local machine/user overrides;
6. explicit process/CLI overrides;
7. Work Order/execution-scoped overrides when contract permits.

Environment variables are an input adapter, not the canonical model.

## Proprietary technologies

### Typed Configuration Fabric (TCF)
Contract-backed configuration registry. Every key belongs to an owner/module, has type/schema, default policy, mutability, sensitivity, validation, scope and semantic-impact metadata.

### Config Provenance Map (CPM)
For every resolved value, records which layer supplied it, what it overrode and why. Enables an operator to ask "why is this value 8?" without reverse-engineering files/env vars.

### Immutable Config Capsule (ICC)
Canonical, fingerprinted configuration snapshot bound to an execution/Work Order. Mid-flight reload creates a new capsule and never mutates the old execution invisibly.

### Semantic Config Fingerprint (SCF)
Hashes only behaviorally relevant normalized configuration, excluding secrets and non-semantic metadata. Integrates with S06 Change Cone and Green Proof reuse.

### Transactional Reload Gate (TRG)
Stages -> validates -> impact-analyzes -> prepares affected modules -> atomically publishes or rolls back. No partial reload state.

### Config Blast-Radius Classifier (CBRC)
Keys declare impact classes such as LIVE_SAFE, NEW_EXECUTIONS_ONLY, MODULE_RESTART, KERNEL_RESTART, MIGRATION_REQUIRED, SECURITY_CRITICAL. S06 verifies downstream impact.

### Secret Reference Membrane (SRM)
Config stores references such as secret://provider/key rather than secret bytes. Resolution occurs only at authorized use boundary. Secret value never enters config fingerprint, diagnostic dump or provenance map.

### Drift Sentinel
Compares desired config fingerprint with active runtime capsules and reports intentional/stale/unauthorized drift without automatically mutating active work.

## Canonical representation
Human-facing project configuration should be readable and reviewable. Candidate formats: TOML for native Forge/project config, with JSON/YAML adapters where ecosystem interoperability requires them. Final format/library is benchmark/ergonomics driven. Internally, validated typed values are canonical.

## Ownership
Every key has one authoritative owner. Global namespace dumping is forbidden. Suggested hierarchy:
forge.kernel.*
forge.runtime.*
forge.security.*
module.<module_id>.*
integration.hive.*
integration.core.*
integration.iris.*
provider.<provider_id>.*

## Schema metadata
Each key/schema declares:
- type and constraints;
- owner;
- default/required semantics;
- scope;
- mutability/reload class;
- sensitivity;
- semantic/non-semantic impact;
- deprecation/version;
- validation rules;
- permission required to override;
- documentation.

## Secret providers
Standalone baseline: OS-supported/local secure secret mechanism or explicitly protected local provider.
Optional adapters may include CI/CD secret stores and cloud vaults later.
Forge config must operate when remote vaults are unavailable; capabilities requiring unavailable secrets become explicit DEGRADED/UNAVAILABLE rather than kernel failure unless the secret is mandatory for a core local invariant.

## Reload flow
CHANGE DETECTED
-> parse candidate
-> schema validate
-> resolve layers
-> resolve non-secret references metadata
-> semantic diff
-> S06 impact cone
-> permission/security gate
-> module prepare/quiesce if required
-> publish new ICC
-> verify health
-> commit
or
-> rollback previous ICC + evidence.

Security-critical changes may require restart/approval and cannot use ordinary live reload.

## Reproducibility
Evidence records ICC fingerprint, config schema versions and safe provenance, not secret values. Replaying an execution requires compatible config semantics and separately authorized secret availability.

## Performance
- parse/merge only on config change, not per command;
- immutable snapshots for lock-light reads;
- precompiled validators;
- intern stable key IDs;
- incremental semantic diff;
- no secret-provider call on ordinary non-secret config reads;
- filesystem watchers are debounced/coalesced;
- benchmark startup parse, snapshot lookup, large config merge and reload.

## Security
- secrets redacted by type, not string heuristics;
- env vars considered untrusted input;
- override permissions enforced;
- path/URL/file values normalized and validated;
- config cannot silently enable broader network/exec/filesystem authority;
- security-relevant downgrade is explicit and evidenced;
- config export/support bundles use safe views.

## Integration with S03/S04/S05/S06
S03 supplies config contracts.
S04 capability descriptors reference relevant config requirements.
S05 resolution binds to ICC fingerprint and cannot route around privacy/security config.
S06 receives semantic config deltas and computes blast radius.

## HIVE/Core/IRIS
Integration config is adapter-owned and contract-versioned. HIVE may index safe configuration documentation/fingerprints but not secret material. Core/IRIS configuration remains separated behind their adapters; Forge does not import private mutable config state.

## Test strategy
- precedence/property tests;
- deterministic snapshot/fingerprint tests;
- semantic vs non-semantic change tests;
- secret non-leak tests;
- malformed/env injection tests;
- override permission tests;
- transactional reload rollback;
- concurrent readers during reload;
- module restart/reload classification;
- drift detection;
- cross-platform path/config behavior;
- large configuration benchmarks;
- outage of optional secret provider;
- migration/deprecation tests.

## Acceptance criteria
S07 is accepted when Forge configuration is typed, owner-scoped, layered, immutable per execution, provenance-explainable, secret-safe, transactionally reloadable and integrated with causality/evidence without requiring external services.
