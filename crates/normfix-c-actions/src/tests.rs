use std::fmt::Write;

use camino::Utf8Path;
use normfix_core::Severity;

use crate::{
    CActionError, CActionOptions, ReportedDiagnostic, analyze_budget, analyze_c,
    analyze_external_calls, apply_c_actions, normalize_hygiene, visual_width,
};

fn diagnostic(code: &str, line: u32, column: u32) -> ReportedDiagnostic {
    ReportedDiagnostic::new(code, line, column, code)
}

fn apply(source: &str, diagnostics: &[ReportedDiagnostic]) -> String {
    apply_c_actions(
        Utf8Path::new("fixture.c"),
        source,
        diagnostics,
        &CActionOptions::default(),
    )
    .expect("fixture must remain safe")
    .source
}

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

#[test]
fn structural_analysis_reports_exact_limits_and_manual_guidance() {
    let mut source = String::from("int\tmany(int a, int b, int c, int d, int e)\n{\n");
    for value in 0..26 {
        writeln!(source, "\ta = a + {value};").unwrap();
    }
    source.push_str("\treturn (a);\n}\n");
    let diagnostics = analyze_c(Utf8Path::new("many.c"), &source, 80).unwrap();
    assert!(
        diagnostics
            .iter()
            .any(|item| item.rule_id == "TOO_MANY_LINES")
    );
    assert!(
        diagnostics
            .iter()
            .any(|item| item.rule_id == "TOO_MANY_ARGS")
    );

    let result = apply_c_actions(
        Utf8Path::new("manual.c"),
        "int\tmain(void)\n{\n\treturn (0);\n}\n",
        &[diagnostic("VLA_FORBIDDEN", 3, 2)],
        &CActionOptions::default(),
    )
    .unwrap();
    let vla = result
        .diagnostics
        .iter()
        .find(|item| item.rule_id == "VLA_FORBIDDEN")
        .unwrap();
    assert!(
        vla.help
            .as_deref()
            .unwrap()
            .contains("compile-time constant")
    );
}

proptest::proptest! {
    // Reordering moves whole lines, so the risk is losing, duplicating, or
    // mangling one. Assert the invariants on generated blocks instead of a
    // single example.
    #![proptest_config(proptest::prelude::ProptestConfig::with_cases(192))]

    #[test]
    fn formatting_is_idempotent_for_any_generated_function(
        bodies in proptest::collection::vec(
            proptest::sample::select(vec![
                "return 0;",
                "return (0);",
                "if(a)return 1;",
                "if (a)\n\t\treturn (1);",
                "while(a){a--;}",
                "a = b + c;",
                "return a+b;",
            ]),
            1..6,
        ),
        tabs in proptest::bool::ANY,
    ) {
        let indent = if tabs { "\t" } else { "  " };
        let mut source = String::new();
        for (index, body) in bodies.iter().enumerate() {
            let _ = write!(
                source,
                "int\tfn_{index}(int a, int b, int c)\n{{\n{indent}{body}\n}}\n\n"
            );
        }

        let once = apply(&source, &[]);
        let twice = apply(&once, &[]);
        proptest::prop_assert_eq!(&once, &twice, "a second run kept changing the file");
    }

    #[test]
    fn formatting_never_loses_a_significant_identifier(
        names in proptest::collection::vec("[a-z][a-z_0-9]{2,10}", 1..6),
    ) {
        let mut source = String::new();
        for name in &names {
            let _ = write!(source, "int {name}(int a){{\nreturn a;\n}}\n\n");
        }

        let fixed = apply(&source, &[]);

        // Layout may move bytes anywhere, but a function name is a significant
        // token and must survive verbatim.
        for name in &names {
            proptest::prop_assert!(
                fixed.contains(name.as_str()),
                "{name} disappeared from the formatted source"
            );
        }
    }

    #[test]
    fn reordering_preserves_every_include_and_sorts_it(
        includes in proptest::collection::vec(
            (proptest::bool::ANY, "[a-z][a-z_0-9]{0,7}"),
            1..8,
        )
    ) {
        let block = includes
            .iter()
            .map(|(system, name)| {
                if *system {
                    format!("#include <{name}.h>\n")
                } else {
                    format!("#include \"{name}.h\"\n")
                }
            })
            .collect::<Vec<_>>();
        let source = format!(
            "{}\nint\tmain(void)\n{{\n\treturn (0);\n}}\n",
            block.concat()
        );

        let fixed = apply(&source, &[]);
        let fixed_block = fixed
            .lines()
            .take_while(|line| line.starts_with("#include "))
            .map(|line| format!("{line}\n"))
            .collect::<Vec<_>>();

        // Nothing is lost, duplicated, or rewritten: the same multiset returns.
        let mut before = block.clone();
        let mut after = fixed_block.clone();
        before.sort();
        after.sort();
        proptest::prop_assert_eq!(&before, &after);

        // System headers first, then alphabetically inside each category.
        let keys = fixed_block
            .iter()
            .map(|line| {
                let system = line.contains('<');
                let name = line
                    .trim_end()
                    .trim_start_matches("#include ")
                    .trim_matches(['<', '>', '"'])
                    .to_ascii_lowercase();
                (u8::from(!system), name)
            })
            .collect::<Vec<_>>();
        proptest::prop_assert!(keys.windows(2).all(|pair| pair[0] <= pair[1]));

        // A second run changes nothing.
        proptest::prop_assert_eq!(apply(&fixed, &[]), fixed);
    }
}

#[test]
fn include_block_is_reordered_with_system_headers_first_then_alphabetically() {
    let source = concat!(
        "# include \"zeta.h\"\n",
        "# include <stdlib.h>\n",
        "# include <limits.h>\n",
        "# include \"alpha.h\"\n",
        "\n",
        "int\tmain(void)\n",
        "{\n",
        "\treturn (0);\n",
        "}\n",
    );

    let fixed = apply(source, &[]);

    assert!(
        fixed.starts_with(concat!(
            "#include <limits.h>\n",
            "#include <stdlib.h>\n",
            "#include \"alpha.h\"\n",
            "#include \"zeta.h\"\n",
        )),
        "{fixed}"
    );
    assert_eq!(
        apply(&fixed, &[]),
        fixed,
        "reordering must reach a fixpoint"
    );
    assert!(
        analyze_c(Utf8Path::new("includes.c"), &fixed, 80)
            .unwrap()
            .iter()
            .all(|diagnostic| diagnostic.rule_id != "INCLUDE_ORDER_REVIEW")
    );
}

#[test]
fn an_interrupted_include_block_keeps_each_run_in_place() {
    let source = concat!(
        "# include \"zeta.h\"\n",
        "# include <stdlib.h>\n",
        "# ifdef DEBUG\n",
        "#  include <stdio.h>\n",
        "# endif\n",
        "# include \"beta.h\"\n",
        "# include <limits.h>\n",
        "\n",
        "int\tmain(void)\n",
        "{\n",
        "\treturn (0);\n",
        "}\n",
    );

    let fixed = apply(source, &[]);

    // Each contiguous run is sorted on its own; nothing crosses the conditional.
    assert!(
        fixed.contains(concat!(
            "#include <stdlib.h>\n",
            "#include \"zeta.h\"\n",
            "#ifdef DEBUG\n",
        )),
        "{fixed}"
    );
    assert!(
        fixed.contains(concat!(
            "#endif\n",
            "#include <limits.h>\n",
            "#include \"beta.h\"\n",
        )),
        "{fixed}"
    );
}

#[test]
fn include_reordering_can_be_disabled() {
    let source = concat!(
        "# include \"zeta.h\"\n",
        "# include <stdlib.h>\n",
        "\n",
        "int\tmain(void)\n",
        "{\n",
        "\treturn (0);\n",
        "}\n",
    );
    let options = CActionOptions {
        reorder_includes: false,
        ..CActionOptions::default()
    };

    let fixed = apply_c_actions(Utf8Path::new("fixture.c"), source, &[], &options)
        .expect("fixture must remain safe")
        .source;

    assert!(
        fixed.starts_with("#include \"zeta.h\"\n#include <stdlib.h>\n"),
        "{fixed}"
    );
}

#[test]
fn include_order_is_reported_without_reordering_preprocessor_tokens() {
    let source = "# include \"zeta.h\"\n# include <stdlib.h>\n# include <limits.h>\n# include \"alpha.h\"\n\nint\tmain(void)\n{\n\treturn (0);\n}\n";

    let diagnostics = analyze_c(Utf8Path::new("includes.c"), source, 80).unwrap();
    let order = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.rule_id == "INCLUDE_ORDER_REVIEW")
        .expect("include order review");

    assert_eq!(order.severity, Severity::Warning);
    assert!(
        order
            .help
            .as_deref()
            .is_some_and(|help| help.contains("preprocessor order"))
    );
}

#[test]
fn structural_analysis_counts_initial_locals_and_function_definitions() {
    let source = concat!(
        "int\tfirst(void)\n",
        "{\n",
        "\tint\ta;\n",
        "\tint\tb;\n",
        "\tint\tc;\n",
        "\tint\td;\n",
        "\tint\te;\n",
        "\tint\tf;\n",
        "\treturn (a + b + c + d + e + f);\n",
        "}\n",
        "int\tsecond(void)\n{\n\treturn (2);\n}\n",
        "int\tthird(void)\n{\n\treturn (3);\n}\n",
        "int\tfourth(void)\n{\n\treturn (4);\n}\n",
        "int\tfifth(void)\n{\n\treturn (5);\n}\n",
        "int\tsixth(void)\n{\n\treturn (6);\n}\n",
    );
    let diagnostics = analyze_c(Utf8Path::new("many.c"), source, 80).unwrap();
    assert!(
        diagnostics
            .iter()
            .any(|item| item.rule_id == "TOO_MANY_VARS_FUNC")
    );
    assert!(
        diagnostics
            .iter()
            .any(|item| item.rule_id == "TOO_MANY_FUNCS")
    );
}

#[test]
fn parser_recovery_blocks_all_actions() {
    let error = apply_c_actions(
        Utf8Path::new("broken.c"),
        "int\tmain( {\n\treturn (0)\n",
        &[],
        &CActionOptions::default(),
    )
    .unwrap_err();
    assert_eq!(error, CActionError::UnsafeSyntax);
}

#[test]
fn control_braces_are_printed_on_their_own_lines_without_oracle_diagnostics() {
    let source = concat!(
        "void\trotate(t_context *ctx, int index)\n",
        "{\n",
        "\tif (index > 0) {\n",
        "\t\tra(ctx);\n",
        "\t\tindex--;\n",
        "\t} else {\n",
        "\t\tindex = ctx->a.size - index;\n",
        "\t\twhile (index > 0) {\n",
        "\t\t\trra(ctx);\n",
        "\t\t\tindex--;\n",
        "\t\t}\n",
        "\t}\n",
        "}\n",
    );
    let fixed = apply(source, &[]);
    assert!(fixed.contains("\telse\n\t{"), "{fixed}");
    assert!(fixed.contains("\t\twhile (index > 0)\n\t\t{"));
    assert!(!fixed.contains("else {"));
    assert_eq!(apply(&fixed, &[]), fixed);
}

#[test]
fn single_scope_free_statements_lose_braces_but_declarations_and_comments_do_not() {
    let source = concat!(
        "int\tcheck(int value)\n",
        "{\n",
        "\tif (value)\n",
        "\t{\n",
        "\t\treturn (1);\n",
        "\t}\n",
        "\tif (!value)\n",
        "\t{\n",
        "\t\tint\tcopy;\n",
        "\t}\n",
        "\twhile (value)\n",
        "\t{\n",
        "\t\t/* keep scope and comment */\n",
        "\t\tvalue--;\n",
        "\t}\n",
        "\treturn (0);\n",
        "}\n",
    );
    let fixed = apply(source, &[]);
    assert!(fixed.contains("\tif (value)\n\t\treturn (1);"));
    assert!(fixed.contains("\tif (!value)\n\t{"));
    assert!(fixed.contains("/* keep scope and comment */"));
    assert!(fixed.contains("\twhile (value)\n\t{"));
    assert_eq!(apply(&fixed, &[]), fixed);
}

#[test]
fn else_after_a_single_return_is_removed_structurally() {
    let source = concat!(
        "int\tget_chunk_size(int size)\n",
        "{\n",
        "\tif (size <= 100)\n",
        "\t{\n",
        "\t\treturn (20);\n",
        "\t}\n",
        "\telse\n",
        "\t{\n",
        "\t\treturn (40);\n",
        "\t}\n",
        "}\n",
    );
    let fixed = apply(source, &[]);
    assert_eq!(
        fixed,
        concat!(
            "int\tget_chunk_size(int size)\n",
            "{\n",
            "\tif (size <= 100)\n",
            "\t\treturn (20);\n",
            "\treturn (40);\n",
            "}\n",
        )
    );
    assert_eq!(apply(&fixed, &[]), fixed);
}

#[test]
fn initial_declaration_is_indented_tabbed_and_separated_without_a_diagnostic() {
    let source = concat!(
        "void\tsort_simple(t_context *ctx)\n",
        "{\n",
        "\t\tint min;\n",
        "\twhile (ctx->a.size > 0)\n",
        "\t{\n",
        "\t\tmin = find_min_index(&ctx->a);\n",
        "\t\tpb(ctx);\n",
        "\t}\n",
        "}\n",
    );
    let fixed = apply(source, &[]);
    assert!(fixed.contains("{\n\tint\tmin;\n\n\twhile"));
    assert_eq!(apply(&fixed, &[]), fixed);
}

#[test]
fn null_compaction_is_unsafe_opt_in_and_preserves_boolean_value() {
    let source = concat!(
        "int\tpresent(char *value)\n",
        "{\n",
        "\tif (value == NULL)\n",
        "\t\treturn (0);\n",
        "\treturn (value != NULL);\n",
        "}\n",
    );
    assert_eq!(apply(source, &[]), source);
    let options = CActionOptions {
        compact_null_checks: true,
        ..CActionOptions::default()
    };
    let fixed = apply_c_actions(Utf8Path::new("null.c"), source, &[], &options)
        .unwrap()
        .source;
    assert!(fixed.contains("if (!value)"));
    assert!(fixed.contains("return (!!value);"));
}

#[test]
fn pointer_zero_return_uses_null_only_with_a_prior_proven_provider() {
    let proven = concat!(
        "#include <stddef.h>\n",
        "\n",
        "char\t*next_value(void)\n",
        "{\n",
        "\treturn (0);\n",
        "}\n",
    );
    let fixed = apply(proven, &[]);
    assert!(fixed.contains("return (NULL);"));

    let unavailable = concat!("char\t*next_value(void)\n", "{\n", "\treturn (0);\n", "}\n",);
    let result = apply_c_actions(
        Utf8Path::new("pointer.c"),
        unavailable,
        &[],
        &CActionOptions::default(),
    )
    .unwrap();
    assert_eq!(result.source, unavailable);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|item| item.rule_id == "POINTER_ZERO_RETURN_REVIEW")
    );

    let invalidated = concat!(
        "#include <stddef.h>\n",
        "#undef NULL\n",
        "\n",
        "char\t*next_value(void)\n",
        "{\n",
        "\treturn (0);\n",
        "}\n",
    );
    assert!(apply(invalidated, &[]).contains("return (0);"));

    let conditional = concat!(
        "#ifdef FEATURE\n",
        "# include <stddef.h>\n",
        "#endif\n",
        "\n",
        "char\t*next_value(void)\n",
        "{\n",
        "\treturn (0);\n",
        "}\n",
    );
    assert!(apply(conditional, &[]).contains("return (0);"));
}

#[test]
fn review_analysis_and_budget_are_structured_but_budget_is_not_emitted() {
    let source = concat!(
        "struct context\n",
        "{\n",
        "\tint\tvalue;\n",
        "};\n",
        "int\thelper(void)\n",
        "{\n",
        "\twhile (1)\n",
        "\t{\n",
        "\t\thelper();\n",
        "\t}\n",
        "\tint\tlate;\n",
        "\treturn (late);\n",
        "}\n",
    );
    let diagnostics = analyze_c(Utf8Path::new("review.c"), source, 80).unwrap();
    for rule in [
        "STRUCT_TYPE_NAMING_REVIEW",
        "LATE_DECLARATION_REVIEW",
        "POSSIBLE_INFINITE_LOOP",
    ] {
        assert!(
            diagnostics.iter().any(|item| item.rule_id == rule),
            "missing {rule}"
        );
    }
    assert!(
        !diagnostics
            .iter()
            .any(|item| item.rule_id == "FUNCTION_BUDGET")
    );
    let budgets = analyze_budget(Utf8Path::new("review.c"), source).unwrap();
    assert_eq!(budgets.len(), 1);
    assert_eq!(budgets[0].path, Utf8Path::new("review.c"));
    assert_eq!(budgets[0].function, "helper");
    assert_eq!(budgets[0].line, 5);
    assert_eq!(budgets[0].parameters, 0);
    assert_eq!(budgets[0].variables, 1);
    assert_eq!(budgets[0].line_limit, 25);
    assert_eq!(budgets[0].variable_limit, 5);
    assert_eq!(budgets[0].parameter_limit, 4);
}

#[test]
fn external_call_candidates_exclude_definitions_and_shadowing_function_pointers() {
    let source = concat!(
        "static void\thelper(void)\n",
        "{\n",
        "}\n",
        "void\trun(void (*callback)(void))\n",
        "{\n",
        "\tint\t(*local_callback)(void);\n",
        "\n",
        "\thelper();\n",
        "\tcallback();\n",
        "\tlocal_callback();\n",
        "\twrite(1, \"x\", 1);\n",
        "}\n",
    );
    let candidates = analyze_external_calls(Utf8Path::new("calls.c"), source).unwrap();
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].path, Utf8Path::new("calls.c"));
    assert_eq!(candidates[0].name, "write");
    assert_eq!(
        &source[candidates[0].name_range.start().get() as usize
            ..candidates[0].name_range.end().get() as usize],
        "write"
    );
}

#[test]
fn external_call_candidates_track_nested_local_scope_without_hiding_later_calls() {
    let source = concat!(
        "void\trun(int enabled)\n",
        "{\n",
        "\tif (enabled)\n",
        "\t{\n",
        "\t\tint\t(*nested)(void);\n",
        "\n",
        "\t\tnested();\n",
        "\t}\n",
        "\texternal_call();\n",
        "}\n",
    );
    let candidates = analyze_external_calls(Utf8Path::new("nested.c"), source).unwrap();
    assert_eq!(
        candidates
            .iter()
            .map(|candidate| candidate.name.as_str())
            .collect::<Vec<_>>(),
        ["external_call"]
    );
}

#[test]
fn external_call_candidates_fail_closed_for_macros_and_ambiguous_declarations() {
    let macros = concat!(
        "#define WRAPPER() hidden_external()\n",
        "#define TARGET hidden_target\n",
        "void\trun(void)\n",
        "{\n",
        "\tWRAPPER();\n",
        "\tTARGET();\n",
        "\tvisible_external();\n",
        "}\n",
    );
    let candidates = analyze_external_calls(Utf8Path::new("macros.c"), macros).unwrap();
    assert_eq!(
        candidates
            .iter()
            .map(|candidate| candidate.name.as_str())
            .collect::<Vec<_>>(),
        ["visible_external"]
    );

    let ambiguous = concat!(
        "void\trun(void)\n",
        "{\n",
        "\tint\tfirst, unknown_shadow;\n",
        "\n",
        "\tvisible_external();\n",
        "}\n",
    );
    assert!(
        analyze_external_calls(Utf8Path::new("ambiguous.c"), ambiguous)
            .unwrap()
            .is_empty()
    );

    let token_paste = concat!(
        "#define JOIN(a, b) a ## b\n",
        "void\trun(void)\n",
        "{\n",
        "\tvisible_external();\n",
        "}\n",
    );
    assert!(
        analyze_external_calls(Utf8Path::new("paste.c"), token_paste)
            .unwrap()
            .is_empty()
    );
}

fn visual_column(line: &str, needle: &str) -> u32 {
    let index = line.find(needle).unwrap();
    visual_width(&line[..index]) + 1
}

#[test]
fn else_is_removed_only_when_both_branches_are_a_single_return() {
    // Reported as a bug from the playground, and it is not one: the proof
    // requires each branch to be exactly one `return`, which makes dropping the
    // `else` behaviour-preserving. This pins that the proof is what decides.
    let provable = concat!(
        "int main(void)\n",
        "{\n",
        "\tif (x)\n\t\treturn (0);\n",
        "\telse\n\t\treturn (1);\n",
        "}\n",
    );
    assert!(
        !apply(provable, &[]).contains("else"),
        "both branches return, so the else carries no behaviour",
    );

    // The case the proof exists to refuse. The consequence does not always
    // return, so removing the `else` would run the alternative unconditionally.
    let unprovable = concat!(
        "int main(void)\n",
        "{\n",
        "\tif (x)\n",
        "\t{\n\t\ty = 1;\n\t\treturn (0);\n\t}\n",
        "\telse\n\t\treturn (1);\n",
        "}\n",
    );
    let fixed = apply(unprovable, &[]);
    assert!(
        fixed.contains("else"),
        "the consequence is more than one return, so the else must stay:\n{fixed}",
    );
}

#[test]
fn a_brace_on_the_condition_line_does_not_cost_every_other_fix() {
    // Observed on a real file: the official checker reports EXP_NEWLINE for the
    // body sharing the condition's line, and the native layout rule already
    // moves that brace. Both edited the same byte with different text, so the
    // batch was rejected as conflicting — and every other fix in the file died
    // with it. The file came back with 23 findings and 0 fixes.
    let source = concat!(
        "int main(void)\n",
        "{\n",
        "    if(x > 0) { return (0); }\n",
        "}\n",
    );
    let diagnostics = [
        diagnostic("EXP_NEWLINE", 3, 15),
        diagnostic("SPACE_AFTER_KW", 3, 5),
    ];

    let fixed = apply(source, &diagnostics);

    // The brace edit is the one that used to collide. What proves the batch
    // survived is that the *other* fix in it landed.
    assert!(
        fixed.contains("if (x"),
        "the whole batch was lost with the conflicting edit:\n{fixed}",
    );
    assert!(
        !fixed.contains("> 0) {"),
        "the brace was left on the condition line:\n{fixed}",
    );
}

#[test]
fn a_statement_that_is_only_a_semicolon_is_removed() {
    let source = concat!(
        "int\tanswer(void)\n",
        "{\n",
        "\tif (1)\n",
        "\t{\n",
        "\t\treturn (42);\n",
        "\t};\n",
        "\treturn (0);;\n",
        "}\n",
        ";\n",
    );
    let fixed = apply(source, &[]);
    assert!(!fixed.contains(";;"), "{fixed}");
    assert!(!fixed.contains("};"), "{fixed}");
    assert!(!fixed.ends_with(";\n;\n"), "{fixed}");
    assert!(fixed.contains("return (42);"), "{fixed}");
    assert_eq!(apply(&fixed, &[]), fixed, "the removal must be idempotent");
}

#[test]
fn an_empty_loop_body_is_never_mistaken_for_a_stray_semicolon() {
    // `while (x)\n\t;` is a real body. Deleting it would promote the next
    // statement into the loop, which is a different program.
    let source = concat!(
        "int\tanswer(int c)\n",
        "{\n",
        "\twhile (c--)\n",
        "\t\t;\n",
        "\tfor (c = 0; c < 2; c++)\n",
        "\t\t;\n",
        "\treturn (c);\n",
        "}\n",
    );
    assert_eq!(apply(source, &[]), source);
}

#[test]
fn a_semicolon_after_a_preprocessor_branch_is_left_alone() {
    // The `;` may terminate a statement that only exists when the macro is
    // defined, and this parse cannot see that configuration.
    let source = concat!(
        "int\tanswer(void)\n",
        "{\n",
        "#ifdef TRACE\n",
        "\tanswer();\n",
        "#endif\n",
        "\t;\n",
        "\treturn (0);\n",
        "}\n",
    );
    assert_eq!(apply(source, &[]), source);
}

#[test]
fn a_run_without_an_official_report_still_reaches_norm_layout() {
    // The browser playground has no Norminette, so every rule that was gated
    // on an official diagnostic was silently inert there: it handed back a
    // file it called formatted while `if(`, a one-line block, and space
    // indentation were all still in it.
    let source = concat!(
        "#include <unistd.h>\n",
        "\n",
        "int main(void)\n",
        "{\n",
        "    if(write(1, \"x\", 1) > 0) { return (0); }\n",
        "    else { return (1); }\n",
        "}\n",
    );
    let fixed = apply(source, &[]);
    assert_eq!(
        fixed,
        concat!(
            "#include <unistd.h>\n",
            "\n",
            "int\tmain(void)\n",
            "{\n",
            "\tif (write(1, \"x\", 1) > 0)\n",
            "\t\treturn (0);\n",
            "\treturn (1);\n",
            "}\n",
        ),
        "{fixed}"
    );
    assert_eq!(apply(&fixed, &[]), fixed, "the layout must be idempotent");
}

#[test]
fn a_call_never_gains_a_space_before_its_parenthesis() {
    // The keyword set is reserved, so nothing else may be widened. A call that
    // gained a space here would be a new Norm error, not a fixed one.
    let source = concat!(
        "int\tanswer(void)\n",
        "{\n",
        "\tif (answer() > 0)\n",
        "\t\treturn (sizeof(int));\n",
        "\treturn (0);\n",
        "}\n",
    );
    assert_eq!(apply(source, &[]), source);
}

#[test]
fn indentation_inside_a_string_or_comment_is_content() {
    let source = concat!(
        "char\t*answer(void)\n",
        "{\n",
        "\t/*\n",
        "    indented on purpose\n",
        "\t*/\n",
        "\treturn (\"line\\n\\\n",
        "    kept as written\");\n",
        "}\n",
    );
    let fixed = apply(source, &[]);
    assert!(fixed.contains("    indented on purpose"), "{fixed}");
    assert!(fixed.contains("    kept as written"), "{fixed}");
}

#[test]
fn spacing_and_layout_match_the_checker_run_without_one() {
    // Every gap this covers was found by running the same file through both
    // paths and diffing: the terminal fixed it, the browser did not.
    let source = concat!(
        "int\tadd(int a,int b)\n",
        "{\n",
        "\tint\tsum;\n",
        "\n",
        "\tsum = a+b;\n",
        "\tsum = sum-1;\n",
        "\tif (sum>10)\n",
        "\t\tsum = sum*2;\n",
        "\treturn (sum);\n",
        "}\n",
    );
    assert_eq!(
        apply(source, &[]),
        concat!(
            "int\tadd(int a, int b)\n",
            "{\n",
            "\tint\tsum;\n",
            "\n",
            "\tsum = a + b;\n",
            "\tsum = sum - 1;\n",
            "\tif (sum > 10)\n",
            "\t\tsum = sum * 2;\n",
            "\treturn (sum);\n",
            "}\n",
        )
    );
}

#[test]
fn a_unary_operator_never_gains_a_space() {
    // The grammar, not the surrounding whitespace, decides this. A rule that
    // read the text would turn `-a` into `- a` and `a++` into `a+ +`.
    let source = concat!(
        "int\tsign(int a)\n",
        "{\n",
        "\ta++;\n",
        "\ta = -a;\n",
        "\ta = !a;\n",
        "\treturn (a);\n",
        "}\n",
    );
    assert_eq!(apply(source, &[]), source);
}

#[test]
fn a_declarator_star_binds_to_its_name_and_multiplication_does_not() {
    let source = concat!(
        "int\tarea(int * width, int height)\n",
        "{\n",
        "\treturn (*(&width[0]) * height);\n",
        "}\n",
    );
    let fixed = apply(source, &[]);
    assert!(fixed.contains("int *width"), "{fixed}");
    assert!(fixed.contains("]) * height"), "{fixed}");
}

#[test]
fn a_brace_written_against_the_condition_still_moves() {
    // `){` has no whitespace to replace, and the rule used to bail there,
    // leaving an official error behind a run that reported success.
    let source = concat!(
        "int\tpick(int n)\n",
        "{\n",
        "\twhile (n > 0){\n",
        "\t\tn--;\n",
        "\t}\n",
        "\treturn (n);\n",
        "}\n",
    );
    let fixed = apply(source, &[]);
    assert!(fixed.contains("while (n > 0)\n"), "{fixed}");
    assert!(!fixed.contains("){"), "{fixed}");
}

#[test]
fn a_control_body_and_a_brace_never_share_a_line() {
    let source = concat!(
        "int\tpick(int n)\n",
        "{\n",
        "\tif (n)n = 2;\n",
        "\treturn (n);\n",
        "}\n",
    );
    let fixed = apply(source, &[]);
    assert!(fixed.contains("\tif (n)\n\t\tn = 2;\n"), "{fixed}");
}

#[test]
fn a_blank_line_never_sits_against_a_brace() {
    let source = concat!(
        "int\tfirst(void)\n",
        "{\n",
        "\n",
        "\treturn (1);\n",
        "\n",
        "}\n",
    );
    assert_eq!(
        apply(source, &[]),
        concat!("int\tfirst(void)\n", "{\n", "\treturn (1);\n", "}\n")
    );
}

#[test]
fn nested_braceless_bodies_each_take_a_level() {
    // The indentation model counted braces and treated everything else as one
    // continuation, so a second braceless `if` shared the first one's level and
    // the file kept an official TOO_FEW_TAB after a run that reported success.
    let source = concat!(
        "int\tf(int x)\n",
        "{\n",
        "    if (x)\n",
        "        if (x > 1)\n",
        "            return (2);\n",
        "    return (0);\n",
        "}\n",
    );
    assert_eq!(
        apply(source, &[]),
        concat!(
            "int\tf(int x)\n",
            "{\n",
            "\tif (x)\n",
            "\t\tif (x > 1)\n",
            "\t\t\treturn (2);\n",
            "\treturn (0);\n",
            "}\n",
        )
    );
}

#[test]
fn an_expression_split_over_lines_stays_one_level_deep() {
    // The counterpart to the rule above: an unfinished expression indents its
    // continuation once, however many lines it runs for. Counting it the way a
    // control header is counted would push each line one deeper than the last.
    let source = concat!(
        "int\tf(int alpha, int beta, int gamma)\n",
        "{\n",
        "\treturn (alpha * beta + gamma * alpha + beta * gamma +\n",
        "\t\talpha * gamma + beta * alpha +\n",
        "\t\tgamma * beta);\n",
        "}\n",
    );
    let fixed = apply(source, &[]);
    for line in fixed.lines().filter(|line| line.starts_with('\t')) {
        let depth = line.bytes().take_while(|byte| *byte == b'\t').count();
        assert!(
            depth <= 2,
            "a continuation kept stacking: {line:?}\n{fixed}"
        );
    }
}

#[test]
fn a_subscript_carries_no_padding() {
    let source = concat!(
        "int\tf(void)\n",
        "{\n",
        "\tint\tn[3];\n",
        "\n",
        "\tn[ 2 ] = 3;\n",
        "\treturn (n[ 0 ]);\n",
        "}\n",
    );
    let fixed = apply(source, &[]);
    assert!(fixed.contains("n[2] = 3;"), "{fixed}");
    assert!(fixed.contains("return (n[0]);"), "{fixed}");
}

#[test]
fn a_run_parses_the_source_once_per_pass_and_no_more() {
    // A parse dominates a run, and the scheduler decides how many it needs.
    // This release doubled that number without anyone noticing: a pass parsed
    // to plan against, validating the accepted batch parsed the result, and
    // that parse was discarded so the next pass re-read the same bytes. The
    // benchmark in CI is informational and would not have failed.
    //
    // The budget is one parse to start, one per accepted batch, and nothing
    // else. It is a property of the code, so it holds on any machine.
    let messy = concat!(
        "int ft_probe(int argc,char **argv){\n",
        "  int index;\n",
        "  char *value;\n",
        "  index = 0;\n",
        "  value = argv[argc-1];\n",
        "  while(index<argc){\n",
        "    if(value[index]=='-'){\n",
        "      return index;\n",
        "    }\n",
        "    index++;\n",
        "  }\n",
        "  return 0;\n",
        "}\n",
    );
    let result = apply_c_actions(
        Utf8Path::new("budget.c"),
        messy,
        &[],
        &CActionOptions::default(),
    )
    .expect("the fixture must stay safe");

    assert!(
        result.accepted_batches > 1,
        "the fixture must need several batches to be worth measuring"
    );
    assert_eq!(
        result.parses,
        result.accepted_batches + 2,
        "one parse of the input, one of the hygiene result, one per accepted          batch, and no others"
    );
}

#[test]
fn a_declaration_is_separated_from_the_value_it_was_given() {
    // The official checker calls this DECL_ASSIGN_LINE and normfix only ever
    // reported it. The assignments keep declaration order, so an initializer
    // that reads a variable declared above it still reads what was assigned.
    let source = concat!(
        "int\tf(int n)\n",
        "{\n",
        "\tint\tbase = n * 2;\n",
        "\tint\ttotal = base + 1;\n",
        "\n",
        "\treturn (total);\n",
        "}\n",
    );
    assert_eq!(
        apply(source, &[]),
        concat!(
            "int\tf(int n)\n",
            "{\n",
            "\tint\tbase;\n",
            "\tint\ttotal;\n",
            "\n",
            "\tbase = n * 2;\n",
            "\ttotal = base + 1;\n",
            "\treturn (total);\n",
            "}\n",
        )
    );
}

#[test]
fn a_pointer_declaration_assigns_the_name_not_the_star() {
    let source = concat!(
        "char\t*f(char *s)\n",
        "{\n",
        "\tchar\t*p = s;\n",
        "\n",
        "\treturn (p);\n",
        "}\n",
    );
    let fixed = apply(source, &[]);
    assert!(fixed.contains("\tchar\t*p;\n"), "{fixed}");
    assert!(fixed.contains("\tp = s;\n"), "{fixed}");
    assert!(!fixed.contains("*p = s;"), "{fixed}");
}

#[test]
fn a_declaration_that_cannot_be_assigned_later_is_left_alone() {
    // Each of these would be a different program after the split: a `const`
    // cannot be assigned at all, an aggregate initializer is initialization
    // syntax rather than an expression, a `static` is initialized once where an
    // assignment would run on every call, and one range cannot tell two
    // initializers apart.
    let source = concat!(
        "int\tf(void)\n",
        "{\n",
        "\tconst int\ta = 1;\n",
        "\tstatic int\tb = 2;\n",
        "\tint\t\tc[] = {1, 2};\n",
        "\tint\t\td = 1, e = 2;\n",
        "\n",
        "\treturn (a + b + c[0] + d + e);\n",
        "}\n",
    );
    // Each check names the value still attached to its declaration, because a
    // split leaves the declaration itself looking unchanged.
    let fixed = apply(source, &[]);
    for kept in [
        "const int\ta = 1;",
        "static int\tb = 2;",
        "c[] = {1, 2};",
        "d = 1, e = 2;",
    ] {
        assert!(fixed.contains(kept), "{kept:?} was rewritten:\n{fixed}");
    }
}
