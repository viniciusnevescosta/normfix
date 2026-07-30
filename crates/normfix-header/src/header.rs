//! Exact official 42 C/header comment block.

use std::sync::OnceLock;

use regex::Regex;
use thiserror::Error;

use crate::{ByteRange, Fix, Identity42, Issue, RunClock};

/// Exact top and bottom edge of the official C header.
pub const C_HEADER_EDGE: &str =
    "/* ************************************************************************** */";

const FILE_SUFFIX: &str = ":+:      :+:    :+:   ";
const BY_SUFFIX: &str = "+#+  +:+       +#+        ";
const CREATED_SUFFIX: &str = "#+#    #+#             ";
const UPDATED_SUFFIX: &str = "###   ########.fr       ";

/// Result of inserting or updating one official header.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HeaderTransform {
    /// Complete transformed source.
    pub output: String,
    /// Accepted fixes with ranges in the input snapshot.
    pub fixes: Vec<Fix>,
    /// English issues that prevented a requested edit.
    pub issues: Vec<Issue>,
    /// Whether this operation inserted a new header.
    pub inserted: bool,
}

impl HeaderTransform {
    fn unchanged(source: &str) -> Self {
        Self {
            output: source.to_owned(),
            fixes: Vec::new(),
            issues: Vec::new(),
            inserted: false,
        }
    }

    /// Returns whether the source changed.
    #[must_use]
    pub fn changed(&self, input: &str) -> bool {
        self.output != input
    }
}

/// Error preventing an exact 11×80 official header.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum HeaderBuildError {
    /// No verified identity was supplied.
    #[error("a verified 42 student identity is required")]
    IdentityUnavailable,
    /// Identity fields do not form one canonical matching 42 identity.
    #[error("the supplied identity is not a canonical matching 42 student identity")]
    InvalidIdentity,
    /// Filename contains whitespace/control text or is empty.
    #[error("filename `{0}` cannot be represented in the official header")]
    InvalidFilename(String),
    /// A field contains non-ASCII text, which has no stable template width.
    #[error("{0} contains non-ASCII text")]
    NonAsciiField(&'static str),
    /// One field would exceed the fixed template.
    #[error("{field} needs {width} bytes but only {capacity} fit in the official header")]
    FieldTooLong {
        /// Field name.
        field: &'static str,
        /// Required ASCII/UTF-8 byte width.
        width: usize,
        /// Template capacity.
        capacity: usize,
    },
}

/// Returns whether the validated identity fits every author/timestamp field.
#[must_use]
pub fn identity_fits_c_header(identity: &Identity42) -> bool {
    if !identity.is_valid() {
        return false;
    }
    let timestamp = "0000/00/00 00:00:00";
    let fields = [
        (
            format!("   By: {} <{}>", identity.login, identity.email),
            BY_SUFFIX,
        ),
        (
            format!("   Created: {timestamp} by {}", identity.login),
            CREATED_SUFFIX,
        ),
        (
            format!("   Updated: {timestamp} by {}", identity.login),
            UPDATED_SUFFIX,
        ),
    ];
    fields
        .iter()
        .all(|(left, right)| left.is_ascii() && left.len() <= 76_usize.saturating_sub(right.len()))
}

/// Returns whether filename and identity can produce an exact header.
#[must_use]
pub fn c_header_fits(filename: &str, identity: &Identity42) -> bool {
    valid_filename(filename)
        && field_fits(&format!("   {filename}"), FILE_SUFFIX)
        && identity_fits_c_header(identity)
}

/// Builds the exact official 11-line, 80-byte C header.
///
/// # Errors
///
/// Returns an error if the filename or identity cannot be represented exactly
/// without truncation.
pub fn build_c_header(
    filename: &str,
    identity: &Identity42,
    clock: &RunClock,
) -> Result<String, HeaderBuildError> {
    if !identity.is_valid() {
        return Err(HeaderBuildError::InvalidIdentity);
    }
    validate_filename(filename)?;
    validate_field("filename", &format!("   {filename}"), FILE_SUFFIX)?;
    let timestamp = clock.timestamp();
    let by = format!("   By: {} <{}>", identity.login, identity.email);
    let created = format!("   Created: {timestamp} by {}", identity.login);
    let updated = format!("   Updated: {timestamp} by {}", identity.login);
    validate_field("author", &by, BY_SUFFIX)?;
    validate_field("created metadata", &created, CREATED_SUFFIX)?;
    validate_field("updated metadata", &updated, UPDATED_SUFFIX)?;

    let separator_art = "+#".repeat(5) + "+   +#+           ";
    let lines = [
        C_HEADER_EDGE.to_owned(),
        framed("", "")?,
        framed("", ":::      ::::::::   ")?,
        framed(&format!("   {filename}"), FILE_SUFFIX)?,
        framed("", "+:+ +:+         +:+     ")?,
        framed(&by, BY_SUFFIX)?,
        framed("", &separator_art)?,
        framed(&created, CREATED_SUFFIX)?,
        framed(&updated, UPDATED_SUFFIX)?,
        framed("", "")?,
        C_HEADER_EDGE.to_owned(),
    ];
    debug_assert!(lines.iter().all(|line| line.len() == 80));
    Ok(lines.join("\n"))
}

/// Returns the byte span occupied by a header-like 11-line block at byte zero.
#[must_use]
pub fn c_header_span(source: &str) -> Option<ByteRange> {
    let lines = line_spans(source);
    if lines.len() < 11 {
        return None;
    }
    if lines[0].content(source) != C_HEADER_EDGE || lines[10].content(source) != C_HEADER_EDGE {
        return None;
    }
    Some(ByteRange::new(0, lines[10].end))
}

/// Inserts a missing/malformed header without deleting any header-like prefix.
#[must_use]
pub fn ensure_c_header(
    source: &str,
    filename: &str,
    identity: Option<&Identity42>,
    clock: &RunClock,
) -> HeaderTransform {
    if c_header_is_valid(source) {
        return HeaderTransform::unchanged(source);
    }
    let Some(identity) = identity else {
        return blocked_transform(
            source,
            "INVALID_HEADER",
            "The official 42 header is missing or malformed.",
            "Configure one verified 42 student email so the header can be inserted safely.",
        );
    };
    let header = match build_c_header(filename, identity, clock) {
        Ok(header) => header,
        Err(error) => {
            return blocked_transform(
                source,
                "HEADER_FIELD_TOO_LONG",
                &format!("Official header not added: {error}."),
                "Shorten the filename or use a verified identity that fits without truncation.",
            );
        }
    };
    HeaderTransform {
        output: format!("{header}\n\n{source}"),
        fixes: vec![Fix {
            code: "INVALID_HEADER",
            description: "inserted the official 42 header".to_owned(),
            range: ByteRange::new(0, 0),
        }],
        issues: Vec::new(),
        inserted: true,
    }
}

/// Updates only filename and `Updated`, preserving author and `Created`.
#[must_use]
pub fn update_c_header(
    source: &str,
    filename: &str,
    identity: Option<&Identity42>,
    clock: &RunClock,
) -> HeaderTransform {
    let Some(identity) = identity else {
        return HeaderTransform::unchanged(source);
    };
    if !c_header_is_valid(source) {
        return HeaderTransform::unchanged(source);
    }
    if let Err(error) = validate_filename(filename)
        .and_then(|()| validate_field("filename", &format!("   {filename}"), FILE_SUFFIX))
        .and_then(|()| {
            validate_field(
                "updated metadata",
                &format!("   Updated: {} by {}", clock.timestamp(), identity.login),
                UPDATED_SUFFIX,
            )
        })
    {
        return blocked_transform(
            source,
            "HEADER_FIELD_TOO_LONG",
            &format!("Official header not updated: {error}."),
            "Use a filename and verified identity that fit without truncation.",
        );
    }

    let lines = line_spans(source);
    let Some(span) = c_header_span(source) else {
        return HeaderTransform::unchanged(source);
    };
    let Ok(file_line) = framed(&format!("   {filename}"), FILE_SUFFIX) else {
        return HeaderTransform::unchanged(source);
    };
    let Ok(updated_line) = framed(
        &format!("   Updated: {} by {}", clock.timestamp(), identity.login),
        UPDATED_SUFFIX,
    ) else {
        return HeaderTransform::unchanged(source);
    };
    let old_file = lines[3].content(source);
    let old_updated = lines[8].content(source);
    if old_file == file_line && old_updated == updated_line {
        return HeaderTransform::unchanged(source);
    }

    let mut block = source[span.start..span.end]
        .trim_end_matches(['\r', '\n'])
        .lines()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    block[3] = file_line;
    block[8] = updated_line;
    let replacement = block.join("\n") + "\n";
    let mut output = String::with_capacity(source.len() + replacement.len());
    output.push_str(&replacement);
    output.push_str(&source[span.end..]);
    let mut fixes = Vec::new();
    if old_file != block[3] {
        fixes.push(Fix {
            code: "UPDATE_HEADER_FILENAME",
            description: "updated the official header filename".to_owned(),
            range: lines[3].content_range(),
        });
    }
    if old_updated != block[8] {
        fixes.push(Fix {
            code: "UPDATE_HEADER_TIMESTAMP",
            description: "updated the official header modification metadata".to_owned(),
            range: lines[8].content_range(),
        });
    }
    HeaderTransform {
        output,
        fixes,
        issues: Vec::new(),
        inserted: false,
    }
}

/// Returns whether a valid existing header contains the expected filename.
#[must_use]
pub fn c_header_filename_matches(source: &str, filename: &str) -> bool {
    if !c_header_is_valid(source) {
        return false;
    }
    framed(&format!("   {filename}"), FILE_SUFFIX)
        .ok()
        .is_some_and(|expected| line_spans(source)[3].content(source) == expected)
}

pub(crate) fn c_header_is_valid(source: &str) -> bool {
    static FILE: OnceLock<Regex> = OnceLock::new();
    static BY: OnceLock<Regex> = OnceLock::new();
    static CREATED: OnceLock<Regex> = OnceLock::new();
    static UPDATED: OnceLock<Regex> = OnceLock::new();
    let Some(_span) = c_header_span(source) else {
        return false;
    };
    let spans = line_spans(source);
    let lines = spans
        .iter()
        .take(11)
        .map(|line| line.content(source))
        .collect::<Vec<_>>();
    if lines.iter().any(|line| line.len() != 80)
        || lines[1] != framed("", "").expect("empty frame fits")
        || lines[9] != framed("", "").expect("empty frame fits")
        || lines[2] != framed("", ":::      ::::::::   ").expect("fixed frame fits")
        || lines[4] != framed("", "+:+ +:+         +:+     ").expect("fixed frame fits")
        || lines[6]
            != framed("", &("+#".repeat(5) + "+   +#+           ")).expect("fixed frame fits")
    {
        return false;
    }
    FILE.get_or_init(|| Regex::new(r"^/\*   \S+").expect("constant regex"))
        .is_match(lines[3])
        && lines[3].ends_with(&format!("{FILE_SUFFIX}*/"))
        && BY
            .get_or_init(|| Regex::new(r"^/\*   By: \S+ <[^<> ]+>").expect("constant regex"))
            .is_match(lines[5])
        && lines[5].ends_with(&format!("{BY_SUFFIX}*/"))
        && CREATED
            .get_or_init(|| {
                Regex::new(r"^/\*   Created: \d{4}/\d{2}/\d{2} \d{2}:\d{2}:\d{2} by \S+")
                    .expect("constant regex")
            })
            .is_match(lines[7])
        && lines[7].ends_with(&format!("{CREATED_SUFFIX}*/"))
        && UPDATED
            .get_or_init(|| {
                Regex::new(r"^/\*   Updated: \d{4}/\d{2}/\d{2} \d{2}:\d{2}:\d{2} by \S+")
                    .expect("constant regex")
            })
            .is_match(lines[8])
        && lines[8].ends_with(&format!("{UPDATED_SUFFIX}*/"))
}

fn blocked_transform(
    source: &str,
    code: &'static str,
    message: &str,
    suggestion: &str,
) -> HeaderTransform {
    HeaderTransform {
        output: source.to_owned(),
        fixes: Vec::new(),
        issues: vec![Issue {
            code,
            message: message.to_owned(),
            range: c_header_span(source).unwrap_or_default(),
            suggestion: suggestion.to_owned(),
        }],
        inserted: false,
    }
}

fn validate_filename(filename: &str) -> Result<(), HeaderBuildError> {
    if !valid_filename(filename) {
        return Err(HeaderBuildError::InvalidFilename(filename.to_owned()));
    }
    Ok(())
}

fn valid_filename(filename: &str) -> bool {
    !filename.is_empty()
        && filename.is_ascii()
        && !filename
            .chars()
            .any(|character| character.is_whitespace() || character.is_control())
}

fn field_fits(left: &str, right: &str) -> bool {
    left.len() <= 76_usize.saturating_sub(right.len())
}

fn validate_field(field: &'static str, left: &str, right: &str) -> Result<(), HeaderBuildError> {
    if !left.is_ascii() {
        return Err(HeaderBuildError::NonAsciiField(field));
    }
    let capacity = 76_usize.saturating_sub(right.len());
    if left.len() > capacity {
        return Err(HeaderBuildError::FieldTooLong {
            field,
            width: left.len(),
            capacity,
        });
    }
    Ok(())
}

fn framed(left: &str, right: &str) -> Result<String, HeaderBuildError> {
    validate_field("header field", left, right)?;
    Ok(format!(
        "/*{left}{}{right}*/",
        " ".repeat(76 - left.len() - right.len())
    ))
}

#[derive(Clone, Copy)]
pub(crate) struct LineSpan {
    pub(crate) start: usize,
    pub(crate) content_end: usize,
    pub(crate) end: usize,
}

impl LineSpan {
    pub(crate) fn content(self, source: &str) -> &str {
        &source[self.start..self.content_end]
    }

    fn content_range(self) -> ByteRange {
        ByteRange::new(self.start, self.content_end)
    }
}

pub(crate) fn line_spans(source: &str) -> Vec<LineSpan> {
    let bytes = source.as_bytes();
    let mut lines = Vec::new();
    let mut start = 0;
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'\n' {
            let content_end =
                index.saturating_sub(usize::from(index > start && bytes[index - 1] == b'\r'));
            lines.push(LineSpan {
                start,
                content_end,
                end: index + 1,
            });
            start = index + 1;
        } else if bytes[index] == b'\r' && bytes.get(index + 1).is_none_or(|next| *next != b'\n') {
            lines.push(LineSpan {
                start,
                content_end: index,
                end: index + 1,
            });
            start = index + 1;
        }
        index += 1;
    }
    if start < source.len() {
        lines.push(LineSpan {
            start,
            content_end: source.len(),
            end: source.len(),
        });
    }
    lines
}

#[cfg(test)]
mod tests {
    use crate::{Identity42, RunClock};

    use super::{
        C_HEADER_EDGE, build_c_header, c_header_filename_matches, c_header_span, ensure_c_header,
        update_c_header,
    };

    fn identity() -> Identity42 {
        Identity42 {
            login: "vncosta".to_owned(),
            email: "vncosta@student.42sp.org".to_owned(),
            source: "test".to_owned(),
            inferred_login: false,
            inferred_email: false,
        }
    }

    #[test]
    fn official_header_is_exactly_eleven_by_eighty() {
        let clock = RunClock::fixed("2026/07/23 12:34:56").expect("clock");
        let header = build_c_header("main.c", &identity(), &clock).expect("header");
        let lines = header.lines().collect::<Vec<_>>();
        assert_eq!(lines.len(), 11);
        assert!(lines.iter().all(|line| line.len() == 80));
        assert_eq!(lines[0], C_HEADER_EDGE);
        assert_eq!(lines[10], C_HEADER_EDGE);
        assert!(lines[5].contains("By: vncosta <vncosta@student.42sp.org>"));
    }

    #[test]
    fn official_header_matches_the_audited_python_template_byte_for_byte() {
        let identity = Identity42 {
            login: "student-a".to_owned(),
            email: "student-a@student.42.fr".to_owned(),
            source: "test".to_owned(),
            inferred_login: false,
            inferred_email: false,
        };
        let clock = RunClock::fixed("2026/06/18 15:20:13").expect("clock");
        let expected = concat!(
            "/* ************************************************************************** */\n",
            "/*                                                                            */\n",
            "/*                                                        :::      ::::::::   */\n",
            "/*   main.c                                             :+:      :+:    :+:   */\n",
            "/*                                                    +:+ +:+         +:+     */\n",
            "/*   By: student-a <student-a@student.42.fr>        +#+  +:+       +#+        */\n",
            "/*                                                +#+#+#+#+#+   +#+           */\n",
            "/*   Created: 2026/06/18 15:20:13 by student-a         #+#    #+#             */\n",
            "/*   Updated: 2026/06/18 15:20:13 by student-a        ###   ########.fr       */\n",
            "/*                                                                            */\n",
            "/* ************************************************************************** */"
        );
        assert_eq!(
            build_c_header("main.c", &identity, &clock).expect("header"),
            expected
        );
    }

    #[test]
    fn insertion_is_idempotent_and_preserves_malformed_prefixes() {
        let clock = RunClock::fixed("2026/07/23 12:34:56").expect("clock");
        let malformed = format!("{C_HEADER_EDGE}\n/* malformed */\nint keep_me;\n");
        let first = ensure_c_header(&malformed, "main.c", Some(&identity()), &clock);
        assert!(first.inserted);
        assert!(first.output.contains("int keep_me;"));
        let second = ensure_c_header(&first.output, "main.c", Some(&identity()), &clock);
        assert!(!second.changed(&first.output));
    }

    #[test]
    fn valid_header_preserves_created_and_updates_only_mutable_metadata() {
        let created = RunClock::fixed("2026/07/23 12:34:56").expect("clock");
        let updated = RunClock::fixed("2026/07/23 13:00:00").expect("clock");
        let source = build_c_header("old.c", &identity(), &created).expect("header")
            + "\n\nint\tmain(void);\n";
        let result = update_c_header(&source, "main.c", Some(&identity()), &updated);
        assert!(result.output.contains("Created: 2026/07/23 12:34:56"));
        assert!(result.output.contains("Updated: 2026/07/23 13:00:00"));
        assert!(c_header_filename_matches(&result.output, "main.c"));
        assert_eq!(c_header_span(&result.output).expect("span").start, 0);
    }

    #[test]
    fn overlong_filename_is_refused_without_truncation() {
        let clock = RunClock::fixed("2026/07/23 12:34:56").expect("clock");
        let filename = format!("{}.c", "x".repeat(70));
        let result = ensure_c_header("int x;\n", &filename, Some(&identity()), &clock);
        assert!(!result.changed("int x;\n"));
        assert_eq!(result.issues[0].code, "HEADER_FIELD_TOO_LONG");
        assert!(!result.output.contains("..."));
    }
}
