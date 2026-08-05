//! Bounded, deterministic external-tool adapters for `normfix`.
//!
//! This crate never invokes a shell. It verifies the official Norminette
//! release, materializes in-memory sources in isolated temporary directories,
//! bounds wall time and output, and keeps operational failures separate from
//! source diagnostics. It also exposes a Tree-sitter token-tape proof for
//! layout-only edits and an optional syntax-only C compiler validator.

#![forbid(unsafe_code)]

mod compiler;
mod executable;
mod norminette;
mod process;
mod token;

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
