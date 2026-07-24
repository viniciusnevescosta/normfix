from __future__ import annotations

import re
from dataclasses import dataclass, replace
from pathlib import Path

from .models import Diagnostic, Highlight
from .source import masked_source, visual_width


@dataclass(frozen=True)
class FunctionInfo:
    name: str
    signature_line: int
    opening_line: int
    closing_line: int
    body_lines: int
    parameter_count: int
    variable_count: int

    def contains(self, line: int) -> bool:
        return self.signature_line <= line <= self.closing_line


def analyze_functions(source: str) -> list[FunctionInfo]:
    masked = masked_source(source)
    line_starts = [0]
    for match in re.finditer("\n", masked):
        line_starts.append(match.end())

    def line_of(offset: int) -> int:
        # The files are small; avoiding another dependency keeps this transparent.
        low, high = 0, len(line_starts)
        while low < high:
            mid = (low + high) // 2
            if line_starts[mid] <= offset:
                low = mid + 1
            else:
                high = mid
        return low

    functions: list[FunctionInfo] = []
    depth = 0
    stack: list[int] = []
    candidates: dict[int, tuple[str, int, int, int]] = {}
    for index, char in enumerate(masked):
        if char == "{":
            if depth == 0:
                candidate = _function_before_brace(masked, index)
                if candidate:
                    name, signature_start, params_start, params_end = candidate
                    candidates[index] = (
                        name,
                        signature_start,
                        params_start,
                        params_end,
                    )
            stack.append(index)
            depth += 1
        elif char == "}" and depth:
            depth -= 1
            opening = stack.pop()
            if depth != 0 or opening not in candidates:
                continue
            name, signature_start, params_start, params_end = candidates[opening]
            opening_line = line_of(opening)
            closing_line = line_of(index)
            body = source[opening + 1 : index]
            functions.append(
                FunctionInfo(
                    name=name,
                    signature_line=line_of(signature_start),
                    opening_line=opening_line,
                    closing_line=closing_line,
                    body_lines=max(0, closing_line - opening_line - 1),
                    parameter_count=_parameter_count(source[params_start + 1 : params_end]),
                    variable_count=_variable_count(body),
                )
            )
    return functions


def _function_before_brace(source: str, brace: int) -> tuple[str, int, int, int] | None:
    probe = brace - 1
    while probe >= 0 and source[probe].isspace():
        probe -= 1
    if probe < 0 or source[probe] != ")":
        return None
    close = probe
    depth = 1
    probe -= 1
    while probe >= 0:
        if source[probe] == ")":
            depth += 1
        elif source[probe] == "(":
            depth -= 1
            if depth == 0:
                break
        probe -= 1
    if probe < 0:
        return None
    opening = probe
    probe -= 1
    while probe >= 0 and source[probe].isspace():
        probe -= 1
    end = probe + 1
    while probe >= 0 and (source[probe].isalnum() or source[probe] == "_"):
        probe -= 1
    name = source[probe + 1 : end]
    if not name or name in {"if", "while", "for", "switch", "sizeof"}:
        return None
    delimiter = max(
        source.rfind(";", 0, probe + 1),
        source.rfind("}", 0, probe + 1),
        source.rfind("{", 0, probe + 1),
    )
    signature_start = delimiter + 1
    while signature_start < len(source) and source[signature_start].isspace():
        signature_start += 1
    return name, signature_start, opening, close


def _parameter_count(parameters: str) -> int:
    if not parameters.strip() or parameters.strip() == "void":
        return 0
    depth = 0
    count = 1
    for char in parameters:
        if char in "([{":
            depth += 1
        elif char in ")]}":
            depth = max(0, depth - 1)
        elif char == "," and depth == 0:
            count += 1
    return count


def _variable_count(body: str) -> int:
    count = 0
    declaration = re.compile(
        r"^[ \t]*(?:(?:const|static|unsigned|signed|long|short|volatile)[ \t]+)*"
        r"(?:void|char|int|float|double|struct[ \t]+\w+|enum[ \t]+\w+|"
        r"union[ \t]+\w+|t_\w+)\b.*;",
        re.MULTILINE,
    )
    for match in declaration.finditer(body):
        line = match.group(0)
        # Prototypes and obvious expression statements are not local variables.
        if "(" not in line.split(";", 1)[0]:
            count += 1
    return count


GUIDANCE = {
    "TOO_MANY_LINES": (
        "Extract one coherent responsibility into a well-named static helper; "
        "a safe tool cannot choose the function boundary for you."
    ),
    "LINE_TOO_LONG": (
        "No proven break point was available. Split at a comma or operator, "
        "keeping strings, comments and macro semantics intact."
    ),
    "TOO_MANY_ARGS": (
        "Reduce the public contract to four parameters, or group genuinely related "
        "state in a project-appropriate structure."
    ),
    "TOO_MANY_VARS_FUNC": (
        "Split the responsibility or simplify the state so the function declares no "
        "more than five local variables."
    ),
    "TOO_MANY_FUNCS": (
        "Move a cohesive group of functions to another .c file and update its header "
        "prototypes and Makefile source list."
    ),
    "FORBIDDEN_CS": (
        "Rewrite the forbidden control structure deliberately, usually as a while loop."
    ),
    "TERNARY_FBIDDEN": "Replace the ternary with an explicit if/else assignment or return.",
    "GOTO_FBIDDEN": "Restructure control flow without goto.",
    "LABEL_FBIDDEN": "Remove the label and restructure the associated control flow.",
    "VLA_FORBIDDEN": (
        "Use a compile-time constant size or an allowed dynamic-allocation strategy "
        "appropriate for the project."
    ),
    "ASSIGN_IN_CONTROL": "Move the assignment to its own instruction before the condition.",
    "DECL_ASSIGN_LINE": (
        "Declare the local at the beginning of the function, then assign it after the "
        "declaration block. This is not automated because initialization and assignment "
        "can differ for const, arrays and aggregates."
    ),
    "MULT_DECL_LINE": "Write exactly one variable declaration per line.",
    "VAR_DECL_START_FUNC": "Move the declaration into the function's initial declaration block.",
    "MULT_ASSIGN_LINE": "Split chained assignments while preserving their evaluation order.",
    "MISALIGNED_VAR_DECL": (
        "Align this declarator with the first declaration in the same scope; "
        "complex declaration alignment is left for review."
    ),
    "TOO_MANY_INSTR": (
        "Split the remaining instructions manually; the tool could not prove independence."
    ),
    "FORBIDDEN_CHAR_NAME": (
        "Rename the symbol project-wide to lowercase snake_case and check for collisions."
    ),
    "GLOBAL_VAR_NAMING": "Rename the global project-wide with the required g_ prefix.",
    "USER_DEFINED_TYPEDEF": "Rename the typedef project-wide with the required t_ prefix.",
    "STRUCT_TYPE_NAMING": "Rename the structure tag project-wide with the required s_ prefix.",
    "ENUM_TYPE_NAMING": "Rename the enum tag project-wide with the required e_ prefix.",
    "UNION_TYPE_NAMING": "Rename the union tag project-wide with the required u_ prefix.",
    "MACRO_NAME_CAPITAL": "Rename the macro and every use to uppercase.",
    "GLOBAL_VAR_DETECTED": (
        "Confirm that the global is const or static and justified by the project rules; "
        "otherwise remove the global design."
    ),
    "INCLUDE_START_FILE": (
        "Move the include to the file's include block after checking conditional dependencies."
    ),
    "INCLUDE_HEADER_ONLY": "Include a .h interface instead of a .c implementation file.",
    "WRONG_SCOPE_COMMENT": (
        "Move or remove the comment; comments are not allowed inside function bodies."
    ),
    "COMMENT_ON_INSTR": (
        "Place the comment on its own allowed line or at the end of an allowed global line."
    ),
    "PREPROC_MULTLINE": "Replace the multiline macro; multiline macros are forbidden.",
    "PREPOC_ONLY_GLOBAL": "Move the preprocessor directive to global scope.",
    "PREPROC_GLOBAL": "Move the preprocessor directive to global scope.",
    "FORBIDDEN_STRUCT": "Move the structure definition to an appropriate header.",
    "FORBIDDEN_ENUM": "Move the enum definition to an appropriate header.",
    "FORBIDDEN_UNION": "Move the union definition to an appropriate header.",
    "FORBIDDEN_TYPEDEF": "Move the typedef to an appropriate header.",
    "INVALID_HEADER": "Verify the official 42 header and configured login/email metadata.",
    "NO_ARGS_VOID": (
        "This is a prototype: changing () to (void) changes its C function type. "
        "Verify every declaration and call before making it explicit."
    ),
    "HEADER_GUARD_FILENAME": (
        "Rename this header so its uppercase filename can form a valid C identifier guard."
    ),
    "FILE_NAME_NORM": "Rename the file to lowercase snake_case and update build references.",
    "TRAILING_SPACE_AFTER_BACKSLASH": (
        "Remove or redesign the line splice manually; blindly stripping this whitespace "
        "would change C preprocessing semantics."
    ),
}


def enrich_diagnostics(diagnostics: list[Diagnostic], source: str, path: Path) -> list[Diagnostic]:
    functions = analyze_functions(source)
    lines = source.splitlines()
    enriched: list[Diagnostic] = []
    for diagnostic in diagnostics:
        function = next((item for item in functions if item.contains(diagnostic.line)), None)
        detail = diagnostic.detail
        if diagnostic.code == "TOO_MANY_LINES" and function:
            detail = f"{function.name}() has {function.body_lines} body line(s); the limit is 25."
        elif diagnostic.code == "TOO_MANY_ARGS" and function:
            detail = (
                f"{function.name}() has {function.parameter_count} parameter(s); the limit is 4."
            )
        elif diagnostic.code == "TOO_MANY_VARS_FUNC" and function:
            detail = (
                f"{function.name}() declares approximately {function.variable_count} "
                "local variable(s); the limit is 5."
            )
        elif diagnostic.code == "TOO_MANY_FUNCS":
            detail = f"{path.name} defines {len(functions)} function(s); the limit is 5."
        elif diagnostic.code == "LINE_TOO_LONG" and 1 <= diagnostic.line <= len(lines):
            width = visual_width(lines[diagnostic.line - 1])
            detail = f"This line is {width} display column(s); the limit is 80."
        suggestion = diagnostic.suggestion or GUIDANCE.get(
            diagnostic.code,
            "Review this location and apply the named Norm rule manually; no "
            "semantics-preserving automatic edit was proven.",
        )
        enriched.append(replace(diagnostic, suggestion=suggestion, detail=detail))
    return enriched


def supplemental_diagnostics(path: Path, source: str) -> list[Diagnostic]:
    diagnostics: list[Diagnostic] = []
    if not re.fullmatch(r"[a-z0-9_]+\.[ch]", path.name):
        diagnostics.append(
            Diagnostic(
                code="FILE_NAME_NORM",
                message="File names may contain only lowercase letters, digits and underscores",
                level="Error",
                path=path,
                highlights=(Highlight(1, 1, len(path.name)),),
                source="Norm v4.1 manual",
            )
        )
    if path.suffix == ".h" and path.name[0].isdigit():
        diagnostics.append(
            Diagnostic(
                code="HEADER_GUARD_FILENAME",
                message="A header beginning with a digit cannot form the required guard",
                level="Error",
                path=path,
                highlights=(Highlight(1, 1, len(path.name)),),
                source="norminette-fix safety check",
            )
        )
    for line_number, line in enumerate(source.splitlines(), start=1):
        stripped = line.rstrip(" \t")
        if stripped.endswith("\\") and stripped != line:
            diagnostics.append(
                Diagnostic(
                    code="TRAILING_SPACE_AFTER_BACKSLASH",
                    message="Trailing whitespace after a backslash was not changed safely",
                    level="Error",
                    path=path,
                    highlights=(
                        Highlight(line_number, len(stripped) + 1, len(line) - len(stripped)),
                    ),
                    source="norminette-fix safety check",
                )
            )
    return diagnostics
