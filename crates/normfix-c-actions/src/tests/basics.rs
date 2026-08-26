use super::*;

#[test]
fn width_uses_terminal_cells_for_unicode() {
    assert_eq!(visual_width("界"), 2);
    assert_eq!(visual_width("e\u{301}"), 1);
}

#[test]
fn hygiene_is_idempotent_and_does_not_create_a_line_splice() {
    let source = "\u{feff}\r\n\r\nint\tx;   \r\n#define X \\  \r\n\r\n\r\n";
    let first = normalize_hygiene(source).unwrap();
    let second = normalize_hygiene(&first.source).unwrap();
    assert_eq!(first.source, "int\tx;\n#define X \\  \n");
    assert_eq!(second.source, first.source);
    assert!(second.fixes.is_empty());
}

#[test]
fn mixed_enum_indentation_uses_syntax_depth_instead_of_preserving_bad_width() {
    let source = concat!(
        "typedef enum e_operation\n",
        "{\n",
        "\t OP_FIRST,\n",
        "\t\tOP_LAST\n",
        "}\tt_operation;\n",
    );
    let fixed = apply(source, &[diagnostic("MIXED_SPACE_TAB", 3, 1)]);
    assert!(fixed.contains("\n\tOP_FIRST,\n"));
    assert!(!fixed.contains("\t OP_FIRST"));
    assert_eq!(apply(&fixed, &[]), fixed);
}

#[test]
fn reported_extra_enum_tab_converges_to_the_syntax_depth() {
    let source = concat!(
        "typedef enum e_operation\n",
        "{\n",
        "\t\tOP_FIRST,\n",
        "\tOP_LAST\n",
        "}\tt_operation;\n",
    );
    let fixed = apply(source, &[diagnostic("TOO_MANY_TAB", 3, 1)]);
    assert!(fixed.contains("\n\tOP_FIRST,\n"));
    assert_eq!(apply(&fixed, &[]), fixed);
}

#[test]
fn reported_space_before_composite_typedef_alias_becomes_a_tab() {
    for keyword in ["struct", "union", "enum"] {
        let member = if keyword == "enum" {
            "\tITEM_VALUE\n"
        } else {
            "\tint\tvalue;\n"
        };
        let source = format!("typedef {keyword} s_item\n{{\n{member}}} t_item;\n");
        let fixed = apply(&source, &[diagnostic("SPACE_REPLACE_TAB", 4, 2)]);
        assert_eq!(
            fixed,
            format!("typedef {keyword} s_item\n{{\n{member}}}\tt_item;\n")
        );
        assert_eq!(apply(&fixed, &[]), fixed);
    }
}

#[test]
fn reported_space_after_unrelated_closing_brace_is_not_guessed_as_a_typedef() {
    let source = "struct s_value\n{\n\tint\tvalue;\n} value;\n";
    assert_eq!(
        apply(source, &[diagnostic("SPACE_REPLACE_TAB", 4, 2)]),
        source
    );
}

#[test]
fn preprocessor_spacing_tracks_nested_conditional_depth() {
    let source = concat!(
        "# include <stddef.h>\n",
        "# ifdef FEATURE\n",
        "#define VALUE 42\n",
        "#  else\n",
        "#define VALUE 0\n",
        "# endif\n",
        "int\tvalue(void);\n",
    );
    let fixed = apply(source, &[]);
    assert_eq!(
        fixed,
        concat!(
            "#include <stddef.h>\n",
            "#ifdef FEATURE\n",
            "# define VALUE 42\n",
            "#else\n",
            "# define VALUE 0\n",
            "#endif\n",
            "int\t\tvalue(void);\n",
        )
    );
}

#[test]
fn blank_line_diagnostics_insert_and_remove_only_physical_blank_lines() {
    let source = concat!(
        "int\tglobal_value;\n",
        "int\tanswer(void)\n",
        "{\n",
        "\n",
        "\treturn (42);\n",
        "}\n",
    );
    let fixed = apply(
        source,
        &[
            diagnostic("NL_AFTER_VAR_DECL", 2, 1),
            diagnostic("EMPTY_LINE_FUNCTION", 4, 1),
        ],
    );
    assert_eq!(
        fixed,
        concat!(
            "int\tglobal_value;\n",
            "\n",
            "int\tanswer(void)\n",
            "{\n",
            "\treturn (42);\n",
            "}\n",
        )
    );
}

#[test]
fn prototype_names_are_aligned_at_a_shared_tab_stop() {
    let source = concat!(
        "int\tvalidate_numbers(int argc, char **argv);\n",
        "char\t*next_value(void);\n",
    );
    let fixed = apply(source, &[diagnostic("MISALIGNED_FUNC_DECL", 1, 5)]);
    assert_eq!(
        fixed,
        concat!(
            "int\t\tvalidate_numbers(int argc, char **argv);\n",
            "char\t*next_value(void);\n",
        )
    );
    assert_eq!(apply(&fixed, &[]), fixed);
}

#[test]
fn simple_variable_groups_align_but_complex_declarators_remain_untouched() {
    let source = concat!(
        "int\tshort_name;\n",
        "long int\tlong_name;\n",
        "\n",
        "int\t(*callback)(int);\n",
        "char\t*payload;\n",
    );
    let fixed = apply(source, &[diagnostic("MISALIGNED_VAR_DECL", 2, 9)]);
    let lines: Vec<_> = fixed.lines().collect();
    let first_column = lines[0].find("short_name").unwrap();
    let second_column = lines[1].find("long_name").unwrap();
    assert_ne!(lines[0], "int\tshort_name;");
    assert!(
        first_column < second_column,
        "byte columns differ because tabs expand"
    );
    assert_eq!(
        visual_column(lines[0], "short_name"),
        visual_column(lines[1], "long_name")
    );
    assert_eq!(lines[3], "int\t(*callback)(int);");
}

#[test]
fn continuation_compaction_is_greedy_and_respects_comment_and_macro_barriers() {
    let compactable = concat!(
        "int\tvalue(void)\n",
        "{\n",
        "\treturn (sum(first,\n",
        "\t\tsecond,\n",
        "\t\tthird));\n",
        "}\n",
    );
    let fixed = apply(compactable, &[]);
    assert!(fixed.contains("return (sum(first, second, third));"));

    let barriers = concat!(
        "#define PICK(a, b) ((a) + \\\n",
        "\t(b))\n",
        "int\tvalue(void)\n",
        "{\n",
        "\treturn (first + // keep the boundary\n",
        "\t\tsecond);\n",
        "}\n",
    );
    assert_eq!(apply(barriers, &[]), barriers);
}

#[test]
fn continuation_compaction_accepts_eighty_columns_and_refuses_eighty_one() {
    let at_limit_identifier = "a".repeat(59);
    let over_limit_identifier = "a".repeat(60);
    let at_limit =
        format!("void\trun(void)\n{{\n\tvalue = {at_limit_identifier}\n\t\t+ other;\n}}\n");
    let over_limit =
        format!("void\trun(void)\n{{\n\tvalue = {over_limit_identifier}\n\t\t+ other;\n}}\n");
    let compacted = apply(&at_limit, &[]);
    let refused = apply(&over_limit, &[]);
    assert!(compacted.lines().any(|line| visual_width(line) == 80));
    assert!(refused.contains(&format!("{over_limit_identifier}\n\t\t+ other")));
}

#[test]
fn comment_markers_inside_strings_do_not_block_safe_compaction() {
    let source = concat!(
        "int\tvalue(void)\n",
        "{\n",
        "\treturn (pick(\"https://42.fr/a//b\",\n",
        "\t\t1));\n",
        "}\n",
    );
    let fixed = apply(source, &[]);
    assert!(fixed.contains("pick(\"https://42.fr/a//b\", 1)"));
}

#[test]
fn adjacent_string_literals_compact_without_treating_comment_text_as_comments() {
    let source = concat!(
        "char\t*message(void)\n",
        "{\n",
        "\treturn (\"not // a comment\"\n",
        "\t\t\" nor /* a comment */\");\n",
        "}\n",
    );
    let fixed = apply(source, &[]);
    assert!(fixed.contains("\"not // a comment\" \" nor /* a comment */\""));
}

#[test]
fn long_conditions_wrap_at_logical_operators_and_never_inside_exponents() {
    let source = concat!(
        "int\tcheck(int alpha, int beta, int gamma, int delta)\n",
        "{\n",
        "\treturn (alpha == beta && beta == gamma && gamma == delta && alpha + beta + gamma + delta > 1e+10);\n",
        "}\n",
    );
    let fixed = apply(source, &[]);
    assert!(fixed.contains("\n\t\t&& "));
    assert!(fixed.contains("1e+10"));
    assert!(!fixed.contains("1e\n"));
    assert!(fixed.lines().all(|line| visual_width(line) <= 80));
    assert_eq!(apply(&fixed, &[]), fixed);
}

#[test]
fn multiline_macros_and_long_strings_are_never_reflowed() {
    let source = concat!(
        "#define VERY_LONG(a) ((a) + (a) + (a) + (a) + (a) + (a) + (a) + (a) + (a) + \\\n",
        "\t(a))\n",
        "char\t*message(void)\n",
        "{\n",
        "\treturn (\"this string literal is intentionally longer than eighty display columns and cannot be split safely\");\n",
        "}\n",
    );
    let fixed = apply(source, &[]);
    assert!(fixed.contains("+ \\\n"));
    assert!(fixed.contains("this string literal is intentionally longer"));
}

#[test]
fn brace_control_and_token_spacing_edits_leave_literal_contents_untouched() {
    let source = concat!(
        "int\tcheck(int value) {\n",
        "\tif(value) return(value==42);\n",
        "}\n",
    );
    let diagnostics = vec![
        diagnostic("BRACE_NEWLINE", 1, 22),
        diagnostic("EXP_NEWLINE", 2, 2),
        diagnostic("SPACE_AFTER_KW", 2, 2),
        diagnostic("SPC_BFR_OPERATOR", 2, 25),
        diagnostic("SPC_AFTER_OPERATOR", 2, 25),
    ];
    let fixed = apply(source, &diagnostics);
    assert!(fixed.contains("int\tcheck(int value)\n{"));
    assert!(fixed.contains("if (value)\n"));
}

#[test]
fn return_parentheses_and_definition_void_are_structurally_bounded() {
    let source = concat!(
        "int\tanswer()\n",
        "{\n",
        "\treturn 42;\n",
        "}\n",
        "int\tlegacy_api();\n",
    );
    let fixed = apply(
        source,
        &[
            diagnostic("NO_ARGS_VOID", 1, 12),
            diagnostic("RETURN_PARENTHESIS", 3, 2),
            diagnostic("NO_ARGS_VOID", 5, 15),
        ],
    );
    assert!(fixed.contains("answer(void)"));
    assert!(fixed.contains("return (42);"));
    assert!(fixed.contains("legacy_api();"));
    assert_eq!(apply(&fixed, &[]), fixed);
}

#[test]
fn invalid_comment_removal_is_exact_opt_in_and_code_token_preserving() {
    let source = concat!(
        "int\tanswer(void)\n",
        "{\n",
        "\t/* rejected */\n",
        "\treturn (42);\n",
        "}\n",
    );
    let diagnostics = [diagnostic("WRONG_SCOPE_COMMENT", 3, 5)];
    let without_permission = apply(source, &diagnostics);
    assert_eq!(without_permission, source);

    let options = CActionOptions {
        remove_invalid_comments: true,
        ..CActionOptions::default()
    };
    let fixed = apply_c_actions(Utf8Path::new("comment.c"), source, &diagnostics, &options)
        .unwrap()
        .source;
    assert!(!fixed.contains("rejected"));
    assert!(fixed.contains("return (42);"));
    assert_eq!(
        apply_c_actions(Utf8Path::new("comment.c"), &fixed, &diagnostics, &options,)
            .unwrap()
            .source,
        fixed
    );
}

#[test]
fn official_42_header_is_never_a_comment_removal_target() {
    let source = concat!(
        "/* ************************************************************************** */\n",
        "/*                                                                            */\n",
        "/*                                                        :::      ::::::::   */\n",
        "/*   fixture.c                                          :+:      :+:    :+:   */\n",
        "/*                                                    +:+ +:+         +:+     */\n",
        "/*   By: student <student@student.42.fr>              +#+  +:+       +#+        */\n",
        "/*                                                +#+#+#+#+#+   +#+           */\n",
        "/*   Created: 2026/07/30 12:00:00 by student           #+#    #+#             */\n",
        "/*   Updated: 2026/07/30 12:00:00 by student          ###   ########.fr       */\n",
        "/*                                                                            */\n",
        "/* ************************************************************************** */\n",
        "\n",
        "int\tanswer(void)\n",
        "{\n",
        "\treturn (42);\n",
        "}\n",
    );
    let options = CActionOptions {
        remove_invalid_comments: true,
        ..CActionOptions::default()
    };
    let fixed = apply_c_actions(
        Utf8Path::new("fixture.c"),
        source,
        &[diagnostic("COMMENT_ON_INSTR", 1, 1)],
        &options,
    )
    .unwrap()
    .source;
    assert!(fixed.starts_with(
        "/* ************************************************************************** */"
    ));
    assert!(fixed.contains("By: student"));
}

#[test]
fn spliced_line_comment_is_removed_as_one_comment() {
    let source = concat!(
        "int\tanswer(void)\n",
        "{\n",
        "\t// rejected \\\n",
        "\tcontinued text that must never become code\n",
        "\treturn (42);\n",
        "}\n",
    );
    let options = CActionOptions {
        remove_invalid_comments: true,
        ..CActionOptions::default()
    };
    let fixed = apply_c_actions(
        Utf8Path::new("comment.c"),
        source,
        &[diagnostic("WRONG_SCOPE_COMMENT", 3, 5)],
        &options,
    )
    .unwrap()
    .source;
    assert!(!fixed.contains("continued text"));
    assert!(fixed.contains("return (42);"));
}
