//! Stable machine reports and source-aware terminal diagnostics.
//!
//! Analysis crates emit backend-neutral data. This crate is the only layer
//! responsible for ANSI styling, snippets, tables, diffs and the versioned
//! JSON contract.

#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::ops::Range;
use std::sync::Arc;
use std::time::Duration;

use annotate_snippets::{Annotation, AnnotationKind, Group, Level, Origin, Renderer, Snippet};
use camino::{Utf8Path, Utf8PathBuf};
use normfix_core::{Diagnostic, DiagnosticSource, FixRecord, LineIndex, Severity};
use normfix_i18n::{Locale, Messages, fill};
use serde::{Deserialize, Serialize};
use similar::TextDiff;

/// Version of the stable JSON report schema.
pub const REPORT_SCHEMA_VERSION: u32 = 2;

/// Fixed width every snippet is rendered against.
///
/// Norm-conforming lines fit in 80 columns; the rest is gutter and margin for
/// the few lines that do not yet conform.
const RENDER_WIDTH: usize = 120;

/// Execution mode represented in a report.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportMode {
    /// Changes were committed.
    Fix,
    /// Changes were planned but not written.
    Check,
    /// Changes were planned and unified diffs requested.
    Diff,
}

/// Header identity metadata safe for human and machine output.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct ReportIdentity {
    /// Resolved 42 login.
    pub login: String,
    /// Resolved 42 student email.
    pub email: String,
    /// Human-readable resolution source.
    pub source: String,
    /// Whether any identity field came from an inferred source.
    pub inferred: bool,
    /// Whether both login and email are usable.
    pub available: bool,
}

/// Complete outcome for one processable file.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FileReport {
    /// Stable project-relative path where possible.
    pub path: Utf8PathBuf,
    /// Whether original and proposed bytes differ.
    pub changed: bool,
    /// Whether the proposed bytes were committed.
    pub written: bool,
    /// External backup path.
    pub backup: Option<Utf8PathBuf>,
    /// Operational failure, distinct from source diagnostics.
    pub failure: Option<String>,
    /// Accepted fixes in stable order.
    pub fixes: Vec<FixRecord>,
    /// Diagnostics before transformation.
    pub before: Vec<Diagnostic>,
    /// Diagnostics after transformation.
    pub after: Vec<Diagnostic>,
    /// Original UTF-8 source, omitted from JSON.
    #[serde(skip)]
    pub original: Option<Arc<str>>,
    /// Proposed UTF-8 source, omitted from JSON.
    #[serde(skip)]
    pub fixed: Option<Arc<str>>,
}

impl FileReport {
    /// Returns the status used by human and machine summaries.
    #[must_use]
    pub fn status(&self) -> FileStatus {
        if self.failure.is_some() {
            FileStatus::Failed
        } else if has_blocking_diagnostic(&self.after) {
            FileStatus::Review
        } else if self.changed && self.written {
            FileStatus::Fixed
        } else if self.changed {
            FileStatus::WouldFix
        } else if !self.after.is_empty() {
            FileStatus::Advisory
        } else {
            FileStatus::Clean
        }
    }
}

/// Stable per-file status.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FileStatus {
    /// No change or remaining issue.
    Clean,
    /// Informational diagnostics exist, but no manual fix is required.
    Advisory,
    /// Proposed changes were committed.
    Fixed,
    /// Proposed changes were not written.
    WouldFix,
    /// Source diagnostics remain.
    Review,
    /// An operational error prevented completion.
    Failed,
}

/// Aggregate numeric report.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct ReportSummary {
    /// Processable files.
    pub files: usize,
    /// Files with a proposed byte change.
    pub changed: usize,
    /// Files actually committed.
    pub written: usize,
    /// Number of represented concrete fixes.
    pub fixes: u64,
    /// Diagnostics remaining.
    pub remaining: usize,
    /// Informational diagnostics that do not affect the exit status.
    pub advisories: usize,
    /// Operational failures.
    pub failed: usize,
    /// Unsupported files observed during directory scans.
    pub unexpected_files: usize,
    /// Unexpected files selected for recoverable quarantine.
    pub quarantine_candidates: usize,
    /// Unexpected files moved to recovery storage.
    pub quarantined: usize,
}

/// Non-conclusive pre-defense grade shown only by the evaluation workflow.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EvaluationGrade {
    /// Coverage failed before a meaningful grade could be computed.
    Incomplete,
    /// At least 90 heuristic points and no hard-fail rule.
    A,
    /// At least 80 heuristic points and no hard-fail rule.
    B,
    /// At least 70 heuristic points and no hard-fail rule.
    C,
    /// Fewer than 70 heuristic points without a hard-fail rule.
    D,
    /// One or more objective hard-fail rules matched.
    Fail,
}

/// Whether objective preflight rules rejected this snapshot.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EvaluationVerdict {
    /// Required project input or analysis failed, so no pass/fail claim is made.
    Incomplete,
    /// No configured hard-fail rule matched; manual evaluation is still required.
    AdvisoryPass,
    /// At least one configured hard-fail rule matched.
    HardFail,
}

/// One exactly located reason for an evaluation hard fail.
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct EvaluationFinding {
    /// Stable rule identifier.
    pub rule_id: String,
    /// Project-relative path.
    pub path: Utf8PathBuf,
    /// One-based physical line when source bytes are available.
    pub line: Option<u32>,
    /// One-based display column when source bytes are available.
    pub column: Option<u32>,
    /// Concise English explanation.
    pub message: String,
}

/// Heuristic, explicitly non-conclusive pre-defense assessment.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EvaluationReport {
    /// Always false: only the official evaluation can be conclusive.
    pub conclusive: bool,
    /// Transparent 0–100 heuristic, capped below 60 on hard fail.
    pub score: u8,
    /// Letter band or hard-fail grade.
    pub grade: EvaluationGrade,
    /// Objective hard-fail outcome.
    pub verdict: EvaluationVerdict,
    /// Exact evidence for hard-fail rules.
    pub hard_failures: Vec<EvaluationFinding>,
    /// Stable caveats for machine and human consumers.
    pub notes: Vec<String>,
}

/// Versioned output of one complete run.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RunReport {
    /// Machine schema version.
    pub schema_version: u32,
    /// Tool version.
    pub tool_version: String,
    /// Run mode.
    pub mode: ReportMode,
    /// Header identity metadata.
    pub identity: ReportIdentity,
    /// Discovery failures.
    pub discovery_errors: Vec<String>,
    /// Unsupported files that were only reported.
    pub unexpected_files: Vec<Utf8PathBuf>,
    /// Unexpected files selected for recoverable quarantine.
    pub quarantine_candidates: Vec<Utf8PathBuf>,
    /// Unexpected files moved to external recovery storage.
    pub quarantined_files: Vec<Utf8PathBuf>,
    /// Operational quarantine failures.
    pub quarantine_errors: Vec<String>,
    /// Per-file reports.
    pub files: Vec<FileReport>,
    /// Aggregate counts.
    pub summary: ReportSummary,
    /// Non-conclusive assessment, present only for preflight/evaluation runs.
    #[serde(default)]
    pub evaluation: Option<EvaluationReport>,
    /// Wall-clock duration; the only intentionally nondeterministic field.
    pub duration_seconds: f64,
}

impl RunReport {
    /// Constructs, sorts and summarizes a report.
    #[must_use]
    pub fn new(
        tool_version: impl Into<String>,
        mode: ReportMode,
        identity: ReportIdentity,
        mut discovery_errors: Vec<String>,
        mut unexpected_files: Vec<Utf8PathBuf>,
        mut files: Vec<FileReport>,
        duration: Duration,
    ) -> Self {
        discovery_errors.sort();
        unexpected_files.sort();
        files.sort_by(|left, right| left.path.cmp(&right.path));
        for file in &mut files {
            file.fixes.sort();
            file.before.sort();
            file.after.sort();
        }
        let summary = ReportSummary {
            files: files.len(),
            changed: files.iter().filter(|file| file.changed).count(),
            written: files.iter().filter(|file| file.written).count(),
            fixes: files
                .iter()
                .flat_map(|file| &file.fixes)
                .map(|fix| u64::from(fix.count))
                .sum(),
            remaining: files
                .iter()
                .flat_map(|file| &file.after)
                .filter(|diagnostic| diagnostic.severity != Severity::Info)
                .count(),
            advisories: files
                .iter()
                .flat_map(|file| &file.after)
                .filter(|diagnostic| diagnostic.severity == Severity::Info)
                .count(),
            failed: files.iter().filter(|file| file.failure.is_some()).count(),
            unexpected_files: unexpected_files.len(),
            quarantine_candidates: 0,
            quarantined: 0,
        };
        Self {
            schema_version: REPORT_SCHEMA_VERSION,
            tool_version: tool_version.into(),
            mode,
            identity,
            discovery_errors,
            unexpected_files,
            quarantine_candidates: Vec::new(),
            quarantined_files: Vec::new(),
            quarantine_errors: Vec::new(),
            files,
            summary,
            evaluation: None,
            duration_seconds: duration.as_secs_f64(),
        }
    }

    /// Returns the documented process exit code.
    #[must_use]
    pub fn exit_code(&self) -> u8 {
        if !self.discovery_errors.is_empty()
            || !self.quarantine_errors.is_empty()
            || self.summary.failed > 0
            || self
                .evaluation
                .as_ref()
                .is_some_and(|evaluation| evaluation.verdict == EvaluationVerdict::Incomplete)
        {
            return 2;
        }
        if self.summary.remaining > 0
            || self
                .evaluation
                .as_ref()
                .is_some_and(|evaluation| evaluation.verdict == EvaluationVerdict::HardFail)
            || (self.mode != ReportMode::Fix
                && (self.summary.changed > 0 || self.summary.quarantine_candidates > 0))
        {
            return 1;
        }
        0
    }

    /// Computes the deterministic, non-conclusive pre-defense assessment.
    pub fn enable_preflight_evaluation(&mut self) {
        self.evaluation = Some(build_preflight_evaluation(self));
    }

    /// Attaches one deterministic recoverable-quarantine outcome.
    pub fn set_quarantine_outcome(
        &mut self,
        mut candidates: Vec<Utf8PathBuf>,
        mut quarantined: Vec<Utf8PathBuf>,
        mut errors: Vec<String>,
    ) {
        candidates.sort();
        candidates.dedup();
        quarantined.sort();
        quarantined.dedup();
        errors.sort();
        errors.dedup();
        self.summary.quarantine_candidates = candidates.len();
        self.summary.quarantined = quarantined.len();
        self.quarantine_candidates = candidates;
        self.quarantined_files = quarantined;
        self.quarantine_errors = errors;
    }

    /// Serializes stable pretty JSON.
    ///
    /// # Errors
    ///
    /// Returns a serialization error only if the report schema becomes
    /// internally inconsistent.
    pub fn to_pretty_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self).map(|mut json| {
            json.push('\n');
            json
        })
    }
}

/// Human renderer configuration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RenderOptions {
    /// Emit ANSI colors.
    pub color: bool,
    /// Show individual fixes.
    pub verbose: bool,
    /// Include unified diffs.
    pub show_diff: bool,
    /// Language for the report's own prose.
    ///
    /// Rule identifiers, paths, and backend messages are unaffected: only text
    /// this crate authors is translated.
    pub locale: Locale,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            color: true,
            verbose: false,
            show_diff: false,
            locale: Locale::English,
        }
    }
}

/// Renders one complete human report.
#[must_use]
pub fn render_human(report: &RunReport, options: RenderOptions) -> String {
    let paint = Paint::new(options.color);
    let messages = normfix_i18n::messages(options.locale);
    let mut output = String::new();
    let _ = writeln!(
        output,
        "{}normfix{} {}",
        paint.bold_cyan, paint.reset, report.tool_version
    );
    let _ = writeln!(output, "{}", messages.report_tagline);
    if report.files.iter().any(|file| {
        matches!(file.path.extension(), Some("c" | "h"))
            || file
                .path
                .file_name()
                .is_some_and(|name| name.eq_ignore_ascii_case("makefile"))
    }) {
        render_identity(&mut output, &paint, &report.identity);
    }
    let _ = writeln!(
        output,
        "\n{}{}{} {}",
        paint.bold_blue,
        messages.report_project_reminder_label,
        paint.reset,
        messages.report_project_reminder
    );
    // Backend rule messages are not translated yet. Saying so is better than
    // letting a localized frame imply the whole report was translated.
    if options.locale != Locale::English {
        let _ = writeln!(output, "{}", messages.report_translation_scope);
    }
    render_discovery(&mut output, &paint, report, messages);
    render_quarantine(&mut output, &paint, report, messages);
    render_file_table(
        &mut output,
        &paint,
        &report.files,
        options.verbose,
        messages,
    );
    if options.verbose {
        render_fixes(&mut output, &paint, &report.files);
    }
    render_diagnostics(
        &mut output,
        &paint,
        &report.files,
        options.verbose,
        messages,
    );
    render_failures(&mut output, &paint, &report.files);
    if options.show_diff {
        render_diffs(&mut output, &report.files);
    }
    render_evaluation(&mut output, &paint, report.evaluation.as_ref(), messages);
    render_summary(&mut output, &paint, report, messages);
    output
}

fn render_evaluation(
    output: &mut String,
    paint: &Paint,
    evaluation: Option<&EvaluationReport>,
    messages: &Messages,
) {
    let Some(evaluation) = evaluation else {
        return;
    };
    let verdict = match evaluation.verdict {
        EvaluationVerdict::Incomplete => "INCOMPLETE",
        EvaluationVerdict::AdvisoryPass => "ADVISORY PASS",
        EvaluationVerdict::HardFail => "HARD FAIL",
    };
    let grade = match evaluation.grade {
        EvaluationGrade::Incomplete => "—",
        EvaluationGrade::A => "A",
        EvaluationGrade::B => "B",
        EvaluationGrade::C => "C",
        EvaluationGrade::D => "D",
        EvaluationGrade::Fail => "FAIL",
    };
    let style = match evaluation.verdict {
        EvaluationVerdict::Incomplete => paint.bold_yellow,
        EvaluationVerdict::HardFail => paint.bold_red,
        EvaluationVerdict::AdvisoryPass => paint.bold_blue,
    };
    let _ = writeln!(
        output,
        "\n{}{}{} {}",
        style,
        messages.report_estimate_label,
        paint.reset,
        fill(
            messages.report_estimate_value,
            &[
                ("verdict", verdict),
                ("grade", grade),
                ("score", &evaluation.score.to_string()),
            ]
        )
    );
    let _ = writeln!(output, "{}", messages.report_estimate_caveat);
    if !evaluation.hard_failures.is_empty() {
        let _ = writeln!(output, "{}", messages.report_hard_fail_heading);
        for finding in &evaluation.hard_failures {
            let location = match (finding.line, finding.column) {
                (Some(line), Some(column)) => {
                    format!("{}:{line}:{column}", safe_path(&finding.path))
                }
                _ => safe_path(&finding.path),
            };
            let _ = writeln!(
                output,
                "  {} [{}] {}",
                location,
                terminal_safe_inline(&finding.rule_id),
                terminal_safe_inline(&finding.message)
            );
        }
    }
}

// Keeping current-snapshot evidence, shadow-only additions, bounded scoring,
// and the non-conclusive verdict together makes the grading boundary auditable.
#[allow(clippy::too_many_lines)]
fn build_preflight_evaluation(report: &RunReport) -> EvaluationReport {
    let mut hard_failures = report
        .unexpected_files
        .iter()
        .map(|path| EvaluationFinding {
            rule_id: "UNEXPECTED_PROJECT_FILE".to_owned(),
            path: path.clone(),
            line: None,
            column: None,
            message: "Unexpected project file is present in the evaluated scope.".to_owned(),
        })
        .collect::<Vec<_>>();
    let mut norm_count = 0_usize;
    let mut makefile_count = 0_usize;
    let mut other_blocking = 0_usize;
    for file in &report.files {
        if file.failure.is_some() && is_makefile_path(&file.path) {
            makefile_count += 1;
            hard_failures.push(EvaluationFinding {
                rule_id: "MAKEFILE_OPERATION_FAILED".to_owned(),
                path: file.path.clone(),
                line: None,
                column: None,
                message: "The Makefile could not be evaluated completely.".to_owned(),
            });
        }
        let mut original_counts = BTreeMap::<(EvaluationFailureKind, String), usize>::new();
        for diagnostic in &file.before {
            let Some(kind) = evaluation_failure_kind(diagnostic) else {
                continue;
            };
            *original_counts
                .entry((kind, diagnostic.rule_id.clone()))
                .or_default() += 1;
            increment_evaluation_count(kind, &mut norm_count, &mut makefile_count);
            push_evaluation_finding(&mut hard_failures, diagnostic, file.original.as_deref());
        }
        let mut shadow_counts = BTreeMap::<(EvaluationFailureKind, String), usize>::new();
        for diagnostic in &file.after {
            let Some(kind) = evaluation_failure_kind(diagnostic) else {
                if diagnostic.severity != Severity::Info {
                    other_blocking += 1;
                }
                continue;
            };
            let key = (kind, diagnostic.rule_id.clone());
            let occurrence = shadow_counts.entry(key.clone()).or_default();
            let represented_by_original =
                *occurrence < original_counts.get(&key).copied().unwrap_or_default();
            *occurrence += 1;
            if represented_by_original {
                continue;
            }
            increment_evaluation_count(kind, &mut norm_count, &mut makefile_count);
            push_evaluation_finding(&mut hard_failures, diagnostic, file.fixed.as_deref());
        }
    }
    hard_failures.sort();
    hard_failures.dedup();

    let operational =
        report.discovery_errors.len() + report.quarantine_errors.len() + report.summary.failed;
    let mut deduction = norm_count.saturating_mul(8).min(45)
        + makefile_count.saturating_mul(8).min(30)
        + report.unexpected_files.len().saturating_mul(5).min(25)
        + other_blocking.saturating_mul(2).min(20)
        + operational.saturating_mul(10).min(30);
    if report.mode != ReportMode::Fix {
        deduction = deduction.saturating_add(report.summary.changed.min(10));
    }
    let mut score = u8::try_from(100_usize.saturating_sub(deduction).min(100))
        .expect("the preflight score is capped at 100");
    let coverage_incomplete = report.files.is_empty()
        || !report.discovery_errors.is_empty()
        || !report.quarantine_errors.is_empty()
        || report.summary.failed > 0;
    let verdict = if !hard_failures.is_empty() {
        score = score.min(59);
        EvaluationVerdict::HardFail
    } else if coverage_incomplete {
        score = 0;
        EvaluationVerdict::Incomplete
    } else {
        EvaluationVerdict::AdvisoryPass
    };
    let grade = match verdict {
        EvaluationVerdict::Incomplete => EvaluationGrade::Incomplete,
        EvaluationVerdict::HardFail => EvaluationGrade::Fail,
        EvaluationVerdict::AdvisoryPass => match score {
            90..=100 => EvaluationGrade::A,
            80..=89 => EvaluationGrade::B,
            70..=79 => EvaluationGrade::C,
            _ => EvaluationGrade::D,
        },
    };
    EvaluationReport {
        conclusive: false,
        score,
        grade,
        verdict,
        hard_failures,
        notes: vec![
            "Incomplete means discovery or file analysis failed, or no processable file was covered; no grade can be inferred from that run."
                .to_owned(),
            "Hard fail: an unexpected project file, a finding corroborated by the installed official Norminette, or a Makefile finding was present."
                .to_owned(),
            "The score deducts bounded category weights for those findings, other warnings, operational failures, and pending edits; it is not a 42 grade."
                .to_owned(),
            "Runtime behavior, subject-specific tests, peer judgment, leaks, and defense questions remain outside this estimate."
                .to_owned(),
        ],
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum EvaluationFailureKind {
    OfficialNorm,
    Makefile,
}

fn evaluation_failure_kind(diagnostic: &Diagnostic) -> Option<EvaluationFailureKind> {
    if diagnostic.severity == Severity::Info {
        return None;
    }
    if diagnostic.source == DiagnosticSource::Makefile {
        return Some(EvaluationFailureKind::Makefile);
    }
    (matches!(diagnostic.source, DiagnosticSource::NorminetteCompat(_))
        && diagnostic.rule_id != "NORMINETTE_VERSION_UNTESTED")
        .then_some(EvaluationFailureKind::OfficialNorm)
}

const fn increment_evaluation_count(
    kind: EvaluationFailureKind,
    norm_count: &mut usize,
    makefile_count: &mut usize,
) {
    match kind {
        EvaluationFailureKind::OfficialNorm => *norm_count += 1,
        EvaluationFailureKind::Makefile => *makefile_count += 1,
    }
}

fn push_evaluation_finding(
    hard_failures: &mut Vec<EvaluationFinding>,
    diagnostic: &Diagnostic,
    source: Option<&str>,
) {
    let (line, column) = diagnostic_location(source, diagnostic);
    hard_failures.push(EvaluationFinding {
        rule_id: diagnostic.rule_id.clone(),
        path: diagnostic.path.clone(),
        line,
        column,
        message: diagnostic.message.clone(),
    });
}

fn diagnostic_location(
    source: Option<&str>,
    diagnostic: &Diagnostic,
) -> (Option<u32>, Option<u32>) {
    let Some(source) = source else {
        return (None, None);
    };
    let Ok(index) = LineIndex::new(Arc::from(source)) else {
        return (None, None);
    };
    index
        .line_column(diagnostic.range.start())
        .map_or((None, None), |location| {
            (Some(location.line), Some(location.visual_column))
        })
}

fn is_makefile_path(path: &Utf8Path) -> bool {
    path.file_name()
        .is_some_and(|name| name.eq_ignore_ascii_case("makefile"))
}

fn render_identity(output: &mut String, paint: &Paint, identity: &ReportIdentity) {
    if !identity.available {
        let _ = writeln!(
            output,
            "\n{}Official header not added:{} no verified 42 student email is available.",
            paint.bold_red, paint.reset
        );
        if !identity.source.is_empty() {
            let _ = writeln!(output, "  {}", terminal_safe_inline(&identity.source));
        }
    } else if identity.inferred {
        let _ = writeln!(
            output,
            "\n{}Header identity inferred:{} {} <{}> ({})",
            paint.yellow,
            paint.reset,
            terminal_safe_inline(&identity.login),
            terminal_safe_inline(&identity.email),
            terminal_safe_inline(&identity.source)
        );
    }
}

fn render_discovery(output: &mut String, paint: &Paint, report: &RunReport, messages: &Messages) {
    for error in &report.discovery_errors {
        let _ = writeln!(
            output,
            "\n{}Input error:{} {}",
            paint.bold_red,
            paint.reset,
            terminal_safe_inline(error)
        );
    }
    if report.unexpected_files.is_empty() {
        return;
    }
    let _ = writeln!(
        output,
        "\n{}Unexpected project files (not modified){}",
        paint.bold_yellow, paint.reset
    );
    for path in &report.unexpected_files {
        let _ = writeln!(output, "  {}", safe_path(path));
    }
    let _ = writeln!(output, "{}", messages.report_expected_files);
}

fn render_quarantine(output: &mut String, paint: &Paint, report: &RunReport, messages: &Messages) {
    if !report.quarantined_files.is_empty() {
        let _ = writeln!(
            output,
            "\n{}Unexpected files moved to recoverable quarantine{}",
            paint.bold_green, paint.reset
        );
        for path in &report.quarantined_files {
            let _ = writeln!(output, "  {}", safe_path(path));
        }
    } else if !report.quarantine_candidates.is_empty() {
        let _ = writeln!(
            output,
            "\n{}Unexpected files selected for quarantine{}",
            paint.bold_blue, paint.reset
        );
        for path in &report.quarantine_candidates {
            let _ = writeln!(output, "  {}", safe_path(path));
        }
        let _ = writeln!(output, "  {}", messages.report_preview_kept_files);
    }
    for error in &report.quarantine_errors {
        let _ = writeln!(
            output,
            "\n{}Quarantine failed:{} {}",
            paint.bold_red,
            paint.reset,
            terminal_safe_inline(error)
        );
    }
}

fn render_file_table(
    output: &mut String,
    paint: &Paint,
    files: &[FileReport],
    verbose: bool,
    messages: &Messages,
) {
    let _ = writeln!(output, "\n{}", messages.report_files_heading);
    output.push_str("STATUS      FIXES  REMAINING  INFO  FILE\n");
    let clean_count = files
        .iter()
        .filter(|file| file.status() == FileStatus::Clean)
        .count();
    if !verbose && clean_count > 0 {
        let noun = if clean_count == 1 { "file" } else { "files" };
        let _ = writeln!(
            output,
            "{}CLEAN{}          0          0     0  {clean_count} {noun}",
            paint.green, paint.reset
        );
    }
    for file in files {
        if !verbose && file.status() == FileStatus::Clean {
            continue;
        }
        let (label, style) = match file.status() {
            FileStatus::Clean => ("CLEAN", paint.green),
            FileStatus::Advisory => ("INFO", paint.bold_blue),
            FileStatus::Fixed => ("FIXED", paint.bold_green),
            FileStatus::WouldFix => ("WOULD FIX", paint.bold_blue),
            FileStatus::Review => ("REVIEW", paint.bold_yellow),
            FileStatus::Failed => ("FAILED", paint.bold_red),
        };
        let fix_count = file
            .fixes
            .iter()
            .map(|fix| u64::from(fix.count))
            .sum::<u64>();
        let remaining = file
            .after
            .iter()
            .filter(|diagnostic| diagnostic.severity != Severity::Info)
            .count();
        let advisories = file
            .after
            .iter()
            .filter(|diagnostic| diagnostic.severity == Severity::Info)
            .count();
        let _ = writeln!(
            output,
            "{style}{label:<10}{}{fix_count:>5}  {remaining:>9}  {advisories:>4}  {}",
            paint.reset,
            safe_path(&file.path)
        );
    }
}

fn render_fixes(output: &mut String, paint: &Paint, files: &[FileReport]) {
    for file in files {
        if file.fixes.is_empty() {
            continue;
        }
        let label = if file.written {
            "Applied fixes"
        } else {
            "Proposed fixes"
        };
        let _ = writeln!(
            output,
            "\n{}{label}: {}{}",
            paint.bold_cyan,
            safe_path(&file.path),
            paint.reset
        );
        for fix in &file.fixes {
            let location = fix
                .line
                .map_or_else(String::new, |line| format!(" at line {line}"));
            let _ = writeln!(
                output,
                "  {} ×{}{}: {}",
                terminal_safe_inline(&fix.rule_id),
                fix.count,
                location,
                terminal_safe_inline(&fix.description)
            );
        }
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct DiagnosticGroupKey {
    severity: Severity,
    rule_id: String,
    source: DiagnosticSource,
    help: Option<String>,
}

/// Occurrences shown per rule before the rest are summarized.
///
/// A project can carry thousands of one diagnostic. Printing every snippet
/// would make the report unreadable in exactly the way snippets are meant to
/// prevent, so the default shows enough to recognize the pattern and names the
/// flag that shows the rest.
const GROUPED_OCCURRENCE_LIMIT: usize = 3;

fn render_diagnostics(
    output: &mut String,
    paint: &Paint,
    files: &[FileReport],
    verbose: bool,
    messages: &Messages,
) {
    if verbose {
        render_diagnostics_expanded(output, paint, files, messages);
        return;
    }
    let diagnostic_count = files.iter().map(|file| file.after.len()).sum::<usize>();
    if diagnostic_count == 0 {
        return;
    }

    let mut groups = BTreeMap::<DiagnosticGroupKey, Vec<&Diagnostic>>::new();
    for diagnostic in files.iter().flat_map(|file| &file.after) {
        groups
            .entry(DiagnosticGroupKey {
                severity: diagnostic.severity,
                rule_id: diagnostic.rule_id.clone(),
                source: diagnostic.source.clone(),
                help: reader_text(diagnostic).2.cloned(),
            })
            .or_default()
            .push(diagnostic);
    }

    let _ = writeln!(output, "\n{}", messages.report_grouped_heading);
    let sources = source_map(files);
    let renderer = snippet_renderer(paint.color);
    for (group, mut diagnostics) in groups {
        diagnostics.sort_by(|left, right| {
            left.path
                .cmp(&right.path)
                .then_with(|| left.range.cmp(&right.range))
                .then_with(|| left.message.cmp(&right.message))
        });
        output.push('\n');
        output.push_str(&render_rule_group(
            &renderer,
            &group,
            &diagnostics,
            &sources,
        ));
    }
}

/// One rule, its occurrences marked in the source they appear in.
///
/// Occurrences in the same file share a snippet so the reader sees the pattern
/// in its own context, rather than a list of coordinates to go look up.
fn render_rule_group(
    renderer: &Renderer,
    group: &DiagnosticGroupKey,
    diagnostics: &[&Diagnostic],
    sources: &BTreeMap<&Utf8Path, &str>,
) -> String {
    let paths = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.path.as_path())
        .collect::<BTreeSet<_>>();

    // Every borrowed string the report shows has to outlive the groups that
    // reference it, so the escaping happens here and the builder below only
    // borrows.
    let shown = diagnostics.len().min(GROUPED_OCCURRENCE_LIMIT);
    let mut plans: Vec<SnippetPlan<'_>> = Vec::new();
    let mut missing_sources: Vec<String> = Vec::new();
    let mut notes: Vec<String> = Vec::new();
    for diagnostic in diagnostics.iter().take(shown) {
        let (reader_message, reader_notes, _) = reader_text(diagnostic);
        for note in reader_notes {
            let note = terminal_safe_inline(note);
            if !notes.contains(&note) {
                notes.push(note);
            }
        }
        let label = if diagnostics.len() == 1 {
            String::new()
        } else {
            terminal_safe_inline(reader_message)
        };
        let source = sources
            .get(diagnostic.path.as_path())
            .filter(|_| has_locatable_range(diagnostic));
        let Some(source) = source else {
            let path = safe_path(&diagnostic.path);
            if !missing_sources.contains(&path) {
                missing_sources.push(path);
            }
            continue;
        };
        let span = snippet_span(source, diagnostic);
        match plans
            .iter_mut()
            .find(|plan| plan.path == diagnostic.path.as_path())
        {
            Some(plan) => plan.spans.push((span, label)),
            None => plans.push(SnippetPlan {
                path: diagnostic.path.as_path(),
                safe_path: safe_path(&diagnostic.path),
                source,
                spans: vec![(span, label)],
            }),
        }
    }

    let occurrence_word = if diagnostics.len() == 1 {
        "occurrence"
    } else {
        "occurrences"
    };
    let file_word = if paths.len() == 1 { "file" } else { "files" };
    let title = if diagnostics.len() == 1 {
        terminal_safe_inline(reader_text(diagnostics[0]).0)
    } else {
        format!(
            "{} {occurrence_word} in {} {file_word}",
            diagnostics.len(),
            paths.len()
        )
    };
    let rule_id = terminal_safe_inline(&group.rule_id);
    let help = group.help.as_deref().map(terminal_safe_inline);
    let origin = terminal_safe_inline(&source_label(&group.source));
    let explain = format!("normfix explain {rule_id}");
    let remaining = diagnostics.len().saturating_sub(shown);
    let hidden = format!(
        "{remaining} further {} not shown; run the same command with --verbose",
        if remaining == 1 {
            "occurrence"
        } else {
            "occurrences"
        }
    );

    let mut report = Group::with_title(
        annotation_level(group.severity)
            .primary_title(title.as_str())
            .id(rule_id.as_str()),
    );
    for plan in &plans {
        report = report.element(plan.to_snippet());
    }
    for path in &missing_sources {
        report = report.element(Origin::path(path.as_str()));
    }
    if remaining > 0 {
        report = report.element(Level::NOTE.message(hidden.as_str()));
    }
    if let Some(help) = &help {
        report = report.element(Level::HELP.message(help.as_str()));
    }
    for note in &notes {
        report = report.element(Level::NOTE.message(note.as_str()));
    }
    report = report
        .element(Level::NOTE.with_name("source").message(origin.as_str()))
        .element(Level::NOTE.with_name("explain").message(explain.as_str()));

    let mut text = renderer.render(&[report]);
    text.push('\n');
    text
}

/// Every annotation normfix wants to place in one file's source.
struct SnippetPlan<'a> {
    path: &'a Utf8Path,
    safe_path: String,
    source: &'a str,
    spans: Vec<(Range<usize>, String)>,
}

impl<'a> SnippetPlan<'a> {
    fn to_snippet(&'a self) -> Snippet<'a, Annotation<'a>> {
        let mut snippet = Snippet::source(self.source)
            .path(self.safe_path.as_str())
            .fold(true);
        for (span, label) in &self.spans {
            let mut annotation = AnnotationKind::Primary.span(span.clone());
            if !label.is_empty() {
                annotation = annotation.label(label.as_str());
            }
            snippet = snippet.annotation(annotation);
        }
        snippet
    }
}

fn snippet_renderer(color: bool) -> Renderer {
    let renderer = if color {
        Renderer::styled()
    } else {
        Renderer::plain()
    };
    // Reading the real terminal width would make one report render two ways on
    // two machines, and these reports get diffed and pasted into issues.
    renderer.term_width(RENDER_WIDTH)
}

const fn annotation_level(severity: Severity) -> Level<'static> {
    match severity {
        Severity::Error => Level::ERROR,
        Severity::Warning => Level::WARNING,
        Severity::Info => Level::INFO,
    }
}

/// Whether pointing at this diagnostic's range would tell the reader the truth.
///
/// A compiler reports against the whole translation unit, so a diagnostic can
/// belong to a file whose local position is unknown; the pipeline records that
/// as an empty range at the start of the file and names the real location in a
/// note. Drawing a caret there would mark the 42 header block as the problem,
/// which is worse than showing no snippet at all.
fn has_locatable_range(diagnostic: &Diagnostic) -> bool {
    diagnostic.source != DiagnosticSource::Compiler
        || diagnostic.range.start().get() != 0
        || diagnostic.range.end().get() != 0
}

/// Clamps a diagnostic range to something the source can actually slice.
///
/// Ranges arrive from three independent authorities and travel through the
/// cache, so a stale entry or a column landing inside a multi-byte character
/// must degrade to a caret in roughly the right place, never to a panic in the
/// renderer.
fn snippet_span(source: &str, diagnostic: &Diagnostic) -> Range<usize> {
    let limit = source.len();
    let mut start = usize::try_from(diagnostic.range.start().get())
        .unwrap_or(usize::MAX)
        .min(limit);
    let mut end = usize::try_from(diagnostic.range.end().get())
        .unwrap_or(usize::MAX)
        .min(limit)
        .max(start);
    while start > 0 && !source.is_char_boundary(start) {
        start -= 1;
    }
    while end < limit && !source.is_char_boundary(end) {
        end += 1;
    }
    start..end
}

fn render_diagnostics_expanded(
    output: &mut String,
    paint: &Paint,
    files: &[FileReport],
    messages: &Messages,
) {
    let mut emitted_header = false;
    let renderer = snippet_renderer(paint.color);
    for file in files {
        let source = file.fixed.as_deref().or(file.original.as_deref());
        for diagnostic in &file.after {
            if !emitted_header {
                let _ = writeln!(output, "\n{}", messages.report_diagnostics_heading);
                emitted_header = true;
            }
            output.push('\n');
            output.push_str(&render_one_diagnostic(&renderer, diagnostic, source));
        }
    }
}

/// One diagnostic with its own snippet, help, notes and origin.
fn render_one_diagnostic(
    renderer: &Renderer,
    diagnostic: &Diagnostic,
    source: Option<&str>,
) -> String {
    let (reader_message, reader_notes, reader_help) = reader_text(diagnostic);
    let rule_id = terminal_safe_inline(&diagnostic.rule_id);
    let message = terminal_safe_inline(reader_message);
    let path = safe_path(&diagnostic.path);
    let help = reader_help.map(|help| terminal_safe_inline(help));
    let notes = reader_notes
        .iter()
        .map(|note| terminal_safe_inline(note))
        .collect::<Vec<_>>();
    let origin = terminal_safe_inline(&source_label(&diagnostic.source));

    let mut report = Group::with_title(
        annotation_level(diagnostic.severity)
            .primary_title(message.as_str())
            .id(rule_id.as_str()),
    );
    report = match source.filter(|_| has_locatable_range(diagnostic)) {
        Some(source) => report.element(
            Snippet::source(source)
                .path(path.as_str())
                .fold(true)
                .annotation(AnnotationKind::Primary.span(snippet_span(source, diagnostic))),
        ),
        // A cached report drops its source buffer, so the location is all that
        // survives. Naming it beats dropping the diagnostic.
        None => report.element(Origin::path(path.as_str())),
    };
    if let Some(help) = &help {
        report = report.element(Level::HELP.message(help.as_str()));
    }
    for note in &notes {
        report = report.element(Level::NOTE.message(note.as_str()));
    }
    report = report.element(Level::NOTE.with_name("source").message(origin.as_str()));

    let mut text = renderer.render(&[report]);
    text.push('\n');
    text
}

fn render_failures(output: &mut String, paint: &Paint, files: &[FileReport]) {
    for file in files {
        if let Some(failure) = &file.failure {
            let _ = writeln!(
                output,
                "\n{}FAILED{} {}: {}",
                paint.bold_red,
                paint.reset,
                safe_path(&file.path),
                terminal_safe_inline(failure)
            );
        }
    }
}

fn render_diffs(output: &mut String, files: &[FileReport]) {
    for file in files {
        if let Some(diff) = unified_diff(file) {
            let _ = writeln!(output, "\n{diff}");
        }
    }
}

/// Builds the unified diff for one changed file report.
///
/// Returns `None` when source buffers are unavailable or byte-identical.
#[must_use]
pub fn unified_diff(file: &FileReport) -> Option<String> {
    let (Some(original), Some(fixed)) = (&file.original, &file.fixed) else {
        return None;
    };
    if original == fixed {
        return None;
    }
    let diff = TextDiff::from_lines(original.as_ref(), fixed.as_ref())
        .unified_diff()
        .header(
            &format!("a/{}", safe_path(&file.path)),
            &format!("b/{}", safe_path(&file.path)),
        )
        .to_string();
    Some(terminal_safe_multiline(&diff))
}

fn render_summary(output: &mut String, paint: &Paint, report: &RunReport, messages: &Messages) {
    let summary = &report.summary;
    let written = report.files.iter().filter(|file| file.written).count();
    let counts = fill(
        messages.report_summary_counts,
        &[
            ("files", &summary.files.to_string()),
            ("proposed", &summary.changed.to_string()),
            ("written", &written.to_string()),
            ("fixes", &summary.fixes.to_string()),
            ("remaining", &summary.remaining.to_string()),
            ("info", &summary.advisories.to_string()),
            ("failed", &summary.failed.to_string()),
            ("unexpected", &summary.unexpected_files.to_string()),
            ("quarantined", &summary.quarantined.to_string()),
        ],
    );
    let _ = writeln!(
        output,
        "\n{}{}{} {counts}",
        paint.bold, messages.report_summary_label, paint.reset
    );
    let _ = writeln!(
        output,
        "{}",
        fill(
            messages.report_completed_in,
            &[("duration", &format_duration(report.duration_seconds))]
        )
    );
}

/// Returns the text a reader should see for a diagnostic.
///
/// A diagnostic authored by this project carries a translation; one relayed
/// from the official checker or the C compiler does not, and is shown exactly
/// as that tool produced it.
fn reader_text(diagnostic: &Diagnostic) -> (&str, &[String], Option<&String>) {
    diagnostic.localized.as_ref().map_or_else(
        || {
            (
                diagnostic.message.as_str(),
                diagnostic.notes.as_slice(),
                diagnostic.help.as_ref(),
            )
        },
        |localized| {
            (
                localized.message.as_str(),
                localized.notes.as_slice(),
                localized.help.as_ref(),
            )
        },
    )
}

fn has_blocking_diagnostic(diagnostics: &[Diagnostic]) -> bool {
    diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity != Severity::Info)
}

fn source_label(source: &DiagnosticSource) -> String {
    match source {
        DiagnosticSource::NativeNorm41 => "Norm v4.1 native rule".to_owned(),
        DiagnosticSource::NorminetteCompat(version) => {
            format!(
                "official Norminette {} compatibility",
                terminal_safe_inline(version)
            )
        }
        DiagnosticSource::Parser => "C parser".to_owned(),
        DiagnosticSource::Compiler => "C compiler".to_owned(),
        DiagnosticSource::Project => "project safety check".to_owned(),
        DiagnosticSource::Makefile => "Makefile check".to_owned(),
        DiagnosticSource::Markdown => "Markdown check".to_owned(),
    }
}

fn safe_path(path: &Utf8Path) -> String {
    terminal_safe_inline(path.as_str())
}

fn terminal_safe_inline(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    for character in input.chars() {
        match character {
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character if character.is_control() => {
                let _ = write!(output, "\\u{{{:x}}}", u32::from(character));
            }
            character => output.push(character),
        }
    }
    output
}

fn terminal_safe_multiline(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    for character in input.chars() {
        match character {
            // Newlines and tabs are meaningful bytes in a unified diff. They
            // are safe terminal controls and must remain copyable as-is.
            '\n' | '\t' => output.push(character),
            '\r' => output.push_str("\\r"),
            character if character.is_control() => {
                let _ = write!(output, "\\u{{{:x}}}", u32::from(character));
            }
            character => output.push(character),
        }
    }
    output
}

fn format_duration(seconds: f64) -> String {
    if seconds < 0.001 {
        return format!("{:.0} µs", seconds * 1_000_000.0);
    }
    if seconds < 1.0 {
        return format!("{:.0} ms", seconds * 1_000.0);
    }
    if seconds < 60.0 {
        return format!("{seconds:.2} s");
    }
    let minutes = (seconds / 60.0).floor();
    let remainder = seconds - minutes * 60.0;
    format!("{minutes:.0} min {remainder:.1} s")
}

struct Paint {
    /// Whether this run emits ANSI styling, which the snippet renderer needs
    /// to answer for itself.
    color: bool,
    reset: &'static str,
    bold: &'static str,
    green: &'static str,
    bold_green: &'static str,
    yellow: &'static str,
    bold_yellow: &'static str,
    bold_red: &'static str,
    bold_blue: &'static str,
    bold_cyan: &'static str,
}

impl Paint {
    const fn new(color: bool) -> Self {
        if color {
            Self {
                color,
                reset: "\x1b[0m",
                bold: "\x1b[1m",
                green: "\x1b[32m",
                bold_green: "\x1b[1;32m",
                yellow: "\x1b[33m",
                bold_yellow: "\x1b[1;33m",
                bold_red: "\x1b[1;31m",
                bold_blue: "\x1b[1;34m",

                bold_cyan: "\x1b[1;36m",
            }
        } else {
            Self {
                color,
                reset: "",
                bold: "",
                green: "",
                bold_green: "",
                yellow: "",
                bold_yellow: "",
                bold_red: "",
                bold_blue: "",

                bold_cyan: "",
            }
        }
    }
}

/// Builds a map used by callers that need direct source lookup by path.
#[must_use]
pub fn source_map(files: &[FileReport]) -> BTreeMap<&Utf8Path, &str> {
    files
        .iter()
        .filter_map(|file| {
            file.fixed
                .as_deref()
                .or(file.original.as_deref())
                .map(|source| (file.path.as_path(), source))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use camino::Utf8PathBuf;
    use normfix_core::{Diagnostic, DiagnosticSource, FixRecord, Severity, TextRange, TextSize};

    use super::{
        EvaluationGrade, EvaluationVerdict, FileReport, GROUPED_OCCURRENCE_LIMIT, Locale,
        RenderOptions, ReportIdentity, ReportMode, RunReport, render_human,
    };

    fn diagnostic() -> Diagnostic {
        Diagnostic {
            rule_id: "TOO_MANY_LINES".to_owned(),
            path: Utf8PathBuf::from("src/main.c"),
            range: TextRange::new(TextSize::new(5), TextSize::new(9)).expect("range"),
            severity: Severity::Warning,
            message: "Function exceeds the 25-line limit".to_owned(),
            source: DiagnosticSource::NativeNorm41,
            notes: vec!["main() has 30 body lines".to_owned()],
            help: Some("Extract one coherent responsibility into a static helper.".to_owned()),
            localized: None,
        }
    }

    fn file() -> FileReport {
        let source: Arc<str> = Arc::from("int\tmain(void)\n{\n\treturn (0);\n}\n");
        FileReport {
            path: Utf8PathBuf::from("src/main.c"),
            changed: true,
            written: false,
            backup: None,
            failure: None,
            fixes: vec![FixRecord {
                rule_id: "MIXED_SPACE_TAB".to_owned(),
                description: "normalized indentation".to_owned(),
                line: Some(3),
                count: 1,
            }],
            before: Vec::new(),
            after: vec![diagnostic()],
            original: Some(Arc::clone(&source)),
            fixed: Some(source),
        }
    }

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

    #[test]
    fn json_is_versioned_sorted_and_does_not_include_source_buffers() {
        let report = RunReport::new(
            "0.4.0",
            ReportMode::Diff,
            ReportIdentity::default(),
            vec!["z".to_owned(), "a".to_owned()],
            vec![Utf8PathBuf::from("z.bin"), Utf8PathBuf::from("a.bin")],
            vec![file()],
            Duration::ZERO,
        );
        let json = report.to_pretty_json().expect("JSON");

        assert!(json.contains("\"schema_version\": 2"));
        assert!(json.contains("\"mode\": \"diff\""));
        assert!(!json.contains("\"original\""));
        assert!(!json.contains("\"fixed\""));
        assert!(json.find("\"a\"").expect("a") < json.find("\"z\"").expect("z"));
        assert_eq!(report.exit_code(), 2);
    }

    #[test]
    fn report_schema_one_without_evaluation_still_deserializes() {
        let fixture = include_str!("../tests/fixtures/report-schema-v1.json");
        let decoded: RunReport = serde_json::from_str(fixture).expect("schema-one report");

        assert_eq!(decoded.schema_version, 1);
        assert!(decoded.evaluation.is_none());
        assert_eq!(decoded.tool_version, "1.0.0-rc.0");
    }

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
}
