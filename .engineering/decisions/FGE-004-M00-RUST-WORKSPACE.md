# ADR-FGE-004-M00-01 — Rust workspace boundaries and native runtime

Status: ACCEPTED FOR THE M00 CANDIDATE; MODULE CERTIFICATION PENDING
Work Order: FGE-004-M00

## Decision

Implement M00 as a four-crate Rust workspace:

- `forge-contracts` owns contract identities, JSON Schema validation, typed errors and fingerprints.
- `forge-state` owns SQLite migrations, canonical state, idempotency, durable event outbox, evidence and disposable cache storage.
- `forge-kernel` owns the S01-S22 runtime authorities as internal modules. Each authority has one module and public contract surface; service extraction is deferred.
- `forge-cli` is the leaf diagnostics/validation executable and depends on the kernel.

This is a modular monolith. The smaller crate count makes dependency direction enforceable without introducing crates whose only contents would be pass-through facades. The architecture authority map remains the module-level boundary. No crate depends on the CLI, and the contracts/state crates do not depend on the kernel.

## Technology choices

- Pin Rust to 1.98.1 for repeatable local and CI builds.
- Use Tokio for bounded runtime primitives; do not start background providers at boot.
- Use Serde and `jsonschema` 0.57 with default features disabled. Contract compilation rejects external schema references.
- Use SQLx with SQLite for transactional local canonical state, versioned schema migrations and the outbox.
- Use BLAKE3 for domain-separated content/semantic fingerprints and evidence integrity.
- Use `proptest` for bounded property checks. The initial benchmark harness uses the standard library so it adds no production or benchmark dependency.
- Keep dynamic adapters dormant after validation. M00 does not execute third-party binaries or require provider APIs.

## Consequences and limits

The exact-head candidate can build and run locally without HIVE, network services or remote model credentials. This decision does not certify OS-level plugin sandboxing, Windows/Linux network isolation, performance budgets or dependency advisories; those need their own executable evidence and review. Semantic embeddings remain disabled because the Codex Desktop plan does not expose an embedding API credential to this runtime.

## Revisit triggers

Split a crate only when dependency direction, build time, ownership or measured scaling demonstrates a concrete benefit. Replace a candidate technology only with contract-preserving security, portability, maintenance or benchmark evidence.
