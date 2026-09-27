# M00-S15 — Plugin & Adapter SDK

Status: CANDIDATE IMPLEMENTATION PRESENT / NOT CERTIFIED
Module: M00 Forge Kernel & Contract Runtime
Depends on: S01-S14

## Mission
Provide a stable, capability-oriented extension boundary that lets Forge integrate ecosystem systems, executors, tools and future plugins without exposing kernel internals or granting arbitrary in-process authority.

## Extension classes
1. NATIVE_MODULE — trusted Forge code shipped/certified with the product.
2. FIRST_PARTY_ADAPTER — Forge-maintained bridge to HIVE/Core/IRIS/Git/GitHub/etc.
3. LOCAL_WORKER — separate local process communicating through versioned IPC.
4. MCP/SKILL_ADAPTER — tool/capability projection through explicit schemas.
5. THIRD_PARTY_PLUGIN — untrusted/restricted extension requiring stronger isolation.
6. REMOTE_PROVIDER — network service behind an adapter contract.

Class determines trust, permissions, isolation and certification requirements.

## Core laws
- Kernel private APIs are never plugin APIs.
- Extensions export capabilities through S03 contracts/S04 registry.
- Registration does not grant permission.
- Untrusted third-party code is not dynamically loaded into the trusted kernel by default.
- Extension failure cannot corrupt kernel authority.
- Protocol/SDK compatibility is versioned and negotiable.
- Every side effect is declared and permission-bounded.
- Forge remains bootable with every optional adapter/plugin absent.

## Proprietary technologies

### Forge Extension Protocol (FEP)
Protocol-neutral contract defining extension identity, capability manifest, lifecycle, permissions, resource budgets, health, commands/events and evidence.

### Adapter Capability Manifest (ACM)
Signed/provenance-bound manifest describing exported/required capabilities, FCF contracts, permissions, resources, network/filesystem/process access, side-effect classes and compatibility.

### Permission Lease Sandbox (PLS)
Extensions receive scoped, revocable capability/permission leases rather than ambient process authority. Leases bind operation, project, duration and resource policy.

### Isolation Tier Matrix (ITM)
Selects execution boundary from trust/risk:
T0 trusted native module
T1 first-party restricted in-process adapter
T2 isolated local worker/process
T3 OS/container/sandbox isolated worker
T4 remote service
Risk can force a stronger tier; performance cannot silently weaken isolation.

### Adapter ABI Firewall (AAF)
Stable wire/contract boundary prevents Rust/internal ABI/private crate layouts from becoming ecosystem compatibility promises.

### Capability Translation Layer (CTL)
Maps external APIs/protocols into FCF capabilities/errors/events without leaking provider-specific semantics through the Forge core.

### Extension Conformance Kit (ECK)
Generated/curated contract, lifecycle, permission, fault, cancellation, resource and compatibility tests required before an adapter/plugin is promoted.

### Extension Trust Attestation (ETA)
Evidence bundle linking extension package identity, provenance/signature, manifest fingerprint, SDK/protocol version, conformance result and allowed trust tier.

### Hot Adapter Swap Gate (HASG)
Allows provider/adapter replacement only after compatibility, state, in-flight work and capability snapshot checks. Otherwise requires quiesce/restart.

## SDK surface
The SDK exposes only stable primitives:
- contract/capability registration;
- command handling/client calls;
- event publish/subscribe;
- config schema/access;
- state namespace/port access where authorized;
- cancellation/deadline context;
- resource lease requests;
- telemetry/evidence APIs;
- health/readiness;
- secret-reference resolution handles when authorized.
No raw access to kernel globals/internal DB/private scheduler.

## IPC/wire candidates
For isolated local workers, evaluate:
- local sockets/named pipes with framed messages;
- Protobuf/gRPC where streaming/interop benefits justify it;
- compact custom framing only if benchmark evidence warrants complexity.
Windows and Linux parity is required. Transport is replaceable beneath FEP.

## Package/manifest identity
Extension package identity includes:
- extension_id/version;
- publisher/provenance;
- package/content fingerprint;
- FEP/SDK compatibility range;
- ACM fingerprint;
- supported platforms/architectures;
- entrypoint;
- required Forge capability ranges;
- optional capabilities;
- migration/state ownership declarations;
- signature/attestation metadata when applicable.

## Discovery/install
M00 defines primitives only; marketplace UX belongs M22.
Discovery sources are explicit local directories/registries/config. Discovery never auto-executes code. Install/stage -> verify -> conformance -> permission review -> activate.

## Permission model
Granular capabilities include filesystem scopes, network destinations/classes, process execution, secrets, state namespaces, Git/GitHub operations, GPU, external side effects and privileged commands. Default deny for undeclared authority.

## Resource isolation
Every extension declares resource class/budget. S13 enforces concurrency/queue limits. Isolated workers can receive process-level CPU/memory limits where platform support permits. Misbehaving extensions can be throttled/quarantined.

## Failure/isolation
Crash/hang/protocol violation/contract violation maps through S11. Repeated violations trigger quarantine/circuit isolation. Worker restart uses bounded policy. Kernel does not retry irreversible side effects merely because adapter process died.

## Versioning
FEP has explicit protocol versions and capability negotiation. Minor SDK convenience APIs do not automatically alter wire contract. Deprecation includes compatibility window and migration evidence. Version skew is tested.

## HIVE/Core/IRIS
Each is a first-party adapter/provider:
- HIVE: context/memory/retrieval/project/checkpoint capabilities.
- Core/Hades: orchestration/agent/workflow capabilities.
- IRIS: creative asset/job/provenance capabilities.
Adapters use public/versioned contracts only. Private repo internals are not dependencies.

## MCP/Skills
MCP servers and Skills map through CTL. Tool schemas become FCF projections. MCP transport failure cannot compromise kernel lifecycle. Tool permissions and data egress are explicit.

## Executors
Codex/Hive Coder/CLI/IDE/local workers become executor adapters in later M04. S15 provides the extension substrate but does not embed executor-specific orchestration in M00.

## Performance
- typed in-process fast path for trusted native/first-party cases;
- zero/low-copy handles for large immutable artifacts where safe;
- connection/process pooling for isolated workers;
- batch calls where contracts allow;
- lazy extension activation;
- immutable manifest/capability snapshots;
- benchmark in-process vs IPC overhead before assigning default isolation tiers;
- security tier cannot be downgraded solely for benchmark gains.

## Security/supply chain
- provenance/signature/attestation verification according to trust class;
- package fingerprint pinning;
- no auto-update/execute of unverified plugin code;
- path traversal/archive bomb checks;
- dependency/SBOM hooks for M08;
- secret least privilege;
- network egress allowlists/policy;
- protocol fuzzing;
- confused-deputy protections: extension authority cannot exceed caller + lease policy.

## Test strategy
- FEP version negotiation;
- manifest canonicalization/fingerprint;
- permission denial/escalation attempts;
- lifecycle/crash/hang;
- cancellation/deadline propagation;
- resource exhaustion;
- protocol malformed/fuzz tests;
- compatibility/version skew;
- Windows named-pipe/local IPC and Linux socket parity;
- quarantine/recovery;
- extension install without execution;
- HIVE/Core/IRIS absent/reconnect;
- MCP/Skill malformed schemas;
- performance/IPC overhead benchmarks;
- confused-deputy and secret-leak tests.

## Anti-pattern gates
Forbidden:
- arbitrary dylib/plugin loading into kernel from untrusted sources;
- plugin access to private kernel structs/globals;
- ambient filesystem/network/secret authority;
- provider-specific types leaking across core contracts;
- auto-executing discovered extensions;
- silent permission expansion on update;
- infinite worker restart;
- transport choice becoming architectural source of truth.

## Acceptance criteria
S15 is accepted when Forge can extend through a stable capability/contract protocol, isolate by trust/risk, grant revocable least-privilege authority, certify compatibility and remain fully operational with optional extensions absent or failing.
