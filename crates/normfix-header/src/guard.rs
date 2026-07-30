//! Conservative recognition of canonical inclusion guards.

use std::sync::OnceLock;

use regex::Regex;
use sha2::{Digest, Sha256};

use crate::header::{c_header_span, line_spans};
use crate::{ByteRange, Fix};

/// One canonical whole-file `#ifndef`/`#define` inclusion guard.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalGuard {
    /// Macro shared by the opening directives.
    pub name: String,
    /// Macro token in `#ifndef`.
    pub ifndef_range: ByteRange,
    /// Macro token in `#define`.
    pub define_range: ByteRange,
    /// Source body inspected after the official header.
    pub body_range: ByteRange,
    /// SHA-256 of the exact inspected body.
    pub body_sha256: [u8; 32],
}

/// Snapshot-bound single-file rename candidate.
///
/// This is not project-wide authorization. Callers must prove that the old
/// macro has no consumers and that the expected macro has no collisions before
/// applying it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GuardRenameCandidate {
    /// Existing guard macro.
    pub current: String,
    /// Filename-derived guard macro.
    pub expected: String,
    /// Macro token in the `#ifndef`.
    pub ifndef_range: ByteRange,
    /// Macro token in the `#define`.
    pub define_range: ByteRange,
    /// SHA-256 of the exact inspected body.
    pub body_sha256: [u8; 32],
}

/// Derives the Norm guard spelling from a filename.
#[must_use]
pub fn expected_guard(filename: &str) -> String {
    filename
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect()
}

/// Recognizes one canonical whole-file guard, allowing surrounding comments.
///
/// # Panics
///
/// Caller input cannot cause a panic. Initialization would panic only if a
/// built-in regular-expression literal were invalid.
#[must_use]
pub fn canonical_guard(source: &str) -> Option<CanonicalGuard> {
    static IFNDEF: OnceLock<Regex> = OnceLock::new();
    static DEFINE: OnceLock<Regex> = OnceLock::new();
    static ENDIF: OnceLock<Regex> = OnceLock::new();
    static CONDITIONAL: OnceLock<Regex> = OnceLock::new();
    let body_start = guard_body_start(source);
    let body = &source[body_start..];
    let masked = mask_comments_and_literals(body);
    let raw_lines = line_spans(body);
    let masked_lines = line_spans(&masked);
    let nonempty = masked_lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| (!line.content(&masked).trim().is_empty()).then_some(index))
        .collect::<Vec<_>>();
    if nonempty.len() < 3 {
        return None;
    }
    let first = nonempty[0];
    let second = nonempty[1];
    let last = *nonempty.last()?;
    let ifndef_regex = IFNDEF.get_or_init(|| {
        Regex::new(r"^#\s*ifndef\s+([A-Za-z_][A-Za-z0-9_]*)\s*$").expect("constant guard regex")
    });
    let define_regex = DEFINE.get_or_init(|| {
        Regex::new(r"^#\s*define\s+([A-Za-z_][A-Za-z0-9_]*)\s*$").expect("constant guard regex")
    });
    let endif_regex =
        ENDIF.get_or_init(|| Regex::new(r"^#\s*endif\s*$").expect("constant guard regex"));
    let ifndef = ifndef_regex.captures(masked_lines[first].content(&masked))?;
    let define = define_regex.captures(masked_lines[second].content(&masked))?;
    let name = ifndef.get(1)?.as_str();
    if name != define.get(1)?.as_str() || !endif_regex.is_match(masked_lines[last].content(&masked))
    {
        return None;
    }

    let conditional = CONDITIONAL.get_or_init(|| {
        Regex::new(r"^#\s*(if|ifdef|ifndef|endif)\b").expect("constant conditional regex")
    });
    let mut depth = 0_i32;
    for (index, line) in masked_lines.iter().enumerate().take(last + 1).skip(first) {
        let Some(captures) = conditional.captures(line.content(&masked)) else {
            continue;
        };
        if captures.get(1)?.as_str() == "endif" {
            depth -= 1;
            if depth == 0 && index != last {
                return None;
            }
        } else {
            depth += 1;
        }
        if depth < 0 {
            return None;
        }
    }
    if depth != 0 {
        return None;
    }

    let ifndef_range = macro_range(body, raw_lines[first], "ifndef")?;
    let define_range = macro_range(body, raw_lines[second], "define")?;
    let digest: [u8; 32] = Sha256::digest(body.as_bytes()).into();
    Some(CanonicalGuard {
        name: name.to_owned(),
        ifndef_range: shift(ifndef_range, body_start),
        define_range: shift(define_range, body_start),
        body_range: ByteRange::new(body_start, source.len()),
        body_sha256: digest,
    })
}

/// Returns whether a canonical guard already matches the filename.
#[must_use]
pub fn header_guard_matches(source: &str, filename: &str) -> bool {
    let expected = expected_guard(filename);
    valid_identifier(&expected)
        && canonical_guard(source).is_some_and(|guard| guard.name == expected)
}

/// Builds a snapshot-bound rename candidate without claiming project safety.
#[must_use]
pub fn guard_rename_candidate(source: &str, filename: &str) -> Option<GuardRenameCandidate> {
    let expected = expected_guard(filename);
    if !valid_identifier(&expected) {
        return None;
    }
    let guard = canonical_guard(source)?;
    if guard.name == expected {
        return None;
    }
    Some(GuardRenameCandidate {
        current: guard.name,
        expected,
        ifndef_range: guard.ifndef_range,
        define_range: guard.define_range,
        body_sha256: guard.body_sha256,
    })
}

/// Applies an externally approved candidate only to the exact source snapshot.
///
/// Returns `None` if any guard spelling, range or body byte changed.
#[must_use]
pub fn apply_guard_rename(
    source: &str,
    approved: &GuardRenameCandidate,
) -> Option<(String, Vec<Fix>)> {
    let current = canonical_guard(source)?;
    if current.name != approved.current
        || current.ifndef_range != approved.ifndef_range
        || current.define_range != approved.define_range
        || current.body_sha256 != approved.body_sha256
        || !valid_identifier(&approved.expected)
    {
        return None;
    }
    let mut output = source.to_owned();
    for range in [approved.define_range, approved.ifndef_range] {
        if output.get(range.start..range.end)? != approved.current {
            return None;
        }
        output.replace_range(range.start..range.end, &approved.expected);
    }
    Some((
        output,
        vec![
            Fix {
                code: "HEADER_GUARD_RENAME",
                description: format!(
                    "renamed inclusion guard {} to {} after external project approval",
                    approved.current, approved.expected
                ),
                range: approved.ifndef_range,
            },
            Fix {
                code: "HEADER_GUARD_RENAME",
                description: format!(
                    "renamed inclusion guard {} to {} after external project approval",
                    approved.current, approved.expected
                ),
                range: approved.define_range,
            },
        ],
    ))
}

fn guard_body_start(source: &str) -> usize {
    let mut start = c_header_span(source).map_or(0, |range| range.end);
    while source
        .as_bytes()
        .get(start)
        .is_some_and(|byte| matches!(byte, b'\r' | b'\n'))
    {
        start += 1;
    }
    start
}

fn valid_identifier(value: &str) -> bool {
    let mut bytes = value.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    (first == b'_' || first.is_ascii_alphabetic())
        && bytes.all(|byte| byte == b'_' || byte.is_ascii_alphanumeric())
}

fn macro_range(body: &str, line: crate::header::LineSpan, directive: &str) -> Option<ByteRange> {
    let regex = Regex::new(&format!(
        r"^[ \t]*#[ \t]*{directive}[ \t]+([A-Za-z_][A-Za-z0-9_]*)"
    ))
    .ok()?;
    let captures = regex.captures(line.content(body))?;
    let name = captures.get(1)?;
    Some(ByteRange::new(
        line.start + name.start(),
        line.start + name.end(),
    ))
}

fn shift(range: ByteRange, amount: usize) -> ByteRange {
    ByteRange::new(range.start + amount, range.end + amount)
}

fn mask_comments_and_literals(source: &str) -> String {
    #[derive(Clone, Copy)]
    enum State {
        Code,
        LineComment,
        BlockComment,
        String,
        Character,
    }
    let bytes = source.as_bytes();
    let mut masked = bytes.to_vec();
    let mut state = State::Code;
    let mut escaped = false;
    let mut index = 0;
    while index < bytes.len() {
        match state {
            State::Code if bytes[index..].starts_with(b"//") => {
                masked[index] = b' ';
                masked[index + 1] = b' ';
                index += 2;
                state = State::LineComment;
                continue;
            }
            State::Code if bytes[index..].starts_with(b"/*") => {
                masked[index] = b' ';
                masked[index + 1] = b' ';
                index += 2;
                state = State::BlockComment;
                continue;
            }
            State::Code if bytes[index] == b'"' => {
                masked[index] = b' ';
                state = State::String;
                escaped = false;
            }
            State::Code if bytes[index] == b'\'' => {
                masked[index] = b' ';
                state = State::Character;
                escaped = false;
            }
            State::LineComment => {
                if bytes[index] == b'\n' || bytes[index] == b'\r' {
                    state = State::Code;
                } else {
                    masked[index] = b' ';
                }
            }
            State::BlockComment if bytes[index..].starts_with(b"*/") => {
                masked[index] = b' ';
                masked[index + 1] = b' ';
                index += 2;
                state = State::Code;
                continue;
            }
            State::BlockComment => {
                if bytes[index] != b'\n' && bytes[index] != b'\r' {
                    masked[index] = b' ';
                }
            }
            State::String | State::Character => {
                let terminator = match state {
                    State::String => b'"',
                    State::Character => b'\'',
                    _ => unreachable!("matched literal state"),
                };
                let byte = bytes[index];
                if byte != b'\n' && byte != b'\r' {
                    masked[index] = b' ';
                }
                if escaped {
                    escaped = false;
                } else if byte == b'\\' {
                    escaped = true;
                } else if byte == terminator {
                    state = State::Code;
                }
            }
            State::Code => {}
        }
        index += 1;
    }
    String::from_utf8(masked).expect("masking ASCII bytes preserves UTF-8")
}

#[cfg(test)]
mod tests {
    use crate::{Identity42, RunClock, build_c_header};

    use super::{
        apply_guard_rename, canonical_guard, expected_guard, guard_rename_candidate,
        header_guard_matches,
    };

    #[test]
    fn expected_name_uses_ascii_filename_rules() {
        assert_eq!(expected_guard("ft_demo.h"), "FT_DEMO_H");
        assert_eq!(expected_guard("42.h"), "42_H");
    }

    #[test]
    fn canonical_guard_allows_comments_around_the_guard() {
        let source = "// public API\n#ifndef FT_DEMO_H\n# define FT_DEMO_H\n\nint x;\n#endif\n"
            .to_owned()
            + "// end API\n";
        let guard = canonical_guard(&source).expect("canonical guard");
        assert_eq!(guard.name, "FT_DEMO_H");
        assert!(header_guard_matches(&source, "ft_demo.h"));
    }

    #[test]
    fn partial_guard_is_not_treated_as_whole_file_protection() {
        let source = "#ifndef FT_DEMO_H\n#define FT_DEMO_H\nint x;\n#endif\nint outside;\n";
        assert!(canonical_guard(source).is_none());
    }

    #[test]
    fn directives_in_comments_and_literals_do_not_change_guard_depth() {
        let source = concat!(
            "/* #ifndef FAKE */\n",
            "#ifndef FT_DEMO_H\n",
            "# define FT_DEMO_H\n",
            "# if ENABLED\n",
            "const char *text = \"#endif\";\n",
            "# endif\n",
            "#endif\n",
            "// #endif\n"
        );
        let guard = canonical_guard(source).expect("canonical nested guard");
        assert_eq!(guard.name, "FT_DEMO_H");
    }

    #[test]
    fn approved_candidate_is_bound_to_the_exact_snapshot() {
        let identity = Identity42 {
            login: "student".to_owned(),
            email: "student@student.42.fr".to_owned(),
            source: "test".to_owned(),
            inferred_login: false,
            inferred_email: false,
        };
        let clock = RunClock::fixed("2026/06/18 15:20:13").expect("clock");
        let header = build_c_header("ft_demo.h", &identity, &clock).expect("header");
        let source = format!("{header}\n\n#ifndef OLD_GUARD\n# define OLD_GUARD\nint x;\n#endif\n");
        let candidate = guard_rename_candidate(&source, "ft_demo.h").expect("candidate");
        let (renamed, fixes) = apply_guard_rename(&source, &candidate).expect("approved rename");
        assert!(renamed.contains("#ifndef FT_DEMO_H"));
        assert_eq!(fixes.len(), 2);
        assert!(apply_guard_rename(&(source + "\n"), &candidate).is_none());
    }
}
