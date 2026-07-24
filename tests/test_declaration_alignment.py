from norminette_fix.declaration_alignment import (
    align_simple_declaration_groups,
    plan_declaration_alignment,
)
from norminette_fix.source import index_to_visual_column


def _declarator_columns(source: str, declarators: tuple[str, ...]) -> list[int]:
    lines = source.splitlines()
    assert len(lines) == len(declarators)
    return [
        index_to_visual_column(line, line.rindex(declarators[index]))
        for index, line in enumerate(lines)
    ]


def test_aligns_a_mixed_width_global_group_with_tabs() -> None:
    source = "int\talpha;\nfloat\tbeta;\ndouble\tgamma;\n"

    plan = plan_declaration_alignment(source)

    assert plan.changed
    assert _declarator_columns(
        plan.candidate,
        ("alpha", "beta", "gamma"),
    ) == [9, 9, 9]
    assert all(" " not in line for line in plan.candidate.splitlines())
    assert plan.groups[0].lines == (1, 2, 3)


def test_pointer_stars_are_part_of_the_aligned_declarator() -> None:
    source = "\tint\t****alpha;\n\tint\t\tbeta;\n"

    candidate, edits = align_simple_declaration_groups(source)

    assert edits
    assert _declarator_columns(candidate, ("****alpha", "beta")) == [9, 9]


def test_aligns_conventional_typedef_names_ending_in_t() -> None:
    source = "\tuint64_t\twide;\n\tint\tvalue;\n"

    candidate, edits = align_simple_declaration_groups(source)

    assert edits
    assert _declarator_columns(candidate, ("wide", "value")) == [17, 17]


def test_preserves_a_reachable_first_declaration_anchor() -> None:
    source = "\tchar\t\t*base;\n\tint\tcount;\n"

    plan = plan_declaration_alignment(source)

    assert _declarator_columns(plan.candidate, ("*base", "count")) == [17, 17]
    assert plan.candidate == "\tchar\t\t*base;\n\tint\t\t\tcount;\n"


def test_aligns_struct_fields_to_the_longest_type_prefix() -> None:
    source = "struct s_outer\n{\n\tstruct s_pair\tpair;\n\tint\tvalue;\n};\n"

    plan = plan_declaration_alignment(source)
    field_lines = plan.candidate.splitlines()[2:4]

    assert [
        index_to_visual_column(field_lines[0], field_lines[0].rindex("pair")),
        index_to_visual_column(field_lines[1], field_lines[1].rindex("value")),
    ] == [21, 21]
    assert plan.groups[0].lines == (3, 4)


def test_target_lines_select_the_intersecting_whole_group_only() -> None:
    source = "int\tone;\ndouble\ttwo;\n\nint\tthree;\ndouble\tfour;\n"

    plan = plan_declaration_alignment(source, target_lines=[2])

    assert plan.candidate.splitlines()[:2] == ["int\t\tone;", "double\ttwo;"]
    assert plan.candidate.splitlines()[3:] == ["int\tthree;", "double\tfour;"]
    assert tuple(edit.line for edit in plan.edits) == (1,)
    assert tuple(group.lines for group in plan.groups) == ((1, 2),)


def test_rejects_complex_or_ambiguous_declarations() -> None:
    rejected_groups = (
        "int a, b;\ndouble c;\n",
        "int (*callback)(int);\ndouble value;\n",
        "int value __attribute__((unused));\ndouble other;\n",
        "struct s_bits\n{\n\tunsigned int\tflag:1;\n\tint\tvalue;\n};\n",
        "int\nvalue;\ndouble other;\n",
        "int values[] = {1, 2};\ndouble other;\n",
        "unknown_type\tvalue;\ndouble\tother;\n",
    )

    for source in rejected_groups:
        plan = plan_declaration_alignment(source)
        assert not plan.changed
        assert plan.candidate == source


def test_rejects_a_group_if_alignment_would_exceed_max_width() -> None:
    source = (
        "int\tshort_name;\n"
        "struct s_extremely_long_type_name\t"
        "a_name_that_already_reaches_the_width_limit;\n"
    )

    plan = plan_declaration_alignment(source, max_width=60)

    assert not plan.changed
    assert plan.candidate == source
    assert not plan.groups


def test_alignment_is_idempotent() -> None:
    source = "int\talpha;\ndouble\tbeta;\n"

    first = plan_declaration_alignment(source)
    second = plan_declaration_alignment(first.candidate)

    assert first.changed
    assert second.candidate == first.candidate
    assert not second.changed


def test_comments_strings_and_separate_scopes_are_not_grouped() -> None:
    source = (
        'char\t*message = "int fake;";\n'
        "int\tglobal;\n"
        "\n"
        "void\tfunction(void)\n"
        "{\n"
        "\tint\tlocal;\n"
        "\tdouble\tother;\n"
        "}\n"
    )

    plan = plan_declaration_alignment(source)

    assert plan.candidate.splitlines()[0] == 'char\t*message = "int fake;";'
    assert plan.candidate.splitlines()[1] == "int\tglobal;"
    local_lines = plan.candidate.splitlines()[5:7]
    assert _declarator_columns(
        "\n".join(local_lines),
        ("local", "other"),
    ) == [13, 13]
    assert tuple(group.lines for group in plan.groups) == ((6, 7),)
