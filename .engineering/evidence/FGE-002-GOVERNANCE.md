# FGE-002 Governance Evidence

Observed 2026-09-21.

## Exact baseline
Main SHA: e8e99c234669e3fab9956a97bb42f3fbd2b8e6f5

GitHub Actions run 35592817052:
- workflow: Repository validation
- event: push
- head SHA: e8e99c234669e3fab9956a97bb42f3fbd2b8e6f5
- conclusion: SUCCESS

## Repository governance inspection
Repository rulesets endpoint returned an empty list. Therefore no repository ruleset protecting main is currently proven.

The connected GitHub integration does not expose an administrative ruleset mutation action. This evidence must not be interpreted as protection being enabled.

## Required target
Main governance should require:
- changes through pull requests;
- required status check: Repository validation;
- review conversation resolution;
- deletion prevention;
- non-fast-forward prevention;
- no routine bypass actors.

## Remaining local proof
GEF source workspace v1.0.0 must be checked out and validated with npm ci + npm run validate.
Forge must be cloned below HIVE_PROJECTS_ROOT, registered in the HIVE Project Registry and inspected as READY.

Until these proofs exist, productImplementationAuthorized remains false.

---

# FGE-002 closure evidence (2026-09-21)

Executed under Work Order FGE-002 on the existing PR #1 branch. Product implementation remains
blocked; nothing in this increment adds Forge product code.

## Authentication and toolchain (tokens not reproduced)

- git 2.55.0.windows.3, gh 2.98.0.
- `gh auth status`: account `KayzenRoot`, active account, https transport, scopes
  `gist, read:org, repo, workflow`. No token value recorded here.
- Repository identity verified before any mutation: `origin` =
  `https://github.com/KayzenRoot/forge.git`, `KayzenRoot/forge`, PUBLIC, default branch `main`.

## Local synchronization

- Local path: `D:\Projetos Codex\forge`.
- The directory was empty before this increment, so no user work existed to preserve. No reset,
  clean, force push or history rewrite was used at any point.
- Base SHA after clone/fetch: `e8e99c234669e3fab9956a97bb42f3fbd2b8e6f5`, equal to `origin/main`
  and equal to the baseline recorded above.
- Working branch: `fge-002-governance-validation`.
- Line endings normalized to LF for the working copy and `core.autocrlf` set to `false` for this
  repository only, matching the convention already used by the sibling project that HIVE reports
  clean. The Windows global Git config ships `core.autocrlf=true`, which made the read-only Linux
  bind mount inside the HIVE API container report every tracked file as modified.

## Prompt-policy delta

- `origin/fge-002-prompt-policy` (`fc0e8947514394d7fabc74442737c332302ef33d`) was confirmed by
  `git merge-base --is-ancestor` to be a strict descendant of the PR head `564a3fd`.
- Incorporated with `git merge --ff-only`, which adds exactly the two inspected commits
  (`365cb23`, `fc0e894`) and rewrites nothing. One logical FGE-002 PR is preserved; no duplicate
  PR was opened and the second branch was not deleted.
- `.engineering/CODEX-PROMPT-POLICY.md` is now present and `AGENTS.md` carries the
  "External executor prompts" section that binds it. Acceptance criterion B is met in-repository.

## GitHub main governance

Ruleset `main-governance`, id `23765135`, `target: branch`, `enforcement: active`,
condition `ref_name.include = ["refs/heads/main"]`, `bypass_actors: []`.

Effective rules read back from the API:

| Rule | Effective configuration |
| --- | --- |
| `deletion` | enabled |
| `non_fast_forward` | enabled |
| `pull_request` | `required_approving_review_count: 1`, `dismiss_stale_reviews_on_push: true`, `require_code_owner_review: false`, `require_last_push_approval: false`, `required_review_thread_resolution: true`, `require_extra_approval_for_unattributed_changes: true`, `allowed_merge_methods: [merge, squash, rebase]` |
| `required_status_checks` | `strict_required_status_checks_policy: true`, contexts `["governance"]` |

Schema notes discovered rather than assumed:

- This API build rejects the object form of `rules` and requires the array form
  (`{type, parameters}`); `integration_id` must be omitted rather than sent as `null`.
- The required status check context is `governance`, the check run name reported by
  `gh pr checks` and by the check-runs API. The suite exposes no workflow-prefixed context, so
  `Repository validation / governance` is the UI rendering, not the stored context.
- The context was verified behaviourally, not by assumption: after activation the already-green
  `governance` run on PR #1 stays `SUCCESS` in the status rollup, so the rule is satisfied by the
  real check rather than left permanently expected.

`require_extra_approval_for_unattributed_changes: true` was applied by GitHub as a default and is
recorded here because it tightens, not weakens, the target policy. No bypass actor was added, so no
administrator recovery exception exists.

## GEF Bootstrap v1.0.0 validation

Validated in a separate throwaway checkout at `D:\Projetos Codex\hive-gef-validation`; GEF source
was not vendored into Forge.

- Tag `v1.0.0` resolves to `866fe3af8cccc65c929aaf6a47a924401fa448b3`, exactly the commit pinned
  in `.engineering/gef/GEF-ADOPTION.md`. No discrepancy.
- `npm ci`: 29 packages added, 58 audited, **0 vulnerabilities**.
- `npm run validate`: exit 0. 1153 tests, 1153 pass, 0 fail, 0 skipped, 0 todo.
- `npm audit --audit-level=high`: exit 0, **0 vulnerabilities**. Nothing suppressed.

## Local HIVE registration

HIVE 1.0.0 verified installed and running: `hive-api` healthy on `127.0.0.1:8000`,
`hive-dashboard` on `127.0.0.1:3000`, plus internal `hive-postgres` (pgvector enabled) and
`hive-redis`. `/api/v1/health` reports `status ok`, `version 1.0.0`.

- Project UUID: `08d8ac6c-eedb-42d5-a03c-d699eea6fc85`
- Name: `FORGE`
- Relative path: `forge`
- State: `READY`, `inspection_error: null`
- Git branch: `fge-002-governance-validation`
- Git HEAD: `fc0e8947514394d7fabc74442737c332302ef33d`
- `detached_head: false`, `repository_accessible: true`, `working_tree_clean: true`
- Detected language stack: `[]` (the repository carries no top-level manifest or language signal
  at the bootstrap stage; this is reported as observed, not inferred)
- Last inspection timestamp: recorded by re-inspection after the line-ending correction, at the
  exact head above.

### Host projects-root drift (defect corrected)

`HIVE_HOME/.env` and the persisted `User` environment variable declared
`HIVE_PROJECTS_ROOT=D:/HIVE/projects`, which is empty, while the running `hive-api` container
actually mounts `D:/Projetos Codex` at `/workspace/projects`. The registry rows for `CORE` and
`NEXLABS-WEB` resolve under the mounted directory, proving `D:/Projetos Codex` is the effective,
already-configured arrangement. Left uncorrected, the next `hive-up` would have recreated the API
container against the empty root and dropped all three projects to `OFFLINE`.

Fixed by aligning `.env` (backup kept as `.env.bak-20260921`) and the persisted `User` variable to
the root already in use, then confirming `docker compose config` now resolves
`D:/Projetos Codex -> /workspace/projects` read-only, identical to the running container.
The boundary was **not** widened to satisfy the check: it is the boundary the deployment was
already operating under, and the mount remains read-only.

## MCP surface

`hive-mcp` v`mcp-core-surface-v1` was probed over stdio and answers `initialize`, `tools/list` and
a real `tools/call`. The implemented surface is the documented read-only core of seven tools:
`project.list`, `project.status`, `context.build`, `context.search`, `memory.search`,
`memory.get`, `checkpoint.read`. The wider catalog in
`docs/project-brain/09-MCP-SKILLS-INTEGRATION.md` (`code.*`, `decision.*`, `run.*`, `validation.*`,
`telemetry.*`, `project.open`) is a design intent that v1.0.0 does **not** expose; it is not
claimed here.

`project.status` called through MCP returns FORGE `READY` with `working_tree_clean: true` at
`fc0e8947`, proving the chain end to end. `hive-mcp.cmd` was launched from an unrelated working
directory to confirm it resolves `HIVE_HOME` itself.

Client registration for the shared launcher `cmd.exe /d /s /c call <HIVE_HOME>\..\bin\hive-mcp.cmd`:

| Client | Before | Now |
| --- | --- | --- |
| Codex (`~/.codex/config.toml`) | `[mcp_servers.hive]` present | unchanged, verified |
| Claude Code (`~/.claude.json`) | global `hive` present | unchanged, verified |
| Qoder (`~/.qoder/settings.json`) | absent | `mcpServers.hive` added |
| opencode (`~/.config/opencode/opencode.json`) | absent | `mcp.hive` added, `timeout` raised to 60000 ms |
| Gemini / Antigravity (`~/.gemini/config/mcp_config.json`) | 0-byte file | valid `mcpServers.hive` |
| Cursor, VS Code | no MCP configuration of any kind on this machine | not added |

Cursor and VS Code were deliberately left untouched: neither has any existing MCP server entry, the
`~/.cursor/mcp.json` file does not exist, and VS Code's settings carry no MCP or Copilot-chat keys,
so there is no working pattern here to extend. Writing speculative editor-level MCP config could not
be validated on this machine and would risk breaking those installs.

The launcher requires the stack to be up: `hive-mcp.cmd` exits with an explicit
"the API service is not running. Run hive-up first." message rather than hanging, so a cold client
startup fails loudly. Because the server cold-starts through `docker compose exec`, a client default
timeout of five seconds is too short; that is why the opencode entry sets an explicit timeout, and it
is the value to raise if another client reports MCP startup failures.

## Defects found and corrected

1. Projects-root drift between `.env`/user environment and the live container, which would have
   silently taken all three registered projects offline on the next restart. Corrected as above.
2. `FORGE` reporting `working_tree_clean: false` while the host Git tree was provably clean. Root
   cause was Windows `core.autocrlf=true` producing CRLF working files against LF blobs; the Linux
   container's Git has no conversion, so it flagged all 24 tracked files. Content identity was
   confirmed by hashing (`git hash-object` equals the index blob, `git diff` empty on both sides)
   before the index was rebuilt from `HEAD`. HIVE now reports clean.

## Acceptance criteria status

| Criterion | Result |
| --- | --- |
| A safe local synchronization, no user work destroyed | PASS |
| B PDF prompt policy present and referenced by `AGENTS.md` | PASS |
| C active `main` ruleset meeting the target policy | PASS |
| D GEF v1.0.0 pinned commit plus validation and audit | PASS |
| E Forge registered and `READY` in local HIVE at the relevant Git state | PASS |
| F PR #1 carries the final delta and required check passes on the exact head | pending push/CI below |
| G Evidence bundle truthful and complete | PASS |
| H no HIGH/CRITICAL known defect remains | PASS |

## Checkpoint position and remaining risk

The checkpoint is intentionally **not** promoted in this PR. `.github/workflows/repository-validation.yml`
asserts `grep -q "FGE_BOOTSTRAP_IN_PROGRESS" .engineering/CHECKPOINT.md` and
`grep -q '"productImplementationAuthorized":false' .engineering/CHECKPOINT.json`, so editing either value
here would fail the very gate this increment must satisfy. Promotion is a post-approval, post-merge
increment.

`productImplementationAuthorized` stays `false`. Acceptance of this increment belongs to the reviewer
and to the now-mandatory approving review, not to the executor.

Remaining risks:

- The ruleset has no bypass actor, so if repository administration is ever lost there is no routine
  override path. This is the requested posture, not an accident.
- HIVE language detection returns an empty stack for Forge; it will only become meaningful once the
  repository carries real manifests, which FGE-002 forbids in this increment.
- `require_extra_approval_for_unattributed_changes` is a GitHub-applied default that can require an
  extra approval for commits lacking author attribution.
- The GEF validation checkout at `D:\Projetos Codex\hive-gef-validation` is disposable scratch and
  may be removed at any time; it holds no unique state.
- HIVE keeps read-only visibility over every project folder beneath `D:/Projetos Codex`, not only
  Forge. Accepted because it is the pre-existing deployment boundary; narrowing it would require
  relocating unrelated user projects, which this Work Order forbids.
