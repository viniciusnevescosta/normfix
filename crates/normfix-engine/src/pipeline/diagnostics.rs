//! Diagnostic construction, ranges, and the official/native merge.
//!
//! The two authorities disagree about what a column is: the official checker
//! counts display columns, a C compiler counts bytes. Both conventions are
//! resolved here so a caret always lands on the character the rule is about.

use std::collections::{BTreeMap, BTreeSet};

use camino::Utf8PathBuf;
use normfix_c_semantics::{ArrayBoundKind, analyze as analyze_semantics};
use normfix_c_syntax::CParser;
use normfix_core::{Diagnostic, DiagnosticSource, Localized, Severity, TextRange, TextSize};
use normfix_header::ByteRange;
use normfix_i18n::{DiagnosticKey, DiagnosticText, Locale, diagnostic_text, fill};
use normfix_oracle::NorminetteDiagnostic;
use normfix_project::DiscoveredFile;

use super::OracleContext;

/// A diagnostic's text, ready to spread into a `Diagnostic`.
pub(super) struct Text {
    /// English summary. This is what reaches JSON and what equality uses.
    pub message: String,
    /// English context lines.
    pub notes: Vec<String>,
    /// English next step.
    pub help: Option<String>,
    /// The same three in the reader's language, when it is not English.
    pub localized: Option<Localized>,
}

/// Renders a diagnostic in English and, when the reader's language differs,
/// in that language too.
///
/// English is always produced: it is what reaches JSON, what equality and
/// ordering use, and the only text when the reader reads English.
pub(super) fn localized_text(
    locale: Locale,
    key: DiagnosticKey,
    arguments: &[(&str, &str)],
) -> Text {
    let render = |text: DiagnosticText| {
        (
            fill(text.message, arguments),
            text.notes
                .iter()
                .map(|note| fill(note, arguments))
                .collect::<Vec<_>>(),
            fill(text.help, arguments),
        )
    };
    let (message, notes, help) = render(diagnostic_text(Locale::English, key));
    let localized = (locale != Locale::English).then(|| {
        let (message, notes, help) = render(diagnostic_text(locale, key));
        Localized {
            message,
            notes,
            help: Some(help),
        }
    });
    Text {
        message,
        notes,
        help: Some(help),
        localized,
    }
}

pub(super) fn project_diagnostic(
    path: Utf8PathBuf,
    rule_id: &str,
    message: &str,
    help: &str,
) -> Diagnostic {
    Diagnostic {
        rule_id: rule_id.to_owned(),
        path,
        range: TextRange::empty(TextSize::new(0)),
        severity: Severity::Warning,
        message: message.to_owned(),
        source: DiagnosticSource::Project,
        notes: Vec::new(),
        help: Some(help.to_owned()),
        localized: None,
    }
}

pub(super) fn text_range(range: ByteRange) -> TextRange {
    let start = u32::try_from(range.start).unwrap_or(u32::MAX);
    let end = u32::try_from(range.end).unwrap_or(u32::MAX).max(start);
    TextRange::new(TextSize::new(start), TextSize::new(end))
        .unwrap_or_else(|| TextRange::empty(TextSize::new(start)))
}

pub(super) fn line_for_offset(source: &str, offset: usize) -> Option<u32> {
    if offset > source.len() || !source.is_char_boundary(offset) {
        return None;
    }
    u32::try_from(
        source[..offset]
            .bytes()
            .filter(|byte| *byte == b'\n')
            .count()
            + 1,
    )
    .ok()
}

pub(super) fn official_diagnostics(
    path: &Utf8PathBuf,
    source: &str,
    diagnostics: &[NorminetteDiagnostic],
    norminette_version: &str,
) -> Vec<Diagnostic> {
    diagnostics
        .iter()
        .map(|item| Diagnostic {
            rule_id: item.rule_id.clone(),
            path: path.clone(),
            range: diagnostic_range(source, item.line, item.column, ColumnUnit::Display),
            // A `Notice:` is the checker asking the student to confirm a
            // deliberate choice, not reporting a broken rule: a file whose only
            // remark is a notice is one the official checker calls OK. Ranking
            // it with the violations would inflate what is left to do and hide
            // the difference the checker took care to draw.
            severity: if item.advisory {
                Severity::Info
            } else {
                Severity::Warning
            },
            message: item.message.clone(),
            source: DiagnosticSource::NorminetteCompat(norminette_version.to_owned()),
            notes: Vec::new(),
            help: Some(diagnostic_help(&item.rule_id).to_owned()),
            localized: None,
        })
        .collect()
}

/// Merges official findings without allowing one native rule occurrence to
/// hide other official occurrences of that rule at distinct source locations.
pub(super) fn merge_official_diagnostics(
    diagnostics: &mut Vec<Diagnostic>,
    official: Vec<Diagnostic>,
    corroborate_native: bool,
    norminette_version: &str,
) {
    let represented = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.source == DiagnosticSource::NativeNorm41)
        .filter(|diagnostic| {
            official.iter().any(|candidate| {
                candidate.rule_id == diagnostic.rule_id
                    && candidate.range.start() == diagnostic.range.start()
            })
        })
        .map(|diagnostic| (diagnostic.rule_id.clone(), diagnostic.range.start()))
        .collect::<BTreeSet<_>>();
    if corroborate_native {
        for diagnostic in diagnostics.iter_mut().filter(|diagnostic| {
            represented.contains(&(diagnostic.rule_id.clone(), diagnostic.range.start()))
        }) {
            diagnostic.source = DiagnosticSource::NorminetteCompat(norminette_version.to_owned());
        }
    }
    diagnostics.extend(official.into_iter().filter(|diagnostic| {
        !represented.contains(&(diagnostic.rule_id.clone(), diagnostic.range.start()))
    }));
}

pub(super) fn diagnostic_help(rule_id: &str) -> &'static str {
    match rule_id {
        "TOO_MANY_LINES" => "Extract one coherent responsibility into a well-named static helper.",
        "TOO_MANY_ARGS" => {
            "Reduce the function contract to four parameters or group genuinely related state."
        }
        "TOO_MANY_VARS_FUNC" => {
            "Split the responsibility or simplify the local declaration block to five variables."
        }
        "TOO_MANY_FUNCS" => {
            "Move a cohesive group of functions to another .c file and update interfaces and the Makefile."
        }
        "LINE_TOO_LONG" => "Shorten a literal/comment manually when no token-safe break exists.",
        "VLA_FORBIDDEN" => {
            "Use a proven integer constant expression or an allowed dynamic-allocation strategy."
        }
        "WRONG_SCOPE_COMMENT" | "COMMENT_ON_INSTR" => {
            "Move the comment to an allowed scope, or rerun with --remove-invalid-comments to delete this exact rejected comment."
        }
        "INVALID_HEADER" => {
            "Configure a verified 42 student email so the official header can be inserted."
        }
        "HEADER_PROT_NAME" | "HEADER_PROT_NODEF" => {
            "Use one canonical filename-derived #ifndef/#define guard around the whole header."
        }
        "MISALIGNED_FUNC_DECL" => {
            "Align this prototype with the complete simple declaration group."
        }
        "MISALIGNED_VAR_DECL" => {
            "Align this declarator with the complete simple declaration group."
        }
        // Everything else falls through to the shared rule catalogue, so a rule
        // the native analysis can advise on is advised on identically when the
        // official checker is the one that reported it. The arms above stay
        // here because they name normfix's own flags and concepts, which the
        // rule-level catalogue has no business knowing about.
        other => normfix_c_actions::rule_guidance(other).unwrap_or(
            "Review this location and apply the named Norm rule manually; no semantics-preserving automatic edit was proven.",
        ),
    }
}

/// The unit a reported column is expressed in.
///
/// The two authorities disagree, and the disagreement is invisible until a line
/// is indented with tabs. The official Norminette counts display columns, so a
/// tab advances to the next four-column tab stop. A C compiler counts bytes, so
/// a tab is one column. Reading one convention as the other puts the caret on
/// the wrong character of every indented line, which is most lines of a 42
/// project.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ColumnUnit {
    /// Tab-expanded display column, as the official Norminette reports.
    Display,
    /// One-based byte offset within the physical line, as a C compiler reports.
    Byte,
}

pub(super) fn diagnostic_range(
    source: &str,
    line: u32,
    column: u32,
    unit: ColumnUnit,
) -> TextRange {
    let start = offset_for_line_column(source, line, column, unit);
    let bytes = source.as_bytes();
    let mut end = start;
    if let Some(byte) = bytes.get(start) {
        if *byte == b'_' || byte.is_ascii_alphanumeric() {
            while bytes
                .get(end)
                .is_some_and(|byte| *byte == b'_' || byte.is_ascii_alphanumeric())
            {
                end += 1;
            }
        } else if !byte.is_ascii_whitespace() {
            end += 1;
            while end < source.len() && !source.is_char_boundary(end) {
                end += 1;
            }
        }
    }
    compact_range(start, end)
}

pub(super) fn offset_for_line_column(
    source: &str,
    line: u32,
    column: u32,
    unit: ColumnUnit,
) -> usize {
    let target_line = line.max(1);
    let target_column = column.max(1);
    let mut current_line = 1_u32;
    let mut line_start = 0_usize;
    for (index, byte) in source.bytes().enumerate() {
        if current_line == target_line {
            break;
        }
        if byte == b'\n' {
            current_line = current_line.saturating_add(1);
            line_start = index + 1;
        }
    }
    if current_line != target_line {
        return source.len();
    }
    let line_end = source[line_start..]
        .find('\n')
        .map_or(source.len(), |offset| line_start + offset);
    if unit == ColumnUnit::Byte {
        let mut offset = line_start
            .saturating_add(target_column.saturating_sub(1) as usize)
            .min(line_end);
        // A compiler column can land mid-character only on a malformed report;
        // snapping forward keeps the range sliceable either way.
        while offset < line_end && !source.is_char_boundary(offset) {
            offset += 1;
        }
        return offset;
    }
    let mut column = 1_u32;
    for (offset, character) in source[line_start..line_end].char_indices() {
        if column >= target_column {
            return line_start + offset;
        }
        column = if character == '\t' {
            column.saturating_add(4 - ((column.saturating_sub(1)) % 4))
        } else {
            column.saturating_add(1)
        };
    }
    line_end
}

pub(super) fn compact_range(start: usize, end: usize) -> TextRange {
    let start = u32::try_from(start).unwrap_or(u32::MAX);
    let end = u32::try_from(end).unwrap_or(u32::MAX).max(start);
    TextRange::new(TextSize::new(start), TextSize::new(end))
        .unwrap_or_else(|| TextRange::empty(TextSize::new(start)))
}

pub(super) fn line_point_range(source: &str, line: u32) -> TextRange {
    compact_range(
        offset_for_line_column(source, line, 1, ColumnUnit::Display),
        offset_for_line_column(source, line, 1, ColumnUnit::Display),
    )
}

pub(super) fn introduces_diagnostics(
    before: &[NorminetteDiagnostic],
    after: &[NorminetteDiagnostic],
) -> bool {
    let counts = |items: &[NorminetteDiagnostic]| {
        let mut counts = BTreeMap::<String, usize>::new();
        for item in items {
            *counts.entry(item.rule_id.clone()).or_default() += 1;
        }
        counts
    };
    let before = counts(before);
    counts(after)
        .into_iter()
        .any(|(rule, count)| count > before.get(&rule).copied().unwrap_or_default())
}

/// Warns once when the run used a Norminette release this version has not been
/// verified against.
pub(super) fn untested_norminette_diagnostic(
    oracle: &OracleContext,
    file: &DiscoveredFile,
    path: &Utf8PathBuf,
) -> Option<Diagnostic> {
    let fingerprint = oracle.oracle.fingerprint();
    if !fingerprint.untested || oracle.norminette_notice_path.as_path() != file.path.as_path() {
        return None;
    }
    Some(point_diagnostic(
        path,
        "NORMINETTE_VERSION_UNTESTED",
        Severity::Info,
        format!(
            "This run used Norminette {}, which this normfix release has not been verified against; {} is the supported version.",
            fingerprint.version,
            normfix_oracle::SUPPORTED_NORMINETTE_VERSION
        ),
        DiagnosticSource::NorminetteCompat(fingerprint.version.clone()),
        Some(
            "The before/after proof still compares two answers from this same checker, so a run cannot make its own result worse. What is not guaranteed is that the native rules agree with this release; review the diff."
                .to_owned(),
        ),
    ))
}

pub(super) fn parser_diagnostics(path: &Utf8PathBuf, source: &str) -> Vec<Diagnostic> {
    normfix_c_actions::syntax_recovery_diagnostics(path, source)
}

pub(super) fn point_diagnostic(
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

pub(super) fn explain_constant_array_false_positives(
    path: &Utf8PathBuf,
    source: &str,
    diagnostics: &mut Vec<NorminetteDiagnostic>,
    norminette_version: &str,
) -> Vec<Diagnostic> {
    if !diagnostics
        .iter()
        .any(|diagnostic| diagnostic.rule_id == "VLA_FORBIDDEN")
    {
        return Vec::new();
    }
    let Ok(mut parser) = CParser::new() else {
        return Vec::new();
    };
    let Ok(parsed) = parser.parse(source) else {
        return Vec::new();
    };
    if parsed.has_syntax_errors() {
        return Vec::new();
    }
    let semantic = analyze_semantics(&parsed);
    let mut advisories = Vec::new();
    diagnostics.retain(|diagnostic| {
        if diagnostic.rule_id != "VLA_FORBIDDEN" {
            return true;
        }
        let offset = u32::try_from(offset_for_line_column(
            source,
            diagnostic.line,
            diagnostic.column,
            ColumnUnit::Display,
        ))
        .map_or(TextSize::new(u32::MAX), TextSize::new);
        let constant = semantic.arrays.iter().find(|array| {
            array.range.contains(offset) && matches!(array.kind, ArrayBoundKind::Constant(_))
        });
        let Some(array) = constant else {
            return true;
        };
        let ArrayBoundKind::Constant(value) = array.kind else {
            return true;
        };
        advisories.push(Diagnostic {
            rule_id: "VLA_COMPAT_FALSE_POSITIVE".to_owned(),
            path: path.clone(),
            range: array.bound_range.unwrap_or(array.range),
            severity: Severity::Info,
            message: format!(
                "Norminette reported a VLA, but `{}` resolves to the integer constant {value}.",
                array.expression.as_deref().unwrap_or("this bound")
            ),
            source: DiagnosticSource::NorminetteCompat(norminette_version.to_owned()),
            notes: vec![
                "The native enum evaluator proved this bound within the current translation unit."
                    .to_owned(),
            ],
            help: Some(
                "No code change is required; keep the enum definition visible before this array."
                    .to_owned(),
            ),
            localized: None,
        });
        false
    });
    advisories
}

#[cfg(test)]
mod tests {
    use normfix_core::Severity;
    use normfix_oracle::NorminetteDiagnostic;

    use super::{diagnostic_help, official_diagnostics};

    fn remark(rule_id: &str, advisory: bool) -> NorminetteDiagnostic {
        NorminetteDiagnostic {
            rule_id: rule_id.to_owned(),
            line: 1,
            column: 1,
            message: "…".to_owned(),
            advisory,
        }
    }

    #[test]
    fn advice_written_for_a_rule_reaches_a_diagnostic_the_checker_reported() {
        // The advice for these rules is written once, in the rule catalogue.
        // Before it was shared, whether a student saw it depended on which
        // analyzer happened to notice the violation — and for the rules the
        // checker owns, which is most of them, nobody ever saw it.
        assert_eq!(
            diagnostic_help("TERNARY_FBIDDEN"),
            "Replace the ternary with an explicit if/else."
        );
        assert_eq!(
            diagnostic_help("GOTO_FBIDDEN"),
            "Restructure the associated control flow without goto or labels."
        );

        // The engine keeps the arms that name normfix's own flags, because the
        // rule catalogue has no business knowing they exist.
        assert!(diagnostic_help("WRONG_SCOPE_COMMENT").contains("--remove-invalid-comments"));

        // An identifier from no catalogue still answers something true.
        assert!(diagnostic_help("NOT_A_RULE").contains("no semantics-preserving automatic edit"));
    }

    #[test]
    fn a_notice_is_ranked_as_an_advisory_and_an_error_is_not() {
        let path = camino::Utf8PathBuf::from("a.c");
        let diagnostics = official_diagnostics(
            &path,
            "int\tg_counter;\n",
            &[
                remark("GLOBAL_VAR_DETECTED", true),
                remark("GOTO_FBIDDEN", false),
            ],
            "3.3.59",
        );

        // The checker calls a notice-only file OK, so ranking the notice with
        // the violations would report work that its own author says is done.
        assert_eq!(diagnostics[0].severity, Severity::Info);
        assert_eq!(diagnostics[1].severity, Severity::Warning);
    }

    #[test]
    fn each_authority_column_convention_lands_on_the_same_character() {
        use super::{ColumnUnit, offset_for_line_column};

        // Two tabs then the call: the shape of almost every 42 statement.
        let source = "int\tmain(void)\n{\n\t\tsort_medium(ctx);\n}\n";
        let call = source.find("sort_medium").expect("the call");

        // A C compiler counts bytes, so the call starts at column 3.
        assert_eq!(
            offset_for_line_column(source, 3, 3, ColumnUnit::Byte),
            call,
            "a compiler column must be read as a byte offset"
        );
        // The official Norminette counts display columns, so two four-column
        // tab stops put the same character at column 9.
        assert_eq!(
            offset_for_line_column(source, 3, 9, ColumnUnit::Display),
            call,
            "a Norminette column must be read as a tab-expanded display column"
        );
        // Reading a compiler column as a display column is the bug this guards:
        // it stops inside the indentation instead of on the call.
        assert_ne!(
            offset_for_line_column(source, 3, 3, ColumnUnit::Display),
            call
        );
    }

    #[test]
    fn a_byte_column_past_the_line_or_inside_a_character_stays_sliceable() {
        use super::{ColumnUnit, offset_for_line_column};

        let source = "\tchar\t*s = \"caf\u{e9}\"; boom(s);\n";
        // Clang counts both bytes of `é`, so the reported column is the byte
        // offset plus one, not the character count.
        let boom = source.find("boom").expect("the call");
        let reported = u32::try_from(boom).expect("fits") + 1;
        assert_eq!(
            offset_for_line_column(source, 1, reported, ColumnUnit::Byte),
            boom
        );
        assert!(reported > u32::try_from(source[..boom].chars().count()).expect("fits"));

        // A column past the end clamps to the line end rather than running on
        // into the next line.
        let line_end = source.find('\n').expect("the newline");
        assert_eq!(
            offset_for_line_column(source, 1, 9_999, ColumnUnit::Byte),
            line_end
        );

        // A column landing mid-character snaps forward to a boundary, so the
        // range can still be sliced.
        let accent = source.find('\u{e9}').expect("the accent");
        let offset = offset_for_line_column(
            source,
            1,
            u32::try_from(accent).expect("fits") + 2,
            ColumnUnit::Byte,
        );
        assert!(source.is_char_boundary(offset));
    }
}
