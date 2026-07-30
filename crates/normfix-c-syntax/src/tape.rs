//! Full-fidelity token and trivia tape.

use std::sync::Arc;

use normfix_core::{TextRange, TextSize};

use crate::parser::ParseFailure;

/// A lossless sequence covering every byte of one UTF-8 C source.
#[derive(Clone, Debug)]
pub struct TokenTape {
    source: Arc<str>,
    pieces: Vec<TapePiece>,
}

impl TokenTape {
    pub(crate) fn from_terminals(
        source: Arc<str>,
        mut terminals: Vec<TerminalSpan>,
    ) -> Result<Self, ParseFailure> {
        terminals.sort_unstable_by_key(|span| (span.start, span.end));
        let pieces = Scanner::new(&source, &terminals).scan()?;
        Ok(Self { source, pieces })
    }

    /// Returns the tape pieces in source order.
    #[must_use]
    pub fn pieces(&self) -> &[TapePiece] {
        &self.pieces
    }

    /// Returns the original source.
    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Returns whether the parser left non-trivia bytes unclassified.
    #[must_use]
    pub fn has_unknown(&self) -> bool {
        self.pieces
            .iter()
            .any(|piece| matches!(piece, TapePiece::Unknown(_)))
    }

    /// Reconstructs the source by concatenating tape pieces.
    ///
    /// The implementation remains defensive: if an internal range invariant is
    /// ever violated, it returns the original immutable source rather than
    /// panicking or emitting truncated code.
    #[must_use]
    pub fn reconstruct(&self) -> String {
        let mut output = String::with_capacity(self.source.len());
        let mut expected_start = 0;
        for piece in &self.pieces {
            let span = piece.byte_span();
            if span.start != expected_start {
                return self.source.to_string();
            }
            let Some(fragment) = self.source.get(span.start..span.end) else {
                return self.source.to_string();
            };
            output.push_str(fragment);
            expected_start = span.end;
        }
        if expected_start != self.source.len() {
            return self.source.to_string();
        }
        output
    }

    /// Verifies contiguity, UTF-8 boundaries and complete byte coverage.
    #[must_use]
    pub fn is_lossless(&self) -> bool {
        let mut expected_start = 0;
        for piece in &self.pieces {
            let span = piece.byte_span();
            if span.start != expected_start
                || span.start > span.end
                || self.source.get(span.start..span.end).is_none()
            {
                return false;
            }
            expected_start = span.end;
        }
        expected_start == self.source.len() && self.reconstruct() == self.source.as_ref()
    }
}

/// One lossless source segment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TapePiece {
    /// A terminal token recognized by the C grammar.
    Token(TokenPiece),
    /// Whitespace, comments or other formatting trivia.
    Trivia(TriviaPiece),
    /// Non-trivia bytes not owned by a terminal syntax node.
    Unknown(UnknownPiece),
}

impl TapePiece {
    /// Returns the UTF-8 byte range occupied by this piece.
    #[must_use]
    pub const fn range(&self) -> TextRange {
        match self {
            Self::Token(piece) => piece.range,
            Self::Trivia(piece) => piece.range,
            Self::Unknown(piece) => piece.range,
        }
    }

    fn byte_span(&self) -> ByteSpan {
        match self {
            Self::Token(piece) => piece.bytes,
            Self::Trivia(piece) => piece.bytes,
            Self::Unknown(piece) => piece.bytes,
        }
    }
}

/// A terminal C token.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TokenPiece {
    range: TextRange,
    bytes: ByteSpan,
    syntax_kind: Box<str>,
}

impl TokenPiece {
    /// Returns this token's UTF-8 byte range.
    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.range
    }

    /// Returns the backend-neutral textual grammar kind.
    #[must_use]
    pub fn syntax_kind(&self) -> &str {
        &self.syntax_kind
    }
}

/// A formatting or comment segment outside the logical token stream.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TriviaPiece {
    range: TextRange,
    bytes: ByteSpan,
    kind: TriviaKind,
}

impl TriviaPiece {
    /// Returns this trivia's UTF-8 byte range.
    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.range
    }

    /// Returns the trivia classification.
    #[must_use]
    pub const fn kind(&self) -> TriviaKind {
        self.kind
    }
}

/// Bytes not owned by a terminal syntax node and not recognized as trivia.
///
/// Unknown regions are preserved losslessly and must block automatic edits.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnknownPiece {
    range: TextRange,
    bytes: ByteSpan,
}

impl UnknownPiece {
    /// Returns the unknown region's UTF-8 byte range.
    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.range
    }
}

/// Trivia classifications relevant to deterministic C formatting.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TriviaKind {
    /// One or more ordinary ASCII spaces.
    Spaces,
    /// One or more horizontal tabs.
    Tabs,
    /// A physical line ending.
    Newline(LineEnding),
    /// A `//` comment, excluding its final physical newline.
    LineComment,
    /// A `/* ... */` comment, including internal newlines.
    BlockComment,
    /// A backslash followed by LF or CRLF.
    EscapedNewline(LineEnding),
    /// A leading UTF-8 byte-order mark.
    Bom,
}

/// Physical newline encoding.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum LineEnding {
    /// Line feed (`\n`).
    Lf,
    /// Carriage return followed by line feed (`\r\n`).
    CrLf,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ByteSpan {
    start: usize,
    end: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct TerminalSpan {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) syntax_kind: Box<str>,
}

struct Scanner<'source> {
    source: &'source str,
    bytes: &'source [u8],
    terminals: &'source [TerminalSpan],
    pieces: Vec<TapePiece>,
    terminal_index: usize,
    cursor: usize,
}

impl<'source> Scanner<'source> {
    fn new(source: &'source str, terminals: &'source [TerminalSpan]) -> Self {
        Self {
            source,
            bytes: source.as_bytes(),
            terminals,
            pieces: Vec::new(),
            terminal_index: 0,
            cursor: 0,
        }
    }

    fn scan(mut self) -> Result<Vec<TapePiece>, ParseFailure> {
        while self.cursor < self.bytes.len() {
            if self.cursor == 0 && self.bytes.starts_with(&[0xef, 0xbb, 0xbf]) {
                self.emit_trivia(self.cursor, self.cursor + 3, TriviaKind::Bom)?;
                self.cursor += 3;
            } else if self.starts_with(b"//") {
                self.scan_line_comment()?;
            } else if self.starts_with(b"/*") {
                self.scan_block_comment()?;
            } else if let Some((length, ending)) = self.escaped_newline_at(self.cursor) {
                self.emit_trivia(
                    self.cursor,
                    self.cursor + length,
                    TriviaKind::EscapedNewline(ending),
                )?;
                self.cursor += length;
            } else {
                match self.bytes[self.cursor] {
                    b' ' => self.scan_repeated_byte(b' ', TriviaKind::Spaces)?,
                    b'\t' => self.scan_repeated_byte(b'\t', TriviaKind::Tabs)?,
                    b'\r' if self.starts_with(b"\r\n") => {
                        self.emit_trivia(
                            self.cursor,
                            self.cursor + 2,
                            TriviaKind::Newline(LineEnding::CrLf),
                        )?;
                        self.cursor += 2;
                    }
                    b'\n' => {
                        self.emit_trivia(
                            self.cursor,
                            self.cursor + 1,
                            TriviaKind::Newline(LineEnding::Lf),
                        )?;
                        self.cursor += 1;
                    }
                    b'"' | b'\'' => self.scan_quoted(self.bytes[self.cursor])?,
                    _ => self.scan_code()?,
                }
            }
        }
        Ok(self.pieces)
    }

    fn scan_line_comment(&mut self) -> Result<(), ParseFailure> {
        let mut segment_start = self.cursor;
        self.cursor += 2;
        while self.cursor < self.bytes.len() {
            if let Some((length, ending)) = self.escaped_newline_at(self.cursor) {
                if segment_start < self.cursor {
                    self.emit_trivia(segment_start, self.cursor, TriviaKind::LineComment)?;
                }
                self.emit_trivia(
                    self.cursor,
                    self.cursor + length,
                    TriviaKind::EscapedNewline(ending),
                )?;
                self.cursor += length;
                segment_start = self.cursor;
            } else if self.bytes[self.cursor] == b'\n' || self.starts_with_at(self.cursor, b"\r\n")
            {
                break;
            } else {
                self.cursor = next_char_boundary(self.source, self.cursor);
            }
        }
        if segment_start < self.cursor {
            self.emit_trivia(segment_start, self.cursor, TriviaKind::LineComment)?;
        }
        Ok(())
    }

    fn scan_block_comment(&mut self) -> Result<(), ParseFailure> {
        let start = self.cursor;
        self.cursor += 2;
        while self.cursor < self.bytes.len() {
            if self.starts_with_at(self.cursor, b"*/") {
                self.cursor += 2;
                break;
            }
            self.cursor = next_char_boundary(self.source, self.cursor);
        }
        self.emit_trivia(start, self.cursor, TriviaKind::BlockComment)
    }

    fn scan_repeated_byte(&mut self, byte: u8, kind: TriviaKind) -> Result<(), ParseFailure> {
        let start = self.cursor;
        while self.bytes.get(self.cursor).copied() == Some(byte) {
            self.cursor += 1;
        }
        self.emit_trivia(start, self.cursor, kind)
    }

    fn scan_quoted(&mut self, quote: u8) -> Result<(), ParseFailure> {
        let start = self.cursor;
        self.cursor += 1;
        while self.cursor < self.bytes.len() {
            if self.bytes[self.cursor] == quote {
                self.cursor += 1;
                break;
            }
            if self.bytes[self.cursor] == b'\\' {
                if let Some((length, _)) = self.escaped_newline_at(self.cursor) {
                    self.cursor += length;
                } else if self.cursor + 1 < self.bytes.len() {
                    self.cursor = next_char_boundary(self.source, self.cursor + 1);
                } else {
                    self.cursor += 1;
                }
            } else {
                self.cursor = next_char_boundary(self.source, self.cursor);
            }
        }
        self.emit_code(start, self.cursor)
    }

    fn scan_code(&mut self) -> Result<(), ParseFailure> {
        let start = self.cursor;
        self.cursor = next_char_boundary(self.source, self.cursor);
        while self.cursor < self.bytes.len() && !self.is_special_at(self.cursor) {
            self.cursor = next_char_boundary(self.source, self.cursor);
        }
        self.emit_code(start, self.cursor)
    }

    fn is_special_at(&self, offset: usize) -> bool {
        matches!(
            self.bytes.get(offset).copied(),
            Some(b' ' | b'\t' | b'\r' | b'\n' | b'"' | b'\'')
        ) || self.starts_with_at(offset, b"//")
            || self.starts_with_at(offset, b"/*")
            || self.escaped_newline_at(offset).is_some()
    }

    fn emit_code(&mut self, start: usize, end: usize) -> Result<(), ParseFailure> {
        let mut cursor = start;
        self.skip_terminals_ending_before(cursor);

        while cursor < end {
            let Some(terminal) = self.terminals.get(self.terminal_index) else {
                self.emit_unknown(cursor, end)?;
                break;
            };
            let terminal_start = terminal.start;
            let terminal_end = terminal.end;
            let syntax_kind = terminal.syntax_kind.clone();

            if terminal_start >= end {
                self.emit_unknown(cursor, end)?;
                break;
            }
            if terminal_end <= cursor {
                self.terminal_index += 1;
                continue;
            }
            if terminal_start > cursor {
                let unknown_end = terminal_start.min(end);
                self.emit_unknown(cursor, unknown_end)?;
                cursor = unknown_end;
                continue;
            }

            let token_end = terminal_end.min(end);
            self.emit_token(cursor, token_end, syntax_kind)?;
            cursor = token_end;
            if cursor >= terminal_end {
                self.terminal_index += 1;
            }
        }
        Ok(())
    }

    fn skip_terminals_ending_before(&mut self, offset: usize) {
        while self
            .terminals
            .get(self.terminal_index)
            .is_some_and(|terminal| terminal.end <= offset)
        {
            self.terminal_index += 1;
        }
    }

    fn emit_token(
        &mut self,
        start: usize,
        end: usize,
        syntax_kind: Box<str>,
    ) -> Result<(), ParseFailure> {
        let (range, bytes) = piece_range(start, end)?;
        self.pieces.push(TapePiece::Token(TokenPiece {
            range,
            bytes,
            syntax_kind,
        }));
        Ok(())
    }

    fn emit_trivia(
        &mut self,
        start: usize,
        end: usize,
        kind: TriviaKind,
    ) -> Result<(), ParseFailure> {
        let (range, bytes) = piece_range(start, end)?;
        self.pieces
            .push(TapePiece::Trivia(TriviaPiece { range, bytes, kind }));
        Ok(())
    }

    fn emit_unknown(&mut self, start: usize, end: usize) -> Result<(), ParseFailure> {
        let (range, bytes) = piece_range(start, end)?;
        self.pieces
            .push(TapePiece::Unknown(UnknownPiece { range, bytes }));
        Ok(())
    }

    fn starts_with(&self, pattern: &[u8]) -> bool {
        self.starts_with_at(self.cursor, pattern)
    }

    fn starts_with_at(&self, offset: usize, pattern: &[u8]) -> bool {
        self.bytes
            .get(offset..)
            .is_some_and(|remaining| remaining.starts_with(pattern))
    }

    fn escaped_newline_at(&self, offset: usize) -> Option<(usize, LineEnding)> {
        if self.starts_with_at(offset, b"\\\r\n") {
            Some((3, LineEnding::CrLf))
        } else if self.starts_with_at(offset, b"\\\n") {
            Some((2, LineEnding::Lf))
        } else {
            None
        }
    }
}

fn piece_range(start: usize, end: usize) -> Result<(TextRange, ByteSpan), ParseFailure> {
    if start > end {
        return Err(ParseFailure::InvalidRange { start, end });
    }
    let start_size =
        TextSize::try_from(start).map_err(|_| ParseFailure::InvalidRange { start, end })?;
    let end_size =
        TextSize::try_from(end).map_err(|_| ParseFailure::InvalidRange { start, end })?;
    let range =
        TextRange::new(start_size, end_size).ok_or(ParseFailure::InvalidRange { start, end })?;
    Ok((range, ByteSpan { start, end }))
}

fn next_char_boundary(source: &str, offset: usize) -> usize {
    let Some(remaining) = source.get(offset..) else {
        return source.len();
    };
    remaining
        .chars()
        .next()
        .map_or(source.len(), |character| offset + character.len_utf8())
}

#[cfg(test)]
mod tests {
    use super::{LineEnding, TapePiece, TokenTape, TriviaKind};
    use crate::CParser;

    fn trivia_kinds(tape: &TokenTape) -> Vec<TriviaKind> {
        tape.pieces()
            .iter()
            .filter_map(|piece| match piece {
                TapePiece::Trivia(trivia) => Some(trivia.kind()),
                TapePiece::Token(_) | TapePiece::Unknown(_) => None,
            })
            .collect()
    }

    #[test]
    fn valid_c_round_trips_losslessly() {
        let source = "int\tmain(void)\n{\n\treturn (0);\n}\n";
        let mut parser = CParser::new().expect("embedded C grammar must load");
        let parsed = parser.parse(source).expect("valid C must parse");

        assert!(!parsed.has_syntax_errors());
        assert!(parsed.permits_automatic_edits());
        assert!(parsed.tape().is_lossless());
        assert_eq!(parsed.tape().reconstruct(), source);
        assert_eq!(parsed.root_kind(), "translation_unit");
    }

    #[test]
    fn comments_macros_crlf_bom_and_unicode_are_classified() {
        let source = concat!(
            "\u{feff}# define DOUBLE(x) \\",
            "\r\n\t((x) + (x))\r\n",
            "/* comentário: λ */\r\n",
            "const char\t*text = \"olá mundo\"; // fim\\",
            "\ncontinua\n",
        );
        let mut parser = CParser::new().expect("embedded C grammar must load");
        let parsed = parser.parse(source).expect("source must produce a tree");
        let kinds = trivia_kinds(parsed.tape());

        assert!(parsed.tape().is_lossless());
        assert_eq!(parsed.tape().reconstruct(), source);
        assert!(kinds.contains(&TriviaKind::Bom));
        assert!(kinds.contains(&TriviaKind::Spaces));
        assert!(kinds.contains(&TriviaKind::Tabs));
        assert!(kinds.contains(&TriviaKind::Newline(LineEnding::CrLf)));
        assert!(kinds.contains(&TriviaKind::Newline(LineEnding::Lf)));
        assert!(kinds.contains(&TriviaKind::BlockComment));
        assert!(kinds.contains(&TriviaKind::LineComment));
        assert!(kinds.contains(&TriviaKind::EscapedNewline(LineEnding::CrLf)));
        assert!(kinds.contains(&TriviaKind::EscapedNewline(LineEnding::Lf)));
    }

    #[test]
    fn enum_array_bound_is_valid_c_syntax_not_a_vla_guess() {
        let source = concat!(
            "typedef enum e_op\n",
            "{\n",
            "\top_sa,\n",
            "\top_sb,\n",
            "\top_total\n",
            "}\tt_op;\n\n",
            "typedef struct s_context\n",
            "{\n",
            "\tint\tcount[op_total];\n",
            "}\tt_context;\n",
        );
        let mut parser = CParser::new().expect("embedded C grammar must load");
        let parsed = parser.parse(source).expect("fixture must produce a tree");

        assert!(
            !parsed.has_syntax_errors(),
            "an enum constant array bound is syntactically valid C: {:?}",
            parsed.issues()
        );
        assert_eq!(parsed.tape().reconstruct(), source);
        assert_eq!(parsed.root_kind(), "translation_unit");
    }

    #[test]
    fn unclassified_gap_is_unknown_and_still_round_trips() {
        let source = "int\rvalue;\n";
        let mut parser = CParser::new().expect("embedded C grammar must load");
        let parsed = parser.parse(source).expect("source must produce a tree");

        assert!(parsed.tape().has_unknown());
        assert!(!parsed.permits_automatic_edits());
        assert!(parsed.tape().is_lossless());
        assert_eq!(parsed.tape().reconstruct(), source);
    }

    #[test]
    fn pathological_but_valid_utf8_input_does_not_panic() {
        let samples = [
            "",
            "\0",
            "\u{10ffff}",
            "/* unterminated",
            "\"unterminated\n",
            "\\\r\n",
            "(((((((((((((((((((((((((((((((((((((((((",
        ];
        let mut parser = CParser::new().expect("embedded C grammar must load");

        for source in samples {
            let parsed = parser
                .parse(source)
                .expect("Tree-sitter should recover from arbitrary UTF-8 input");
            assert!(parsed.tape().is_lossless());
            assert_eq!(parsed.tape().reconstruct(), source);
        }
    }
}
