//! Conservative packing of explicit C source assignments.

use std::collections::BTreeSet;
use std::sync::OnceLock;

use normfix_header::{ByteRange, Fix};
use regex::Regex;
use unicode_width::UnicodeWidthChar as _;

const COLUMN_LIMIT: usize = 80;

/// Result of packing eligible explicit source assignments.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssignmentCompaction {
    /// Complete transformed source.
    pub output: String,
    /// Accepted replacements with ranges in the input snapshot.
    pub fixes: Vec<Fix>,
}

/// Returns display width using the project's four-column tab stops.
#[must_use]
pub fn visual_width(text: &str) -> usize {
    let mut column = 1;
    for character in text.chars() {
        if character == '\t' {
            column += 4 - ((column - 1) % 4);
        } else {
            column += character.width().unwrap_or_default();
        }
    }
    column - 1
}

/// Greedily packs plain explicit `.c` assignments up to 80 display columns.
///
/// The pass is disabled by `.RECIPEPREFIX`. It also skips `define` bodies,
/// shell assignments and any value containing Make expansion, patterns,
/// comments, quotes or command separators.
///
/// # Panics
///
/// Caller input cannot cause a panic. Initialization would panic only if a
/// built-in regular-expression literal were invalid.
#[must_use]
pub fn compact_source_assignments(source: &str) -> AssignmentCompaction {
    static RECIPE_PREFIX: OnceLock<Regex> = OnceLock::new();
    let recipe_prefix = RECIPE_PREFIX.get_or_init(|| {
        Regex::new(r"(?m)^[ \t]*\.RECIPEPREFIX[ \t]*(?::=|\+=|\?=|=)")
            .expect("constant recipe-prefix regex")
    });
    if recipe_prefix.is_match(source) {
        return AssignmentCompaction {
            output: source.to_owned(),
            fixes: Vec::new(),
        };
    }

    let lines = split_lines(source);
    if lines.is_empty() {
        return AssignmentCompaction {
            output: source.to_owned(),
            fixes: Vec::new(),
        };
    }
    let definition_lines = make_definition_lines(&lines);
    let mut replacements = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        let mut logical_end = index;
        while logical_end + 1 < lines.len() && has_clean_continuation(lines[logical_end].raw) {
            logical_end += 1;
        }
        let eligible = !(index..=logical_end).any(|line| definition_lines.contains(&line));
        let replacement = eligible
            .then(|| compact_assignment_block(&lines[index..=logical_end]))
            .flatten();
        if let Some(replacement) = replacement {
            let range = ByteRange::new(lines[index].start, lines[logical_end].end);
            if source[range.start..range.end] != replacement {
                replacements.push((range, replacement));
            }
        }
        index = logical_end + 1;
    }
    if replacements.is_empty() {
        return AssignmentCompaction {
            output: source.to_owned(),
            fixes: Vec::new(),
        };
    }

    let mut output = source.to_owned();
    for (range, replacement) in replacements.iter().rev() {
        output.replace_range(range.start..range.end, replacement);
    }
    let fixes = replacements
        .into_iter()
        .map(|(range, _)| Fix {
            code: "MAKEFILE_COMPACT_SOURCES",
            description: "packed an explicit C source list up to the 80-column limit".to_owned(),
            range,
        })
        .collect();
    AssignmentCompaction { output, fixes }
}

#[derive(Clone, Copy, Debug)]
struct SourceLine<'a> {
    raw: &'a str,
    start: usize,
    end: usize,
}

impl<'a> SourceLine<'a> {
    fn content(self) -> &'a str {
        self.raw.trim_end_matches(['\r', '\n'])
    }
}

fn split_lines(source: &str) -> Vec<SourceLine<'_>> {
    if source.is_empty() {
        return Vec::new();
    }
    let bytes = source.as_bytes();
    let mut lines = Vec::new();
    let mut start = 0;
    let mut index = 0;
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
            });
            start = end;
            index = end;
        } else {
            index += 1;
        }
    }
    if start < source.len() {
        lines.push(SourceLine {
            raw: &source[start..],
            start,
            end: source.len(),
        });
    }
    lines
}

fn compact_assignment_block(block: &[SourceLine<'_>]) -> Option<String> {
    static ASSIGNMENT: OnceLock<Regex> = OnceLock::new();
    static PLAIN_C_SOURCE: OnceLock<Regex> = OnceLock::new();
    let assignment = ASSIGNMENT.get_or_init(|| {
        Regex::new(concat!(
            r"^(?P<name>[A-Za-z_][A-Za-z0-9_]*)(?P<spacing>[ \t]*)",
            r"(?P<operator>::=|:::=|:=|\+=|\?=|!=|=)(?P<after>[ \t]*)(?P<body>.*)$"
        ))
        .expect("constant assignment regex")
    });
    let plain_source = PLAIN_C_SOURCE
        .get_or_init(|| Regex::new(r"^[A-Za-z0-9_./+-]+\.c$").expect("constant source regex"));
    let first = block.first()?.content();
    let captures = assignment.captures(first)?;
    if captures.name("operator")?.as_str() == "!=" {
        return None;
    }

    let mut parts = Vec::with_capacity(block.len());
    for (offset, line) in block.iter().enumerate() {
        let mut text = if offset == 0 {
            captures.name("body")?.as_str()
        } else {
            line.content().trim_start_matches([' ', '\t'])
        };
        if offset + 1 < block.len() {
            text = text.strip_suffix('\\')?;
        }
        if text
            .chars()
            .any(|character| matches!(character, '$' | '%' | '#' | '"' | '\'' | ';'))
        {
            return None;
        }
        parts.push(text);
    }
    let joined = parts.join(" ");
    let tokens = joined.split_whitespace().collect::<Vec<_>>();
    if tokens.is_empty() || tokens.iter().any(|token| !plain_source.is_match(token)) {
        return None;
    }
    let prefix = format!(
        "{}{}{} ",
        captures.name("name")?.as_str(),
        captures.name("spacing")?.as_str(),
        captures.name("operator")?.as_str()
    );
    let continuation = "\t".repeat(visual_width(&prefix).div_ceil(4).max(1));
    let packed = pack_tokens(&tokens, &prefix, &continuation)?;
    let newline = if block.last()?.raw.ends_with(['\r', '\n']) {
        "\n"
    } else {
        ""
    };
    Some(packed.join("\n") + newline)
}

pub(crate) fn pack_tokens(
    tokens: &[&str],
    first_prefix: &str,
    continuation_prefix: &str,
) -> Option<Vec<String>> {
    let mut packed = Vec::new();
    let mut index = 0;
    let mut prefix = first_prefix;
    while index < tokens.len() {
        let mut current = prefix.to_owned();
        let mut added = 0;
        while index < tokens.len() {
            let separator = if current.ends_with([' ', '\t']) {
                ""
            } else {
                " "
            };
            let candidate = format!("{current}{separator}{}", tokens[index]);
            let suffix = if index + 1 < tokens.len() { " \\" } else { "" };
            if visual_width(&(candidate.clone() + suffix)) > COLUMN_LIMIT {
                break;
            }
            current = candidate;
            index += 1;
            added += 1;
        }
        if added == 0 {
            return None;
        }
        if index < tokens.len() {
            current.push_str(" \\");
        }
        packed.push(current);
        prefix = continuation_prefix;
    }
    Some(packed)
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
    use super::{compact_source_assignments, visual_width};

    #[test]
    fn width_uses_terminal_cells_for_unicode() {
        assert_eq!(visual_width("界"), 2);
        assert_eq!(visual_width("e\u{301}"), 1);
    }

    #[test]
    fn plain_source_list_is_greedily_packed_without_reordering() {
        let names = (0..18)
            .map(|index| format!("source_{index:02}.c"))
            .collect::<Vec<_>>();
        let source = format!("SRC\t\t= \t{}\n", names.join(" \\\n\t\t\t"));
        let compacted = compact_source_assignments(&source);
        let observed = compacted
            .output
            .replace('\\', "")
            .split_whitespace()
            .filter(|token| token.as_bytes().ends_with(b".c"))
            .map(str::to_owned)
            .collect::<Vec<_>>();
        assert_eq!(observed, names);
        assert!(compacted.output.lines().count() < source.lines().count());
        assert!(
            compacted
                .output
                .lines()
                .all(|line| visual_width(line) <= 80)
        );
        assert_eq!(
            compacted.output,
            concat!(
                "SRC\t\t= source_00.c source_01.c source_02.c source_03.c source_04.c \\\n",
                "\t\t\tsource_05.c source_06.c source_07.c source_08.c source_09.c \\\n",
                "\t\t\tsource_10.c source_11.c source_12.c source_13.c source_14.c \\\n",
                "\t\t\tsource_15.c source_16.c source_17.c\n"
            )
        );
        assert_eq!(
            compact_source_assignments(&compacted.output).output,
            compacted.output
        );
    }

    #[test]
    fn complex_make_constructs_are_never_reflowed() {
        let cases = [
            "SRC = $(addprefix src/,one.c two.c)\n",
            "SRC != printf 'one.c two.c'\n",
            "define TEMPLATE\nSRC = one.c \\\n two.c\nendef\n",
            "SRC = one.c two.c # selected by the subject\n",
            ".RECIPEPREFIX = >\nSRC = one.c \\\n two.c\n",
            "\tSRC = one.c two.c\n",
        ];
        for source in cases {
            let result = compact_source_assignments(source);
            assert_eq!(result.output, source);
            assert!(result.fixes.is_empty());
        }
    }

    #[test]
    fn one_token_that_cannot_fit_is_preserved() {
        let source = format!("SRC = {}.c\n", "x".repeat(80));
        let result = compact_source_assignments(&source);
        assert_eq!(result.output, source);
        assert!(result.fixes.is_empty());
    }
}
