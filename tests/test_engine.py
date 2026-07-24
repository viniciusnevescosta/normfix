from pathlib import Path

from norminette_fix.engine import EngineOptions, FixEngine
from norminette_fix.models import Identity

IDENTITY = Identity("vncosta", "vncosta@student.42sp.org", "test")


def engine() -> FixEngine:
    return FixEngine(
        identity=IDENTITY,
        options=EngineOptions(write=False, backup=False, max_passes=40),
    )


def test_common_errors_are_fixed_without_changing_instructions(tmp_path: Path) -> None:
    path = tmp_path / "main.c"
    path.write_text(
        "int  main( ) {  \n    int x;\n    x=0;\n    return x;\n}\n",
        encoding="utf-8",
    )

    result = engine().process_file(path)

    assert result.failure is None
    assert result.changed
    assert result.fixed is not None
    assert result.fixed.startswith("/* " + ("*" * 74) + " */\n")
    assert "int\tmain(void)\n{" in result.fixed
    assert "\tint\tx;\n\n" in result.fixed
    assert "\tx = 0;" in result.fixed
    assert "\treturn (x);" in result.fixed
    assert result.diagnostics_after == []


def test_fixing_twice_is_idempotent(tmp_path: Path) -> None:
    path = tmp_path / "main.c"
    path.write_text(
        "int main(){return 0;}\n",
        encoding="utf-8",
    )
    first = engine().process_file(path)
    assert first.fixed is not None
    path.write_text(first.fixed, encoding="utf-8")

    second = engine().process_file(path)

    assert not second.changed
    assert second.fixes == []
    assert second.diagnostics_after == []


def test_structural_issue_gets_actionable_english_warning(tmp_path: Path) -> None:
    path = tmp_path / "long_function.c"
    body = "\n".join(f"\tvalue += {index};" for index in range(27))
    path.write_text(
        "int\tlong_function(void)\n"
        "{\n"
        "\tint\tvalue;\n"
        "\n"
        "\tvalue = 0;\n"
        f"{body}\n"
        "\treturn (value);\n"
        "}\n",
        encoding="utf-8",
    )

    result = engine().process_file(path)

    warning = next(item for item in result.diagnostics_after if item.code == "TOO_MANY_LINES")
    assert "long_function()" in warning.detail
    assert "limit is 25" in warning.detail
    assert "static helper" in warning.suggestion


def test_header_file_receives_guard_and_header(tmp_path: Path) -> None:
    path = tmp_path / "ft_demo.h"
    path.write_text("int ft_demo(void);\n", encoding="utf-8")

    result = engine().process_file(path)

    assert result.fixed is not None
    assert "#ifndef FT_DEMO_H" in result.fixed
    assert "# define FT_DEMO_H" in result.fixed
    assert result.fixed.rstrip().endswith("#endif")
    assert result.diagnostics_after == []


def test_pointer_spacing_and_literal_contents_are_preserved(tmp_path: Path) -> None:
    path = tmp_path / "pointer.c"
    path.write_text(
        "char*copy_value(char * value)\n"
        "{\n"
        "    char*result;\n"
        "\n"
        '    result=value + "a==b  c"[0];\n'
        "    return result;\n"
        "}\n",
        encoding="utf-8",
    )

    result = engine().process_file(path)

    assert result.fixed is not None
    assert '"a==b  c"' in result.fixed
    assert "char\t*copy_value(char *value)" in result.fixed
    assert "\tchar\t*result;" in result.fixed
    assert result.diagnostics_after == []


def test_nested_preprocessor_spacing_is_fixed(tmp_path: Path) -> None:
    path = tmp_path / "config.h"
    path.write_text(
        "#ifndef wrong\n#define wrong\n#if FEATURE\n#define VALUE 1\n#endif\n#endif\n",
        encoding="utf-8",
    )

    result = engine().process_file(path)

    assert result.fixed is not None
    assert "#ifndef CONFIG_H" in result.fixed
    assert "# define CONFIG_H" in result.fixed
    assert "# if FEATURE" in result.fixed
    assert "#  define VALUE 1" in result.fixed
    assert "# endif" in result.fixed
    assert result.diagnostics_after == []


def test_return_parser_ignores_semicolons_and_urls_inside_strings(
    tmp_path: Path,
) -> None:
    path = tmp_path / "literal.c"
    path.write_text(
        'char\t*literal_value(void)\n{\n\treturn "https://example.com/a;b";\n}\n',
        encoding="utf-8",
    )

    result = engine().process_file(path)

    assert result.failure is None
    assert result.fixed is not None
    assert 'return ("https://example.com/a;b");' in result.fixed
    assert result.diagnostics_after == []


def test_bom_is_removed_before_header_detection(tmp_path: Path) -> None:
    path = tmp_path / "bom.c"
    path.write_text("\ufeffint main(){return 0;}\n", encoding="utf-8")

    result = engine().process_file(path)

    assert result.fixed is not None
    assert "\ufeff" not in result.fixed
    assert result.fixed.startswith("/* " + ("*" * 74))
    assert result.diagnostics_after == []


def test_internal_feature_condition_is_wrapped_not_renamed(tmp_path: Path) -> None:
    path = tmp_path / "conditional.h"
    path.write_text(
        "int\talways_available(void);\n"
        "\n"
        "#ifndef FEATURE_ENABLED\n"
        "# define OPTIONAL_API 1\n"
        "int\toptional_api(void);\n"
        "#endif\n",
        encoding="utf-8",
    )

    result = engine().process_file(path)

    assert result.fixed is not None
    assert "#ifndef CONDITIONAL_H" in result.fixed
    assert "# define CONDITIONAL_H" in result.fixed
    assert "# ifndef FEATURE_ENABLED" in result.fixed
    assert "#  define OPTIONAL_API 1" in result.fixed
    assert "FEATURE_ENABLED" in result.fixed
    assert result.diagnostics_after == []


def test_numeric_header_name_is_not_given_an_invalid_guard(tmp_path: Path) -> None:
    path = tmp_path / "42.h"
    path.write_text("int\tanswer(void);\n", encoding="utf-8")

    result = engine().process_file(path)

    assert result.fixed is not None
    assert "#ifndef 42_H" not in result.fixed
    assert any(
        diagnostic.code == "HEADER_GUARD_FILENAME" for diagnostic in result.diagnostics_after
    )
    assert all(diagnostic.code != "PARSER_FAILURE" for diagnostic in result.diagnostics_after)


def test_empty_prototype_is_left_for_project_wide_review(tmp_path: Path) -> None:
    path = tmp_path / "prototype.h"
    path.write_text("int\tlegacy_api();\n", encoding="utf-8")

    result = engine().process_file(path)

    assert result.fixed is not None
    assert "legacy_api();" in result.fixed
    warning = next(
        diagnostic for diagnostic in result.diagnostics_after if diagnostic.code == "NO_ARGS_VOID"
    )
    assert "changes its C function type" in warning.suggestion


def test_inline_control_body_is_split_after_the_condition(tmp_path: Path) -> None:
    path = tmp_path / "control.c"
    path.write_text(
        "int\tpositive(int value)\n{\n\tif (value > 0) return 1;\n\treturn 0;\n}\n",
        encoding="utf-8",
    )

    result = engine().process_file(path)

    assert result.fixed is not None
    assert "\tif (value > 0)\n\t\treturn (1);" in result.fixed
    assert result.diagnostics_after == []


def test_long_condition_is_wrapped_at_logical_operators(tmp_path: Path) -> None:
    path = tmp_path / "condition.c"
    path.write_text(
        "int\tmatches(int value)\n"
        "{\n"
        "\tif (value == 1 && value == 2 && value == 3 && value == 4 "
        "&& value == 5 && value == 6)\n"
        "\t\treturn (1);\n"
        "\treturn (0);\n"
        "}\n",
        encoding="utf-8",
    )

    result = engine().process_file(path)

    assert result.fixed is not None
    assert "\n\t\t&& value ==" in result.fixed
    assert all(len(line.expandtabs(4)) <= 80 for line in result.fixed.splitlines())
    assert result.diagnostics_after == []


def test_void_return_spacing_is_idempotent(tmp_path: Path) -> None:
    path = tmp_path / "noop.c"
    path.write_text("void\tnoop(void)\n{\n\treturn;\n}\n", encoding="utf-8")
    first = engine().process_file(path)
    assert first.fixed is not None
    path.write_text(first.fixed, encoding="utf-8")

    second = engine().process_file(path)

    assert "return ;" in first.fixed
    assert not second.changed
