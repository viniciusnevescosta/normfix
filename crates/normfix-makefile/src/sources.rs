//! Closed-shape analysis of literal C source assignments.

use std::collections::BTreeSet;
use std::sync::OnceLock;

use normfix_header::{ByteRange, Fix};
use regex::Regex;

use crate::compact::{pack_tokens, visual_width};

/// One literal `.c` token from an unambiguous source-list assignment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MakefileSourceReference {
    /// Path exactly as written in the Makefile.
    pub path: String,
    /// Exact token bytes in the original source.
    pub range: ByteRange,
    /// One-based physical line.
    pub line: usize,
    /// One-based display column using four-column tab stops.
    pub column: usize,
}

/// Result of checking and optionally removing missing literal sources.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceReconciliation {
    /// Complete transformed Makefile.
    pub output: String,
    /// Literal source paths for which the caller could prove nonexistence.
    pub missing: Vec<MakefileSourceReference>,
    /// Unsafe removals accepted from the exact input snapshot.
    pub fixes: Vec<Fix>,
}

/// Caller-owned filesystem proof for one literal source path.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourcePathStatus {
    /// The path exists or otherwise must be preserved.
    Exists,
    /// The path was proven absent, normally through an exact `NotFound` result.
    Missing,
    /// Existence could not be established safely.
    Unknown,
}

/// Checks literal `SRC`/`SRCS`-style assignments with a caller-owned path proof.
///
/// Only assignments whose complete value consists of ordinary relative `.c`
/// paths are eligible. Make expansion, patterns, comments, quotes, commands,
/// recipes, `define` bodies, and `.RECIPEPREFIX` projects fail closed. When
/// `remove_missing` is true, missing tokens are removed and the remaining list
/// is packed without reordering. The callback must return true only for paths
/// it has independently proved to exist.
#[must_use]
pub fn reconcile_source_references<F>(
    source: &str,
    remove_missing: bool,
    mut exists: F,
) -> SourceReconciliation
where
    F: FnMut(&str) -> SourcePathStatus,
{
    if has_recipe_prefix(source) {
        return SourceReconciliation {
            output: source.to_owned(),
            missing: Vec::new(),
            fixes: Vec::new(),
        };
    }
    let lines = split_lines(source);
    let definitions = make_definition_lines(&lines);
    let mut missing = Vec::new();
    let mut fixes = Vec::new();
    let mut replacements = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        let mut logical_end = index;
        while logical_end + 1 < lines.len() && has_clean_continuation(lines[logical_end].raw) {
            logical_end += 1;
        }
        if !(index..=logical_end).any(|line| definitions.contains(&line)) {
            if let Some(assignment) = parse_assignment(&lines[index..=logical_end]) {
                let assignment_missing = assignment
                    .tokens
                    .iter()
                    .filter(|token| exists(&token.reference.path) == SourcePathStatus::Missing)
                    .cloned()
                    .collect::<Vec<_>>();
                missing.extend(
                    assignment_missing
                        .iter()
                        .map(|token| token.reference.clone()),
                );
                if remove_missing && !assignment_missing.is_empty() {
                    let missing_ranges = assignment_missing
                        .iter()
                        .map(|token| token.reference.range)
                        .collect::<BTreeSet<_>>();
                    let retained = assignment
                        .tokens
                        .iter()
                        .filter(|token| !missing_ranges.contains(&token.reference.range))
                        .map(|token| token.reference.path.as_str())
                        .collect::<Vec<_>>();
                    if let Some(replacement) = assignment.reprint(&retained) {
                        replacements.push((assignment.range, replacement));
                        fixes.extend(assignment_missing.into_iter().map(|token| Fix {
                            code: "MAKEFILE_REMOVE_MISSING_SOURCE",
                            description: format!(
                                "removed missing literal source `{}` from a proven source list",
                                token.reference.path
                            ),
                            range: token.reference.range,
                        }));
                    }
                }
            }
        }
        index = logical_end + 1;
    }
    let mut output = source.to_owned();
    for (range, replacement) in replacements.iter().rev() {
        output.replace_range(range.start..range.end, replacement);
    }
    missing.sort_by_key(|reference| (reference.range.start, reference.range.end));
    fixes.sort_by_key(|fix| (fix.range.start, fix.range.end));
    SourceReconciliation {
        output,
        missing,
        fixes,
    }
}

#[derive(Clone, Copy, Debug)]
struct SourceLine<'source> {
    raw: &'source str,
    start: usize,
    end: usize,
    number: usize,
}

impl<'source> SourceLine<'source> {
    fn content(self) -> &'source str {
        self.raw.trim_end_matches(['\r', '\n'])
    }
}

#[derive(Clone, Debug)]
struct SourceToken {
    reference: MakefileSourceReference,
}

#[derive(Clone, Debug)]
struct SourceAssignment {
    range: ByteRange,
    prefix: String,
    continuation: String,
    newline: &'static str,
    tokens: Vec<SourceToken>,
}

impl SourceAssignment {
    fn reprint(&self, tokens: &[&str]) -> Option<String> {
        if tokens.is_empty() {
            return Some(format!("{}{}", self.prefix.trim_end(), self.newline));
        }
        pack_tokens(tokens, &self.prefix, &self.continuation)
            .map(|lines| lines.join("\n") + self.newline)
    }
}

fn parse_assignment(block: &[SourceLine<'_>]) -> Option<SourceAssignment> {
    static ASSIGNMENT: OnceLock<Regex> = OnceLock::new();
    static PLAIN_C_SOURCE: OnceLock<Regex> = OnceLock::new();
    let assignment = ASSIGNMENT.get_or_init(|| {
        Regex::new(concat!(
            r"^(?P<name>[A-Za-z_][A-Za-z0-9_]*)(?P<spacing>[ \t]*)",
            r"(?P<operator>::=|:::=|:=|\+=|\?=|=)(?P<after>[ \t]*)(?P<body>.*)$"
        ))
        .expect("constant assignment regex")
    });
    let plain_source = PLAIN_C_SOURCE.get_or_init(|| {
        Regex::new(r"^(?:[A-Za-z0-9_+.-]+/)*[A-Za-z0-9_+.-]+\.c$").expect("constant source regex")
    });
    let first = *block.first()?;
    let captures = assignment.captures(first.content())?;
    let name = captures.name("name")?.as_str();
    if !source_variable_name(name) || captures.name("operator")?.as_str() == "!=" {
        return None;
    }
    let mut tokens = Vec::new();
    for (offset, line) in block.iter().copied().enumerate() {
        let (mut body, body_start) = if offset == 0 {
            let capture = captures.name("body")?;
            (capture.as_str(), line.start + capture.start())
        } else {
            let content = line.content();
            let trimmed = content.trim_start_matches([' ', '\t']);
            (trimmed, line.start + content.len() - trimmed.len())
        };
        if offset + 1 < block.len() {
            body = body.strip_suffix('\\')?;
        }
        if body
            .chars()
            .any(|character| matches!(character, '$' | '%' | '#' | '"' | '\'' | ';' | ':'))
        {
            return None;
        }
        for (local_start, token) in whitespace_tokens(body) {
            if !plain_source.is_match(token)
                || token.starts_with(['/', '-'])
                || token
                    .split('/')
                    .any(|component| matches!(component, "." | ".."))
            {
                return None;
            }
            let start = body_start + local_start;
            let range = ByteRange::new(start, start + token.len());
            let before = &line.content()[..start.saturating_sub(line.start)];
            tokens.push(SourceToken {
                reference: MakefileSourceReference {
                    path: token.to_owned(),
                    range,
                    line: line.number,
                    column: visual_width(before) + 1,
                },
            });
        }
    }
    if tokens.is_empty() {
        return None;
    }
    let prefix = format!(
        "{}{}{} ",
        name,
        captures.name("spacing")?.as_str(),
        captures.name("operator")?.as_str()
    );
    let continuation = "\t".repeat(visual_width(&prefix).div_ceil(4).max(1));
    Some(SourceAssignment {
        range: ByteRange::new(first.start, block.last()?.end),
        prefix,
        continuation,
        newline: if block.last()?.raw.ends_with(['\r', '\n']) {
            "\n"
        } else {
            ""
        },
        tokens,
    })
}

fn source_variable_name(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    matches!(upper.as_str(), "SRC" | "SRCS" | "SOURCE" | "SOURCES")
        || upper.ends_with("_SRC")
        || upper.ends_with("_SRCS")
        || upper.ends_with("_SOURCE")
        || upper.ends_with("_SOURCES")
}

fn whitespace_tokens(text: &str) -> impl Iterator<Item = (usize, &str)> {
    let mut start = None;
    text.char_indices()
        .chain(std::iter::once((text.len(), ' ')))
        .filter_map(move |(index, character)| {
            if character.is_ascii_whitespace() {
                start.take().map(|start| (start, &text[start..index]))
            } else {
                start.get_or_insert(index);
                None
            }
        })
}

fn has_recipe_prefix(source: &str) -> bool {
    static RECIPE_PREFIX: OnceLock<Regex> = OnceLock::new();
    RECIPE_PREFIX
        .get_or_init(|| {
            Regex::new(r"(?m)^[ \t]*\.RECIPEPREFIX[ \t]*(?::=|\+=|\?=|=)")
                .expect("constant recipe-prefix regex")
        })
        .is_match(source)
}

fn split_lines(source: &str) -> Vec<SourceLine<'_>> {
    if source.is_empty() {
        return Vec::new();
    }
    let bytes = source.as_bytes();
    let mut lines = Vec::new();
    let mut start = 0;
    let mut index = 0;
    let mut number = 1;
    while index < bytes.len() {
        if matches!(bytes[index], b'\r' | b'\n') {
            let end = if bytes[index] == b'\r' && bytes.get(index + 1) == Some(&b'\n') {
                index + 2
            } else {
                index + 1
            };
            lines.push(SourceLine {
                raw: &source[start..end],
                start,
                end,
                number,
            });
            start = end;
            index = end;
            number += 1;
        } else {
            index += 1;
        }
    }
    if start < source.len() {
        lines.push(SourceLine {
            raw: &source[start..],
            start,
            end: source.len(),
            number,
        });
    }
    lines
}

fn has_clean_continuation(raw: &str) -> bool {
    raw.trim_end_matches(['\r', '\n']).ends_with('\\')
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
        let stripped = line.content().trim();
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

#[cfg(test)]
mod tests {
    use super::{SourcePathStatus, reconcile_source_references};

    #[test]
    fn reports_and_optionally_removes_only_missing_literal_sources() {
        let source = "SRC\t= src/kept.c src/missing.c \\\n\t\tother.c\n";
        let checked = reconcile_source_references(source, false, |path| {
            if matches!(path, "src/kept.c" | "other.c") {
                SourcePathStatus::Exists
            } else {
                SourcePathStatus::Missing
            }
        });
        assert_eq!(checked.output, source);
        assert_eq!(checked.missing.len(), 1);
        assert_eq!(checked.missing[0].path, "src/missing.c");
        assert!(checked.fixes.is_empty());

        let fixed = reconcile_source_references(source, true, |path| {
            if matches!(path, "src/kept.c" | "other.c") {
                SourcePathStatus::Exists
            } else {
                SourcePathStatus::Missing
            }
        });
        assert_eq!(fixed.output, "SRC\t= src/kept.c other.c\n");
        assert_eq!(fixed.fixes.len(), 1);
        assert_eq!(fixed.fixes[0].code, "MAKEFILE_REMOVE_MISSING_SOURCE");
        assert_eq!(
            reconcile_source_references(&fixed.output, true, |_| SourcePathStatus::Exists).output,
            fixed.output
        );
    }

    #[test]
    fn complex_or_unrelated_assignments_fail_closed() {
        let cases = [
            "FILES = missing.c\n",
            "SRC = $(addprefix src/,missing.c)\n",
            "SRC = missing.c # optional\n",
            "SRC = missing.c generated.o\n",
            "define LIST\nSRC = missing.c\nendef\n",
            ".RECIPEPREFIX = >\nSRC = missing.c\n",
            "\tSRC = missing.c\n",
            "SRC = ../outside.c\n",
        ];
        for source in cases {
            let result = reconcile_source_references(source, true, |_| SourcePathStatus::Missing);
            assert_eq!(result.output, source, "{source:?}");
            assert!(result.missing.is_empty(), "{source:?}");
            assert!(result.fixes.is_empty(), "{source:?}");
        }
    }

    #[test]
    fn removing_the_only_missing_source_keeps_a_valid_empty_assignment() {
        let result =
            reconcile_source_references("SRCS = gone.c\n", true, |_| SourcePathStatus::Missing);
        assert_eq!(result.output, "SRCS =\n");
        assert_eq!(result.missing.len(), 1);
        assert_eq!(result.fixes.len(), 1);
    }

    #[test]
    fn unknown_paths_are_never_reported_or_removed() {
        let result =
            reconcile_source_references("SRC = uncertain.c\n", true, |_| SourcePathStatus::Unknown);
        assert_eq!(result.output, "SRC = uncertain.c\n");
        assert!(result.missing.is_empty());
        assert!(result.fixes.is_empty());
    }
}
