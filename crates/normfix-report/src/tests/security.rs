use std::sync::Arc;
use std::time::Duration;

use camino::Utf8PathBuf;
use normfix_core::{TextRange, TextSize};
use normfix_i18n::Locale;

use super::common::file;
use crate::{RenderOptions, ReportIdentity, ReportMode, RunReport, render_human};

#[test]
fn human_output_escapes_untrusted_terminal_controls() {
    let mut unsafe_file = file();
    unsafe_file.path = Utf8PathBuf::from("src/\u{1b}[31m.c");
    unsafe_file.after[0].path = unsafe_file.path.clone();
    unsafe_file.after[0].message = "message\u{1b}[2J\nforged".to_owned();
    unsafe_file.after[0].notes = vec!["note\r\u{7}".to_owned()];
    unsafe_file.failure = Some("failure\u{1b}]0;owned\u{7}".to_owned());
    unsafe_file.original = Some(Arc::from("int\tmain(void)\n{\n\treturn (0);\n}\n"));
    unsafe_file.fixed = Some(Arc::from("int\tmain(void)\n{\n\treturn (0);\u{1b}[2J\n}\n"));
    let report = RunReport::new(
        "0.4.0",
        ReportMode::Diff,
        ReportIdentity::default(),
        vec!["bad\u{1b}[2J".to_owned()],
        vec![Utf8PathBuf::from("bad\u{1b}]0;x\u{7}.bin")],
        vec![unsafe_file],
        Duration::ZERO,
    );

    let rendered = render_human(
        &report,
        RenderOptions {
            color: false,
            verbose: true,
            show_diff: true,
            locale: Locale::English,
        },
    );

    assert!(!rendered.contains('\u{1b}'));
    assert!(!rendered.contains('\u{7}'));
    assert!(rendered.contains("\\u{1b}"));
    assert!(rendered.contains("message\\u{1b}[2J\\nforged"));
    assert!(rendered.contains("note\\r\\u{7}"));
    assert!(rendered.contains("int\tmain(void)"));
    assert!(!rendered.contains("int\\tmain(void)"));
}

#[test]
fn snippets_and_metadata_escape_bidi_and_terminal_controls() {
    let source: Arc<str> = Arc::from("int\u{1b}[2J\tmain\u{202e}(void)\n{\n\treturn (0);\n}\n");
    let start = source.find("main").expect("identifier");
    let mut unsafe_file = file();
    unsafe_file.fixed = Some(Arc::clone(&source));
    unsafe_file.after[0].message = "unsafe \u{202e}message".to_owned();
    unsafe_file.after[0].range = TextRange::new(
        TextSize::new(u32::try_from(start).expect("test offset")),
        TextSize::new(u32::try_from(start + "main".len()).expect("test offset")),
    )
    .expect("range");
    let report = RunReport::new(
        "1.0.0\u{1b}[2J\u{202e}",
        ReportMode::Check,
        ReportIdentity::default(),
        Vec::new(),
        Vec::new(),
        vec![unsafe_file],
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

        assert!(!rendered.contains('\u{1b}'));
        assert!(!rendered.contains('\u{202e}'));
        assert!(rendered.contains("1.0.0\\u{1b}[2J\\u{202e}"));
        assert!(rendered.contains("unsafe \\u{202e}message"));
        assert!(rendered.contains("int?[2J    main???(void)"));
        let lines = rendered.lines().collect::<Vec<_>>();
        let source_line = lines
            .iter()
            .position(|line| line.contains("int?[2J    main???(void)"))
            .expect("sanitized source line");
        assert_eq!(
            lines[source_line + 1].find('^'),
            lines[source_line].find("main"),
            "sanitizing source controls must not move the diagnostic caret"
        );
    }
}
