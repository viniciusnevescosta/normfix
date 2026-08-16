//! Conservative formatting and English diagnostics for 42 Makefiles.
//!
//! The formatter changes only byte-order marks, line endings, the official
//! header, final-newline hygiene and plain explicit `.c` assignments. Recipes,
//! computed variables, functions, comments, templates and other complex GNU
//! Make constructs are deliberately preserved.

#![forbid(unsafe_code)]

mod analysis;
mod compact;
mod header;
mod sources;

use std::path::Path;

pub use analysis::{MakefileDiagnostic, analyze_makefile, makefile_artifact};
pub use compact::{compact_source_assignments, visual_width};
pub use header::{
    MAKEFILE_HEADER_EDGE, MakefileHeaderError, build_makefile_header, ensure_makefile_header,
    identity_fits_makefile_header, makefile_header_filename_matches, makefile_header_fits,
    makefile_header_is_valid, makefile_header_span, update_makefile_header,
};
use normfix_header::{ByteRange, Fix, Identity42, Issue, RunClock};
pub use sources::{
    MakefileSourceReference, SourcePathStatus, SourceReconciliation, reconcile_source_references,
};

/// Result of the complete conservative Makefile formatting pass.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MakefileFormatResult {
    /// Complete transformed source.
    pub output: String,
    /// Accepted safe edits, in execution order.
    pub fixes: Vec<Fix>,
    /// English issues that prevented requested header work.
    pub issues: Vec<Issue>,
}

impl MakefileFormatResult {
    /// Returns whether the source changed.
    #[must_use]
    pub fn changed(&self, input: &str) -> bool {
        self.output != input
    }
}

/// Returns whether a path names a Makefile, ignoring ASCII case.
#[must_use]
pub fn is_makefile(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("makefile"))
}

/// Applies every semantics-preserving Makefile formatting operation.
///
/// Fix ranges refer to the snapshot immediately before that fix. This keeps
/// ranges exact even when an earlier hygiene edit changes every later offset.
#[must_use]
pub fn format_makefile(
    source: &str,
    filename: &str,
    identity: Option<&Identity42>,
    clock: &RunClock,
) -> MakefileFormatResult {
    let mut current = source.to_owned();
    let mut fixes = Vec::new();
    let mut issues = Vec::new();

    if current.starts_with('\u{feff}') {
        current.replace_range(..'\u{feff}'.len_utf8(), "");
        fixes.push(Fix {
            code: "REMOVE_BOM",
            description: "removed the UTF-8 byte-order mark".to_owned(),
            range: ByteRange::new(0, '\u{feff}'.len_utf8()),
        });
    }

    if current.contains('\r') {
        let old_len = current.len();
        current = current.replace("\r\n", "\n").replace('\r', "\n");
        fixes.push(Fix {
            code: "NORMALIZE_NEWLINES",
            description: "normalized line endings to LF".to_owned(),
            range: ByteRange::new(0, old_len),
        });
    }

    let header = ensure_makefile_header(&current, filename, identity, clock);
    current = header.output;
    let header_inserted = header.inserted;
    fixes.extend(header.fixes);
    issues.extend(header.issues);

    let compacted = compact_source_assignments(&current);
    current = compacted.output;
    fixes.extend(compacted.fixes);

    if !current.is_empty() && !current.ends_with('\n') {
        let offset = current.len();
        current.push('\n');
        fixes.push(Fix {
            code: "MISSING_NEWLINE",
            description: "added the final newline".to_owned(),
            range: ByteRange::new(offset, offset),
        });
    }

    if !header_inserted
        && (current != source || !makefile_header_filename_matches(&current, filename))
    {
        let updated = update_makefile_header(&current, filename, identity, clock);
        current = updated.output;
        fixes.extend(updated.fixes);
        issues.extend(updated.issues);
    }

    MakefileFormatResult {
        output: current,
        fixes,
        issues,
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use normfix_header::{Identity42, RunClock};

    use super::{analyze_makefile, format_makefile, is_makefile};

    fn identity() -> Identity42 {
        Identity42 {
            login: "student-a".to_owned(),
            email: "student-a@student.42.fr".to_owned(),
            source: "test".to_owned(),
            inferred_login: false,
            inferred_email: false,
        }
    }

    fn clock() -> RunClock {
        RunClock::fixed("2026/06/18 15:20:13").expect("valid test clock")
    }

    #[test]
    fn recognizes_only_makefile_basename() {
        assert!(is_makefile(Path::new("src/Makefile")));
        assert!(is_makefile(Path::new("makeFILE")));
        assert!(!is_makefile(Path::new("Makefile.am")));
    }

    #[test]
    fn full_formatting_is_idempotent_and_preserves_recipes() {
        let source = concat!(
            "\u{feff}NAME = demo\r\n",
            "SRC = one.c \\\r\n",
            "\ttwo.c\r\n",
            "all: $(NAME)\r\n",
            "$(NAME):\r\n",
            "\t$(CC) $(OBJ) -o $(NAME)\r\n",
            "clean:\r\nfclean: clean\r\nre: fclean all"
        );
        let first = format_makefile(source, "Makefile", Some(&identity()), &clock());
        assert!(first.output.starts_with("# "));
        assert!(first.output.ends_with('\n'));
        assert!(first.output.contains("\t$(CC) $(OBJ) -o $(NAME)\n"));
        assert!(first.fixes.iter().any(|fix| fix.code == "REMOVE_BOM"));
        assert!(
            first
                .fixes
                .iter()
                .any(|fix| fix.code == "NORMALIZE_NEWLINES")
        );
        let later_clock = RunClock::fixed("2026/06/19 09:10:11").expect("valid later test clock");
        let second = format_makefile(&first.output, "Makefile", Some(&identity()), &later_clock);
        assert_eq!(second.output, first.output);
        assert!(second.fixes.is_empty());
        assert!(second.issues.is_empty());
    }

    #[test]
    fn formatted_complete_makefile_has_no_native_diagnostics() {
        let source = concat!(
            "NAME\t= demo\n",
            "SRC\t= one.c \\\n",
            "\ttwo.c\n",
            "OBJ\t= $(SRC:.c=.o)\n\n",
            "all: $(NAME)\n\n",
            "$(NAME): $(OBJ)\n",
            "\t$(CC) $(OBJ) -o $(NAME)\n\n",
            "clean:\n",
            "\trm -f $(OBJ)\n\n",
            "fclean: clean\n",
            "\trm -f $(NAME)\n\n",
            "re: fclean all\n\n",
            ".PHONY: all clean fclean re\n"
        );
        let result = format_makefile(source, "Makefile", Some(&identity()), &clock());
        assert!(result.issues.is_empty());
        assert_eq!(analyze_makefile(&result.output), []);
    }
}
