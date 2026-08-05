//! Pure in-memory API and WebAssembly bridge for the normfix playground.
//!
//! This crate deliberately depends only on parser, formatter, and report data
//! that can execute inside the browser sandbox. It never opens files, spawns a
//! checker or compiler, reads environment variables, or sends source code over
//! the network.

#![forbid(unsafe_code)]

use std::collections::BTreeSet;
use std::sync::Arc;

use camino::{Utf8Component, Utf8Path};
use normfix_c_actions::{CActionOptions, Fix, FunctionBudget, analyze_budget, apply_c_actions};
use normfix_core::{Applicability, Diagnostic, DiagnosticSource, LineIndex, Severity};
use serde::{Deserialize, Serialize};
use similar::TextDiff;
use thiserror::Error;

const SCHEMA_VERSION: u32 = 1;
const MAX_FILES: usize = 128;
const MAX_PATH_BYTES: usize = 240;
const MAX_FILE_BYTES: usize = 1024 * 1024;
const MAX_PROJECT_BYTES: usize = 4 * 1024 * 1024;

/// One browser-owned source file.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SourceFile {
    /// Relative display path. Only `.c` and `.h` files are accepted.
    pub path: String,
    /// UTF-8 source contents.
    pub source: String,
}

/// Complete input for one private browser run.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PlaygroundRequest {
    /// Files to format independently in memory.
    pub files: Vec<SourceFile>,
}

/// Capabilities and deliberate browser-only boundaries.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PlaygroundCapabilities {
    /// Features executed locally by this browser build.
    pub available: Vec<&'static str>,
    /// Features deliberately reserved for the desktop CLI.
    pub desktop_only: Vec<&'static str>,
    /// Stable description of how source data is handled.
    pub data_handling: &'static str,
}

impl Default for PlaygroundCapabilities {
    fn default() -> Self {
        Self {
            available: vec![
                "native_formatter",
                "native_diagnostics",
                "function_budget",
                "unified_diff",
            ],
            desktop_only: vec![
                "official_header",
                "project_header_guards",
                "external_norminette",
                "compiler_preflight",
                "makefile",
                "git_scope",
                "backups_and_undo",
            ],
            data_handling: "in_memory_only",
        }
    }
}

/// A source location ready for direct browser rendering.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct BrowserLocation {
    /// One-based physical line.
    pub line: u32,
    /// One-based display column using the Norm's four-column tab stops.
    pub column: u32,
}

/// One accepted formatter change.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct BrowserFix {
    /// Stable rule identifier.
    pub rule_id: String,
    /// Concise English description.
    pub description: String,
    /// One-based source line when known.
    pub line: Option<u32>,
    /// Safety proof category.
    pub applicability: &'static str,
}

/// One diagnostic ready for display in the playground.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct BrowserDiagnostic {
    /// Stable rule identifier.
    pub rule_id: String,
    /// Error, warning, or informational severity.
    pub severity: &'static str,
    /// Human-readable English summary.
    pub message: String,
    /// One-based location in the formatted output.
    pub location: Option<BrowserLocation>,
    /// Concrete next action when one is available.
    pub help: Option<String>,
    /// Supporting facts in stable order.
    pub notes: Vec<String>,
    /// Human-readable diagnostic producer.
    pub source: String,
}

/// Norm budget for one function.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct BrowserBudget {
    /// Function identifier.
    pub function: String,
    /// One-based definition line.
    pub line: u32,
    /// Body lines currently used.
    pub lines: u32,
    /// Maximum body lines.
    pub line_limit: u32,
    /// Local variables currently used.
    pub variables: u32,
    /// Maximum local variables.
    pub variable_limit: u32,
    /// Parameters currently used.
    pub parameters: u32,
    /// Maximum parameters.
    pub parameter_limit: u32,
}

/// Result for one input file. A syntax failure is isolated to this file.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct BrowserFileResult {
    /// Original relative display path.
    pub path: String,
    /// Proposed source, or the unchanged input when processing failed.
    pub formatted: String,
    /// Whether the proposed source differs byte-for-byte.
    pub changed: bool,
    /// Whether the formatter reached a fixed point.
    pub stable: bool,
    /// Accepted formatter changes.
    pub fixes: Vec<BrowserFix>,
    /// Diagnostics remaining after formatting.
    pub diagnostics: Vec<BrowserDiagnostic>,
    /// Per-function Norm budget.
    pub budget: Vec<BrowserBudget>,
    /// Unified diff with `a/` and `b/` paths.
    pub diff: String,
    /// Operational error for this file, if any.
    pub error: Option<String>,
}

/// Aggregate counters for a browser run.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
pub struct BrowserSummary {
    /// Files submitted.
    pub files: usize,
    /// Files with a proposed byte change.
    pub changed: usize,
    /// Accepted formatter fixes.
    pub fixes: usize,
    /// Remaining diagnostics.
    pub diagnostics: usize,
    /// Files that could not be safely processed.
    pub failed: usize,
}

/// Stable response returned to JavaScript.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PlaygroundResponse {
    /// JSON contract version.
    pub schema_version: u32,
    /// Browser-safe capabilities for this build.
    pub capabilities: PlaygroundCapabilities,
    /// Per-file results in deterministic path order.
    pub files: Vec<BrowserFileResult>,
    /// Aggregate counters.
    pub summary: BrowserSummary,
}

/// Invalid playground input or serialization failure.
#[derive(Debug, Error)]
pub enum PlaygroundError {
    /// The JSON request could not be decoded.
    #[error("invalid playground request: {0}")]
    InvalidJson(#[from] serde_json::Error),
    /// No source files were supplied.
    #[error("select or add at least one .c or .h file")]
    EmptyProject,
    /// The browser request exceeded its deterministic file-count bound.
    #[error("the playground accepts at most {MAX_FILES} files per run")]
    TooManyFiles,
    /// One source file exceeded the per-file memory bound.
    #[error("{path} exceeds the {MAX_FILE_BYTES}-byte browser limit")]
    FileTooLarge {
        /// Rejected path.
        path: String,
    },
    /// The combined source exceeded the project memory bound.
    #[error("the selected sources exceed the {MAX_PROJECT_BYTES}-byte browser limit")]
    ProjectTooLarge,
    /// A display path was unsafe or unsupported.
    #[error("unsupported source path `{path}`: {reason}")]
    InvalidPath {
        /// Rejected path.
        path: String,
        /// Deterministic rejection reason.
        reason: &'static str,
    },
    /// Two files used the same path.
    #[error("duplicate source path `{0}`")]
    DuplicatePath(String),
}

/// Formats and analyzes a complete browser-owned request in memory.
///
/// Invalid project metadata rejects the request. A C parse failure is instead
/// recorded on only the affected file, allowing the rest of a multi-file run
/// to finish.
///
/// # Errors
///
/// Returns [`PlaygroundError`] for invalid paths, duplicate paths, or bounded
/// input limits.
pub fn format_project(request: PlaygroundRequest) -> Result<PlaygroundResponse, PlaygroundError> {
    validate_request(&request)?;
    let options = CActionOptions::default();
    let mut files = request
        .files
        .into_iter()
        .map(|file| format_file(file, &options))
        .collect::<Vec<_>>();
    files.sort_by(|left, right| left.path.cmp(&right.path));
    let summary = BrowserSummary {
        files: files.len(),
        changed: files.iter().filter(|file| file.changed).count(),
        fixes: files.iter().map(|file| file.fixes.len()).sum(),
        diagnostics: files.iter().map(|file| file.diagnostics.len()).sum(),
        failed: files.iter().filter(|file| file.error.is_some()).count(),
    };
    Ok(PlaygroundResponse {
        schema_version: SCHEMA_VERSION,
        capabilities: PlaygroundCapabilities::default(),
        files,
        summary,
    })
}

/// JSON-in/JSON-out adapter shared by native tests and the WASM export.
///
/// # Errors
///
/// Returns [`PlaygroundError`] when the JSON or request is invalid.
pub fn format_project_json(input: &str) -> Result<String, PlaygroundError> {
    let request = serde_json::from_str(input)?;
    let response = format_project(request)?;
    serde_json::to_string(&response).map_err(PlaygroundError::from)
}

/// Browser export. Errors become JavaScript exceptions with safe messages.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(js_name = formatProject)]
pub fn format_project_wasm(input: &str) -> Result<String, wasm_bindgen::JsValue> {
    format_project_json(input).map_err(|error| wasm_bindgen::JsValue::from_str(&error.to_string()))
}

fn validate_request(request: &PlaygroundRequest) -> Result<(), PlaygroundError> {
    if request.files.is_empty() {
        return Err(PlaygroundError::EmptyProject);
    }
    if request.files.len() > MAX_FILES {
        return Err(PlaygroundError::TooManyFiles);
    }
    let mut paths = BTreeSet::new();
    let mut project_bytes = 0_usize;
    for file in &request.files {
        validate_path(&file.path)?;
        if !paths.insert(file.path.clone()) {
            return Err(PlaygroundError::DuplicatePath(file.path.clone()));
        }
        if file.source.len() > MAX_FILE_BYTES {
            return Err(PlaygroundError::FileTooLarge {
                path: file.path.clone(),
            });
        }
        project_bytes = project_bytes.saturating_add(file.source.len());
        if project_bytes > MAX_PROJECT_BYTES {
            return Err(PlaygroundError::ProjectTooLarge);
        }
    }
    Ok(())
}

fn validate_path(path: &str) -> Result<(), PlaygroundError> {
    let parsed = Utf8Path::new(path);
    if path.is_empty() || path.chars().any(char::is_control) {
        return Err(invalid_path(
            path,
            "paths must be non-empty printable UTF-8",
        ));
    }
    if path.len() > MAX_PATH_BYTES {
        return Err(invalid_path(path, "paths must fit within 240 UTF-8 bytes"));
    }
    if !portable_tar_path(path) {
        return Err(invalid_path(
            path,
            "paths must fit the portable tar name fields (100-byte name and 155-byte prefix)",
        ));
    }
    if path.contains(['\\', ':']) {
        return Err(invalid_path(
            path,
            "backslashes and drive-like names are not portable",
        ));
    }
    if parsed.is_absolute()
        || parsed.components().any(|component| {
            matches!(
                component,
                Utf8Component::CurDir
                    | Utf8Component::ParentDir
                    | Utf8Component::RootDir
                    | Utf8Component::Prefix(_)
            )
        })
    {
        return Err(invalid_path(
            path,
            "use a canonical relative path without dot components",
        ));
    }
    let canonical = parsed
        .components()
        .filter_map(|component| match component {
            Utf8Component::Normal(name) => Some(name),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/");
    if canonical != path {
        return Err(invalid_path(
            path,
            "remove repeated separators and use the canonical relative spelling",
        ));
    }
    if !matches!(parsed.extension(), Some("c" | "h")) {
        return Err(invalid_path(path, "only .c and .h files are supported"));
    }
    Ok(())
}

fn portable_tar_path(path: &str) -> bool {
    if path.len() <= 100 {
        return true;
    }
    path.match_indices('/')
        .rev()
        .any(|(separator, _)| separator <= 155 && path.len().saturating_sub(separator + 1) <= 100)
}

fn invalid_path(path: &str, reason: &'static str) -> PlaygroundError {
    PlaygroundError::InvalidPath {
        path: path.to_owned(),
        reason,
    }
}

fn format_file(file: SourceFile, options: &CActionOptions) -> BrowserFileResult {
    let path = Utf8Path::new(&file.path);
    match apply_c_actions(path, &file.source, &[], options) {
        Ok(result) => {
            if !result.stable {
                return BrowserFileResult {
                    path: file.path,
                    formatted: file.source,
                    changed: false,
                    stable: false,
                    fixes: Vec::new(),
                    diagnostics: Vec::new(),
                    budget: Vec::new(),
                    diff: String::new(),
                    error: Some(
                        "the formatter did not reach a fixed point; partial output was discarded"
                            .to_owned(),
                    ),
                };
            }
            let formatted = result.source;
            let diagnostics = browser_diagnostics(&formatted, result.diagnostics);
            let budget = analyze_budget(path, &formatted)
                .unwrap_or_default()
                .into_iter()
                .map(BrowserBudget::from)
                .collect();
            let diff = unified_diff(&file.path, &file.source, &formatted);
            BrowserFileResult {
                path: file.path,
                changed: file.source != formatted,
                formatted,
                stable: result.stable,
                fixes: result.fixes.into_iter().map(BrowserFix::from).collect(),
                diagnostics,
                budget,
                diff,
                error: None,
            }
        }
        Err(error) => BrowserFileResult {
            path: file.path,
            formatted: file.source,
            changed: false,
            stable: false,
            fixes: Vec::new(),
            diagnostics: Vec::new(),
            budget: Vec::new(),
            diff: String::new(),
            error: Some(error.to_string()),
        },
    }
}

fn browser_diagnostics(source: &str, diagnostics: Vec<Diagnostic>) -> Vec<BrowserDiagnostic> {
    let index = LineIndex::new(Arc::from(source)).ok();
    diagnostics
        .into_iter()
        .map(|diagnostic| {
            let location = index.as_ref().and_then(|line_index| {
                line_index
                    .line_column(diagnostic.range.start())
                    .map(|position| BrowserLocation {
                        line: position.line,
                        column: position.visual_column,
                    })
            });
            BrowserDiagnostic {
                rule_id: diagnostic.rule_id,
                severity: severity_name(diagnostic.severity),
                message: diagnostic.message,
                location,
                help: diagnostic.help,
                notes: diagnostic.notes,
                source: source_name(&diagnostic.source),
            }
        })
        .collect()
}

fn unified_diff(path: &str, original: &str, formatted: &str) -> String {
    if original == formatted {
        return String::new();
    }
    TextDiff::from_lines(original, formatted)
        .unified_diff()
        .header(&format!("a/{path}"), &format!("b/{path}"))
        .to_string()
}

impl From<Fix> for BrowserFix {
    fn from(fix: Fix) -> Self {
        Self {
            rule_id: fix.rule_id,
            description: fix.description,
            line: fix.line,
            applicability: applicability_name(fix.applicability),
        }
    }
}

impl From<FunctionBudget> for BrowserBudget {
    fn from(budget: FunctionBudget) -> Self {
        Self {
            function: budget.function,
            line: budget.line,
            lines: budget.lines,
            line_limit: budget.line_limit,
            variables: budget.variables,
            variable_limit: budget.variable_limit,
            parameters: budget.parameters,
            parameter_limit: budget.parameter_limit,
        }
    }
}

const fn severity_name(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Info => "info",
    }
}

const fn applicability_name(applicability: Applicability) -> &'static str {
    match applicability {
        Applicability::SafeLayout => "safe_layout",
        Applicability::SafeSemantic => "safe_semantic",
        Applicability::ReviewRequired => "review_required",
        Applicability::UnsafeDestructive => "unsafe_destructive",
    }
}

fn source_name(source: &DiagnosticSource) -> String {
    match source {
        DiagnosticSource::NativeNorm41 => "Norm v4.1 native rule".to_owned(),
        DiagnosticSource::NorminetteCompat(version) => {
            format!("Norminette compatibility ({version})")
        }
        DiagnosticSource::Parser => "C parser".to_owned(),
        DiagnosticSource::Compiler => "C compiler".to_owned(),
        DiagnosticSource::Project => "project safety check".to_owned(),
        DiagnosticSource::Makefile => "Makefile check".to_owned(),
        DiagnosticSource::Markdown => "Markdown check".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        PlaygroundError, PlaygroundRequest, SourceFile, format_project, format_project_json,
    };

    fn request(path: &str, source: &str) -> PlaygroundRequest {
        PlaygroundRequest {
            files: vec![SourceFile {
                path: path.to_owned(),
                source: source.to_owned(),
            }],
        }
    }

    #[test]
    fn formats_in_memory_and_returns_diff_and_budget() {
        let response = format_project(request(
            "src/main.c",
            "int\tmain(void)   \n{\n\treturn (0);\t\n}\n",
        ))
        .expect("valid project");
        let file = &response.files[0];

        assert!(file.changed);
        assert!(file.error.is_none());
        assert!(file.diff.contains("--- a/src/main.c"));
        assert!(file.diff.contains("+++ b/src/main.c"));
        assert_eq!(file.budget[0].function, "main");
        assert_eq!(response.summary.files, 1);
        assert_eq!(response.summary.changed, 1);
    }

    #[test]
    fn isolates_an_unsafe_parse_to_its_file() {
        let response = format_project(PlaygroundRequest {
            files: vec![
                SourceFile {
                    path: "broken.c".to_owned(),
                    source: "int main( {\n".to_owned(),
                },
                SourceFile {
                    path: "ok.c".to_owned(),
                    source: "int main(void)\n{\n\treturn (0);\n}\n".to_owned(),
                },
            ],
        })
        .expect("request metadata is valid");

        assert_eq!(response.summary.failed, 1);
        assert!(response.files[0].error.is_some());
        assert!(response.files[1].error.is_none());
    }

    #[test]
    fn rejects_traversal_and_non_c_files() {
        let traversal = format_project(request("../main.c", "int x;\n"));
        let markdown = format_project(request("README.md", "hello\n"));

        assert!(matches!(
            traversal,
            Err(PlaygroundError::InvalidPath { .. })
        ));
        assert!(matches!(markdown, Err(PlaygroundError::InvalidPath { .. })));
    }

    #[test]
    fn rejects_noncanonical_and_platform_ambiguous_paths() {
        for path in ["./a.c", "src//a.c", "..\\evil.c", "C:/evil.c"] {
            let result = format_project(request(path, "int x;\n"));
            assert!(
                matches!(result, Err(PlaygroundError::InvalidPath { .. })),
                "accepted {path}"
            );
        }
    }

    #[test]
    fn rejects_paths_that_cannot_fit_the_portable_download_archive() {
        let path = format!("{}.c", "a".repeat(240));
        let result = format_project(request(&path, "int x;\n"));

        assert!(matches!(result, Err(PlaygroundError::InvalidPath { .. })));

        let overlong_basename = format!("{}.c", "a".repeat(99));
        assert!(matches!(
            format_project(request(&overlong_basename, "int x;\n")),
            Err(PlaygroundError::InvalidPath { .. })
        ));

        let nested = format!("{}/{}.c", "directory", "a".repeat(97));
        assert!(format_project(request(&nested, "int x;\n")).is_ok());
    }

    #[test]
    fn rejects_duplicate_paths() {
        let error = format_project(PlaygroundRequest {
            files: vec![
                SourceFile {
                    path: "a.c".to_owned(),
                    source: "int a;\n".to_owned(),
                },
                SourceFile {
                    path: "a.c".to_owned(),
                    source: "int b;\n".to_owned(),
                },
            ],
        })
        .expect_err("duplicates must fail");

        assert!(matches!(error, PlaygroundError::DuplicatePath(_)));
    }

    #[test]
    fn json_adapter_has_a_versioned_contract() {
        let input = serde_json::json!({
            "files": [{"path": "main.c", "source": "int main(void)\n{\n\treturn (0);\n}\n"}]
        });
        let output = format_project_json(&input.to_string()).expect("valid JSON request");
        let output: serde_json::Value = serde_json::from_str(&output).expect("valid JSON response");

        assert_eq!(output["schema_version"], 1);
        assert_eq!(output["capabilities"]["data_handling"], "in_memory_only");
        assert_eq!(output["files"][0]["path"], "main.c");
    }
}
