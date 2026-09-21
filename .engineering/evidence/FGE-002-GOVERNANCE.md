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
- Git HEAD at capture: `fc0e8947514394d7fabc74442737c332302ef33d`
- `detached_head: false`, `repository_accessible: true`, `working_tree_clean: true`
- Detected language stack: `[]` (the repository carries no top-level manifest or language signal
  at the bootstrap stage; this is reported as observed, not inferred)
- Last inspection timestamp: recorded by re-inspection after the line-ending correction, at the
  exact head above.

The registry tracks the checked-out branch, so its head advances with the commits below. The
capture head is recorded instead of the final head because any further edit to this file moves the
head again; re-inspecting the merged `main` state is a mandatory step of the promotion increment
that follows this PR, and `READY` here is claimed only for the state actually observed.

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

## PR #1 and exact-head validation

- PR: https://github.com/KayzenRoot/forge/pull/1 — state OPEN, base `main` at
  `e8e99c234669e3fab9956a97bb42f3fbd2b8e6f5`.
- Final head: `f7329bb11437a585514a03d32ad997c747a1b299`.
- Published by fast-forwarding the existing branch only (`564a3fd..f7329bb`); no force push, no
  history rewrite, and the pre-existing `fge-002-prompt-policy` branch was left in place rather
  than deleted.
- `Repository validation` run `35596989160`, event `pull_request`, head SHA exactly
  `f7329bb11437a585514a03d32ad997c747a1b299`: **success**. The required `governance` check reports
  `pass` on this exact head.
- Delta scope confirmed: 6 files, +331 / -0, all of them governance, policy, work-order or
  documentation files. No Forge product implementation is present in this increment.
- `mergeStateStatus` is `BLOCKED` by design: the ruleset installed in this increment requires one
  approving review, which the executor must not self-award. `mergeable` is `MERGEABLE`.

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
| F PR #1 carries the final delta and required check passes on the exact head | PASS |
| G Evidence bundle truthful and complete | PASS |
| H no HIGH/CRITICAL known defect remains | PASS |

## Checkpoint position and remaining risk

> Superseded for the checkpoint decision by "Post-merge checkpoint promotion" at the end of this file.
> The workflow assertions quoted below no longer exist; the surrounding paragraph is kept as the
> rationale recorded at the time of PR #1.

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

---

# Post-merge checkpoint promotion (2026-09-21)

Executed by the FGE-002 Correction Delta after independent review verified acceptance criteria A-H.
This section is the promotion evidence bundle. No Forge product code is introduced here.

## Why a correction delta was needed

Criteria A-H passed, but GitHub rejected the reviewer approval with HTTP 422: PR #1 is authored by
`KayzenRoot`, the only maintainer account, and GitHub prohibits a self-approval
(`Review Can not approve your own pull request`). The active ruleset demanded one approving review, so
the repository was in a governance deadlock that no code change can resolve. This is an administrative
correction, not a code defect.

## Ruleset readback after the correction

`PUT /repos/KayzenRoot/forge/rulesets/23765135` (the ruleset update route is PUT; PATCH returns 404).
A semantic diff of the before/after readback contained exactly one change:
`.rules[pull_request].parameters.required_approving_review_count: 1 -> 0`.

Confirmed state read back from the API after the mutation:

| Field | Value |
| --- | --- |
| id / name | `23765135` / `main-governance` |
| enforcement | `active` |
| target | `branch`, `refs/heads/main` (no excludes) |
| rules | `deletion`, `non_fast_forward`, `pull_request`, `required_status_checks` |
| required_approving_review_count | `0` |
| required_review_thread_resolution | `true` |
| dismiss_stale_reviews_on_push | `true` |
| require_extra_approval_for_unattributed_changes | `true` |
| require_code_owner_review / last_push_approval | `false` / `false` |
| allowed_merge_methods | `merge`, `squash`, `rebase` |
| required status check context | `governance`, strict policy `true` |
| bypass_actors | `[]` |

Zero approvals is appropriate here because this is a single-maintainer repository where GitHub forbids
self-approval; the objective gates (strict `governance` status check, conversation resolution,
stale-review dismissal, deletion and non-fast-forward blocks, no bypass actor) remain mandatory and are
what actually gates merge.

## Merge through the protected path

- PR #1 head bound at merge time: `8dba60df84316722df5f62c4054102697953c960`.
- `mergeStateStatus` transitioned `BLOCKED -> CLEAN` after the ruleset correction; `reviewThreads` was
  empty (no unresolved conversations).
- Merged with `gh pr merge 1 --squash --match-head-commit 8dba60df...`; no `--admin` bypass, no force
  push, no reset, no history rewrite.
- PR #1 state `MERGED` at `2026-09-21T12:18:21Z`; squash commit on `main`:
  **`9fcea3e6b5877a743bd49e01d3f67c4a53bbf62d`**.
- Local `main` was synchronized with `git checkout main` + `git merge --ff-only origin/main` only, and
  `git status --short` was empty afterwards.

## Repository validation on merged main

- Run `35598778741`, workflow `Repository validation`, event `push`, head `9fcea3e6b5877a743bd49e01d3f67c4a53bbf62d`,
  status completed, conclusion **success**; job `governance` check-run conclusion `success`.
- The governance/policy documents promoted by FGE-002 are now on `main`:
  `.engineering/CODEX-PROMPT-POLICY.md`, the `AGENTS.md` binding paragraph (1 reference),
  `.engineering/work-orders/FGE-002.md` and this evidence file.

## HIVE READY-on-main proof

Re-inspection of the existing registration through
`POST /api/v1/projects/08d8ac6c-eedb-42d5-a03c-d699eea6fc85/inspect` with the local checkout on merged
`main`:

- Project UUID `08d8ac6c-eedb-42d5-a03c-d699eea6fc85`, name `FORGE`, relative path `forge`.
- `state: READY`, `inspection_error: null`.
- `git_branch: main`, `git_head_sha: 9fcea3e6b5877a743bd49e01d3f67c4a53bbf62d` (equal to the merge SHA).
- `working_tree_clean: true`, `detached_head: false`, `repository_accessible: true`.
- `last_inspected_at: 2026-09-21T12:19:53.243525Z`.

## What this promotion changes and why

1. `repository-validation.yml` previously hard-grepped `FGE_BOOTSTRAP_IN_PROGRESS` and
   `"productImplementationAuthorized":false`. That made the gate and the checkpoint mutually exclusive:
   the checkpoint could only be promoted in a PR whose validation would then fail. The gate now
   validates checkpoint **schema and cross-file consistency** instead: JSON key set and types,
   `schemaVersion == 1`, UPPER_SNAKE tokens for `state`/`stopCondition`/`hive.integrationState`,
   `FGE-NNN` increment, semver GEF version, boolean authorization flag, plus agreement between
   `CHECKPOINT.md` and `CHECKPOINT.json` on State / Active increment / Risk / Product implementation,
   `activeIncrement` present in `BACKLOG.md`, `gef.version` documented in `GEF-ADOPTION.md`, project name
   matching `PROJECT-OVERVIEW.md`, `AGENTS.md` binding the prompt policy, a live-HIVE claim being
   incompatible with the bootstrap disclaimer in `HIVE-INTEGRATION.md`, and
   `productImplementationAuthorized=true` being impossible without a non-empty Work Order for the active
   increment.
2. The gate no longer names any lifecycle state, so the next promotion does not require editing it.
   Verified locally: the extracted gate passes unchanged on the legacy `FGE_BOOTSTRAP_IN_PROGRESS`
   checkpoint and on the promoted `FOUNDATION_READY` checkpoint, and fails with the specific reason for
   each injected mutation (lowercase state, JSON/markdown drift, authorization without a Work Order,
   live claim against the disclaimer).
3. The checkpoint moves to `FOUNDATION_READY` with `activeIncrement: FGE-003` and
   `stopCondition: FGE_003_WORK_ORDER_ADMISSION`.
4. `hive.integrationState` moves from `DOCUMENTED_NOT_LIVE_VALIDATED` to `LIVE_REGISTERED_READY`, and
   `HIVE-INTEGRATION.md` drops the bootstrap disclaimer, because live registration is now proven above
   (criterion 4 of this delta). Leaving the disclaimer in place would have made the documentation
   untruthful in the opposite direction.
5. `BACKLOG.md` marks FGE-001/FGE-002 complete and FGE-003 as the next, not-yet-admitted increment.
   `DECISIONS.md` records ADR-0005 (solo-maintainer review policy) and ADR-0006 (foundation completion is
   necessary but not sufficient).

## Authorization position after promotion

`productImplementationAuthorized` remains **`false`**. Foundation completion is a gate that FGE-002
satisfied, not the gate that admits product work: `PROJECT-OVERVIEW.md` states that product capabilities
are not considered implemented until admitted by Work Orders and proven by evidence, and
`DEFINITION-OF-DONE.md` requires admitted scope and acceptance criteria before an increment runs. No
FGE-003 Work Order exists yet, so asserting authorization here would be a fabricated approval. The
new gate enforces that rule mechanically.

## Verification commands (reproducible)

```bash
gh pr view 1 --repo KayzenRoot/forge --json state,mergedAt,headRefOid,mergeCommit
gh api repos/KayzenRoot/forge/rulesets/23765135
gh run list --repo KayzenRoot/forge --workflow repository-validation.yml --limit 3
git -C "D:/Projetos Codex/forge" rev-parse main origin/main
curl.exe -s -X POST "http://127.0.0.1:8000/api/v1/projects/08d8ac6c-eedb-42d5-a03c-d699eea6fc85/inspect"
```

## Remaining risk after promotion

- Zero required approvals removes the human-review barrier; the compensating control is the strict
  `governance` check plus the mechanical checkpoint-consistency gate described above. A second
  maintainer, or a bot account whose reviews GitHub counts as independent, should restore
  `required_approving_review_count: 1`.
- `require_extra_approval_for_unattributed_changes` is `true`, so any future commit pushed without
  author attribution will still demand an approval that this account cannot self-grant.
- The gate itself is edited inside the PR it validates. For a same-repository `pull_request` event
  GitHub runs the workflow definition from the head commit, so the new checks are what gate this merge;
  the residual risk is that a contributor able to open a PR could weaken the gate in their own head.
  That is accepted for a single-maintainer repository where no outside write access exists, and it is why
  the substantive rules still live in canonical documents rather than only in CI.
