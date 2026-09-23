# M00-S20 — Resource Governor

Status: CANDIDATE IMPLEMENTATION PRESENT / NOT CERTIFIED
Module: M00 Forge Kernel & Contract Runtime
Depends on: S01-S19

## Mission
Give Forge explicit authority over finite machine, provider and economic resources so every execution has bounded consumption, attributable ownership and predictable behavior under pressure.

## Boundary with S13
S13 answers: how much work may enter/run concurrently and how backpressure flows.
S20 answers: what resources each admitted execution may reserve/consume, who owns them, when they expire/revoke and how budgets are accounted.

## Resource dimensions
- CPU time/share;
- RAM/working set;
- GPU compute;
- VRAM;
- disk capacity;
- disk I/O;
- network bandwidth/connections;
- processes/threads/tasks;
- file/socket/OS handles;
- external API quota/rate;
- model tokens/context;
- monetary/cost budget;
- wall/compute time;
- artifact/cache quota.

Not every platform can hard-enforce every dimension; enforcement capability is explicit.

## Resource modes
HARD_LIMIT — enforceable ceiling.
SOFT_LIMIT — pressure threshold with throttling/degradation.
RESERVATION — guaranteed capacity held for owned work.
QUOTA — cumulative budget over a scope/window.
OBSERVED_ONLY — measurable but not safely enforceable on current platform.
UNKNOWN — measurement/enforcement unavailable; conservative policy applies.

## Proprietary technologies

### Forge Resource Governor (FRG)
Canonical resource policy/accounting authority coordinating S01 leases, S13 scheduling and module/extension execution.

### Resource Lease Protocol (RLP)
Versioned lease describing resource vector, owner, scope, expiry, priority class, revocation policy and evidence identity. A lease grants bounded consumption, not unrestricted access.

### Multi-Dimensional Resource Budget (MDRB)
Budget vector spanning machine/provider/token/cost dimensions. Child work can receive delegated sub-budgets whose total cannot exceed parent authority.

### Survival Reserve (SR)
Protected capacity for kernel supervision, cancellation, recovery, state flush, evidence and shutdown. Ordinary workloads cannot consume this reserve.

### Resource Pressure Gradient (RPG)
Normalizes heterogeneous pressure signals into dimension-specific states: GREEN, ELEVATED, HIGH, CRITICAL, UNKNOWN. It preserves raw evidence and does not collapse all resources into one misleading scalar.

### Lease Revocation Protocol (LRP)
Safe sequence for shrinking/revoking resources: stop child admission, signal cancellation/throttle, reach safe points, release capacity, escalate isolation if noncompliant.

### Resource Borrowing Market (RBM)
Within deterministic policy, idle soft/reserved capacity may be temporarily borrowed by eligible workloads. Borrowed capacity is preemptible and never includes Survival Reserve or security-isolated quotas.

### Cost & Token Budget Wallet (CTBW)
Hierarchical non-financial accounting primitive for allowed model/API/token/monetary spend per project/work-order/command. It is a budget ledger, not a payment wallet.

### Resource Efficiency Proof (REP)
Evidence comparing useful output against resources consumed for a workload/profile, supporting later M19 optimization without rewarding unsafe under-provisioning.

## Budget hierarchy
SYSTEM
-> PROJECT
-> WORK_ORDER
-> COMMAND/EXECUTION
-> CHILD OPERATION

Delegation conserves authority. A child cannot mint CPU/token/cost budget beyond its ancestors. Explicit policy may grant a separate root budget.

## Admission/reservation
Before S13 admits expensive work, FRG can reserve required minimum resources. Admission distinguishes:
- guaranteed minimum;
- desired target;
- burst maximum;
- borrowed/preemptible capacity.
If minimum cannot be satisfied within deadline/policy, reject/defer instead of starting doomed work.

## CPU
Use OS/runtime mechanisms where available plus cooperative accounting. CPU-heavy pools receive quotas/shares. CPU time and saturation are observed; platform-specific hard controls remain adapters.

## Memory
Memory budgets include expected working set and bounded burst. Large immutable data should use CAS/handles/mmap/streaming where appropriate. HIGH/CRITICAL pressure reduces admission and triggers reclaim before OOM.

## GPU/VRAM
Device capability, VRAM reservation/estimate, concurrency and exclusivity are represented explicitly. Jobs exceeding safe VRAM budget are rejected/rerouted/degraded before predictable OOM. Device-specific enforcement is adapter-driven.

## Disk
Separate capacity from IOPS/throughput pressure. Cache/artifact quotas are reclaimable; canonical state/evidence protection has higher priority. Disk-full reserve is kept for state/evidence/recovery metadata where feasible.

## Network
Budgets can cover concurrency, bandwidth, destinations/classes and provider rate quotas. Security egress policy remains authoritative; resource availability never grants destination permission.

## Tokens/cost
AI/provider commands declare estimated minimum/target/max token and cost budgets. S05 may choose cheaper provider/model/cached/deterministic path. Budget exhaustion yields explicit policy outcome, never silent truncation unless contract allows bounded partial result.

## Processes/handles
Plugins/workers receive bounded process/task/handle budgets. Fork/process storms and descriptor exhaustion trigger throttling/quarantine.

## Resource estimation
Estimates come from:
1. declared contract defaults;
2. historical HPR/REP fingerprints;
3. hardware/workload profile;
4. conservative fallback.
Adaptive estimates are bounded and versioned. Unknown estimates do not become zero-cost.

## Revocation/preemption
Preemption allowed only for contracts declaring safe cancellation/checkpoint behavior. Irreversible/high-assurance effects are not killed mid-commit merely to reclaim ordinary capacity; policy reserves enough capacity to reach a safe boundary.

## Pressure response
GREEN -> normal.
ELEVATED -> stop expansion/borrowing.
HIGH -> reduce concurrency, reclaim cache, pause background, preempt borrowable work.
CRITICAL -> OSM, preserve Survival Reserve, reject expensive work, cancel safe low-priority work, prioritize recovery.
UNKNOWN -> conservative ceilings; no adaptive expansion.

## Accounting
Every meaningful consumption record links to project/work-order/execution/capability/provider. S18 TCAL consumes token/cost data. Accounting can be sampled for very cheap resources, but budget enforcement counters remain authoritative where required.

## Standalone/platform portability
FRG works locally with conservative software budgets even where OS hard limits are unavailable. Windows/Linux adapters expose actual enforceable features. Missing cgroups/job-object/GPU controls are capabilities, not hidden assumptions.

## Security
Resource leases do not imply data/permission authority. Quotas are scoped to principals/projects. Untrusted extensions cannot raise their own limits. Resource metadata avoids secret leakage. Denial-of-service patterns feed S11/S15 quarantine.

## Test strategy
- parent/child budget conservation;
- lease expiry/revocation;
- borrow/preempt behavior;
- Survival Reserve protection;
- memory/disk pressure;
- GPU VRAM admission;
- token/cost exhaustion;
- process/handle storm;
- provider quota/rate exhaustion;
- UNKNOWN telemetry conservative fallback;
- cancellation during revocation;
- high-assurance safe-boundary protection;
- multi-project noisy-neighbor;
- Windows/Linux enforcement capability matrix;
- accounting/REP accuracy;
- sustained pressure/soak benchmarks.

## Metrics
- reserved/used/borrowed resources;
- budget exhaustion/rejection;
- reclaim/preemption;
- Survival Reserve consumption;
- estimate vs actual error;
- useful work/resource;
- tokens/cost per successful outcome;
- pressure state duration;
- resource-related failures.

## Anti-pattern gates
Forbidden:
- child budgets exceeding parent authority;
- relying on OOM/disk-full as normal admission control;
- consuming Survival Reserve for ordinary work;
- treating observed-only resource as hard-enforced;
- resource lease granting security permission;
- killing irreversible effects at unsafe boundaries;
- unlimited plugin/process/token budgets;
- adaptive expansion under UNKNOWN resource telemetry;
- optimizing token/cost by silently degrading required correctness.

## Acceptance criteria
S20 is accepted when Forge can reserve, delegate, account, borrow, revoke and protect multi-dimensional resources with explicit enforcement capability, survival headroom and hierarchical token/cost budgets, while coordinating with S13 without conflating scheduling with resource ownership.
