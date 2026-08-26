use std::sync::Arc;
use std::time::Duration;

use camino::Utf8PathBuf;
use normfix_core::{DiagnosticSource, TextRange, TextSize};
use normfix_i18n::Locale;

use super::common::file;
use crate::human::GROUPED_OCCURRENCE_LIMIT;
use crate::{RenderOptions, ReportIdentity, ReportMode, RunReport, render_human};

#[test]
fn human_diagnostic_contains_source_location_snippet_help_and_origin() {
    let report = RunReport::new(
        "0.4.0",
        ReportMode::Check,
        ReportIdentity::default(),
        Vec::new(),
        Vec::new(),
        vec![file()],
        Duration::from_millis(12),
    );
    let rendered = render_human(
        &report,
        RenderOptions {
            color: false,
            verbose: true,
            show_diff: false,
            locale: Locale::English,
        },
    );

    assert!(rendered.contains("warning[TOO_MANY_LINES]"));
    assert!(rendered.contains("--> src/main.c:1:6"));
    // The source line is shown with its tab expanded, and the carets sit
    // under the exact bytes the range covers.
    assert!(rendered.contains("1 | int    main(void)"));
    assert!(rendered.contains("  |         ^^^^"));
    assert!(rendered.contains("= help: Extract one coherent responsibility"));
    assert!(rendered.contains("= source: Norm v4.1 native rule"));
    assert!(!rendered.contains('\u{1b}'));
}

#[test]
fn a_caret_marks_the_bytes_the_range_covers_on_a_tab_indented_line() {
    // The shape of a real 42 statement: two tabs, then the call.
    let source: Arc<str> = Arc::from("void\tf(void)\n{\n\t\tsort_medium(ctx);\n}\n");
    let call = source.find("sort_medium").expect("the call");
    let mut file = file();
    file.original = Some(Arc::clone(&source));
    file.fixed = Some(Arc::clone(&source));
    file.after[0].range = TextRange::new(
        TextSize::new(u32::try_from(call).expect("fits")),
        TextSize::new(u32::try_from(call + "sort_medium".len()).expect("fits")),
    )
    .expect("range");

    let report = RunReport::new(
        "0.4.0",
        ReportMode::Check,
        ReportIdentity::default(),
        Vec::new(),
        Vec::new(),
        vec![file],
        Duration::ZERO,
    );
    let rendered = render_human(
        &report,
        RenderOptions {
            color: false,
            verbose: true,
            show_diff: false,
            locale: Locale::English,
        },
    );

    let lines = rendered.lines().collect::<Vec<_>>();
    let source_line = lines
        .iter()
        .position(|line| line.contains("sort_medium(ctx);"))
        .expect("the source line");
    let carets = lines[source_line + 1];
    let identifier = lines[source_line]
        .find("sort_medium")
        .expect("the identifier");
    assert_eq!(
        carets.find('^'),
        Some(identifier),
        "the caret must start under the identifier\n{}\n{carets}",
        lines[source_line]
    );
    assert_eq!(carets.matches('^').count(), "sort_medium".len());
}

#[test]
fn default_human_output_groups_the_same_rule_across_files() {
    let mut second = file();
    second.path = Utf8PathBuf::from("src/other.c");
    second.after[0].path = second.path.clone();
    second.after[0].message = "other() exceeds the 25-line limit".to_owned();
    let report = RunReport::new(
        "0.4.0",
        ReportMode::Check,
        ReportIdentity::default(),
        Vec::new(),
        Vec::new(),
        vec![file(), second],
        Duration::ZERO,
    );

    let rendered = render_human(
        &report,
        RenderOptions {
            color: false,
            verbose: false,
            show_diff: false,
            locale: Locale::English,
        },
    );

    assert!(rendered.contains("warning[TOO_MANY_LINES]: 2 occurrences in 2 files"));
    assert_eq!(rendered.matches("= help:").count(), 1);
    assert_eq!(rendered.matches("= source:").count(), 1);
    assert!(rendered.contains("src/main.c:1:6"));
    assert!(rendered.contains("src/other.c:1:6"));
    assert!(rendered.contains("= explain: normfix explain TOO_MANY_LINES"));
    // The default view shows the source, not only coordinates, and each
    // occurrence keeps its own message as the label on its own carets.
    assert_eq!(rendered.matches("1 | int    main(void)").count(), 2);
    assert!(rendered.contains("^^^^ Function exceeds the 25-line limit"));
    assert!(rendered.contains("^^^^ other() exceeds the 25-line limit"));
    // The shared note is stated once for the whole rule, not repeated per
    // occurrence, which is what made the old grouped output noisy.
    assert_eq!(
        rendered.matches("= note: main() has 30 body lines").count(),
        1
    );
}

#[test]
fn a_rule_with_many_occurrences_shows_a_bounded_number_of_snippets() {
    let files = (0..12)
        .map(|index| {
            let mut file = file();
            file.path = Utf8PathBuf::from(format!("src/f{index:02}.c"));
            file.after[0].path = file.path.clone();
            file
        })
        .collect::<Vec<_>>();
    let report = RunReport::new(
        "0.4.0",
        ReportMode::Check,
        ReportIdentity::default(),
        Vec::new(),
        Vec::new(),
        files,
        Duration::ZERO,
    );

    let grouped = render_human(
        &report,
        RenderOptions {
            color: false,
            verbose: false,
            show_diff: false,
            locale: Locale::English,
        },
    );

    assert!(grouped.contains("warning[TOO_MANY_LINES]: 12 occurrences in 12 files"));
    assert_eq!(
        grouped.matches("1 | int    main(void)").count(),
        GROUPED_OCCURRENCE_LIMIT,
        "the default view must stay bounded on a project with many hits"
    );
    assert!(grouped.contains("9 further occurrences not shown"));
    assert!(grouped.contains("--verbose"));

    // The flag it names has to actually show the rest.
    let expanded = render_human(
        &report,
        RenderOptions {
            color: false,
            verbose: true,
            show_diff: false,
            locale: Locale::English,
        },
    );
    assert_eq!(expanded.matches("1 | int    main(void)").count(), 12);
    assert!(!expanded.contains("further occurrences not shown"));
}

#[test]
fn a_compiler_diagnostic_with_no_local_position_draws_no_caret() {
    // The pipeline records "reported against this file, position unknown"
    // as an empty range at offset zero. Line 1 of a 42 file is the header
    // block, so a caret there would accuse the wrong code.
    let mut file = file();
    file.after[0].source = DiagnosticSource::Compiler;
    file.after[0].range = TextRange::empty(TextSize::new(0));
    file.after[0].notes = vec!["Compiler location: includes/a.h:82:30".to_owned()];
    let report = RunReport::new(
        "0.4.0",
        ReportMode::Check,
        ReportIdentity::default(),
        Vec::new(),
        Vec::new(),
        vec![file],
        Duration::ZERO,
    );

    for verbose in [false, true] {
        let rendered = render_human(
            &report,
            RenderOptions {
                color: false,
                verbose,
                show_diff: false,
                locale: Locale::English,
            },
        );
        assert!(
            rendered.contains("--> src/main.c"),
            "verbose={verbose}: the file must still be named"
        );
        assert!(
            !rendered.contains("1 | int"),
            "verbose={verbose}: no snippet may be drawn for an unknown position\n{rendered}"
        );
        assert!(rendered.contains("= note: Compiler location: includes/a.h:82:30"));
    }
}

#[test]
fn a_diagnostic_without_its_source_still_names_where_it_is() {
    let mut file = file();
    file.original = None;
    file.fixed = None;
    let report = RunReport::new(
        "0.4.0",
        ReportMode::Check,
        ReportIdentity::default(),
        Vec::new(),
        Vec::new(),
        vec![file],
        Duration::ZERO,
    );

    for verbose in [false, true] {
        let rendered = render_human(
            &report,
            RenderOptions {
                color: false,
                verbose,
                show_diff: false,
                locale: Locale::English,
            },
        );
        assert!(
            rendered.contains("src/main.c"),
            "verbose={verbose}: the path must survive a missing source buffer"
        );
        assert!(rendered.contains("TOO_MANY_LINES"));
    }
}
