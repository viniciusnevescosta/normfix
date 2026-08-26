use std::sync::Arc;
use std::time::Duration;

use camino::Utf8PathBuf;
use normfix_core::{Diagnostic, FixRecord, Severity};
use serde::{Deserialize, Serialize};

use crate::evaluation::{EvaluationReport, EvaluationVerdict};

/// Version of the stable JSON report schema.
pub const REPORT_SCHEMA_VERSION: u32 = 2;

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

/// One function's headroom against the Norm's limits.
///
/// The same numbers appear in the `NORM_BUDGET` sentence a person reads. A
/// caller should not have to parse that sentence to get them back, which is the
/// whole reason this exists beside it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FunctionBudget {
    /// Function identifier.
    pub function: String,
    /// One-based line carrying the identifier.
    pub line: u32,
    /// Physical lines between the function braces.
    pub lines: u32,
    /// The Norm's limit for those lines.
    pub line_limit: u32,
    /// Locals in the initial declaration block.
    pub variables: u32,
    /// The Norm's limit for those locals.
    pub variable_limit: u32,
    /// Declared parameters.
    pub parameters: u32,
    /// The Norm's limit for those parameters.
    pub parameter_limit: u32,
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
    /// Per-function headroom, when the run was asked for it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub budget: Vec<FunctionBudget>,
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

fn has_blocking_diagnostic(diagnostics: &[Diagnostic]) -> bool {
    diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity != Severity::Info)
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
        let mut summary = ReportSummary {
            files: files.len(),
            unexpected_files: unexpected_files.len(),
            ..ReportSummary::default()
        };
        for file in &files {
            summary.changed += usize::from(file.changed);
            summary.written += usize::from(file.written);
            summary.failed += usize::from(file.failure.is_some());
            summary.fixes += file
                .fixes
                .iter()
                .map(|fix| u64::from(fix.count))
                .sum::<u64>();
            for diagnostic in &file.after {
                if diagnostic.severity == Severity::Info {
                    summary.advisories += 1;
                } else {
                    summary.remaining += 1;
                }
            }
        }
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
        self.evaluation = Some(crate::evaluation::build_preflight_evaluation(self));
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
}
