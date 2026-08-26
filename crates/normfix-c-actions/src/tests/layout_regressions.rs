use super::*;

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
    let fixed = apply(source, &[]);
    assert!(fixed.contains("\twhile (c--)\n\t\t;\n"), "{fixed}");
    // The `for` is forbidden and becomes a `while`, which moves its step into
    // the body. Its own empty body then holds nothing and is no longer a loop
    // body, so losing it is the one case where losing it changes nothing.
    assert!(
        fixed.contains("\tc = 0;\n\twhile (c < 2)\n\t\tc++;\n"),
        "{fixed}"
    );
    assert_eq!(apply(&fixed, &[]), fixed);
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
