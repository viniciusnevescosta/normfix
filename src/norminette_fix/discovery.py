from __future__ import annotations

import os
import subprocess
from dataclasses import dataclass
from pathlib import Path


@dataclass(frozen=True)
class DiscoveryResult:
    paths: list[Path]
    failures: list[str]
    unexpected_files: list[Path]


def discover(
    targets: list[str],
    *,
    cwd: Path,
    use_gitignore: bool = False,
) -> tuple[list[Path], list[str]]:
    result = discover_with_warnings(
        targets,
        cwd=cwd,
        use_gitignore=use_gitignore,
    )
    return result.paths, result.failures


def discover_with_warnings(
    targets: list[str],
    *,
    cwd: Path,
    use_gitignore: bool = False,
) -> DiscoveryResult:
    requested = targets or ["."]
    found: dict[Path, Path] = {}
    unexpected: dict[Path, Path] = {}
    failures: list[str] = []
    for raw in requested:
        path = Path(raw)
        if not path.is_absolute():
            path = cwd / path
        path = path.absolute()
        symlink_component = _first_symlink_component(path)
        if symlink_component is not None:
            failures.append(
                f"'{raw}' passes through symbolic link '{symlink_component}' and was skipped."
            )
            continue
        if not path.exists():
            failures.append(f"'{raw}' does not exist.")
            continue
        if path.is_symlink():
            failures.append(f"'{raw}' is a symbolic link and was skipped.")
            continue
        if path.is_file():
            if not _is_processable_file(path):
                failures.append(f"'{raw}' is not a .c, .h, or Makefile file.")
                continue
            found[path] = path
            continue

        def record_walk_error(error: OSError, scan_path: Path = path) -> None:
            failures.append(
                f"Could not scan '{error.filename or scan_path}': {error.strerror or error}."
            )

        for root, directories, filenames in os.walk(
            path,
            followlinks=False,
            onerror=record_walk_error,
        ):
            directories[:] = sorted(
                name
                for name in directories
                if name != ".git" and not (Path(root) / name).is_symlink()
            )
            for filename in sorted(filenames):
                candidate = Path(root) / filename
                if _is_processable_file(candidate) and not candidate.is_symlink():
                    found[candidate.absolute()] = candidate.absolute()
                elif not _is_expected_project_file(candidate) and not candidate.is_symlink():
                    unexpected[candidate.absolute()] = candidate.absolute()
    paths = sorted(found.values(), key=lambda item: str(item))
    if use_gitignore:
        paths, ignore_failures = _remove_gitignored(paths)
        failures.extend(ignore_failures)
    return DiscoveryResult(
        paths=paths,
        failures=failures,
        unexpected_files=sorted(unexpected.values(), key=lambda item: str(item)),
    )


def _is_processable_file(path: Path) -> bool:
    return path.suffix in {".c", ".h"} or path.name.casefold() == "makefile"


def _is_expected_project_file(path: Path) -> bool:
    name = path.name.casefold()
    return (
        _is_processable_file(path)
        or name == "readme"
        or name.startswith("readme.")
    )


def _first_symlink_component(path: Path) -> Path | None:
    absolute = path.absolute()
    parts = absolute.parts
    if not parts:
        return None
    current = Path(parts[0])
    for part in parts[1:]:
        current /= part
        if current.is_symlink():
            # macOS and some Unix layouts expose system roots such as /var,
            # /tmp or /home through a root-level compatibility symlink. Trust
            # that filesystem prefix, but refuse any symlink below it.
            if current.parent == Path(current.anchor):
                continue
            return current
    return None


def _remove_gitignored(paths: list[Path]) -> tuple[list[Path], list[str]]:
    included = [True] * len(paths)
    failures_by_index: list[list[str]] = [[] for _path in paths]
    repository_groups: dict[Path, list[tuple[int, Path]]] = {}
    marker_cache: dict[Path, Path | None] = {}
    lookup_groups: dict[tuple[str, Path], list[tuple[int, Path]]] = {}

    for index, path in enumerate(paths):
        parent = path.parent
        marker = _nearest_git_marker(parent, marker_cache)
        lookup_key = ("marker", marker) if marker is not None else ("parent", parent)
        lookup_groups.setdefault(lookup_key, []).append((index, path))

    for entries in lookup_groups.values():
        probe_directory = entries[0][1].parent
        result = subprocess.run(
            ["git", "-C", str(probe_directory), "rev-parse", "--show-toplevel"],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            check=False,
        )
        repository = Path(result.stdout.strip()) if result.returncode == 0 else None
        if repository is None:
            for index, path in entries:
                failures_by_index[index].append(
                    f"Could not apply --use-gitignore to '{path}': "
                    "it is not in a Git repository."
                )
            continue
        repository_groups.setdefault(repository, []).extend(entries)

    for repository, entries in repository_groups.items():
        entries.sort(key=lambda item: item[0])
        ignored = _batch_gitignored(repository, [path for _index, path in entries])
        if ignored is not None:
            for index, path in entries:
                if os.fsencode(path) in ignored:
                    included[index] = False
            continue
        for index, path in entries:
            result = subprocess.run(
                ["git", "-C", str(repository), "check-ignore", "-q", "--", str(path)],
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
                check=False,
            )
            if result.returncode == 0:
                included[index] = False
            elif result.returncode not in {0, 1}:
                failures_by_index[index].append(
                    f"Git could not check ignore rules for '{path}'."
                )

    failures = [failure for path_failures in failures_by_index for failure in path_failures]
    return [path for index, path in enumerate(paths) if included[index]], failures


def _nearest_git_marker(
    directory: Path,
    cache: dict[Path, Path | None],
) -> Path | None:
    current = directory.absolute()
    visited: list[Path] = []
    marker: Path | None
    while current not in cache:
        visited.append(current)
        if (current / ".git").exists():
            marker = current
            break
        parent = current.parent
        if parent == current:
            marker = None
            break
        current = parent
    else:
        marker = cache[current]
    for path in visited:
        cache[path] = marker
    return marker


def _batch_gitignored(repository: Path, paths: list[Path]) -> set[bytes] | None:
    encoded_paths = [os.fsencode(path) for path in paths]
    result = subprocess.run(
        ["git", "-C", str(repository), "check-ignore", "--stdin", "-z"],
        input=b"".join(path + b"\0" for path in encoded_paths),
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        check=False,
    )
    if result.returncode not in {0, 1} or not isinstance(result.stdout, bytes):
        return None
    if result.stdout and not result.stdout.endswith(b"\0"):
        return None
    ignored = set(result.stdout.split(b"\0")[:-1]) if result.stdout else set()
    expected_returncode = 0 if ignored else 1
    if result.returncode != expected_returncode or not ignored.issubset(encoded_paths):
        return None
    return ignored
