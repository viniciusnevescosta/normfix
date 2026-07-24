from __future__ import annotations

import re
from collections.abc import Iterable
from dataclasses import dataclass

from .source import (
    Edit,
    apply_edits,
    index_to_visual_column,
    line_offsets,
    protected_mask,
)

_TYPE = r"""
    (?:
        (?:
            (?:static|extern|register|const|volatile|restrict|signed|unsigned|short|long)
            [ ]+
        )*
        (?:
            (?:struct|union|enum)[ ]+[A-Za-z_][A-Za-z0-9_]*
            | void
            | char
            | int
            | float
            | double
            | _Bool
            | short
            | long
            | signed
            | unsigned
            | va_list
            | size_t
            | ssize_t
            | ptrdiff_t
            | bool
            | FILE
            | t_[A-Za-z0-9_]+
            | [A-Za-z_][A-Za-z0-9_]*_t
            | [A-Z][A-Za-z0-9_]*
        )
        (?:
            [ ]+
            (?:const|volatile|restrict|signed|unsigned|short|long|int)
        )*
    )
"""

_SIMPLE_DECLARATION = re.compile(
    rf"""
    ^
    (?P<indent>\t*)
    (?P<type>{_TYPE})
    (?P<gap>[ \t]+)
    (?P<declarator>\*+[A-Za-z_][A-Za-z0-9_]*|[A-Za-z_][A-Za-z0-9_]*)
    (?P<arrays>(?:\[[0-9A-Z_+\-*/ \t]*\])*)
    (?P<initializer>
        [ \t]*=[ \t]*[A-Za-z0-9_+\-*/%&|^~!.<> \t]+
    )?
    [ \t]*;
    $
    """,
    re.VERBOSE,
)

_UNSUPPORTED_TOKENS = frozenset(",():\\{}")


@dataclass(frozen=True)
class DeclarationGroup:
    """A same-scope declaration group and the selected declarator column."""

    lines: tuple[int, ...]
    target_column: int


@dataclass(frozen=True)
class AlignmentPlan:
    """The complete, immutable result of declaration-alignment planning."""

    candidate: str
    edits: tuple[Edit, ...]
    groups: tuple[DeclarationGroup, ...]

    @property
    def changed(self) -> bool:
        return bool(self.edits)


@dataclass(frozen=True)
class _Declaration:
    line: int
    scope: tuple[int, ...]
    text: str
    offset: int
    gap_start: int
    gap_end: int
    prefix_column: int
    declarator_column: int


def plan_declaration_alignment(
    source: str,
    target_lines: Iterable[int] | None = None,
    *,
    max_width: int = 80,
) -> AlignmentPlan:
    """Plan conservative tab-only alignment for contiguous declaration groups.

    When ``target_lines`` is supplied, a group is considered only if at least
    one of its lines is targeted. This makes the helper directly usable with
    Norminette's ``MISALIGNED_VAR_DECL`` diagnostics while still aligning the
    whole group.
    """

    targets = None if target_lines is None else frozenset(target_lines)
    if targets == frozenset() or max_width < 1:
        return AlignmentPlan(source, (), ())

    declarations = _declarations_by_group(source)
    edits: list[Edit] = []
    groups: list[DeclarationGroup] = []
    for declaration_group in declarations:
        if len(declaration_group) < 2:
            continue
        group_lines = tuple(item.line for item in declaration_group)
        if targets is not None and targets.isdisjoint(group_lines):
            continue
        minimum_column = max(_next_tab_stop(item.prefix_column) for item in declaration_group)
        anchor_column = declaration_group[0].declarator_column
        if anchor_column >= minimum_column and all(
            _tabs_to_column(item.prefix_column, anchor_column) is not None
            for item in declaration_group
        ):
            target_column = anchor_column
        else:
            target_column = minimum_column
        replacements: list[tuple[_Declaration, str]] = []
        group_is_safe = True
        for declaration in declaration_group:
            whitespace = _tabs_to_column(
                declaration.prefix_column,
                target_column,
            )
            if whitespace is None:
                group_is_safe = False
                break
            rebuilt = (
                declaration.text[: declaration.gap_start]
                + whitespace
                + declaration.text[declaration.gap_end :]
            )
            if _visual_width(rebuilt) > max_width:
                group_is_safe = False
                break
            replacements.append((declaration, whitespace))
        if not group_is_safe:
            continue
        groups.append(DeclarationGroup(group_lines, target_column))
        for declaration, whitespace in replacements:
            edits.append(
                Edit(
                    declaration.offset + declaration.gap_start,
                    declaration.offset + declaration.gap_end,
                    whitespace,
                    "MISALIGNED_VAR_DECL",
                    "aligned a simple declaration group with tabs",
                    declaration.line,
                )
            )

    candidate, accepted = apply_edits(source, edits)
    return AlignmentPlan(candidate, tuple(accepted), tuple(groups))


def align_simple_declaration_groups(
    source: str,
    target_lines: Iterable[int] | None = None,
    *,
    max_width: int = 80,
) -> tuple[str, list[Edit]]:
    """Return a candidate and edits in the same shape as transform helpers."""

    plan = plan_declaration_alignment(
        source,
        target_lines,
        max_width=max_width,
    )
    return plan.candidate, list(plan.edits)


def _declarations_by_group(source: str) -> list[list[_Declaration]]:
    raw_lines, offsets = line_offsets(source)
    mask = protected_mask(source)
    scope_stack = [0]
    next_scope = 1
    groups: list[list[_Declaration]] = []
    current_group: list[_Declaration] = []

    for line_index, raw_line in enumerate(raw_lines):
        text = raw_line.rstrip("\r\n")
        offset = offsets[line_index]
        declaration = _parse_declaration(
            text,
            line=line_index + 1,
            offset=offset,
            scope=tuple(scope_stack),
            has_protected_text=any(mask[offset : offset + len(text)]),
        )
        if (
            declaration is not None
            and current_group
            and declaration.scope == current_group[-1].scope
            and declaration.line == current_group[-1].line + 1
        ):
            current_group.append(declaration)
        elif declaration is not None:
            if current_group:
                groups.append(current_group)
            current_group = [declaration]
        elif current_group:
            groups.append(current_group)
            current_group = []

        if not text.lstrip().startswith("#"):
            visible = "".join(
                " " if mask[offset + index] else char for index, char in enumerate(text)
            )
            for char in visible:
                if char == "}":
                    if len(scope_stack) > 1:
                        scope_stack.pop()
                elif char == "{":
                    scope_stack.append(next_scope)
                    next_scope += 1

    if current_group:
        groups.append(current_group)
    return groups


def _parse_declaration(
    text: str,
    *,
    line: int,
    offset: int,
    scope: tuple[int, ...],
    has_protected_text: bool,
) -> _Declaration | None:
    if has_protected_text:
        return None
    if any(token in text for token in _UNSUPPORTED_TOKENS):
        return None
    if text.count(";") != 1 or not text.rstrip().endswith(";"):
        return None
    match = _SIMPLE_DECLARATION.fullmatch(text)
    if match is None:
        return None
    gap_start, gap_end = match.span("gap")
    return _Declaration(
        line=line,
        scope=scope,
        text=text,
        offset=offset,
        gap_start=gap_start,
        gap_end=gap_end,
        prefix_column=index_to_visual_column(text, gap_start),
        declarator_column=index_to_visual_column(
            text,
            match.start("declarator"),
        ),
    )


def _next_tab_stop(column: int) -> int:
    return column + 4 - ((column - 1) % 4)


def _tabs_to_column(start_column: int, target_column: int) -> str | None:
    column = start_column
    tabs: list[str] = []
    while column < target_column:
        column = _next_tab_stop(column)
        tabs.append("\t")
    if column != target_column:
        return None
    return "".join(tabs)


def _visual_width(text: str) -> int:
    column = 1
    for char in text:
        if char == "\t":
            column = _next_tab_stop(column)
        else:
            column += 1
    return column - 1
