from __future__ import annotations

import contextlib
import hashlib
import io
import os
import signal
import threading
from collections import OrderedDict
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
        self._analysis_cache: OrderedDict[
            tuple[str, bytes],
            tuple[
                tuple[Diagnostic, ...],
                str | None,
                tuple[tuple[str, str | None], ...] | None,
            ],
        ] = OrderedDict()

    def lint(self, path: Path, source: str) -> tuple[list[Diagnostic], str | None]:
        diagnostics, failure, _ = self._analyze(path, source)
        return list(diagnostics), failure

    def _analyze(
        self,
        path: Path,
        source: str,
    ) -> tuple[
        tuple[Diagnostic, ...],
        str | None,
        tuple[tuple[str, str | None], ...] | None,
    ]:
        key = self._cache_key(path, source)
        cached = self._analysis_cache.get(key)
        if cached is not None:
            self._analysis_cache.move_to_end(key)
            return cached

        norm_file = File(str(path), source)
        fingerprint: tuple[tuple[str, str | None], ...] | None = None
        try:
            with _deadline(self.timeout_seconds):
                tokens = list(Lexer(norm_file))
                fingerprint = self._fingerprint(tokens)
                context = Context(norm_file, tokens, debug=0, added_value=[])
                self._registry.run(context)
        except _NorminetteTimeout:
            result = (
                (),
                (
                    "Norminette exceeded the "
                    f"{self.timeout_seconds:g}-second per-file safety timeout"
                ),
                fingerprint,
            )
        except NorminetteError as exc:
            result = ((), f"Norminette could not parse this file: {exc}", fingerprint)
        except Exception as exc:  # keep one malformed file from hiding the rest
            result = (
                (),
                f"Norminette failed unexpectedly: {type(exc).__name__}: {exc}",
                fingerprint,
            )
        else:
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
                diagnostic_key = (error.name, first.line, first.column)
                if diagnostic_key in seen:
                    continue
                seen.add(diagnostic_key)
                diagnostics.append(
                    Diagnostic(
                        code=error.name,
                        message=error.text,
                        level=error.level,
                        path=path,
                        highlights=highlights,
                    )
                )
            result = (tuple(diagnostics), None, fingerprint)

        self._analysis_cache[key] = result
        self._analysis_cache.move_to_end(key)
        while len(self._analysis_cache) > 256:
            self._analysis_cache.popitem(last=False)
        return result

    def token_fingerprint(self, path: Path, source: str) -> tuple[tuple[str, str | None], ...]:
        """Return significant tokens for layout-preservation checks."""
        _, failure, fingerprint = self._analyze(path, source)
        if fingerprint is None:
            raise RuntimeError(failure or "Norminette could not tokenize this file")
        return fingerprint

    def code_token_fingerprint(
        self,
        path: Path,
        source: str,
    ) -> tuple[tuple[str, str | None], ...]:
        """Return code tokens while intentionally ignoring comments and layout."""
        return tuple(
            token
            for token in self.token_fingerprint(path, source)
            if token[0] not in {"COMMENT", "MULT_COMMENT"}
        )

    @staticmethod
    def _fingerprint(tokens) -> tuple[tuple[str, str | None], ...]:
        ignored = {"SPACE", "TAB", "NEWLINE", "ESCAPED_NEWLINE"}
        return tuple(
            (token.type, token.value) for token in tokens if token.type not in ignored
        )

    @staticmethod
    def _cache_key(path: Path, source: str) -> tuple[str, bytes]:
        digest = hashlib.sha256(source.encode("utf-8")).digest()
        return str(path), digest


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
