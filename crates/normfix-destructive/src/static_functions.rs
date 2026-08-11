//! Conservative project-wide planning for dead `static` functions.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use camino::{Utf8Path, Utf8PathBuf};
use normfix_c_syntax::{CFunctionFact, CFunctionKind, CParser, ParsedFile, TapePiece};
use normfix_core::{
    Applicability, Diagnostic, DiagnosticSource, FixRecord, ProofRequirement, ProofResult,
    Severity, SourceEdit, SourceSnapshot, TextRange, TextSize, apply_source_edits,
};
use thiserror::Error;

use crate::{AuthorizationError, DestructiveAuthorization, DestructiveCapability};

const RULE_REMOVE: &str = "UNSAFE_REMOVE_UNUSED_STATIC";
const RULE_AMBIGUOUS: &str = "UNSAFE_STATIC_PROOF_BLOCKED";
const RULE_PARSE: &str = "UNSAFE_STATIC_CLOSED_SET_INVALID";

/// An immutable, duplicate-free set containing every relevant C/header source.
///
/// Construction validates shape, not project discovery. The engine must call
/// [`Self::from_complete_discovery`] only after discovery has included every
/// project-local `.c` and `.h` file plus every source that can contribute
/// macros. Missing inputs invalidate the closed-world proof.
#[derive(Clone, Debug)]
pub struct ClosedCSourceSet {
    snapshots: Vec<SourceSnapshot>,
}

impl ClosedCSourceSet {
    /// Validates snapshots from a complete project discovery.
    ///
    /// # Errors
    ///
    /// Rejects an empty set, duplicate paths or identifiers, and paths whose
    /// extension is neither lowercase `.c` nor lowercase `.h`.
    pub fn from_complete_discovery(
        mut snapshots: Vec<SourceSnapshot>,
    ) -> Result<Self, ClosedSourceError> {
        if snapshots.is_empty() {
            return Err(ClosedSourceError::Empty);
        }
        snapshots.sort_by(|left, right| left.relative_path().cmp(right.relative_path()));
        let mut paths = BTreeSet::new();
        let mut ids = BTreeSet::new();
        for snapshot in &snapshots {
            let path = snapshot.relative_path().to_owned();
            if !matches!(path.extension(), Some("c" | "h")) {
                return Err(ClosedSourceError::NotCSource(path));
            }
            if !paths.insert(path.clone()) {
                return Err(ClosedSourceError::DuplicatePath(path));
            }
            if !ids.insert(snapshot.file_id()) {
                return Err(ClosedSourceError::DuplicateFileId(snapshot.file_id().get()));
            }
        }
        Ok(Self { snapshots })
    }

    /// Returns snapshots in deterministic path order.
    #[must_use]
    pub fn snapshots(&self) -> &[SourceSnapshot] {
        &self.snapshots
    }
}

/// A source collection could not represent a closed C world.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ClosedSourceError {
    /// At least one source is required.
    #[error("the closed C source set cannot be empty")]
    Empty,
    /// A source path occurred more than once.
    #[error("duplicate source path in the closed C set: {0}")]
    DuplicatePath(Utf8PathBuf),
    /// A stable file identifier occurred more than once.
    #[error("duplicate file identifier in the closed C set: {0}")]
    DuplicateFileId(u32),
    /// A snapshot was not a C source or header.
    #[error("closed C source set contains a non-C path: {0}")]
    NotCSource(Utf8PathBuf),
}

/// Proposed destructive edits and user-visible fix records for one file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DestructiveFilePlan {
    /// Project-relative source path.
    pub path: Utf8PathBuf,
    /// BLAKE3 hash of the immutable input expected by an executor.
    pub original_hash: String,
    /// Non-overlapping source deletions in stable order.
    pub edits: Vec<SourceEdit>,
    /// English summaries corresponding to accepted deletions.
    pub fixes: Vec<FixRecord>,
    /// Proof evidence collected for the complete file plan.
    pub proofs: Vec<ProofResult>,
}

/// A read-only project-wide plan for unreferenced `static` functions.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StaticRemovalPlan {
    /// Files with at least one proposed deletion.
    pub files: Vec<DestructiveFilePlan>,
    /// Reasons why a candidate or the entire closed-world proof was rejected.
    pub diagnostics: Vec<Diagnostic>,
}

/// Static function removal planning failed before analysis.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum StaticRemovalPlanError {
    /// The supplied explicit grant did not authorize this operation.
    #[error(transparent)]
    Authorization(#[from] AuthorizationError),
    /// The parser backend could not initialize.
    #[error("could not initialize the C parser: {0}")]
    ParserInitialization(String),
}

#[derive(Clone, Debug)]
struct ParsedSnapshot<'snapshot> {
    snapshot: &'snapshot SourceSnapshot,
    parsed: ParsedFile,
}

#[derive(Clone, Debug)]
struct Candidate {
    path_index: usize,
    function: CFunctionFact,
    declarations: Vec<TextRange>,
    deletion_ranges: Vec<TextRange>,
    blocked_reasons: BTreeSet<String>,
}

/// Plans removal of unreachable `static` function definitions.
///
/// The planner performs no writes. It parses every source, fails the whole
/// proof closed on recovery/unknown tape regions, builds a conservative
/// reference graph, treats uncertain preprocessor, token-paste, attribute and
/// string cases as roots, and deletes only unreachable `static` definitions.
/// Non-`static` functions are never candidates.
///
/// # Errors
///
/// Returns [`StaticRemovalPlanError::Authorization`] without the scoped
/// destructive capability, or
/// [`StaticRemovalPlanError::ParserInitialization`] if the parser backend
/// cannot be initialized.
pub fn plan_unused_static_functions(
    sources: &ClosedCSourceSet,
    authorization: &DestructiveAuthorization,
) -> Result<StaticRemovalPlan, StaticRemovalPlanError> {
    authorization.require(DestructiveCapability::RemoveUnreferencedStaticFunctions)?;
    let (parsed, mut diagnostics) = parse_closed_sources(sources)?;
    if !diagnostics.is_empty() {
        diagnostics.sort();
        return Ok(StaticRemovalPlan {
            files: Vec::new(),
            diagnostics,
        });
    }

    let (mut candidates, global_ambiguity) = collect_candidates(&parsed);
    if candidates.is_empty() {
        return Ok(StaticRemovalPlan::default());
    }
    if let Some(reason) = global_ambiguity {
        for candidate in &mut candidates {
            candidate.blocked_reasons.insert(reason.clone());
        }
    }

    let names = candidates_by_name(&candidates);
    let mut graph = vec![BTreeSet::new(); candidates.len()];
    let mut roots = BTreeSet::new();
    for (candidate_index, candidate) in candidates.iter().enumerate() {
        if !candidate.blocked_reasons.is_empty() {
            roots.insert(candidate_index);
        }
    }
    collect_token_references(&parsed, &candidates, &names, &mut graph, &mut roots);
    collect_string_ambiguities(&parsed, &mut candidates, &names, &mut roots);
    collect_preprocessor_ambiguities(&parsed, &mut candidates, &mut roots);

    let reachable = reachable_candidates(&graph, &roots);
    let dead = (0..candidates.len())
        .filter(|index| !reachable.contains(index))
        .collect::<BTreeSet<_>>();

    for (index, candidate) in candidates.iter().enumerate() {
        if !dead.contains(&index) && !candidate.blocked_reasons.is_empty() {
            let reason = candidate
                .blocked_reasons
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(" ");
            diagnostics.push(blocking_diagnostic(
                parsed[candidate.path_index].snapshot.relative_path(),
                candidate.function.name_range,
                RULE_AMBIGUOUS,
                &format!(
                    "Static function `{}` was preserved because absence of references could not be proven.",
                    candidate.function.name
                ),
                &reason,
            ));
        }
    }

    let planned_files = build_file_plans(&parsed, &candidates, dead, &mut diagnostics);
    diagnostics.sort();
    Ok(StaticRemovalPlan {
        files: planned_files,
        diagnostics,
    })
}

fn parse_closed_sources(
    sources: &ClosedCSourceSet,
) -> Result<(Vec<ParsedSnapshot<'_>>, Vec<Diagnostic>), StaticRemovalPlanError> {
    let mut parser = CParser::new()
        .map_err(|error| StaticRemovalPlanError::ParserInitialization(error.to_string()))?;
    let mut parsed = Vec::with_capacity(sources.snapshots.len());
    let mut diagnostics = Vec::new();
    for snapshot in &sources.snapshots {
        match parser.parse_arc(snapshot.text().clone()) {
            Ok(file) if file.permits_automatic_edits() && file.tape().is_lossless() => {
                parsed.push(ParsedSnapshot {
                    snapshot,
                    parsed: file,
                });
            }
            Ok(file) => {
                let range = file.issues().first().map_or_else(
                    || TextRange::empty(TextSize::new(0)),
                    normfix_c_syntax::SyntaxIssue::range,
                );
                diagnostics.push(blocking_diagnostic(
                    snapshot.relative_path(),
                    range,
                    RULE_PARSE,
                    "Destructive analysis was skipped because the closed source set contains parser recovery or unclassified bytes.",
                    "Fix every syntax/recovery issue and rerun with a complete source set.",
                ));
            }
            Err(error) => diagnostics.push(blocking_diagnostic(
                snapshot.relative_path(),
                TextRange::empty(TextSize::new(0)),
                RULE_PARSE,
                "Destructive analysis was skipped because a source could not be parsed.",
                &format!("Parser detail: {error}"),
            )),
        }
    }
    Ok((parsed, diagnostics))
}

fn build_file_plans(
    parsed: &[ParsedSnapshot<'_>],
    candidates: &[Candidate],
    dead: BTreeSet<usize>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<DestructiveFilePlan> {
    let mut by_file: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for candidate_index in dead {
        by_file
            .entry(candidates[candidate_index].path_index)
            .or_default()
            .push(candidate_index);
    }
    let mut planned_files = Vec::new();
    for (path_index, indexes) in by_file {
        if let Some(file_plan) =
            build_one_file_plan(&parsed[path_index], candidates, &indexes, diagnostics)
        {
            planned_files.push(file_plan);
        }
    }
    planned_files.sort_by(|left, right| left.path.cmp(&right.path));
    planned_files
}

fn build_one_file_plan(
    input: &ParsedSnapshot<'_>,
    candidates: &[Candidate],
    indexes: &[usize],
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<DestructiveFilePlan> {
    let mut edits = Vec::new();
    let mut fix_records = Vec::new();
    for candidate_index in indexes {
        let candidate = &candidates[*candidate_index];
        for range in &candidate.deletion_ranges {
            edits.push(SourceEdit {
                range: *range,
                replacement: String::new(),
                rule_id: RULE_REMOVE.to_owned(),
                description: format!(
                    "Remove unreachable static function `{}` and its private declaration.",
                    candidate.function.name
                ),
                applicability: Applicability::UnsafeDestructive,
            });
        }
        fix_records.push(FixRecord {
            rule_id: RULE_REMOVE.to_owned(),
            description: format!(
                "Removed unreachable static function `{}`.",
                candidate.function.name
            ),
            line: input
                .snapshot
                .line_index()
                .line_column(candidate.function.name_range.start())
                .map(|location| location.line),
            count: 1,
        });
    }
    edits.sort();
    edits.dedup();
    fix_records.sort();
    let proofs = validate_file_plan(input, &edits);
    if proofs.iter().all(|proof| proof.passed) {
        Some(DestructiveFilePlan {
            path: input.snapshot.relative_path().to_owned(),
            original_hash: input.snapshot.content_hash().to_hex().to_string(),
            edits,
            fixes: fix_records,
            proofs,
        })
    } else {
        diagnostics.push(blocking_diagnostic(
            input.snapshot.relative_path(),
            TextRange::empty(TextSize::new(0)),
            RULE_PARSE,
            "A destructive file plan was discarded because its shadow buffer failed validation.",
            "No edit from this file was emitted.",
        ));
        None
    }
}

fn collect_candidates(parsed: &[ParsedSnapshot<'_>]) -> (Vec<Candidate>, Option<String>) {
    let mut candidates = Vec::new();
    let mut token_paste = false;
    for (path_index, input) in parsed.iter().enumerate() {
        for piece in input.parsed.tape().pieces() {
            let TapePiece::Token(token) = piece else {
                continue;
            };
            if source_range(input.parsed.source(), token.range()) == Some("##") {
                token_paste = true;
            }
        }
        for definition in input
            .parsed
            .facts()
            .functions
            .iter()
            .filter(|function| function.kind == CFunctionKind::Definition && function.is_static)
        {
            let declarations = input
                .parsed
                .facts()
                .functions
                .iter()
                .filter(|function| {
                    function.kind == CFunctionKind::Prototype
                        && function.is_static
                        && function.name == definition.name
                })
                .map(|function| function.range)
                .collect::<Vec<_>>();
            let mut deletion_ranges = declarations.clone();
            deletion_ranges.push(definition.range);
            deletion_ranges = merge_expanded_ranges(input.parsed.source(), &deletion_ranges);

            let mut blocked_reasons = BTreeSet::new();
            if signature_has_ambiguous_attribute(input.parsed.source(), definition.signature_range)
            {
                blocked_reasons.insert(
                    "Its signature contains an attribute, declaration extension or assembly label."
                        .to_owned(),
                );
            }
            if declarations
                .iter()
                .any(|range| signature_has_ambiguous_attribute(input.parsed.source(), *range))
            {
                blocked_reasons.insert(
                    "A private declaration contains an attribute, declaration extension or assembly label."
                        .to_owned(),
                );
            }
            let duplicates = input
                .parsed
                .facts()
                .functions
                .iter()
                .filter(|function| {
                    function.kind == CFunctionKind::Definition && function.name == definition.name
                })
                .count();
            if duplicates != 1 {
                blocked_reasons.insert(
                    "The translation unit contains duplicate definitions with this identifier."
                        .to_owned(),
                );
            }
            candidates.push(Candidate {
                path_index,
                function: definition.clone(),
                declarations,
                deletion_ranges,
                blocked_reasons,
            });
        }
    }
    candidates.sort_by(|left, right| {
        (left.path_index, left.function.range, &left.function.name).cmp(&(
            right.path_index,
            right.function.range,
            &right.function.name,
        ))
    });
    (
        candidates,
        token_paste.then(|| {
            "Token-paste (`##`) occurs in the closed source set and may synthesize an otherwise absent identifier.".to_owned()
        }),
    )
}

fn candidates_by_name(candidates: &[Candidate]) -> BTreeMap<String, Vec<usize>> {
    let mut names: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (index, candidate) in candidates.iter().enumerate() {
        names
            .entry(candidate.function.name.clone())
            .or_default()
            .push(index);
    }
    names
}

fn collect_token_references(
    parsed: &[ParsedSnapshot<'_>],
    candidates: &[Candidate],
    names: &BTreeMap<String, Vec<usize>>,
    graph: &mut [BTreeSet<usize>],
    roots: &mut BTreeSet<usize>,
) {
    for (path_index, input) in parsed.iter().enumerate() {
        for piece in input.parsed.tape().pieces() {
            let TapePiece::Token(token) = piece else {
                continue;
            };
            if !is_identifier_kind(token.syntax_kind()) {
                continue;
            }
            let Some(text) = source_range(input.parsed.source(), token.range()) else {
                continue;
            };
            let Some(targets) = names.get(text) else {
                continue;
            };
            if token_is_declaration_name(path_index, token.range(), targets, candidates) {
                continue;
            }
            let owners = containing_candidates(path_index, token.range(), candidates);
            if owners.is_empty() {
                roots.extend(targets.iter().copied());
            } else {
                for owner in owners {
                    graph[owner].extend(targets.iter().copied());
                }
            }
        }
    }
}

fn collect_string_ambiguities(
    parsed: &[ParsedSnapshot<'_>],
    candidates: &mut [Candidate],
    names: &BTreeMap<String, Vec<usize>>,
    roots: &mut BTreeSet<usize>,
) {
    for (path_index, input) in parsed.iter().enumerate() {
        for piece in input.parsed.tape().pieces() {
            let TapePiece::Token(token) = piece else {
                continue;
            };
            if !token.syntax_kind().contains("string") {
                continue;
            }
            let Some(text) = source_range(input.parsed.source(), token.range()) else {
                continue;
            };
            for (name, indexes) in names {
                if !contains_identifier(text, name) {
                    continue;
                }
                for index in indexes {
                    if candidate_contains(*index, path_index, token.range(), candidates) {
                        continue;
                    }
                    candidates[*index].blocked_reasons.insert(
                        "Its identifier appears in a string literal outside the removable function; reflective or assembly use is ambiguous.".to_owned(),
                    );
                    roots.insert(*index);
                }
            }
        }
    }
}

fn collect_preprocessor_ambiguities(
    parsed: &[ParsedSnapshot<'_>],
    candidates: &mut [Candidate],
    roots: &mut BTreeSet<usize>,
) {
    for (index, candidate) in candidates.iter_mut().enumerate() {
        let facts = parsed[candidate.path_index].parsed.facts();
        if facts
            .preprocessor_ranges
            .iter()
            .any(|range| range.intersects(candidate.function.range))
        {
            candidate.blocked_reasons.insert(
                "The definition is controlled by a preprocessor region, so active translation-unit membership is ambiguous.".to_owned(),
            );
            roots.insert(index);
        }
        if candidate.declarations.iter().any(|declaration| {
            facts
                .preprocessor_ranges
                .iter()
                .any(|range| range.intersects(*declaration))
        }) {
            candidate
                .blocked_reasons
                .insert("A private declaration is controlled by a preprocessor region.".to_owned());
            roots.insert(index);
        }
    }
}

fn reachable_candidates(graph: &[BTreeSet<usize>], roots: &BTreeSet<usize>) -> BTreeSet<usize> {
    let mut reachable = roots.clone();
    let mut queue = roots.iter().copied().collect::<VecDeque<_>>();
    while let Some(current) = queue.pop_front() {
        for next in &graph[current] {
            if reachable.insert(*next) {
                queue.push_back(*next);
            }
        }
    }
    reachable
}

fn validate_file_plan(input: &ParsedSnapshot<'_>, edits: &[SourceEdit]) -> Vec<ProofResult> {
    let applied = apply_source_edits(input.parsed.source(), edits);
    let valid_ranges = applied.is_ok();
    let mut proofs = vec![
        ProofResult {
            requirement: ProofRequirement::DestructiveAuthorization,
            passed: true,
            detail: "The planner received a capability-scoped explicit authorization.".to_owned(),
        },
        ProofResult {
            requirement: ProofRequirement::SemanticEquivalence,
            passed: true,
            detail: "Every deleted static function is unreachable from all conservative reference roots in the complete source set.".to_owned(),
        },
        ProofResult {
            requirement: ProofRequirement::ValidRanges,
            passed: valid_ranges,
            detail: if valid_ranges {
                "Deletion ranges are valid, deterministic and non-overlapping.".to_owned()
            } else {
                "At least one deletion range was invalid or overlapping.".to_owned()
            },
        },
    ];
    let (round_trip, reparses) = match applied {
        Ok(shadow) => match CParser::new().and_then(|mut parser| parser.parse(&shadow)) {
            Ok(parsed) => (
                parsed.tape().is_lossless(),
                parsed.permits_automatic_edits(),
            ),
            Err(_) => (false, false),
        },
        Err(_) => (false, false),
    };
    proofs.push(ProofResult {
        requirement: ProofRequirement::LosslessRoundTrip,
        passed: round_trip,
        detail: if round_trip {
            "The shadow buffer has complete lossless tape coverage.".to_owned()
        } else {
            "The shadow buffer did not round-trip losslessly.".to_owned()
        },
    });
    proofs.push(ProofResult {
        requirement: ProofRequirement::NoNewSyntaxRecovery,
        passed: reparses,
        detail: if reparses {
            "The shadow buffer reparses without syntax recovery.".to_owned()
        } else {
            "The shadow buffer introduced parser recovery.".to_owned()
        },
    });
    proofs.sort();
    proofs
}

fn token_is_declaration_name(
    path_index: usize,
    range: TextRange,
    targets: &[usize],
    candidates: &[Candidate],
) -> bool {
    targets.iter().any(|index| {
        let candidate = &candidates[*index];
        candidate.path_index == path_index
            && (candidate.function.name_range == range
                || candidate.declarations.iter().any(|declaration| {
                    declaration.contains(range.start()) && declaration.contains(last_byte(range))
                }))
    })
}

fn containing_candidates(
    path_index: usize,
    range: TextRange,
    candidates: &[Candidate],
) -> Vec<usize> {
    candidates
        .iter()
        .enumerate()
        .filter(|(_, candidate)| {
            candidate.path_index == path_index
                && candidate.function.range.contains(range.start())
                && candidate.function.range.contains(last_byte(range))
        })
        .map(|(index, _)| index)
        .collect()
}

fn candidate_contains(
    index: usize,
    path_index: usize,
    range: TextRange,
    candidates: &[Candidate],
) -> bool {
    let candidate = &candidates[index];
    candidate.path_index == path_index
        && candidate.function.range.contains(range.start())
        && candidate.function.range.contains(last_byte(range))
}

fn last_byte(range: TextRange) -> TextSize {
    TextSize::new(range.end().get().saturating_sub(1))
}

fn merge_expanded_ranges(source: &str, ranges: &[TextRange]) -> Vec<TextRange> {
    let mut expanded = ranges
        .iter()
        .filter_map(|range| expand_to_whole_lines(source, *range))
        .collect::<Vec<_>>();
    expanded.sort();
    let mut merged: Vec<TextRange> = Vec::new();
    for range in expanded {
        let overlapping = merged
            .last_mut()
            .filter(|previous| previous.end().get() >= range.start().get());
        if let Some(previous) = overlapping {
            if range.end().get() <= previous.end().get() {
                continue;
            }
            if let Some(combined) = TextRange::new(previous.start(), range.end()) {
                *previous = combined;
                continue;
            }
        }
        merged.push(range);
    }
    merged
}

fn expand_to_whole_lines(source: &str, range: TextRange) -> Option<TextRange> {
    let start = usize::try_from(range.start()).ok()?;
    let end = usize::try_from(range.end()).ok()?;
    let line_start = source[..start].rfind('\n').map_or(0, |index| index + 1);
    let prefix = source.get(line_start..start)?;
    let expanded_start = if prefix.chars().all(char::is_whitespace) {
        line_start
    } else {
        start
    };
    let line_end = source[end..]
        .find('\n')
        .map_or(source.len(), |offset| end + offset + 1);
    let suffix_end = if line_end > end && source.as_bytes().get(line_end - 1) == Some(&b'\n') {
        line_end - 1
    } else {
        line_end
    };
    let suffix = source.get(end..suffix_end)?;
    let expanded_end = if suffix.chars().all(char::is_whitespace) {
        line_end
    } else {
        end
    };
    TextRange::new(
        TextSize::try_from(expanded_start).ok()?,
        TextSize::try_from(expanded_end).ok()?,
    )
}

fn signature_has_ambiguous_attribute(source: &str, range: TextRange) -> bool {
    let Some(signature) = source_range(source, range) else {
        return true;
    };
    [
        "__attribute__",
        "__attribute",
        "__declspec",
        "[[",
        "__asm",
        " asm(",
    ]
    .iter()
    .any(|marker| signature.contains(marker))
}

fn source_range(source: &str, range: TextRange) -> Option<&str> {
    source.get(usize::try_from(range.start()).ok()?..usize::try_from(range.end()).ok()?)
}

fn is_identifier_kind(kind: &str) -> bool {
    matches!(kind, "identifier" | "field_identifier" | "type_identifier")
}

fn contains_identifier(haystack: &str, needle: &str) -> bool {
    haystack.match_indices(needle).any(|(start, _)| {
        let before = haystack[..start].chars().next_back();
        let end = start + needle.len();
        let after = haystack[end..].chars().next();
        before.is_none_or(|character| !is_identifier_character(character))
            && after.is_none_or(|character| !is_identifier_character(character))
    })
}

fn is_identifier_character(character: char) -> bool {
    character == '_' || character.is_ascii_alphanumeric()
}

fn blocking_diagnostic(
    path: &Utf8Path,
    range: TextRange,
    rule_id: &str,
    message: &str,
    help: &str,
) -> Diagnostic {
    Diagnostic {
        rule_id: rule_id.to_owned(),
        path: path.to_owned(),
        range,
        severity: Severity::Warning,
        message: message.to_owned(),
        source: DiagnosticSource::Project,
        notes: vec!["No destructive edit was emitted for this candidate.".to_owned()],
        help: Some(help.to_owned()),
        localized: None,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use camino::Utf8PathBuf;
    use normfix_core::{FileId, SourceSnapshot, apply_source_edits};

    use crate::{DestructiveCapability, DestructiveRequest, EXACT_CONFIRMATION_PHRASE};

    use super::{ClosedCSourceSet, RULE_AMBIGUOUS, plan_unused_static_functions};

    fn source_set(files: &[(&str, &str)]) -> ClosedCSourceSet {
        let snapshots = files
            .iter()
            .enumerate()
            .map(|(index, (path, source))| {
                SourceSnapshot::new(
                    FileId::new(u32::try_from(index).expect("test index")),
                    Utf8PathBuf::from(path),
                    Arc::from(*source),
                )
                .expect("test snapshot")
            })
            .collect();
        ClosedCSourceSet::from_complete_discovery(snapshots).expect("closed source set")
    }

    fn authorization() -> crate::DestructiveAuthorization {
        DestructiveRequest::one(DestructiveCapability::RemoveUnreferencedStaticFunctions)
            .authorize_interactively(EXACT_CONFIRMATION_PHRASE)
            .expect("explicit authorization")
    }

    #[test]
    fn removes_dead_static_graph_and_private_prototypes_in_one_idempotent_plan() {
        let source = include_str!("../tests/fixtures/dead_graph.c");
        let set = source_set(&[("dead_graph.c", source)]);
        let plan = plan_unused_static_functions(&set, &authorization()).expect("planned");
        assert_eq!(plan.files.len(), 1);
        assert_eq!(plan.files[0].fixes.len(), 2);
        let shadow = apply_source_edits(source, &plan.files[0].edits).expect("valid edits");
        assert!(!shadow.contains("dead_leaf"));
        assert!(!shadow.contains("dead_root"));
        assert!(shadow.contains("live_leaf"));
        assert!(shadow.contains("public_api"));

        let second = source_set(&[("dead_graph.c", &shadow)]);
        let second_plan =
            plan_unused_static_functions(&second, &authorization()).expect("second plan");
        assert!(second_plan.files.is_empty(), "planning must be idempotent");
    }

    #[test]
    fn never_removes_non_static_or_referenced_static_functions() {
        let source = include_str!("../tests/fixtures/referenced.c");
        let set = source_set(&[("referenced.c", source)]);
        let plan = plan_unused_static_functions(&set, &authorization()).expect("planned");
        assert!(plan.files.is_empty());
        assert!(plan.diagnostics.is_empty());
    }

    #[test]
    fn token_paste_attribute_string_and_preprocessor_cases_fail_closed() {
        for (path, source) in [
            ("paste.c", include_str!("../tests/fixtures/token_paste.c")),
            ("attribute.c", include_str!("../tests/fixtures/attribute.c")),
            (
                "string.c",
                include_str!("../tests/fixtures/string_reference.c"),
            ),
            (
                "conditional.c",
                include_str!("../tests/fixtures/preprocessor.c"),
            ),
        ] {
            let set = source_set(&[(path, source)]);
            let plan = plan_unused_static_functions(&set, &authorization()).expect("planned");
            assert!(plan.files.is_empty(), "{path} unexpectedly produced edits");
            assert!(
                plan.diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.rule_id == RULE_AMBIGUOUS),
                "{path} should explain why proof was blocked"
            );
        }
    }

    #[test]
    fn syntax_recovery_blocks_every_destructive_edit() {
        let set = source_set(&[
            ("good.c", "static void dead(void) {}\n"),
            ("broken.c", "int broken( {\n"),
        ]);
        let plan = plan_unused_static_functions(&set, &authorization()).expect("planned");
        assert!(plan.files.is_empty());
        assert!(plan.diagnostics.iter().any(|diagnostic| {
            diagnostic.rule_id == super::RULE_PARSE && diagnostic.path == "broken.c"
        }));
    }
}
