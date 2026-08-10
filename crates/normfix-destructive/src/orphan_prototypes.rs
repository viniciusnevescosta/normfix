//! Conservative project-wide analysis of header prototypes without implementations.

use std::collections::{BTreeMap, BTreeSet};

use camino::Utf8Path;
use normfix_c_syntax::{CFunctionFact, CFunctionKind, CParser, ParsedFile, TapePiece};
use normfix_core::{
    Applicability, Diagnostic, DiagnosticSource, FixRecord, ProofRequirement, ProofResult,
    Severity, SourceEdit, TextRange, TextSize, apply_source_edits,
};
use thiserror::Error;

use crate::{
    AuthorizationError, ClosedCSourceSet, DestructiveAuthorization, DestructiveCapability,
    DestructiveFilePlan,
};

const RULE_MISSING: &str = "HEADER_PROTOTYPE_IMPLEMENTATION_MISSING";
const RULE_EMPTY: &str = "HEADER_PROTOTYPE_IMPLEMENTATION_EMPTY";
const RULE_REMOVE: &str = "UNSAFE_REMOVE_ORPHAN_PROTOTYPE";
const RULE_BLOCKED: &str = "UNSAFE_ORPHAN_PROTOTYPE_PROOF_BLOCKED";
const RULE_PARSE: &str = "ORPHAN_PROTOTYPE_CLOSED_SET_INVALID";

/// Warnings and optional explicitly authorized removals for orphan prototypes.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct OrphanPrototypePlan {
    /// Header files with one or more validated deletion edits.
    pub files: Vec<DestructiveFilePlan>,
    /// Missing implementation warnings or reasons an unsafe removal was refused.
    pub diagnostics: Vec<Diagnostic>,
}

/// Orphan-prototype planning failed before source analysis.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum OrphanPrototypePlanError {
    /// The supplied grant did not authorize prototype removal.
    #[error(transparent)]
    Authorization(#[from] AuthorizationError),
    /// The C parser backend could not initialize.
    #[error("could not initialize the C parser: {0}")]
    ParserInitialization(String),
}

#[derive(Clone, Debug)]
struct ParsedSnapshot<'snapshot> {
    snapshot: &'snapshot normfix_core::SourceSnapshot,
    parsed: ParsedFile,
}

#[derive(Clone, Debug)]
struct Candidate {
    path_index: usize,
    function: CFunctionFact,
}

/// Finds non-static prototypes in project headers with no project definition.
///
/// Passing `None` is a read-only analysis. Passing a capability-scoped grant
/// additionally removes a missing prototype only when the complete lossless
/// source set contains no call, pointer/reference, macro, string, token-paste,
/// conditional-preprocessor, or attribute evidence for that identifier. This
/// is deliberately stricter than merely observing that no definition exists.
///
/// # Errors
///
/// Returns an authorization error for a grant without the exact capability,
/// or an initialization error when the parser backend is unavailable. Syntax
/// recovery is represented as diagnostics and suppresses every deletion.
// Keeping candidate collection, ambiguity classification, and diagnostic
// construction in one visible sequence makes this destructive proof auditable.
#[allow(clippy::too_many_lines)]
pub fn plan_orphan_prototypes(
    sources: &ClosedCSourceSet,
    authorization: Option<&DestructiveAuthorization>,
) -> Result<OrphanPrototypePlan, OrphanPrototypePlanError> {
    if let Some(authorization) = authorization {
        authorization.require(DestructiveCapability::RemoveOrphanPrototypes)?;
    }
    let (parsed, mut diagnostics) = parse_closed_sources(sources)?;
    if !diagnostics.is_empty() {
        diagnostics.sort();
        return Ok(OrphanPrototypePlan {
            files: Vec::new(),
            diagnostics,
        });
    }

    let mut definitions = BTreeMap::<String, Vec<(usize, CFunctionFact)>>::new();
    for (path_index, input) in parsed.iter().enumerate() {
        for function in
            input.parsed.facts().functions.iter().filter(|function| {
                function.kind == CFunctionKind::Definition && !function.is_static
            })
        {
            definitions
                .entry(function.name.clone())
                .or_default()
                .push((path_index, function.clone()));
        }
    }
    let candidates = parsed
        .iter()
        .enumerate()
        .filter(|(_, input)| input.snapshot.relative_path().extension() == Some("h"))
        .flat_map(|(path_index, input)| {
            input
                .parsed
                .facts()
                .functions
                .iter()
                .filter(|function| {
                    function.kind == CFunctionKind::Prototype
                        && !function.is_static
                        && !definitions.contains_key(&function.name)
                })
                .cloned()
                .map(move |function| Candidate {
                    path_index,
                    function,
                })
        })
        .collect::<Vec<_>>();
    diagnostics.extend(empty_implementation_diagnostics(&parsed, &definitions));
    if candidates.is_empty() {
        diagnostics.sort();
        return Ok(OrphanPrototypePlan {
            files: Vec::new(),
            diagnostics,
        });
    }

    let mut by_name = BTreeMap::<String, Vec<usize>>::new();
    for (index, candidate) in candidates.iter().enumerate() {
        by_name
            .entry(candidate.function.name.clone())
            .or_default()
            .push(index);
    }
    let token_paste = parsed.iter().any(|input| {
        input.parsed.tape().pieces().iter().any(|piece| {
            let TapePiece::Token(token) = piece else {
                return false;
            };
            source_range(input.parsed.source(), token.range()) == Some("##")
        })
    });
    let mut removable = BTreeSet::new();
    let mut blocked = BTreeMap::<usize, String>::new();
    if authorization.is_some() {
        for (name, indexes) in &by_name {
            let reason = removal_blocker(name, indexes, &candidates, &parsed, token_paste);
            if let Some(reason) = reason {
                for index in indexes {
                    blocked.insert(*index, reason.clone());
                }
            } else {
                removable.extend(indexes.iter().copied());
            }
        }
    }

    for (index, candidate) in candidates.iter().enumerate() {
        if removable.contains(&index) {
            continue;
        }
        let input = &parsed[candidate.path_index];
        let mut notes = vec![
            "The complete lossless project source set contains no non-static definition with this identifier. Generated sources and external libraries are not inferred."
                .to_owned(),
        ];
        let (rule_id, help) = if let Some(reason) = blocked.get(&index) {
            notes.push(format!("Unsafe removal was refused: {reason}"));
            (
                RULE_BLOCKED,
                "Implement the function, remove the prototype manually after checking the subject, or eliminate every ambiguous use before retrying unsafe mode.",
            )
        } else {
            (
                RULE_MISSING,
                "Implement the declared function, or run explicitly authorized unsafe mode only if this project-local API is intentionally unused.",
            )
        };
        diagnostics.push(Diagnostic {
            rule_id: rule_id.to_owned(),
            path: input.snapshot.relative_path().to_owned(),
            range: candidate.function.name_range,
            severity: Severity::Warning,
            message: format!(
                "Header prototype `{}` has no implementation in the project source set.",
                candidate.function.name
            ),
            source: DiagnosticSource::Project,
            notes,
            help: Some(help.to_owned()),
        });
    }

    let files = build_file_plans(&parsed, &candidates, &removable, &mut diagnostics);
    diagnostics.sort();
    Ok(OrphanPrototypePlan { files, diagnostics })
}

fn parse_closed_sources(
    sources: &ClosedCSourceSet,
) -> Result<(Vec<ParsedSnapshot<'_>>, Vec<Diagnostic>), OrphanPrototypePlanError> {
    let mut parser = CParser::new()
        .map_err(|error| OrphanPrototypePlanError::ParserInitialization(error.to_string()))?;
    let mut parsed = Vec::with_capacity(sources.snapshots().len());
    let mut diagnostics = Vec::new();
    for snapshot in sources.snapshots() {
        match parser.parse_arc(snapshot.text().clone()) {
            Ok(file) if file.permits_automatic_edits() && file.tape().is_lossless() => {
                parsed.push(ParsedSnapshot {
                    snapshot,
                    parsed: file,
                });
            }
            Ok(file) => diagnostics.push(blocking_diagnostic(
                snapshot.relative_path(),
                file.issues().first().map_or_else(
                    || TextRange::empty(TextSize::new(0)),
                    normfix_c_syntax::SyntaxIssue::range,
                ),
                RULE_PARSE,
                "Prototype implementation analysis was skipped because the closed source set contains parser recovery or unclassified bytes.",
                "Fix every syntax/recovery issue and rerun preflight.",
            )),
            Err(error) => diagnostics.push(blocking_diagnostic(
                snapshot.relative_path(),
                TextRange::empty(TextSize::new(0)),
                RULE_PARSE,
                "Prototype implementation analysis was skipped because a source could not be parsed.",
                &format!("Parser detail: {error}"),
            )),
        }
    }
    Ok((parsed, diagnostics))
}

fn removal_blocker(
    name: &str,
    indexes: &[usize],
    candidates: &[Candidate],
    parsed: &[ParsedSnapshot<'_>],
    token_paste: bool,
) -> Option<String> {
    if token_paste {
        return Some("token-paste (`##`) may synthesize an otherwise absent identifier".to_owned());
    }
    for index in indexes {
        let candidate = &candidates[*index];
        let input = &parsed[candidate.path_index];
        let enclosing_preprocessor = input
            .parsed
            .facts()
            .preprocessor_ranges
            .iter()
            .filter(|range| range.intersects(candidate.function.range))
            .count();
        if enclosing_preprocessor > 0
            && !(enclosing_preprocessor == 1
                && canonical_header_guard_encloses(input.parsed.source(), candidate.function.range))
        {
            return Some("the prototype is controlled by a preprocessor region".to_owned());
        }
        if signature_has_ambiguous_attribute(
            input.parsed.source(),
            candidate.function.signature_range,
        ) {
            return Some(
                "the declaration contains an attribute, extension, or assembly label".to_owned(),
            );
        }
    }
    for (path_index, input) in parsed.iter().enumerate() {
        for piece in input.parsed.tape().pieces() {
            let TapePiece::Token(token) = piece else {
                continue;
            };
            let Some(text) = source_range(input.parsed.source(), token.range()) else {
                return Some("a token range could not be recovered losslessly".to_owned());
            };
            if text == name && is_identifier_kind(token.syntax_kind()) {
                let declaration_name = indexes.iter().any(|index| {
                    let candidate = &candidates[*index];
                    candidate.path_index == path_index
                        && candidate.function.name_range == token.range()
                });
                if !declaration_name {
                    return Some(
                        "the identifier is referenced outside its removable prototype".to_owned(),
                    );
                }
            }
            if token.syntax_kind().contains("string") && contains_identifier(text, name) {
                return Some(
                    "the identifier appears in a string literal and reflective use is ambiguous"
                        .to_owned(),
                );
            }
        }
    }
    None
}

fn canonical_header_guard_encloses(source: &str, declaration: TextRange) -> bool {
    let mut offset = 0_usize;
    let mut directives = Vec::<(usize, usize, &str)>::new();
    for line in source.split_inclusive('\n') {
        let content = line.trim_end_matches(['\r', '\n']);
        let trimmed = content.trim();
        if trimmed.starts_with('#') {
            directives.push((offset, offset + line.len(), trimmed));
        }
        offset += line.len();
    }
    if offset < source.len() {
        let trimmed = source[offset..].trim();
        if trimmed.starts_with('#') {
            directives.push((offset, source.len(), trimmed));
        }
    }
    let Some((_, ifndef_end, ifndef)) = directives.first().copied() else {
        return false;
    };
    let Some((_, define_end, define)) = directives.get(1).copied() else {
        return false;
    };
    let Some((endif_start, _, endif)) = directives.last().copied() else {
        return false;
    };
    if directives.len() < 3 || !endif.starts_with("#endif") {
        return false;
    }
    if directives[2..directives.len() - 1]
        .iter()
        .map(|(_, _, directive)| directive.trim_start_matches('#').trim_start())
        .any(|directive| {
            ["if", "ifdef", "ifndef", "elif", "else", "endif"]
                .iter()
                .any(|keyword| {
                    directive == *keyword
                        || directive
                            .strip_prefix(keyword)
                            .is_some_and(|suffix| suffix.starts_with(char::is_whitespace))
                })
        })
    {
        return false;
    }
    let Some(guard) = ifndef.strip_prefix("#ifndef").map(str::trim) else {
        return false;
    };
    let Some(defined) = define.strip_prefix('#').map(str::trim) else {
        return false;
    };
    let Some(defined) = defined.strip_prefix("define").map(str::trim) else {
        return false;
    };
    let definition = defined.split_ascii_whitespace().next().unwrap_or_default();
    guard == definition
        && is_c_identifier(guard)
        && ifndef_end <= define_end
        && usize::try_from(declaration.start()).is_ok_and(|start| define_end <= start)
        && usize::try_from(declaration.end()).is_ok_and(|end| end <= endif_start)
}

fn empty_implementation_diagnostics(
    parsed: &[ParsedSnapshot<'_>],
    definitions: &BTreeMap<String, Vec<(usize, CFunctionFact)>>,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for header in parsed
        .iter()
        .filter(|input| input.snapshot.relative_path().extension() == Some("h"))
    {
        for prototype in header
            .parsed
            .facts()
            .functions
            .iter()
            .filter(|function| function.kind == CFunctionKind::Prototype && !function.is_static)
        {
            let Some(implementations) = definitions.get(&prototype.name) else {
                continue;
            };
            for (path_index, implementation) in implementations {
                if !definition_body_is_trivia_only(&parsed[*path_index].parsed, implementation) {
                    continue;
                }
                let implementation_path = parsed[*path_index].snapshot.relative_path();
                let line = parsed[*path_index]
                    .snapshot
                    .line_index()
                    .line_column(implementation.name_range.start())
                    .map(|location| location.line);
                diagnostics.push(Diagnostic {
                    rule_id: RULE_EMPTY.to_owned(),
                    path: header.snapshot.relative_path().to_owned(),
                    range: prototype.name_range,
                    severity: Severity::Warning,
                    message: format!(
                        "Header prototype `{}` resolves to a trivia-only implementation body.",
                        prototype.name
                    ),
                    source: DiagnosticSource::Project,
                    notes: vec![format!(
                        "Implementation: {}{}; an intentional no-op may be valid, so normfix does not remove it automatically.",
                        implementation_path,
                        line.map_or_else(String::new, |line| format!(":{line}"))
                    )],
                    help: Some(
                        "Implement the required behavior, or verify against the subject that this no-op API is intentional."
                            .to_owned(),
                    ),
                });
            }
        }
    }
    diagnostics
}

fn definition_body_is_trivia_only(parsed: &ParsedFile, function: &CFunctionFact) -> bool {
    let Some(body) = function.body_range else {
        return false;
    };
    parsed.tape().pieces().iter().all(|piece| {
        if !body.intersects(piece.range()) {
            return true;
        }
        match piece {
            TapePiece::Trivia(_) => true,
            TapePiece::Token(token) => source_range(parsed.source(), token.range())
                .is_some_and(|text| matches!(text, "{" | "}")),
            TapePiece::Unknown(_) => false,
        }
    })
}

fn is_c_identifier(text: &str) -> bool {
    let mut characters = text.chars();
    characters
        .next()
        .is_some_and(|character| character == '_' || character.is_ascii_alphabetic())
        && characters.all(is_identifier_character)
}

fn build_file_plans(
    parsed: &[ParsedSnapshot<'_>],
    candidates: &[Candidate],
    removable: &BTreeSet<usize>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<DestructiveFilePlan> {
    let mut by_file = BTreeMap::<usize, Vec<usize>>::new();
    for index in removable {
        by_file
            .entry(candidates[*index].path_index)
            .or_default()
            .push(*index);
    }
    let mut plans = Vec::new();
    for (path_index, indexes) in by_file {
        let input = &parsed[path_index];
        let mut edits = indexes
            .iter()
            .filter_map(|index| {
                let candidate = &candidates[*index];
                expand_to_whole_lines(input.parsed.source(), candidate.function.range).map(
                    |range| SourceEdit {
                        range,
                        replacement: String::new(),
                        rule_id: RULE_REMOVE.to_owned(),
                        description: format!(
                            "Remove unused prototype `{}` with no project implementation.",
                            candidate.function.name
                        ),
                        applicability: Applicability::UnsafeDestructive,
                    },
                )
            })
            .collect::<Vec<_>>();
        edits.sort();
        edits.dedup();
        let fixes = indexes
            .iter()
            .map(|index| {
                let candidate = &candidates[*index];
                FixRecord {
                    rule_id: RULE_REMOVE.to_owned(),
                    description: format!(
                        "Removed unused prototype `{}` with no project implementation.",
                        candidate.function.name
                    ),
                    line: input
                        .snapshot
                        .line_index()
                        .line_column(candidate.function.name_range.start())
                        .map(|location| location.line),
                    count: 1,
                }
            })
            .collect::<Vec<_>>();
        let proofs = validate_file_plan(input, &edits);
        if !edits.is_empty() && proofs.iter().all(|proof| proof.passed) {
            plans.push(DestructiveFilePlan {
                path: input.snapshot.relative_path().to_owned(),
                original_hash: input.snapshot.content_hash().to_hex().to_string(),
                edits,
                fixes,
                proofs,
            });
        } else {
            diagnostics.push(blocking_diagnostic(
                input.snapshot.relative_path(),
                TextRange::empty(TextSize::new(0)),
                RULE_PARSE,
                "An orphan-prototype deletion plan failed shadow-buffer validation.",
                "No prototype from this header was removed.",
            ));
        }
    }
    plans.sort_by(|left, right| left.path.cmp(&right.path));
    plans
}

fn validate_file_plan(input: &ParsedSnapshot<'_>, edits: &[SourceEdit]) -> Vec<ProofResult> {
    let applied = apply_source_edits(input.parsed.source(), edits);
    let valid_ranges = applied.is_ok();
    let (lossless, recovery_free) = match applied {
        Ok(shadow) => match CParser::new().and_then(|mut parser| parser.parse(&shadow)) {
            Ok(parsed) => (
                parsed.tape().is_lossless(),
                parsed.permits_automatic_edits(),
            ),
            Err(_) => (false, false),
        },
        Err(_) => (false, false),
    };
    vec![
        ProofResult {
            requirement: ProofRequirement::DestructiveAuthorization,
            passed: true,
            detail: "A capability-scoped explicit authorization was supplied.".to_owned(),
        },
        ProofResult {
            requirement: ProofRequirement::SemanticEquivalence,
            passed: true,
            detail: "No definition or identifier use exists in the complete lossless project source set. Public API removal remains explicitly destructive."
                .to_owned(),
        },
        ProofResult {
            requirement: ProofRequirement::ValidRanges,
            passed: valid_ranges,
            detail: "Deletion ranges must be deterministic and non-overlapping.".to_owned(),
        },
        ProofResult {
            requirement: ProofRequirement::LosslessRoundTrip,
            passed: lossless,
            detail: "The edited header must retain complete lossless tape coverage.".to_owned(),
        },
        ProofResult {
            requirement: ProofRequirement::NoNewSyntaxRecovery,
            passed: recovery_free,
            detail: "The edited header must reparse without syntax recovery.".to_owned(),
        },
    ]
}

fn expand_to_whole_lines(source: &str, range: TextRange) -> Option<TextRange> {
    let start = usize::try_from(range.start()).ok()?;
    let end = usize::try_from(range.end()).ok()?;
    let line_start = source[..start].rfind('\n').map_or(0, |index| index + 1);
    if !source
        .get(line_start..start)?
        .chars()
        .all(char::is_whitespace)
    {
        return None;
    }
    let line_end = source[end..]
        .find('\n')
        .map_or(source.len(), |offset| end + offset + 1);
    let content_end = line_end.checked_sub(usize::from(
        source.as_bytes().get(line_end.wrapping_sub(1)) == Some(&b'\n'),
    ))?;
    if !source
        .get(end..content_end)?
        .chars()
        .all(char::is_whitespace)
    {
        return None;
    }
    TextRange::new(
        TextSize::try_from(line_start).ok()?,
        TextSize::try_from(line_end).ok()?,
    )
}

fn signature_has_ambiguous_attribute(source: &str, range: TextRange) -> bool {
    source_range(source, range).is_none_or(|signature| {
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
    })
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
        notes: vec!["No destructive prototype edit was emitted.".to_owned()],
        help: Some(help.to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use camino::Utf8PathBuf;
    use normfix_core::{FileId, SourceSnapshot, apply_source_edits};

    use crate::{
        ClosedCSourceSet, DestructiveCapability, DestructiveRequest, EXACT_CONFIRMATION_PHRASE,
    };

    use super::plan_orphan_prototypes;

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
                .expect("snapshot")
            })
            .collect();
        ClosedCSourceSet::from_complete_discovery(snapshots).expect("closed set")
    }

    fn authorization() -> crate::DestructiveAuthorization {
        DestructiveRequest::one(DestructiveCapability::RemoveOrphanPrototypes)
            .authorize_interactively(EXACT_CONFIRMATION_PHRASE)
            .expect("authorization")
    }

    #[test]
    fn reports_a_header_prototype_at_its_identifier() {
        let header = "int\tmissing(void);\n";
        let plan =
            plan_orphan_prototypes(&source_set(&[("api.h", header)]), None).expect("analysis");

        assert!(plan.files.is_empty());
        assert_eq!(plan.diagnostics.len(), 1);
        assert_eq!(plan.diagnostics[0].rule_id, super::RULE_MISSING);
        let range = plan.diagnostics[0].range;
        assert_eq!(
            &header[range.start().get() as usize..range.end().get() as usize],
            "missing"
        );
    }

    #[test]
    fn matching_project_definition_satisfies_the_prototype() {
        let set = source_set(&[
            ("api.h", "int\tanswer(void);\n"),
            ("answer.c", "int\tanswer(void)\n{\n\treturn (42);\n}\n"),
        ]);
        let plan = plan_orphan_prototypes(&set, None).expect("analysis");

        assert!(plan.files.is_empty());
        assert!(plan.diagnostics.is_empty());
    }

    #[test]
    fn function_typedefs_are_never_orphan_prototype_candidates() {
        let header = concat!(
            "typedef int\tt_callback(void);\n",
            "typedef int\t(*t_callback_pointer)(void);\n",
        );
        let plan =
            plan_orphan_prototypes(&source_set(&[("api.h", header)]), Some(&authorization()))
                .expect("typedef analysis");

        assert!(plan.files.is_empty());
        assert!(plan.diagnostics.is_empty());
    }

    #[test]
    fn reports_a_trivia_only_definition_without_removing_its_public_prototype() {
        let header = "void\tplaceholder(void);\n";
        let set = source_set(&[
            ("api.h", header),
            (
                "placeholder.c",
                "void\tplaceholder(void)\n{\n\t/* TODO: implement */\n}\n",
            ),
        ]);
        let plan =
            plan_orphan_prototypes(&set, Some(&authorization())).expect("empty implementation");

        assert!(plan.files.is_empty());
        assert_eq!(plan.diagnostics.len(), 1);
        assert_eq!(plan.diagnostics[0].rule_id, super::RULE_EMPTY);
        let range = plan.diagnostics[0].range;
        assert_eq!(
            &header[range.start().get() as usize..range.end().get() as usize],
            "placeholder"
        );
        assert!(plan.diagnostics[0].notes[0].contains("placeholder.c:1"));
    }

    #[test]
    fn unsafe_mode_removes_only_an_entire_unused_prototype_line() {
        let header = "#ifndef API_H\n# define API_H\n\nint\tmissing(void);\n\n#endif\n";
        let set = source_set(&[("api.h", header)]);
        let plan = plan_orphan_prototypes(&set, Some(&authorization())).expect("plan");

        assert!(plan.diagnostics.is_empty());
        assert_eq!(plan.files.len(), 1);
        let shadow = apply_source_edits(header, &plan.files[0].edits).expect("edit");
        assert!(!shadow.contains("missing"));
        assert!(shadow.contains("# define API_H"));
    }

    #[test]
    fn a_call_or_function_pointer_reference_blocks_removal() {
        for source in [
            "int\tmissing(void);\nint\tmain(void)\n{\n\treturn (missing());\n}\n",
            "int\tmissing(void);\nint\t(*callback)(void) = missing;\n",
        ] {
            let set = source_set(&[("api.h", "int\tmissing(void);\n"), ("main.c", source)]);
            let plan = plan_orphan_prototypes(&set, Some(&authorization())).expect("plan");
            assert!(plan.files.is_empty());
            assert!(
                plan.diagnostics
                    .iter()
                    .any(|item| item.rule_id == super::RULE_BLOCKED)
            );
        }
    }

    #[test]
    fn syntax_recovery_suppresses_every_orphan_conclusion() {
        let set = source_set(&[
            ("api.h", "int\tmissing(void);\n"),
            ("broken.c", "int broken( {\n"),
        ]);
        let plan = plan_orphan_prototypes(&set, None).expect("analysis");

        assert!(plan.files.is_empty());
        assert!(
            plan.diagnostics
                .iter()
                .all(|item| item.rule_id == super::RULE_PARSE)
        );
    }
}
