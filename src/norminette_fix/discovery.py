from __future__ import annotations

import os
import subprocess
from pathlib import Path


def discover(
    targets: list[str],
    *,
    cwd: Path,
    use_gitignore: bool = False,
) -> tuple[list[Path], list[str]]:
    requested = targets or ["."]
    found: dict[Path, Path] = {}
    failures: list[str] = []
    for raw in requested:
        path = Path(raw)
        if not path.is_absolute():
            path = cwd / path
        path = path.absolute()
        if not path.exists():
            failures.append(f"'{raw}' does not exist.")
            continue
        if path.is_symlink():
            failures.append(f"'{raw}' is a symbolic link and was skipped.")
            continue
        if path.is_file():
            if path.suffix not in {".c", ".h"}:
                failures.append(f"'{raw}' is not a .c or .h file.")
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
                if candidate.suffix in {".c", ".h"} and not candidate.is_symlink():
                    found[candidate.absolute()] = candidate.absolute()
    paths = sorted(found.values(), key=lambda item: str(item))
    if use_gitignore:
        paths, ignore_failures = _remove_gitignored(paths)
        failures.extend(ignore_failures)
    return paths, failures


def _remove_gitignored(paths: list[Path]) -> tuple[list[Path], list[str]]:
    included: list[Path] = []
    failures: list[str] = []
    repositories: dict[Path, Path | None] = {}
    for path in paths:
        parent = path.parent
        if parent not in repositories:
            result = subprocess.run(
                ["git", "-C", str(parent), "rev-parse", "--show-toplevel"],
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.DEVNULL,
                check=False,
            )
            repositories[parent] = Path(result.stdout.strip()) if result.returncode == 0 else None
        repository = repositories[parent]
        if repository is None:
            failures.append(
                f"Could not apply --use-gitignore to '{path}': it is not in a Git repository."
            )
            included.append(path)
            continue
        ignored = subprocess.run(
            ["git", "-C", str(repository), "check-ignore", "-q", "--", str(path)],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            check=False,
        )
        if ignored.returncode == 1:
            included.append(path)
        elif ignored.returncode not in {0, 1}:
            failures.append(f"Git could not check ignore rules for '{path}'.")
            included.append(path)
    return included, failures
