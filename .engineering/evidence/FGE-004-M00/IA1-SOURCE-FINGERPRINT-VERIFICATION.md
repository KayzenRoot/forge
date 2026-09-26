# FGE-004-M00-IA1-CD1 - Source Fingerprint Verification

Status: SOURCE INVENTORY CORRECTED; independent assurance remains pending. This evidence does not certify M00.

## Context Lock

- Repository: `KayzenRoot/forge` (`https://github.com/KayzenRoot/forge`).
- Branch / PR: `fge-004-m00-implementation` / PR #9.
- Base commit/tree: `a850153a596718afdc6926e649e4d87a09d29fff` / `5f66df9f62a52a9e85fccbe005d9ca0e0274b479`.
- Before editing, local branch, origin branch and PR head all matched the base. PR #9 was open, draft and unmerged.
- Exact-base Actions run `35895128860` completed successfully with all four configured jobs.
- The original M00 implementation candidate is `4d6109024d652fd0ac7454e14eeb5aaa9f5113ac`; later commits are evidence-only.
- Environment: Windows x86_64; Python `3.12.14`; Git `2.55.0.windows.3`; `rust-toolchain.toml` pins Rust `1.98.1`. No Rust code or dependencies changed.

The repository had one pre-existing untracked directory, `output/`. It was not opened, changed, staged or added to evidence by content. Its metadata inventory was captured before execution and compared after execution:

| Relative path | Type | Size | SHA-256 before | SHA-256 after |
| --- | --- | ---: | --- | --- |
| `pdf/` | directory | 0 | n/a | n/a |
| `pdf/FGE-004-M00-IA1-CORRECTION-DELTA-EXECUTOR-PROMPT.pdf` | file | 11,627 | `8aaeb0f7c8c48253f48b9490d2450870b882b3fb8ad0fb02fab929c2f1480069` | `8aaeb0f7c8c48253f48b9490d2450870b882b3fb8ad0fb02fab929c2f1480069` |

The sorted inventory JSON SHA-256 was `ca24315981f5ea3b599660d65a71bef22b493aae4dbd00efd35a3fa8e6ccc4d2` before execution and must match after execution.

## Source Set and Finding

The base manifest blob is `8d8ab5d0862d00db0493f70193eae2599750adbe` (SHA-256 `e6775211a3d89d33c23a01f379568e51b9358ff2abbd2ff68d0a7981e5b949b7`). It contains 68 well-formed, unique paths. The path sequence SHA-256 is `4e7866cb3d1f6dc10f0112d4e0cfd8c0aebe71b2f2fd07c5b185ee61c11a7ffa`; the corrected manifest preserves that exact path sequence, order and two-space separator.

At the locked base, 22 old entries differed from SHA-256 of their raw Git blob bytes; the other 46 already matched. For all 22 mismatches, the old digest equalled the SHA-256 of the current checkout file. For each, replacing `CRLF` pairs in the checkout bytes with `LF` reproduced the exact Git blob bytes, and `git hash-object --path=<path> <path>` returned the blob OID in the locked tree. The checkout contained mixed `CRLF` and bare `LF`; a uniform `LF`-to-`CRLF` transformation of the Git blob reproduces only one old digest. The reproducible cause is that the prior inventory hashed checkout representations rather than Git object bytes. The historical process that produced the mixed checkout bytes cannot be determined from the repository evidence.

The checked-in `.gitattributes` specifies `* text=auto eol=lf`; both system and repository Git configuration reported `core.autocrlf=true`. These facts explain why checkout bytes are a separate representation and are not used by the corrected verifier. The correction hashes Git blobs directly and performs no newline normalization.

### Complete old-to-corrected mismatch list

| Path | Previous SHA-256 | Corrected Git blob SHA-256 |
| --- | --- | --- |
| `.engineering/modules/M00-CONSOLIDATED-ARCHITECTURE.md` | `7d5e52cc4e8d5c6e809787aac81a0bf21e3ca0b3d71a7eb78b486594bab4e07e` | `e2cc4300b00a24f2b3d5ccfdca965c28233a19e38216e08ce270e4ac530830e5` |
| `.engineering/modules/M00-S01-KERNEL-RUNTIME.md` | `b77ed82ecc61fc4af4fd22247c90ce6b2ce9ab26c3e3b73bf2bcbb344b3a9a99` | `6a80ddd67c270859b23a6c282ef495bb1ae0e1a8fe8ff02dda7255a31ca97f6d` |
| `.engineering/modules/M00-S02-MODULE-LIFECYCLE.md` | `84a40220a8dca9a831f1b5a1ecd86e26ee3f046aadfe3eef4bfc69ac21c7012d` | `0db072aa27f8b9c8bc9c941c5445d6eaf720734cc664757ca7c8eb19dee346a5` |
| `.engineering/modules/M00-S03-FORGE-CONTRACT-FABRIC.md` | `457b08a0ee9b3a60606d00a110623e55f656c5e4c1b356aa2c250c04ba5ba167` | `6751642e593a21dd50341ab036fa911e1760971245a535b22cba0d4f2ef6d196` |
| `.engineering/modules/M00-S04-CAPABILITY-REGISTRY.md` | `675a04f21de69867c76cd1bdef628e8670a1d28a054c039cf46f8cfeea9e91e7` | `860716c09de0e387ca9f6ea500bc004d096cda0fe856166938c8df24b8911340` |
| `.engineering/modules/M00-S05-CAPABILITY-RESOLVER-DECISION-PLANE.md` | `fdff401927e4f960a0480a64b73badb51e320c1e1c9819cacd3603452967ef10` | `8136c69a3f2eca5678ccbbd740e6dd61b92284612f591da2df4795176c51c229` |
| `.engineering/modules/M00-S06-DEPENDENCY-CAUSALITY-GRAPH.md` | `6eaf289a194ca318ab9734f9a6adfd41f9521e83109423c65347109908cefed7` | `5657bc274786a58984249ae43d02d5c8a3400c58292a3f8b865153ad9486bcfa` |
| `.engineering/modules/M00-S07-CONFIGURATION-SYSTEM.md` | `b1df4f5ce629cd84206f99f3f3eca8de87f6f8c96354d73b64d9407e1b41baae` | `e8745c8f921b15553b770d20601443afea79c8c0d9c4160025d5540f108d1f68` |
| `.engineering/modules/M00-S09-EVENT-BUS.md` | `7eca76a87ad7cb7933920201e7742700d82a25a8690eef2d45fc6ecb9fda2d0a` | `f62a0745b62310e27f18a52db9ebd63f129ba1553aef88acac0cc5a0db3f8898` |
| `.engineering/modules/M00-S10-COMMAND-BUS.md` | `c0702618969252e5d498b655b4f65444bc2f0dce03e6b8637b27e1fd21bac87d` | `3af66ea8df98f53504c0cbb5c142089d622bff80bc1110dd7bd2f0ee00abb049` |
| `.engineering/modules/M00-S11-ERROR-TAXONOMY.md` | `371796413dbb141030e505857ac39d68bb7c6c3468d67bcf789b7c9aa18c2d5f` | `a4f86c29054fd2a4c11d3d43447d3faa9627badf0c8968c90ef38a88681c0f19` |
| `.engineering/modules/M00-S13-CONCURRENCY-BACKPRESSURE.md` | `f2a3f1725b4cd47667e666e9e4f12ef0dbb9777d8ca570c498040eac485ce578` | `756a472655bdb710501e286fb1e819af4858efd203014c8af0fecdf387418f33` |
| `.engineering/modules/M00-S14-CACHE-FINGERPRINTS.md` | `087d1c87e9ad9d904aeece02dc7bddf1e35db7a8ce3de409e658ced54613cff0` | `557e602e7995bb3ed7084ca7ffbdc4220d05ad1396785e44d285def3648aa597` |
| `.engineering/modules/M00-S15-PLUGIN-ADAPTER-SDK.md` | `93cf850e2e6c988053b37dfbb0754c78a2e1623541df1982c3849f55bda7e1e0` | `9c49ac1875d165e91558aad1175a385e97e988f395f9a9539f3066e8c28f7a58` |
| `.engineering/modules/M00-S16-COMPATIBILITY-ENGINE.md` | `016b47d86541467a911fde1534f36a18b1a2300794b7a7f93da16b00b0860ac9` | `569d4bf76905fb3cab0ea3ffca5be81f4ea1df2b8d0625c9970bd42aababe2e5` |
| `.engineering/modules/M00-S17-HEALTH-READINESS.md` | `e94e60106193e3f880fa8fb0fd026e79f8410ca244fa370c6e52e4d542c4ff06` | `f49b78d5483e267f404d9553abf05e29aeef9fd0d682fba19fe5accd3eed520a` |
| `.engineering/modules/M00-S18-TELEMETRY-HOT-PATH-REGISTRY.md` | `dce3a237cbbd3e93a13978c7cbfef916648141fa7749697487e90699e4524570` | `121f738981462a3d93f5d2aa56e20f7f5508ea36fae628f9f5f1cf753a7b7e3e` |
| `.engineering/modules/M00-S19-DETERMINISTIC-EXECUTION-ENVELOPE.md` | `60928eaec4ab63d4c473907d6934b0781f9a82cf192dd02d0b84da476a075683` | `e7e3093c46e36e96229a60202868f346cb112e5db66efe930b29893814056704` |
| `.engineering/modules/M00-S21-ZERO-DEPENDENCY-BOOT.md` | `7a025cc73beb976664ed9007e849e3f12100dc474f6055e724a0c6a362ba49ed` | `ebd4a83679ceab736e4824d365659965d7efb3f5acaf5cff16fe4c36ad71f251` |
| `.engineering/modules/M00-S22-TEST-PROOF-FOUNDATION.md` | `e6ac21644e868f1e7d63a827dcd674a51c1b378ccef3acea9ae41783fdcc2362` | `7e6f260f2ae8de09c3dae8a550a31fad08a613ce179f314b7f4e4077a2dc666b` |
| `.engineering/modules/M00-TECHNOLOGY-RESEARCH.md` | `c97d3c48b93702b8e71451e6c63956e662b76c1fa9dca196fdcd969d47af99e8` | `98fc487852a2c247b2432f7683484181b0fb01e7a6faac4fdd5e812462b6f789` |
| `.github/workflows/repository-validation.yml` | `b1e7b67a97dcaf6b25ca26265e5464252dca1dea7916aedb24645605540ac5f0` | `1383d5ed594dc97cb25b687f1608c4c398284a436aa3f771dff40bf824c85879` |

## Verifier and Focused Proofs

The standard-library verifier reads the committed manifest and source objects from one resolved commit/tree. It uses Git argument arrays and `git cat-file --batch`; it does not read checkout source bytes, normalize line endings, follow symlinks or access the network. It rejects malformed rows/digests, duplicate and case-aliased paths, traversal/ambiguous paths, a changed path/order fingerprint, wrong count, missing paths, non-regular blobs, unavailable objects and digest mismatches. JSON output includes `commit_sha`, `tree_sha`, `manifest_count`, `pass`, `errors` and per-path `mismatches`.

The fixture harness uses only Python's standard library and temporary local repositories outside this checkout. Its positive fixture commits CRLF bytes, then changes only checkout bytes; the pinned-commit verification still passes. Negative cases cover digest mismatch, invalid digest, duplicate path, malformed row, missing path, traversal, wrong count, symlink/non-regular tree entry and Git command failure. Repeated verification of the same immutable fixture emits identical JSON.

Commands run from the repository root:

```text
python .engineering/scripts/test_source_fingerprints.py
python .engineering/scripts/verify_source_fingerprints.py --commit a850153a596718afdc6926e649e4d87a09d29fff
python .engineering/scripts/verify_source_fingerprints.py --commit HEAD
```

The base-commit invocation intentionally reports the historical 22 mismatches. The fixture command passes. The `HEAD` invocation is the post-correction check run in a fresh clone; its exact JSON is included in the PR #9 final-head update.

Observed focused results:

- Python in-memory syntax checks for both scripts: PASS.
- `.engineering/scripts/test_source_fingerprints.py`: PASS for all listed positive/negative fixtures.
- The repository's canonical checkpoint governance validation from `repository-validation.yml`: PASS.
- `git diff --check`: PASS on the working diff; it is repeated against the explicit staged allowlist before commit.
- Fresh clean clone verification and exact pushed-head Actions evidence are bound to the final commit in the PR #9 update, as required by the Work Order.
- Rust format/build/test/security suites were not rerun: no Rust source, test, dependency or workflow changed; the repository's exact-head CI is the hosted validation for this evidence-only correction.

## Tool and Assurance Capability Evidence

- `codex --version`: `codex-cli 0.158.0-alpha.2.1`; `codex --help` succeeded and listed `agents`, `review`, `exec` and `doctor` commands. CLI help alone does not prove an independent reviewer backend.
- `uads`: unavailable; no executable was found, so version/help/capability status could not be collected.
- No assurance-packet digest was computed; no reviewers were launched; no independent identities or verdicts are claimed. The independent-review gate remains open.

## Files and Checkpoint Delta Proposal

This correction changes only the 68-entry source manifest, the verifier and fixture test, this Work Order, this evidence record, and a pointer in `EVIDENCE-BUNDLE.md`. No Rust/runtime, dependency, schema, API, workflow, or canonical checkpoint file changes. The output preservation directory is excluded from the index and PR diff.

Proposed future checkpoint-delta wording after independent assurance: “The IA1 source-fingerprint integrity blocker is resolved by commit `<exact correction head>` with 68/68 Git-blob verification and exact-head CI. M00 remains a candidate pending independent assurance; do not promote or begin M01.” This proposal does not change the checkpoint in this increment.

## Final Exact-Head Record

The final commit SHA, tree SHA, clean-clone verifier JSON, Actions run ID and PR draft/unmerged status are recorded in PR #9 after the push and exact-head run complete. The implementation candidate remains distinct from every evidence-only head.

## C01 - Verifier Negative-Fixture Coverage Correction

Status: All three requested negative fixtures pass locally on Windows. The verifier implementation needed no change; the audit found missing regression coverage, not a reproduced verifier defect.

### Entry Context Lock

- Repository/branch/PR: KayzenRoot/forge, fge-004-m00-implementation, PR #9.
- Entry commit/tree: d6bf00928f60f2809bdefbbcbd30ca55049789e3 / fb6d04961c3a025c73a2971053beade38ef45931.
- PR #9 was open, draft and unmerged. The branch and PR head matched the entry commit before editing.
- Entry-head run 36241342785 passed all four configured jobs.
- Python 3.12.14; Git 2.55.0.windows.3.
- At C01 entry, output/ was the only pre-existing untracked path. Its before-execution metadata inventory still matches aggregate SHA-256 ca24315981f5ea3b599660d65a71bef22b493aae4dbd00efd35a3fa8e6ccc4d2.
- Final C01 commit/tree and exact-head Actions evidence are bound in the PR #9 update after push. The evidence file cannot contain its own final commit ID.

### Required C01 Proofs

| Fixture | Contract and assertion | Result |
| --- | --- | --- |
| Manifest order | Swap the first two existing rows, preserving the same 68 paths, digests and total count. Require nonzero exit, JSON, manifest_count=68, path_list_mismatch, and no wrong_entry_count. | PASS on Windows. |
| Case-only Git tree alias | Insert a second tree path differing only by case using git update-index --cacheinfo with core.ignorecase=false; no conflicting filesystem entry is created. Require nonzero exit, ambiguous_tree_path error and mismatch reason, and repeatable JSON. | PASS on Windows; repeated output was byte-identical. |
| Missing source blob | Commit a valid fixture repository, then remove exactly one referenced loose source blob from that temporary repository. Require nonzero exit, valid JSON, object_unavailable for that path/OID, and no traceback. | PASS on Windows. The fixture makes only the temporary object writable before unlinking; TemporaryDirectory cleans the entire fixture on success or assertion failure. |

The harness runs each verifier call in a subprocess and parses one JSON result with empty stderr. Git invocations clear inherited GIT_* variables, disable global/system Git configuration, terminal prompts and lazy fetch, and use only local repositories with no remotes. No tests are skipped by platform.

### C01 Commands and Results

    python -B .engineering/scripts/test_source_fingerprints.py
    python -B .engineering/scripts/verify_source_fingerprints.py --commit HEAD
    python -B .engineering/scripts/verify_source_fingerprints.py --commit a850153a596718afdc6926e649e4d87a09d29fff
    git diff --check

- Fixture suite: PASS, including existing positive/negative fixtures and all three C01 proofs.
- Verifier on entry HEAD: PASS; manifest_count=68, mismatches empty, errors empty.
- Verifier on historical base a850153a596718afdc6926e649e4d87a09d29fff: expected nonzero result with 22 mismatches and zero errors. The historical 22 are not a regression in the corrected head.
- Canonical foundation/governance validation block from .github/workflows/repository-validation.yml: PASS; M00_EXECUTION_READY, FGE-004, implementation authorized.
- C01 modifies the test harness and its evidence only. The verifier, 68 source fingerprints and order, checkpoint, Rust/runtime, dependencies and workflow remain unchanged.

### C01 Final Exact-Head Binding

After push, the final clean-clone harness/verifier result, final commit/tree, four-job Actions conclusion, output/ inventory comparison and PR #9 open/draft/unmerged state are recorded in the exact-head PR description. C01 stops for audit after that binding. It creates no assurance-packet digest or reviewer verdict and does not certify/promote M00 or begin M01.
