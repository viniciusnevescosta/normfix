use std::fmt::Write as _;

use normfix_core::Severity;
use normfix_i18n::Messages;

use crate::model::{FileReport, FileStatus, ReportIdentity, RunReport};
use crate::terminal::{Paint, safe_path, terminal_safe_inline};

pub(super) fn render_identity(output: &mut String, paint: &Paint, identity: &ReportIdentity) {
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

pub(super) fn render_discovery(
    output: &mut String,
    paint: &Paint,
    report: &RunReport,
    messages: &Messages,
) {
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

pub(super) fn render_quarantine(
    output: &mut String,
    paint: &Paint,
    report: &RunReport,
    messages: &Messages,
) {
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

pub(super) fn render_file_table(
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
        let status = file.status();
        if !verbose && status == FileStatus::Clean {
            continue;
        }
        let (label, style) = match status {
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

pub(super) fn render_fixes(output: &mut String, paint: &Paint, files: &[FileReport]) {
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
