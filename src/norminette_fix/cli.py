from __future__ import annotations

import argparse
import os
import sys
from pathlib import Path

from . import __version__
from .discovery import discover
from .engine import EngineOptions, FixEngine
from .header import resolve_identity
from .report import Reporter


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="norminette-fix",
        description=(
            "Recursively fix safe 42 Norm issues in .c/.h files and report "
            "structural problems that require manual refactoring."
        ),
    )
    parser.add_argument(
        "paths",
        nargs="*",
        metavar="PATH",
        help=(
            "One or more C/header files or directories. Without arguments, "
            "the current directory is scanned recursively."
        ),
    )
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument(
        "--check",
        action="store_true",
        help="Do not write files; exit 1 if fixes or manual issues remain.",
    )
    mode.add_argument(
        "--diff",
        action="store_true",
        help="Do not write files; print a unified diff of proposed fixes.",
    )
    parser.add_argument(
        "--use-gitignore",
        action="store_true",
        help="Skip directory-discovered files ignored by Git.",
    )
    parser.add_argument(
        "--login",
        help="42 login used when inserting/updating official headers.",
    )
    parser.add_argument(
        "--email",
        help="Email used in the official 42 header.",
    )
    parser.add_argument(
        "--no-backup",
        action="store_true",
        help="Do not save originals in the external backup store before writing.",
    )
    parser.add_argument(
        "--backup-dir",
        type=Path,
        help="Use a custom external directory for backups.",
    )
    parser.add_argument(
        "--format",
        choices=("human", "json"),
        default="human",
        help="Output format (default: human).",
    )
    parser.add_argument(
        "--no-color",
        action="store_true",
        help="Disable colored terminal output.",
    )
    parser.add_argument(
        "-v",
        "--verbose",
        action="store_true",
        help="Show every category of applied fix.",
    )
    parser.add_argument(
        "--max-passes",
        type=_positive_int,
        default=30,
        help=argparse.SUPPRESS,
    )
    parser.add_argument(
        "--timeout",
        type=_positive_float,
        default=5.0,
        metavar="SECONDS",
        help="Per-file Norminette timeout (default: 5 seconds).",
    )
    parser.add_argument(
        "--version",
        action="version",
        version=f"%(prog)s {__version__}",
    )
    return parser


def _positive_int(value: str) -> int:
    number = int(value)
    if number < 1:
        raise argparse.ArgumentTypeError("must be at least 1")
    return number


def _positive_float(value: str) -> float:
    number = float(value)
    if number <= 0:
        raise argparse.ArgumentTypeError("must be greater than 0")
    return number


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    cwd = Path.cwd().absolute()
    paths, discovery_failures = discover(
        args.paths,
        cwd=cwd,
        use_gitignore=args.use_gitignore,
    )
    identity = resolve_identity(login=args.login, email=args.email, cwd=cwd)
    write = not (args.check or args.diff)
    engine = FixEngine(
        identity=identity,
        options=EngineOptions(
            write=write,
            backup=not args.no_backup,
            backup_root=args.backup_dir,
            max_passes=args.max_passes,
            norminette_timeout=args.timeout,
        ),
    )
    results = engine.process(paths)
    reporter = Reporter(
        output_format=args.format,
        no_color=args.no_color or bool(os.environ.get("NO_COLOR")),
        verbose=args.verbose,
        show_diff=args.diff,
        cwd=cwd,
    )
    reporter.render(
        results,
        identity=identity,
        discovery_failures=discovery_failures,
        check_mode=not write,
    )

    if discovery_failures or any(result.failure for result in results):
        return 2
    if any(result.diagnostics_after for result in results):
        return 1
    if not write and any(result.changed for result in results):
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
