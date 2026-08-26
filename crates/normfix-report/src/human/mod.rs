mod diagnostics;
mod files;
mod outcome;

use std::fmt::Write as _;

use normfix_i18n::Locale;

use crate::evaluation::render_evaluation;
use crate::model::{FileReport, RunReport};
use crate::terminal::{Paint, terminal_safe_inline};

use diagnostics::render_diagnostics;
use files::{
    render_discovery, render_file_table, render_fixes, render_identity, render_quarantine,
};
use outcome::{render_diffs, render_failures, render_summary};

#[cfg(test)]
pub use diagnostics::GROUPED_OCCURRENCE_LIMIT;
pub use outcome::unified_diff;

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

/// Renders only the diagnostics of a set of files, with their source snippets.
///
/// `render_human` reports a formatting run and says a great deal a leak check
/// has no answer for: what was written, what was backed up, how many fixes were
/// accepted. A leak check has findings and sources and nothing else, and it
/// deserves the same caret under the same line as every other finding rather
/// than a second presentation invented for it.
#[must_use]
pub fn render_findings(files: &[FileReport], options: RenderOptions) -> String {
    let paint = Paint::new(options.color);
    let messages = normfix_i18n::messages(options.locale);
    let mut output = String::new();
    render_diagnostics(&mut output, &paint, files, options.verbose, messages);
    output
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
        paint.bold_cyan,
        paint.reset,
        terminal_safe_inline(&report.tool_version)
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
