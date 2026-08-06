//! Native, conservative C actions for `normfix`.
//!
//! This crate is intentionally independent from the command-line and write
//! layers. It accepts an immutable UTF-8 buffer plus optional diagnostics from
//! Norminette, applies deterministic edits in a shadow buffer, and returns the
//! resulting text. Every layout action is reparsed and must preserve the exact
//! significant token stream, including comments.

#![forbid(unsafe_code)]

mod analysis;
mod context;
mod edit;
mod source;
mod transforms;

use std::collections::{BTreeSet, HashSet};

use camino::Utf8Path;
use normfix_c_syntax::{CParser, ParseFailure};
pub use normfix_core::{Applicability, Diagnostic, Severity, TextRange, TextSize};
use thiserror::Error;

pub use analysis::{
    ExternalCallCandidate, FunctionBudget, analyze_budget, analyze_c, analyze_external_calls,
};
pub use edit::{Edit, EditError, apply_edits};
pub use source::{HygieneResult, normalize_hygiene, visual_width};

use crate::context::{FingerprintMode, ParsedContext};
use crate::transforms::{ActionBatch, phases};

/// A diagnostic reported by an external Norminette-compatible checker.
///
/// Lines and display columns are one-based. Display columns use the 42
/// four-column tab-stop convention.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ReportedDiagnostic {
    /// Stable Norminette rule code.
    pub code: String,
    /// One-based physical line.
    pub line: u32,
    /// One-based display column.
    pub visual_column: u32,
    /// Human-readable checker message.
    pub message: String,
}

impl ReportedDiagnostic {
    /// Creates a diagnostic at `line:visual_column`.
    #[must_use]
    pub fn new(
        code: impl Into<String>,
        line: u32,
        visual_column: u32,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code: code.into(),
            line,
            visual_column,
            message: message.into(),
        }
    }
}

/// Configuration for one native C action run.
// Each switch enables an independent phase; collapsing them into one state enum
// would incorrectly make combinations that are valid together exclusive.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CActionOptions {
    /// Maximum permitted display columns per physical line.
    pub max_columns: u32,
    /// Maximum accepted formatting batches before declaring instability.
    pub max_passes: usize,
    /// Explicit permission to delete comments rejected by Norminette.
    pub remove_invalid_comments: bool,
    /// Format unambiguously simple prototype and variable declaration groups
    /// even when an external checker did not report their location.
    pub format_proven_declarations: bool,
    /// Explicit permission to compact standard `NULL` comparisons into unary
    /// truth tests. This is opt-in because projects may redefine `NULL`.
    pub compact_null_checks: bool,
    /// Reorder contiguous include blocks so system headers precede project
    /// headers, alphabetically inside each category. Enabled by default; a
    /// block is only rewritten when every one of its lines is exactly one
    /// include directive.
    pub reorder_includes: bool,
}

impl Default for CActionOptions {
    fn default() -> Self {
        Self {
            max_columns: 80,
            max_passes: 100,
            remove_invalid_comments: false,
            format_proven_declarations: true,
            compact_null_checks: false,
            reorder_includes: true,
        }
    }
}

/// One accepted fix.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Fix {
    /// Rule or native action identifier.
    pub rule_id: String,
    /// Concise English description.
    pub description: String,
    /// One-based source line when known.
    pub line: Option<u32>,
    /// Safety class proven by the action.
    pub applicability: Applicability,
}

/// Result of formatting one C translation unit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CActionResult {
    /// Fully formatted source. The caller remains responsible for writing it.
    pub source: String,
    /// Accepted fixes in application order.
    pub fixes: Vec<Fix>,
    /// Native structural and review-required diagnostics for the final source.
    pub diagnostics: Vec<Diagnostic>,
    /// Whether the fixed-point scheduler reached a stable result.
    pub stable: bool,
}

impl CActionResult {
    /// Returns whether at least one fix changed the source.
    #[must_use]
    pub fn changed(&self) -> bool {
        !self.fixes.is_empty()
    }
}

/// Failure that prevents a safe action run.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum CActionError {
    /// The embedded parser could not analyze the source.
    #[error("could not parse C source: {0}")]
    Parser(String),
    /// Tree-sitter recovered from invalid or unsupported syntax.
    #[error("automatic edits require a lossless C parse without ERROR, MISSING, or unknown bytes")]
    UnsafeSyntax,
    /// A proposed edit set was invalid or overlapping.
    #[error(transparent)]
    Edit(#[from] EditError),
    /// A supposedly safe layout action changed significant tokens.
    #[error("layout action {rule_id} changed the significant C token stream")]
    TokenMismatch {
        /// Action whose proof failed.
        rule_id: String,
    },
    /// A candidate produced parser recovery or unknown bytes.
    #[error("action {rule_id} made the C source unsafe to edit")]
    CandidateSyntax {
        /// Action whose parse validation failed.
        rule_id: String,
    },
    /// The fixed-point scheduler detected an edit cycle.
    #[error("native C actions entered a formatting cycle")]
    Cycle,
    /// The configured pass bound was exhausted while work remained.
    #[error("native C actions did not converge within {passes} passes")]
    PassLimit {
        /// Configured bound.
        passes: usize,
    },
}

impl From<ParseFailure> for CActionError {
    fn from(value: ParseFailure) -> Self {
        Self::Parser(value.to_string())
    }
}

/// Applies the conservative native C action pipeline.
///
/// The input is never mutated. Hygiene normalization happens first in the
/// shadow buffer; the normalized source must then parse losslessly before any
/// syntax-aware action is considered. Layout batches are accepted only when
/// the ordered token/comment fingerprint is identical. Comment removal is
/// opt-in and uses the stricter code-token fingerprint proof.
///
/// # Errors
///
/// Returns an error when the input cannot be proven safe, an edit conflicts,
/// a proof fails, or formatting does not converge. On error the caller still
/// owns the unchanged original buffer.
pub fn apply_c_actions(
    path: &Utf8Path,
    source: &str,
    reported: &[ReportedDiagnostic],
    options: &CActionOptions,
) -> Result<CActionResult, CActionError> {
    let mut parser = CParser::new()?;
    let (mut current, mut fixes) = prepare_source(&mut parser, source)?;
    let mut active_diagnostics = reported.to_vec();

    let mut seen = HashSet::new();
    seen.insert(source_digest(&current));
    let mut completed_one_shot = BTreeSet::new();
    let ordered_phases = phases(options);
    let mut stable = false;

    for _ in 0..options.max_passes {
        let mut accepted = false;
        for phase in &ordered_phases {
            if phase.one_shot() && completed_one_shot.contains(phase) {
                continue;
            }
            let before = ParsedContext::parse(&mut parser, &current)?;
            before.require_safe()?;
            let Some(batch) = phase.plan(&before, &active_diagnostics, options)? else {
                if phase.one_shot() {
                    completed_one_shot.insert(*phase);
                }
                continue;
            };
            if batch.edits.is_empty() {
                if phase.one_shot() {
                    completed_one_shot.insert(*phase);
                }
                continue;
            }
            let (candidate, accepted_edits) = apply_edits(&current, &batch.edits)?;
            if accepted_edits.is_empty() || candidate == current {
                if phase.one_shot() {
                    completed_one_shot.insert(*phase);
                }
                continue;
            }
            validate_candidate(&mut parser, &before, &candidate, &batch)?;
            if !seen.insert(source_digest(&candidate)) {
                return Err(CActionError::Cycle);
            }
            active_diagnostics =
                remap_diagnostics(&current, &candidate, &active_diagnostics, &accepted_edits);
            current = candidate;
            fixes.extend(accepted_edits.into_iter().map(|edit| Fix {
                rule_id: edit.rule_id,
                description: edit.description,
                line: edit.line,
                applicability: batch.applicability,
            }));
            if phase.one_shot() {
                completed_one_shot.insert(*phase);
            }
            accepted = true;
            break;
        }
        if !accepted {
            stable = true;
            break;
        }
    }

    if !stable {
        let context = ParsedContext::parse(&mut parser, &current)?;
        if ordered_phases.iter().any(|phase| {
            phase
                .plan(&context, &active_diagnostics, options)
                .ok()
                .flatten()
                .is_some()
        }) {
            return Err(CActionError::PassLimit {
                passes: options.max_passes,
            });
        }
        stable = true;
    }

    let final_context = ParsedContext::parse(&mut parser, &current)?;
    final_context.require_safe()?;
    let diagnostics = final_diagnostics(
        path,
        &final_context,
        &active_diagnostics,
        options.max_columns,
    );

    Ok(CActionResult {
        source: current,
        fixes,
        diagnostics,
        stable,
    })
}

fn source_digest(source: &str) -> [u8; 32] {
    *blake3::hash(source.as_bytes()).as_bytes()
}

fn prepare_source(parser: &mut CParser, source: &str) -> Result<(String, Vec<Fix>), CActionError> {
    let original = ParsedContext::parse(parser, source)?;
    original.require_safe()?;
    let hygiene = normalize_hygiene(source)?;
    let normalized = ParsedContext::parse(parser, &hygiene.source)?;
    normalized.require_safe()?;
    if original.fingerprint(FingerprintMode::CodeOnly)
        != normalized.fingerprint(FingerprintMode::CodeOnly)
    {
        return Err(CActionError::TokenMismatch {
            rule_id: "SOURCE_HYGIENE".to_owned(),
        });
    }
    Ok((hygiene.source, hygiene.fixes))
}

fn final_diagnostics(
    path: &Utf8Path,
    context: &ParsedContext,
    reported: &[ReportedDiagnostic],
    max_columns: u32,
) -> Vec<Diagnostic> {
    let mut diagnostics = analysis::analyze_native(path, context, max_columns);
    diagnostics.extend(analysis::unsupported_reported_diagnostics(
        path, context, reported,
    ));
    diagnostics.sort();
    diagnostics.dedup();
    diagnostics
}

fn remap_diagnostics(
    before: &str,
    after: &str,
    diagnostics: &[ReportedDiagnostic],
    edits: &[Edit],
) -> Vec<ReportedDiagnostic> {
    let before_index = source::SourceLines::index(before);
    let after_index = source::SourceLines::index(after);
    let before_lines = source::SourceLines::new(before, &before_index);
    let after_lines = source::SourceLines::new(after, &after_index);
    diagnostics
        .iter()
        .map(|diagnostic| {
            let Some(line) = before_lines.get(diagnostic.line) else {
                return diagnostic.clone();
            };
            let anchor = before_lines.byte_for_visual_column(line, diagnostic.visual_column);
            let mapped = map_offset(anchor, edits).min(after.len());
            let mapped = floor_char_boundary(after, mapped);
            let line_number = after_lines.line_number_at(mapped);
            let visual_column = after_lines
                .get(line_number)
                .map_or(1, |line| after_lines.visual_column(line, mapped));
            ReportedDiagnostic {
                code: diagnostic.code.clone(),
                line: line_number,
                visual_column,
                message: diagnostic.message.clone(),
            }
        })
        .collect()
}

fn map_offset(anchor: usize, edits: &[Edit]) -> usize {
    let mut mapped = anchor;
    for edit in edits {
        let start = edit.range.start().get() as usize;
        let end = edit.range.end().get() as usize;
        if anchor < start {
            break;
        }
        let replacement_len = edit.replacement.len();
        if start == end {
            mapped = mapped.saturating_add(replacement_len);
        } else if anchor >= end {
            let removed = end - start;
            if replacement_len >= removed {
                mapped = mapped.saturating_add(replacement_len - removed);
            } else {
                mapped = mapped.saturating_sub(removed - replacement_len);
            }
        } else {
            let within = anchor.saturating_sub(start).min(replacement_len);
            return mapped
                .saturating_sub(anchor.saturating_sub(start))
                .saturating_add(within);
        }
    }
    mapped
}

fn floor_char_boundary(source: &str, mut offset: usize) -> usize {
    while offset > 0 && !source.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
}

fn validate_candidate(
    parser: &mut CParser,
    before: &ParsedContext,
    candidate: &str,
    batch: &ActionBatch,
) -> Result<(), CActionError> {
    let after = ParsedContext::parse(parser, candidate)?;
    if after.require_safe().is_err() {
        return Err(CActionError::CandidateSyntax {
            rule_id: batch.rule_id.to_owned(),
        });
    }
    let fingerprints_match = match batch.applicability {
        Applicability::SafeLayout => {
            before.fingerprint(FingerprintMode::TokensAndComments)
                == after.fingerprint(FingerprintMode::TokensAndComments)
        }
        Applicability::UnsafeDestructive => {
            before.fingerprint(FingerprintMode::CodeOnly)
                == after.fingerprint(FingerprintMode::CodeOnly)
        }
        Applicability::SafeSemantic => true,
        Applicability::ReviewRequired => false,
    };
    if !fingerprints_match {
        return Err(CActionError::TokenMismatch {
            rule_id: batch.rule_id.to_owned(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests;
