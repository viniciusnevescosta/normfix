//! Lossless parsed context and exact token proofs.

use normfix_c_syntax::{CParser, ParsedFile, SyntaxFacts, TapePiece, TriviaKind};

use crate::CActionError;
use crate::source::{LexicalMap, SourceLines};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FingerprintMode {
    TokensAndComments,
    CodeOnly,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Token {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) text: String,
}

#[derive(Clone, Debug)]
pub(crate) struct ParsedContext {
    parsed: ParsedFile,
    tokens: Vec<Token>,
    lexical: LexicalMap,
}

impl ParsedContext {
    pub(crate) fn parse(parser: &mut CParser, source: &str) -> Result<Self, CActionError> {
        let parsed = parser.parse(source)?;
        let mut tokens = Vec::new();
        for piece in parsed.tape().pieces() {
            let TapePiece::Token(token) = piece else {
                continue;
            };
            let start = token.range().start().get() as usize;
            let end = token.range().end().get() as usize;
            let Some(text) = parsed.source().get(start..end) else {
                return Err(CActionError::UnsafeSyntax);
            };
            tokens.push(Token {
                start,
                end,
                text: text.to_owned(),
            });
        }
        let lexical = LexicalMap::scan(parsed.source());
        Ok(Self {
            parsed,
            tokens,
            lexical,
        })
    }

    pub(crate) fn require_safe(&self) -> Result<(), CActionError> {
        if self.parsed.permits_automatic_edits() && self.parsed.tape().is_lossless() {
            Ok(())
        } else {
            Err(CActionError::UnsafeSyntax)
        }
    }

    pub(crate) fn source(&self) -> &str {
        self.parsed.source()
    }

    pub(crate) fn lines(&self) -> SourceLines<'_> {
        SourceLines::new(self.source())
    }

    pub(crate) fn tokens(&self) -> &[Token] {
        &self.tokens
    }

    pub(crate) fn facts(&self) -> &SyntaxFacts {
        self.parsed.facts()
    }

    pub(crate) const fn lexical(&self) -> &LexicalMap {
        &self.lexical
    }

    pub(crate) fn fingerprint(&self, mode: FingerprintMode) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        let mut previous_preprocessor: Option<String> = None;
        for piece in self.parsed.tape().pieces() {
            match piece {
                TapePiece::Token(token) => {
                    let start = token.range().start().get() as usize;
                    let end = token.range().end().get() as usize;
                    if let Some(text) = self.source().get(start..end) {
                        let preprocessor = token.syntax_kind().starts_with('#');
                        if preprocessor
                            && previous_preprocessor
                                .as_deref()
                                .is_some_and(|previous_kind| previous_kind == token.syntax_kind())
                        {
                            continue;
                        }
                        let canonical_text = if preprocessor {
                            token.syntax_kind()
                        } else {
                            text
                        };
                        hash_atom(&mut hasher, token.syntax_kind(), canonical_text);
                        previous_preprocessor =
                            preprocessor.then(|| token.syntax_kind().to_owned());
                    }
                }
                TapePiece::Trivia(trivia)
                    if mode == FingerprintMode::TokensAndComments
                        && matches!(
                            trivia.kind(),
                            TriviaKind::LineComment | TriviaKind::BlockComment
                        ) =>
                {
                    let start = trivia.range().start().get() as usize;
                    let end = trivia.range().end().get() as usize;
                    if let Some(text) = self.source().get(start..end) {
                        let kind = match trivia.kind() {
                            TriviaKind::LineComment => "line_comment",
                            TriviaKind::BlockComment => "block_comment",
                            _ => unreachable!("guarded above"),
                        };
                        hash_atom(&mut hasher, kind, text);
                    }
                    previous_preprocessor = None;
                }
                TapePiece::Trivia(trivia) => {
                    if matches!(
                        trivia.kind(),
                        TriviaKind::Newline(_) | TriviaKind::EscapedNewline(_)
                    ) {
                        previous_preprocessor = None;
                    }
                }
                TapePiece::Unknown(_) => previous_preprocessor = None,
            }
        }
        *hasher.finalize().as_bytes()
    }
}

fn hash_atom(hasher: &mut blake3::Hasher, kind: &str, text: &str) {
    let kind_len = u64::try_from(kind.len()).unwrap_or(u64::MAX);
    let text_len = u64::try_from(text.len()).unwrap_or(u64::MAX);
    hasher.update(&kind_len.to_le_bytes());
    hasher.update(kind.as_bytes());
    hasher.update(&text_len.to_le_bytes());
    hasher.update(text.as_bytes());
}
