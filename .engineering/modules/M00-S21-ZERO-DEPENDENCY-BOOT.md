# M00-S21 — Zero-Dependency Boot Path

Status: PLANNED / NOT IMPLEMENTED
Module: M00 Forge Kernel & Contract Runtime
Depends on: S01-S20

## Mission
Make Sovereign Standalone Mode a mechanically testable kernel property: Forge must boot, recover, diagnose itself and execute its native minimum engineering path without network, HIVE, Core/Hades, IRIS, cloud, remote database, remote telemetry, LLM or optional plugin/provider availability.

## Definition of zero-dependency
"Zero-dependency" means zero REQUIRED external runtime/service dependencies for kernel readiness. It does not mean zero local libraries/files/OS APIs. Forge ships with the local code, schemas, migrations and native capabilities required for its bootstrap path.

## Core laws
1. No network call is required to determine kernel readiness.
2. Optional integrations are discovered after the native boot-critical path can establish local truth.
3. Missing HIVE/Core/IRIS/Internet/LLM/provider credentials cannot make the native kernel fail boot.
4. Local canonical state recovery never requires a remote control plane.
5. Bootstrap configuration has safe local defaults and no mandatory secret.
6. Boot remains bounded; optional discovery cannot hang readiness forever.
7. The offline path is continuously tested, not a disaster-only code path.

## Native minimum capability set
Before optional integrations, Forge must be able to:
- parse/validate local configuration;
- open/recover local state;
- inspect platform/resource capabilities;
- load native contracts/modules;
- establish capability snapshot;
- provide health/readiness diagnostics;
- perform local repo/filesystem intake within authorized scope;
- execute native deterministic commands required for diagnosis;
- produce local evidence/telemetry;
- shut down/restart safely.

Later modules may extend the native engineering minimum, but cannot weaken this foundation.

## Boot phases
B0 EARLY_BOOT
-> B1 PLATFORM_PROBE
-> B2 SAFE_CONFIG
-> B3 LOCAL_STATE_OPEN/RECOVERY
-> B4 NATIVE_CONTRACTS
-> B5 NATIVE_MODULES
-> B6 NATIVE_CAPABILITIES
-> B7 RESOURCE/SCHEDULER BASELINE
-> B8 LOCAL_HEALTH/READINESS
-> NATIVE_READY
-> B9 OPTIONAL_EXTENSION_DISCOVERY
-> B10 OPTIONAL_INTEGRATION_NEGOTIATION
-> FULL/DEGRADED_READY

Optional B9/B10 cannot retroactively invalidate NATIVE_READY unless they reveal a genuine local safety/integrity problem.

## Proprietary technologies

### Sovereign Boot Capsule (SBC)
Versioned manifest/fingerprint of the exact local artifacts, contracts, migrations, defaults and native modules required to reach NATIVE_READY.

### Zero-Dependency Readiness Proof (ZDRP)
Evidence bundle proving a specific Forge build reached NATIVE_READY with all external integrations/network disabled.

### Boot Dependency Firewall (BDF)
Static/runtime guard that prevents boot-critical crates/modules from acquiring undeclared network/remote-service dependencies.

### Native Capability Seed (NCS)
Compile/package-time seed describing the minimal native capability definitions required before dynamic discovery. Runtime verification turns declarations into S04 evidence.

### Optional Integration Airlock (OIA)
Explicit boundary after NATIVE_READY where HIVE/Core/IRIS/MCP/remote providers/plugins may negotiate. Failure is contained and represented as capability degradation.

### Offline Recovery Console (ORC)
Local CLI/diagnostic surface available when normal adapters/control surfaces are unavailable, capable of health/state/config/evidence inspection and bounded repair/recovery commands.

### Boot Time Budget Ledger (BTBL)
Attributes startup latency to phases and optional discovery, allowing regression gates and preventing one adapter from silently turning startup into minutes.

### Offline Drift Sentinel (ODS)
Detects when a change accidentally introduces a required external lookup/credential/provider into the native boot path.

## Boot dependency graph
Boot-critical dependency graph is explicitly tagged in S06. Nodes are classified:
BOOT_REQUIRED_LOCAL
POST_READY_LOCAL
OPTIONAL_EXTERNAL
DIAGNOSTIC_ONLY
Any path from NATIVE_READY prerequisites to OPTIONAL_EXTERNAL is a policy violation unless the edge is optional/nonblocking and not required for readiness.

## Configuration
S07 safe defaults must allow startup without external credentials. Integration-specific configuration can remain invalid/unconfigured without blocking native readiness. Invalid local safety-critical config still blocks readiness.

## State
S08 local state/migrations are packaged with Forge. Recovery can operate offline. If canonical local state is corrupt beyond safe recovery, Forge may enter NOT_READY/RECOVERY mode, but the reason is local integrity, not absence of cloud services.

## DNS/network isolation
Certification tests run with network disabled/blocked, not merely with integrations unconfigured. DNS/proxy/metadata-service access attempts are treated as failures of the zero-dependency proof.

## Time/certificates/licensing
Native readiness cannot require online time synchronization, certificate validation against remote services or license-server contact. Features with future commercial licensing must degrade through local entitlement/cache/grace mechanisms designed outside M00, never brick recovery tooling.

## Telemetry
S18 local ring/file/snapshot telemetry works without collector. Exporters initialize after airlock. Export failure never blocks native readiness.

## HIVE/Core/IRIS
They enter through OIA:
- HIVE enriches context/memory;
- Core adds orchestration/intelligence;
- IRIS adds creative capabilities.
Each can transition independently from UNAVAILABLE to VERIFIED after boot and can disconnect without destroying native state.

## LLM/model independence
No generative/model call is required for boot, config validation, state recovery, contract validation, health, native capability discovery or shutdown. Decision Plane L0/L1 remains available; higher levels simply become unavailable.

## Packaging
SBC contents are versioned and included in release artifacts. Missing/corrupt boot-critical files are detected locally with actionable S11 errors. Installation does not fetch runtime-critical schemas/migrations from the Internet on first boot.

## Upgrade
An upgrade must prove the candidate SBC can migrate/open local state and reach NATIVE_READY offline before promotion where risk policy requires it. Rollback compatibility follows S16/S08 rules.

## Performance
Boot-critical path minimizes dynamic scanning, remote waits and heavyweight initialization. Lazy-load optional adapters. Use immutable packaged manifests/fingerprints and parallelize independent local probes only when deterministic/resource-safe.

## Security
Offline mode is not insecure mode. Auth/permission/local project boundaries remain enforced. ORC privileged operations require local authorization. Network absence does not bypass policy or signature/integrity checks that can be performed locally.

## Test strategy
- hard network namespace/firewall isolation;
- HIVE/Core/IRIS unavailable;
- no provider credentials;
- DNS blackhole/slow DNS;
- telemetry collector unavailable;
- empty optional plugin directory;
- broken optional adapter;
- local state recovery/migration offline;
- corrupt cache vs corrupt canonical state;
- missing SBC artifact;
- local config error;
- boot timeout/budget;
- ORC diagnostics/recovery;
- upgrade candidate offline boot;
- static boot dependency graph gate;
- runtime unexpected-network-call detector;
- Windows/Linux offline certification.

## Certification scenarios
ZDRP-01 fresh install offline.
ZDRP-02 existing healthy state offline.
ZDRP-03 crash-recovery offline.
ZDRP-04 optional integrations previously configured but unreachable.
ZDRP-05 remote telemetry/provider DNS blackhole.
ZDRP-06 candidate upgrade/migration offline.
All must produce deterministic evidence and bounded completion.

## Anti-pattern gates
Forbidden:
- first-run schema/config download;
- remote feature flag required for kernel boot;
- cloud DB required for local state;
- license server required for recovery;
- synchronous adapter discovery before native readiness;
- telemetry exporter required for readiness;
- hidden DNS/network call in boot-critical library;
- HIVE/Core/IRIS status used as global kernel readiness;
- offline mode bypassing security.

## Acceptance criteria
S21 is accepted when a packaged Forge build can prove NATIVE_READY and execute its native diagnostic/minimum engineering path under enforced network isolation with all ecosystem/remote/model integrations unavailable, producing a Zero-Dependency Readiness Proof on Windows and Linux.
