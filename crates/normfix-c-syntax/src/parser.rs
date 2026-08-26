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
    parses: usize,
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
        Ok(Self { parser, parses: 0 })
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

    /// How many times this parser has read a source buffer.
    ///
    /// A parse is the dominant cost of a formatting run, and how many a run
    /// does is a property of the scheduler rather than of the machine. Exposing
    /// the count lets a test hold that property without timing anything, which
    /// a shared CI runner is far too noisy to do.
    #[must_use]
    pub const fn parses(&self) -> usize {
        self.parses
    }

    /// Parses an already shared source buffer without copying it.
    ///
    /// # Errors
    ///
    /// Returns an error if the source is too large for the shared text range
    /// representation or if Tree-sitter declines to produce a tree.
    pub fn parse_arc(&mut self, source: Arc<str>) -> Result<ParsedFile, ParseFailure> {
        ensure_supported_length(source.len())?;
        self.parses = self.parses.saturating_add(1);
        let tree = self
            .parser
            .parse(source.as_ref(), None)
            .ok_or(ParseFailure::NoTree)?;
        let original = ParsedFile::from_tree(Arc::clone(&source), &tree)?;
        if !original.has_syntax_errors() {
            return Ok(original);
        }
        let spans = compatible_va_arg_spans(&source, original.issues());
        if spans.is_empty() {
            return Ok(original);
        }
        let mut shadow = source.as_bytes().to_vec();
        for span in &spans {
            shadow[span.start..span.end].fill(b'_');
        }
        let shadow = String::from_utf8(shadow).map_err(|_| ParseFailure::InvalidRange {
            start: 0,
            end: source.len(),
        })?;
        let compatibility_tree = self
            .parser
            .parse(shadow.as_str(), None)
            .ok_or(ParseFailure::NoTree)?;
        let mut compatible = ParsedFile::from_tree(source, &compatibility_tree)?;
        if compatible.has_syntax_errors() || compatible.tape().has_unknown() {
            return Ok(original);
        }
        compatible.issues = spans
            .into_iter()
            .map(|span| {
                Ok(SyntaxIssue {
                    kind: SyntaxIssueKind::Compatibility,
                    range: text_range(span.start, span.end)?,
                    syntax_kind: "va_arg_type_argument".into(),
                })
            })
            .collect::<Result<Vec<_>, ParseFailure>>()?;
        Ok(compatible)
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
        self.issues
            .iter()
            .any(|issue| issue.kind != SyntaxIssueKind::Compatibility)
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
    /// A known grammar limitation isolated as one opaque, lossless token.
    ///
    /// Automatic edits remain permitted outside this exact byte range. No
    /// formatter phase can see or rewrite the bytes inside the token.
    Compatibility,
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

#[derive(Clone, Copy, Debug)]
struct CompatibilitySpan {
    start: usize,
    end: usize,
}

/// Finds single-line `va_arg` calls only when they explain every recovery node.
///
/// Tree-sitter-c parses the second argument as an expression even though the C
/// macro takes a raw type. Replacing the complete call with an equal-length
/// identifier creates an offset-stable syntax tree while the lossless tape
/// still carries the original call as one opaque token. Any unrelated recovery,
/// multiline call, comment/string occurrence, or unmatched parenthesis keeps
/// the original fail-closed parse.
fn compatible_va_arg_spans(source: &str, issues: &[SyntaxIssue]) -> Vec<CompatibilitySpan> {
    let bytes = source.as_bytes();
    let mut candidates = Vec::new();
    let mut index = 0_usize;
    let mut state = LexState::Code;
    while index < bytes.len() {
        match state {
            LexState::Code if bytes.get(index..index + 2) == Some(b"//") => {
                state = LexState::LineComment;
                index += 2;
            }
            LexState::Code if bytes.get(index..index + 2) == Some(b"/*") => {
                state = LexState::BlockComment;
                index += 2;
            }
            LexState::Code if bytes[index] == b'"' => {
                state = LexState::String;
                index += 1;
            }
            LexState::Code if bytes[index] == b'\'' => {
                state = LexState::Character;
                index += 1;
            }
            LexState::Code if identifier_at(bytes, index, b"va_arg") => {
                if let Some(end) = single_line_call_end(bytes, index + b"va_arg".len()) {
                    candidates.push(CompatibilitySpan { start: index, end });
                    index = end;
                } else {
                    index += b"va_arg".len();
                }
            }
            LexState::LineComment if matches!(bytes[index], b'\r' | b'\n') => {
                state = LexState::Code;
                index += 1;
            }
            LexState::BlockComment if bytes.get(index..index + 2) == Some(b"*/") => {
                state = LexState::Code;
                index += 2;
            }
            LexState::String | LexState::Character if bytes[index] == b'\\' => {
                index = (index + 2).min(bytes.len());
            }
            LexState::String if bytes[index] == b'"' => {
                state = LexState::Code;
                index += 1;
            }
            LexState::Character if bytes[index] == b'\'' => {
                state = LexState::Code;
                index += 1;
            }
            _ => index += 1,
        }
    }
    let candidates = candidates
        .into_iter()
        .filter(|span| issues.iter().any(|issue| issue_within_span(issue, *span)))
        .collect::<Vec<_>>();
    if issues.iter().all(|issue| {
        candidates
            .iter()
            .any(|span| issue_within_span(issue, *span))
    }) {
        candidates
    } else {
        Vec::new()
    }
}

#[derive(Clone, Copy)]
enum LexState {
    Code,
    LineComment,
    BlockComment,
    String,
    Character,
}

fn identifier_at(source: &[u8], start: usize, identifier: &[u8]) -> bool {
    source.get(start..start + identifier.len()) == Some(identifier)
        && source
            .get(start.wrapping_sub(1))
            .is_none_or(|byte| !is_identifier_byte(*byte))
        && source
            .get(start + identifier.len())
            .is_none_or(|byte| !is_identifier_byte(*byte))
}

const fn is_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn single_line_call_end(source: &[u8], after_name: usize) -> Option<usize> {
    let mut cursor = after_name;
    while source.get(cursor).is_some_and(u8::is_ascii_whitespace) {
        if matches!(source[cursor], b'\r' | b'\n') {
            return None;
        }
        cursor += 1;
    }
    if source.get(cursor) != Some(&b'(') {
        return None;
    }
    let mut depth = 0_usize;
    let mut saw_top_level_comma = false;
    let mut state = LexState::Code;
    while cursor < source.len() {
        let byte = source[cursor];
        if matches!(byte, b'\r' | b'\n') {
            return None;
        }
        match state {
            LexState::Code if source.get(cursor..cursor + 2) == Some(b"//") => return None,
            LexState::Code if source.get(cursor..cursor + 2) == Some(b"/*") => {
                state = LexState::BlockComment;
                cursor += 2;
            }
            LexState::Code if byte == b'"' => {
                state = LexState::String;
                cursor += 1;
            }
            LexState::Code if byte == b'\'' => {
                state = LexState::Character;
                cursor += 1;
            }
            LexState::Code if byte == b'(' => {
                depth += 1;
                cursor += 1;
            }
            LexState::Code if byte == b')' => {
                depth = depth.checked_sub(1)?;
                cursor += 1;
                if depth == 0 {
                    return saw_top_level_comma.then_some(cursor);
                }
            }
            LexState::Code if byte == b',' && depth == 1 => {
                saw_top_level_comma = true;
                cursor += 1;
            }
            LexState::BlockComment if source.get(cursor..cursor + 2) == Some(b"*/") => {
                state = LexState::Code;
                cursor += 2;
            }
            LexState::String | LexState::Character if byte == b'\\' => {
                cursor = (cursor + 2).min(source.len());
            }
            LexState::String if byte == b'"' => {
                state = LexState::Code;
                cursor += 1;
            }
            LexState::Character if byte == b'\'' => {
                state = LexState::Code;
                cursor += 1;
            }
            _ => cursor += 1,
        }
    }
    None
}

fn issue_within_span(issue: &SyntaxIssue, span: CompatibilitySpan) -> bool {
    let start = issue.range.start().get() as usize;
    let end = issue.range.end().get() as usize;
    if start == end {
        span.start <= start && start <= span.end
    } else {
        span.start < end && start < span.end
    }
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
    fn raw_va_arg_type_arguments_are_isolated_as_one_opaque_compatible_token() {
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

        assert!(!parsed.has_syntax_errors());
        assert!(parsed.permits_automatic_edits());
        assert_eq!(parsed.issues().len(), 1);
        assert_eq!(parsed.issues()[0].kind(), SyntaxIssueKind::Compatibility);
        assert!(parsed.tape().is_lossless());
        assert_eq!(parsed.tape().reconstruct(), source);
    }

    #[test]
    fn va_arg_compatibility_never_hides_an_unrelated_syntax_error() {
        let mut parser = CParser::new().expect("embedded C grammar must load");
        let source = concat!(
            "char\t*next_string(va_list *args)\n",
            "{\n",
            "\treturn (va_arg(*args, char *));\n",
            "}\n",
            "int broken( {\n",
        );
        let parsed = parser.parse(source).expect("recovered translation unit");

        assert!(parsed.has_syntax_errors());
        assert!(!parsed.permits_automatic_edits());
        assert!(parsed.issues().iter().all(|issue| {
            matches!(
                issue.kind(),
                SyntaxIssueKind::Error | SyntaxIssueKind::Missing
            )
        }));
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
        assert_eq!(facts.functions[0].parameters.len(), 2);
        assert_eq!(facts.functions[0].parameters[0].name, "text");
        assert_eq!(facts.functions[1].kind, CFunctionKind::Definition);
        assert!(facts.functions[1].is_static);
        assert!(!facts.functions[1].returns_pointer);
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

    #[test]
    fn function_typedefs_are_not_function_prototype_facts() {
        let mut parser = CParser::new().expect("embedded C grammar must load");
        let source = concat!(
            "typedef int\tt_callback(void);\n",
            "typedef int\t(*t_callback_pointer)(void);\n",
        );
        let parsed = parser.parse(source).expect("valid function typedefs");

        assert!(parsed.issues().is_empty());
        assert!(parsed.facts().functions.is_empty());
    }

    #[test]
    fn structural_facts_cover_controls_calls_returns_tags_and_null_checks() {
        let mut parser = CParser::new().expect("embedded C grammar must load");
        let source = concat!(
            "#include <stddef.h>\n",
            "struct context { int value; };\n",
            "char\t*pick(char *value)\n",
            "{\n",
            "\tint\tseen;\n",
            "\tif (value == NULL)\n",
            "\t{\n",
            "\t\treturn (0);\n",
            "\t}\n",
            "\telse\n",
            "\t{\n",
            "\t\treturn (value);\n",
            "\t}\n",
            "\tseen = helper();\n",
            "\twhile (1)\n",
            "\t\tseen++;\n",
            "}\n",
        );
        let parsed = parser.parse(source).expect("valid translation unit");
        let facts = parsed.facts();
        assert!(facts.functions[0].returns_pointer);
        assert_eq!(facts.control_compounds.len(), 2);
        assert_eq!(facts.single_statement_bodies.len(), 2);
        assert_eq!(facts.redundant_else_branches.len(), 1);
        assert_eq!(facts.local_declarations.len(), 1);
        assert_eq!(facts.initial_declaration_blocks.len(), 1);
        assert_eq!(facts.returns.len(), 2);
        assert_eq!(facts.null_checks.len(), 1);
        assert_eq!(facts.null_providers.len(), 1);
        assert!(facts.macros.is_empty());
        assert_eq!(facts.type_tags.len(), 1);
        assert!(facts.calls.iter().any(|call| call.name == "helper"));
        assert!(facts.loops.iter().any(|loop_fact| loop_fact.unconditional));
    }

    #[test]
    fn named_parameter_and_local_facts_recover_function_pointer_declarators() {
        let mut parser = CParser::new().expect("embedded C grammar must load");
        let source = concat!(
            "void\trun(void (*callback)(int), int value)\n",
            "{\n",
            "\tint\t(*local_callback)(int);\n",
            "\tcallback(value);\n",
            "}\n",
        );
        let parsed = parser.parse(source).expect("valid translation unit");
        let facts = parsed.facts();
        assert_eq!(facts.functions[0].parameters.len(), 2);
        assert_eq!(facts.functions[0].parameters[0].name, "callback");
        assert_eq!(facts.functions[0].parameters[1].name, "value");
        assert_eq!(
            facts.local_declarations[0].name.as_deref(),
            Some("local_callback")
        );
    }

    #[test]
    fn macro_facts_distinguish_function_like_and_object_like_names() {
        let mut parser = CParser::new().expect("embedded C grammar must load");
        let source = concat!(
            "#define OBJECT target\n",
            "#define FUNCTION(value) ((value) + 1)\n",
        );
        let parsed = parser.parse(source).expect("valid preprocessing source");
        let facts = parsed.facts();
        assert_eq!(facts.macros.len(), 2);
        assert_eq!(facts.macros[0].name, "OBJECT");
        assert!(!facts.macros[0].function_like);
        assert_eq!(facts.macros[1].name, "FUNCTION");
        assert!(facts.macros[1].function_like);
    }
}
