//! Encapsulated Tree-sitter C parser.

use std::error::Error;
use std::fmt;
use std::sync::Arc;

use normfix_core::{TextRange, TextSize};
use tree_sitter::{Node, Parser, Tree};

use crate::tape::{TerminalSpan, TokenTape};
use crate::{SyntaxFacts, facts::collect_facts};

/// A reusable C parser.
///
/// `tree_sitter::Parser` is intentionally not exposed. Create one parser per
/// worker thread and reuse it for the files handled by that worker.
pub struct CParser {
    parser: Parser,
}

impl CParser {
    /// Creates a parser configured for the C grammar.
    ///
    /// # Errors
    ///
    /// Returns [`ParseFailure::Language`] if the embedded grammar cannot be
    /// installed in Tree-sitter.
    #[allow(clippy::new_without_default)]
    pub fn new() -> Result<Self, ParseFailure> {
        let mut parser = Parser::new();
        let language = tree_sitter_c::LANGUAGE.into();
        parser
            .set_language(&language)
            .map_err(|error| ParseFailure::Language(error.to_string()))?;
        Ok(Self { parser })
    }

    /// Parses UTF-8 C source and builds a lossless token tape.
    ///
    /// Syntax errors are returned inside [`ParsedFile`], not as a failure.
    /// This method does not attempt semantic classification.
    ///
    /// # Errors
    ///
    /// Returns an error if the source is too large for the shared text range
    /// representation or if Tree-sitter declines to produce a tree.
    pub fn parse(&mut self, source: &str) -> Result<ParsedFile, ParseFailure> {
        self.parse_arc(Arc::<str>::from(source))
    }

    /// Parses an already shared source buffer without copying it.
    ///
    /// # Errors
    ///
    /// Returns an error if the source is too large for the shared text range
    /// representation or if Tree-sitter declines to produce a tree.
    pub fn parse_arc(&mut self, source: Arc<str>) -> Result<ParsedFile, ParseFailure> {
        ensure_supported_length(source.len())?;
        let tree = self
            .parser
            .parse(source.as_ref(), None)
            .ok_or(ParseFailure::NoTree)?;
        ParsedFile::from_tree(source, &tree)
    }
}

impl fmt::Debug for CParser {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("CParser").finish_non_exhaustive()
    }
}

/// A C source file parsed into backend-neutral metadata and a lossless tape.
#[derive(Clone, Debug)]
pub struct ParsedFile {
    source: Arc<str>,
    root_kind: Box<str>,
    root_range: TextRange,
    issues: Vec<SyntaxIssue>,
    tape: TokenTape,
    facts: SyntaxFacts,
}

impl ParsedFile {
    fn from_tree(source: Arc<str>, tree: &Tree) -> Result<Self, ParseFailure> {
        let root = tree.root_node();
        let root_range = text_range(root.start_byte(), root.end_byte())?;
        let mut issues = Vec::new();
        let mut terminals = Vec::new();
        collect_tree_metadata(root, &mut issues, &mut terminals)?;
        let tape = TokenTape::from_terminals(Arc::clone(&source), terminals)?;
        let facts = collect_facts(&source, root)?;

        Ok(Self {
            source,
            root_kind: root.kind().into(),
            root_range,
            issues,
            tape,
            facts,
        })
    }

    /// Returns the exact source used for parsing.
    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Returns the grammar name of the root node.
    ///
    /// For ordinary C files this is `translation_unit`.
    #[must_use]
    pub fn root_kind(&self) -> &str {
        &self.root_kind
    }

    /// Returns the root node's byte range.
    #[must_use]
    pub const fn root_range(&self) -> TextRange {
        self.root_range
    }

    /// Returns concrete `ERROR` and `MISSING` regions reported by the parser.
    #[must_use]
    pub fn issues(&self) -> &[SyntaxIssue] {
        &self.issues
    }

    /// Returns whether Tree-sitter recovered from at least one syntax problem.
    #[must_use]
    pub fn has_syntax_errors(&self) -> bool {
        !self.issues.is_empty()
    }

    /// Returns whether this milestone permits automatic edits to the file.
    ///
    /// The initial syntax foundation is deliberately conservative: any
    /// `ERROR`, `MISSING`, or unknown tape region blocks all automatic edits.
    /// Later action crates may refine this to non-overlapping regions.
    #[must_use]
    pub fn permits_automatic_edits(&self) -> bool {
        !self.has_syntax_errors() && !self.tape.has_unknown()
    }

    /// Returns the lossless token and trivia tape.
    #[must_use]
    pub const fn tape(&self) -> &TokenTape {
        &self.tape
    }

    /// Returns backend-neutral structural facts collected by this parse.
    #[must_use]
    pub const fn facts(&self) -> &SyntaxFacts {
        &self.facts
    }
}

/// A concrete syntax recovery issue.
///
/// These ranges are intentionally read-only in the first Rust milestone:
/// formatters and actions must not edit through an `ERROR` or `MISSING` region.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxIssue {
    kind: SyntaxIssueKind,
    range: TextRange,
    syntax_kind: Box<str>,
}

impl SyntaxIssue {
    /// Returns whether this is an error node or a missing token.
    #[must_use]
    pub const fn kind(&self) -> SyntaxIssueKind {
        self.kind
    }

    /// Returns the affected UTF-8 byte range.
    ///
    /// Missing tokens have an empty range at the insertion point.
    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.range
    }

    /// Returns Tree-sitter's stable grammar name as plain text.
    #[must_use]
    pub fn syntax_kind(&self) -> &str {
        &self.syntax_kind
    }
}

/// The kind of parser recovery represented by a [`SyntaxIssue`].
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SyntaxIssueKind {
    /// Tree-sitter grouped unexpected input into an `ERROR` node.
    Error,
    /// Tree-sitter inserted a zero-width missing token.
    Missing,
}

/// A failure to initialize or run the parsing backend.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ParseFailure {
    /// The bundled C language could not be installed.
    Language(String),
    /// The source exceeds the range representation supported by the core.
    SourceTooLarge {
        /// Source length in bytes.
        bytes: usize,
    },
    /// Tree-sitter returned no tree.
    NoTree,
    /// Tree-sitter returned an invalid or unsupported byte range.
    InvalidRange {
        /// Start byte supplied by the backend.
        start: usize,
        /// End byte supplied by the backend.
        end: usize,
    },
}

impl fmt::Display for ParseFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Language(message) => {
                write!(formatter, "could not configure the C grammar: {message}")
            }
            Self::SourceTooLarge { bytes } => {
                write!(
                    formatter,
                    "C source contains {bytes} bytes, exceeding supported text ranges"
                )
            }
            Self::NoTree => formatter.write_str("the C parser returned no syntax tree"),
            Self::InvalidRange { start, end } => {
                write!(
                    formatter,
                    "the C parser returned an invalid byte range {start}..{end}"
                )
            }
        }
    }
}

impl Error for ParseFailure {}

fn ensure_supported_length(length: usize) -> Result<(), ParseFailure> {
    TextSize::try_from(length)
        .map(|_| ())
        .map_err(|_| ParseFailure::SourceTooLarge { bytes: length })
}

fn text_range(start: usize, end: usize) -> Result<TextRange, ParseFailure> {
    if start > end {
        return Err(ParseFailure::InvalidRange { start, end });
    }
    let start_size =
        TextSize::try_from(start).map_err(|_| ParseFailure::InvalidRange { start, end })?;
    let end_size =
        TextSize::try_from(end).map_err(|_| ParseFailure::InvalidRange { start, end })?;
    TextRange::new(start_size, end_size).ok_or(ParseFailure::InvalidRange { start, end })
}

fn collect_tree_metadata(
    root: Node<'_>,
    issues: &mut Vec<SyntaxIssue>,
    terminals: &mut Vec<TerminalSpan>,
) -> Result<(), ParseFailure> {
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        if node.is_missing() {
            issues.push(SyntaxIssue {
                kind: SyntaxIssueKind::Missing,
                range: text_range(node.start_byte(), node.end_byte())?,
                syntax_kind: node.kind().into(),
            });
        } else if node.is_error() {
            issues.push(SyntaxIssue {
                kind: SyntaxIssueKind::Error,
                range: text_range(node.start_byte(), node.end_byte())?,
                syntax_kind: node.kind().into(),
            });
        }

        if node.child_count() == 0 {
            if !node.is_missing() && node.start_byte() < node.end_byte() {
                terminals.push(TerminalSpan {
                    start: node.start_byte(),
                    end: node.end_byte(),
                    syntax_kind: node.kind().into(),
                });
            }
            continue;
        }

        let mut cursor = node.walk();
        let children: Vec<_> = node.children(&mut cursor).collect();
        pending.extend(children.into_iter().rev());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::CFunctionKind;

    use super::{CParser, SyntaxIssueKind};

    #[test]
    fn parser_debug_does_not_expose_backend_state() {
        let parser = CParser::new().expect("embedded C grammar must load");
        assert_eq!(format!("{parser:?}"), "CParser { .. }");
    }

    #[test]
    fn malformed_input_returns_recovery_issues_instead_of_failure() {
        let mut parser = CParser::new().expect("embedded C grammar must load");
        let source = "int main( {\n\treturn (0)\n";
        let parsed = parser
            .parse(source)
            .expect("recovery should produce a tree");

        assert!(parsed.has_syntax_errors());
        assert!(!parsed.permits_automatic_edits());
        assert!(parsed.issues().iter().any(|issue| matches!(
            issue.kind(),
            SyntaxIssueKind::Error | SyntaxIssueKind::Missing
        )));
        assert_eq!(parsed.tape().reconstruct(), source);
    }

    #[test]
    fn a_missing_token_is_reported_and_blocks_automatic_edits() {
        let mut parser = CParser::new().expect("embedded C grammar must load");
        let source = "int\tanswer(void)\n{\n\treturn (42)\n}\n";
        let parsed = parser
            .parse(source)
            .expect("recovery should produce a tree");

        assert!(
            parsed
                .issues()
                .iter()
                .any(|issue| issue.kind() == SyntaxIssueKind::Missing)
        );
        assert!(!parsed.permits_automatic_edits());
        assert_eq!(parsed.tape().reconstruct(), source);
    }

    #[test]
    fn raw_va_arg_type_arguments_are_preserved_conservatively() {
        let mut parser = CParser::new().expect("embedded C grammar must load");
        let source = concat!(
            "char\t*next_string(va_list *args)\n",
            "{\n",
            "\treturn (va_arg(*args, char *));\n",
            "}\n",
        );
        let parsed = parser
            .parse(source)
            .expect("recovery should produce a tree");

        assert!(parsed.has_syntax_errors());
        assert!(!parsed.permits_automatic_edits());
        assert!(parsed.tape().is_lossless());
        assert_eq!(parsed.tape().reconstruct(), source);
    }

    #[test]
    fn structural_facts_cover_functions_enums_arrays_and_preprocessors() {
        let mut parser = CParser::new().expect("embedded C grammar must load");
        let source = concat!(
            "#ifndef SAMPLE_H\n",
            "# define SAMPLE_H\n",
            "typedef enum e_op { op_first = 4, op_next, op_total = op_next + 6 } t_op;\n",
            "typedef struct s_context { int count[op_total]; } t_context;\n",
            "int\tdeclared(char *text, int size);\n",
            "static int\tdefined(void)\n",
            "{\n",
            "\treturn (0);\n",
            "}\n",
            "#endif\n",
        );
        let parsed = parser.parse(source).expect("valid translation unit");
        let facts = parsed.facts();

        assert_eq!(facts.functions.len(), 2);
        assert_eq!(facts.functions[0].kind, CFunctionKind::Prototype);
        assert_eq!(facts.functions[0].name, "declared");
        assert_eq!(facts.functions[0].parameter_count, 2);
        assert_eq!(facts.functions[1].kind, CFunctionKind::Definition);
        assert!(facts.functions[1].is_static);
        assert_eq!(facts.enum_constants.len(), 3);
        assert_eq!(
            facts.enum_constants[2].explicit_value.as_deref(),
            Some("op_next + 6")
        );
        assert_eq!(facts.arrays.len(), 1);
        assert_eq!(facts.arrays[0].name.as_deref(), Some("count"));
        assert_eq!(facts.arrays[0].bound.as_deref(), Some("op_total"));
        assert!(!facts.preprocessor_ranges.is_empty());
    }
}
