use std::sync::Arc;
use std::time::Duration;

use camino::Utf8PathBuf;
use normfix_core::{Diagnostic, DiagnosticSource, Severity, TextRange, TextSize};
use normfix_i18n::Locale;

use super::common::{diagnostic, file};
use crate::{
    EvaluationGrade, EvaluationVerdict, FileReport, RenderOptions, ReportIdentity, ReportMode,
    RunReport, render_human,
};

#[test]
fn failed_or_empty_coverage_is_incomplete_instead_of_an_a_grade() {
    for discovery_errors in [
        Vec::new(),
        vec!["missing.c: could not read file".to_owned()],
    ] {
        let mut report = RunReport::new(
            "1.0.0",
            ReportMode::Check,
            ReportIdentity::default(),
            discovery_errors,
            Vec::new(),
            Vec::new(),
            Duration::ZERO,
        );
        report.enable_preflight_evaluation();

        let evaluation = report.evaluation.as_ref().expect("evaluation");
        assert_eq!(evaluation.verdict, EvaluationVerdict::Incomplete);
        assert_eq!(evaluation.grade, EvaluationGrade::Incomplete);
        assert_eq!(evaluation.score, 0);
        assert_eq!(report.exit_code(), 2);
        let rendered = render_human(
            &report,
            RenderOptions {
                color: false,
                verbose: false,
                show_diff: false,
                locale: Locale::English,
            },
        );
        assert!(rendered.contains("Pre-defense estimate: INCOMPLETE | grade — | 0/100"));
        assert!(!rendered.contains("grade A"));
    }
}

#[test]
fn preflight_evaluation_hard_fails_unexpected_files_without_claiming_conclusiveness() {
    let mut report = RunReport::new(
        "1.0.0",
        ReportMode::Check,
        ReportIdentity::default(),
        Vec::new(),
        vec![Utf8PathBuf::from("notes.txt")],
        Vec::new(),
        Duration::ZERO,
    );
    report.enable_preflight_evaluation();

    let evaluation = report.evaluation.as_ref().expect("evaluation");
    assert!(!evaluation.conclusive);
    assert_eq!(evaluation.verdict, EvaluationVerdict::HardFail);
    assert_eq!(evaluation.grade, EvaluationGrade::Fail);
    assert!(evaluation.score <= 59);
    assert_eq!(evaluation.hard_failures[0].path, "notes.txt");
    assert_eq!(report.exit_code(), 1);
}

#[test]
fn installed_norminette_finding_is_a_located_hard_fail() {
    let mut source_file = file();
    source_file.changed = false;
    source_file.fixes.clear();
    source_file.after[0].source = DiagnosticSource::NorminetteCompat("3.3.59".to_owned());
    let mut report = RunReport::new(
        "1.0.0",
        ReportMode::Check,
        ReportIdentity::default(),
        Vec::new(),
        Vec::new(),
        vec![source_file],
        Duration::ZERO,
    );
    report.enable_preflight_evaluation();

    let evaluation = report.evaluation.as_ref().expect("evaluation");
    assert_eq!(evaluation.verdict, EvaluationVerdict::HardFail);
    let finding = evaluation.hard_failures.first().expect("Norm finding");
    assert_eq!(finding.rule_id, "TOO_MANY_LINES");
    assert_eq!(finding.path, "src/main.c");
    assert_eq!((finding.line, finding.column), (Some(1), Some(6)));
}

#[test]
fn a_successful_fix_evaluates_the_bytes_written_to_disk() {
    let mut source_file = file();
    let mut official = diagnostic();
    official.source = DiagnosticSource::NorminetteCompat("3.3.59".to_owned());
    source_file.before = vec![official];
    source_file.after.clear();
    source_file.written = true;
    let mut report = RunReport::new(
        "1.0.0",
        ReportMode::Fix,
        ReportIdentity::default(),
        Vec::new(),
        Vec::new(),
        vec![source_file],
        Duration::ZERO,
    );

    report.enable_preflight_evaluation();

    let evaluation = report.evaluation.as_ref().expect("evaluation");
    assert_eq!(evaluation.verdict, EvaluationVerdict::AdvisoryPass);
    assert!(evaluation.hard_failures.is_empty());
}

#[test]
fn a_refused_fix_keeps_the_original_disk_failure_authoritative() {
    let mut source_file = file();
    let mut official = diagnostic();
    official.source = DiagnosticSource::NorminetteCompat("3.3.59".to_owned());
    source_file.before = vec![official];
    source_file.after.clear();
    source_file.written = false;
    let mut report = RunReport::new(
        "1.0.0",
        ReportMode::Fix,
        ReportIdentity::default(),
        Vec::new(),
        Vec::new(),
        vec![source_file],
        Duration::ZERO,
    );

    report.enable_preflight_evaluation();

    let evaluation = report.evaluation.as_ref().expect("evaluation");
    assert_eq!(evaluation.verdict, EvaluationVerdict::HardFail);
    assert_eq!(evaluation.hard_failures[0].rule_id, "TOO_MANY_LINES");
}

#[test]
fn untested_norminette_version_warning_is_not_an_official_rule_failure() {
    let mut source_file = file();
    source_file.changed = false;
    source_file.fixes.clear();
    source_file.after[0].rule_id = "NORMINETTE_VERSION_UNTESTED".to_owned();
    source_file.after[0].source = DiagnosticSource::NorminetteCompat("3.3.60".to_owned());
    source_file.after[0].severity = Severity::Info;
    let mut report = RunReport::new(
        "1.0.0",
        ReportMode::Check,
        ReportIdentity::default(),
        Vec::new(),
        Vec::new(),
        vec![source_file],
        Duration::ZERO,
    );
    report.enable_preflight_evaluation();

    let evaluation = report.evaluation.as_ref().expect("evaluation");
    assert_eq!(evaluation.verdict, EvaluationVerdict::AdvisoryPass);
    assert!(evaluation.hard_failures.is_empty());
    assert_eq!(report.exit_code(), 0);
}

#[test]
fn makefile_hard_fail_preserves_the_exact_source_location() {
    let source: Arc<str> = Arc::from("NAME = app\nSRCS = missing.c\n");
    let start = source.find("missing.c").expect("token");
    let makefile = FileReport {
        budget: Vec::new(),
        path: Utf8PathBuf::from("Makefile"),
        changed: false,
        written: false,
        backup: None,
        failure: None,
        fixes: Vec::new(),
        before: Vec::new(),
        after: vec![Diagnostic {
            rule_id: "MAKEFILE_SOURCE_NOT_FOUND".to_owned(),
            path: Utf8PathBuf::from("Makefile"),
            range: TextRange::new(
                TextSize::new(u32::try_from(start).expect("test offset")),
                TextSize::new(u32::try_from(start + "missing.c".len()).expect("test offset")),
            )
            .expect("range"),
            severity: Severity::Warning,
            message: "The literal source is missing.".to_owned(),
            source: DiagnosticSource::Makefile,
            notes: Vec::new(),
            help: None,
            localized: None,
        }],
        original: Some(Arc::clone(&source)),
        fixed: Some(source),
    };
    let mut report = RunReport::new(
        "1.0.0",
        ReportMode::Check,
        ReportIdentity::default(),
        Vec::new(),
        Vec::new(),
        vec![makefile],
        Duration::ZERO,
    );
    report.enable_preflight_evaluation();

    let finding = &report
        .evaluation
        .as_ref()
        .expect("evaluation")
        .hard_failures[0];
    assert_eq!((finding.line, finding.column), (Some(2), Some(8)));
    let rendered = render_human(
        &report,
        RenderOptions {
            color: false,
            verbose: false,
            show_diff: false,
            locale: Locale::English,
        },
    );
    assert!(rendered.contains("Pre-defense estimate: HARD FAIL"));
    assert!(rendered.contains("Makefile:2:8 [MAKEFILE_SOURCE_NOT_FOUND]"));
    assert!(rendered.contains("never replaces the official evaluation"));
}

#[test]
fn makefile_operational_failure_is_a_hard_fail_at_the_file_boundary() {
    let mut makefile = file();
    makefile.path = Utf8PathBuf::from("libft/Makefile");
    makefile.failure = Some("could not read Makefile".to_owned());
    makefile.after.clear();
    makefile.original = None;
    makefile.fixed = None;
    let mut report = RunReport::new(
        "1.0.0",
        ReportMode::Check,
        ReportIdentity::default(),
        Vec::new(),
        Vec::new(),
        vec![makefile],
        Duration::ZERO,
    );
    report.enable_preflight_evaluation();

    let evaluation = report.evaluation.as_ref().expect("evaluation");
    assert_eq!(evaluation.verdict, EvaluationVerdict::HardFail);
    assert_eq!(evaluation.hard_failures.len(), 1);
    assert_eq!(
        evaluation.hard_failures[0].rule_id,
        "MAKEFILE_OPERATION_FAILED"
    );
    assert_eq!(evaluation.hard_failures[0].path, "libft/Makefile");
    assert_eq!(evaluation.hard_failures[0].line, None);
    assert_eq!(evaluation.hard_failures[0].column, None);
}
