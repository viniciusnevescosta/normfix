from pathlib import Path

from norminette_fix.engine import EngineOptions, FixEngine
from norminette_fix.models import Diagnostic, Highlight, Identity

IDENTITY = Identity("vncosta", "vncosta@student.42sp.org", "test")


def engine() -> FixEngine:
    return FixEngine(
        identity=IDENTITY,
        options=EngineOptions(write=False, backup=False, max_passes=40),
    )


def diagnostic(code: str) -> Diagnostic:
    return Diagnostic(
        code=code,
        message=code,
        level="Error",
        path=Path("fixture.c"),
        highlights=(Highlight(1, 1),),
    )


def test_guarded_diagnostic_gate_rejects_any_new_error_code() -> None:
    before = [diagnostic("MISALIGNED_VAR_DECL")]
    after = [diagnostic("PREPROC_BAD_INDENT")]

    assert not FixEngine._diagnostics_improve(
        before,
        after,
        {"MISALIGNED_VAR_DECL"},
    )


def test_extra_tab_probe_may_make_bounded_progress_without_changing_error_count() -> None:
    unchanged_code = [diagnostic("TOO_MANY_TAB")]

    assert FixEngine._diagnostics_improve(
        unchanged_code,
        unchanged_code,
        {"TOO_MANY_TAB"},
    )
    assert not FixEngine._diagnostics_improve(
        unchanged_code,
        [diagnostic("TOO_FEW_TAB")],
        {"TOO_MANY_TAB"},
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


def test_opt_in_comment_removal_is_safe_and_idempotent(tmp_path: Path) -> None:
    path = tmp_path / "comments.c"
    path.write_text(
        "/* Allowed global comment. */\n"
        "int\tanswer(void)\n"
        "{\n"
        "\t/* forbidden block\n"
        "\t * comment */\n"
        "\treturn (42); /* forbidden trailing comment */\n"
        "}\n",
        encoding="utf-8",
    )
    comment_engine = FixEngine(
        identity=IDENTITY,
        options=EngineOptions(
            write=False,
            backup=False,
            max_passes=40,
            remove_invalid_comments=True,
        ),
    )

    first = comment_engine.process_file(path)

    assert first.failure is None
    assert first.fixed is not None
    assert "/* Allowed global comment. */" in first.fixed
    assert "forbidden block" not in first.fixed
    assert "forbidden trailing comment" not in first.fixed
    assert first.fixed.startswith("/* " + ("*" * 74) + " */")
    assert not any(item.code == "WRONG_SCOPE_COMMENT" for item in first.diagnostics_after)
    assert sum(fix.code == "REMOVE_INVALID_COMMENT" for fix in first.fixes) == 2

    path.write_text(first.fixed, encoding="utf-8")
    second = comment_engine.process_file(path)

    assert not second.changed
    assert second.fixes == []


def test_opt_in_removes_the_full_spliced_line_comment(tmp_path: Path) -> None:
    path = tmp_path / "line_comment.c"
    path.write_text(
        "int\tanswer(void)\n"
        "{\n"
        "\t// the next physical line is part of this comment \\\n"
        "\tthis text must not become live code\n"
        "\treturn (42);\n"
        "}\n",
        encoding="utf-8",
    )
    comment_engine = FixEngine(
        identity=IDENTITY,
        options=EngineOptions(
            write=False,
            backup=False,
            max_passes=40,
            remove_invalid_comments=True,
        ),
    )

    result = comment_engine.process_file(path)

    assert result.failure is None
    assert result.fixed is not None
    assert "the next physical line" not in result.fixed
    assert "this text must not become live code" not in result.fixed
    assert "\treturn (42);" in result.fixed


def test_opt_in_removes_a_global_comment_rejected_between_tokens(
    tmp_path: Path,
) -> None:
    path = tmp_path / "prototype.c"
    path.write_text(
        "int /* misplaced */\tanswer(void);\n",
        encoding="utf-8",
    )
    comment_engine = FixEngine(
        identity=IDENTITY,
        options=EngineOptions(
            write=False,
            backup=False,
            max_passes=40,
            remove_invalid_comments=True,
        ),
    )

    result = comment_engine.process_file(path)

    assert result.failure is None
    assert result.fixed is not None
    assert "misplaced" not in result.fixed
    assert "int\tanswer(void);" in result.fixed
    assert not any(item.code == "COMMENT_ON_INSTR" for item in result.diagnostics_after)


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


def test_header_file_receives_header_and_manual_guard_warning(tmp_path: Path) -> None:
    path = tmp_path / "ft_demo.h"
    path.write_text("int ft_demo(void);\n", encoding="utf-8")

    result = engine().process_file(path)

    assert result.fixed is not None
    assert result.fixed.startswith("/* " + ("*" * 74))
    assert "#ifndef FT_DEMO_H" not in result.fixed
    assert any(diagnostic.code.startswith("HEADER_PROT") for diagnostic in result.diagnostics_after)


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
    assert "#ifndef wrong" in result.fixed
    assert "# define wrong" in result.fixed
    assert "# if FEATURE" in result.fixed
    assert "#  define VALUE 1" in result.fixed
    assert "# endif" in result.fixed
    assert any(diagnostic.code.startswith("HEADER_PROT") for diagnostic in result.diagnostics_after)


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
    assert "#ifndef CONDITIONAL_H" not in result.fixed
    assert "#ifndef FEATURE_ENABLED" in result.fixed
    assert "# define OPTIONAL_API 1" in result.fixed
    assert "FEATURE_ENABLED" in result.fixed
    assert any(diagnostic.code.startswith("HEADER_PROT") for diagnostic in result.diagnostics_after)


def test_repeat_inclusion_header_is_never_auto_guarded(tmp_path: Path) -> None:
    path = tmp_path / "items.h"
    body = "#ifdef WANT_INT\nint\titem;\n#endif\n#ifdef WANT_CHAR\nchar\titem_name;\n#endif\n"
    path.write_text(body, encoding="utf-8")

    result = engine().process_file(path)

    assert result.fixed is not None
    assert "#ifndef ITEMS_H" not in result.fixed
    assert "#ifdef WANT_INT" in result.fixed
    assert "int\titem;" in result.fixed
    assert "#ifdef WANT_CHAR" in result.fixed
    assert "char\titem_name;" in result.fixed
    assert any(diagnostic.code.startswith("HEADER_PROT") for diagnostic in result.diagnostics_after)


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


def test_safe_continuation_lines_are_compacted_by_the_engine(tmp_path: Path) -> None:
    path = tmp_path / "sum.c"
    path.write_text(
        "int\tsum(int left, int right)\n"
        "{\n"
        "\treturn (left\n"
        "\t\t+ right);\n"
        "}\n",
        encoding="utf-8",
    )

    result = engine().process_file(path)

    assert result.failure is None
    assert result.fixed is not None
    assert "\treturn (left + right);\n" in result.fixed
    assert any(fix.code == "COMPACT_CONTINUATION" for fix in result.fixes)
    assert result.diagnostics_after == []
    assert result.diagnostics_after == []


def test_very_long_condition_uses_stable_continuation_tabs(tmp_path: Path) -> None:
    path = tmp_path / "condition.c"
    terms = " && ".join(f"value != {number}" for number in range(30))
    path.write_text(
        f"int\tmatches(int value)\n{{\n\tif ({terms})\n\t\treturn (1);\n\treturn (0);\n}}\n",
        encoding="utf-8",
    )

    result = engine().process_file(path)

    assert result.fixed is not None
    assert all(len(line.expandtabs(4)) <= 80 for line in result.fixed.splitlines())
    continuation_lines = [
        line for line in result.fixed.splitlines() if line.lstrip().startswith("&&")
    ]
    assert len(continuation_lines) > 1
    assert all(line.startswith("\t\t&&") for line in continuation_lines)
    assert result.diagnostics_after == []


def test_very_long_call_keeps_comma_continuation_tabs_stable(tmp_path: Path) -> None:
    path = tmp_path / "call.c"
    arguments = ", ".join(f"value + {number * 100000}" for number in range(16))
    path.write_text(
        f"int\tcall_many(int value)\n{{\n\treturn (combine({arguments}));\n}}\n",
        encoding="utf-8",
    )

    result = engine().process_file(path)

    assert result.fixed is not None
    assert all(len(line.expandtabs(4)) <= 80 for line in result.fixed.splitlines())
    continued_arguments = [
        line for line in result.fixed.splitlines() if line.lstrip().startswith("value +")
    ]
    assert len(continued_arguments) > 1
    assert all(line.startswith("\t\t\tvalue +") for line in continued_arguments)
    assert result.diagnostics_after == []


def test_parenthesized_arithmetic_prefers_top_level_breaks(tmp_path: Path) -> None:
    path = tmp_path / "arithmetic.c"
    expression = " ^ ".join(f"(value + {number})" for number in range(18))
    path.write_text(
        "int\tcalculate(int value)\n"
        "{\n"
        "\tint\tresult;\n"
        "\n"
        f"\tresult = {expression};\n"
        "\treturn (result);\n"
        "}\n",
        encoding="utf-8",
    )

    result = engine().process_file(path)

    assert result.fixed is not None
    assert all(len(line.expandtabs(4)) <= 80 for line in result.fixed.splitlines())
    assert result.diagnostics_after == []


def test_multiplication_and_float_exponents_wrap_at_real_operators(
    tmp_path: Path,
) -> None:
    path = tmp_path / "numbers.c"
    multiplication = " * ".join(f"(value + {number})" for number in range(16))
    exponents = " + ".join(f"1.0e+{number}" for number in range(3, 18))
    path.write_text(
        "float\tnumbers(int value)\n"
        "{\n"
        "\tfloat\tresult;\n"
        "\n"
        f"\tresult = {multiplication};\n"
        f"\tresult += {exponents};\n"
        "\treturn (result);\n"
        "}\n",
        encoding="utf-8",
    )

    result = engine().process_file(path)

    assert result.fixed is not None
    assert "1.0e+3" in result.fixed
    assert all(len(line.expandtabs(4)) <= 80 for line in result.fixed.splitlines())
    assert result.diagnostics_after == []


def test_hex_float_exponent_sign_is_never_spaced_as_an_operator(
    tmp_path: Path,
) -> None:
    path = tmp_path / "hex_float.c"
    path.write_text(
        "double\thex_float(void)\n{\n\treturn (0x.8p-2 + 0x10.p+1);\n}\n",
        encoding="utf-8",
    )

    result = engine().process_file(path)

    assert result.fixed is not None
    assert "0x.8p-2" in result.fixed
    assert "0x10.p+1" in result.fixed
    assert "0x.8p -2" not in result.fixed
    assert "0x10.p +1" not in result.fixed


def test_multiline_macro_continuation_is_never_line_wrapped(
    tmp_path: Path,
) -> None:
    path = tmp_path / "macro.c"
    continuation = "\t" + " + ".join("value" for _ in range(24))
    macro = "#define SUM(value) value + \\   \n" + continuation + "\n"
    path.write_text(
        macro + "\nint\tmain(void)\n{\n\treturn (SUM(1));\n}\n",
        encoding="utf-8",
    )

    result = engine().process_file(path)

    assert result.fixed is not None
    assert macro in result.fixed
    assert any(diagnostic.code == "LINE_TOO_LONG" for diagnostic in result.diagnostics_after)


def test_unary_statement_is_not_mistaken_for_continuation(tmp_path: Path) -> None:
    path = tmp_path / "unary.c"
    path.write_text(
        "void\tset_value(int *value)\n{\n*value = 1;\n}\n",
        encoding="utf-8",
    )

    result = engine().process_file(path)

    assert result.fixed is not None
    assert "\n\t*value = 1;\n" in result.fixed
    assert result.diagnostics_after == []


def test_parser_failure_still_receives_safe_official_header(tmp_path: Path) -> None:
    class FailingAdapter:
        def lint(self, path: Path, source: str):
            return [], "synthetic parser failure"

    path = tmp_path / "broken.c"
    broken_body = "\n\nint broken(  \n\n\n"
    path.write_text(broken_body, encoding="utf-8")
    fixer = FixEngine(
        identity=IDENTITY,
        options=EngineOptions(write=True, backup=False),
        adapter=FailingAdapter(),
    )

    result = fixer.process_file(path)

    assert result.wrote
    assert result.fixed is not None
    assert result.fixed.startswith("/* " + ("*" * 74))
    assert result.fixed.endswith(broken_body)
    assert path.read_text(encoding="utf-8") == result.fixed
    assert any(item.code == "PARSER_FAILURE" for item in result.diagnostics_after)


def test_void_return_spacing_is_idempotent(tmp_path: Path) -> None:
    path = tmp_path / "noop.c"
    path.write_text("void\tnoop(void)\n{\n\treturn;\n}\n", encoding="utf-8")
    first = engine().process_file(path)
    assert first.fixed is not None
    path.write_text(first.fixed, encoding="utf-8")

    second = engine().process_file(path)

    assert "return ;" in first.fixed
    assert not second.changed
