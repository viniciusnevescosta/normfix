//! Lossless C syntax support for `normfix`.
//!
//! This crate deliberately keeps Tree-sitter behind a small, backend-neutral
//! API. [`CParser`] exposes parse errors and a full-fidelity [`TokenTape`], but
//! no `tree_sitter` type. Semantic questions such as whether an array bound is
//! a constant expression belong to a later semantic crate.

#![forbid(unsafe_code)]

mod facts;
mod parser;
mod tape;

pub use facts::{
    ArrayDeclaratorFact, CFunctionFact, CFunctionKind, CParameterFact, CStatementKind,
    CTypeTagKind, CallFact, EnumConstantFact, InitialDeclarationBlockFact, LocalDeclarationFact,
    LoopFact, MacroFact, NullCheckFact, RedundantElseFact, ReturnFact, SingleStatementBodyFact,
    SyntaxFacts, TypeTagFact,
};
pub use parser::{CParser, ParseFailure, ParsedFile, SyntaxIssue, SyntaxIssueKind};
pub use tape::{
    LineEnding, TapePiece, TokenPiece, TokenTape, TriviaKind, TriviaPiece, UnknownPiece,
};
