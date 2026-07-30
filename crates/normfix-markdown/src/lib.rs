//! Opt-in, AST-backed Markdown checks and canonical formatting.
//!
//! README files remain untouched by default. Callers must explicitly enable
//! canonical formatting because `CommonMark` reprinting can produce a broad
//! diff even when the document is semantically equivalent.

#![forbid(unsafe_code)]

use comrak::{Arena, Options, format_commonmark, nodes::NodeValue, parse_document};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// One Markdown issue in stable source order.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct MarkdownIssue {
    /// Stable rule identifier.
    pub rule_id: String,
    /// One-based source line.
    pub line: u32,
    /// English explanation.
    pub message: String,
    /// Concrete next step.
    pub help: String,
}

/// Markdown analysis and optional canonical candidate.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MarkdownResult {
    /// Deterministically ordered issues.
    pub issues: Vec<MarkdownIssue>,
    /// Canonical `CommonMark` source when explicitly requested.
    pub formatted: Option<String>,
}

/// Markdown formatting failed.
#[derive(Debug, Error)]
pub enum MarkdownError {
    /// Comrak could not print the parsed tree.
    #[error("could not format Markdown: {0}")]
    Format(#[from] std::fmt::Error),
}

/// Parses Markdown once, emits structural issues and optionally reprints it.
///
/// # Errors
///
/// Returns [`MarkdownError`] only when canonical reprinting was requested and
/// failed. Analysis alone is total for UTF-8 input.
pub fn analyze_markdown(
    source: &str,
    canonical_format: bool,
) -> Result<MarkdownResult, MarkdownError> {
    let arena = Arena::new();
    let options = Options::default();
    let root = parse_document(&arena, source, &options);
    let mut issues = structural_issues(root);
    issues.extend(line_issues(source));
    issues.sort();
    issues.dedup();
    let formatted = if canonical_format {
        let mut formatted = String::new();
        format_commonmark(root, &options, &mut formatted)?;
        if !formatted.is_empty() && !formatted.ends_with('\n') {
            formatted.push('\n');
        }
        Some(formatted)
    } else {
        None
    };
    Ok(MarkdownResult { issues, formatted })
}

fn structural_issues<'a>(root: &'a comrak::nodes::AstNode<'a>) -> Vec<MarkdownIssue> {
    let mut issues = Vec::new();
    let mut previous_heading = None;
    for node in root.descendants() {
        let data = node.data.borrow();
        if let NodeValue::Heading(heading) = &data.value {
            let level = u32::from(heading.level);
            if let Some(previous) = previous_heading {
                if level > previous + 1 {
                    issues.push(MarkdownIssue {
                        rule_id: "MD_HEADING_LEVEL_JUMP".to_owned(),
                        line: u32::try_from(data.sourcepos.start.line).unwrap_or(u32::MAX),
                        message: format!("Heading level jumps from {previous} to {level}"),
                        help: "Use the next consecutive heading level or restructure the section."
                            .to_owned(),
                    });
                }
            }
            previous_heading = Some(level);
        }
    }
    issues
}

fn line_issues(source: &str) -> Vec<MarkdownIssue> {
    let mut issues = Vec::new();
    for (index, line) in source.lines().enumerate() {
        if line.ends_with(' ') || line.ends_with('\t') {
            issues.push(MarkdownIssue {
                rule_id: "MD_TRAILING_WHITESPACE".to_owned(),
                line: u32::try_from(index + 1).unwrap_or(u32::MAX),
                message: "Line has trailing whitespace".to_owned(),
                help: "Remove trailing spaces unless an intentional hard line break is required."
                    .to_owned(),
            });
        }
    }
    if !source.is_empty() && !source.ends_with('\n') {
        issues.push(MarkdownIssue {
            rule_id: "MD_FINAL_NEWLINE".to_owned(),
            line: u32::try_from(source.lines().count()).unwrap_or(u32::MAX),
            message: "Document does not end with a newline".to_owned(),
            help: "Add exactly one final newline.".to_owned(),
        });
    }
    issues
}

#[cfg(test)]
mod tests {
    use super::analyze_markdown;

    #[test]
    fn analysis_is_read_only_unless_canonical_format_is_requested() {
        let source = "# Title\n\n### Jump  \n";
        let result = analyze_markdown(source, false).expect("analysis");

        assert!(result.formatted.is_none());
        assert!(
            result
                .issues
                .iter()
                .any(|issue| issue.rule_id == "MD_HEADING_LEVEL_JUMP")
        );
        assert!(
            result
                .issues
                .iter()
                .any(|issue| issue.rule_id == "MD_TRAILING_WHITESPACE")
        );
    }

    #[test]
    fn canonical_printing_is_idempotent() {
        let source = "# Title\n\n- one\n- two\n";
        let first = analyze_markdown(source, true)
            .expect("first")
            .formatted
            .expect("formatted");
        let second = analyze_markdown(&first, true)
            .expect("second")
            .formatted
            .expect("formatted");

        assert_eq!(first, second);
        assert!(first.ends_with('\n'));
    }
}
