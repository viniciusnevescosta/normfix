from __future__ import annotations

import re
from dataclasses import dataclass


def visual_width(text: str, *, start_column: int = 1) -> int:
    column = start_column
    for char in text:
        if char == "\t":
            column += 4 - ((column - 1) % 4)
        else:
            column += 1
    return column - start_column


def visual_column_to_index(line: str, column: int) -> int:
    """Map Norminette's 1-based display column to a Python string index."""
    current = 1
    for index, char in enumerate(line):
        if current >= column:
            return index
        if char == "\t":
            current += 4 - ((current - 1) % 4)
        else:
            current += 1
    return len(line)


def index_to_visual_column(line: str, index: int) -> int:
    return visual_width(line[:index]) + 1


def line_offsets(source: str) -> tuple[list[str], list[int]]:
    lines = source.splitlines(keepends=True)
    if not lines:
        return [""], [0]
    offsets: list[int] = []
    position = 0
    for line in lines:
        offsets.append(position)
        position += len(line)
    return lines, offsets


def source_offset(source: str, line: int, column: int) -> int:
    lines, offsets = line_offsets(source)
    line_index = min(max(line - 1, 0), len(lines) - 1)
    raw_line = lines[line_index].rstrip("\r\n")
    return offsets[line_index] + visual_column_to_index(raw_line, column)


@dataclass(frozen=True)
class Edit:
    start: int
    end: int
    replacement: str
    code: str
    description: str
    line: int | None = None


def apply_edits(source: str, edits: list[Edit]) -> tuple[str, list[Edit]]:
    """Apply non-overlapping edits right-to-left, ignoring duplicate/conflicting ones."""
    accepted: list[Edit] = []
    occupied: list[tuple[int, int]] = []
    identities: set[tuple[int, int, str]] = set()
    for edit in sorted(edits, key=lambda item: (item.start, item.end), reverse=True):
        identity = (edit.start, edit.end, edit.replacement)
        if identity in identities:
            continue
        identities.add(identity)
        if edit.start < 0 or edit.end < edit.start or edit.end > len(source):
            continue
        if source[edit.start : edit.end] == edit.replacement:
            continue
        # Insertions at the same boundary conflict; adjacent ranges do not.
        conflict = any(
            (edit.start == edit.end == start == end)
            or max(edit.start, start) < min(edit.end, end)
            or (edit.start == edit.end and start <= edit.start < end)
            or (start == end and edit.start <= start < edit.end)
            for start, end in occupied
        )
        if conflict:
            continue
        source = source[: edit.start] + edit.replacement + source[edit.end :]
        occupied.append((edit.start, edit.end))
        accepted.append(edit)
    accepted.reverse()
    return source, accepted


def leading_whitespace_span(line: str) -> tuple[int, int]:
    match = re.match(r"[ \t]*", line)
    assert match is not None
    return match.span()


def whitespace_before(line: str, index: int) -> tuple[int, int]:
    start = index
    while start > 0 and line[start - 1] in " \t":
        start -= 1
    return start, index


def whitespace_after(line: str, index: int) -> tuple[int, int]:
    end = index
    while end < len(line) and line[end] in " \t":
        end += 1
    return index, end


def operator_span(line: str, index: int) -> tuple[int, int] | None:
    operators = (
        ">>=",
        "<<=",
        "...",
        "++",
        "--",
        "->",
        "&&",
        "||",
        "==",
        "!=",
        "<=",
        ">=",
        "+=",
        "-=",
        "*=",
        "/=",
        "%=",
        "&=",
        "|=",
        "^=",
        "<<",
        ">>",
        "+",
        "-",
        "*",
        "/",
        "%",
        "<",
        ">",
        "=",
        "!",
        "&",
        "|",
        "^",
        "~",
        "?",
        ":",
        ",",
        ";",
        ".",
    )
    for probe in (index, index - 1, index - 2):
        if probe < 0:
            continue
        for operator in operators:
            if line.startswith(operator, probe):
                return probe, probe + len(operator)
    return None


def protected_mask(source: str) -> list[bool]:
    """Mark string, character and comment characters."""
    mask = [False] * len(source)
    index = 0
    state = "code"
    while index < len(source):
        char = source[index]
        nxt = source[index + 1] if index + 1 < len(source) else ""
        if state == "code":
            if char == "/" and nxt == "/":
                mask[index] = mask[index + 1] = True
                index += 2
                state = "line_comment"
                continue
            if char == "/" and nxt == "*":
                mask[index] = mask[index + 1] = True
                index += 2
                state = "block_comment"
                continue
            if char == '"':
                mask[index] = True
                state = "string"
            elif char == "'":
                mask[index] = True
                state = "char"
        elif state == "line_comment":
            if char == "\n":
                state = "code"
            else:
                mask[index] = True
        elif state == "block_comment":
            mask[index] = True
            if char == "*" and nxt == "/":
                mask[index + 1] = True
                index += 2
                state = "code"
                continue
        elif state in {"string", "char"}:
            mask[index] = True
            if char == "\\" and nxt:
                mask[index + 1] = True
                index += 2
                continue
            if (state == "string" and char == '"') or (state == "char" and char == "'"):
                state = "code"
        index += 1
    return mask


def masked_source(source: str) -> str:
    mask = protected_mask(source)
    return "".join(
        "\n" if char == "\n" else (" " if mask[index] else char)
        for index, char in enumerate(source)
    )


def normalize_hygiene(source: str) -> tuple[str, list[tuple[str, str, int | None]]]:
    fixes: list[tuple[str, str, int | None]] = []
    if source.startswith("\ufeff"):
        source = source[1:]
        fixes.append(("REMOVE_BOM", "removed the UTF-8 byte-order mark", 1))
    if "\r" in source:
        source = source.replace("\r\n", "\n").replace("\r", "\n")
        fixes.append(("NORMALIZE_EOL", "normalized line endings to LF", None))

    original_lines = source.split("\n")
    cleaned: list[str] = []
    trailing_count = 0
    for line in original_lines:
        stripped = line.rstrip(" \t")
        # Turning "\\   \n" into "\\\n" creates a C line splice.
        if stripped.endswith("\\") and stripped != line:
            cleaned.append(line)
            continue
        if stripped != line:
            trailing_count += 1
        cleaned.append(stripped)
    if trailing_count:
        fixes.append(
            (
                "TRAILING_WHITESPACE",
                f"removed trailing whitespace from {trailing_count} line(s)",
                None,
            )
        )
    source = "\n".join(cleaned)

    leading = len(source) - len(source.lstrip("\n"))
    if leading:
        source = source.lstrip("\n")
        fixes.append(("EMPTY_LINE_FILE_START", "removed blank line(s) at file start", 1))

    collapsed, count = re.subn(r"\n{3,}", "\n\n", source)
    if count:
        source = collapsed
        fixes.append(("CONSECUTIVE_NEWLINES", "collapsed consecutive blank lines", None))

    normalized = source.rstrip("\n") + "\n" if source else ""
    if normalized != source:
        fixes.append(("EMPTY_LINE_EOF", "normalized the final newline", None))
        source = normalized
    return source, fixes
