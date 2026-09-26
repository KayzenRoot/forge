#!/usr/bin/env python3
"""Run isolated Git-object fixtures for the source fingerprint verifier."""

from __future__ import annotations

import hashlib
import json
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Callable

from verify_source_fingerprints import (
    MAX_MANIFEST_PATH_DEPTH,
    MAX_TREE_ENTRIES,
    GitResourceLimit,
    parse_tree,
    safe_manifest_path,
)


ROOT = Path(__file__).resolve().parents[2]
VERIFIER = Path(__file__).with_name("verify_source_fingerprints.py")
MANIFEST_RELATIVE = ".engineering/evidence/FGE-004-M00/SOURCE-FINGERPRINTS.sha256"
ROW_PATTERN = re.compile(rb"^([0-9a-f]{64})  (.+)$")


def isolated_git_environment() -> dict[str, str]:
    environment = {
        key: value for key, value in os.environ.items() if not key.upper().startswith("GIT_")
    }
    environment["GIT_CONFIG_GLOBAL"] = os.devnull
    environment["GIT_CONFIG_NOSYSTEM"] = "1"
    environment["GIT_NO_LAZY_FETCH"] = "1"
    environment["GIT_OPTIONAL_LOCKS"] = "0"
    environment["GIT_TERMINAL_PROMPT"] = "0"
    return environment


def run_git(repo: Path, *args: str, input_bytes: bytes | None = None) -> bytes:
    command = [
        "git",
        "-c",
        f"safe.directory={repo.resolve().as_posix()}",
        "--no-replace-objects",
        "-C",
        str(repo),
        *args,
    ]
    completed = subprocess.run(
        command,
        input=input_bytes,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
        env=isolated_git_environment(),
    )
    if completed.returncode != 0:
        raise AssertionError(
            f"fixture git {args[0]} failed with exit {completed.returncode}: "
            f"{completed.stderr.decode('utf-8', 'replace').strip()}"
        )
    return completed.stdout


def source_paths() -> list[str]:
    manifest = (ROOT / MANIFEST_RELATIVE).read_bytes()
    paths: list[str] = []
    for line_number, line in enumerate(manifest.splitlines(), start=1):
        match = ROW_PATTERN.fullmatch(line)
        if match is None:
            raise AssertionError(f"committed manifest row {line_number} is malformed")
        paths.append(match.group(2).decode("utf-8", "strict"))
    if len(paths) != 68 or len(set(path.casefold() for path in paths)) != 68:
        raise AssertionError("the repository manifest must contain 68 unique paths before fixtures run")
    return paths


def invoke(repo: Path, commit: str | None = None) -> tuple[int, dict[str, object], str]:
    command = [sys.executable, str(VERIFIER), "--repo", str(repo)]
    if commit is not None:
        command.extend(["--commit", commit])
    completed = subprocess.run(
        command,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
        env=isolated_git_environment(),
    )
    if completed.stderr:
        raise AssertionError(f"verifier wrote unexpected stderr: {completed.stderr.decode('utf-8', 'replace')}")
    try:
        payload = json.loads(completed.stdout.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise AssertionError("verifier did not emit one JSON result") from exc
    return completed.returncode, payload, completed.stdout.decode("utf-8")


def make_manifest_rows(paths: list[str]) -> tuple[list[bytes], dict[str, bytes]]:
    payloads: dict[str, bytes] = {}
    rows: list[bytes] = []
    for number, path in enumerate(paths):
        if path == ".gitattributes":
            content = b"* -text\n"
        elif number == 0 or path.endswith("M00-CONSOLIDATED-ARCHITECTURE.md"):
            content = b"fixture line one\r\nfixture line two\r\n"
        else:
            content = f"fixture source {number}\n".encode("utf-8")
        payloads[path] = content
        rows.append(f"{hashlib.sha256(content).hexdigest()}  {path}".encode("utf-8"))
    return rows, payloads


def create_fixture(
    parent: Path,
    name: str,
    paths: list[str],
    mutate_rows: Callable[[list[bytes]], list[bytes]] | None = None,
    object_format: str = "sha1",
) -> tuple[Path, list[bytes], dict[str, bytes]]:
    repo = parent / name
    repo.mkdir()
    run_git(repo, "init", "--quiet", "--initial-branch=main", f"--object-format={object_format}")
    run_git(repo, "config", "core.autocrlf", "false")
    run_git(repo, "config", "gc.auto", "0")
    run_git(repo, "config", "user.name", "Fingerprint Fixture")
    run_git(repo, "config", "user.email", "fixture@example.invalid")
    rows, payloads = make_manifest_rows(paths)
    chosen_rows = mutate_rows(list(rows)) if mutate_rows else rows
    manifest_path = repo / MANIFEST_RELATIVE
    manifest_path.parent.mkdir(parents=True, exist_ok=True)
    manifest_path.write_bytes(b"\n".join(chosen_rows) + b"\n")
    for path, content in payloads.items():
        file_path = repo.joinpath(*path.split("/"))
        file_path.parent.mkdir(parents=True, exist_ok=True)
        file_path.write_bytes(content)
    run_git(repo, "add", "--all")
    run_git(repo, "commit", "--quiet", "-m", "fixture")
    commit = run_git(repo, "rev-parse", "HEAD").decode("ascii").strip()
    return repo, rows, payloads


def expect_failure(
    parent: Path,
    name: str,
    paths: list[str],
    mutate_rows: Callable[[list[bytes]], list[bytes]],
    expected_code: str,
) -> None:
    repo, _, _ = create_fixture(parent, name, paths, mutate_rows)
    code, payload, _ = invoke(repo)
    if code == 0 or payload.get("pass") is not False:
        raise AssertionError(f"{name}: expected a nonzero verifier result")
    errors = payload.get("errors")
    codes = {item.get("code") for item in errors if isinstance(item, dict)}
    mismatches = payload.get("mismatches")
    reasons = {item.get("reason") for item in mismatches if isinstance(item, dict)}
    if expected_code not in codes and expected_code not in reasons:
        raise AssertionError(f"{name}: expected {expected_code}, got errors={errors} mismatches={mismatches}")


def tree_entry(repo: Path, commit: str, path: str) -> tuple[str, str, str]:
    records = run_git(repo, "ls-tree", "-r", "-z", commit).split(b"\0")
    wanted_path = path.encode("utf-8")
    for record in records:
        if not record:
            continue
        metadata, entry_path = record.split(b"\t", 1)
        if entry_path == wanted_path:
            mode, object_type, object_id = metadata.decode("ascii").split()
            return mode, object_type, object_id
    raise AssertionError(f"fixture tree is missing path {path}")


def assert_specific_failure(
    name: str,
    code: int,
    payload: dict[str, object],
    *,
    error_code: str | None = None,
    mismatch_reason: str | None = None,
    manifest_count: int | None = None,
) -> None:
    if code == 0 or payload.get("pass") is not False:
        raise AssertionError(f"{name}: expected a nonzero verifier result: {payload}")
    if manifest_count is not None and payload.get("manifest_count") != manifest_count:
        raise AssertionError(
            f"{name}: expected manifest_count={manifest_count}, got {payload.get('manifest_count')}"
        )
    errors = payload.get("errors")
    codes = {item.get("code") for item in errors if isinstance(item, dict)} if isinstance(errors, list) else set()
    if error_code is not None and error_code not in codes:
        raise AssertionError(f"{name}: expected error {error_code}, got {errors}")
    mismatches = payload.get("mismatches")
    reasons = (
        {item.get("reason") for item in mismatches if isinstance(item, dict)}
        if isinstance(mismatches, list)
        else set()
    )
    if mismatch_reason is not None and mismatch_reason not in reasons:
        raise AssertionError(f"{name}: expected mismatch reason {mismatch_reason}, got {mismatches}")


def main() -> int:
    paths = source_paths()
    deep_path = b"/".join([b"segment"] * (MAX_MANIFEST_PATH_DEPTH + 1))
    if safe_manifest_path(deep_path)[1] != "path exceeds its configured depth bound":
        raise AssertionError("deep source path did not hit the configured depth bound")
    tree_prefix = b"100644 blob " + b"0" * 40 + b"\t"
    oversized_tree = b"".join(
        tree_prefix + str(index).encode("ascii") + b"\0"
        for index in range(MAX_TREE_ENTRIES + 1)
    )
    try:
        parse_tree(oversized_tree)
    except GitResourceLimit:
        pass
    else:
        raise AssertionError("oversized Git tree did not hit the configured entry bound")

    with tempfile.TemporaryDirectory(prefix="forge-source-fingerprint-") as temporary:
        parent = Path(temporary)
        valid_repo, rows, payloads = create_fixture(parent, "valid", paths)
        valid_commit = run_git(valid_repo, "rev-parse", "HEAD").decode("ascii").strip()
        special_path = next(
            path
            for path, content in payloads.items()
            if b"\r\n" in content and path != ".gitattributes"
        )
        special_file = valid_repo.joinpath(*special_path.split("/"))
        special_file.write_bytes(payloads[special_path].replace(b"\r\n", b"\n"))
        code, result, first_output = invoke(valid_repo, valid_commit)
        if code != 0 or result.get("pass") is not True:
            raise AssertionError(f"valid Git blob fixture failed: {result}")
        if result.get("manifest_count") != 68 or result.get("mismatches") != []:
            raise AssertionError(f"valid fixture returned unexpected counts: {result}")
        if not re.fullmatch(r"[0-9a-f]{40,64}", str(result.get("commit_sha"))):
            raise AssertionError("valid fixture did not report its full commit SHA")
        if not re.fullmatch(r"[0-9a-f]{40,64}", str(result.get("tree_sha"))):
            raise AssertionError("valid fixture did not report its full tree SHA")
        if result.get("git_object_format") != "sha1" or len(str(result["commit_sha"])) != 40:
            raise AssertionError("SHA-1 fixture did not report a matching object format and commit ID")
        code_again, result_again, second_output = invoke(valid_repo, valid_commit)
        if code_again != 0 or first_output != second_output or result_again != result:
            raise AssertionError("verifier output was not deterministic for an immutable Git tree")

        fabricated_code, fabricated_result, _ = invoke(valid_repo, "f" * 40)
        if fabricated_code == 0 or "git_command_failed" not in {
            item.get("code") for item in fabricated_result.get("errors", []) if isinstance(item, dict)
        }:
            raise AssertionError("fabricated commit ID was accepted as an exact Git revision")

        sha256_repo, _, _ = create_fixture(parent, "valid-sha256", paths, object_format="sha256")
        sha256_code, sha256_result, _ = invoke(sha256_repo)
        if sha256_code != 0 or sha256_result.get("pass") is not True:
            raise AssertionError(f"valid SHA-256 Git fixture failed: {sha256_result}")
        if sha256_result.get("git_object_format") != "sha256" or len(str(sha256_result["commit_sha"])) != 64:
            raise AssertionError("SHA-256 fixture did not report a matching object format and commit ID")

        def bad_digest(lines: list[bytes]) -> list[bytes]:
            digest, separator, path = lines[0].partition(b"  ")
            replacement = b"0" * 64 if digest != b"0" * 64 else b"f" * 64
            lines[0] = replacement + separator + path
            return lines

        expect_failure(parent, "mismatch", paths, bad_digest, "digest_mismatch")
        expect_failure(
            parent,
            "invalid-digest",
            paths,
            lambda lines: [b"z" * 64 + b"  " + lines[0].split(b"  ", 1)[1], *lines[1:]],
            "malformed_row",
        )
        expect_failure(parent, "duplicate", paths, lambda lines: lines + [lines[0]], "duplicate_path")
        expect_failure(parent, "malformed", paths, lambda lines: [b"invalid row", *lines[1:]], "malformed_row")
        expect_failure(
            parent,
            "missing-path",
            paths,
            lambda lines: [lines[0].split(b"  ", 1)[0] + b"  missing/absent.txt", *lines[1:]],
            "missing_path",
        )
        expect_failure(
            parent,
            "traversal",
            paths,
            lambda lines: [lines[0].split(b"  ", 1)[0] + b"  ../outside.txt", *lines[1:]],
            "unsafe_path",
        )
        expect_failure(parent, "wrong-count", paths, lambda lines: lines[:-1], "wrong_entry_count")

        def reorder_first_two_rows(lines: list[bytes]) -> list[bytes]:
            if len(lines) != 68 or len(set(lines)) != 68:
                raise AssertionError("path-order fixture must start with 68 distinct manifest rows")
            lines[0], lines[1] = lines[1], lines[0]
            return lines

        order_repo, _, _ = create_fixture(parent, "path-order", paths, reorder_first_two_rows)
        order_commit = run_git(order_repo, "rev-parse", "HEAD").decode("ascii").strip()
        code, result, _ = invoke(order_repo, order_commit)
        assert_specific_failure(
            "path-order",
            code,
            result,
            error_code="path_list_mismatch",
            manifest_count=68,
        )
        order_codes = {
            item.get("code")
            for item in result.get("errors", [])
            if isinstance(item, dict)
        }
        if "wrong_entry_count" in order_codes:
            raise AssertionError("path-order fixture must preserve all 68 valid manifest rows")

        alias_repo, _, _ = create_fixture(parent, "case-only-tree-alias", paths)
        alias_source_path = next(
            path for path in paths if any(character.isalpha() for character in path)
        )
        alias_path = alias_source_path.swapcase()
        if alias_path == alias_source_path or alias_path.casefold() != alias_source_path.casefold():
            raise AssertionError("case-only tree fixture did not construct a case-only alias")
        alias_commit_before = run_git(alias_repo, "rev-parse", "HEAD").decode("ascii").strip()
        mode, object_type, object_id = tree_entry(
            alias_repo, alias_commit_before, alias_source_path
        )
        if object_type != "blob" or mode not in ("100644", "100755"):
            raise AssertionError("case-only tree fixture source is not a regular blob")
        run_git(alias_repo, "config", "core.ignorecase", "false")
        run_git(
            alias_repo,
            "update-index",
            "--add",
            "--cacheinfo",
            f"{mode},{object_id},{alias_path}",
        )
        run_git(alias_repo, "commit", "--quiet", "-m", "case-only tree alias fixture")
        alias_commit = run_git(alias_repo, "rev-parse", "HEAD").decode("ascii").strip()
        alias_tree_paths = {
            record.split(b"\t", 1)[1]
            for record in run_git(alias_repo, "ls-tree", "-r", "-z", alias_commit).split(b"\0")
            if record
        }
        if (
            alias_source_path.encode("utf-8") not in alias_tree_paths
            or alias_path.encode("utf-8") not in alias_tree_paths
        ):
            raise AssertionError("case-only alias was not preserved in the committed Git tree")
        code, result, alias_output = invoke(alias_repo, alias_commit)
        assert_specific_failure(
            "case-only-tree-alias",
            code,
            result,
            error_code="ambiguous_tree_path",
            mismatch_reason="ambiguous_tree_path",
            manifest_count=68,
        )
        code_again, result_again, alias_output_again = invoke(alias_repo, alias_commit)
        if code_again == 0 or result_again != result or alias_output_again != alias_output:
            raise AssertionError("case-only tree alias output was not deterministic")

        missing_repo, _, _ = create_fixture(parent, "missing-source-blob", paths)
        missing_commit = run_git(missing_repo, "rev-parse", "HEAD").decode("ascii").strip()
        missing_source_path = next(path for path in paths if path != ".gitattributes")
        _, missing_type, missing_oid = tree_entry(missing_repo, missing_commit, missing_source_path)
        if missing_type != "blob":
            raise AssertionError("missing-object fixture source is not a blob")
        loose_object_path = (
            missing_repo / ".git" / "objects" / missing_oid[:2] / missing_oid[2:]
        )
        if not loose_object_path.is_file():
            raise AssertionError("missing-object fixture source was not stored as a loose Git object")
        loose_object_path.chmod(0o600)
        loose_object_path.unlink()
        code, result, _ = invoke(missing_repo, missing_commit)
        assert_specific_failure(
            "missing-source-blob",
            code,
            result,
            mismatch_reason="object_unavailable",
            manifest_count=68,
        )
        unavailable_mismatch = any(
            item.get("path") == missing_source_path
            and item.get("blob_oid") == missing_oid
            and item.get("reason") == "object_unavailable"
            for item in result.get("mismatches", [])
            if isinstance(item, dict)
        )
        if not unavailable_mismatch:
            raise AssertionError("missing-object fixture did not identify the unavailable source blob")

        oversized_repo, _, _ = create_fixture(parent, "oversized-source-blob", paths)
        oversized_path = next(path for path in paths if path != ".gitattributes")
        oversized_bytes = b"x" * (16 * 1024 * 1024 + 1)
        oversized_file = oversized_repo.joinpath(*oversized_path.split("/"))
        oversized_file.write_bytes(oversized_bytes)
        manifest_path = oversized_repo / MANIFEST_RELATIVE
        manifest_rows = manifest_path.read_bytes().splitlines()
        replacement = f"{hashlib.sha256(oversized_bytes).hexdigest()}  {oversized_path}".encode("utf-8")
        manifest_rows = [
            replacement if row.endswith(b"  " + oversized_path.encode("utf-8")) else row
            for row in manifest_rows
        ]
        manifest_path.write_bytes(b"\n".join(manifest_rows) + b"\n")
        run_git(oversized_repo, "add", "--all")
        run_git(oversized_repo, "commit", "--quiet", "-m", "oversized source fixture")
        oversized_commit = run_git(oversized_repo, "rev-parse", "HEAD").decode("ascii").strip()
        code, result, _ = invoke(oversized_repo, oversized_commit)
        assert_specific_failure("oversized-source-blob", code, result, error_code="resource_limit")

        symlink_repo, _, _ = create_fixture(parent, "symlink", paths)
        symlink_path = next(path for path in paths if path != ".gitattributes")
        target_oid = run_git(symlink_repo, "hash-object", "-w", "--stdin", input_bytes=b"outside-target").decode("ascii").strip()
        run_git(symlink_repo, "update-index", "--add", "--cacheinfo", f"120000,{target_oid},{symlink_path}")
        run_git(symlink_repo, "commit", "--quiet", "-m", "symlink fixture")
        code, result, _ = invoke(symlink_repo)
        if code == 0 or "not_regular_blob" not in {
            item.get("reason") for item in result.get("mismatches", []) if isinstance(item, dict)
        }:
            raise AssertionError(f"symlink tree entry was not rejected: {result}")

        empty_repo = parent / "not-a-repository"
        empty_repo.mkdir()
        code, result, _ = invoke(empty_repo)
        if code == 0 or "git_command_failed" not in {
            item.get("code") for item in result.get("errors", []) if isinstance(item, dict)
        }:
            raise AssertionError(f"Git command failure was not reported as JSON: {result}")

    print(
        "source fingerprint verifier fixtures passed: valid CRLF object, deterministic output, "
        "digest mismatch, invalid digest, duplicate, malformed, missing path, traversal, "
        "wrong count, path order, case-only Git tree alias, unavailable source blob, "
        "oversized source blob bound, symlink, fabricated commit rejection, SHA-1/SHA-256 "
        "object-format validation, path-depth and tree-entry bounds, Git failure"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
