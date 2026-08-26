use super::*;

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
