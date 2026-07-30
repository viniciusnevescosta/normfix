//! Native orchestration for inspection, formatting, validation and writes.
//!
//! The engine builds immutable snapshots and shadow buffers first. Fix mode
//! reaches the filesystem only through the validated multi-file transaction;
//! the legacy [`inspect`] entry point remains available for read-only callers.

#![forbid(unsafe_code)]

mod pipeline;

pub use pipeline::{BackupPolicy, FixOptions, FixRunError, run_fixes};

use std::path::{Path, PathBuf};
use std::sync::Arc;

use camino::Utf8PathBuf;
use normfix_c_syntax::{CParser, ParseFailure, SyntaxIssueKind};
use normfix_core::{FileId, SourceSnapshot};
use normfix_project::{DiscoveryError, DiscoveryOptions, ProjectFileKind, discover};
use rayon::ThreadPoolBuilder;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Implemented native-engine migration milestones.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MigrationPhase {
    /// Workspace, core types, deterministic discovery and lossless C parsing.
    Foundation,
    /// Complete native formatting, validation, reporting and write pipeline.
    NativeFixer,
}

/// Returns the highest migration phase implemented by this build.
#[must_use]
pub const fn migration_phase() -> MigrationPhase {
    MigrationPhase::NativeFixer
}

/// Configuration for one read-only inspection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InspectOptions {
    /// Directory used for relative inputs and stable display paths.
    pub cwd: PathBuf,
    /// Whether directory discovery respects Git ignore rules.
    pub respect_gitignore: bool,
    /// Worker count, or `None` for Rayon's automatic choice.
    pub threads: Option<usize>,
}

impl InspectOptions {
    /// Creates default read-only options rooted at `cwd`.
    #[must_use]
    pub fn new(cwd: impl Into<PathBuf>) -> Self {
        Self {
            cwd: cwd.into(),
            respect_gitignore: false,
            threads: None,
        }
    }
}

/// Stable report produced by [`inspect`].
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct InspectionReport {
    /// Native migration milestone used for this report.
    pub migration_phase: MigrationPhase,
    /// Successfully read files in canonical discovery order.
    pub files: Vec<FileInspection>,
    /// Unexpected files found during directory scans.
    pub unexpected_files: Vec<String>,
    /// Discovery, I/O, encoding and parser failures.
    pub failures: Vec<InspectionFailure>,
}

impl InspectionReport {
    /// Returns the process exit code matching the Python CLI contract.
    ///
    /// `0` is clean, `1` means syntax review remains and `2` means an
    /// operational or lossless-reconstruction failure occurred.
    #[must_use]
    pub fn exit_code(&self) -> u8 {
        if !self.failures.is_empty() || self.files.iter().any(|file| !file.lossless) {
            return 2;
        }
        if self.files.iter().any(|file| !file.syntax_issues.is_empty()) {
            return 1;
        }
        0
    }

    /// Returns the total number of parser recovery issues.
    #[must_use]
    pub fn syntax_issue_count(&self) -> usize {
        self.files.iter().map(|file| file.syntax_issues.len()).sum()
    }
}

/// Read-only metadata for one file.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FileInspection {
    /// Stable path relative to the current directory when possible.
    pub path: String,
    /// Supported file kind.
    pub kind: InspectionFileKind,
    /// File length in bytes.
    pub bytes: usize,
    /// Lowercase BLAKE3 content hash.
    pub content_hash: String,
    /// Whether the token tape reconstructed every source byte.
    pub lossless: bool,
    /// Backend-neutral C root name; absent for Makefiles.
    pub root_kind: Option<String>,
    /// Parser recovery issues; empty for clean C and Makefiles.
    pub syntax_issues: Vec<SyntaxIssueReport>,
    /// Whether this milestone permits automatic edits to the file.
    pub automatic_edits_permitted: bool,
}

/// File kinds exposed by the native report schema.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InspectionFileKind {
    /// C implementation file.
    CSource,
    /// C header.
    CHeader,
    /// Makefile (read-only in this migration slice).
    Makefile,
    /// README Markdown document.
    Markdown,
}

impl From<ProjectFileKind> for InspectionFileKind {
    fn from(value: ProjectFileKind) -> Self {
        match value {
            ProjectFileKind::CSource => Self::CSource,
            ProjectFileKind::CHeader => Self::CHeader,
            ProjectFileKind::Makefile => Self::Makefile,
            ProjectFileKind::Markdown => Self::Markdown,
        }
    }
}

/// Backend-neutral parser recovery issue.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SyntaxIssueReport {
    /// `error` or `missing`.
    pub kind: String,
    /// Inclusive UTF-8 byte start.
    pub start_byte: u32,
    /// Exclusive UTF-8 byte end.
    pub end_byte: u32,
    /// Grammar name represented as text.
    pub syntax_kind: String,
}

/// One non-fatal inspection failure.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct InspectionFailure {
    /// Best available stable path.
    pub path: String,
    /// Stable category.
    pub code: String,
    /// English explanation.
    pub message: String,
}

/// A report could not be scheduled.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum InspectError {
    /// Zero worker threads is invalid.
    #[error("thread count must be at least one")]
    ZeroThreads,
    /// Rayon could not create the requested local pool.
    #[error("could not create the inspection worker pool: {0}")]
    ThreadPool(String),
}

/// Discovers and losslessly inspects C/header/Makefile inputs.
///
/// This function never writes files. Results are sorted after parallel work so
/// worker completion order cannot affect human or JSON output.
///
/// # Errors
///
/// Returns [`InspectError`] only when an explicit worker configuration cannot
/// be honored. Per-file and discovery failures remain inside the report.
pub fn inspect(
    inputs: &[PathBuf],
    options: &InspectOptions,
) -> Result<InspectionReport, InspectError> {
    if options.threads == Some(0) {
        return Err(InspectError::ZeroThreads);
    }
    let discovery_options =
        DiscoveryOptions::new(&options.cwd).with_respect_gitignore(options.respect_gitignore);
    let discovery = discover(inputs, &discovery_options);
    let work = || {
        discovery
            .processable_files
            .par_iter()
            .enumerate()
            .map_init(CParser::new, |parser, (index, file)| {
                inspect_file(index, &file.path, file.kind, &options.cwd, parser)
            })
            .collect::<Vec<_>>()
    };
    let outcomes = if let Some(threads) = options.threads {
        ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .map_err(|error| InspectError::ThreadPool(error.to_string()))?
            .install(work)
    } else {
        work()
    };

    let mut files = Vec::new();
    let mut failures = discovery
        .errors
        .iter()
        .map(|error| discovery_failure(error, &options.cwd))
        .collect::<Vec<_>>();
    for outcome in outcomes {
        match outcome {
            Ok(file) => files.push(file),
            Err(failure) => failures.push(failure),
        }
    }
    files.sort_by(|left, right| left.path.cmp(&right.path));
    failures.sort();
    let mut unexpected_files = discovery
        .unexpected_files
        .iter()
        .map(|path| display_path(path, &options.cwd))
        .collect::<Vec<_>>();
    unexpected_files.sort();

    Ok(InspectionReport {
        migration_phase: migration_phase(),
        files,
        unexpected_files,
        failures,
    })
}

fn inspect_file(
    index: usize,
    path: &Path,
    kind: ProjectFileKind,
    cwd: &Path,
    parser: &mut Result<CParser, ParseFailure>,
) -> Result<FileInspection, InspectionFailure> {
    let bytes = std::fs::read(path).map_err(|error| InspectionFailure {
        path: display_path(path, cwd),
        code: "read_error".to_owned(),
        message: format!("Could not read the file: {error}"),
    })?;
    let report_path = display_path(path, cwd);
    if matches!(kind, ProjectFileKind::Makefile | ProjectFileKind::Markdown) {
        return Ok(FileInspection {
            path: report_path,
            kind: kind.into(),
            bytes: bytes.len(),
            content_hash: blake3::hash(&bytes).to_hex().to_string(),
            lossless: true,
            root_kind: None,
            syntax_issues: Vec::new(),
            automatic_edits_permitted: false,
        });
    }
    if bytes.contains(&0) {
        return Err(InspectionFailure {
            path: report_path,
            code: "binary_file".to_owned(),
            message: "Refused to parse a C file containing NUL bytes.".to_owned(),
        });
    }
    let source = String::from_utf8(bytes).map_err(|error| InspectionFailure {
        path: report_path.clone(),
        code: "invalid_utf8".to_owned(),
        message: format!("The C file is not valid UTF-8: {error}"),
    })?;
    let file_id = u32::try_from(index)
        .map(FileId::new)
        .map_err(|_| InspectionFailure {
            path: report_path.clone(),
            code: "too_many_files".to_owned(),
            message: "The project contains more files than compact file IDs support.".to_owned(),
        })?;
    let snapshot_path =
        snapshot_relative_path(path, cwd, index).ok_or_else(|| InspectionFailure {
            path: report_path.clone(),
            code: "non_utf8_path".to_owned(),
            message: "The file path is not valid UTF-8.".to_owned(),
        })?;
    let snapshot =
        SourceSnapshot::new(file_id, snapshot_path, Arc::<str>::from(source)).map_err(|error| {
            InspectionFailure {
                path: report_path.clone(),
                code: "snapshot_error".to_owned(),
                message: error.to_string(),
            }
        })?;
    let parser = parser.as_mut().map_err(|error| InspectionFailure {
        path: report_path.clone(),
        code: "parser_initialization".to_owned(),
        message: error.to_string(),
    })?;
    let parsed = parser
        .parse_arc(Arc::clone(snapshot.text()))
        .map_err(|error| InspectionFailure {
            path: report_path.clone(),
            code: "parser_failure".to_owned(),
            message: error.to_string(),
        })?;
    let syntax_issues = parsed
        .issues()
        .iter()
        .map(|issue| SyntaxIssueReport {
            kind: match issue.kind() {
                SyntaxIssueKind::Error => "error",
                SyntaxIssueKind::Missing => "missing",
            }
            .to_owned(),
            start_byte: issue.range().start().get(),
            end_byte: issue.range().end().get(),
            syntax_kind: issue.syntax_kind().to_owned(),
        })
        .collect();
    let lossless = parsed.tape().is_lossless();
    Ok(FileInspection {
        path: report_path,
        kind: kind.into(),
        bytes: snapshot.text().len(),
        content_hash: snapshot.content_hash().to_hex().to_string(),
        lossless,
        root_kind: Some(parsed.root_kind().to_owned()),
        syntax_issues,
        automatic_edits_permitted: lossless && parsed.permits_automatic_edits(),
    })
}

fn snapshot_relative_path(path: &Path, cwd: &Path, index: usize) -> Option<Utf8PathBuf> {
    let candidate = path
        .strip_prefix(cwd)
        .ok()
        .filter(|path| !path.as_os_str().is_empty());
    if let Some(relative) = candidate {
        return Utf8PathBuf::from_path_buf(relative.to_path_buf()).ok();
    }
    let name = path.file_name()?.to_str()?;
    Some(Utf8PathBuf::from(format!("external/{index}/{name}")))
}

fn discovery_failure(error: &DiscoveryError, cwd: &Path) -> InspectionFailure {
    InspectionFailure {
        path: display_path(&error.path, cwd),
        code: "discovery_error".to_owned(),
        message: error.to_string(),
    }
}

fn display_path(path: &Path, cwd: &Path) -> String {
    path.strip_prefix(cwd)
        .ok()
        .filter(|relative| !relative.as_os_str().is_empty())
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::{InspectError, InspectOptions, inspect};

    #[test]
    fn reports_enum_bound_as_clean_syntax_without_guessing_vla_semantics() {
        let temporary = TempDir::new().expect("temporary directory");
        let header = temporary.path().join("push_swap.h");
        fs::write(
            &header,
            concat!(
                "typedef enum e_op\n",
                "{\n",
                "\top_sa,\n",
                "\top_total\n",
                "}\tt_op;\n",
                "typedef struct s_context\n",
                "{\n",
                "\tint\tcount[op_total];\n",
                "}\tt_context;\n",
            ),
        )
        .expect("fixture");

        let report = inspect(&[], &InspectOptions::new(temporary.path())).expect("inspection");

        assert_eq!(report.exit_code(), 0);
        assert_eq!(report.files.len(), 1);
        assert!(report.files[0].lossless);
        assert!(report.files[0].syntax_issues.is_empty());
        assert!(report.files[0].automatic_edits_permitted);
    }

    #[test]
    fn one_and_four_threads_produce_identical_reports() {
        let temporary = TempDir::new().expect("temporary directory");
        fs::write(temporary.path().join("b.c"), "int\tb(void);\n").expect("b.c");
        fs::write(temporary.path().join("a.h"), "int\ta(void);\n").expect("a.h");
        fs::write(temporary.path().join(".DS_Store"), "metadata").expect("unexpected");

        let mut one = InspectOptions::new(temporary.path());
        one.threads = Some(1);
        let mut four = one.clone();
        four.threads = Some(4);

        assert_eq!(
            inspect(&[], &one).expect("single-thread inspection"),
            inspect(&[], &four).expect("parallel inspection")
        );
    }

    #[test]
    fn syntax_recovery_causes_review_exit_without_enabling_edits() {
        let temporary = TempDir::new().expect("temporary directory");
        fs::write(temporary.path().join("broken.c"), "int main( {\n").expect("fixture");

        let report = inspect(&[], &InspectOptions::new(temporary.path())).expect("inspection");

        assert_eq!(report.exit_code(), 1);
        assert!(report.syntax_issue_count() > 0);
        assert!(!report.files[0].automatic_edits_permitted);
    }

    #[test]
    fn binary_c_input_is_a_structured_operational_failure() {
        let temporary = TempDir::new().expect("temporary directory");
        fs::write(temporary.path().join("binary.c"), b"int\0main(void);\n").expect("fixture");

        let report = inspect(&[], &InspectOptions::new(temporary.path())).expect("inspection");

        assert_eq!(report.exit_code(), 2);
        assert!(report.files.is_empty());
        assert_eq!(report.failures.len(), 1);
        assert_eq!(report.failures[0].code, "binary_file");
        assert_eq!(report.failures[0].path, "binary.c");
    }

    #[test]
    fn invalid_utf8_is_a_structured_operational_failure() {
        let temporary = TempDir::new().expect("temporary directory");
        fs::write(temporary.path().join("invalid.c"), b"int main(void);\n\xff").expect("fixture");

        let report = inspect(&[], &InspectOptions::new(temporary.path())).expect("inspection");

        assert_eq!(report.exit_code(), 2);
        assert!(report.files.is_empty());
        assert_eq!(report.failures.len(), 1);
        assert_eq!(report.failures[0].code, "invalid_utf8");
        assert_eq!(report.failures[0].path, "invalid.c");
    }

    #[test]
    fn a_missing_explicit_input_is_reported_without_panicking() {
        let temporary = TempDir::new().expect("temporary directory");
        let input = std::path::PathBuf::from("missing.c");

        let report = inspect(&[input], &InspectOptions::new(temporary.path())).expect("inspection");

        assert_eq!(report.exit_code(), 2);
        assert!(report.files.is_empty());
        assert_eq!(report.failures.len(), 1);
        assert_eq!(report.failures[0].code, "discovery_error");
        assert_eq!(report.failures[0].path, "missing.c");
    }

    #[test]
    fn inspection_preserves_input_bytes() {
        let temporary = TempDir::new().expect("temporary directory");
        let source = temporary.path().join("source.c");
        let original = b"int\tanswer(void)\n{\n\treturn (42);\n}\n";
        fs::write(&source, original).expect("fixture");

        let report = inspect(&[], &InspectOptions::new(temporary.path())).expect("inspection");

        assert_eq!(report.exit_code(), 0);
        assert_eq!(fs::read(source).expect("read after inspection"), original);
    }

    #[test]
    fn zero_worker_threads_is_rejected_before_inspection() {
        let temporary = TempDir::new().expect("temporary directory");
        let mut options = InspectOptions::new(temporary.path());
        options.threads = Some(0);

        assert_eq!(inspect(&[], &options), Err(InspectError::ZeroThreads));
    }

    #[test]
    fn a_reconstruction_mismatch_is_an_operational_failure() {
        let report = super::InspectionReport {
            migration_phase: super::MigrationPhase::Foundation,
            files: vec![super::FileInspection {
                path: "source.c".to_owned(),
                kind: super::InspectionFileKind::CSource,
                bytes: 1,
                content_hash: "hash".to_owned(),
                lossless: false,
                root_kind: Some("translation_unit".to_owned()),
                syntax_issues: Vec::new(),
                automatic_edits_permitted: false,
            }],
            unexpected_files: Vec::new(),
            failures: Vec::new(),
        };

        assert_eq!(report.exit_code(), 2);
    }
}
