//! Exact official 42 Makefile header support.

use std::sync::OnceLock;

use normfix_header::{ByteRange, Fix, Identity42, Issue, RunClock};
use regex::Regex;
use thiserror::Error;

/// Exact top and bottom edge of the official Makefile header.
pub const MAKEFILE_HEADER_EDGE: &str =
    "# **************************************************************************** #";

const TOP_SUFFIX: &str = ":::      ::::::::    ";
const FILE_SUFFIX: &str = ":+:      :+:    :+:    ";
const MIDDLE_SUFFIX: &str = "+:+ +:+         +:+      ";
const BY_SUFFIX: &str = "+#+  +:+       +#+         ";
const SEPARATOR_SUFFIX: &str = " +#+#+#+#+#+   +#+            ";
const CREATED_SUFFIX: &str = "#+#    #+#              ";
const UPDATED_SUFFIX: &str = "###   ########.fr        ";

/// Error preventing an exact 11×80 official Makefile header.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum MakefileHeaderError {
    /// Identity fields do not form one canonical matching 42 identity.
    #[error("the supplied identity is not a canonical matching 42 student identity")]
    InvalidIdentity,
    /// Filename contains whitespace/control text or is empty.
    #[error("filename `{0}` cannot be represented in the official Makefile header")]
    InvalidFilename(String),
    /// A field contains non-ASCII text, which cannot have a stable 80-column fit.
    #[error("{0} contains non-ASCII text")]
    NonAsciiField(&'static str),
    /// One field would exceed the fixed template.
    #[error("{field} needs {width} bytes but only {capacity} fit in the official Makefile header")]
    FieldTooLong {
        /// Field name.
        field: &'static str,
        /// Required byte width.
        width: usize,
        /// Template capacity.
        capacity: usize,
    },
}

/// Result of a Makefile-header insert or update.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MakefileHeaderTransform {
    /// Complete transformed source.
    pub output: String,
    /// Accepted edits with byte ranges in the input snapshot.
    pub fixes: Vec<Fix>,
    /// English issues that prevented a requested edit.
    pub issues: Vec<Issue>,
    /// Whether a new header was inserted.
    pub inserted: bool,
}

impl MakefileHeaderTransform {
    fn unchanged(source: &str) -> Self {
        Self {
            output: source.to_owned(),
            fixes: Vec::new(),
            issues: Vec::new(),
            inserted: false,
        }
    }
}

/// Returns whether a verified identity fits every fixed-width field.
#[must_use]
pub fn identity_fits_makefile_header(identity: &Identity42) -> bool {
    if !identity.is_valid() {
        return false;
    }
    let timestamp = "0000/00/00 00:00:00";
    [
        (
            format!("    By: {} <{}>", identity.login, identity.email),
            BY_SUFFIX,
        ),
        (
            format!("    Created: {timestamp} by {}", identity.login),
            CREATED_SUFFIX,
        ),
        (
            format!("    Updated: {timestamp} by {}", identity.login),
            UPDATED_SUFFIX,
        ),
    ]
    .iter()
    .all(|(left, right)| left.is_ascii() && left.len() <= 78_usize.saturating_sub(right.len()))
}

/// Returns whether filename and identity can produce an exact header.
#[must_use]
pub fn makefile_header_fits(filename: &str, identity: &Identity42) -> bool {
    validate_filename(filename).is_ok()
        && validate_field("filename", &format!("    {filename}"), FILE_SUFFIX).is_ok()
        && identity_fits_makefile_header(identity)
}

/// Builds an exact official 11-line, 80-byte Makefile header.
///
/// # Errors
///
/// Returns an error if any filename or identity field cannot be represented
/// exactly without truncation.
pub fn build_makefile_header(
    filename: &str,
    identity: &Identity42,
    clock: &RunClock,
) -> Result<String, MakefileHeaderError> {
    if !identity.is_valid() {
        return Err(MakefileHeaderError::InvalidIdentity);
    }
    validate_filename(filename)?;
    let timestamp = clock.timestamp();
    let file = format!("    {filename}");
    let by = format!("    By: {} <{}>", identity.login, identity.email);
    let created = format!("    Created: {timestamp} by {}", identity.login);
    let updated = format!("    Updated: {timestamp} by {}", identity.login);
    validate_field("filename", &file, FILE_SUFFIX)?;
    validate_field("author", &by, BY_SUFFIX)?;
    validate_field("created metadata", &created, CREATED_SUFFIX)?;
    validate_field("updated metadata", &updated, UPDATED_SUFFIX)?;

    let lines = [
        MAKEFILE_HEADER_EDGE.to_owned(),
        framed("", "")?,
        framed("", TOP_SUFFIX)?,
        framed(&file, FILE_SUFFIX)?,
        framed("", MIDDLE_SUFFIX)?,
        framed(&by, BY_SUFFIX)?,
        framed("", SEPARATOR_SUFFIX)?,
        framed(&created, CREATED_SUFFIX)?,
        framed(&updated, UPDATED_SUFFIX)?,
        framed("", "")?,
        MAKEFILE_HEADER_EDGE.to_owned(),
    ];
    debug_assert!(lines.iter().all(|line| line.len() == 80));
    Ok(lines.join("\n"))
}

/// Returns the byte span occupied by a header-like block at byte zero.
#[must_use]
pub fn makefile_header_span(source: &str) -> Option<ByteRange> {
    let lines = line_spans(source);
    if lines.len() < 11
        || lines[0].content(source) != MAKEFILE_HEADER_EDGE
        || lines[10].content(source) != MAKEFILE_HEADER_EDGE
    {
        return None;
    }
    Some(ByteRange::new(0, lines[10].end))
}

/// Returns whether the leading block has the exact official shape.
///
/// # Panics
///
/// Caller input cannot cause a panic. Initialization would panic only if a
/// built-in regular-expression literal were invalid.
#[must_use]
pub fn makefile_header_is_valid(source: &str) -> bool {
    static FILE: OnceLock<Regex> = OnceLock::new();
    static BY: OnceLock<Regex> = OnceLock::new();
    static CREATED: OnceLock<Regex> = OnceLock::new();
    static UPDATED: OnceLock<Regex> = OnceLock::new();
    let Some(span) = makefile_header_span(source) else {
        return false;
    };
    let lines = line_spans(&source[span.start..span.end]);
    if lines.len() != 11 || lines.iter().any(|line| line.content(source).len() != 80) {
        return false;
    }
    let contents = lines
        .iter()
        .map(|line| line.content(source))
        .collect::<Vec<_>>();
    let empty = framed("", "").expect("fixed frame fits");
    let top = framed("", TOP_SUFFIX).expect("fixed frame fits");
    let middle = framed("", MIDDLE_SUFFIX).expect("fixed frame fits");
    let separator = framed("", SEPARATOR_SUFFIX).expect("fixed frame fits");
    let fixed = [
        (0, MAKEFILE_HEADER_EDGE),
        (1, empty.as_str()),
        (2, top.as_str()),
        (4, middle.as_str()),
        (6, separator.as_str()),
        (9, empty.as_str()),
        (10, MAKEFILE_HEADER_EDGE),
    ];
    if fixed
        .iter()
        .any(|(index, expected)| contents[*index] != *expected)
    {
        return false;
    }
    let file = FILE.get_or_init(|| Regex::new(r"^#    \S+").expect("constant header regex"));
    let by =
        BY.get_or_init(|| Regex::new(r"^#    By: \S+ <[^<> ]+>").expect("constant header regex"));
    let created = CREATED.get_or_init(|| {
        Regex::new(r"^#    Created: \d{4}/\d{2}/\d{2} \d{2}:\d{2}:\d{2} by \S+")
            .expect("constant header regex")
    });
    let updated = UPDATED.get_or_init(|| {
        Regex::new(r"^#    Updated: \d{4}/\d{2}/\d{2} \d{2}:\d{2}:\d{2} by \S+")
            .expect("constant header regex")
    });
    [
        (3, file, FILE_SUFFIX),
        (5, by, BY_SUFFIX),
        (7, created, CREATED_SUFFIX),
        (8, updated, UPDATED_SUFFIX),
    ]
    .iter()
    .all(|(index, regex, suffix)| {
        regex.is_match(contents[*index]) && contents[*index].ends_with(&format!("{suffix}#"))
    })
}

/// Inserts a missing or malformed header without deleting existing source.
#[must_use]
pub fn ensure_makefile_header(
    source: &str,
    filename: &str,
    identity: Option<&Identity42>,
    clock: &RunClock,
) -> MakefileHeaderTransform {
    if makefile_header_is_valid(source) {
        return MakefileHeaderTransform::unchanged(source);
    }
    let Some(identity) = identity else {
        return blocked(
            source,
            "INVALID_HEADER",
            "The official 42 Makefile header is missing or malformed.",
            "Configure a verified 42 student email so the header can be inserted safely.",
        );
    };
    let header = match build_makefile_header(filename, identity, clock) {
        Ok(header) => header,
        Err(error) => {
            return blocked(
                source,
                "HEADER_FIELD_TOO_LONG",
                &format!("Official Makefile header not added: {error}."),
                "Shorten the filename or use a verified identity that fits without truncation.",
            );
        }
    };
    MakefileHeaderTransform {
        output: format!("{header}\n\n{source}"),
        fixes: vec![Fix {
            code: "INVALID_HEADER",
            description: "inserted the official 42 Makefile header".to_owned(),
            range: ByteRange::new(0, 0),
        }],
        issues: Vec::new(),
        inserted: true,
    }
}

/// Updates filename and `Updated`, preserving author and `Created`.
#[must_use]
pub fn update_makefile_header(
    source: &str,
    filename: &str,
    identity: Option<&Identity42>,
    clock: &RunClock,
) -> MakefileHeaderTransform {
    let Some(identity) = identity else {
        return MakefileHeaderTransform::unchanged(source);
    };
    if !makefile_header_is_valid(source) {
        return MakefileHeaderTransform::unchanged(source);
    }
    let file = format!("    {filename}");
    let updated = format!("    Updated: {} by {}", clock.timestamp(), identity.login);
    if let Err(error) = validate_filename(filename)
        .and_then(|()| validate_field("filename", &file, FILE_SUFFIX))
        .and_then(|()| validate_field("updated metadata", &updated, UPDATED_SUFFIX))
    {
        return blocked(
            source,
            "HEADER_FIELD_TOO_LONG",
            &format!("Official Makefile header not updated: {error}."),
            "Use a filename and verified identity that fit without truncation.",
        );
    }

    let lines = line_spans(source);
    let Some(span) = makefile_header_span(source) else {
        return MakefileHeaderTransform::unchanged(source);
    };
    let Ok(file_line) = framed(&file, FILE_SUFFIX) else {
        return MakefileHeaderTransform::unchanged(source);
    };
    let Ok(updated_line) = framed(&updated, UPDATED_SUFFIX) else {
        return MakefileHeaderTransform::unchanged(source);
    };
    let old_file = lines[3].content(source);
    let old_updated = lines[8].content(source);
    if old_file == file_line && old_updated == updated_line {
        return MakefileHeaderTransform::unchanged(source);
    }

    let mut block = lines[..11]
        .iter()
        .map(|line| line.content(source).to_owned())
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
            description: "updated the official Makefile header filename".to_owned(),
            range: lines[3].content_range(),
        });
    }
    if old_updated != block[8] {
        fixes.push(Fix {
            code: "UPDATE_HEADER_TIMESTAMP",
            description: "updated the official Makefile modification metadata".to_owned(),
            range: lines[8].content_range(),
        });
    }
    MakefileHeaderTransform {
        output,
        fixes,
        issues: Vec::new(),
        inserted: false,
    }
}

/// Returns whether a valid header contains the requested filename.
#[must_use]
pub fn makefile_header_filename_matches(source: &str, filename: &str) -> bool {
    if !makefile_header_is_valid(source) {
        return false;
    }
    framed(&format!("    {filename}"), FILE_SUFFIX)
        .ok()
        .is_some_and(|expected| line_spans(source)[3].content(source) == expected)
}

fn blocked(
    source: &str,
    code: &'static str,
    message: &str,
    suggestion: &str,
) -> MakefileHeaderTransform {
    MakefileHeaderTransform {
        output: source.to_owned(),
        fixes: Vec::new(),
        issues: vec![Issue {
            code,
            message: message.to_owned(),
            range: makefile_header_span(source).unwrap_or(ByteRange::new(0, 0)),
            suggestion: suggestion.to_owned(),
        }],
        inserted: false,
    }
}

fn validate_filename(filename: &str) -> Result<(), MakefileHeaderError> {
    if filename.is_empty() || filename.chars().any(char::is_whitespace) {
        return Err(MakefileHeaderError::InvalidFilename(filename.to_owned()));
    }
    if !filename.is_ascii() {
        return Err(MakefileHeaderError::NonAsciiField("filename"));
    }
    Ok(())
}

fn validate_field(field: &'static str, left: &str, right: &str) -> Result<(), MakefileHeaderError> {
    if !left.is_ascii() {
        return Err(MakefileHeaderError::NonAsciiField(field));
    }
    let capacity = 78_usize.saturating_sub(right.len());
    if left.len() > capacity {
        return Err(MakefileHeaderError::FieldTooLong {
            field,
            width: left.len(),
            capacity,
        });
    }
    Ok(())
}

fn framed(left: &str, right: &str) -> Result<String, MakefileHeaderError> {
    validate_field("header field", left, right)?;
    Ok(format!(
        "#{left}{}{right}#",
        " ".repeat(78 - left.len() - right.len())
    ))
}

#[derive(Clone, Copy, Debug)]
struct LineSpan {
    start: usize,
    content_end: usize,
    end: usize,
}

impl LineSpan {
    fn content(self, source: &str) -> &str {
        &source[self.start..self.content_end]
    }

    const fn content_range(self) -> ByteRange {
        ByteRange::new(self.start, self.content_end)
    }
}

fn line_spans(source: &str) -> Vec<LineSpan> {
    let bytes = source.as_bytes();
    let mut spans = Vec::new();
    let mut start = 0;
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'\n' {
            let content_end =
                index.saturating_sub(usize::from(index > start && bytes[index - 1] == b'\r'));
            spans.push(LineSpan {
                start,
                content_end,
                end: index + 1,
            });
            start = index + 1;
        } else if bytes[index] == b'\r' {
            let end = if bytes.get(index + 1) == Some(&b'\n') {
                index + 2
            } else {
                index + 1
            };
            spans.push(LineSpan {
                start,
                content_end: index,
                end,
            });
            start = end;
            index = end.saturating_sub(1);
        }
        index += 1;
    }
    if start < source.len() {
        spans.push(LineSpan {
            start,
            content_end: source.len(),
            end: source.len(),
        });
    }
    spans
}

#[cfg(test)]
mod tests {
    use normfix_header::{Identity42, RunClock};

    use super::{
        MAKEFILE_HEADER_EDGE, MakefileHeaderError, build_makefile_header, ensure_makefile_header,
        makefile_header_is_valid, makefile_header_span, update_makefile_header,
    };

    fn identity() -> Identity42 {
        Identity42 {
            login: "student-a".to_owned(),
            email: "student-a@student.42.fr".to_owned(),
            source: "test".to_owned(),
            inferred_login: false,
            inferred_email: false,
        }
    }

    fn clock(value: &str) -> RunClock {
        RunClock::fixed(value).expect("valid test clock")
    }

    #[test]
    fn official_header_is_exactly_eleven_by_eighty() {
        let header = build_makefile_header("Makefile", &identity(), &clock("2026/06/18 15:20:13"))
            .expect("header fits");
        let lines = header.lines().collect::<Vec<_>>();
        assert_eq!(lines.len(), 11);
        assert!(lines.iter().all(|line| line.len() == 80));
        assert_eq!(lines[0], MAKEFILE_HEADER_EDGE);
        assert!(lines[5].contains("By: student-a <student-a@student.42.fr>"));
        assert!(makefile_header_is_valid(&header));
        assert_eq!(
            makefile_header_span(&(header + "\n")).map(|range| range.start),
            Some(0)
        );
    }

    #[test]
    fn official_header_matches_the_audited_python_template_byte_for_byte() {
        let expected = concat!(
            "# **************************************************************************** #\n",
            "#                                                                              #\n",
            "#                                                         :::      ::::::::    #\n",
            "#    Makefile                                           :+:      :+:    :+:    #\n",
            "#                                                     +:+ +:+         +:+      #\n",
            "#    By: student-a <student-a@student.42.fr>        +#+  +:+       +#+         #\n",
            "#                                                 +#+#+#+#+#+   +#+            #\n",
            "#    Created: 2026/06/18 15:20:13 by student-a         #+#    #+#              #\n",
            "#    Updated: 2026/06/18 15:20:13 by student-a        ###   ########.fr        #\n",
            "#                                                                              #\n",
            "# **************************************************************************** #"
        );
        assert_eq!(
            build_makefile_header("Makefile", &identity(), &clock("2026/06/18 15:20:13"))
                .expect("header"),
            expected
        );
    }

    #[test]
    fn long_filename_is_refused_instead_of_truncated_or_panicking() {
        let filename = format!("{}.c", "x".repeat(80));
        let error = build_makefile_header(&filename, &identity(), &clock("2026/06/18 15:20:13"))
            .expect_err("filename cannot fit");
        assert!(matches!(error, MakefileHeaderError::FieldTooLong { .. }));
        let source = "NAME = demo\n";
        let transform = ensure_makefile_header(
            source,
            &filename,
            Some(&identity()),
            &clock("2026/06/18 15:20:13"),
        );
        assert_eq!(transform.output, source);
        assert_eq!(transform.issues[0].code, "HEADER_FIELD_TOO_LONG");
    }

    #[test]
    fn valid_header_preserves_author_and_created_metadata() {
        let original = build_makefile_header("Oldfile", &identity(), &clock("2026/06/18 15:20:13"))
            .expect("header fits")
            + "\n\nNAME = demo\n";
        let updated = update_makefile_header(
            &original,
            "Makefile",
            Some(&identity()),
            &clock("2026/06/18 16:16:36"),
        );
        assert!(updated.output.contains("Created: 2026/06/18 15:20:13"));
        assert!(updated.output.contains("Updated: 2026/06/18 16:16:36"));
        assert!(updated.output.contains("    Makefile"));
        assert_eq!(updated.fixes.len(), 2);
    }

    #[test]
    fn malformed_prefix_is_preserved_when_a_new_header_is_added() {
        let malformed = "# **************************************************************************** #\nnot a header\n";
        let transform = ensure_makefile_header(
            malformed,
            "Makefile",
            Some(&identity()),
            &clock("2026/06/18 15:20:13"),
        );
        assert!(transform.inserted);
        assert!(transform.output.ends_with(malformed));
    }
}
