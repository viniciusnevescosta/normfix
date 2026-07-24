from pathlib import Path

from norminette_fix.line_compaction import compact_continuation_lines
from norminette_fix.norminette_adapter import NorminetteAdapter
from norminette_fix.source import visual_width


def _tokens(source: str) -> tuple[tuple[str, str | None], ...]:
    return NorminetteAdapter().token_fingerprint(Path("fixture.c"), source)


def test_compacts_a_parenthesized_continuation_greedily() -> None:
    source = "int\tresult(void)\n{\n\treturn (\n\t\tcombine(first,\n\t\t\tsecond)\n\t);\n}\n"

    compacted, edits = compact_continuation_lines(source)

    assert "\treturn (combine(first, second));\n" in compacted
    assert len(edits) == 3
    assert _tokens(compacted) == _tokens(source)


def test_exactly_eighty_display_columns_is_allowed_but_eighty_one_is_not() -> None:
    identifier_at_limit = "a" * 59
    at_limit = f"\tvalue = {identifier_at_limit}\n\t\t+ other;\n"
    identifier_over_limit = "a" * 60
    over_limit = f"\tvalue = {identifier_over_limit}\n\t\t+ other;\n"

    compacted, edits = compact_continuation_lines(at_limit)
    refused, refused_edits = compact_continuation_lines(over_limit)

    assert len(compacted.splitlines()) == 1
    assert visual_width(compacted.rstrip("\n")) == 80
    assert edits
    assert refused == over_limit
    assert refused_edits == []


def test_packs_later_continuations_without_moving_an_operator_to_line_end() -> None:
    source = (
        "\tresult = " + ("very_long_identifier_" * 3) + "\n" + "\t\t&& short\n" + "\t\t&& tail;\n"
    )

    compacted, edits = compact_continuation_lines(source)

    lines = compacted.splitlines()
    assert len(lines) == 2
    assert lines[0] == source.splitlines()[0]
    assert lines[1].startswith("\t\t&&")
    assert lines[1].endswith("short && tail;")
    assert len(edits) == 1
    assert all(visual_width(line) <= 80 for line in lines)


def test_comments_preprocessors_and_line_splices_are_hard_barriers() -> None:
    comment = "\treturn (left\n\t\t/* explanation */ + right);\n"
    directive = "#define SUM(left, right) (left + \\\n\tright)\n"
    splice = "\tvalue = (left + \\\n\t\tright\n\t\t+ tail);\n"
    trigraph_splice = "\tvalue = (left + ??/\n\t\tright\n\t\t+ tail);\n"
    unmatched_macro_delimiter = "#define OPEN (\n\tfirst();\n\tsecond();\n"

    for source in (
        comment,
        directive,
        splice,
        trigraph_splice,
        unmatched_macro_delimiter,
    ):
        compacted, edits = compact_continuation_lines(source)
        assert compacted == source
        assert edits == []


def test_does_not_merge_separate_instructions_or_a_control_body() -> None:
    source = "void\trun(void)\n{\n\tfirst();\n\tsecond();\n\tif (ready())\n\t\trun_once();\n}\n"

    compacted, edits = compact_continuation_lines(source)

    assert compacted == source
    assert edits == []


def test_ambiguous_pointer_declaration_is_not_treated_as_multiplication() -> None:
    source = "t_item\n\t*item;\n"

    compacted, edits = compact_continuation_lines(source)

    assert compacted == source
    assert edits == []


def test_compaction_is_idempotent_and_preserves_significant_tokens() -> None:
    source = (
        "int\tmatches(int value)\n"
        "{\n"
        "\treturn (value == 1\n"
        "\t\t&& value == 2\n"
        "\t\t&& value == 3);\n"
        "}\n"
    )

    first, first_edits = compact_continuation_lines(source)
    second, second_edits = compact_continuation_lines(first)

    assert first_edits
    assert second == first
    assert second_edits == []
    assert _tokens(first) == _tokens(source)


def test_operator_like_text_inside_strings_is_not_a_comment_barrier() -> None:
    source = '\treturn ("not // a comment"\n\t\t" nor /* a comment */");\n'

    compacted, edits = compact_continuation_lines(source)

    assert compacted == '\treturn ("not // a comment" " nor /* a comment */");\n'
    assert edits
    assert _tokens(compacted) == _tokens(source)
