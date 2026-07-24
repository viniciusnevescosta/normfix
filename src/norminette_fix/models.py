from __future__ import annotations

from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import Any

__version__ = "0.1.0"


@dataclass(frozen=True)
class Highlight:
    line: int
    column: int
    length: int | None = None
    hint: str | None = None


@dataclass(frozen=True)
class Diagnostic:
    code: str
    message: str
    level: str
    path: Path
    highlights: tuple[Highlight, ...]
    suggestion: str = ""
    detail: str = ""
    source: str = "norminette"

    @property
    def line(self) -> int:
        return self.highlights[0].line if self.highlights else 1

    @property
    def column(self) -> int:
        return self.highlights[0].column if self.highlights else 1

    def to_dict(self) -> dict[str, Any]:
        data = asdict(self)
        data["path"] = str(self.path)
        return data


@dataclass(frozen=True)
class Fix:
    code: str
    description: str
    line: int | None = None
    count: int = 1

    def to_dict(self) -> dict[str, Any]:
        return asdict(self)


@dataclass
class FileResult:
    path: Path
    original: str | None = None
    fixed: str | None = None
    fixes: list[Fix] = field(default_factory=list)
    diagnostics_before: list[Diagnostic] = field(default_factory=list)
    diagnostics_after: list[Diagnostic] = field(default_factory=list)
    failure: str | None = None
    backup: Path | None = None
    wrote: bool = False

    @property
    def changed(self) -> bool:
        return self.original is not None and self.fixed is not None and self.original != self.fixed

    @property
    def error_count(self) -> int:
        return sum(d.level != "Notice" for d in self.diagnostics_after)

    def to_dict(self, *, include_source: bool = False) -> dict[str, Any]:
        data: dict[str, Any] = {
            "path": str(self.path),
            "changed": self.changed,
            "written": self.wrote,
            "backup": str(self.backup) if self.backup else None,
            "failure": self.failure,
            "fixes": [fix.to_dict() for fix in self.fixes],
            "before": [diag.to_dict() for diag in self.diagnostics_before],
            "after": [diag.to_dict() for diag in self.diagnostics_after],
        }
        if include_source:
            data["original"] = self.original
            data["fixed"] = self.fixed
        return data


@dataclass(frozen=True)
class Identity:
    login: str
    email: str
    source: str
    inferred_login: bool = False
    inferred_email: bool = False

    @property
    def inferred(self) -> bool:
        return self.inferred_login or self.inferred_email
