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

## Agent skills

### Matt Pocock skill invocation

When the `mattpocock-skills` plugin is available, automatically use a model-invoked skill whose description matches the current task; these skills do not require the user to type a slash command. Use `/ask-matt` when skill routing is unclear. User-invoked workflows remain user-led. Do not force-fit a skill or let it override the user's request, this repository's governance/security rules, or deterministic evidence.

Prefer deterministic Git, text search, AST, hashes, tests, lint, typecheck, and build checks whenever they are sufficient. Use Jev only when available and useful for bounded context relevance, triage, classification, reranking, claim verification, comparison, or explicit-gate decisions. Jev output is advisory; never send it secrets or credentials.

### Issue tracker

Issues and specs for this repository live in GitHub Issues. Use the `gh` CLI and follow `docs/agents/issue-tracker.md`.

### Triage labels

Use `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, and `wontfix` for the five canonical triage roles. Follow `docs/agents/triage-labels.md`.

### Domain docs

This repository uses a single-context domain layout. Read relevant glossary and ADR material before domain changes; follow the actual paths recorded in `docs/agents/domain.md`.
