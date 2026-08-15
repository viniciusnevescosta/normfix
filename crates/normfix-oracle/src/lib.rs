//! Bounded, deterministic external-tool adapters for `normfix`.
//!
//! This crate never invokes a shell. It verifies the official Norminette
//! release, materializes in-memory sources in isolated temporary directories,
//! bounds wall time and output, and keeps operational failures separate from
//! source diagnostics. It also exposes a Tree-sitter token-tape proof for
//! layout-only edits and an optional syntax-only C compiler validator.
//!
//! One adapter here runs a program rather than reading one: the Valgrind leak
//! checker. It is the exception, it is never reached by a default run, and the
//! module says why the boundary is drawn where it is.

#![forbid(unsafe_code)]

mod clang_tidy;
mod compiler;
mod executable;
mod norminette;
mod process;
mod token;
mod valgrind;

pub use clang_tidy::{ClangTidy, ClangTidyConfig, ClangTidyError, ClangTidyFinding};
pub use compiler::{
    CompilerConfig, CompilerError, CompilerFingerprint, CompilerReport, CompilerValidator,
};
pub use norminette::{
    NorminetteConfig, NorminetteDiagnostic, NorminetteError, NorminetteFingerprint,
    NorminetteOracle, NorminetteReport, SUPPORTED_NORMINETTE_VERSION,
};
pub use process::{BoundedOutput, ProcessError, ProcessLimits};
pub use token::{
    SignificantTokenFingerprint, TokenPreservation, TokenProofError,
    prove_significant_tokens_preserved, significant_token_fingerprint,
};
pub use valgrind::{
    LeakLocation, LeakSite, MemoryError, ValgrindChecker, ValgrindConfig, ValgrindError,
    ValgrindReport,
};
