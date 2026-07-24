from __future__ import annotations

import hashlib
import os
import re
import subprocess
from collections import Counter
from dataclasses import dataclass, replace
from pathlib import Path

from .header import HeaderGuardRename, expected_guard, header_guard_rename_candidate

_MAX_GUARD_CANDIDATES = 256
_MAX_PROJECT_FILES = 25_000
_MAX_PROJECT_BYTES = 512 * 1024 * 1024
_STANDARD_LINE_SPLICE = re.compile(rb"\\[ \t]*(?:\r\n|\n|\r)")
_TRIGRAPH_LINE_SPLICE = re.compile(rb"\?\?/[ \t]*(?:\r\n|\n|\r)")
_BUILD_CONFIG_NAMES = frozenset(
    {
        "build",
        "build.bazel",
        "build.ninja",
        "cmakelists.txt",
        "compile_commands.json",
        "compile_flags.txt",
        "configure.ac",
        "gnumakefile",
        "makefile",
        "meson.build",
        "meson_options.txt",
        "project.pbxproj",
        "sconscript",
        "sconstruct",
        "workspace",
        "workspace.bazel",
        "xmake.lua",
    }
)
_BUILD_CONFIG_SUFFIXES = frozenset(
    {
        ".bazel",
        ".bzl",
        ".cmake",
        ".flags",
        ".gn",
        ".gni",
        ".mk",
        ".ninja",
        ".pri",
        ".pro",
        ".props",
        ".rsp",
        ".targets",
        ".vcxproj",
        ".xcconfig",
    }
)
_BUILD_DEFINITION_MECHANISM = re.compile(
    rb"(?i)\b(?:"
    rb"add_compile_definitions|add_compile_options|add_defines|add_definitions|"
    rb"add_global_arguments|add_project_arguments|compile_definitions|"
    rb"compile_options|copts|cpp_args|c_args|defines|define_symbols|"
    rb"gcc_preprocessor_definitions|local_defines|preprocessor_definitions|"
    rb"target_compile_definitions|target_compile_options"
    rb")\b"
)
_COMPILER_DEFINE_OPTION = re.compile(rb"(?<![A-Za-z0-9_])(?:-D|/D)(?=[ \t\"'$({A-Za-z_])")
_DYNAMIC_MAKE_FLAGS = re.compile(
    rb"(?im)^[ \t]*(?:CPPFLAGS|CFLAGS)[ \t]*[+:?]?="
    rb"[^\r\n]*(?:\$\(|\$\{)"
)


@dataclass(frozen=True)
class _ProjectScope:
    root: Path
    paths: tuple[Path, ...]


@dataclass(frozen=True)
class GuardApproval:
    rename: HeaderGuardRename
    root: Path
    project_snapshot: ProjectSnapshot


@dataclass(frozen=True)
class ProjectSnapshot:
    digest: str
    records: tuple[tuple[str, int, int, int, int, int], ...]


def plan_header_guard_renames(paths: list[Path]) -> dict[Path, GuardApproval]:
    """Approve only guard renames whose complete project scope has no conflicts."""
    approvals: dict[Path, GuardApproval] = {}
    for scope in _project_scopes(paths):
        candidates = _guard_candidates(scope.paths)
        if not candidates or len(candidates) > _MAX_GUARD_CANDIDATES:
            continue
        project_files = _project_files(scope)
        if project_files is None:
            continue
        snapshot_before = _project_snapshot(project_files)
        if snapshot_before is None:
            continue
        expected_files = Counter(
            expected_guard(path.name) for path in project_files if path.suffix == ".h"
        )
        names = {
            name
            for candidate in candidates.values()
            for name in (candidate.current, candidate.expected)
        }
        references = _identifier_references(project_files, names)
        if references is None:
            continue
        snapshot_after = _project_snapshot(project_files)
        if snapshot_after is None or snapshot_after != snapshot_before:
            continue
        totals, by_file = references
        expected_claims = Counter(candidate.expected for candidate in candidates.values())
        for path, candidate in candidates.items():
            if expected_claims[candidate.expected] != 1:
                continue
            if expected_files[candidate.expected] != 1:
                continue
            if totals[candidate.expected] != 0:
                continue
            if totals[candidate.current] != 2:
                continue
            if by_file.get(path, Counter())[candidate.current] != 2:
                continue
            approvals[path] = GuardApproval(
                rename=candidate,
                root=scope.root,
                project_snapshot=snapshot_after,
            )
    return approvals


def guard_approval_is_current(approval: GuardApproval) -> bool:
    files = _walk_project_files(approval.root)
    if files is None:
        return False
    return _project_snapshot(files) == approval.project_snapshot


def advance_guard_approvals_after_write(
    approvals: dict[Path, GuardApproval],
    written_path: Path,
) -> dict[Path, GuardApproval]:
    """Advance snapshots after one verified, engine-owned atomic write.

    Every other project file must still match the approved snapshot. The
    written path itself is allowed to have new metadata because its exact
    contents were just supplied to the atomic writer.
    """

    canonical = written_path.resolve()
    roots = {
        approval.root
        for approval in approvals.values()
        if any(record[0] == str(canonical) for record in approval.project_snapshot.records)
    }
    if not roots:
        return approvals
    refreshed = dict(approvals)
    for root in roots:
        scoped = {path: approval for path, approval in refreshed.items() if approval.root == root}
        previous = next(iter(scoped.values())).project_snapshot
        files = _walk_project_files(root)
        current = _project_snapshot(files) if files is not None else None
        if current is None or not _only_written_path_changed(
            previous,
            current,
            canonical,
        ):
            refreshed = {
                path: approval for path, approval in refreshed.items() if approval.root != root
            }
            continue
        for path, approval in scoped.items():
            refreshed[path] = replace(approval, project_snapshot=current)
    return refreshed


def _only_written_path_changed(
    previous: ProjectSnapshot,
    current: ProjectSnapshot,
    written_path: Path,
) -> bool:
    old_records = {record[0]: record[1:] for record in previous.records}
    new_records = {record[0]: record[1:] for record in current.records}
    canonical = str(written_path)
    if old_records.keys() != new_records.keys() or canonical not in old_records:
        return False
    return all(
        new_records[path] == metadata for path, metadata in old_records.items() if path != canonical
    )


def _project_scopes(paths: list[Path]) -> list[_ProjectScope]:
    normalized = sorted({path.resolve() for path in paths}, key=str)
    git_groups: dict[Path, list[Path]] = {}
    roots: dict[Path, Path | None] = {}
    for path in normalized:
        parent = path.parent
        if parent not in roots:
            roots[parent] = _git_root(parent)
        root = roots[parent]
        if root is not None:
            git_groups.setdefault(root, []).append(path)

    return [
        _ProjectScope(root, tuple(group))
        for root, group in sorted(git_groups.items(), key=lambda item: str(item[0]))
    ]


def _git_root(directory: Path) -> Path | None:
    try:
        result = subprocess.run(
            ["git", "-C", str(directory), "rev-parse", "--show-toplevel"],
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            check=False,
            timeout=2,
        )
    except (OSError, subprocess.TimeoutExpired):
        return None
    if result.returncode != 0:
        return None
    try:
        root = Path(os.fsdecode(result.stdout).strip()).resolve()
    except (TypeError, ValueError):
        return None
    return root if root != root.parent else None


def _guard_candidates(paths: tuple[Path, ...]) -> dict[Path, HeaderGuardRename]:
    candidates: dict[Path, HeaderGuardRename] = {}
    for path in paths:
        if path.suffix != ".h" or path.is_symlink():
            continue
        try:
            source = path.read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError):
            continue
        candidate = header_guard_rename_candidate(source, path.name)
        if candidate is not None:
            candidates[path] = candidate
    return candidates


def _project_files(scope: _ProjectScope) -> set[Path] | None:
    files = _walk_project_files(scope.root)
    if files is None:
        return None
    files.update(scope.paths)
    return files


def _walk_project_files(root: Path) -> set[Path] | None:
    files: set[Path] = set()
    total_bytes = 0
    failed = False

    def record_error(_error: OSError) -> None:
        nonlocal failed
        failed = True

    for current, directories, filenames in os.walk(
        root,
        followlinks=False,
        onerror=record_error,
    ):
        current_path = Path(current)
        if any((current_path / name).is_symlink() for name in directories):
            return None
        metadata_names = (".git", ".hg", ".svn")
        directories[:] = [
            name
            for name in directories
            if name not in metadata_names
            and not (current_path.name in {".claude", ".codex"} and name == "worktrees")
        ]
        for name in filenames:
            path = (current_path / name).absolute()
            if path.is_symlink():
                return None
            try:
                total_bytes += path.stat().st_size
            except OSError:
                return None
            if len(files) >= _MAX_PROJECT_FILES or total_bytes > _MAX_PROJECT_BYTES:
                return None
            files.add(path)
    return None if failed else files


def _identifier_references(
    paths: set[Path],
    names: set[str],
) -> tuple[Counter[str], dict[Path, Counter[str]]] | None:
    alternatives = b"|".join(
        re.escape(name.encode("ascii"))
        for name in sorted(names, key=lambda item: (-len(item), item))
    )
    pattern = re.compile(rb"(?<![A-Za-z0-9_])(?:[-/][DU])?(" + alternatives + rb")(?![A-Za-z0-9_])")
    totals: Counter[str] = Counter()
    by_file: dict[Path, Counter[str]] = {}
    scanned_bytes = 0
    for path in sorted(paths, key=str):
        if not path.exists():
            continue
        if path.is_symlink() or not path.is_file():
            return None
        try:
            data = path.read_bytes()
        except OSError:
            return None
        scanned_bytes += len(data)
        if scanned_bytes > _MAX_PROJECT_BYTES:
            return None
        logical = _STANDARD_LINE_SPLICE.sub(
            b"",
            _TRIGRAPH_LINE_SPLICE.sub(b"", data),
        )
        if _has_build_definition_mechanism(logical, path):
            return None
        if b"##" in logical or b"%:%:" in logical or b"??=" in logical or b"\\#" in logical:
            if _has_build_token_paste_definition(logical, path):
                return None
            code = _without_comments_and_literals(logical)
            if _has_token_paste_definition(code):
                return None
        counts = Counter(match.group(1).decode("ascii") for match in pattern.finditer(logical))
        if counts:
            by_file[path] = counts
            totals.update(counts)
    return totals, by_file


def _has_build_definition_mechanism(data: bytes, path: Path) -> bool:
    if not _is_build_configuration(path):
        return False
    uncommented = _without_hash_comments(data)
    return any(
        pattern.search(uncommented) is not None
        for pattern in (
            _BUILD_DEFINITION_MECHANISM,
            _COMPILER_DEFINE_OPTION,
            _DYNAMIC_MAKE_FLAGS,
        )
    )


def _has_build_token_paste_definition(data: bytes, path: Path) -> bool:
    translated = data.replace(b"??=", b"#").replace(b"%:%:", b"##")
    unescaped = _unescape_build_punctuation(translated)
    if b"##" not in unescaped:
        return False
    definition = re.compile(
        rb"(?<![A-Za-z0-9_])(?:-D|/D)[ \t]*[\"']?"
        rb"[A-Za-z_][A-Za-z0-9_]*(?:\([^\r\n)]*\))?[ \t]*="
        rb"[^\r\n]*##"
    )
    if definition.search(unescaped) is not None:
        return True
    if not _is_build_configuration(path):
        return False
    uncommented = _unescape_build_punctuation(_without_hash_comments(translated))
    return b"##" in uncommented


def _unescape_build_punctuation(data: bytes) -> bytes:
    for punctuation in b"#(),=":
        data = data.replace(b"\\" + bytes([punctuation]), bytes([punctuation]))
    return data.replace(b"$#", b"#")


def _without_hash_comments(data: bytes) -> bytes:
    masked = bytearray(data)
    quote: int | None = None
    bracket_close: bytes | None = None
    index = 0
    while index < len(data):
        if bracket_close is not None:
            if data.startswith(bracket_close, index):
                index += len(bracket_close)
                bracket_close = None
            else:
                index += 1
            continue
        byte = data[index]
        if quote is not None:
            if byte == ord("\\") and index + 1 < len(data):
                index += 2
                continue
            if byte == quote:
                quote = None
            index += 1
            continue
        if byte in {ord('"'), ord("'")}:
            quote = byte
            index += 1
            continue
        if byte == ord("["):
            bracket = re.match(rb"\[(=*)\[", data[index:])
            if bracket is not None:
                bracket_close = b"]" + bracket.group(1) + b"]"
                index += bracket.end()
                continue
        if byte in {ord("\\"), ord("$")} and index + 1 < len(data):
            index += 2
            continue
        if byte == ord("#"):
            end = index
            while end < len(data) and data[end] not in {ord("\n"), ord("\r")}:
                masked[end] = ord(" ")
                end += 1
            index = end
            continue
        index += 1
    return bytes(masked)


def _is_build_configuration(path: Path) -> bool:
    name = path.name.lower()
    return (
        name in _BUILD_CONFIG_NAMES
        or name.startswith(("cmake", "makefile"))
        or path.suffix.lower() in _BUILD_CONFIG_SUFFIXES
    )


def _has_token_paste_definition(code: bytes) -> bool:
    translated = code.replace(b"??=", b"#").replace(b"%:%:", b"##")
    for line in translated.splitlines():
        directive = line.lstrip(b" \t")
        if directive.startswith(b"#"):
            directive = directive[1:].lstrip(b" \t")
        elif directive.startswith(b"%:"):
            directive = directive[2:].lstrip(b" \t")
        else:
            continue
        if not directive.startswith(b"define"):
            continue
        if len(directive) > len(b"define") and directive[len(b"define")] in (
            b"_0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz"
        ):
            continue
        if b"##" in directive[len(b"define") :]:
            return True
    return False


def _without_comments_and_literals(data: bytes) -> bytes:
    """Mask bytes where preprocessing operators cannot be active tokens."""

    masked = bytearray(data)
    state = "code"
    index = 0
    while index < len(data):
        byte = data[index]
        following = data[index + 1] if index + 1 < len(data) else None
        if state == "code":
            if byte == ord("/") and following == ord("*"):
                masked[index : index + 2] = b"  "
                state = "block-comment"
                index += 2
                continue
            if byte == ord("/") and following == ord("/"):
                masked[index : index + 2] = b"  "
                state = "line-comment"
                index += 2
                continue
            if byte == ord('"'):
                masked[index] = ord(" ")
                state = "string"
            elif byte == ord("'"):
                masked[index] = ord(" ")
                state = "character"
        elif state == "line-comment":
            if byte in {ord("\n"), ord("\r")}:
                state = "code"
            else:
                masked[index] = ord(" ")
        elif state == "block-comment":
            if byte == ord("*") and following == ord("/"):
                masked[index : index + 2] = b"  "
                state = "code"
                index += 2
                continue
            if byte not in {ord("\n"), ord("\r")}:
                masked[index] = ord(" ")
        else:
            masked[index] = ord(" ")
            delimiter = ord('"') if state == "string" else ord("'")
            if byte == ord("\\") and following is not None:
                if following not in {ord("\n"), ord("\r")}:
                    masked[index + 1] = ord(" ")
                index += 2
                continue
            if byte == delimiter:
                state = "code"
        index += 1
    return bytes(masked)


def _project_snapshot(paths: set[Path]) -> ProjectSnapshot | None:
    digest = hashlib.sha256()
    records: list[tuple[str, int, int, int, int, int]] = []
    for path in sorted(paths, key=str):
        if path.is_symlink() or not path.is_file():
            return None
        try:
            metadata = path.stat()
        except OSError:
            return None
        record = (
            str(path),
            metadata.st_dev,
            metadata.st_ino,
            metadata.st_size,
            metadata.st_mtime_ns,
            metadata.st_ctime_ns,
        )
        records.append(record)
        digest.update(os.fsencode(record[0]))
        digest.update(b"\0")
        digest.update(
            (f"{record[1]}:{record[2]}:{record[3]}:{record[4]}:{record[5]}").encode("ascii")
        )
        digest.update(b"\0")
    return ProjectSnapshot(digest.hexdigest(), tuple(records))
