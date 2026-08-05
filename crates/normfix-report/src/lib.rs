//! Stable machine reports and source-aware terminal diagnostics.
//!
//! Analysis crates emit backend-neutral data. This crate is the only layer
//! responsible for ANSI styling, snippets, tables, diffs and the versioned
//! JSON contract.

#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::sync::Arc;
use std::time::Duration;

use camino::{Utf8Path, Utf8PathBuf};
use normfix_core::{
    Diagnostic, DiagnosticSource, FixRecord, LineIndex, Severity, TextSize, visual_width,
};
use serde::{Deserialize, Serialize};
use similar::TextDiff;

/// Version of the stable JSON report schema.
pub const REPORT_SCHEMA_VERSION: u32 = 1;

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
            duration_seconds: duration.as_secs_f64(),
        }
    }

    /// Returns the documented process exit code.
    #[must_use]
    pub fn exit_code(&self) -> u8 {
        if !self.discovery_errors.is_empty()
            || !self.quarantine_errors.is_empty()
            || self.summary.failed > 0
        {
            return 2;
        }
        if self.summary.remaining > 0
            || (self.mode != ReportMode::Fix
                && (self.summary.changed > 0 || self.summary.quarantine_candidates > 0))
        {
            return 1;
        }
        0
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
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            color: true,
            verbose: false,
            show_diff: false,
        }
    }
}

/// Renders one complete human report.
#[must_use]
pub fn render_human(report: &RunReport, options: RenderOptions) -> String {
    let paint = Paint::new(options.color);
    let mut output = String::new();
    let _ = writeln!(
        output,
        "{}normfix{} {}",
        paint.bold_cyan, paint.reset, report.tool_version
    );
    output.push_str("Safe automatic fixes for the 42 Norm v4.1\n");
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
        "\n{}Project reminder:{} keep submitted code and permitted comments in English (not a Norm rule).",
        paint.bold_blue, paint.reset
    );
    render_discovery(&mut output, &paint, report);
    render_quarantine(&mut output, &paint, report);
    render_file_table(&mut output, &paint, &report.files, options.verbose);
    if options.verbose {
        render_fixes(&mut output, &paint, &report.files);
    }
    render_diagnostics(&mut output, &paint, &report.files, options.verbose);
    render_failures(&mut output, &paint, &report.files);
    if options.show_diff {
        render_diffs(&mut output, &report.files);
    }
    render_summary(&mut output, &paint, report);
    output
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

fn render_discovery(output: &mut String, paint: &Paint, report: &RunReport) {
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
    output.push_str("Only .c, .h, Makefile, and README files are expected.\n");
}

fn render_quarantine(output: &mut String, paint: &Paint, report: &RunReport) {
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
        output.push_str("  Preview mode did not move these files.\n");
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

fn render_file_table(output: &mut String, paint: &Paint, files: &[FileReport], verbose: bool) {
    output.push_str("\nFiles\n");
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
            "\n{}{label} — {}{}",
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
                "  {} ×{}{} — {}",
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

fn render_diagnostics(output: &mut String, paint: &Paint, files: &[FileReport], verbose: bool) {
    if verbose {
        render_diagnostics_expanded(output, paint, files);
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
                help: diagnostic.help.clone(),
            })
            .or_default()
            .push(diagnostic);
    }

    output.push_str("\nDiagnostics grouped by rule\n");
    let sources = source_map(files);
    for (group, mut diagnostics) in groups {
        diagnostics.sort_by(|left, right| {
            left.path
                .cmp(&right.path)
                .then_with(|| left.range.cmp(&right.range))
                .then_with(|| left.message.cmp(&right.message))
        });
        let paths = diagnostics
            .iter()
            .map(|diagnostic| diagnostic.path.as_path())
            .collect::<BTreeSet<_>>();
        let (level, style) = severity_label(group.severity, paint);
        let occurrence_word = if diagnostics.len() == 1 {
            "occurrence"
        } else {
            "occurrences"
        };
        let file_word = if paths.len() == 1 { "file" } else { "files" };
        let _ = writeln!(
            output,
            "\n{style}{level}[{}]:{} {} {occurrence_word} in {} {file_word}",
            terminal_safe_inline(&group.rule_id),
            paint.reset,
            diagnostics.len(),
            paths.len()
        );
        for diagnostic in diagnostics {
            let location = diagnostic_location(diagnostic, &sources);
            let _ = writeln!(
                output,
                "  {location:<36} {}",
                terminal_safe_inline(&diagnostic.message)
            );
            for note in &diagnostic.notes {
                let _ = writeln!(output, "    note: {}", terminal_safe_inline(note));
            }
        }
        if let Some(help) = &group.help {
            let _ = writeln!(output, " = help: {}", terminal_safe_inline(help));
        }
        let _ = writeln!(
            output,
            " = source: {}",
            terminal_safe_inline(&source_label(&group.source))
        );
        let _ = writeln!(
            output,
            " = explain: normfix explain {}",
            terminal_safe_inline(&group.rule_id)
        );
    }
}

fn diagnostic_location(diagnostic: &Diagnostic, sources: &BTreeMap<&Utf8Path, &str>) -> String {
    let Some(source) = sources.get(diagnostic.path.as_path()) else {
        return format!(
            "{}:{}..{}",
            safe_path(&diagnostic.path),
            diagnostic.range.start().get(),
            diagnostic.range.end().get()
        );
    };
    let Ok(index) = LineIndex::new(Arc::from(*source)) else {
        return safe_path(&diagnostic.path);
    };
    index.line_column(diagnostic.range.start()).map_or_else(
        || safe_path(&diagnostic.path),
        |position| {
            format!(
                "{}:{}:{}",
                safe_path(&diagnostic.path),
                position.line,
                position.visual_column
            )
        },
    )
}

fn render_diagnostics_expanded(output: &mut String, paint: &Paint, files: &[FileReport]) {
    let mut emitted_header = false;
    for file in files {
        let Some(source) = file.fixed.as_ref().or(file.original.as_ref()) else {
            for diagnostic in &file.after {
                if !emitted_header {
                    output.push_str("\nDiagnostics\n");
                    emitted_header = true;
                }
                render_diagnostic_without_source(output, paint, diagnostic);
            }
            continue;
        };
        let line_index = LineIndex::new(Arc::clone(source)).ok();
        for diagnostic in &file.after {
            if !emitted_header {
                output.push_str("\nDiagnostics\n");
                emitted_header = true;
            }
            if let Some(index) = &line_index {
                render_source_diagnostic(output, paint, diagnostic, source, index);
            } else {
                render_diagnostic_without_source(output, paint, diagnostic);
            }
        }
    }
}

fn render_diagnostic_without_source(output: &mut String, paint: &Paint, diagnostic: &Diagnostic) {
    let (level, style) = severity_label(diagnostic.severity, paint);
    let _ = writeln!(
        output,
        "\n{style}{level}[{}]:{} {}",
        terminal_safe_inline(&diagnostic.rule_id),
        paint.reset,
        terminal_safe_inline(&diagnostic.message)
    );
    let _ = writeln!(
        output,
        " --> {}:{}..{}",
        safe_path(&diagnostic.path),
        diagnostic.range.start().get(),
        diagnostic.range.end().get()
    );
    render_diagnostic_footer(output, diagnostic);
}

fn render_source_diagnostic(
    output: &mut String,
    paint: &Paint,
    diagnostic: &Diagnostic,
    source: &str,
    line_index: &LineIndex,
) {
    let Some(position) = line_index.line_column(diagnostic.range.start()) else {
        render_diagnostic_without_source(output, paint, diagnostic);
        return;
    };
    let (level, style) = severity_label(diagnostic.severity, paint);
    let _ = writeln!(
        output,
        "\n{style}{level}[{}]:{} {}",
        terminal_safe_inline(&diagnostic.rule_id),
        paint.reset,
        terminal_safe_inline(&diagnostic.message)
    );
    let _ = writeln!(
        output,
        " {}--> {}:{}:{}{}",
        paint.blue,
        safe_path(&diagnostic.path),
        position.line,
        position.visual_column,
        paint.reset
    );
    let Some(range) = line_index.line_range(position.line) else {
        render_diagnostic_footer(output, diagnostic);
        return;
    };
    let start = usize::try_from(range.start().get()).ok();
    let end = usize::try_from(range.end().get()).ok();
    let Some(raw_line) = start
        .zip(end)
        .and_then(|(start, end)| source.get(start..end))
        .map(|line| line.trim_end_matches(['\r', '\n']))
    else {
        render_diagnostic_footer(output, diagnostic);
        return;
    };
    let expanded = expand_tabs(raw_line);
    let number_width = position.line.to_string().len();
    let _ = writeln!(output, "{:>number_width$} |", "");
    let _ = writeln!(output, "{} | {expanded}", position.line);
    let caret_offset = position.visual_column.saturating_sub(1) as usize;
    let caret_length = diagnostic_caret_length(diagnostic, source, raw_line, range.start());
    let marker = format!(
        "{style}{}{reset}",
        "^".repeat(caret_length),
        reset = paint.reset
    );
    let _ = writeln!(
        output,
        "{:>number_width$} | {}{}",
        "",
        " ".repeat(caret_offset),
        marker
    );
    render_diagnostic_footer(output, diagnostic);
}

fn render_diagnostic_footer(output: &mut String, diagnostic: &Diagnostic) {
    if let Some(help) = &diagnostic.help {
        let _ = writeln!(output, " = help: {}", terminal_safe_inline(help));
    }
    for note in &diagnostic.notes {
        let _ = writeln!(output, " = note: {}", terminal_safe_inline(note));
    }
    let _ = writeln!(
        output,
        " = source: {}",
        terminal_safe_inline(&source_label(&diagnostic.source))
    );
}

fn diagnostic_caret_length(
    diagnostic: &Diagnostic,
    source: &str,
    raw_line: &str,
    line_start: TextSize,
) -> usize {
    let relative_start = diagnostic
        .range
        .start()
        .get()
        .saturating_sub(line_start.get()) as usize;
    let relative_end = diagnostic
        .range
        .end()
        .get()
        .saturating_sub(line_start.get()) as usize;
    let end = relative_end.min(raw_line.len());
    let start = relative_start.min(end);
    source
        .get(
            usize::try_from(line_start.get()).unwrap_or_default() + start
                ..usize::try_from(line_start.get()).unwrap_or_default() + end,
        )
        .map_or(1, |fragment| {
            visual_width(fragment, 1).saturating_sub(1) as usize
        })
        .max(1)
}

fn expand_tabs(line: &str) -> String {
    let mut output = String::new();
    let mut column = 1_u32;
    for character in line.chars() {
        if character == '\t' {
            let count = 4 - ((column - 1) % 4);
            output.push_str(&" ".repeat(count as usize));
            column += count;
        } else if character.is_control() {
            let escaped = format!("\\u{{{:x}}}", u32::from(character));
            column =
                column.saturating_add(u32::try_from(escaped.chars().count()).unwrap_or(u32::MAX));
            output.push_str(&escaped);
        } else {
            output.push(character);
            column = column.saturating_add(1);
        }
    }
    output
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

fn render_summary(output: &mut String, paint: &Paint, report: &RunReport) {
    let summary = &report.summary;
    let written = report.files.iter().filter(|file| file.written).count();
    let _ = writeln!(
        output,
        "\n{}Summary:{} {} files | {} proposed | {} written | {} fixes | {} remaining | {} info | {} failed | {} unexpected | {} quarantined",
        paint.bold,
        paint.reset,
        summary.files,
        summary.changed,
        written,
        summary.fixes,
        summary.remaining,
        summary.advisories,
        summary.failed,
        summary.unexpected_files,
        summary.quarantined
    );
    let _ = writeln!(
        output,
        "Completed in {}.",
        format_duration(report.duration_seconds)
    );
}

fn has_blocking_diagnostic(diagnostics: &[Diagnostic]) -> bool {
    diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity != Severity::Info)
}

fn severity_label(severity: Severity, paint: &Paint) -> (&'static str, &str) {
    match severity {
        Severity::Error => ("error", paint.bold_red),
        Severity::Warning => ("warning", paint.bold_yellow),
        Severity::Info => ("info", paint.bold_blue),
    }
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
    for segment in input.split_inclusive('\n') {
        if let Some(line) = segment.strip_suffix('\n') {
            output.push_str(&terminal_safe_inline(line));
            output.push('\n');
        } else {
            output.push_str(&terminal_safe_inline(segment));
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
    reset: &'static str,
    bold: &'static str,
    green: &'static str,
    bold_green: &'static str,
    yellow: &'static str,
    bold_yellow: &'static str,
    bold_red: &'static str,
    bold_blue: &'static str,
    blue: &'static str,
    bold_cyan: &'static str,
}

impl Paint {
    const fn new(color: bool) -> Self {
        if color {
            Self {
                reset: "\x1b[0m",
                bold: "\x1b[1m",
                green: "\x1b[32m",
                bold_green: "\x1b[1;32m",
                yellow: "\x1b[33m",
                bold_yellow: "\x1b[1;33m",
                bold_red: "\x1b[1;31m",
                bold_blue: "\x1b[1;34m",
                blue: "\x1b[34m",
                bold_cyan: "\x1b[1;36m",
            }
        } else {
            Self {
                reset: "",
                bold: "",
                green: "",
                bold_green: "",
                yellow: "",
                bold_yellow: "",
                bold_red: "",
                bold_blue: "",
                blue: "",
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

    use super::{FileReport, RenderOptions, ReportIdentity, ReportMode, RunReport, render_human};

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
            },
        );

        assert!(rendered.contains("warning[TOO_MANY_LINES]"));
        assert!(rendered.contains("--> src/main.c:1:6"));
        assert!(rendered.contains("1 | int main(void)"));
        assert!(rendered.contains('^'));
        assert!(rendered.contains("= help: Extract one coherent responsibility"));
        assert!(rendered.contains("= source: Norm v4.1 native rule"));
        assert!(!rendered.contains('\u{1b}'));
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
            },
        );

        assert!(rendered.contains("warning[TOO_MANY_LINES]: 2 occurrences in 2 files"));
        assert_eq!(rendered.matches("= help:").count(), 1);
        assert_eq!(rendered.matches("= source:").count(), 1);
        assert!(rendered.contains("src/main.c:1:6"));
        assert!(rendered.contains("src/other.c:1:6"));
        assert!(rendered.contains("= explain: normfix explain TOO_MANY_LINES"));
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

        assert!(json.contains("\"schema_version\": 1"));
        assert!(json.contains("\"mode\": \"diff\""));
        assert!(!json.contains("\"original\""));
        assert!(!json.contains("\"fixed\""));
        assert!(json.find("\"a\"").expect("a") < json.find("\"z\"").expect("z"));
        assert_eq!(report.exit_code(), 2);
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
            },
        );

        assert!(!rendered.contains('\u{1b}'));
        assert!(!rendered.contains('\u{7}'));
        assert!(rendered.contains("\\u{1b}"));
        assert!(rendered.contains("message\\u{1b}[2J\\nforged"));
        assert!(rendered.contains("note\\r\\u{7}"));
    }
}
