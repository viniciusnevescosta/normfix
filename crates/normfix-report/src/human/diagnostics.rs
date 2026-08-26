use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::ops::Range;

use annotate_snippets::{Annotation, AnnotationKind, Group, Level, Origin, Renderer, Snippet};
use camino::Utf8Path;
use normfix_core::{Diagnostic, DiagnosticSource, Severity};
use normfix_i18n::Messages;

use crate::model::FileReport;
use crate::source::source_map;
use crate::terminal::terminal_safe_source;
use crate::terminal::{Paint, reader_text, safe_path, source_label, terminal_safe_inline};

/// Fixed width every snippet is rendered against.
///
/// Norm-conforming lines fit in 80 columns; the rest is gutter and margin for
/// the few lines that do not yet conform.
const RENDER_WIDTH: usize = 120;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct DiagnosticGroupKey<'a> {
    severity: Severity,
    rule_id: &'a str,
    source: &'a DiagnosticSource,
    help: Option<&'a str>,
}

/// Occurrences shown per rule before the rest are summarized.
///
/// A project can carry thousands of one diagnostic. Printing every snippet
/// would make the report unreadable in exactly the way snippets are meant to
/// prevent, so the default shows enough to recognize the pattern and names the
/// flag that shows the rest.
pub const GROUPED_OCCURRENCE_LIMIT: usize = 3;

pub(super) fn render_diagnostics(
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
    let mut groups = BTreeMap::<DiagnosticGroupKey<'_>, Vec<&Diagnostic>>::new();
    for diagnostic in files.iter().flat_map(|file| &file.after) {
        groups
            .entry(DiagnosticGroupKey {
                severity: diagnostic.severity,
                rule_id: diagnostic.rule_id.as_str(),
                source: &diagnostic.source,
                help: reader_text(diagnostic).2.map(String::as_str),
            })
            .or_default()
            .push(diagnostic);
    }
    if groups.is_empty() {
        return;
    }

    let _ = writeln!(output, "\n{}", messages.report_grouped_heading);
    let sources = source_map(files)
        .into_iter()
        .map(|(path, source)| (path, terminal_safe_source(source)))
        .collect::<BTreeMap<_, _>>();
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
    group: &DiagnosticGroupKey<'_>,
    diagnostics: &[&Diagnostic],
    sources: &BTreeMap<&Utf8Path, Cow<'_, str>>,
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
            .map(AsRef::as_ref)
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
    let rule_id = terminal_safe_inline(group.rule_id);
    let help = group.help.map(terminal_safe_inline);
    let origin = terminal_safe_inline(&source_label(group.source));
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

const fn snippet_renderer(color: bool) -> Renderer {
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
        let safe_source = source.map(terminal_safe_source);
        for diagnostic in &file.after {
            if !emitted_header {
                let _ = writeln!(output, "\n{}", messages.report_diagnostics_heading);
                emitted_header = true;
            }
            output.push('\n');
            output.push_str(&render_one_diagnostic(
                &renderer,
                diagnostic,
                safe_source.as_deref(),
            ));
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
