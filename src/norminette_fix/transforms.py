from __future__ import annotations

import math
import re
from collections import defaultdict

from .declaration_alignment import align_simple_declaration_groups
from .models import Diagnostic
from .source import (
    Edit,
    apply_edits,
    index_to_visual_column,
    line_offsets,
    operator_span,
    protected_mask,
    visual_column_to_index,
    visual_width,
    whitespace_after,
    whitespace_before,
)


def _lines(source: str) -> tuple[list[str], list[int]]:
    lines, offsets = line_offsets(source)
    return [line.rstrip("\r\n") for line in lines], offsets


def _line_context(source: str, diagnostic: Diagnostic) -> tuple[str, int, int, int] | None:
    lines, offsets = _lines(source)
    if not 1 <= diagnostic.line <= len(lines):
        return None
    line = lines[diagnostic.line - 1]
    index = visual_column_to_index(line, diagnostic.column)
    return line, offsets[diagnostic.line - 1], index, diagnostic.line


def _multiline_preprocessor_lines(source: str) -> set[int]:
    blocked: set[int] = set()
    active = False
    for line_number, line in enumerate(source.splitlines(), start=1):
        starts_directive = line.lstrip(" \t").startswith("#")
        # GCC/Clang accept spaces after a splice backslash as an extension,
        # and normalize_hygiene deliberately preserves that sensitive form.
        continues = line.rstrip(" \t").endswith("\\")
        if active:
            blocked.add(line_number)
            active = continues
        elif starts_directive and continues:
            blocked.add(line_number)
            active = True
    return blocked


def format_preprocessors(source: str) -> tuple[str, list[Edit]]:
    lines = source.splitlines(keepends=True)
    mask = protected_mask(source)
    position = 0
    depth = 0
    edits: list[Edit] = []
    for line_number, line_with_nl in enumerate(lines, start=1):
        line = line_with_nl.rstrip("\r\n")
        masked = "".join(" " if mask[position + i] else char for i, char in enumerate(line))
        position += len(line_with_nl)
        match = re.match(r"^[ \t]*#[ \t]*([A-Za-z_][A-Za-z0-9_]*)(.*)$", masked)
        if not match:
            continue
        directive = match.group(1).lower()
        # Multiline directives/macros are forbidden and formatting their
        # line-splice whitespace may change preprocessing semantics.
        if line.rstrip(" \t").endswith("\\"):
            if directive in {"if", "ifdef", "ifndef"}:
                depth += 1
            continue
        effective_depth = max(
            0,
            depth - (1 if directive in {"elif", "else", "endif"} else 0),
        )
        original_match = re.match(r"^[ \t]*#[ \t]*([A-Za-z_][A-Za-z0-9_]*)(.*)$", line)
        if original_match is None:
            continue
        argument = original_match.group(2).strip(" \t")
        replacement = "#" + (" " * effective_depth) + original_match.group(1)
        if argument:
            replacement += " " + argument
        if replacement != line:
            edits.append(
                Edit(
                    position - len(line_with_nl),
                    position - len(line_with_nl) + len(line),
                    replacement,
                    "PREPROCESSOR_SPACING",
                    "normalized preprocessor indentation and spacing",
                    line_number,
                )
            )
        if directive in {"if", "ifdef", "ifndef"}:
            depth += 1
        elif directive == "endif":
            depth = max(0, depth - 1)
    return apply_edits(source, edits)


def fix_blank_lines(source: str, diagnostics: list[Diagnostic]) -> tuple[str, list[Edit]]:
    lines, offsets = _lines(source)
    edits: list[Edit] = []
    insert_codes = {
        "NEWLINE_PRECEDES_FUNC": "inserted a blank line before a function",
        "NL_AFTER_VAR_DECL": "inserted a blank line after declarations",
        "NL_AFTER_PREPROC": "inserted a blank line after preprocessing directives",
    }
    remove_codes = {
        "EMPTY_LINE_FUNCTION": "removed a forbidden blank line inside a function",
        "CONSECUTIVE_NEWLINES": "removed a consecutive blank line",
    }
    has_local_preprocessor = any(
        diagnostic.code in {"PREPOC_ONLY_GLOBAL", "PREPROC_GLOBAL"} for diagnostic in diagnostics
    )
    for diagnostic in diagnostics:
        index = diagnostic.line - 1
        if not 0 <= index < len(lines):
            continue
        if diagnostic.code in insert_codes:
            if diagnostic.code == "NL_AFTER_PREPROC" and has_local_preprocessor:
                continue
            if index > 0 and lines[index - 1].strip() != "":
                edits.append(
                    Edit(
                        offsets[index],
                        offsets[index],
                        "\n",
                        diagnostic.code,
                        insert_codes[diagnostic.code],
                        diagnostic.line,
                    )
                )
        elif diagnostic.code in remove_codes and lines[index].strip() == "":
            end = offsets[index] + len(lines[index])
            if end < len(source) and source[end : end + 1] == "\n":
                end += 1
            edits.append(
                Edit(
                    offsets[index],
                    end,
                    "",
                    diagnostic.code,
                    remove_codes[diagnostic.code],
                    diagnostic.line,
                )
            )
    return apply_edits(source, edits)


def fix_braces(source: str, diagnostics: list[Diagnostic]) -> tuple[str, list[Edit]]:
    edits: list[Edit] = []
    for diagnostic in diagnostics:
        if diagnostic.code not in {"BRACE_NEWLINE", "BRACE_SHOULD_EOL", "EXP_NEWLINE"}:
            continue
        context = _line_context(source, diagnostic)
        if context is None:
            continue
        line, base, index, line_number = context
        indent = re.match(r"[ \t]*", line).group(0)  # type: ignore[union-attr]

        if diagnostic.code == "BRACE_NEWLINE":
            brace = line.find("{", max(0, index - 2))
            if brace < 0:
                brace = line.rfind("{")
            if brace < 0:
                continue
            start, _ = whitespace_before(line, brace)
            edits.append(
                Edit(
                    base + start,
                    base + brace,
                    "\n" + indent,
                    diagnostic.code,
                    "placed the opening brace on its own line",
                    line_number,
                )
            )
            continue

        if diagnostic.code == "BRACE_SHOULD_EOL":
            candidates = [
                pos
                for pos in range(max(0, index - 2), min(len(line), index + 3))
                if line[pos] in "{}"
            ]
            brace = candidates[0] if candidates else max(line.rfind("{"), line.rfind("}"))
            if brace < 0:
                continue
            after_start, after_end = whitespace_after(line, brace + 1)
            if after_end >= len(line):
                continue
            next_indent = indent + ("\t" if line[brace] == "{" else "")
            edits.append(
                Edit(
                    base + after_start,
                    base + after_end,
                    "\n" + next_indent,
                    diagnostic.code,
                    "placed the brace on its own line",
                    line_number,
                )
            )
            continue

        # A single-line control structure may keep its instruction, but it
        # must start on the following line. Find the closing condition paren.
        close = _control_condition_close(line)
        if close >= 0:
            after_start, after_end = whitespace_after(line, close + 1)
            if after_end < len(line):
                edits.append(
                    Edit(
                        base + after_start,
                        base + after_end,
                        "\n" + indent + "\t",
                        diagnostic.code,
                        "moved the control body to the next line",
                        line_number,
                    )
                )
    return apply_edits(source, edits)


def _control_condition_close(line: str) -> int:
    match = re.search(r"\b(?:if|while|for|switch)\s*\(", line)
    if match is None:
        return -1
    opening = line.find("(", match.start())
    depth = 0
    for index in range(opening, len(line)):
        if line[index] == "(":
            depth += 1
        elif line[index] == ")":
            depth -= 1
            if depth == 0:
                return index
    return -1


def split_extra_instructions(source: str, diagnostics: list[Diagnostic]) -> tuple[str, list[Edit]]:
    target_lines = {
        diagnostic.line for diagnostic in diagnostics if diagnostic.code == "TOO_MANY_INSTR"
    }
    if not target_lines:
        return source, []
    lines, offsets = _lines(source)
    mask = protected_mask(source)
    edits: list[Edit] = []
    for line_number in sorted(target_lines):
        if not 1 <= line_number <= len(lines):
            continue
        line = lines[line_number - 1]
        base = offsets[line_number - 1]
        if line.lstrip().startswith("#"):
            continue
        indent = re.match(r"[ \t]*", line).group(0)  # type: ignore[union-attr]
        paren_depth = 0
        for index, char in enumerate(line):
            if mask[base + index]:
                continue
            if char == "(":
                paren_depth += 1
            elif char == ")":
                paren_depth = max(0, paren_depth - 1)
            elif char == ";" and paren_depth == 0:
                probe = index + 1
                while probe < len(line) and line[probe] in " \t":
                    probe += 1
                if probe < len(line) and line[probe] != "/":
                    next_indent = indent
                    if line[probe] == "}" and next_indent.endswith("\t"):
                        next_indent = next_indent[:-1]
                    edits.append(
                        Edit(
                            base + index + 1,
                            base + probe,
                            "\n" + next_indent,
                            "TOO_MANY_INSTR",
                            "split independent instructions onto separate lines",
                            line_number,
                        )
                    )
    return apply_edits(source, edits)


def fix_no_args_void(source: str, diagnostics: list[Diagnostic]) -> tuple[str, list[Edit]]:
    edits: list[Edit] = []
    for diagnostic in diagnostics:
        if diagnostic.code != "NO_ARGS_VOID":
            continue
        context = _line_context(source, diagnostic)
        if context is None:
            continue
        line, base, index, line_number = context
        left = line.rfind("(", 0, min(len(line), index + 1))
        right = line.find(")", max(0, left))
        suffix = line[right + 1 :] if right >= 0 else ""
        # In a prototype, f() means unspecified parameters in C. Rewriting it
        # to f(void) changes the function type and requires project-wide review.
        is_prototype = suffix.lstrip().startswith(";")
        if left >= 0 and right >= 0 and line[left + 1 : right].strip() == "" and not is_prototype:
            edits.append(
                Edit(
                    base + left + 1,
                    base + right,
                    "void",
                    diagnostic.code,
                    "made the empty argument list explicit with void",
                    line_number,
                )
            )
    return apply_edits(source, edits)


def fix_returns(source: str, diagnostics: list[Diagnostic]) -> tuple[str, list[Edit]]:
    target_lines = {
        diagnostic.line for diagnostic in diagnostics if diagnostic.code == "RETURN_PARENTHESIS"
    }
    lines, offsets = _lines(source)
    mask = protected_mask(source)
    edits: list[Edit] = []
    for line_number in sorted(target_lines):
        if not 1 <= line_number <= len(lines):
            continue
        base = offsets[line_number - 1]
        line = lines[line_number - 1]
        return_match = next(
            (match for match in re.finditer(r"\breturn\b", line) if not mask[base + match.start()]),
            None,
        )
        if return_match is None:
            continue
        expression_start = base + return_match.end()
        while expression_start < len(source) and source[expression_start] in " \t":
            expression_start += 1
        semicolon = _find_statement_semicolon(source, mask, expression_start)
        if semicolon < 0:
            continue
        expression_end = semicolon
        while expression_end > expression_start and source[expression_end - 1] in " \t\n":
            expression_end -= 1
        expression = source[expression_start:expression_end]
        if not expression or _is_wrapped(expression):
            continue
        edits.append(
            Edit(
                expression_start,
                expression_end,
                f"({expression})",
                "RETURN_PARENTHESIS",
                "wrapped the return value in parentheses",
                line_number,
            )
        )
    return apply_edits(source, edits)


def _find_statement_semicolon(source: str, mask: list[bool], start: int) -> int:
    depth = 0
    for index in range(start, len(source)):
        if mask[index]:
            continue
        char = source[index]
        if char in "([{":
            depth += 1
        elif char in ")]}":
            depth = max(0, depth - 1)
        elif char == ";" and depth == 0:
            return index
    return -1


def _is_wrapped(expression: str) -> bool:
    if not (expression.startswith("(") and expression.endswith(")")):
        return False
    depth = 0
    for index, char in enumerate(expression):
        if char == "(":
            depth += 1
        elif char == ")":
            depth -= 1
            if depth == 0 and index != len(expression) - 1:
                return False
    return depth == 0


def fix_function_spacing(source: str, diagnostics: list[Diagnostic]) -> tuple[str, list[Edit]]:
    codes = {
        "SPACE_BEFORE_FUNC",
        "TOO_MANY_TABS_FUNC",
        "MISSING_TAB_FUNC",
    }
    target_lines = {diagnostic.line for diagnostic in diagnostics if diagnostic.code in codes}
    lines, offsets = _lines(source)
    edits: list[Edit] = []
    controls = {"if", "while", "for", "switch", "return", "sizeof"}
    for line_number in sorted(target_lines):
        if not 1 <= line_number <= len(lines):
            continue
        line = lines[line_number - 1]
        candidates = list(re.finditer(r"([A-Za-z_][A-Za-z0-9_]*)[ \t]*\(", line))
        candidates = [item for item in candidates if item.group(1) not in controls]
        if not candidates:
            continue
        name = candidates[-1]
        group_start = name.start(1)
        pointer_start = group_start
        while pointer_start > 0 and line[pointer_start - 1] in "*&":
            pointer_start -= 1
        start, end = whitespace_before(line, pointer_start)
        if start == end:
            start = pointer_start
        edits.append(
            Edit(
                offsets[line_number - 1] + start,
                offsets[line_number - 1] + end,
                "\t",
                "FUNCTION_SPACING",
                "used one tab between the return type and function name",
                line_number,
            )
        )
    return apply_edits(source, edits)


def fix_indentation(source: str, diagnostics: list[Diagnostic]) -> tuple[str, list[Edit]]:
    lines, offsets = _lines(source)
    expected_indents = _expected_indents(source)
    by_line: dict[int, set[str]] = defaultdict(set)
    for diagnostic in diagnostics:
        by_line[diagnostic.line].add(diagnostic.code)
    edits: list[Edit] = []
    guided_probes: list[Edit] = []

    for diagnostic in diagnostics:
        if not 1 <= diagnostic.line <= len(lines):
            continue
        line = lines[diagnostic.line - 1]
        base = offsets[diagnostic.line - 1]
        index = visual_column_to_index(line, diagnostic.column)
        code = diagnostic.code

        if code == "SPACE_REPLACE_TAB":
            if index < len(line) and line[index] == " ":
                start = index
                end = index
                while end < len(line) and line[end] == " ":
                    end += 1
            else:
                start, end = whitespace_before(line, index)
            if start == end:
                edits.append(
                    Edit(
                        base + index,
                        base + index,
                        "\t",
                        code,
                        "inserted a missing declaration or indentation tab",
                        diagnostic.line,
                    )
                )
                continue
            width = max(
                1, visual_width(line[start:end], start_column=index_to_visual_column(line, start))
            )
            replacement = "\t" * max(1, math.ceil(width / 4))
            edits.append(
                Edit(
                    base + start,
                    base + end,
                    replacement,
                    code,
                    "replaced indentation spaces with tab(s)",
                    diagnostic.line,
                )
            )
        elif code == "TAB_REPLACE_SPACE":
            probe = index
            if probe >= len(line) or line[probe] != "\t":
                probe = line.find("\t", max(0, index - 1))
            if probe >= 0:
                edits.append(
                    Edit(
                        base + probe,
                        base + probe + 1,
                        " ",
                        code,
                        "replaced an alignment tab with a space",
                        diagnostic.line,
                    )
                )
        elif code == "TOO_FEW_TAB":
            if "SPACE_REPLACE_TAB" in by_line[diagnostic.line]:
                continue
            leading = re.match(r"[ \t]*", line)
            assert leading is not None
            replacement = "\t" * expected_indents.get(diagnostic.line, 0)
            if leading.group(0) == replacement:
                continue
            edits.append(
                Edit(
                    base,
                    base + leading.end(),
                    replacement,
                    code,
                    "set indentation from the surrounding brace depth",
                    diagnostic.line,
                )
            )
        elif code == "TOO_MANY_TAB":
            leading = re.match(r"[ \t]+", line)
            if leading:
                raw = leading.group(0)
                replacement = "\t" * expected_indents.get(diagnostic.line, 0)
                if visual_width(replacement) >= visual_width(raw):
                    # Norminette has context-sensitive indentation rules for
                    # enum members and nested aggregate initializers that the
                    # lightweight brace model cannot always reproduce.  A
                    # reported extra tab is still safe to probe by removing
                    # exactly one leading tab; the engine re-lints this
                    # candidate and rejects it unless the diagnostic improves
                    # without creating a missing-tab error.
                    if raw and set(raw) == {"\t"}:
                        guided_probes.append(
                            Edit(
                                base,
                                base + leading.end(),
                                raw[:-1],
                                code,
                                ("removed one extra leading tab using a Norminette-guided probe"),
                                diagnostic.line,
                            )
                        )
                    continue
                edits.append(
                    Edit(
                        base,
                        base + leading.end(),
                        replacement,
                        code,
                        "reduced indentation to the surrounding brace depth",
                        diagnostic.line,
                    )
                )
        elif code == "MIXED_SPACE_TAB":
            leading = re.match(r"[ \t]+", line)
            if leading:
                raw = leading.group(0)
                width = visual_width(raw)
                replacement = ("\t" * (width // 4)) + (" " * (width % 4))
                edits.append(
                    Edit(
                        base,
                        base + len(raw),
                        replacement,
                        code,
                        "normalized mixed leading whitespace",
                        diagnostic.line,
                    )
                )
        elif code in {"MISSING_TAB_VAR", "MISSING_TAB_TYPDEF", "NO_TAB_BF_TYPEDEF"}:
            span = whitespace_before(line, index)
            if span[0] != span[1]:
                edits.append(
                    Edit(
                        base + span[0],
                        base + span[1],
                        "\t",
                        code,
                        "inserted the required declaration tab",
                        diagnostic.line,
                    )
                )
    return apply_edits(source, edits if edits else guided_probes)


def _expected_indents(source: str) -> dict[int, int]:
    return _indentation_model(source)[0]


def _indentation_model(
    source: str,
) -> tuple[
    dict[int, int],
    set[int],
    dict[int, int],
    dict[int, int],
    dict[int, int],
]:
    lines, offsets = _lines(source)
    mask = protected_mask(source)
    expected: dict[int, int] = {}
    continuation_lines: set[int] = set()
    brace_indents: dict[int, int] = {}
    delimiter_indents: dict[int, int] = {}
    continuation_extras: dict[int, int] = {}
    depth = 0
    delimiter_depth = 0
    continued = False
    for line_number, line in enumerate(lines, start=1):
        base = offsets[line_number - 1]
        visible = "".join(" " if mask[base + index] else char for index, char in enumerate(line))
        stripped = visible.lstrip(" \t")
        line_depth = max(0, depth - (1 if stripped.startswith("}") else 0))
        brace_indents[line_number] = line_depth
        delimiter_indents[line_number] = delimiter_depth
        continuation_extra = 1 if delimiter_depth == 0 and continued else 0
        if stripped.startswith("{"):
            continuation_extra = 0
        continuation_extras[line_number] = continuation_extra
        is_continuation = delimiter_depth > 0 or continuation_extra > 0
        expected[line_number] = line_depth + delimiter_depth + continuation_extra
        if is_continuation:
            continuation_lines.add(line_number)
        if stripped.startswith("#"):
            continued = False
            continue
        for char in visible:
            if char == "{":
                depth += 1
            elif char == "}":
                depth = max(0, depth - 1)
            elif char in "([":
                delimiter_depth += 1
            elif char in ")]":
                delimiter_depth = max(0, delimiter_depth - 1)
        code = stripped.rstrip()
        continued = bool(code and not re.search(r"(?:;|\{|\}|:)\s*$", code))
    return (
        expected,
        continuation_lines,
        brace_indents,
        delimiter_indents,
        continuation_extras,
    )


def fix_token_spacing(source: str, diagnostics: list[Diagnostic]) -> tuple[str, list[Edit]]:
    edits: list[Edit] = []
    operator_codes = {
        "SPC_BFR_OPERATOR",
        "SPC_AFTER_OPERATOR",
        "NO_SPC_BFR_OPR",
        "NO_SPC_AFR_OPR",
        "SPC_BFR_POINTER",
        "SPC_AFTER_POINTER",
    }
    paren_codes = {
        "SPC_BFR_PAR",
        "SPC_AFTER_PAR",
        "NO_SPC_BFR_PAR",
        "NO_SPC_AFR_PAR",
    }
    for diagnostic in diagnostics:
        context = _line_context(source, diagnostic)
        if context is None:
            continue
        line, base, index, line_number = context
        code = diagnostic.code

        if code in operator_codes:
            span = operator_span(line, index)
            if span is None:
                continue
            start, end = span
            if line[start:end] in {"+", "-"} and _inside_numeric_exponent(
                line,
                start,
            ):
                continue
            if code in {"SPC_BFR_OPERATOR", "SPC_BFR_POINTER"}:
                ws_start, _ = whitespace_before(line, start)
                if ws_start == start:
                    edits.append(
                        Edit(
                            base + start,
                            base + start,
                            " ",
                            code,
                            "inserted required space before an operator",
                            line_number,
                        )
                    )
            elif code == "SPC_AFTER_OPERATOR":
                _, ws_end = whitespace_after(line, end)
                if ws_end == end:
                    edits.append(
                        Edit(
                            base + end,
                            base + end,
                            " ",
                            code,
                            "inserted required space after an operator",
                            line_number,
                        )
                    )
            elif code == "NO_SPC_BFR_OPR":
                ws_start, ws_end = whitespace_before(line, start)
                if line[start:end] == ";" and re.search(r"\breturn[ \t]*$", line[:start]):
                    continue
                if ws_start != ws_end:
                    edits.append(
                        Edit(
                            base + ws_start,
                            base + ws_end,
                            "",
                            code,
                            "removed forbidden space before an operator",
                            line_number,
                        )
                    )
            elif code in {"NO_SPC_AFR_OPR", "SPC_AFTER_POINTER"}:
                ws_start, ws_end = whitespace_after(line, end)
                if ws_start != ws_end:
                    edits.append(
                        Edit(
                            base + ws_start,
                            base + ws_end,
                            "",
                            code,
                            "removed forbidden space after an operator",
                            line_number,
                        )
                    )
            continue

        if code in paren_codes:
            probe = index
            if probe >= len(line) or line[probe] not in "()[]{}":
                nearby = [
                    pos
                    for pos in range(max(0, index - 2), min(len(line), index + 3))
                    if line[pos] in "()[]{}"
                ]
                if not nearby:
                    continue
                probe = nearby[0]
            if code == "SPC_BFR_PAR":
                start, end = whitespace_before(line, probe)
                if start == end:
                    edits.append(
                        Edit(
                            base + probe,
                            base + probe,
                            " ",
                            code,
                            "inserted required space before a parenthesis",
                            line_number,
                        )
                    )
            elif code == "SPC_AFTER_PAR":
                start, end = whitespace_after(line, probe + 1)
                if start == end:
                    edits.append(
                        Edit(
                            base + probe + 1,
                            base + probe + 1,
                            " ",
                            code,
                            "inserted required space after a parenthesis",
                            line_number,
                        )
                    )
            elif code == "NO_SPC_BFR_PAR":
                start, end = whitespace_before(line, probe)
                if start != end:
                    edits.append(
                        Edit(
                            base + start,
                            base + end,
                            "",
                            code,
                            "removed forbidden space before a parenthesis",
                            line_number,
                        )
                    )
            elif code == "NO_SPC_AFR_PAR":
                start, end = whitespace_after(line, probe + 1)
                if start != end:
                    edits.append(
                        Edit(
                            base + start,
                            base + end,
                            "",
                            code,
                            "removed forbidden space after a parenthesis",
                            line_number,
                        )
                    )
            continue

        if code in {"CONSECUTIVE_SPC", "CONSECUTIVE_WS"}:
            start = index
            while start > 0 and line[start - 1] in " \t":
                start -= 1
            end = index
            while end < len(line) and line[end] in " \t":
                end += 1
            if end - start > 1:
                edits.append(
                    Edit(
                        base + start,
                        base + end,
                        " ",
                        code,
                        "collapsed consecutive whitespace",
                        line_number,
                    )
                )
        elif code == "TAB_INSTEAD_SPC":
            probe = index
            if probe >= len(line) or line[probe] != "\t":
                probe = line.find("\t", max(0, index - 1))
            if probe >= 0:
                edits.append(
                    Edit(
                        base + probe,
                        base + probe + 1,
                        " ",
                        code,
                        "replaced a tab with a natural space",
                        line_number,
                    )
                )
        elif code == "SPACE_AFTER_KW":
            word = re.match(r"[A-Za-z_][A-Za-z0-9_]*", line[index:])
            if word:
                end = index + word.end()
                if end >= len(line) or line[end] not in " \t":
                    edits.append(
                        Edit(
                            base + end,
                            base + end,
                            " ",
                            code,
                            "inserted required space after a keyword",
                            line_number,
                        )
                    )
        elif code == "SPC_LINE_START":
            leading = re.match(r"[ \t]+", line)
            if leading:
                edits.append(
                    Edit(
                        base,
                        base + leading.end(),
                        "",
                        code,
                        "removed unexpected leading whitespace",
                        line_number,
                    )
                )
    return apply_edits(source, edits)


def align_variable_declarations(
    source: str, diagnostics: list[Diagnostic]
) -> tuple[str, list[Edit]]:
    target_lines = {
        diagnostic.line for diagnostic in diagnostics if diagnostic.code == "MISALIGNED_VAR_DECL"
    }
    return align_simple_declaration_groups(source, target_lines)


def fix_long_lines(source: str, diagnostics: list[Diagnostic]) -> tuple[str, list[Edit]]:
    target_lines = {
        diagnostic.line for diagnostic in diagnostics if diagnostic.code == "LINE_TOO_LONG"
    }
    lines, offsets = _lines(source)
    mask = protected_mask(source)
    (
        expected_indents,
        continuation_lines,
        brace_indents,
        delimiter_indents,
        continuation_extras,
    ) = _indentation_model(source)
    edits: list[Edit] = []
    for line_number in sorted(target_lines):
        if not 1 <= line_number <= len(lines):
            continue
        line = lines[line_number - 1]
        if visual_width(line) <= 80 or line.lstrip().startswith("#"):
            continue
        base = offsets[line_number - 1]
        indent_match = re.match(r"[ \t]*", line)
        assert indent_match is not None
        candidates: list[tuple[int, int, int, int, int]] = []

        operator_pattern = (
            r"&&|\|\||==|!=|<=|>=|<<=|>>=|<<|>>|->|\+=|-=|\*=|/=|%=|&=|"
            r"\|=|\^=|\+\+|--|[+\-*/%<>|^&=]"
        )
        for match in re.finditer(operator_pattern, line):
            start, end = match.span()
            if mask[base + start]:
                continue
            operator = match.group(0)
            if operator in {"++", "--"}:
                continue
            if operator in {"+", "-", "*", "&"} and _looks_unary(line, start):
                continue
            if operator in {"+", "-"} and _inside_numeric_exponent(line, start):
                continue
            whitespace_start, _ = whitespace_before(line, start)
            prefix_width = visual_width(line[:whitespace_start].rstrip())
            if not 12 <= prefix_width <= 80:
                continue
            priority = 0 if operator in {"&&", "||"} else 2
            nesting = _delimiter_depth_on_line(
                line,
                whitespace_start,
                initial=delimiter_indents.get(line_number, 0),
                mask=mask,
                base=base,
            )
            candidates.append((priority, nesting, prefix_width, whitespace_start, start))

        for index, char in enumerate(line):
            if char != "," or mask[base + index]:
                continue
            after_start, after_end = whitespace_after(line, index + 1)
            prefix_width = visual_width(line[: index + 1])
            if 12 <= prefix_width <= 80:
                nesting = _delimiter_depth_on_line(
                    line,
                    index,
                    initial=delimiter_indents.get(line_number, 0),
                    mask=mask,
                    base=base,
                )
                candidates.append((1, nesting, prefix_width, after_start, after_end))

        if not candidates:
            continue
        best_priority = min(candidate[0] for candidate in candidates)
        best_nesting = min(
            candidate[1] for candidate in candidates if candidate[0] == best_priority
        )
        _, _, _, start, end = max(
            (candidate for candidate in candidates if candidate[0] == best_priority),
            key=lambda candidate: (
                candidate[1] == best_nesting,
                candidate[2] if candidate[1] == best_nesting else -1,
            ),
        )
        delimiter_depth = _delimiter_depth_on_line(
            line,
            start,
            initial=delimiter_indents.get(line_number, 0),
            mask=mask,
            base=base,
        )
        continuation_depth = delimiter_depth + continuation_extras.get(line_number, 0)
        continuation_level = brace_indents.get(line_number, 0) + max(
            1,
            continuation_depth,
        )
        if line_number in continuation_lines:
            continuation_level = max(
                continuation_level,
                expected_indents.get(line_number, 0),
            )
        continuation = "\t" * continuation_level
        edits.append(
            Edit(
                base + start,
                base + end,
                "\n" + continuation,
                "LINE_TOO_LONG",
                "wrapped a long line at a token-safe operator or comma",
                line_number,
            )
        )
    return apply_edits(source, edits)


def _delimiter_depth_on_line(
    line: str,
    end: int,
    *,
    initial: int,
    mask: list[bool],
    base: int,
) -> int:
    depth = initial
    for index, char in enumerate(line[:end]):
        if mask[base + index]:
            continue
        if char in "([":
            depth += 1
        elif char in ")]":
            depth = max(0, depth - 1)
    return depth


def _inside_numeric_exponent(line: str, operator_start: int) -> bool:
    if operator_start < 2 or operator_start + 1 >= len(line):
        return False
    marker = line[operator_start - 1]
    if marker not in "eEpP" or not line[operator_start + 1].isdigit():
        return False
    allowed = "0123456789." if marker in "eE" else "0123456789abcdefABCDEFxX."
    start = operator_start - 2
    while start >= 0 and line[start] in allowed:
        start -= 1
    literal = line[start + 1 : operator_start - 1]
    if start >= 0 and (line[start].isalnum() or line[start] == "_"):
        return False
    if marker in "eE":
        return re.fullmatch(r"(?:\d+(?:\.\d*)?|\.\d+)", literal) is not None
    return (
        re.fullmatch(
            r"0[xX](?:[0-9A-Fa-f]+(?:\.[0-9A-Fa-f]*)?|\.[0-9A-Fa-f]+)",
            literal,
        )
        is not None
    )


def _looks_unary(line: str, operator_start: int) -> bool:
    probe = operator_start - 1
    while probe >= 0 and line[probe] in " \t":
        probe -= 1
    return probe < 0 or line[probe] in "([{,=?:!~+-*/%&|^<>"


PHASES = (
    (
        {
            "NEWLINE_PRECEDES_FUNC",
            "NL_AFTER_VAR_DECL",
            "NL_AFTER_PREPROC",
            "EMPTY_LINE_FUNCTION",
            "CONSECUTIVE_NEWLINES",
        },
        fix_blank_lines,
        True,
    ),
    (
        {"BRACE_NEWLINE", "BRACE_SHOULD_EOL", "EXP_NEWLINE"},
        fix_braces,
        True,
    ),
    ({"TOO_MANY_INSTR"}, split_extra_instructions, True),
    ({"NO_ARGS_VOID"}, fix_no_args_void, False),
    ({"RETURN_PARENTHESIS"}, fix_returns, False),
    (
        {"SPACE_BEFORE_FUNC", "TOO_MANY_TABS_FUNC", "MISSING_TAB_FUNC"},
        fix_function_spacing,
        True,
    ),
    (
        {
            "SPACE_REPLACE_TAB",
            "TAB_REPLACE_SPACE",
            "TOO_FEW_TAB",
            "TOO_MANY_TAB",
            "MIXED_SPACE_TAB",
            "MISSING_TAB_VAR",
            "MISSING_TAB_TYPDEF",
            "NO_TAB_BF_TYPEDEF",
        },
        fix_indentation,
        True,
    ),
    (
        {
            "SPC_BFR_OPERATOR",
            "SPC_AFTER_OPERATOR",
            "NO_SPC_BFR_OPR",
            "NO_SPC_AFR_OPR",
            "SPC_BFR_POINTER",
            "SPC_AFTER_POINTER",
            "SPC_BFR_PAR",
            "SPC_AFTER_PAR",
            "NO_SPC_BFR_PAR",
            "NO_SPC_AFR_PAR",
            "CONSECUTIVE_SPC",
            "CONSECUTIVE_WS",
            "TAB_INSTEAD_SPC",
            "SPACE_AFTER_KW",
            "SPC_LINE_START",
        },
        fix_token_spacing,
        True,
    ),
    ({"MISALIGNED_VAR_DECL"}, align_variable_declarations, True),
    ({"LINE_TOO_LONG"}, fix_long_lines, True),
)


def choose_phase(
    source: str,
    diagnostics: list[Diagnostic],
) -> tuple[str, list[Edit], bool]:
    blocked_lines = _multiline_preprocessor_lines(source)
    eligible = [diagnostic for diagnostic in diagnostics if diagnostic.line not in blocked_lines]
    codes = {diagnostic.code for diagnostic in eligible}
    for phase_codes, handler, preserves_tokens in PHASES:
        if not codes.intersection(phase_codes):
            continue
        updated, edits = handler(source, eligible)
        if edits and updated != source:
            return updated, edits, preserves_tokens
    return source, [], True
