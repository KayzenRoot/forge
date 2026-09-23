# M00 native runtime candidate

M00 is implemented as a Rust workspace with four crates. The CLI is an offline diagnostic surface, not a project generator or cloud service.

## Local commands

```powershell
cargo run -p forge-cli -- --version
cargo run -p forge-cli -- doctor --data-dir "$env:LOCALAPPDATA\HiveForge\state"
cargo run -p forge-cli -- hash .engineering\CHECKPOINT.json
cargo run -p forge-cli -- contract validate schema.json instance.json
```

`doctor` opens or initializes the local SQLite state root, checks database integrity, records the native boot phase, and reports scoped readiness and capability fingerprints. It reports HIVE as not configured and semantic embeddings as disabled. It does not make network calls or start a model/provider.

The `hash` command accepts regular files up to 256 MiB. Contract validation accepts JSON files up to 1 MiB and rejects external schema references. A valid instance exits zero; an invalid instance prints safe invalid paths and exits nonzero without echoing the submitted values.

## State boundary

Canonical state, command idempotency, the durable event outbox, immutable evidence and the append-only resource-usage ledger live in SQLite under the selected state root. Schema version 2 migrates existing version 1 stores forward. Disposable cache entries use a separate table and may be deleted without changing canonical state. Backups are written to a new directory with a BLAKE3 manifest; restore validates the manifest and database integrity and writes to a new root. See [M00 state recovery](M00-STATE-RECOVERY.md).

`NativeBoot` restores recorded token/cost consumption before issuing new resource leases. Callers must explicitly configure token/cost budgets; they default to zero. Live usage records require a stable 64-hex identity plus project, work-order, execution, capability and optional provider attribution. An exact replay is not charged twice, and conflicting reuse of an identity is rejected. The ledger fingerprint is included in the boot report.

## Optional HIVE context

HIVE is an optional context/retrieval integration. M00 does not depend on it for boot, contract validation or local development diagnostics. Semantic embeddings are disabled. Git and canonical repository files remain the source of project truth.

## Candidate status

The local workspace tests and Windows smoke are candidate evidence. Linux certification, enforced network/DNS isolation on both platforms, security/dependency review, full performance baselines and independent assurance remain separate exact-head gates. M00 is not certified or complete until those gates pass.
