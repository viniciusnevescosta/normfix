//! UTF-8 source coordinates, lexical masks, and hygiene normalization.

use std::borrow::Cow;

use unicode_width::UnicodeWidthChar as _;

use crate::{Applicability, CActionError, Fix};

/// Result of source hygiene normalization.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HygieneResult {
    /// Normalized UTF-8 source.
    pub source: String,
    /// Accepted hygiene fixes.
    pub fixes: Vec<Fix>,
}

/// Returns display width using four-column tab stops.
///
/// Non-tab characters use their terminal display width.
#[must_use]
pub fn visual_width(text: &str) -> u32 {
    visual_width_from(text, 1)
}

pub(crate) fn visual_width_from(text: &str, start_column: u32) -> u32 {
    let final_column = text.chars().fold(start_column, |column, character| {
        if character == '\t' {
            column.saturating_add(4 - ((column.saturating_sub(1)) % 4))
        } else {
            let width = u32::try_from(character.width().unwrap_or_default()).unwrap_or(u32::MAX);
            column.saturating_add(width)
        }
    });
    final_column.saturating_sub(start_column)
}

/// Performs the conservative hygiene phase.
///
/// A backslash followed by spaces or tabs at physical end-of-line is left
/// untouched because stripping it would create a new C line splice.
///
/// # Errors
///
/// This signature shares the action pipeline error type for forward
/// compatibility. The current normalization operations cannot fail.
pub fn normalize_hygiene(source: &str) -> Result<HygieneResult, CActionError> {
    let mut fixes = Vec::new();
    let without_bom = source.strip_prefix('\u{feff}');
    if without_bom.is_some() {
        fixes.push(hygiene_fix(
            "REMOVE_BOM",
            "removed the UTF-8 byte-order mark",
            Some(1),
        ));
    }
    let mut current = Cow::Borrowed(without_bom.unwrap_or(source));

    if current.contains('\r') {
        current = Cow::Owned(current.replace("\r\n", "\n").replace('\r', "\n"));
        fixes.push(hygiene_fix(
            "NORMALIZE_EOL",
            "normalized line endings to LF",
            None,
        ));
    }

    let mut cleaned = String::with_capacity(current.len());
    let mut trailing_count = 0_u32;
    for (index, line) in current.split('\n').enumerate() {
        if index > 0 {
            cleaned.push('\n');
        }
        let stripped = line.trim_end_matches([' ', '\t']);
        if stripped.ends_with('\\') && stripped != line {
            cleaned.push_str(line);
        } else {
            if stripped != line {
                trailing_count += 1;
            }
            cleaned.push_str(stripped);
        }
    }
    if trailing_count > 0 {
        fixes.push(hygiene_fix(
            "TRAILING_WHITESPACE",
            format!("removed trailing whitespace from {trailing_count} line(s)"),
            None,
        ));
    }

    let leading_newlines = cleaned.bytes().take_while(|byte| *byte == b'\n').count();
    if leading_newlines > 0 {
        cleaned.drain(..leading_newlines);
        fixes.push(hygiene_fix(
            "EMPTY_LINE_FILE_START",
            "removed blank line(s) at file start",
            Some(1),
        ));
    }

    let mut collapsed = String::with_capacity(cleaned.len());
    let mut newline_run = 0_u8;
    let mut collapsed_any = false;
    for character in cleaned.chars() {
        if character == '\n' {
            newline_run = newline_run.saturating_add(1);
            if newline_run <= 2 {
                collapsed.push(character);
            } else {
                collapsed_any = true;
            }
        } else {
            newline_run = 0;
            collapsed.push(character);
        }
    }
    if collapsed_any {
        fixes.push(hygiene_fix(
            "CONSECUTIVE_NEWLINES",
            "collapsed consecutive blank lines",
            None,
        ));
    }

    let final_source = if collapsed.is_empty() {
        collapsed
    } else {
        let trimmed_len = collapsed.trim_end_matches('\n').len();
        let mut normalized = collapsed[..trimmed_len].to_owned();
        normalized.push('\n');
        if normalized != collapsed {
            fixes.push(hygiene_fix(
                "EMPTY_LINE_EOF",
                "normalized the final newline",
                None,
            ));
        }
        normalized
    };

    Ok(HygieneResult {
        source: final_source,
        fixes,
    })
}

fn hygiene_fix(
    rule_id: impl Into<String>,
    description: impl Into<String>,
    line: Option<u32>,
) -> Fix {
    Fix {
        rule_id: rule_id.into(),
        description: description.into(),
        line,
        applicability: Applicability::SafeSemantic,
    }
}

#[derive(Clone, Debug)]
pub(crate) struct SourceLines<'source> {
    source: &'source str,
    lines: Vec<PhysicalLine>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct PhysicalLine {
    pub(crate) start: usize,
    pub(crate) content_end: usize,
    pub(crate) end: usize,
}

impl<'source> SourceLines<'source> {
    pub(crate) fn new(source: &'source str) -> Self {
        let mut lines = Vec::new();
        let mut start = 0;
        for (index, byte) in source.bytes().enumerate() {
            if byte == b'\n' {
                let content_end = if index > start && source.as_bytes()[index - 1] == b'\r' {
                    index - 1
                } else {
                    index
                };
                lines.push(PhysicalLine {
                    start,
                    content_end,
                    end: index + 1,
                });
                start = index + 1;
            }
        }
        if start < source.len() || lines.is_empty() {
            let content_end = source.len();
            lines.push(PhysicalLine {
                start,
                content_end,
                end: source.len(),
            });
        }
        Self { source, lines }
    }

    pub(crate) fn len(&self) -> usize {
        self.lines.len()
    }

    pub(crate) fn get(&self, one_based: u32) -> Option<PhysicalLine> {
        let index = usize::try_from(one_based.checked_sub(1)?).ok()?;
        self.lines.get(index).copied()
    }

    pub(crate) fn text(&self, line: PhysicalLine) -> &'source str {
        &self.source[line.start..line.content_end]
    }

    pub(crate) fn line_number_at(&self, offset: usize) -> u32 {
        match self.lines.binary_search_by_key(&offset, |line| line.start) {
            Ok(index) => u32::try_from(index + 1).unwrap_or(u32::MAX),
            Err(0) => 1,
            Err(index) => u32::try_from(index).unwrap_or(u32::MAX),
        }
    }

    pub(crate) fn byte_for_visual_column(&self, line: PhysicalLine, column: u32) -> usize {
        let text = self.text(line);
        let mut current = 1_u32;
        for (relative, character) in text.char_indices() {
            if current >= column {
                return line.start + relative;
            }
            if character == '\t' {
                current = current.saturating_add(4 - ((current.saturating_sub(1)) % 4));
            } else {
                current = current.saturating_add(1);
            }
        }
        line.content_end
    }

    pub(crate) fn visual_column(&self, line: PhysicalLine, offset: usize) -> u32 {
        visual_width_from(&self.source[line.start..offset], 1) + 1
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (u32, PhysicalLine, &'source str)> + '_ {
        self.lines.iter().enumerate().map(|(index, line)| {
            (
                u32::try_from(index + 1).unwrap_or(u32::MAX),
                *line,
                self.text(*line),
            )
        })
    }
}

#[derive(Clone, Debug)]
pub(crate) struct LexicalMap {
    protected: Vec<bool>,
    comment_lines: std::collections::BTreeSet<u32>,
}

impl LexicalMap {
    pub(crate) fn scan(source: &str) -> Self {
        let bytes = source.as_bytes();
        let mut protected = vec![false; bytes.len()];
        let mut comment_lines = std::collections::BTreeSet::new();
        let mut state = LexState::Code;
        let mut index = 0;
        let mut line = 1_u32;
        while index < bytes.len() {
            let following = bytes.get(index + 1).copied();
            match state {
                LexState::Code => {
                    if bytes[index] == b'/' && following == Some(b'/') {
                        protected[index] = true;
                        protected[index + 1] = true;
                        comment_lines.insert(line);
                        state = LexState::LineComment;
                        index += 2;
                        continue;
                    }
                    if bytes[index] == b'/' && following == Some(b'*') {
                        protected[index] = true;
                        protected[index + 1] = true;
                        comment_lines.insert(line);
                        state = LexState::BlockComment;
                        index += 2;
                        continue;
                    }
                    if bytes[index] == b'"' {
                        protected[index] = true;
                        state = LexState::String;
                    } else if bytes[index] == b'\'' {
                        protected[index] = true;
                        state = LexState::Character;
                    }
                }
                LexState::LineComment => {
                    comment_lines.insert(line);
                    if bytes[index] == b'\n' {
                        if escaped_physical_newline(bytes, index) {
                            protected[index] = true;
                        } else {
                            state = LexState::Code;
                        }
                    } else {
                        protected[index] = true;
                    }
                }
                LexState::BlockComment => {
                    protected[index] = true;
                    comment_lines.insert(line);
                    if bytes[index] == b'*' && following == Some(b'/') {
                        protected[index + 1] = true;
                        index += 2;
                        state = LexState::Code;
                        continue;
                    }
                }
                LexState::String | LexState::Character => {
                    protected[index] = true;
                    if bytes[index] == b'\\' && following.is_some() {
                        protected[index + 1] = true;
                        index += 2;
                        continue;
                    }
                    if (state == LexState::String && bytes[index] == b'"')
                        || (state == LexState::Character && bytes[index] == b'\'')
                    {
                        state = LexState::Code;
                    }
                }
            }
            if bytes[index] == b'\n' {
                line = line.saturating_add(1);
            }
            index += 1;
        }
        Self {
            protected,
            comment_lines,
        }
    }

    pub(crate) fn is_protected(&self, offset: usize) -> bool {
        self.protected.get(offset).copied().unwrap_or(false)
    }

    pub(crate) fn line_has_comment(&self, line: u32) -> bool {
        self.comment_lines.contains(&line)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LexState {
    Code,
    LineComment,
    BlockComment,
    String,
    Character,
}

pub(crate) fn escaped_physical_newline(bytes: &[u8], newline: usize) -> bool {
    if newline > 0 && bytes[newline - 1] == b'\\' {
        return true;
    }
    newline >= 3 && &bytes[newline - 3..newline] == b"??/"
}

pub(crate) fn leading_whitespace(text: &str) -> usize {
    text.bytes()
        .take_while(|byte| matches!(byte, b' ' | b'\t'))
        .count()
}

pub(crate) fn whitespace_before(text: &str, index: usize) -> (usize, usize) {
    let bytes = text.as_bytes();
    let mut start = index.min(bytes.len());
    while start > 0 && matches!(bytes[start - 1], b' ' | b'\t') {
        start -= 1;
    }
    (start, index.min(bytes.len()))
}

pub(crate) fn whitespace_after(text: &str, index: usize) -> (usize, usize) {
    let bytes = text.as_bytes();
    let start = index.min(bytes.len());
    let mut end = start;
    while end < bytes.len() && matches!(bytes[end], b' ' | b'\t') {
        end += 1;
    }
    (start, end)
}
