use std::fmt::Write as _;

use normfix_i18n::{Messages, fill};
use similar::TextDiff;

use crate::model::{FileReport, RunReport};
use crate::terminal::{
    Paint, format_duration, safe_path, terminal_safe_inline, terminal_safe_multiline,
};

pub(super) fn render_failures(output: &mut String, paint: &Paint, files: &[FileReport]) {
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

pub(super) fn render_diffs(output: &mut String, files: &[FileReport]) {
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

pub(super) fn render_summary(
    output: &mut String,
    paint: &Paint,
    report: &RunReport,
    messages: &Messages,
) {
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
