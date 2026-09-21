# Architecture

## System boundary
Forge is the software/product factory. It consumes governed project intent and produces validated software artifacts.

## Integration principles
- **GEF** governs lifecycle, evidence, gates and promotion.
- **HIVE** provides context, memory and retrieval.
- **Git/GitHub** stores canonical repository truth and review evidence.
- **Core/Hades** and **IRIS** are future peer integrations through explicit contracts, not hidden coupling.

## Rules
1. Ports/adapters around external integrations.
2. Fail-safe degradation when HIVE is unavailable.
3. No direct dependency on mutable conversational state.
4. Contracts are versioned before deep integration.
5. Security boundaries and secrets remain explicit.
6. Prefer modular monolith boundaries initially; split services only with measured justification.
