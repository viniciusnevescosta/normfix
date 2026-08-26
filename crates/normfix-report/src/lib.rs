//! Stable machine reports and source-aware terminal diagnostics.
//!
//! Analysis crates emit backend-neutral data. This crate is the only layer
//! responsible for ANSI styling, snippets, tables, diffs and the versioned
//! JSON contract.

#![forbid(unsafe_code)]

mod evaluation;
mod human;
mod json;
mod model;
mod source;
mod terminal;

pub use evaluation::{EvaluationFinding, EvaluationGrade, EvaluationReport, EvaluationVerdict};
pub use human::{RenderOptions, render_findings, render_human, unified_diff};
pub use model::{
    FileReport, FileStatus, FunctionBudget, REPORT_SCHEMA_VERSION, ReportIdentity, ReportMode,
    ReportSummary, RunReport,
};
pub use source::source_map;

#[cfg(test)]
mod tests;
