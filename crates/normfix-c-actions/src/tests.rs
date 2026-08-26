use std::fmt::Write;

use camino::Utf8Path;
use normfix_core::Severity;

use crate::{
    CActionError, CActionOptions, ReportedDiagnostic, analyze_budget, analyze_c,
    analyze_external_calls, apply_c_actions, normalize_hygiene, visual_width,
};

fn diagnostic(code: &str, line: u32, column: u32) -> ReportedDiagnostic {
    ReportedDiagnostic::new(code, line, column, code)
}

fn apply(source: &str, diagnostics: &[ReportedDiagnostic]) -> String {
    apply_c_actions(
        Utf8Path::new("fixture.c"),
        source,
        diagnostics,
        &CActionOptions::default(),
    )
    .expect("fixture must remain safe")
    .source
}

fn visual_column(line: &str, needle: &str) -> u32 {
    let index = line.find(needle).unwrap();
    visual_width(&line[..index]) + 1
}

mod analysis;
mod basics;
mod layout_regressions;
mod properties_and_includes;
mod semantic_rewrites;
