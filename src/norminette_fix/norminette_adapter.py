from __future__ import annotations

import contextlib
import io
import os
import signal
import threading
from pathlib import Path

from .models import Diagnostic, Highlight

# Norminette prints a locale banner while importing. Keep our JSON and
# non-interactive output clean.
os.environ.setdefault("NORMINETTE_LOCALE", "en_US")
with contextlib.redirect_stdout(io.StringIO()):
    from norminette.context import Context
    from norminette.exceptions import NorminetteError
    from norminette.file import File
    from norminette.lexer import Lexer
    from norminette.registry import Registry


class NorminetteAdapter:
    """Small compatibility boundary around Norminette 3.3.59."""

    def __init__(self, *, timeout_seconds: float = 5.0) -> None:
        self._registry = Registry()
        self.timeout_seconds = timeout_seconds

    def lint(self, path: Path, source: str) -> tuple[list[Diagnostic], str | None]:
        norm_file = File(str(path), source)
        try:
            with _deadline(self.timeout_seconds):
                tokens = list(Lexer(norm_file))
                context = Context(norm_file, tokens, debug=0, added_value=[])
                self._registry.run(context)
        except _NorminetteTimeout:
            return [], (
                f"Norminette exceeded the {self.timeout_seconds:g}-second per-file safety timeout"
            )
        except NorminetteError as exc:
            return [], f"Norminette could not parse this file: {exc}"
        except Exception as exc:  # keep one malformed file from hiding the rest
            return [], f"Norminette failed unexpectedly: {type(exc).__name__}: {exc}"

        diagnostics: list[Diagnostic] = []
        seen: set[tuple[str, int, int]] = set()
        for error in norm_file.errors:
            highlights = tuple(
                Highlight(
                    line=item.lineno,
                    column=item.column,
                    length=item.length,
                    hint=item.hint,
                )
                for item in error.highlights
            )
            first = highlights[0] if highlights else Highlight(1, 1)
            key = (error.name, first.line, first.column)
            if key in seen:
                continue
            seen.add(key)
            diagnostics.append(
                Diagnostic(
                    code=error.name,
                    message=error.text,
                    level=error.level,
                    path=path,
                    highlights=highlights,
                )
            )
        return diagnostics, None

    def token_fingerprint(self, path: Path, source: str) -> tuple[tuple[str, str | None], ...]:
        """Return significant tokens for layout-preservation checks."""
        norm_file = File(str(path), source)
        with _deadline(self.timeout_seconds):
            tokens = list(Lexer(norm_file))
        ignored = {"SPACE", "TAB", "NEWLINE", "ESCAPED_NEWLINE"}
        return tuple((token.type, token.value) for token in tokens if token.type not in ignored)


class _NorminetteTimeout(RuntimeError):
    pass


@contextlib.contextmanager
def _deadline(seconds: float):
    supported = (
        seconds > 0
        and hasattr(signal, "SIGALRM")
        and threading.current_thread() is threading.main_thread()
    )
    if not supported:
        yield
        return

    def handle_timeout(_signum, _frame) -> None:
        raise _NorminetteTimeout

    previous_handler = signal.getsignal(signal.SIGALRM)
    signal.signal(signal.SIGALRM, handle_timeout)
    previous_timer = signal.setitimer(signal.ITIMER_REAL, seconds)
    try:
        yield
    finally:
        signal.setitimer(signal.ITIMER_REAL, 0)
        signal.signal(signal.SIGALRM, previous_handler)
        if previous_timer[0] > 0:
            signal.setitimer(signal.ITIMER_REAL, *previous_timer)
