from __future__ import annotations

from dataclasses import dataclass

from .analysis import analyze_functions
from .header import header_span
from .models import Diagnostic
from .source import Edit, apply_edits, index_to_visual_column, line_offsets


@dataclass(frozen=True)
class _Comment:
    start: int
    end: int
    line: int
    column: int


def remove_invalid_function_comments(
    source: str,
    diagnostics: list[Diagnostic],
) -> tuple[str, list[Edit]]:
    """Remove only comments whose exact locations Norminette rejected."""
    invalid_locations = {
        (diagnostic.line, diagnostic.column): diagnostic.code
        for diagnostic in diagnostics
        if diagnostic.code in {"WRONG_SCOPE_COMMENT", "COMMENT_ON_INSTR"}
    }
    if not invalid_locations:
        return source, []

    function_lines = {
        line
        for function in analyze_functions(source)
        for line in range(function.opening_line, function.closing_line + 1)
    }
    official_header = header_span(source)
    edits: list[Edit] = []
    for comment in _comments(source):
        diagnostic_code = invalid_locations.get((comment.line, comment.column))
        if diagnostic_code is None:
            continue
        if diagnostic_code == "WRONG_SCOPE_COMMENT" and comment.line not in function_lines:
            continue
        if official_header is not None and comment.start < official_header[1]:
            continue
        start, end, replacement = _removal_span(source, comment)
        edits.append(
            Edit(
                start,
                end,
                replacement,
                "REMOVE_INVALID_COMMENT",
                "removed a comment at the exact location rejected by Norminette",
                comment.line,
            )
        )
    return apply_edits(source, edits)


def _comments(source: str) -> list[_Comment]:
    lines, offsets = line_offsets(source)
    line_by_offset: list[tuple[int, int]] = [
        (offset, line_number) for line_number, offset in enumerate(offsets, start=1)
    ]
    comments: list[_Comment] = []
    index = 0
    state = "code"
    while index < len(source):
        char = source[index]
        following = source[index + 1] if index + 1 < len(source) else ""
        if state == "code":
            if char == '"':
                state = "string"
            elif char == "'":
                state = "character"
            elif char == "/" and following in {"*", "/"}:
                start = index
                if following == "/":
                    end = source.find("\n", index + 2)
                    if end < 0:
                        end = len(source)
                    while end < len(source) and _line_comment_continues(source, end):
                        following_end = source.find("\n", end + 1)
                        end = len(source) if following_end < 0 else following_end
                else:
                    closing = source.find("*/", index + 2)
                    if closing < 0:
                        return comments
                    end = closing + 2
                line_number = _line_for_offset(line_by_offset, start)
                raw_line = lines[line_number - 1].rstrip("\r\n")
                line_start = offsets[line_number - 1]
                comments.append(
                    _Comment(
                        start=start,
                        end=end,
                        line=line_number,
                        column=index_to_visual_column(raw_line, start - line_start),
                    )
                )
                index = end
                continue
        elif state in {"string", "character"}:
            if char == "\\" and following:
                index += 2
                continue
            if (state == "string" and char == '"') or (state == "character" and char == "'"):
                state = "code"
        index += 1
    return comments


def _line_comment_continues(source: str, newline: int) -> bool:
    if newline == 0:
        return False
    if source[newline - 1] == "\\":
        return True
    return newline >= 3 and source[newline - 3 : newline] == "??/"


def _line_for_offset(line_by_offset: list[tuple[int, int]], offset: int) -> int:
    low = 0
    high = len(line_by_offset)
    while low < high:
        middle = (low + high) // 2
        if line_by_offset[middle][0] <= offset:
            low = middle + 1
        else:
            high = middle
    return line_by_offset[max(0, low - 1)][1]


def _removal_span(source: str, comment: _Comment) -> tuple[int, int, str]:
    line_start = source.rfind("\n", 0, comment.start) + 1
    line_end = source.find("\n", comment.end)
    if line_end < 0:
        line_end = len(source)
    before = source[line_start : comment.start]
    after = source[comment.end : line_end]
    if not before.strip(" \t") and not after.strip(" \t"):
        end = line_end + (line_end < len(source))
        return line_start, end, ""
    if not after.strip(" \t"):
        start = comment.start
        while start > line_start and source[start - 1] in " \t":
            start -= 1
        return start, comment.end, ""
    start = comment.start
    while start > line_start and source[start - 1] in " \t":
        start -= 1
    end = comment.end
    while end < line_end and source[end] in " \t":
        end += 1
    surrounding_layout = source[start : comment.start] + source[comment.end : end]
    left = source[start - 1] if start else ""
    right = source[end] if end < len(source) else ""
    if "\t" in surrounding_layout:
        replacement = "\t"
    elif left in "([{" or right in ")]},;":
        replacement = ""
    else:
        replacement = " "
    return start, end, replacement
