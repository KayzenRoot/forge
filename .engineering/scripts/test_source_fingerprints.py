#!/usr/bin/env python3
"""Run isolated Git-object fixtures for the source fingerprint verifier."""

from __future__ import annotations

import hashlib
import json
import re
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Callable


ROOT = Path(__file__).resolve().parents[2]
VERIFIER = Path(__file__).with_name("verify_source_fingerprints.py")
MANIFEST_RELATIVE = ".engineering/evidence/FGE-004-M00/SOURCE-FINGERPRINTS.sha256"
ROW_PATTERN = re.compile(rb"^([0-9a-f]{64})  (.+)$")


def run_git(repo: Path, *args: str, input_bytes: bytes | None = None) -> bytes:
    command = ["git", "--no-replace-objects", "-C", str(repo), *args]
    completed = subprocess.run(
        command,
        input=input_bytes,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
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
) -> tuple[Path, list[bytes], dict[str, bytes]]:
    repo = parent / name
    repo.mkdir()
    run_git(repo, "init", "--quiet", "--initial-branch=main")
    run_git(repo, "config", "core.autocrlf", "false")
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


def main() -> int:
    paths = source_paths()
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
        code_again, result_again, second_output = invoke(valid_repo, valid_commit)
        if code_again != 0 or first_output != second_output or result_again != result:
            raise AssertionError("verifier output was not deterministic for an immutable Git tree")

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

    print("source fingerprint verifier fixtures passed: valid CRLF object, deterministic output, digest mismatch, invalid digest, duplicate, malformed, missing path, traversal, wrong count, symlink, Git failure")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
