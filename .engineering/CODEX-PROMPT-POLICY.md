# Codex / External Executor Prompt Policy

Status: MANDATORY

For every Forge project chat:

1. After every repository review, if external execution is required, the executor prompt MUST be delivered as a downloadable PDF without waiting for the user to ask.
2. Prefer direct SELF_HEALABLE corrections through available ChatGPT/GitHub tools. Use Codex only for EXECUTOR_REQUIRED work, local-machine operations, unavailable administrative GitHub operations, or heavy execution that cannot be safely completed here.
3. Every Codex PDF prompt MUST identify the Work Order, repository, expected base branch/SHA when known, objective, scope, out-of-scope, sources to read, constraints, acceptance criteria, commands/tests, evidence requirements, STOP CONDITION and final report format.
4. Local bootstrap/synchronization prompts MUST require safe repository synchronization before mutation: verify git/gh authentication, inspect current directory, clone when absent, fetch, verify origin, preserve dirty/untracked user work, never reset/clean destructively, and stop on ambiguous/conflicting state.
5. Codex may use gh for repository administration only after verifying authenticated owner/repository and current policy. Never weaken an existing protection silently.
6. The prompt must require GEF/HIVE evidence when applicable and must not claim success without command output/evidence.
7. Reviews return APPROVED / CORRECTION REQUIRED / BLOCKED. Small safe defects are fixed here when possible; only remaining executor-required deltas go back to Codex.
8. This policy applies automatically to future chats and does not require the user to repeat the request.

9. Independent-review backend availability is NOT a pre-implementation blocker unless the admitted Work Order explicitly requires reviewer availability before dispatch. The executor may proceed through implementation, verification, evidence, push and PR creation, but MUST stop before assurance/finalize when no genuinely independent reviewer session/backend is available. Never satisfy independence by renaming the implementer session or self-approving.
10. Missing host tooling such as GitHub CLI must be handled at the latest safe point that needs it. A prompt may install or repair host tooling non-destructively when authorized, but absence of `gh` alone must not prevent local implementation/testing. If authentication cannot be established safely, preserve the branch/evidence and stop before the affected GitHub action rather than fabricating success.

Canonical precedence remains .engineering/SOURCE-HIERARCHY.md and AGENTS.md.
