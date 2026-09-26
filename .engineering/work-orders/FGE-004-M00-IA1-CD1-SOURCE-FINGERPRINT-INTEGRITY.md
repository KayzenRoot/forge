# FGE-004-M00-IA1-CD1 - Source Fingerprint Integrity Correction

Status: ADMITTED / EXECUTION AUTHORIZED
Risk: HIGH assurance evidence integrity
Repository: `KayzenRoot/forge`
Branch / PR: `fge-004-m00-implementation` / PR #9
Implementation candidate: `4d6109024d652fd0ac7454e14eeb5aaa9f5113ac`

## Objective

Correct the admitted 68-path source inventory using SHA-256 over the exact Git blob bytes from one immutable commit. Add a standard-library verifier that reads the manifest and all source blobs from the same Git tree, then record reproducible evidence and update PR #9. This correction resolves source-fingerprint integrity only; it does not certify M00.

## Context Lock

The execution base is commit `a850153a596718afdc6926e649e4d87a09d29fff`, tree `5f66df9f62a52a9e85fccbe005d9ca0e0274b479`. Before edits, the local branch, origin branch and PR #9 head all resolved to this commit. PR #9 was open, draft and unmerged. Actions run `35895128860` completed successfully on that exact head. The PR metadata and run were checked through the connected GitHub integration because GitHub CLI is not installed in this environment.

The only pre-existing untracked local path was `output/`. The resume authorization in this delta applies to that path alone: preserve it in place, inventory relative paths/types/sizes/SHA-256 before and after, never inspect its contents, and never stage it. This exception does not waive source-tree, scope, or exact-head checks.

The admitted source path sequence contains 68 unique entries. Its SHA-256 is `4e7866cb3d1f6dc10f0112d4e0cfd8c0aebe71b2f2fd07c5b185ee61c11a7ffa`, calculated over the path bytes in manifest order joined by NUL bytes. Preserve the sequence and two-space separator. Evidence, work-order, verifier and test files remain outside the 68 entries.

## In Scope

- Recompute every manifest digest from the raw blob returned by Git for the locked commit tree.
- Preserve exactly the admitted 68 source paths and their order.
- Add `.engineering/scripts/verify_source_fingerprints.py` using only the Python standard library.
- Add focused fixture tests that use temporary local Git repositories outside the project checkout.
- Record the complete old-to-corrected mismatch list, byte-level cause, output-directory preservation inventory, command/test outcomes, risks and a proposed checkpoint delta.
- Update the existing Evidence Bundle and the factual PR #9 description after local checks and exact-head CI pass.

## Out of Scope

- Rust runtime/product changes, Rust tests, dependencies, schemas, APIs, workflows or module changes.
- Changes to `.engineering/CHECKPOINT.md` or `.engineering/CHECKPOINT.json`.
- Assurance-packet digest computation, reviewer dispatch, certification/approval, PR merge or M01 work.
- Any change, staging or content disclosure from `output/`.

## Verifier Contract

The verifier resolves one commit and its tree, reads the committed manifest blob and source blobs with Git plumbing, and hashes raw object bytes. It does not hash working-tree files, invoke a shell, normalize bytes, follow symlinks, access the network or require third-party Python packages. It emits deterministic JSON with commit SHA, tree SHA, manifest count, pass/fail, errors and per-path mismatches.

It must fail nonzero for malformed rows/digests, duplicate or aliased paths, traversal/ambiguous paths, a changed path sequence, a wrong entry count, absent paths, non-regular Git objects, unavailable blobs and digest mismatches. A positive CRLF fixture must still pass when its checkout bytes differ from its committed blob bytes.

Usage from the repository root: `python .engineering/scripts/verify_source_fingerprints.py --commit HEAD`. Run isolated fixtures with `python .engineering/scripts/test_source_fingerprints.py`.

## Acceptance Criteria

1. The 68 unique paths and order are unchanged; every digest equals SHA-256 of its corresponding regular Git blob in the verified tree.
2. The exact byte-level cause of the former mismatches is supported by reproducible data; any unresolved historical detail is stated as unknown.
3. The verifier passes in a fresh clean clone of the pushed commit and fails the focused negative fixtures with clear JSON reasons.
4. The full old-to-corrected mismatch list and before/after `output/` inventory are recorded without copying file contents.
5. Changes are limited to the manifest, verifier, tests, this Work Order and evidence documentation. No output path enters the index or PR diff.
6. PR #9 remains open, draft and unmerged; exact-head CI is green and its run is recorded in the PR description.
7. The canonical checkpoint remains unchanged. No assurance digest or reviewer verdict is created, and independent assurance remains pending.

## Stop Condition

Stop after the corrected manifest and verifier pass in a fresh clean clone, the exact pushed head has green required checks, the PR description records the exact head/tree/result/run, and the independent-review blocker is explicitly preserved. If GitHub authentication is unavailable, complete and preserve local verified work, then report the precise remote step that could not be performed. Never force-push, rewrite history, discard local data, merge PR #9, promote the checkpoint or start M01.
