# Forge Executor Contract

Forge is governed by GEF Bootstrap and integrated with HIVE.

## Authority
Resolve project truth through `.engineering/SOURCE-HIERARCHY.md`. Git, code, tests and exact-head evidence outrank conversational memory. Never silently override an approved ADR or promoted checkpoint.

## Lifecycle
ANALYZE → SOURCE CHECK → NEXT NECESSARY INCREMENT → WORK ORDER → CONTEXT LOCK → PREFLIGHT → EXECUTOR → TESTS/EVIDENCE → PR → AUDIT → APPROVED / CORRECTION REQUIRED / BLOCKED → CHECKPOINT DELTA → MERGE → NEXT.

## Execution
Inspect the repository before mutation. Execute only the admitted Work Order. Prefer deterministic tools, Git, AST, hashes, static analysis and tests before LLM inference. Preserve unrelated work. No force push, history rewrite, destructive cleanup or secret exposure.

## Review self-healing
During review classify defects as SELF_HEALABLE or EXECUTOR_REQUIRED. Small, localized, low-risk defects that can be objectively corrected with available repository tools should be fixed by the reviewer in the same logical increment when safe. Executor-required defects receive only a Correction Delta. Never bypass protected main, tests, evidence, architecture, scope, STOP CONDITION or HIGH/CRITICAL gates.

## External executor prompts
Read and obey `.engineering/CODEX-PROMPT-POLICY.md`. Whenever a review leaves EXECUTOR_REQUIRED work, local-machine work or unavailable GitHub administration, provide the execution prompt automatically as a downloadable PDF. The user must not need to request the PDF again.

## HIVE
HIVE supplies context/memory/retrieval; it does not replace Git authority. HIVE integration must fail safely when unavailable and must not make core repository correctness dependent on an external memory service.

## Completion
A commit, PR or green test alone is not completion. Require the applicable DoD, Evidence Bundle, exact-head audit, approval and checkpoint promotion.
