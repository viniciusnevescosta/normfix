//! README handling.
//!
//! Canonical `CommonMark` reprinting is idempotent but can produce a broad first
//! diff, so it stays behind the same preview and approval path as source code.

use std::sync::Arc;

use camino::Utf8PathBuf;
use normfix_actions::PlannedFile;
use normfix_core::{Diagnostic, DiagnosticSource, FixRecord, Severity, TextRange, TextSize};
use normfix_markdown::analyze_markdown;
use normfix_report::FileReport;

use normfix_project::DiscoveredFile;

use super::{FileWork, FixOptions, failed_source, line_point_range};

pub(super) fn process_markdown(
    file: &DiscoveredFile,
    path: Utf8PathBuf,
    original_bytes: Vec<u8>,
    original: String,
    options: &FixOptions,
) -> FileWork {
    let result = match analyze_markdown(&original, options.format_markdown && !options.lint_only) {
        Ok(result) => result,
        Err(error) => {
            return failed_source(file, path, original_bytes, original, error.to_string());
        }
    };
    let proposed = result.formatted.unwrap_or_else(|| original.clone());
    let mut after = result
        .issues
        .into_iter()
        .map(|issue| Diagnostic {
            rule_id: issue.rule_id,
            path: path.clone(),
            range: line_point_range(&proposed, issue.line),
            severity: Severity::Info,
            message: issue.message,
            source: DiagnosticSource::Markdown,
            notes: Vec::new(),
            help: Some(issue.help),
            localized: None,
        })
        .collect::<Vec<_>>();
    if options.preflight {
        after.push(Diagnostic {
            rule_id: "README_42_CRITERIA_REVIEW".to_owned(),
            path: path.clone(),
            range: TextRange::empty(TextSize::new(0)),
            severity: Severity::Info,
            message: "A README is present, but its subject-specific 42 evaluation criteria cannot be proven automatically."
                .to_owned(),
            source: DiagnosticSource::Markdown,
            notes: vec![
                "README absence is not a normfix preflight failure; this advisory exists only when a README was discovered."
                    .to_owned(),
            ],
            help: Some(
                "Compare the document with the current subject and evaluation sheet: required overview, instructions, resources, attribution, and project-specific sections vary by cursus project."
                    .to_owned(),
            ),
            localized: None,
        });
    }
    after.sort();
    after.dedup();
    let changed = proposed.as_bytes() != original_bytes;
    let fix_records = changed
        .then(|| FixRecord {
            rule_id: "MARKDOWN_CANONICAL_FORMAT".to_owned(),
            description: "reprinted the README through the CommonMark syntax tree".to_owned(),
            line: None,
            count: 1,
        })
        .into_iter()
        .collect::<Vec<_>>();
    let plan = changed.then(|| PlannedFile {
        path: file.path.clone(),
        original: original_bytes.clone(),
        replacement: proposed.as_bytes().to_vec(),
        fixes: fix_records.clone(),
    });
    FileWork {
        absolute_path: file.path.clone(),
        report: FileReport {
            path,
            changed,
            written: false,
            backup: None,
            failure: None,
            fixes: fix_records,
            before: Vec::new(),
            after,
            original: Some(Arc::from(original)),
            fixed: Some(Arc::from(proposed)),
        },
        plan,
        read_preconditions: Vec::new(),
    }
}
