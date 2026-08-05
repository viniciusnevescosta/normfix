//! Read-only Makefile checks that require manual judgment.

use std::collections::BTreeSet;
use std::sync::OnceLock;

use normfix_header::ByteRange;
use regex::Regex;

use crate::compact::visual_width;
use crate::header::makefile_header_is_valid;

const MANDATORY_RULES: [&str; 4] = ["all", "clean", "fclean", "re"];

/// One English Makefile diagnostic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MakefileDiagnostic {
    /// Stable machine-readable code.
    pub code: &'static str,
    /// Human-readable English explanation.
    pub message: String,
    /// Relevant half-open byte range.
    pub range: ByteRange,
    /// One-based source line.
    pub line: usize,
    /// One-based display column using four-column tabs.
    pub column: usize,
    /// Concrete next action.
    pub suggestion: String,
    /// Optional supporting detail.
    pub detail: String,
    /// Diagnostic producer.
    pub source: &'static str,
}

/// Reports official-header, mandatory-rule and layout problems in English.
#[must_use]
pub fn analyze_makefile(source: &str) -> Vec<MakefileDiagnostic> {
    let lines = source_lines(source);
    let mut diagnostics = Vec::new();
    if !makefile_header_is_valid(source) {
        diagnostics.push(diagnostic(
            "INVALID_HEADER",
            "The official 42 Makefile header is missing or malformed",
            ByteRange::new(0, 0),
            1,
            1,
            "Configure a verified 42 student email so the header can be inserted safely.",
            "",
        ));
    }

    let assignments = logical_assignments(&lines);
    if !assignments
        .iter()
        .any(|assignment| assignment.name == "NAME")
    {
        diagnostics.push(diagnostic(
            "MAKEFILE_NAME_MISSING",
            "The mandatory NAME variable was not found",
            ByteRange::new(0, 0),
            1,
            1,
            "Define NAME explicitly with the artifact produced by this Makefile.",
            "",
        ));
    }
    for assignment in &assignments {
        if contains_source_wildcard(&assignment.body) {
            diagnostics.push(diagnostic(
                "MAKEFILE_WILDCARD_SOURCE",
                "Source and object files must be named explicitly",
                assignment.range,
                assignment.line,
                1,
                "Replace wildcard expansion with an explicit list of every required source.",
                "",
            ));
        }
    }

    let rules = rules(&lines);
    let targets = rules
        .iter()
        .flat_map(|rule| rule.targets.iter().copied())
        .collect::<BTreeSet<_>>();
    for mandatory in MANDATORY_RULES {
        if !targets.contains(mandatory) {
            diagnostics.push(diagnostic(
                "MAKEFILE_MISSING_RULE",
                &format!("The mandatory '{mandatory}' rule was not found"),
                ByteRange::new(0, 0),
                1,
                1,
                &format!("Add a {mandatory} rule that follows the project subject."),
                "",
            ));
        }
    }
    if !targets.contains("$(NAME)") && !targets.contains("${NAME}") {
        diagnostics.push(diagnostic(
            "MAKEFILE_NAME_RULE_MISSING",
            "The mandatory $(NAME) build rule was not found",
            ByteRange::new(0, 0),
            1,
            1,
            "Add an explicit $(NAME) target with the object files as prerequisites.",
            "",
        ));
    }
    if let Some(first) = rules
        .iter()
        .find(|rule| {
            rule.targets
                .iter()
                .any(|target| !target.starts_with('.') && !target.contains('%'))
        })
        .filter(|rule| !rule.targets.contains(&"all"))
    {
        diagnostics.push(diagnostic(
            "MAKEFILE_DEFAULT_RULE",
            "The mandatory 'all' rule is not the default target",
            first.range,
            first.line,
            1,
            "Move the all rule before the first concrete build target.",
            "",
        ));
    }

    append_layout_diagnostics(&mut diagnostics, &lines);
    diagnostics
}

fn append_layout_diagnostics(diagnostics: &mut Vec<MakefileDiagnostic>, lines: &[SourceLine<'_>]) {
    for line in lines {
        let width = visual_width(line.content);
        if width > 80 {
            let start = display_column_byte_offset(line.content, 81);
            diagnostics.push(diagnostic(
                "MAKEFILE_LINE_TOO_LONG",
                "This Makefile line exceeds 80 display columns",
                ByteRange::new(line.start + start, line.start + line.content.len()),
                line.number,
                81,
                "Shorten it manually; only plain explicit .c lists are reflowed automatically.",
                &format!("This line is {width} display columns; the limit is 80."),
            ));
        }
        let stripped = line.content.trim_end_matches([' ', '\t']);
        if stripped.ends_with('\\') && stripped != line.content {
            diagnostics.push(diagnostic(
                "MAKEFILE_TRAILING_AFTER_BACKSLASH",
                "Whitespace after a continuation backslash was preserved",
                ByteRange::new(line.start + stripped.len(), line.start + line.content.len()),
                line.number,
                visual_width(stripped) + 1,
                "Remove it manually after confirming that enabling continuation is intended.",
                "",
            ));
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct SourceLine<'a> {
    content: &'a str,
    start: usize,
    end: usize,
    number: usize,
}

fn source_lines(source: &str) -> Vec<SourceLine<'_>> {
    if source.is_empty() {
        return Vec::new();
    }
    let bytes = source.as_bytes();
    let mut lines = Vec::new();
    let mut start = 0;
    let mut number = 1;
    let mut index = 0;
    while index < bytes.len() {
        if matches!(bytes[index], b'\r' | b'\n') {
            let content_end = index;
            let end = if bytes[index] == b'\r' && bytes.get(index + 1) == Some(&b'\n') {
                index + 2
            } else {
                index + 1
            };
            lines.push(SourceLine {
                content: &source[start..content_end],
                start,
                end,
                number,
            });
            number += 1;
            start = end;
            index = end;
        } else {
            index += 1;
        }
    }
    if start < source.len() {
        lines.push(SourceLine {
            content: &source[start..],
            start,
            end: source.len(),
            number,
        });
    }
    lines
}

#[derive(Clone, Debug)]
struct Assignment {
    name: String,
    body: String,
    line: usize,
    range: ByteRange,
}

fn logical_assignments(lines: &[SourceLine<'_>]) -> Vec<Assignment> {
    static ASSIGNMENT: OnceLock<Regex> = OnceLock::new();
    let assignment = ASSIGNMENT.get_or_init(|| {
        Regex::new(concat!(
            r"^(?P<name>[A-Za-z_][A-Za-z0-9_]*)(?P<spacing>[ \t]*)",
            r"(?P<operator>::=|:::=|:=|\+=|\?=|!=|=)(?P<after>[ \t]*)(?P<body>.*)$"
        ))
        .expect("constant assignment regex")
    });
    let definitions = make_definition_lines(lines);
    let mut found = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        let start = index;
        let mut end = index;
        while lines[end].content.ends_with('\\') && end + 1 < lines.len() {
            end += 1;
        }
        let eligible = !(start..=end).any(|line| definitions.contains(&line));
        let captures = eligible
            .then(|| assignment.captures(lines[start].content))
            .flatten();
        if let Some(captures) = captures {
            let mut body = captures
                .name("body")
                .map_or("", |value| value.as_str())
                .strip_suffix('\\')
                .unwrap_or_else(|| captures.name("body").map_or("", |value| value.as_str()))
                .to_owned();
            for line in &lines[start + 1..=end] {
                body.push(' ');
                body.push_str(
                    line.content
                        .trim_start_matches([' ', '\t'])
                        .strip_suffix('\\')
                        .unwrap_or_else(|| line.content.trim_start_matches([' ', '\t'])),
                );
            }
            found.push(Assignment {
                name: captures
                    .name("name")
                    .expect("matched assignment has name")
                    .as_str()
                    .to_owned(),
                body,
                line: lines[start].number,
                range: ByteRange::new(lines[start].start, lines[end].end),
            });
        }
        index = end + 1;
    }
    found
}

#[derive(Clone, Debug)]
struct Rule<'a> {
    targets: Vec<&'a str>,
    line: usize,
    range: ByteRange,
}

fn rules<'a>(lines: &'a [SourceLine<'a>]) -> Vec<Rule<'a>> {
    static RULE: OnceLock<Regex> = OnceLock::new();
    let regex = RULE.get_or_init(|| {
        Regex::new(r"^(?P<targets>[^#=\t][^#=]*?):(?:[^=]|$)").expect("constant rule regex")
    });
    let mut rules = Vec::new();
    for line in lines {
        if line.content.is_empty()
            || line.content.starts_with(['\t', '#'])
            || line
                .content
                .split_once(':')
                .is_some_and(|(before, _)| before.contains('='))
        {
            continue;
        }
        let Some(captures) = regex.captures(line.content) else {
            continue;
        };
        let targets = captures
            .name("targets")
            .expect("matched rule has targets")
            .as_str()
            .split_whitespace()
            .collect::<Vec<_>>();
        if !targets.is_empty() {
            rules.push(Rule {
                targets,
                line: line.number,
                range: ByteRange::new(line.start, line.end),
            });
        }
    }
    rules
}

fn contains_source_wildcard(body: &str) -> bool {
    static GLOB: OnceLock<Regex> = OnceLock::new();
    let lowered = body.to_ascii_lowercase();
    lowered.contains("$(wildcard")
        || lowered.contains("${wildcard")
        || GLOB
            .get_or_init(|| {
                Regex::new(r"(?m)(?:^|[^\\])[*?][^ \t]*(?:\.c|\.o)\b")
                    .expect("constant wildcard regex")
            })
            .is_match(&lowered)
}

fn make_definition_lines(lines: &[SourceLine<'_>]) -> BTreeSet<usize> {
    static DEFINE: OnceLock<Regex> = OnceLock::new();
    static ENDEF: OnceLock<Regex> = OnceLock::new();
    let define = DEFINE.get_or_init(|| {
        Regex::new(r"^(?:(?:override|export|private)[ \t]+)*define(?:[ \t]|$)")
            .expect("constant define regex")
    });
    let endef =
        ENDEF.get_or_init(|| Regex::new(r"^endef(?:[ \t]|$)").expect("constant endef regex"));
    let mut blocked = BTreeSet::new();
    let mut depth = 0_usize;
    for (index, line) in lines.iter().enumerate() {
        let stripped = line.content.trim();
        if define.is_match(stripped) {
            depth += 1;
        }
        if depth > 0 {
            blocked.insert(index);
        }
        if depth > 0 && endef.is_match(stripped) {
            depth -= 1;
        }
    }
    blocked
}

fn display_column_byte_offset(line: &str, target: usize) -> usize {
    let mut column = 1;
    for (offset, character) in line.char_indices() {
        if column >= target {
            return offset;
        }
        column += if character == '\t' {
            4 - ((column - 1) % 4)
        } else {
            1
        };
    }
    line.len()
}

fn diagnostic(
    code: &'static str,
    message: &str,
    range: ByteRange,
    line: usize,
    column: usize,
    suggestion: &str,
    detail: &str,
) -> MakefileDiagnostic {
    MakefileDiagnostic {
        code,
        message: message.to_owned(),
        range,
        line,
        column,
        suggestion: suggestion.to_owned(),
        detail: detail.to_owned(),
        source: "normfix Makefile check",
    }
}

#[cfg(test)]
mod tests {
    use super::analyze_makefile;

    #[test]
    fn reports_manual_norm_rules_in_english() {
        let source = "SRC = $(wildcard *.c)\nfirst:\n\tcc *.c\n";
        let diagnostics = analyze_makefile(source);
        let codes = diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code)
            .collect::<Vec<_>>();
        assert!(codes.contains(&"INVALID_HEADER"));
        assert!(codes.contains(&"MAKEFILE_NAME_MISSING"));
        assert!(codes.contains(&"MAKEFILE_WILDCARD_SOURCE"));
        assert!(codes.contains(&"MAKEFILE_NAME_RULE_MISSING"));
        assert_eq!(
            codes
                .iter()
                .filter(|code| **code == "MAKEFILE_MISSING_RULE")
                .count(),
            4
        );
        assert!(codes.contains(&"MAKEFILE_DEFAULT_RULE"));
        assert!(
            diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.suggestion.is_empty())
        );
    }

    #[test]
    fn reports_display_width_and_backslash_whitespace_with_byte_ranges() {
        let source = format!("{}\nSRC = one.c \\  \n", "a".repeat(81));
        let diagnostics = analyze_makefile(&source);
        let long = diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == "MAKEFILE_LINE_TOO_LONG")
            .expect("long line");
        assert_eq!(long.line, 1);
        assert_eq!(long.column, 81);
        assert_eq!(&source[long.range.start..long.range.end], "a");
        let slash = diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == "MAKEFILE_TRAILING_AFTER_BACKSLASH")
            .expect("backslash whitespace");
        assert_eq!(&source[slash.range.start..slash.range.end], "  ");
    }
}
