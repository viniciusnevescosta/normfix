from __future__ import annotations

import re
from datetime import datetime
from pathlib import Path

from .header import identity_fits_header
from .models import Diagnostic, Fix, Highlight, Identity
from .source import visual_width

MAKEFILE_HEADER_EDGE = "# " + ("*" * 76) + " #"
_HEADER_RIGHT = {
    "top": ":::      ::::::::    ",
    "file": ":+:      :+:    :+:    ",
    "middle": "+:+ +:+         +:+      ",
    "by": "+#+  +:+       +#+         ",
    "separator": " +#+#+#+#+#+   +#+            ",
    "created": "#+#    #+#              ",
    "updated": "###   ########.fr        ",
}
_ASSIGNMENT = re.compile(
    r"^(?P<name>[A-Za-z_][A-Za-z0-9_]*)(?P<spacing>[ \t]*)"
    r"(?P<operator>::=|:::=|:=|\+=|\?=|!=|=)(?P<after>[ \t]*)(?P<body>.*)$"
)
_PLAIN_C_SOURCE = re.compile(r"[A-Za-z0-9_./+-]+\.c")
_RULE = re.compile(r"^(?P<targets>[^#=\t][^#=]*?):(?:[^=]|$)")
_MANDATORY_RULES = ("all", "clean", "fclean", "re")


def is_makefile(path: Path) -> bool:
    return path.name.casefold() == "makefile"


def identity_fits_makefile_header(identity: Identity) -> bool:
    return identity_fits_header(identity)


def format_makefile(
    source: str,
    path: Path,
    identity: Identity,
    *,
    now: datetime | None = None,
) -> tuple[str, list[Fix]]:
    """Apply only layout changes whose GNU Make meaning is unchanged."""
    fixes: list[Fix] = []
    current = source
    if current.startswith("\ufeff"):
        current = current[1:]
        fixes.append(Fix("REMOVE_BOM", "removed the UTF-8 byte-order mark", 1))
    normalized = current.replace("\r\n", "\n").replace("\r", "\n")
    if normalized != current:
        current = normalized
        fixes.append(Fix("NORMALIZE_NEWLINES", "normalized line endings to LF", 1))

    current, header_changed, header_inserted = ensure_makefile_header(
        current,
        path.name,
        identity,
        now=now,
    )
    if header_changed:
        fixes.append(
            Fix(
                "INVALID_HEADER",
                "inserted the official 42 Makefile header",
                1,
            )
        )

    compacted, compact_fixes = compact_source_assignments(current)
    current = compacted
    fixes.extend(compact_fixes)
    if current and not current.endswith("\n"):
        current += "\n"
        fixes.append(Fix("MISSING_NEWLINE", "added the final newline", len(current.splitlines())))

    if (
        current != source or not makefile_header_filename_matches(current, path.name)
    ) and not header_inserted:
        current, updated = update_makefile_header(
            current,
            path.name,
            identity,
            now=now,
        )
        if updated:
            fixes.append(
                Fix(
                    "UPDATE_HEADER",
                    "updated the official header filename and modification metadata",
                    9,
                )
            )
    return current, fixes


def compact_source_assignments(source: str) -> tuple[str, list[Fix]]:
    if re.search(r"(?m)^[ \t]*\.RECIPEPREFIX[ \t]*(?::=|\+=|\?=|=)", source):
        return source, []
    lines = source.splitlines(keepends=True)
    if not lines:
        return source, []
    replacements: list[tuple[int, int, str, int]] = []
    definition_lines = _make_definition_lines(lines)
    index = 0
    while index < len(lines):
        logical_end = index
        while logical_end < len(lines) - 1 and _has_clean_continuation(lines[logical_end]):
            logical_end += 1
        replacement = None
        if not any(line in definition_lines for line in range(index, logical_end + 1)):
            replacement = _compact_assignment_block(lines[index : logical_end + 1])
        if replacement is not None:
            original = "".join(lines[index : logical_end + 1])
            if replacement != original:
                replacements.append((index, logical_end + 1, replacement, index + 1))
        index = logical_end + 1
    if not replacements:
        return source, []
    for start, end, replacement, _line in reversed(replacements):
        lines[start:end] = [replacement]
    fixes = [
        Fix(
            "MAKEFILE_COMPACT_SOURCES",
            "packed an explicit C source list up to the 80-column limit",
            line,
        )
        for _start, _end, _replacement, line in replacements
    ]
    return "".join(lines), fixes


def analyze_makefile(path: Path, source: str) -> list[Diagnostic]:
    diagnostics: list[Diagnostic] = []
    if makefile_header_span(source) is None or not _valid_makefile_header(source):
        diagnostics.append(
            _diagnostic(
                path,
                "INVALID_HEADER",
                "The official 42 Makefile header is missing or malformed",
                1,
                1,
                "Configure a verified 42 student email so the header can be inserted safely.",
            )
        )

    assignments = _logical_assignments(source)
    if not any(name == "NAME" for _line, name, _body in assignments):
        diagnostics.append(
            _diagnostic(
                path,
                "MAKEFILE_NAME_MISSING",
                "The mandatory NAME variable was not found",
                1,
                1,
                "Define NAME explicitly with the artifact produced by this Makefile.",
            )
        )
    for line, _name, body in assignments:
        if _contains_source_wildcard(body):
            diagnostics.append(
                _diagnostic(
                    path,
                    "MAKEFILE_WILDCARD_SOURCE",
                    "Source and object files must be named explicitly",
                    line,
                    1,
                    "Replace wildcard expansion with an explicit list of every required source.",
                )
            )

    rules = _rules(source)
    targets = {target for _line, rule_targets in rules for target in rule_targets}
    for mandatory in _MANDATORY_RULES:
        if mandatory not in targets:
            diagnostics.append(
                _diagnostic(
                    path,
                    "MAKEFILE_MISSING_RULE",
                    f"The mandatory '{mandatory}' rule was not found",
                    1,
                    1,
                    f"Add a {mandatory} rule that follows the project subject.",
                )
            )
    if "$(NAME)" not in targets and "${NAME}" not in targets:
        diagnostics.append(
            _diagnostic(
                path,
                "MAKEFILE_NAME_RULE_MISSING",
                "The mandatory $(NAME) build rule was not found",
                1,
                1,
                "Add an explicit $(NAME) target with the object files as prerequisites.",
            )
        )
    first_rule = next(
        (
            (line, rule_targets)
            for line, rule_targets in rules
            if any(not target.startswith(".") and "%" not in target for target in rule_targets)
        ),
        None,
    )
    if first_rule is not None and "all" not in first_rule[1]:
        diagnostics.append(
            _diagnostic(
                path,
                "MAKEFILE_DEFAULT_RULE",
                "The mandatory 'all' rule is not the default target",
                first_rule[0],
                1,
                "Move the all rule before the first concrete build target.",
            )
        )
    for number, line in enumerate(source.splitlines(), start=1):
        if visual_width(line) > 80:
            diagnostics.append(
                _diagnostic(
                    path,
                    "MAKEFILE_LINE_TOO_LONG",
                    "This Makefile line exceeds 80 display columns",
                    number,
                    81,
                    "Shorten it manually; only plain explicit .c lists are reflowed automatically.",
                    detail=f"This line is {visual_width(line)} display columns; the limit is 80.",
                )
            )
        stripped = line.rstrip(" \t")
        if stripped.endswith("\\") and stripped != line:
            diagnostics.append(
                _diagnostic(
                    path,
                    "MAKEFILE_TRAILING_AFTER_BACKSLASH",
                    "Whitespace after a continuation backslash was preserved",
                    number,
                    len(stripped) + 1,
                    "Remove it manually after confirming that enabling continuation is intended.",
                )
            )
    return diagnostics


def ensure_makefile_header(
    source: str,
    filename: str,
    identity: Identity,
    *,
    now: datetime | None = None,
) -> tuple[str, bool, bool]:
    span = makefile_header_span(source)
    if span is not None and _valid_makefile_header(source):
        return source, False, False
    if not identity.available or not identity_fits_makefile_header(identity):
        return source, False, False
    header = build_makefile_header(filename, identity, now=now)
    return header + "\n\n" + source, True, True


def build_makefile_header(
    filename: str,
    identity: Identity,
    *,
    now: datetime | None = None,
) -> str:
    if not identity.available:
        raise ValueError("A verified 42 student email is required for the official header.")
    if not identity_fits_makefile_header(identity):
        raise ValueError("The verified 42 identity does not fit the Makefile header.")
    now = now or datetime.now()
    timestamp = now.strftime("%Y/%m/%d %H:%M:%S")
    fields = (
        (f"    {filename}", _HEADER_RIGHT["file"]),
        (f"    By: {identity.login} <{identity.email}>", _HEADER_RIGHT["by"]),
        (f"    Created: {timestamp} by {identity.login}", _HEADER_RIGHT["created"]),
        (f"    Updated: {timestamp} by {identity.login}", _HEADER_RIGHT["updated"]),
    )
    return "\n".join(
        (
            MAKEFILE_HEADER_EDGE,
            _makefile_framed(),
            _makefile_framed("", _HEADER_RIGHT["top"]),
            _makefile_framed(*fields[0]),
            _makefile_framed("", _HEADER_RIGHT["middle"]),
            _makefile_framed(*fields[1]),
            _makefile_framed("", _HEADER_RIGHT["separator"]),
            _makefile_framed(*fields[2]),
            _makefile_framed(*fields[3]),
            _makefile_framed(),
            MAKEFILE_HEADER_EDGE,
        )
    )


def makefile_header_span(source: str) -> tuple[int, int] | None:
    lines = source.splitlines(keepends=True)
    if len(lines) < 11:
        return None
    if (
        lines[0].rstrip("\r\n") != MAKEFILE_HEADER_EDGE
        or lines[10].rstrip("\r\n") != MAKEFILE_HEADER_EDGE
    ):
        return None
    return 0, sum(len(line) for line in lines[:11])


def update_makefile_header(
    source: str,
    filename: str,
    identity: Identity,
    *,
    now: datetime | None = None,
) -> tuple[str, bool]:
    span = makefile_header_span(source)
    if (
        span is None
        or not _valid_makefile_header(source)
        or not identity.available
        or not identity_fits_makefile_header(identity)
    ):
        return source, False
    now = now or datetime.now()
    timestamp = now.strftime("%Y/%m/%d %H:%M:%S")
    block = source[span[0] : span[1]]
    lines = block.rstrip("\r\n").splitlines()
    file_line = _makefile_framed(f"    {filename}", _HEADER_RIGHT["file"])
    updated_line = _makefile_framed(
        f"    Updated: {timestamp} by {identity.login}",
        _HEADER_RIGHT["updated"],
    )
    changed = lines[3] != file_line or lines[8] != updated_line
    if not changed:
        return source, False
    lines[3] = file_line
    lines[8] = updated_line
    replacement = "\n".join(lines) + "\n"
    return source[: span[0]] + replacement + source[span[1] :], True


def makefile_header_filename_matches(source: str, filename: str) -> bool:
    if not _valid_makefile_header(source):
        return False
    span = makefile_header_span(source)
    assert span is not None
    lines = source[span[0] : span[1]].rstrip("\r\n").splitlines()
    return lines[3] == _makefile_framed(f"    {filename}", _HEADER_RIGHT["file"])


def _valid_makefile_header(source: str) -> bool:
    span = makefile_header_span(source)
    if span is None:
        return False
    lines = source[span[0] : span[1]].rstrip("\r\n").splitlines()
    if len(lines) != 11 or any(len(line) != 80 for line in lines):
        return False
    if lines[1] != _makefile_framed() or lines[9] != _makefile_framed():
        return False
    fixed = (
        (2, _makefile_framed("", _HEADER_RIGHT["top"])),
        (4, _makefile_framed("", _HEADER_RIGHT["middle"])),
        (6, _makefile_framed("", _HEADER_RIGHT["separator"])),
    )
    if any(lines[index] != expected for index, expected in fixed):
        return False
    variable = (
        (3, r"^#    \S+", _HEADER_RIGHT["file"]),
        (5, r"^#    By: \S+ <[^<> ]+>", _HEADER_RIGHT["by"]),
        (
            7,
            r"^#    Created: \d{4}/\d{2}/\d{2} \d{2}:\d{2}:\d{2} by \S+",
            _HEADER_RIGHT["created"],
        ),
        (
            8,
            r"^#    Updated: \d{4}/\d{2}/\d{2} \d{2}:\d{2}:\d{2} by \S+",
            _HEADER_RIGHT["updated"],
        ),
    )
    return all(
        re.match(pattern, lines[index]) and lines[index].endswith(right + "#")
        for index, pattern, right in variable
    )


def _makefile_framed(left: str = "", right: str = "") -> str:
    return "#" + left + (" " * (78 - len(left) - len(right))) + right + "#"


def _compact_assignment_block(block: list[str]) -> str | None:
    if not block:
        return None
    first = block[0].rstrip("\r\n")
    match = _ASSIGNMENT.fullmatch(first)
    if match is None or match.group("operator") == "!=":
        return None
    parts: list[str] = []
    for offset, raw in enumerate(block):
        text = raw.rstrip("\r\n")
        text = match.group("body") if offset == 0 else text.lstrip(" \t")
        if offset < len(block) - 1:
            if not text.endswith("\\"):
                return None
            text = text[:-1]
        if any(marker in text for marker in ("$", "%", "#", '"', "'", ";")):
            return None
        parts.append(text)
    tokens = " ".join(parts).split()
    if not tokens or not all(_PLAIN_C_SOURCE.fullmatch(token) for token in tokens):
        return None
    prefix = (
        match.group("name")
        + match.group("spacing")
        + match.group("operator")
        + " "
    )
    continuation = "\t" * max(1, (visual_width(prefix) + 3) // 4)
    packed = _pack_tokens(tokens, prefix, continuation)
    if packed is None:
        return None
    newline = "\n" if block[-1].endswith(("\n", "\r")) else ""
    return "\n".join(packed) + newline


def _pack_tokens(
    tokens: list[str],
    first_prefix: str,
    continuation_prefix: str,
) -> list[str] | None:
    packed: list[str] = []
    index = 0
    prefix = first_prefix
    while index < len(tokens):
        current = prefix
        added = 0
        while index < len(tokens):
            separator = "" if current.endswith((" ", "\t")) else " "
            candidate = current + separator + tokens[index]
            suffix = " \\" if index < len(tokens) - 1 else ""
            if visual_width(candidate + suffix) > 80:
                break
            current = candidate
            index += 1
            added += 1
        if added == 0:
            return None
        if index < len(tokens):
            current += " \\"
        packed.append(current)
        prefix = continuation_prefix
    return packed


def _has_clean_continuation(raw: str) -> bool:
    text = raw.rstrip("\r\n")
    return text.endswith("\\")


def _make_definition_lines(lines: list[str]) -> set[int]:
    blocked: set[int] = set()
    depth = 0
    for index, raw in enumerate(lines):
        stripped = raw.strip()
        if re.match(
            r"^(?:(?:override|export|private)[ \t]+)*define(?:[ \t]|$)",
            stripped,
        ):
            depth += 1
        if depth:
            blocked.add(index)
        if depth and re.match(r"^endef(?:[ \t]|$)", stripped):
            depth -= 1
    return blocked


def _logical_assignments(source: str) -> list[tuple[int, str, str]]:
    lines = source.splitlines()
    definition_lines = _make_definition_lines(lines)
    assignments: list[tuple[int, str, str]] = []
    index = 0
    while index < len(lines):
        start = index
        block = [lines[index]]
        while block[-1].endswith("\\") and index + 1 < len(lines):
            index += 1
            block.append(lines[index])
        match = None
        if not any(line in definition_lines for line in range(start, index + 1)):
            match = _ASSIGNMENT.fullmatch(block[0])
        if match is not None:
            body_parts = [match.group("body").removesuffix("\\")]
            body_parts.extend(line.lstrip(" \t").removesuffix("\\") for line in block[1:])
            assignments.append((start + 1, match.group("name"), " ".join(body_parts)))
        index += 1
    return assignments


def _contains_source_wildcard(body: str) -> bool:
    lowered = body.casefold()
    return (
        "$(wildcard" in lowered
        or "${wildcard" in lowered
        or re.search(r"(?<!\\)[*?][^ \t]*(?:\.c|\.o)\b", lowered) is not None
    )


def _rules(source: str) -> list[tuple[int, tuple[str, ...]]]:
    rules: list[tuple[int, tuple[str, ...]]] = []
    for line_number, line in enumerate(source.splitlines(), start=1):
        if not line or line[0] in "\t#" or "=" in line.split(":", 1)[0]:
            continue
        match = _RULE.match(line)
        if match is None:
            continue
        targets = tuple(match.group("targets").split())
        if targets:
            rules.append((line_number, targets))
    return rules


def _diagnostic(
    path: Path,
    code: str,
    message: str,
    line: int,
    column: int,
    suggestion: str,
    *,
    detail: str = "",
) -> Diagnostic:
    return Diagnostic(
        code=code,
        message=message,
        level="Error",
        path=path,
        highlights=(Highlight(line, column),),
        suggestion=suggestion,
        detail=detail,
        source="norminette-fix Makefile check",
    )
