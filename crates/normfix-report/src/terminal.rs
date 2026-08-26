use std::borrow::Cow;
use std::fmt::Write as _;

use camino::Utf8Path;
use normfix_core::{Diagnostic, DiagnosticSource};

/// Returns the text a reader should see for a diagnostic.
///
/// A diagnostic authored by this project carries a translation; one relayed
/// from the official checker or the C compiler does not, and is shown exactly
/// as that tool produced it.
pub fn reader_text(diagnostic: &Diagnostic) -> (&str, &[String], Option<&String>) {
    diagnostic.localized.as_ref().map_or_else(
        || {
            (
                diagnostic.message.as_str(),
                diagnostic.notes.as_slice(),
                diagnostic.help.as_ref(),
            )
        },
        |localized| {
            (
                localized.message.as_str(),
                localized.notes.as_slice(),
                localized.help.as_ref(),
            )
        },
    )
}

pub fn source_label(source: &DiagnosticSource) -> String {
    match source {
        DiagnosticSource::NativeNorm41 => "Norm v4.1 native rule".to_owned(),
        DiagnosticSource::NorminetteCompat(version) => {
            format!(
                "official Norminette {} compatibility",
                terminal_safe_inline(version)
            )
        }
        DiagnosticSource::Parser => "C parser".to_owned(),
        DiagnosticSource::Compiler => "C compiler".to_owned(),
        DiagnosticSource::Project => "project safety check".to_owned(),
        DiagnosticSource::Makefile => "Makefile check".to_owned(),
        DiagnosticSource::Markdown => "Markdown check".to_owned(),
        DiagnosticSource::LeakChecker => "leak checker".to_owned(),
    }
}

pub fn safe_path(path: &Utf8Path) -> String {
    terminal_safe_inline(path.as_str())
}

pub fn terminal_safe_inline(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    for character in input.chars() {
        match character {
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character if character.is_control() || is_bidirectional_control(character) => {
                push_control_escape(&mut output, character);
            }
            character => output.push(character),
        }
    }
    output
}

pub fn terminal_safe_multiline(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    for character in input.chars() {
        match character {
            // Newlines and tabs are meaningful bytes in a unified diff. They
            // are safe terminal controls and must remain copyable as-is.
            '\n' | '\t' => output.push(character),
            '\r' => output.push_str("\\r"),
            character if character.is_control() || is_bidirectional_control(character) => {
                push_control_escape(&mut output, character);
            }
            character => output.push(character),
        }
    }
    output
}

/// Makes source safe for a terminal while preserving every byte offset.
///
/// Snippet annotations are byte ranges into the original source. Replacing an
/// unsafe scalar with the same number of ASCII question marks prevents escape
/// and bidirectional-control injection without moving any caret.
pub fn terminal_safe_source(input: &str) -> Cow<'_, str> {
    if !input.chars().any(is_unsafe_source_character) {
        return Cow::Borrowed(input);
    }
    let mut output = String::with_capacity(input.len());
    for character in input.chars() {
        if is_unsafe_source_character(character) {
            for _ in 0..character.len_utf8() {
                output.push('?');
            }
        } else {
            output.push(character);
        }
    }
    debug_assert_eq!(output.len(), input.len());
    Cow::Owned(output)
}

fn push_control_escape(output: &mut String, character: char) {
    let _ = write!(output, "\\u{{{:x}}}", u32::from(character));
}

fn is_unsafe_source_character(character: char) -> bool {
    !matches!(character, '\n' | '\t')
        && (character.is_control() || is_bidirectional_control(character))
}

const fn is_bidirectional_control(character: char) -> bool {
    matches!(
        character,
        '\u{061c}'
            | '\u{200e}'
            | '\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2066}'..='\u{2069}'
    )
}

pub fn format_duration(seconds: f64) -> String {
    if seconds < 0.001 {
        return format!("{:.0} µs", seconds * 1_000_000.0);
    }
    if seconds < 1.0 {
        return format!("{:.0} ms", seconds * 1_000.0);
    }
    if seconds < 60.0 {
        return format!("{seconds:.2} s");
    }
    let minutes = (seconds / 60.0).floor();
    // Keep the historical arithmetic order: changing to `mul_add` can alter a
    // rounded last digit in the stable human report on platforms with FMA.
    #[allow(clippy::suboptimal_flops)]
    let remainder = seconds - minutes * 60.0;
    format!("{minutes:.0} min {remainder:.1} s")
}

pub struct Paint {
    /// Whether this run emits ANSI styling, which the snippet renderer needs
    /// to answer for itself.
    pub color: bool,
    pub reset: &'static str,
    pub bold: &'static str,
    pub green: &'static str,
    pub bold_green: &'static str,
    pub yellow: &'static str,
    pub bold_yellow: &'static str,
    pub bold_red: &'static str,
    pub bold_blue: &'static str,
    pub bold_cyan: &'static str,
}

impl Paint {
    pub const fn new(color: bool) -> Self {
        if color {
            Self {
                color,
                reset: "\x1b[0m",
                bold: "\x1b[1m",
                green: "\x1b[32m",
                bold_green: "\x1b[1;32m",
                yellow: "\x1b[33m",
                bold_yellow: "\x1b[1;33m",
                bold_red: "\x1b[1;31m",
                bold_blue: "\x1b[1;34m",

                bold_cyan: "\x1b[1;36m",
            }
        } else {
            Self {
                color,
                reset: "",
                bold: "",
                green: "",
                bold_green: "",
                yellow: "",
                bold_yellow: "",
                bold_red: "",
                bold_blue: "",

                bold_cyan: "",
            }
        }
    }
}
