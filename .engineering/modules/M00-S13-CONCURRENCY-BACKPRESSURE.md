# M00-S13 — Concurrency, Scheduling & Backpressure

Status: CANDIDATE IMPLEMENTATION PRESENT / NOT CERTIFIED
Module: M00 Forge Kernel & Contract Runtime
Depends on: S01-S12

## Mission
Maximize useful throughput and predictable latency while preventing CPU, memory, GPU, I/O, network, provider and queue saturation from cascading into system failure.

## Core laws
1. Concurrency is budgeted per bottleneck, not globally maximized.
2. Every production queue is bounded.
3. Admission control happens before expensive work.
4. Backpressure propagates toward producers instead of becoming hidden memory growth.
5. Fairness prevents one project/provider/workload from monopolizing shared resources.
6. Priority cannot bypass security, resource or cancellation policy.
7. Overload must degrade explicitly and recover automatically.

## Work classes
CPU_BOUND
BLOCKING_IO
ASYNC_IO
DISK_HEAVY
NETWORK
GPU
MEMORY_HEAVY
EXTERNAL_PROVIDER
LATENCY_CRITICAL
BACKGROUND
HIGH_ASSURANCE

A command may declare multiple resource dimensions. Scheduling uses the dominant/combined bottleneck profile.

## Proprietary technologies

### Forge Flow Governor (FFG)
Unified admission/backpressure control plane that converts S01 Resource Lease Protocol budgets into executable concurrency limits and queue policy.

### Multi-Resource Admission Vector (MRAV)
Represents a task as a vector of expected CPU, RAM, I/O, GPU/VRAM, network, provider quota, tokens/cost and deadline pressure. Admission considers the vector rather than a single semaphore.

### Adaptive Concurrency Window (ACW)
Per lane/provider dynamic concurrency window adjusted from measured latency, saturation, error rate and queue delay within deterministic min/max policy. Falls back to conservative fixed limits when evidence is insufficient.

### Fair-Share Work Scheduler (FWS)
Weighted fair scheduling across project/work-order/tenant classes with starvation protection. Interactive/critical work may receive bounded priority without permanently starving background work.

### Backpressure Propagation Graph (BPG)
Maps saturation from resource/provider lanes back through command/event producers so upstream work can slow, coalesce, defer or reject before memory queues explode.

### Overload Survival Mode (OSM)
Explicit degraded operating mode triggered by resource pressure. Protects kernel/lifecycle/recovery lanes, sheds/coalesces eligible low-priority work, pauses background jobs and reserves recovery headroom.

### Queue Deadline Filter (QDF)
Rejects or reroutes queued work whose remaining deadline can no longer accommodate estimated minimum service time. Avoids spending resources on already-doomed tasks.

### Concurrency Proof Profile (CPP)
Evidence snapshot recording limits, resource observations, queue behavior and benchmark results used to justify a concurrency configuration for a hardware/workload class.

## Lane architecture
At minimum:
- KERNEL_CRITICAL
- RECOVERY
- INTERACTIVE
- NORMAL
- CPU
- BLOCKING_IO
- DISK
- NETWORK
- GPU
- EXTERNAL_PROVIDER:<id>
- BACKGROUND
- TELEMETRY

Physical runtime pools may serve multiple logical lanes where benchmarks prove safe. Logical isolation remains explicit.

## Admission
Before queueing expensive work:
1. validate lifecycle/cancellation/deadline;
2. estimate declared/measured resource vector;
3. inspect lane capacity and resource leases;
4. apply fairness/priority;
5. ADMIT, DEFER, COALESCE, ROUTE_ALTERNATIVE or REJECT.
No unbounded waiting.

## Backpressure responses
BLOCK_BOUNDED
DEFER
COALESCE
SHED
REJECT
FALLBACK_PROVIDER
REDUCE_PARALLELISM
PAUSE_BACKGROUND
Responses must be contract/policy safe. Side-effecting work cannot be silently dropped.

## Adaptive concurrency
ACW may use AIMD/gradient-like control or another benchmarked controller. Inputs include:
- p50/p95/p99 service/queue latency;
- error/timeout rate;
- CPU/load;
- memory/VRAM pressure;
- I/O wait;
- provider rate-limit signals;
- deadline misses.
The controller cannot exceed hard resource/policy ceilings.

## Memory pressure
Queues have byte/weight budgets in addition to item counts where payload sizes vary. Large payloads may use handles/CAS references rather than duplicated in-memory buffers. OSM reserves memory for shutdown/recovery/evidence.

## CPU
Async runtime workers are not used for long CPU work. CPU-heavy work uses bounded compute lanes/pools. Parallelism considers physical/logical cores and workload measurements, not blindly num_cpus.

## Disk
Disk-heavy operations have separate concurrency and batching policy to avoid random-I/O amplification and SQLite/CAS contention. S08 state durability lanes retain priority over background scans.

## GPU
GPU jobs declare VRAM estimate, device/capability and exclusivity/sharing constraints. Admission prevents predictable OOM. GPU scheduling is a primitive only; domain-specific rendering/AI optimization belongs to IRIS/M16/M19.

## External providers
Each provider has concurrency, rate, token/cost and circuit budgets. 429/rate-limit evidence reduces windows. Retry traffic consumes budget and cannot amplify an outage into a retry storm.

## Priority/fairness
Priority levels are finite and bounded. Aging/starvation prevention promotes waiting eligible work over time. HIGH_ASSURANCE indicates policy rigor, not automatic scheduling priority.

## Load shedding
Only explicitly shed-safe work can be dropped/coalesced. Audit/evidence/durable state commitments are protected. Shedding produces structured evidence/metrics so overload is visible.

## Performance strategy
- bounded lock-light queues/channels;
- per-lane counters/atomic snapshots;
- avoid centralized scheduler lock on every task;
- work stealing only within compatible pools and if fairness remains enforceable;
- batch tiny work where latency budget permits;
- zero/low-copy handles for large immutable payloads;
- cache service-time estimates by operation/resource fingerprint;
- benchmark controller overhead vs fixed semaphores.

## Hardware adaptation
At startup S01 platform probe creates hardware/resource class. Defaults are selected from conservative profiles, then ACW may tune within safe bounds. Profiles are evidence-versioned, not hard-coded folklore.

## Failure behavior
If scheduler/controller fails, Forge falls back to conservative static limits rather than unlimited execution. Resource telemetry loss freezes/reduces adaptive expansion. Queue corruption/invariant failure can prevent affected lane readiness.

## Integration
S01 provides runtime/resource primitives.
S04 exposes hardware/provider capabilities.
S05 can reroute around saturated providers.
S08 supplies durable spill/intent where semantics allow.
S09 uses bounded event lanes.
S10 command admission consumes FFG.
S12 cancellation/deadline trees prune queued/running work.
S18 observes queues/resources.
S19/M19 later deepen performance optimization.

## Test strategy
- bounded queue invariants;
- fairness/starvation property tests;
- deadline queue filtering;
- cancellation while queued/running;
- adaptive window convergence/stability;
- overload survival/recovery;
- CPU saturation;
- disk contention;
- memory pressure/OOM prevention simulations;
- GPU VRAM admission with mocked devices;
- provider 429/outage/retry storm;
- priority inversion;
- multi-project noisy-neighbor;
- 10k/100k queued task synthetic load;
- sustained soak tests;
- scheduler overhead/throughput/p99 benchmarks.

## Metrics
- admitted/deferred/rejected/shed counts;
- queue depth/bytes/age;
- queue/service/end-to-end latency;
- resource utilization/pressure;
- deadline miss rate;
- fairness/starvation indicators;
- adaptive window changes;
- overload-mode duration;
- useful completed work per resource/time;
- retry amplification factor.

## Anti-pattern gates
Forbidden:
- unbounded queues/channels;
- one semaphore for all resources;
- blocking CPU work on async workers;
- priority without starvation control;
- unlimited retries competing with fresh work;
- adapting upward when telemetry is unavailable;
- dropping side effects silently;
- scheduling GPU work without VRAM/resource admission;
- optimizing throughput while p99/deadlines collapse unnoticed.

## Acceptance criteria
S13 is accepted when Forge has bounded multi-resource admission, explicit lanes, fair scheduling, adaptive-but-capped concurrency, deadline-aware queues, overload survival and measurable backpressure that preserves kernel/recovery correctness under saturation.

## C03 bounded-cost rotation

Each lane keeps a FIFO per owner and a FIFO owner-rotation ring. The rotation step is constant time;
owner lookup/removal uses the ordered map and is logarithmic in owner count. Dequeue removes the
map/ring entry only when that owner's queue becomes empty; it does not scan the other owners to
remove an item. The M00 benchmark exercises rotation at 100, 1,000, and 10,000 owners. Durable
resource accounting measures 200 writes at five successive ledger sizes to show whether write cost
changes with history size.
