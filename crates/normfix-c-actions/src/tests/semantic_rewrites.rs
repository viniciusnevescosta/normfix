use super::*;

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
    // syntax rather than an expression, and a `static` is initialized once
    // where an assignment would run on every call.
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
    for kept in ["const int\ta = 1;", "static int\tb = 2;", "c[] = {1, 2};"] {
        assert!(fixed.contains(kept), "{kept:?} was rewritten:\n{fixed}");
    }
    // Two initializers in one declaration are no longer beyond reach: each
    // name takes its own line first, and each value is then separated from a
    // declaration that holds exactly one.
    assert!(fixed.contains("\tint\td;\n"), "{fixed}");
    assert!(fixed.contains("\td = 1;\n\te = 2;\n"), "{fixed}");
}

#[test]
fn a_local_nothing_reads_is_removed_when_it_holds_nothing_that_runs() {
    let source = concat!(
        "int\tf(int n)\n",
        "{\n",
        "\tint\tnever_touched;\n",
        "\tint\tfrom_literal = 10;\n",
        "\tint\tused;\n",
        "\n",
        "\tused = n;\n",
        "\treturn (used);\n",
        "}\n",
    );
    let options = CActionOptions {
        remove_unused_variables: true,
        ..CActionOptions::default()
    };
    let fixed = apply_c_actions(Utf8Path::new("unused.c"), source, &[], &options)
        .expect("the fixture must stay safe")
        .source;
    assert!(!fixed.contains("never_touched"), "{fixed}");
    assert!(!fixed.contains("from_literal"), "{fixed}");
    assert!(fixed.contains("used = n;"), "{fixed}");
}

#[test]
fn a_declaration_holding_a_call_keeps_the_call_even_when_nothing_reads_it() {
    // This is the case a compiler's `-Wunused-variable` would wave through:
    // it fires for `int n = g();` exactly as for `int n;`, and deleting the
    // first deletes the call. A `malloc` there is sharper still — removing it
    // would repair a leak by accident.
    let source = concat!(
        "int\tg(void);\n",
        "\n",
        "int\tf(void)\n",
        "{\n",
        "\tint\tfrom_call = g();\n",
        "\n",
        "\treturn (0);\n",
        "}\n",
    );
    let options = CActionOptions {
        remove_unused_variables: true,
        ..CActionOptions::default()
    };
    let fixed = apply_c_actions(Utf8Path::new("call.c"), source, &[], &options)
        .expect("the fixture must stay safe")
        .source;
    assert!(fixed.contains("g();"), "the call was deleted:\n{fixed}");
}

#[test]
fn a_name_a_macro_mentions_is_never_removed() {
    // The tree does not show a macro body, so a local the tree sees once may
    // still be reached by text the tree never parsed.
    let source = concat!(
        "#define CLEAR() (tracked = 0)\n",
        "\n",
        "int\tf(void)\n",
        "{\n",
        "\tint\ttracked;\n",
        "\n",
        "\tCLEAR();\n",
        "\treturn (0);\n",
        "}\n",
    );
    let options = CActionOptions {
        remove_unused_variables: true,
        ..CActionOptions::default()
    };
    let fixed = apply_c_actions(Utf8Path::new("macro.c"), source, &[], &options)
        .expect("the fixture must stay safe")
        .source;
    assert!(fixed.contains("int\ttracked;"), "{fixed}");
}

#[test]
fn removing_an_unused_local_is_never_done_without_being_asked() {
    let source = concat!(
        "int\tf(void)\n",
        "{\n",
        "\tint\tnever_touched;\n",
        "\n",
        "\treturn (0);\n",
        "}\n",
    );
    assert!(apply(source, &[]).contains("never_touched"));
}

#[test]
fn a_control_body_starting_with_a_star_does_not_deadlock_two_rules() {
    // `*out = a;` opens with what also spells multiplication, so joining it to
    // the line above read the `)` of the header as an operand. The brace rule
    // put the body back on its own line, the join rule pulled it up again, and
    // the run gave up with every fix in the file lost. Assigning through a
    // pointer inside an `if` is in every project that has pointers at all.
    let source = concat!(
        "int\tf(int a, int *out)\n",
        "{\n",
        "\tif (a > 0)\n",
        "\t\t*out = a;\n",
        "\treturn (0);\n",
        "}\n",
    );
    let fixed = apply(source, &[]);
    assert!(fixed.contains("\tif (a > 0)\n\t\t*out = a;\n"), "{fixed}");
    assert_eq!(apply(&fixed, &[]), fixed);

    // The join itself still has to work where it was always right.
    let wrapped = concat!(
        "int\tvalue(void)\n",
        "{\n",
        "\treturn (sum(first,\n",
        "\t\tsecond));\n",
        "}\n",
    );
    assert!(apply(wrapped, &[]).contains("return (sum(first, second));"));
}

#[test]
fn a_chained_assignment_becomes_the_two_it_stood_for() {
    // `a = b = c = n;` assigns from the inside out, and each step reads what
    // the one before it left, so a longer chain unrolls in that order across
    // passes. A compound operator keeps its meaning: only the inner one holds
    // the value.
    let source = concat!(
        "int\tf(int n)\n",
        "{\n",
        "\tint\ta;\n",
        "\tint\tb;\n",
        "\tint\tc;\n",
        "\n",
        "\ta = b = c = n;\n",
        "\tb += c = 2;\n",
        "\treturn (a + b + c);\n",
        "}\n",
    );
    let fixed = apply(source, &[]);
    assert!(fixed.contains("\tc = n;\n\tb = c;\n\ta = b;\n"), "{fixed}");
    assert!(fixed.contains("\tc = 2;\n\tb += c;\n"), "{fixed}");
    assert_eq!(apply(&fixed, &[]), fixed);
}

#[test]
fn each_declared_name_gets_its_own_line_and_keeps_its_star() {
    // `int *p, q;` is one pointer and one int, which is the reason the Norm
    // asks for one per line. Copying each declarator as written keeps that
    // true; rebuilding a type from the specifiers would not.
    let source = concat!(
        "int\tshapes(int n)\n",
        "{\n",
        "\tint\ta, b;\n",
        "\tint\t*p, q;\n",
        "\n",
        "\ta = n;\n",
        "\tb = 2;\n",
        "\tp = &a;\n",
        "\tq = *p;\n",
        "\treturn (a + b + q);\n",
        "}\n",
    );
    let fixed = apply(source, &[]);
    assert!(fixed.contains("\tint\ta;\n"), "{fixed}");
    assert!(fixed.contains("\tint\tb;\n"), "{fixed}");
    assert!(fixed.contains("\tint\t*p;\n"), "{fixed}");
    assert!(fixed.contains("\tint\tq;\n"), "{fixed}");
    assert!(!fixed.contains(", "), "{fixed}");
    assert_eq!(apply(&fixed, &[]), fixed);
}

#[test]
fn a_second_instruction_on_a_line_gets_its_own() {
    let source = concat!(
        "int\tf(void)\n",
        "{\n",
        "\tint\ta;\n",
        "\tint\tb;\n",
        "\n",
        "\ta = 1; b = 2; a = b;\n",
        "\treturn (a);\n",
        "}\n",
    );
    let fixed = apply(source, &[]);
    assert!(fixed.contains("\ta = 1;\n\tb = 2;\n\ta = b;\n"), "{fixed}");
    assert_eq!(apply(&fixed, &[]), fixed);

    // A comment between the two would have to pick a line, and picking one
    // for the reader says something they did not.
    let annotated = concat!(
        "int\tf(void)\n",
        "{\n",
        "\tint\ta;\n",
        "\tint\tb;\n",
        "\n",
        "\ta = 1; /* x */ b = 2;\n",
        "\treturn (a + b);\n",
        "}\n",
    );
    assert!(apply(annotated, &[]).contains("a = 1; /* x */ b = 2;"));
}

#[test]
fn a_for_becomes_the_while_it_stood_for() {
    // The Norm forbids `for`. The pieces map across exactly: the initializer
    // runs once above the loop, the condition is the condition, and the step
    // goes last in the body, where it still runs after every iteration.
    let source = concat!(
        "int\tf(int n)\n",
        "{\n",
        "\tint\ti;\n",
        "\tint\ts;\n",
        "\n",
        "\ts = 0;\n",
        "\tfor (i = 0; i < n; i++)\n",
        "\t\ts += i;\n",
        "\treturn (s);\n",
        "}\n",
    );
    let fixed = apply(source, &[]);
    assert!(
        fixed.contains("\ti = 0;\n\twhile (i < n)\n\t{\n\t\ts += i;\n\t\ti++;\n\t}\n"),
        "{fixed}"
    );
    assert!(!fixed.contains("for ("), "{fixed}");
    assert_eq!(apply(&fixed, &[]), fixed);
}

#[test]
fn a_loop_inside_a_loop_is_reached_on_the_next_pass() {
    // Two edits over the same bytes would take the whole batch down, so the
    // outer loop goes first and the inner one is still a loop in a block
    // afterwards. It only converges if the phase is allowed to come back.
    let source = concat!(
        "int\tf(int n)\n",
        "{\n",
        "\tint\ti;\n",
        "\tint\tj;\n",
        "\tint\ts;\n",
        "\n",
        "\ts = 0;\n",
        "\tfor (i = 0; i < n; i++)\n",
        "\t{\n",
        "\t\tfor (j = 0; j < i; j++)\n",
        "\t\t\ts += j;\n",
        "\t}\n",
        "\treturn (s);\n",
        "}\n",
    );
    let fixed = apply(source, &[]);
    assert!(!fixed.contains("for ("), "{fixed}");
    assert!(fixed.contains("\t\tj = 0;\n\t\twhile (j < i)\n"), "{fixed}");
}

#[test]
fn a_for_whose_step_would_be_skipped_stays() {
    // `continue` reaches the step in a `for` and jumps past it in a `while`,
    // which is a different loop and, here, one that never ends.
    let source = concat!(
        "int\tf(int n)\n",
        "{\n",
        "\tint\ti;\n",
        "\tint\ts;\n",
        "\n",
        "\ts = 0;\n",
        "\tfor (i = 0; i < n; i++)\n",
        "\t{\n",
        "\t\tif (i == 2)\n",
        "\t\t\tcontinue ;\n",
        "\t\ts += i;\n",
        "\t}\n",
        "\treturn (s);\n",
        "}\n",
    );
    assert!(apply(source, &[]).contains("for ("));

    // A `continue` in a nested loop belongs to that loop and never reaches
    // this step, so it is no reason to refuse.
    let inner = concat!(
        "int\tf(int n)\n",
        "{\n",
        "\tint\ti;\n",
        "\tint\tj;\n",
        "\n",
        "\tj = n;\n",
        "\tfor (i = 0; i < n; i++)\n",
        "\t{\n",
        "\t\twhile (j > 0)\n",
        "\t\t{\n",
        "\t\t\tj--;\n",
        "\t\t\tcontinue ;\n",
        "\t\t}\n",
        "\t}\n",
        "\treturn (j);\n",
        "}\n",
    );
    assert!(!apply(inner, &[]).contains("for ("));
}

#[test]
fn a_for_stays_when_moving_its_pieces_would_move_more_than_them() {
    // A declaration in the initializer is scoped to the loop, and lifting it
    // out widens that scope.
    let scoped = concat!(
        "int\tf(int n)\n",
        "{\n",
        "\tint\ts;\n",
        "\n",
        "\ts = 0;\n",
        "\tfor (int i = 0; i < n; i++)\n",
        "\t\ts += i;\n",
        "\treturn (s);\n",
        "}\n",
    );
    assert!(apply(scoped, &[]).contains("for ("));

    // One statement becomes two, and an unbraced body above would keep only
    // the initializer.
    let unbraced = concat!(
        "int\tf(int n)\n",
        "{\n",
        "\tint\ti;\n",
        "\tint\ts;\n",
        "\n",
        "\ts = 0;\n",
        "\tif (n > 0)\n",
        "\t\tfor (i = 0; i < n; i++)\n",
        "\t\t\ts += i;\n",
        "\treturn (s);\n",
        "}\n",
    );
    assert!(apply(unbraced, &[]).contains("for ("));

    // A comment after the last statement is a sibling of the loop, and would
    // be left below the closing brace describing the wrong thing.
    let annotated = concat!(
        "int\tf(int n)\n",
        "{\n",
        "\tint\ti;\n",
        "\tint\ts;\n",
        "\n",
        "\ts = 0;\n",
        "\tfor (i = 0; i < n; i++)\n",
        "\t\ts += i; /* soma */\n",
        "\treturn (s);\n",
        "}\n",
    );
    assert!(apply(annotated, &[]).contains("for ("));
}

#[test]
fn a_ternary_becomes_the_branch_it_stood_for() {
    // The Norm forbids `?:` outright, and both statement shapes it appears in
    // have an exact equivalent: the condition still runs first, and still
    // exactly one side runs after it.
    let source = concat!(
        "int\tlarger(int a, int b)\n",
        "{\n",
        "\tint\tr;\n",
        "\n",
        "\tr = a > b ? a : b;\n",
        "\treturn (r);\n",
        "}\n",
        "\n",
        "int\tsign(int n)\n",
        "{\n",
        "\treturn (n < 0 ? -1 : 1);\n",
        "}\n",
    );
    let fixed = apply(source, &[]);
    assert!(
        fixed.contains("\tif (a > b)\n\t\tr = a;\n\telse\n\t\tr = b;\n"),
        "{fixed}"
    );
    // A return needs no `else`: the first branch already left the function.
    assert!(
        fixed.contains("\tif (n < 0)\n\t\treturn (-1);\n\treturn (1);\n"),
        "{fixed}"
    );
    assert!(!fixed.contains('?'), "{fixed}");
}

#[test]
fn a_ternary_under_an_unbraced_body_stays() {
    // The new `else` would bind to the outer `if`.
    let source = concat!(
        "int\tf(int a, int b, int *out)\n",
        "{\n",
        "\tif (a > 0)\n",
        "\t\t*out = a > b ? a : b;\n",
        "\treturn (0);\n",
        "}\n",
    );
    assert!(apply(source, &[]).contains("a > b ? a : b"));
}

#[test]
fn a_ternary_whose_target_moves_stays() {
    // The target is written into both branches, so it may not do anything
    // that would then have to happen twice.
    let source = concat!(
        "int\tf(int *arr, int i, int a)\n",
        "{\n",
        "\tarr[i++] = a > 0 ? a : 0;\n",
        "\treturn (i);\n",
        "}\n",
    );
    assert!(apply(source, &[]).contains("a > 0 ? a : 0"));
}

#[test]
fn a_nested_ternary_stays() {
    // The inner one would land inside a branch, where the collector no longer
    // looks — the run would claim a ternary gone with one still in the file.
    let source = concat!(
        "int\tf(int a, int b)\n",
        "{\n",
        "\tint\tr;\n",
        "\n",
        "\tr = a > b ? a : (b > 0 ? b : 0);\n",
        "\treturn (r);\n",
        "}\n",
    );
    assert!(apply(source, &[]).contains("(b > 0 ? b : 0)"));
}

#[test]
fn a_ternary_stays_when_the_function_has_no_room_for_it() {
    // One line becomes four. Spending a budget the function does not have
    // would trade a ternary the student can rewrite in place for a structural
    // error that forces them to carve the function up.
    let body = (0..21).fold(String::new(), |mut text, index| {
        use std::fmt::Write as _;
        let _ = writeln!(text, "\tn += {index};");
        text
    });
    let source = format!(
        "int\tf(int a, int b)\n{{\n\tint\tn;\n\tint\tr;\n\n{body}\tr = a > b ? a : b;\n\treturn (r + n);\n}}\n"
    );
    assert!(apply(&source, &[]).contains("a > b ? a : b"));
}

#[test]
fn else_if_stays_on_one_line() {
    // `else if` is one construct written on one line. Treating the `if` as the
    // body of the `else` split it into an `else` holding a nested `if`: a line
    // longer and a level deeper, in a Norm that counts both.
    let source = concat!(
        "int\tclassify(int n)\n",
        "{\n",
        "\tif (n > 10)\n",
        "\t\treturn (1);\n",
        "\telse if (n > 5)\n",
        "\t\treturn (2);\n",
        "\telse\n",
        "\t\treturn (3);\n",
        "}\n",
    );
    let fixed = apply(source, &[]);
    assert!(fixed.contains("\telse if (n > 5)\n"), "{fixed}");
    assert!(!fixed.contains("\telse\n\t\tif "), "{fixed}");
}
