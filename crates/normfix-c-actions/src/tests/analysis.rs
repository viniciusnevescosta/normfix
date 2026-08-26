use super::*;

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
fn an_opaque_va_arg_call_does_not_block_proven_layout_around_it() {
    let source = concat!(
        "char *next_string(va_list *args)\n",
        "{\n",
        " return (va_arg(*args, char *));\n",
        "}\n",
    );

    let fixed = apply(source, &[]);

    assert!(fixed.contains("va_arg(*args, char *)"));
    assert!(fixed.contains("\n\treturn (va_arg"));
    assert_ne!(fixed, source);
    assert_eq!(apply(&fixed, &[]), fixed);
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
