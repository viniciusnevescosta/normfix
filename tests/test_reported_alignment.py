from pathlib import Path

import pytest

from norminette_fix.engine import EngineOptions, FixEngine
from norminette_fix.models import Identity
from norminette_fix.norminette_adapter import NorminetteAdapter
from norminette_fix.source import index_to_visual_column

IDENTITY = Identity("vncosta", "vncosta@student.42sp.org", "test")
HEADER_BORDER = "/* " + ("*" * 74) + " */"


def engine() -> FixEngine:
    return FixEngine(
        identity=IDENTITY,
        options=EngineOptions(write=False, backup=False, max_passes=40),
    )


def without_official_header(source: str) -> str:
    lines = source.splitlines(keepends=True)
    if not lines or lines[0].rstrip("\r\n") != HEADER_BORDER:
        return source
    for index, line in enumerate(lines[1:], start=1):
        if line.rstrip("\r\n") == HEADER_BORDER:
            return "".join(lines[index + 1 :]).lstrip("\r\n")
    return source


def token_fingerprint(path: Path, source: str) -> tuple[tuple[str, str | None], ...]:
    return NorminetteAdapter().token_fingerprint(path, without_official_header(source))


def declarator_columns(source: str, declarators: tuple[str, ...]) -> list[int]:
    lines = without_official_header(source).splitlines()
    columns: list[int] = []
    for declarator in declarators:
        line = next(line for line in lines if line.rstrip().endswith(f"{declarator};"))
        index = line.rindex(declarator)
        gap_start = index
        while gap_start > 0 and line[gap_start - 1] in " \t":
            gap_start -= 1
        gap = line[gap_start:index]
        assert gap
        assert set(gap) == {"\t"}
        columns.append(index_to_visual_column(line, index))
    return columns


def assert_second_run_is_idempotent(path: Path, fixed: str) -> None:
    path.write_text(fixed, encoding="utf-8")

    second = engine().process_file(path)

    assert not second.changed
    assert second.fixes == []
    assert second.diagnostics_after == []


def test_enum_members_converge_to_one_indentation_tab(tmp_path: Path) -> None:
    path = tmp_path / "push_swap.h"
    source = (
        "#ifndef PUSH_SWAP_H\n"
        "# define PUSH_SWAP_H\n"
        "\n"
        "typedef enum e_mode\n"
        "{\n"
        "    mode_none,\n"
        "\t\tmode_simple,\n"
        "        mode_medium,\n"
        "\t\tmode_complex,\n"
        "    mode_adaptive\n"
        "}\t\t\tt_mode;\n"
        "\n"
        "#endif\n"
    )
    path.write_text(source, encoding="utf-8")
    before = token_fingerprint(path, source)

    first = engine().process_file(path)

    assert first.failure is None
    assert first.fixed is not None
    assert (
        "{\n"
        "\tmode_none,\n"
        "\tmode_simple,\n"
        "\tmode_medium,\n"
        "\tmode_complex,\n"
        "\tmode_adaptive\n"
        "}\t\t\tt_mode;"
    ) in first.fixed
    assert first.diagnostics_after == []
    assert token_fingerprint(path, first.fixed) == before
    assert_second_run_is_idempotent(path, first.fixed)


@pytest.mark.parametrize(
    ("filename", "source", "declarators"),
    [
        pytest.param(
            "ft_print_decimal.c",
            (
                "int\tft_print_decimal(int n)\n"
                "{\n"
                "\tlong int\tlong_n;\n"
                "\tchar\t\tres;\n"
                "\tint\tcount;\n"
                "\n"
                "\treturn (n);\n"
                "}\n"
            ),
            ("long_n", "res", "count"),
            id="decimal",
        ),
        pytest.param(
            "ft_print_hexa.c",
            (
                "int\tft_print_hexa(unsigned long n, char c)\n"
                "{\n"
                "\tchar \t*base_convertion;\n"
                "\tint \tcount;\n"
                "\n"
                "\treturn ((int)n + (int)c);\n"
                "}\n"
            ),
            ("*base_convertion", "count"),
            id="hexa",
        ),
        pytest.param(
            "ft_print_unsigned.c",
            (
                "int ft_print_unsigned(unsigned int num)\n"
                "{\n"
                "\tchar\t\tres;\n"
                "    int\tcount;\n"
                "\n"
                "\treturn ((int)num);\n"
                "}\n"
            ),
            ("res", "count"),
            id="unsigned",
        ),
        pytest.param(
            "ft_printf.c",
            (
                "#include <stdarg.h>\n"
                "\n"
                "int\tft_printf(const char *format, ...)\n"
                "{\n"
                "\tva_list\targs;\n"
                "\tint\ti;\n"
                "\tint\tcount;\n"
                "\n"
                "\treturn ((int)format[0]);\n"
                "}\n"
            ),
            ("args", "i", "count"),
            id="printf",
        ),
        pytest.param(
            "ft_print_pointer.c",
            (
                "int\tft_print_pointer(void *n)\n"
                "{\n"
                "\tint\t\tcount;\n"
                "\tunsigned long\tnum;\n"
                "\n"
                "\treturn ((int)(unsigned long)n);\n"
                "}\n"
            ),
            ("count", "num"),
            id="pointer-and-scalar",
        ),
    ],
)
def test_reported_simple_declaration_groups_are_aligned_safely(
    tmp_path: Path,
    filename: str,
    source: str,
    declarators: tuple[str, ...],
) -> None:
    path = tmp_path / filename
    path.write_text(source, encoding="utf-8")
    before = token_fingerprint(path, source)

    first = engine().process_file(path)

    assert first.failure is None
    assert first.fixed is not None
    columns = declarator_columns(first.fixed, declarators)
    assert len(set(columns)) == 1
    assert all(diagnostic.code != "MISALIGNED_VAR_DECL" for diagnostic in first.diagnostics_after)
    assert first.diagnostics_after == []
    assert token_fingerprint(path, first.fixed) == before
    assert_second_run_is_idempotent(path, first.fixed)


def test_complex_declaration_group_is_left_for_review(tmp_path: Path) -> None:
    path = tmp_path / "complex.c"
    source = (
        "int\twork(void)\n"
        "{\n"
        "\tint\tcount;\n"
        "\tchar\t(*table)[4];\n"
        "\n"
        "\tcount = 0;\n"
        "\treturn (count);\n"
        "}\n"
    )
    declarations = "\tint\tcount;\n\tchar\t(*table)[4];"
    path.write_text(source, encoding="utf-8")
    before = token_fingerprint(path, source)

    first = engine().process_file(path)

    assert first.failure is None
    assert first.fixed is not None
    assert declarations in first.fixed
    assert any(diagnostic.code == "MISALIGNED_VAR_DECL" for diagnostic in first.diagnostics_after)
    assert all(fix.code != "MISALIGNED_VAR_DECL" for fix in first.fixes)
    assert token_fingerprint(path, first.fixed) == before

    path.write_text(first.fixed, encoding="utf-8")
    second = engine().process_file(path)

    assert not second.changed
    assert second.fixes == []
    assert any(diagnostic.code == "MISALIGNED_VAR_DECL" for diagnostic in second.diagnostics_after)


def test_nested_aggregate_extra_tabs_converge_without_changing_tokens(
    tmp_path: Path,
) -> None:
    path = tmp_path / "aggregate.c"
    source = (
        "int\tf(void)\n"
        "{\n"
        "\tint\tcube[1][2][2] = {\n"
        "\t\t{\n"
        "\t\t\t{1, 2},\n"
        "\t\t\t{3, 4},\n"
        "\t\t},\n"
        "\t};\n"
        "\n"
        "\treturn (cube[0][0][0]);\n"
        "}\n"
    )
    path.write_text(source, encoding="utf-8")
    before = token_fingerprint(path, source)

    first = engine().process_file(path)

    assert first.failure is None
    assert first.fixed is not None
    assert all(diagnostic.code != "TOO_MANY_TAB" for diagnostic in first.diagnostics_after)
    assert token_fingerprint(path, first.fixed) == before

    path.write_text(first.fixed, encoding="utf-8")
    second = engine().process_file(path)

    assert second.failure is None
    assert not second.changed
    assert second.fixes == []
    assert all(diagnostic.code != "TOO_MANY_TAB" for diagnostic in second.diagnostics_after)
