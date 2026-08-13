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

use camino::{Utf8Path, Utf8PathBuf};
use normfix_c_syntax::{CParser, ParseFailure};
pub use normfix_core::{
    Applicability, Diagnostic, DiagnosticSource, Severity, TextRange, TextSize,
};
use thiserror::Error;

pub use analysis::{
    ExternalCallCandidate, FunctionBudget, analyze_budget, analyze_c, analyze_external_calls,
};
pub use edit::{Edit, EditError, apply_edits};
pub use source::{HygieneResult, normalize_hygiene, visual_width};

use crate::context::{FingerprintMode, ParsedContext};
use crate::transforms::{ActionBatch, Phase, phases};

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
    /// Explicit permission to delete a local the compiler proved unused, when
    /// the declaration carries nothing that runs.
    pub remove_unused_variables: bool,
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
            remove_unused_variables: false,
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
    /// How many times the source was parsed to produce this result.
    ///
    /// A parse dominates the cost of a run, and how many a run needs is decided
    /// by the scheduler, not by the machine. Reporting it alongside the batch
    /// count lets a test hold the scheduler to its budget without timing
    /// anything, which a shared CI runner is far too noisy to do.
    pub parses: usize,
    /// How many edit batches were accepted, one per pass that changed the file.
    pub accepted_batches: usize,
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
    let (mut current, mut fixes, prepared) = prepare_source(&mut parser, source)?;
    let mut active_diagnostics = reported.to_vec();

    let mut seen = HashSet::new();
    seen.insert(source_digest(&current));
    // The parse of the bytes the next pass will start from, when the pass that
    // produced them already proved it safe.
    let mut carried = Some(prepared);
    let mut accepted_batches = 0_usize;
    let mut completed_one_shot = BTreeSet::new();
    let ordered_phases = phases(options);
    let mut stable = false;

    for _ in 0..options.max_passes {
        let mut accepted = false;
        // The source cannot change while the phase loop runs: accepting a batch
        // is the only thing that rewrites it, and that breaks out immediately.
        // Parsing per phase therefore reparsed the same bytes once for every
        // phase, which for an already-correct file was the whole cost.
        //
        // Validating an accepted batch already parses the bytes this pass is
        // about to parse again, and that parse used to be dropped on the floor.
        // Carrying it forward halves the parses a run does, and the more a file
        // needs fixing the more passes it takes, so the saving grows exactly
        // where the cost is.
        let before = if let Some(context) = carried.take() {
            context
        } else {
            let context = ParsedContext::parse(&mut parser, &current)?;
            context.require_safe()?;
            context
        };
        for phase in &ordered_phases {
            if phase.one_shot() && completed_one_shot.contains(phase) {
                continue;
            }
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
            let after = validate_candidate(&mut parser, &before, &candidate, &batch)?;
            if !seen.insert(source_digest(&candidate)) {
                return Err(CActionError::Cycle);
            }
            active_diagnostics =
                remap_diagnostics(&current, &candidate, &active_diagnostics, &accepted_edits);
            current = candidate;
            carried = Some(after);
            fixes.extend(accepted_edits.into_iter().map(|edit| Fix {
                rule_id: edit.rule_id,
                description: edit.description,
                line: edit.line,
                applicability: batch.applicability,
            }));
            if phase.one_shot() {
                completed_one_shot.insert(*phase);
            }
            accepted_batches += 1;
            accepted = true;
            break;
        }
        if !accepted {
            // Nothing changed, so this parse still describes the final bytes.
            carried = Some(before);
            stable = true;
            break;
        }
    }

    if !stable {
        settle_or_fail(
            &mut parser,
            &current,
            &ordered_phases,
            &active_diagnostics,
            options,
        )?;
        stable = true;
    }

    let final_context = if let Some(context) = carried {
        context
    } else {
        ParsedContext::parse(&mut parser, &current)?
    };
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
        parses: parser.parses(),
        accepted_batches,
    })
}

/// Confirms a run that used its whole pass budget has nothing left to do.
///
/// Hitting the limit is only a defect when work remains: a file that needed
/// every pass and then settled is fine, while one that would still change is a
/// scheduler that does not converge.
fn settle_or_fail(
    parser: &mut CParser,
    current: &str,
    ordered_phases: &[Phase],
    active_diagnostics: &[ReportedDiagnostic],
    options: &CActionOptions,
) -> Result<(), CActionError> {
    let context = ParsedContext::parse(parser, current)?;
    if ordered_phases.iter().any(|phase| {
        phase
            .plan(&context, active_diagnostics, options)
            .ok()
            .flatten()
            .is_some()
    }) {
        return Err(CActionError::PassLimit {
            passes: options.max_passes,
        });
    }
    Ok(())
}

fn source_digest(source: &str) -> [u8; 32] {
    *blake3::hash(source.as_bytes()).as_bytes()
}

/// Normalizes hygiene and returns the parse of what it produced.
///
/// The normalized context is the parse of the exact bytes the first pass starts
/// from, so handing it back saves that pass from reading them a second time.
fn prepare_source(
    parser: &mut CParser,
    source: &str,
) -> Result<(String, Vec<Fix>, ParsedContext), CActionError> {
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
    Ok((hygiene.source, hygiene.fixes, normalized))
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

/// Proves a candidate and returns the parse that proved it.
///
/// The context is the caller's to keep: it is the parse of the exact bytes the
/// next pass works on, so returning it saves that pass from parsing them again.
fn validate_candidate(
    parser: &mut CParser,
    before: &ParsedContext,
    candidate: &str,
    batch: &ActionBatch,
) -> Result<ParsedContext, CActionError> {
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
    Ok(after)
}

#[cfg(test)]
mod tests;

/// Diagnostics describing what the parser could not read.
///
/// This lives here rather than in the engine because the browser playground
/// needs the same answer and cannot reach the engine. Without it, a reader who
/// pasted C with an unbalanced parenthesis was told "0 diagnostics" and handed
/// their file back unchanged — which reads as approval of code that does not
/// parse.
#[must_use]
pub fn syntax_recovery_diagnostics(path: &Utf8PathBuf, source: &str) -> Vec<Diagnostic> {
    let mut parser = match CParser::new() {
        Ok(parser) => parser,
        Err(error) => {
            return vec![point_diagnostic(
                path,
                "C_PARSER_FAILURE",
                Severity::Error,
                error.to_string(),
                DiagnosticSource::Parser,
                Some("Repair the source syntax before running automatic fixes.".to_owned()),
            )];
        }
    };
    match parser.parse(source) {
        Ok(parsed) => parsed
            .issues()
            .iter()
            .map(|issue| {
                let va_arg_compatibility = recovery_is_inside_va_arg(source, issue.range());
                Diagnostic {
                    rule_id: if va_arg_compatibility {
                        "C_PARSER_VA_ARG_COMPAT"
                    } else {
                        "C_SYNTAX_RECOVERY"
                    }
                    .to_owned(),
                    path: path.clone(),
                    range: issue.range(),
                    severity: if va_arg_compatibility {
                        Severity::Info
                    } else {
                        Severity::Warning
                    },
                    message: if va_arg_compatibility {
                        "The native parser preserved a raw `va_arg` type argument through its compatibility path."
                            .to_owned()
                    } else {
                        format!(
                            "The C parser recovered around syntax node `{}`.",
                            issue.syntax_kind()
                        )
                    },
                    source: DiagnosticSource::Parser,
                    notes: vec![
                        if va_arg_compatibility {
                            "This is a tree-sitter-c grammar limitation; the source bytes and official Norminette result remain authoritative."
                        } else {
                            "Automatic syntax-aware edits were disabled for this file."
                        }
                        .to_owned(),
                    ],
                    help: Some(
                        if va_arg_compatibility {
                            "No source change is required; native syntax-aware edits remain disabled for this file."
                        } else {
                            "Repair the malformed or unsupported construct, then rerun normfix."
                        }
                        .to_owned(),
                    ),
                    localized: None,
                }
            })
            .collect(),
        Err(error) => vec![point_diagnostic(
            path,
            "C_PARSER_FAILURE",
            Severity::Error,
            error.to_string(),
            DiagnosticSource::Parser,
            Some("Repair the source syntax before running automatic fixes.".to_owned()),
        )],
    }
}

/// Whether a recovery sits on a line using `va_arg`.
///
/// The tree-sitter C grammar cannot read a raw type argument, so this one
/// recovery is a grammar limitation rather than a defect in the source, and is
/// reported as information instead of a warning.
fn recovery_is_inside_va_arg(source: &str, range: TextRange) -> bool {
    let Ok(start) = usize::try_from(range.start().get()) else {
        return false;
    };
    if start > source.len() || !source.is_char_boundary(start) {
        return false;
    }
    let line_start = source[..start].rfind('\n').map_or(0, |newline| newline + 1);
    let line_end = source[start..]
        .find('\n')
        .map_or(source.len(), |newline| start + newline);
    source[line_start..line_end]
        .find("va_arg")
        .is_some_and(|offset| {
            source[line_start + offset + "va_arg".len()..line_end]
                .trim_start()
                .starts_with('(')
        })
}

/// A diagnostic with no meaningful range, anchored at the start of the file.
fn point_diagnostic(
    path: &Utf8PathBuf,
    rule_id: &str,
    severity: Severity,
    message: String,
    source: DiagnosticSource,
    help: Option<String>,
) -> Diagnostic {
    Diagnostic {
        rule_id: rule_id.to_owned(),
        path: path.clone(),
        range: TextRange::empty(TextSize::new(0)),
        severity,
        message,
        source,
        notes: Vec::new(),
        help,
        localized: None,
    }
}
