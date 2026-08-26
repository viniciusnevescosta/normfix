use std::fmt::Write as _;
use std::sync::Arc;

use camino::{Utf8Path, Utf8PathBuf};
use normfix_core::{Diagnostic, DiagnosticSource, LineIndex, Severity};
use normfix_i18n::{Messages, fill};
use serde::{Deserialize, Serialize};

use crate::model::{ReportMode, RunReport};
use crate::terminal::{Paint, safe_path, terminal_safe_inline};

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

pub fn render_evaluation(
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

#[derive(Default)]
struct EvaluationEvidence {
    hard_failures: Vec<EvaluationFinding>,
    norm_count: usize,
    makefile_count: usize,
    other_blocking: usize,
}

fn collect_evaluation_evidence(report: &RunReport) -> EvaluationEvidence {
    let mut evidence = EvaluationEvidence {
        hard_failures: report
            .unexpected_files
            .iter()
            .map(|path| EvaluationFinding {
                rule_id: "UNEXPECTED_PROJECT_FILE".to_owned(),
                path: path.clone(),
                line: None,
                column: None,
                message: "Unexpected project file is present in the evaluated scope.".to_owned(),
            })
            .collect(),
        ..EvaluationEvidence::default()
    };
    for file in &report.files {
        if file.failure.is_some() && is_makefile_path(&file.path) {
            evidence.makefile_count += 1;
            evidence.hard_failures.push(EvaluationFinding {
                rule_id: "MAKEFILE_OPERATION_FAILED".to_owned(),
                path: file.path.clone(),
                line: None,
                column: None,
                message: "The Makefile could not be evaluated completely.".to_owned(),
            });
        }
        // A pre-defense hard fail describes the bytes currently on disk. A
        // successful fix transaction makes the validated shadow authoritative;
        // check/diff, a refused write, or a failed transaction leaves the
        // original authoritative. Some report producers have no distinct
        // `before` phase, so an empty one falls back to `after`.
        let written_shadow = report.mode == ReportMode::Fix && file.written;
        let (authoritative, source) = if written_shadow || file.before.is_empty() {
            (file.after.as_slice(), file.fixed.as_deref())
        } else {
            (file.before.as_slice(), file.original.as_deref())
        };
        // Building the visual-column index is linear in the source length.
        // Reuse one index for every finding in this file instead of rescanning
        // the same source once per diagnostic.
        let line_index = source.and_then(|source| LineIndex::new(Arc::from(source)).ok());
        for diagnostic in authoritative {
            let Some(kind) = evaluation_failure_kind(diagnostic) else {
                if diagnostic.severity != Severity::Info {
                    evidence.other_blocking += 1;
                }
                continue;
            };
            increment_evaluation_count(
                kind,
                &mut evidence.norm_count,
                &mut evidence.makefile_count,
            );
            push_evaluation_finding(&mut evidence.hard_failures, diagnostic, line_index.as_ref());
        }
    }
    evidence.hard_failures.sort();
    evidence.hard_failures.dedup();
    evidence
}

// Keeping current-snapshot evidence, shadow-only additions, bounded scoring,
// and the non-conclusive verdict together makes the grading boundary auditable.
pub fn build_preflight_evaluation(report: &RunReport) -> EvaluationReport {
    let evidence = collect_evaluation_evidence(report);
    let operational =
        report.discovery_errors.len() + report.quarantine_errors.len() + report.summary.failed;
    let mut deduction = evidence.norm_count.saturating_mul(8).min(45)
        + evidence.makefile_count.saturating_mul(8).min(30)
        + report.unexpected_files.len().saturating_mul(5).min(25)
        + evidence.other_blocking.saturating_mul(2).min(20)
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
    let verdict = if !evidence.hard_failures.is_empty() {
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
        hard_failures: evidence.hard_failures,
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
    line_index: Option<&LineIndex>,
) {
    let (line, column) = diagnostic_location(line_index, diagnostic);
    hard_failures.push(EvaluationFinding {
        rule_id: diagnostic.rule_id.clone(),
        path: diagnostic.path.clone(),
        line,
        column,
        message: diagnostic.message.clone(),
    });
}

fn diagnostic_location(
    line_index: Option<&LineIndex>,
    diagnostic: &Diagnostic,
) -> (Option<u32>, Option<u32>) {
    let Some(index) = line_index else {
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
