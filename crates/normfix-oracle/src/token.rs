use normfix_c_syntax::{CParser, ParseFailure, TapePiece};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Stable fingerprint of the significant C token stream.
#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct SignificantTokenFingerprint {
    /// BLAKE3 digest over token kinds and exact token bytes.
    pub digest: [u8; 32],
    /// Number of significant tokens included in the digest.
    pub token_count: usize,
}

/// Result of comparing significant tokens before and after a candidate edit.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TokenPreservation {
    /// Fingerprint of the original source.
    pub before: SignificantTokenFingerprint,
    /// Fingerprint of the candidate source.
    pub after: SignificantTokenFingerprint,
    /// Whether both token streams are byte-for-byte equivalent.
    pub preserved: bool,
}

/// A source cannot safely participate in a token-preservation proof.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum TokenProofError {
    /// The embedded C parser failed.
    #[error("could not parse C for token proof: {0}")]
    Parse(#[from] ParseFailure),
    /// Parser recovery or an unknown tape region made the proof incomplete.
    #[error(
        "token proof is unavailable: {syntax_issues} parser recovery issue(s), unknown region: {has_unknown}"
    )]
    UnsafeSyntax {
        /// Number of Tree-sitter recovery issues.
        syntax_issues: usize,
        /// Whether the full-fidelity tape contains an unknown region.
        has_unknown: bool,
    },
    /// A token range did not address the source snapshot.
    #[error("token range {start}..{end} is outside the source snapshot")]
    InvalidTokenRange {
        /// Inclusive byte start.
        start: u32,
        /// Exclusive byte end.
        end: u32,
    },
}

/// Fingerprints the exact significant token sequence in a C source snapshot.
///
/// Whitespace and comments are trivia and therefore excluded. Parser recovery
/// and unknown tape regions are rejected rather than silently ignored.
///
/// # Errors
///
/// Returns [`TokenProofError`] when the parser cannot prove complete token
/// coverage.
pub fn significant_token_fingerprint(
    source: &str,
) -> Result<SignificantTokenFingerprint, TokenProofError> {
    let mut parser = CParser::new()?;
    let parsed = parser.parse(source)?;
    if !parsed.permits_automatic_edits() {
        return Err(TokenProofError::UnsafeSyntax {
            syntax_issues: parsed.issues().len(),
            has_unknown: parsed.tape().has_unknown(),
        });
    }

    let mut hasher = blake3::Hasher::new();
    hasher.update(b"normfix-significant-c-tokens-v1\0");
    let mut token_count = 0usize;
    for piece in parsed.tape().pieces() {
        let TapePiece::Token(token) = piece else {
            continue;
        };
        let range = token.range();
        let start = usize::try_from(range.start().get()).map_err(|_| {
            TokenProofError::InvalidTokenRange {
                start: range.start().get(),
                end: range.end().get(),
            }
        })?;
        let end =
            usize::try_from(range.end().get()).map_err(|_| TokenProofError::InvalidTokenRange {
                start: range.start().get(),
                end: range.end().get(),
            })?;
        let text = source
            .get(start..end)
            .ok_or(TokenProofError::InvalidTokenRange {
                start: range.start().get(),
                end: range.end().get(),
            })?;
        hash_field(&mut hasher, token.syntax_kind().as_bytes());
        hash_field(&mut hasher, text.as_bytes());
        token_count = token_count.saturating_add(1);
    }
    Ok(SignificantTokenFingerprint {
        digest: *hasher.finalize().as_bytes(),
        token_count,
    })
}

/// Compares the significant C token stream across a candidate edit.
///
/// # Errors
///
/// Returns [`TokenProofError`] unless both sources have complete, recovery-free
/// syntax tapes.
pub fn prove_significant_tokens_preserved(
    before: &str,
    after: &str,
) -> Result<TokenPreservation, TokenProofError> {
    let before = significant_token_fingerprint(before)?;
    let after = significant_token_fingerprint(after)?;
    let preserved = before == after;
    Ok(TokenPreservation {
        before,
        after,
        preserved,
    })
}

fn hash_field(hasher: &mut blake3::Hasher, bytes: &[u8]) {
    hasher.update(&u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_le_bytes());
    hasher.update(bytes);
}

#[cfg(test)]
mod tests {
    use super::{
        TokenProofError, prove_significant_tokens_preserved, significant_token_fingerprint,
    };

    #[test]
    fn layout_and_comments_do_not_change_significant_tokens() {
        let before = "int\tanswer(void)\n{\n\treturn (42);\n}\n";
        let after = "int answer(void) /* explanation */\n{\n    return (42);\n}\n";
        let proof = prove_significant_tokens_preserved(before, after).expect("safe token proof");

        assert!(proof.preserved);
        assert_eq!(proof.before.token_count, proof.after.token_count);
    }

    #[test]
    fn token_text_changes_are_detected() {
        let proof = prove_significant_tokens_preserved(
            "int answer(void) { return (42); }\n",
            "int answer(void) { return (43); }\n",
        )
        .expect("both sources are valid C");

        assert!(!proof.preserved);
    }

    #[test]
    fn parser_recovery_is_not_accepted_as_a_proof() {
        let error = significant_token_fingerprint("int main( {\n").expect_err("must refuse");

        assert!(matches!(error, TokenProofError::UnsafeSyntax { .. }));
    }
}
