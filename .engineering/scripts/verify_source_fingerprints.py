#!/usr/bin/env python3
"""Verify the admitted source manifest against raw blobs in one Git commit."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import subprocess
import sys
import unicodedata
from pathlib import Path
from typing import Any


MANIFEST_PATH = ".engineering/evidence/FGE-004-M00/SOURCE-FINGERPRINTS.sha256"
EXPECTED_ENTRY_COUNT = 68
# SHA-256 of the admitted path sequence joined by NUL bytes, in manifest order.
EXPECTED_PATH_SEQUENCE_SHA256 = "4e7866cb3d1f6dc10f0112d4e0cfd8c0aebe71b2f2fd07c5b185ee61c11a7ffa"
ROW_PATTERN = re.compile(rb"^([0-9a-fA-F]{64})  (.+)$")
OBJECT_ID_PATTERN = re.compile(r"^(?:[0-9a-f]{40}|[0-9a-f]{64})$")
RESERVED_WINDOWS_NAMES = {
    "CON",
    "PRN",
    "AUX",
    "NUL",
    *(f"COM{number}" for number in range(1, 10)),
    *(f"LPT{number}" for number in range(1, 10)),
}
EXCLUDED_SOURCE_PREFIXES = (
    ".engineering/evidence/",
    ".engineering/work-orders/",
    ".engineering/scripts/",
)


class GitCommandError(Exception):
    def __init__(self, step: str, returncode: int) -> None:
        super().__init__(f"git {step} failed with exit code {returncode}")
        self.step = step
        self.returncode = returncode


class GitExecutableError(Exception):
    pass


def run_git(repo: Path, args: list[str], input_bytes: bytes | None = None) -> bytes:
    environment = os.environ.copy()
    for key in (
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_COMMON_DIR",
        "GIT_INDEX_FILE",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        "GIT_CEILING_DIRECTORIES",
        "GIT_PREFIX",
    ):
        environment.pop(key, None)
    environment["GIT_TERMINAL_PROMPT"] = "0"
    environment["GIT_NO_LAZY_FETCH"] = "1"
    environment["GIT_OPTIONAL_LOCKS"] = "0"
    repo_value = repo.as_posix()
    command = [
        "git",
        "-c",
        f"safe.directory={repo_value}",
        "--no-replace-objects",
        "-C",
        repo_value,
        *args,
    ]
    try:
        completed = subprocess.run(
            command,
            input=input_bytes,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
            env=environment,
        )
    except FileNotFoundError as exc:
        raise GitExecutableError("git executable is unavailable") from exc
    if completed.returncode != 0:
        step = args[0] if args else "command"
        raise GitCommandError(step, completed.returncode)
    return completed.stdout


def safe_manifest_path(path_bytes: bytes) -> tuple[str | None, str | None]:
    try:
        path = path_bytes.decode("utf-8", "strict")
    except UnicodeDecodeError:
        return None, "path is not valid UTF-8"

    if not path or unicodedata.normalize("NFC", path) != path:
        return path, "path is empty or not Unicode NFC"
    if path.startswith("/") or "\\" in path or ":" in path:
        return path, "absolute, drive-qualified, or backslash path is not allowed"
    if any(ord(character) < 0x20 or ord(character) == 0x7F for character in path):
        return path, "control characters are not allowed in paths"

    parts = path.split("/")
    if any(part in ("", ".", "..") for part in parts):
        return path, "empty, dot, or parent path component is not allowed"
    if any(part != part.rstrip(" .") for part in parts):
        return path, "path components ending in a dot or space are ambiguous"
    if any(part.split(".", 1)[0].upper() in RESERVED_WINDOWS_NAMES for part in parts):
        return path, "reserved Windows device name is not allowed"
    if path == MANIFEST_PATH or path.startswith(EXCLUDED_SOURCE_PREFIXES):
        return path, "manifest, evidence, work-order, and verifier files are outside the source set"
    return path, None


def parse_tree(data: bytes) -> tuple[dict[bytes, dict[str, str]], list[dict[str, str]]]:
    entries: dict[bytes, dict[str, str]] = {}
    errors: list[dict[str, str]] = []
    for record in data.split(b"\0"):
        if not record:
            continue
        try:
            metadata, path = record.split(b"\t", 1)
            mode, object_type, object_id = metadata.split(b" ", 2)
            entry = {
                "mode": mode.decode("ascii"),
                "type": object_type.decode("ascii"),
                "oid": object_id.decode("ascii"),
            }
        except (ValueError, UnicodeDecodeError):
            errors.append({"code": "invalid_tree_entry", "message": "Git returned a malformed tree entry"})
            continue
        if path in entries:
            errors.append({"code": "duplicate_tree_path", "message": "Git tree contains an ambiguous duplicate path"})
            continue
        entries[path] = entry
    return entries, errors


def read_object_batch(
    repo: Path, object_ids: list[str]
) -> tuple[dict[str, bytes], set[str]]:
    if not object_ids:
        return {}, set()
    payload = b"\n".join(oid.encode("ascii") for oid in object_ids) + b"\n"
    output = run_git(repo, ["cat-file", "--batch"], payload)
    objects: dict[str, bytes] = {}
    missing: set[str] = set()
    offset = 0

    for expected_oid in object_ids:
        header_end = output.find(b"\n", offset)
        if header_end < 0:
            raise GitExecutableError("git cat-file returned a truncated batch header")
        header = output[offset:header_end].split()
        offset = header_end + 1
        if len(header) == 2 and header[1] == b"missing":
            missing.add(expected_oid)
            continue
        if len(header) != 3:
            raise GitExecutableError("git cat-file returned an invalid batch header")
        try:
            returned_oid = header[0].decode("ascii").lower()
            object_type = header[1].decode("ascii")
            size = int(header[2].decode("ascii"))
        except (UnicodeDecodeError, ValueError) as exc:
            raise GitExecutableError("git cat-file returned invalid object metadata") from exc
        if returned_oid != expected_oid.lower() or size < 0:
            raise GitExecutableError("git cat-file returned unexpected object metadata")
        end = offset + size
        if end > len(output) or output[end : end + 1] != b"\n":
            raise GitExecutableError("git cat-file returned a truncated object")
        if object_type == "blob":
            objects[expected_oid] = output[offset:end]
        else:
            missing.add(expected_oid)
        offset = end + 1

    if offset != len(output):
        raise GitExecutableError("git cat-file returned trailing batch data")
    return objects, missing


def emit_result(result: dict[str, Any], exit_code: int) -> int:
    sys.stdout.write(json.dumps(result, ensure_ascii=True, sort_keys=True, separators=(",", ":")) + "\n")
    return exit_code


def base_result(commit_sha: str | None = None, tree_sha: str | None = None) -> dict[str, Any]:
    return {
        "commit_sha": commit_sha,
        "tree_sha": tree_sha,
        "manifest_count": 0,
        "pass": False,
        "mismatches": [],
        "errors": [],
    }


def verify(repo_argument: str | None, revision: str) -> tuple[dict[str, Any], int]:
    if any(ord(character) < 0x20 or ord(character) == 0x7F for character in revision):
        result = base_result()
        result["errors"].append({"code": "invalid_revision", "message": "revision contains a control character"})
        return result, 2

    repo = Path(repo_argument).expanduser() if repo_argument else Path(__file__).resolve().parents[2]
    try:
        repo = repo.resolve(strict=True)
        if not repo.is_dir():
            raise OSError("repository path is not a directory")
    except OSError:
        result = base_result()
        result["errors"].append({"code": "invalid_repository", "message": "repository directory is unavailable"})
        return result, 2

    result = base_result()
    try:
        resolved_commit = run_git(
            repo,
            ["rev-parse", "--verify", "--end-of-options", f"{revision}^{{commit}}"],
        ).decode("ascii").strip().lower()
        if not OBJECT_ID_PATTERN.fullmatch(resolved_commit):
            raise GitExecutableError("git returned an invalid commit object ID")
        result["commit_sha"] = resolved_commit
        tree_sha = run_git(
            repo,
            ["rev-parse", "--verify", "--end-of-options", f"{resolved_commit}^{{tree}}"],
        ).decode("ascii").strip().lower()
        if not OBJECT_ID_PATTERN.fullmatch(tree_sha):
            raise GitExecutableError("git returned an invalid tree object ID")
        result["tree_sha"] = tree_sha
        tree_output = run_git(repo, ["ls-tree", "-r", "-z", "--full-tree", resolved_commit])
        tree, tree_errors = parse_tree(tree_output)
        result["errors"].extend(tree_errors)

        manifest_entry = tree.get(MANIFEST_PATH.encode("utf-8"))
        if manifest_entry is None:
            result["errors"].append({"code": "manifest_missing", "message": "manifest is missing from the selected Git tree"})
            return result, 1
        if manifest_entry["type"] != "blob" or manifest_entry["mode"] not in ("100644", "100755"):
            result["errors"].append({"code": "manifest_not_regular", "message": "manifest is not a regular Git blob"})
            return result, 1

        manifest_objects, missing_manifest_objects = read_object_batch(repo, [manifest_entry["oid"]])
        if manifest_entry["oid"] in missing_manifest_objects or manifest_entry["oid"] not in manifest_objects:
            result["errors"].append({"code": "manifest_object_missing", "message": "manifest blob is unavailable in the Git object database"})
            return result, 1

        manifest_bytes = manifest_objects[manifest_entry["oid"]]
        raw_lines = manifest_bytes.split(b"\n")
        if raw_lines and raw_lines[-1] == b"":
            raw_lines.pop()
        result["manifest_count"] = len(raw_lines)
        records: list[tuple[int, str, bytes, bytes]] = []
        path_bytes_seen: set[bytes] = set()
        folded_paths_seen: dict[str, bytes] = {}
        valid_paths: list[bytes] = []

        for line_number, line in enumerate(raw_lines, start=1):
            match = ROW_PATTERN.fullmatch(line)
            if match is None:
                result["errors"].append(
                    {"code": "malformed_row", "message": f"manifest row {line_number} is malformed"}
                )
                continue
            expected_sha256 = match.group(1).decode("ascii").lower()
            path_bytes = match.group(2)
            path, path_error = safe_manifest_path(path_bytes)
            if path_error is not None or path is None:
                result["errors"].append(
                    {"code": "unsafe_path", "message": f"manifest row {line_number}: {path_error or 'invalid path'}"}
                )
                result["mismatches"].append(
                    {
                        "path": path,
                        "expected_sha256": expected_sha256,
                        "actual_sha256": None,
                        "reason": "unsafe_path",
                    }
                )
                continue
            valid_paths.append(path_bytes)
            folded = path.casefold()
            previous = folded_paths_seen.get(folded)
            if path_bytes in path_bytes_seen or (previous is not None and previous != path_bytes):
                result["errors"].append(
                    {"code": "duplicate_path", "message": f"manifest row {line_number} duplicates or aliases a path"}
                )
                result["mismatches"].append(
                    {
                        "path": path,
                        "expected_sha256": expected_sha256,
                        "actual_sha256": None,
                        "reason": "duplicate_path",
                    }
                )
                continue
            path_bytes_seen.add(path_bytes)
            folded_paths_seen[folded] = path_bytes
            records.append((line_number, path, path_bytes, expected_sha256.encode("ascii")))

        if result["manifest_count"] != EXPECTED_ENTRY_COUNT or len(records) != EXPECTED_ENTRY_COUNT:
            result["errors"].append(
                {
                    "code": "wrong_entry_count",
                    "message": f"expected {EXPECTED_ENTRY_COUNT} valid entries; found {len(records)}",
                }
            )
        if len(valid_paths) == EXPECTED_ENTRY_COUNT:
            path_digest = hashlib.sha256(b"\0".join(valid_paths)).hexdigest()
            if path_digest != EXPECTED_PATH_SEQUENCE_SHA256:
                result["errors"].append(
                    {
                        "code": "path_list_mismatch",
                        "message": "manifest paths or order differ from the admitted 68-path inventory",
                    }
                )

        tree_path_folded: dict[str, set[bytes]] = {}
        for tree_path in tree:
            try:
                decoded_tree_path = tree_path.decode("utf-8", "strict")
            except UnicodeDecodeError:
                continue
            tree_path_folded.setdefault(decoded_tree_path.casefold(), set()).add(tree_path)
        for _, path, path_bytes, expected in records:
            aliases = tree_path_folded.get(path.casefold(), set())
            if len(aliases) > 1 and path_bytes in aliases:
                result["errors"].append(
                    {"code": "ambiguous_tree_path", "message": f"manifest path has a case-insensitive Git tree alias: {path}"}
                )
                result["mismatches"].append(
                    {
                        "path": path,
                        "expected_sha256": expected.decode("ascii"),
                        "actual_sha256": None,
                        "reason": "ambiguous_tree_path",
                    }
                )

        object_ids: list[str] = []
        for _, path, path_bytes, expected in records:
            entry = tree.get(path_bytes)
            if entry is None:
                result["mismatches"].append(
                    {
                        "path": path,
                        "expected_sha256": expected.decode("ascii"),
                        "actual_sha256": None,
                        "reason": "missing_path",
                    }
                )
                continue
            if entry["type"] != "blob" or entry["mode"] not in ("100644", "100755"):
                result["mismatches"].append(
                    {
                        "path": path,
                        "expected_sha256": expected.decode("ascii"),
                        "actual_sha256": None,
                        "reason": "not_regular_blob",
                    }
                )
                continue
            if entry["oid"] not in object_ids:
                object_ids.append(entry["oid"])

        source_objects, missing_source_objects = read_object_batch(repo, object_ids)
        for _, path, path_bytes, expected in records:
            entry = tree.get(path_bytes)
            if entry is None or entry["type"] != "blob" or entry["mode"] not in ("100644", "100755"):
                continue
            if entry["oid"] in missing_source_objects or entry["oid"] not in source_objects:
                result["mismatches"].append(
                    {
                        "path": path,
                        "expected_sha256": expected.decode("ascii"),
                        "actual_sha256": None,
                        "blob_oid": entry["oid"],
                        "reason": "object_unavailable",
                    }
                )
                continue
            actual = hashlib.sha256(source_objects[entry["oid"]]).hexdigest()
            expected_text = expected.decode("ascii")
            if actual != expected_text:
                result["mismatches"].append(
                    {
                        "path": path,
                        "expected_sha256": expected_text,
                        "actual_sha256": actual,
                        "blob_oid": entry["oid"],
                        "reason": "digest_mismatch",
                    }
                )

        result["pass"] = not result["errors"] and not result["mismatches"]
        return result, 0 if result["pass"] else 1
    except GitCommandError as exc:
        result["errors"].append({"code": "git_command_failed", "message": str(exc)})
        return result, 2
    except GitExecutableError as exc:
        result["errors"].append({"code": "git_output_invalid", "message": str(exc)})
        return result, 2


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", help="Git repository path; defaults to this script's repository")
    parser.add_argument("--commit", default="HEAD", help="commit or ref whose tree contains the manifest (default: HEAD)")
    args = parser.parse_args()
    result, exit_code = verify(args.repo, args.commit)
    return emit_result(result, exit_code)


if __name__ == "__main__":
    raise SystemExit(main())
