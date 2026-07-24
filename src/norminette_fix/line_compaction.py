from __future__ import annotations

import re
from dataclasses import dataclass

from .source import Edit, apply_edits, protected_mask, visual_width

_LEADING_OPERATOR = re.compile(
    r"(?:&&|\|\||==|!=|<=|>=|<<=|>>=|<<|>>|->|\+=|-=|\*=|/=|%=|"
    r"&=|\|=|\^=|[+\-*/%<>|^&=])"
)
_TRAILING_OPERATOR = re.compile(
    r"(?:&&|\|\||==|!=|<=|>=|<<=|>>=|<<|>>|->|\+=|-=|\*=|/=|%=|"
    r"&=|\|=|\^=|[+\-*/%<>|^&=])$"
)
_DECLARATION_KEYWORDS = {
    "_Atomic",
    "_Bool",
    "auto",
    "char",
    "const",
    "double",
    "enum",
    "extern",
    "float",
    "inline",
    "int",
    "long",
    "register",
    "restrict",
    "short",
    "signed",
    "static",
    "struct",
    "typedef",
    "union",
    "unsigned",
    "void",
    "volatile",
}


@dataclass(frozen=True)
class _Line:
    number: int
    start: int
    text: str
    indent_end: int
    has_comment: bool
    is_preprocessor: bool
    has_line_splice: bool
    delimiter_depth_after: int


def compact_continuation_lines(
    source: str,
    *,
    max_columns: int = 80,
) -> tuple[str, list[Edit]]:
    """Greedily join adjacent C continuation lines that fit the column limit.

    This intentionally does not reflow arbitrary source. It only removes a physical
    newline when the surrounding lexical shape proves that both lines belong to one
    continued expression, declaration, or control header. Comments, preprocessor
    directives, line splices, and separate instructions are left untouched.

    The returned edits replace only layout characters. Callers should still compare
    lexer token fingerprints before accepting the result, as the fix engine does for
    every token-preserving transformation.
    """
    if max_columns < 1 or "\n" not in source:
        return source, []

    lines = _scan_lines(source)
    if len(lines) < 2:
        return source, []

    edits: list[Edit] = []
    packed = lines[0].text
    for index in range(len(lines) - 1):
        current = lines[index]
        following = lines[index + 1]
        left = packed.rstrip(" \t")
        right = following.text.lstrip(" \t")
        if not _is_safe_continuation_boundary(current, following, left, right):
            packed = following.text
            continue

        separator = _join_separator(left, right)
        candidate = left + separator + right
        if visual_width(candidate) > max_columns:
            packed = following.text
            continue

        edits.append(
            Edit(
                current.start + len(current.text.rstrip(" \t")),
                following.start + following.indent_end,
                separator,
                "COMPACT_CONTINUATION",
                (f"joined a continuation line without exceeding {max_columns} display columns"),
                following.number,
            )
        )
        packed = candidate

    return apply_edits(source, edits)


def _scan_lines(source: str) -> list[_Line]:
    physical = source.splitlines(keepends=True)
    if not physical:
        return []

    mask = protected_mask(source)
    comment_lines = _comment_line_numbers(source)
    preliminary: list[tuple[int, int, str, str, int, bool]] = []
    position = 0
    for number, raw in enumerate(physical, start=1):
        text = raw.rstrip("\r\n")
        visible = "".join(
            " " if mask[position + offset] else char for offset, char in enumerate(text)
        )
        indent = re.match(r"[ \t]*", text)
        assert indent is not None
        preliminary.append(
            (
                number,
                position,
                text,
                visible,
                indent.end(),
                text.rstrip(" \t").endswith(("\\", "??/")),
            )
        )
        position += len(raw)

    preprocessor_lines: set[int] = set()
    splice_lines: set[int] = set()
    active_directive = False
    for number, _, _text, visible, _, has_line_splice in preliminary:
        if has_line_splice:
            splice_lines.update({number, number + 1})
        starts_directive = visible.lstrip(" \t").startswith("#")
        if active_directive or starts_directive:
            preprocessor_lines.add(number)
            active_directive = has_line_splice
        else:
            active_directive = False

    delimiter_depth = 0
    lines: list[_Line] = []
    for number, start, text, visible, indent_end, _has_line_splice in preliminary:
        if number not in preprocessor_lines:
            for char in visible:
                if char in "([":
                    delimiter_depth += 1
                elif char in ")]":
                    delimiter_depth = max(0, delimiter_depth - 1)
        lines.append(
            _Line(
                number=number,
                start=start,
                text=text,
                indent_end=indent_end,
                has_comment=number in comment_lines,
                is_preprocessor=number in preprocessor_lines,
                has_line_splice=number in splice_lines,
                delimiter_depth_after=delimiter_depth,
            )
        )
    return lines


def _is_safe_continuation_boundary(
    current: _Line,
    following: _Line,
    left: str,
    right: str,
) -> bool:
    if not left or not right:
        return False
    if (
        current.has_comment
        or following.has_comment
        or current.is_preprocessor
        or following.is_preprocessor
        or current.has_line_splice
        or following.has_line_splice
    ):
        return False

    if current.delimiter_depth_after > 0:
        return True
    if _ends_with_continuing_operator(left):
        return True

    operator = _LEADING_OPERATOR.match(right)
    if operator is None or not _ends_like_operand(left):
        return False
    return not (operator.group(0) in {"*", "&"} and _looks_like_declaration_prefix(left))


def _ends_with_continuing_operator(text: str) -> bool:
    stripped = text.rstrip()
    if stripped.endswith(("++", "--")):
        return False
    if stripped.endswith(","):
        return False
    return _TRAILING_OPERATOR.search(stripped) is not None


def _ends_like_operand(text: str) -> bool:
    stripped = text.rstrip()
    if not stripped:
        return False
    if stripped.endswith((")", "]", "++", "--")):
        return True
    return stripped[-1].isalnum() or stripped[-1] in {"_", '"', "'"}


def _looks_like_declaration_prefix(text: str) -> bool:
    stripped = text.strip()
    if re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", stripped):
        return True
    words = re.findall(r"[A-Za-z_][A-Za-z0-9_]*", stripped)
    return bool(words) and all(word in _DECLARATION_KEYWORDS for word in words)


def _join_separator(left: str, right: str) -> str:
    if left.endswith(("(", "[")):
        return ""
    if right.startswith((")", "]", ",", ";")):
        return ""
    return " "


def _comment_line_numbers(source: str) -> set[int]:
    lines: set[int] = set()
    state = "code"
    line = 1
    index = 0
    while index < len(source):
        char = source[index]
        following = source[index + 1] if index + 1 < len(source) else ""
        if state == "code":
            if char == "/" and following == "/":
                lines.add(line)
                state = "line_comment"
                index += 2
                continue
            if char == "/" and following == "*":
                lines.add(line)
                state = "block_comment"
                index += 2
                continue
            if char == '"':
                state = "string"
            elif char == "'":
                state = "character"
        elif state == "line_comment":
            lines.add(line)
            if char == "\n":
                state = "code"
        elif state == "block_comment":
            lines.add(line)
            if char == "*" and following == "/":
                index += 2
                state = "code"
                continue
        elif state in {"string", "character"}:
            if char == "\\" and following:
                if following == "\n":
                    line += 1
                index += 2
                continue
            if (state == "string" and char == '"') or (state == "character" and char == "'"):
                state = "code"
        if char == "\n":
            line += 1
        index += 1
    return lines
