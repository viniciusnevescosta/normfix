//! End-to-end native fix pipeline.
//!
//! Analysis and formatting happen in immutable shadow buffers. The only write
//! boundary is the validated multi-file transaction in `normfix-actions`.

mod compiler;
mod diagnostics;
mod makefile;
mod markdown;
mod paths;
mod quarantine;

use compiler::{
    CompilerFamily, CompilerProjectContext, compiler_arguments, compiler_project_context,
    run_compiler_preflight,
};
use diagnostics::{
    ColumnUnit, diagnostic_range, explain_constant_array_false_positives, introduces_diagnostics,
    line_for_offset, line_point_range, merge_official_diagnostics, official_diagnostics,
    parser_diagnostics, point_diagnostic, project_diagnostic, text_range,
    untested_norminette_diagnostic,
};
use makefile::process_makefile;
use markdown::process_markdown;
use quarantine::quarantine_unexpected_files;

use paths::{
    absolute_lexical, automatic_backup_root, report_identity, report_path, run_id, transaction_root,
};

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt as _;

use camino::Utf8PathBuf;
use normfix_actions::{PlannedFile, ReadPrecondition, TransactionOptions, commit_files_guarded};
use normfix_c_actions::{
    CActionError, CActionOptions, ReportedDiagnostic, analyze_budget, analyze_c,
    analyze_external_calls, apply_c_actions,
};
use normfix_c_syntax::{CFunctionKind, CParser};
use normfix_cache::{CacheKey, CachePaths, PersistentCache, PreparedCacheEntry};
use normfix_core::{
    Diagnostic, DiagnosticSource, FileId, FixRecord, Severity, SourceSnapshot, TextRange, TextSize,
    apply_source_edits,
};
use normfix_destructive::{
    ClosedCSourceSet, DestructiveAuthorization, OrphanPrototypePlan, StaticRemovalPlan,
    plan_orphan_prototypes, plan_unused_static_functions,
};
use normfix_header::{
    Identity42, RunClock, c_header_filename_matches, ensure_c_header, update_c_header,
};
use normfix_oracle::{
    CompilerConfig, CompilerError, CompilerReport, CompilerValidator, NorminetteConfig,
    NorminetteError, NorminetteOracle, NorminetteReport, ProcessLimits,
};
use normfix_project::{
    DiscoveredFile, DiscoveryOptions, GuardApproval, GuardInsertionApproval, ProjectFileKind,
    ProjectPolicy, ProjectSnapshot, discover, guard_approval_is_current,
    guard_insertion_approval_is_current, load_project_policy, plan_guard_insertions,
    plan_guard_renames,
};
use normfix_report::{FileReport, ReportMode, RunReport};
use rayon::ThreadPoolBuilder;
use rayon::prelude::*;
use thiserror::Error;

/// Backup behavior for one fixing run.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BackupPolicy {
    /// Use the platform's external normfix data directory.
    Automatic,
    /// Use this external directory as the backup base.
    Directory(PathBuf),
    /// Do not retain original copies for ordinary formatting edits.
    Disabled,
}

/// Snapshot-bound approval for one interactive write.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WriteApproval {
    original_digest: [u8; 32],
    replacement_digest: [u8; 32],
}

impl WriteApproval {
    /// Binds an approval to the exact original and replacement bytes shown to
    /// the user.
    #[must_use]
    pub fn new(original: &[u8], replacement: &[u8]) -> Self {
        Self {
            original_digest: *blake3::hash(original).as_bytes(),
            replacement_digest: *blake3::hash(replacement).as_bytes(),
        }
    }

    fn permits(&self, plan: &PlannedFile) -> bool {
        self.original_digest == *blake3::hash(&plan.original).as_bytes()
            && self.replacement_digest == *blake3::hash(&plan.replacement).as_bytes()
    }
}

/// Complete native pipeline configuration.
//
// These booleans are independent CLI capabilities with different proof and
// authorization boundaries. Collapsing them into a state enum would permit
// invalid combinations or hide the exact capability enabled by the caller.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Debug)]
pub struct FixOptions {
    /// Directory used to resolve inputs and display relative paths.
    pub cwd: PathBuf,
    /// Fix, check or diff behavior.
    pub mode: ReportMode,
    /// Inspect the original bytes without proposing formatting edits.
    pub lint_only: bool,
    /// Add one informational Norm-budget diagnostic per parsed function.
    pub emit_budget: bool,
    /// Add pre-defense coverage and configuration diagnostics.
    pub preflight: bool,
    /// In fix mode, commit only these snapshot-bound absolute-path approvals
    /// while retaining a complete-project analysis snapshot. `None` commits
    /// every proven plan.
    pub write_approvals: Option<BTreeMap<PathBuf, WriteApproval>>,
    /// Treat an empty input list as an explicitly empty scope instead of the
    /// normal "scan the current directory" command-line behavior.
    pub empty_input_is_empty: bool,
    /// Unsupported files selected by an external scope resolver. They are
    /// reported as advisories without becoming explicit discovery failures.
    pub additional_unexpected_files: Vec<PathBuf>,
    /// Reusable run clock for a multi-stage interactive preview and commit.
    pub run_clock: Option<RunClock>,
    /// Respect Git ignore rules while walking directory inputs.
    pub respect_gitignore: bool,
    /// Worker count, or `None` for the hardware-aware Rayon default.
    pub threads: Option<usize>,
    /// Verified 42 identity, when available.
    pub identity: Option<Identity42>,
    /// Explanation of identity discovery or refusal.
    pub identity_source: String,
    /// Backup policy used only in fix mode.
    pub backup: BackupPolicy,
    /// Explicit official Norminette executable.
    pub norminette_executable: Option<PathBuf>,
    /// Run the real project through `cc -fsyntax-only -Wall -Wextra -Werror`.
    ///
    /// This is diagnostics-only and never participates in edit authorization.
    pub compiler_preflight: bool,
    /// Exact compiler executable for preflight, or `None` to resolve `cc`.
    pub compiler_executable: Option<PathBuf>,
    /// Run GCC `-fanalyzer` as an informational, fail-open advisory backend.
    pub analyzer: bool,
    /// Refuse a Norminette release the project has not verified.
    pub strict_norminette_version: bool,
    /// Per-file official-tool timeout.
    pub timeout: Duration,
    /// Enable the external content-addressed cache.
    pub cache: bool,
    /// Explicitly remove only comments rejected at exact official locations.
    pub remove_invalid_comments: bool,
    /// Compact simple standard NULL comparisons under explicit unsafe mode.
    pub compact_null_checks: bool,
    /// Reorder contiguous include blocks: system headers before project
    /// headers, alphabetically inside each category.
    pub reorder_includes: bool,
    /// Remove proven-missing or trivia-only paths from literal Makefile source lists.
    pub remove_missing_makefile_sources: bool,
    /// Remove project-local prototypes proven to have no definition or use.
    pub remove_orphan_prototypes: bool,
    /// Remove only unreachable `static` functions under explicit authorization.
    pub remove_unused_static: bool,
    /// Quarantine unexpected files under explicit authorization.
    pub quarantine_unexpected: bool,
    /// Capability-scoped grant for destructive operations.
    pub destructive_authorization: Option<DestructiveAuthorization>,
    /// Canonically format README files through a `CommonMark` syntax tree.
    pub format_markdown: bool,
    /// Maximum display columns for native C formatting.
    pub max_columns: u32,
    /// Maximum native fixed-point passes.
    pub max_passes: usize,
}

impl FixOptions {
    /// Creates production defaults rooted at `cwd`.
    #[must_use]
    pub fn new(cwd: impl Into<PathBuf>) -> Self {
        Self {
            cwd: cwd.into(),
            mode: ReportMode::Fix,
            lint_only: false,
            emit_budget: false,
            preflight: false,
            write_approvals: None,
            empty_input_is_empty: false,
            additional_unexpected_files: Vec::new(),
            run_clock: None,
            respect_gitignore: false,
            threads: None,
            identity: None,
            identity_source: "No verified 42 student email was found.".to_owned(),
            backup: BackupPolicy::Automatic,
            norminette_executable: None,
            compiler_preflight: true,
            compiler_executable: None,
            analyzer: false,
            strict_norminette_version: false,
            timeout: Duration::from_secs(5),
            cache: true,
            remove_invalid_comments: false,
            compact_null_checks: false,
            reorder_includes: true,
            remove_missing_makefile_sources: false,
            remove_orphan_prototypes: false,
            remove_unused_static: false,
            quarantine_unexpected: false,
            destructive_authorization: None,
            format_markdown: true,
            max_columns: 80,
            max_passes: 100,
        }
    }
}

/// Failure that prevents the complete run from being scheduled safely.
#[derive(Debug, Error)]
pub enum FixRunError {
    /// Zero workers is invalid.
    #[error("thread count must be at least one")]
    ZeroThreads,
    /// The one-shot header clock could not be captured.
    #[error("could not capture the official-header timestamp: {0}")]
    Clock(String),
    /// The required official compatibility oracle was unavailable.
    #[error(transparent)]
    Norminette(#[from] NorminetteError),
    /// Rayon could not construct the requested local pool.
    #[error("could not create the worker pool: {0}")]
    ThreadPool(String),
}

struct OracleContext {
    oracle: NorminetteOracle,
    norminette_notice_path: PathBuf,
    compiler: Option<CompilerValidator>,
    compiler_unavailable: Option<String>,
    compiler_notice_path: Option<PathBuf>,
    compiler_project_fingerprint: Option<[u8; 32]>,
    compiler_include_directories: Vec<PathBuf>,
    cache: Option<PersistentCache>,
    project_root: PathBuf,
}

impl OracleContext {
    fn lint(&self, path: &Path, source: &str) -> Result<NorminetteReport, NorminetteError> {
        let relative = path
            .strip_prefix(&self.project_root)
            .unwrap_or(path)
            .to_string_lossy();
        let key = CacheKey::derive(
            "norminette-3.3.59-parser-v2",
            &relative,
            source.as_bytes(),
            b"norm-v4.1",
            &self.oracle.fingerprint().digest,
        );
        if let Some(cache) = &self.cache {
            let cached = cache.lookup::<NorminetteReport>(key);
            if let Some(report) = cached.value {
                return Ok(report);
            }
        }
        let report = self.oracle.lint(path, source)?;
        if let Some(cache) = &self.cache {
            if let Ok(entry) = PreparedCacheEntry::new(key, &report) {
                let _ = cache.store(&entry);
            }
        }
        Ok(report)
    }

    fn compiler_preflight(
        &self,
        path: &Path,
        source: &str,
        analyzer: bool,
    ) -> Result<Option<CompilerReport>, CompilerError> {
        let Some(compiler) = &self.compiler else {
            return Ok(None);
        };
        let relative = path
            .strip_prefix(&self.project_root)
            .unwrap_or(path)
            .to_string_lossy();
        let family = CompilerFamily::from_version(&compiler.fingerprint().version_output);
        let namespace = if analyzer {
            family.analyzer_namespace()
        } else {
            "cc-wall-wextra-werror-v3"
        };
        let key = self.compiler_project_fingerprint.map(|fingerprint| {
            let mut configuration = Vec::with_capacity(33);
            configuration.extend_from_slice(&fingerprint);
            configuration.push(u8::from(analyzer));
            CacheKey::derive(
                namespace,
                &relative,
                source.as_bytes(),
                &configuration,
                &compiler.fingerprint().digest,
            )
        });
        if let (Some(cache), Some(key)) = (&self.cache, key) {
            let cached = cache.lookup::<CompilerReport>(key);
            if let Some(report) = cached.value {
                return Ok(Some(report));
            }
        }
        let arguments = compiler_arguments(analyzer, family, &self.compiler_include_directories);
        let before = std::fs::read(path).map_err(|error| {
            CompilerError::InvalidProjectSource(format!(
                "could not re-read `{}` before compiler preflight: {error}",
                path.display()
            ))
        })?;
        if before != source.as_bytes() {
            return Err(CompilerError::InvalidProjectSource(format!(
                "source `{}` changed before compiler preflight",
                path.display()
            )));
        }
        let report = compiler.validate_project_file(&self.project_root, path, &arguments)?;
        let after = std::fs::read(path).map_err(|error| {
            CompilerError::InvalidProjectSource(format!(
                "could not re-read `{}` after compiler preflight: {error}",
                path.display()
            ))
        })?;
        if after != source.as_bytes() {
            return Err(CompilerError::InvalidProjectSource(format!(
                "source `{}` changed during compiler preflight",
                path.display()
            )));
        }
        if let (Some(cache), Some(key)) = (&self.cache, key) {
            if let Ok(entry) = PreparedCacheEntry::new(key, &report) {
                let _ = cache.store(&entry);
            }
        }
        Ok(Some(report))
    }
}

pub(super) struct FileWork {
    absolute_path: PathBuf,
    report: FileReport,
    plan: Option<PlannedFile>,
    read_preconditions: Vec<ReadPrecondition>,
}

#[derive(Clone, Debug)]
struct DestructivePrelude {
    source: Option<String>,
    original_blake3: Option<[u8; 32]>,
    fixes: Vec<FixRecord>,
    diagnostics: Vec<Diagnostic>,
    read_preconditions: Vec<ReadPrecondition>,
}

impl DestructivePrelude {
    fn confirmed_source(&self, original: &[u8]) -> Option<&str> {
        if self.original_blake3 == Some(*blake3::hash(original).as_bytes()) {
            self.source.as_deref()
        } else {
            None
        }
    }
}

struct ClosedSourcePreparationError {
    path: PathBuf,
    message: String,
    help: &'static str,
}

#[derive(Clone, Debug)]
struct FunctionPolicyProof {
    policy: ProjectPolicy,
    external_definitions: BTreeSet<String>,
    source_digests: BTreeMap<PathBuf, [u8; 32]>,
}

#[derive(Clone, Debug, Default)]
struct FunctionPolicyPlan {
    proof: Option<FunctionPolicyProof>,
    diagnostics: BTreeMap<PathBuf, Vec<Diagnostic>>,
}

/// Discovers, formats, revalidates and optionally commits all selected files.
///
/// Files are processed in parallel but reports and commits are sorted by path.
/// In check and diff modes this function never writes a project file.
///
/// # Errors
///
/// Returns [`FixRunError`] only for run-wide prerequisites. Per-file I/O,
/// parser and transaction failures are represented inside the returned report.
// This is the intentionally linear orchestration boundary: keeping discovery,
// shadow analysis, commit, quarantine, and report construction in visible
// order makes the fail-closed transaction sequence auditable.
#[allow(clippy::too_many_lines)]
pub fn run_fixes(inputs: &[PathBuf], options: &FixOptions) -> Result<RunReport, FixRunError> {
    let started = Instant::now();
    if options.threads == Some(0) {
        return Err(FixRunError::ZeroThreads);
    }
    if inputs.is_empty() && options.empty_input_is_empty {
        let mut unexpected = options.additional_unexpected_files.clone();
        unexpected.sort();
        unexpected.dedup();
        let quarantine_candidates = if options.quarantine_unexpected {
            unexpected
                .iter()
                .filter_map(|path| report_path(path, &options.cwd).ok())
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let (quarantined, quarantine_errors) =
            if options.mode == ReportMode::Fix && options.quarantine_unexpected {
                quarantine_unexpected_files(&unexpected, options)
            } else {
                (Vec::new(), Vec::new())
            };
        let quarantined_set = quarantined.iter().cloned().collect::<BTreeSet<_>>();
        let unexpected_files = unexpected
            .iter()
            .filter_map(|path| report_path(path, &options.cwd).ok())
            .filter(|path| !quarantined_set.contains(path))
            .collect();
        let mut report = RunReport::new(
            env!("CARGO_PKG_VERSION"),
            options.mode,
            report_identity(options),
            Vec::new(),
            unexpected_files,
            Vec::new(),
            started.elapsed(),
        );
        report.set_quarantine_outcome(quarantine_candidates, quarantined, quarantine_errors);
        if options.preflight {
            report.enable_preflight_evaluation();
        }
        return Ok(report);
    }
    let clock = options
        .run_clock
        .clone()
        .map_or_else(RunClock::from_process_environment, Ok)
        .map_err(|error| FixRunError::Clock(error.to_string()))?;
    let discovery_options =
        DiscoveryOptions::new(&options.cwd).with_respect_gitignore(options.respect_gitignore);
    let discovery = discover(inputs, &discovery_options);
    let mut discovered_unexpected = discovery.unexpected_files.clone();
    discovered_unexpected.extend(options.additional_unexpected_files.iter().cloned());
    discovered_unexpected.sort();
    discovered_unexpected.dedup();
    let oracle = build_oracle_context(options, &discovery.processable_files)?;

    let header_paths = if options.lint_only {
        Vec::new()
    } else {
        discovery
            .processable_files
            .iter()
            .filter(|file| file.kind == ProjectFileKind::CHeader)
            .map(|file| file.path.clone())
            .collect::<Vec<_>>()
    };
    let (guard_approvals, guard_failure) = match plan_guard_renames(&header_paths) {
        Ok(approvals) => (approvals, None),
        Err(error) => (BTreeMap::new(), Some(error.to_string())),
    };
    let (guard_insertions, insertion_failure) = match plan_guard_insertions(&header_paths) {
        Ok(approvals) => (approvals, None),
        Err(error) => (BTreeMap::new(), Some(error.to_string())),
    };
    let guard_failure = guard_failure.or(insertion_failure);
    let destructive_preludes =
        plan_destructive_preludes(inputs, &discovery.processable_files, options);
    let dependent_destructive_bundle = dependent_destructive_bundle_paths(&destructive_preludes);
    let mut policy_plan = plan_policy_diagnostics(&discovery.processable_files, options);
    append_preflight_diagnostics(
        &mut policy_plan.diagnostics,
        &discovery.processable_files,
        options,
    );

    let execute = || {
        discovery
            .processable_files
            .par_iter()
            .map(|file| {
                process_file(
                    file,
                    options,
                    &clock,
                    oracle.as_ref(),
                    &guard_approvals,
                    &guard_insertions,
                    guard_failure.as_deref(),
                    destructive_preludes.get(&file.path),
                    policy_plan.diagnostics.get(&file.path).map(Vec::as_slice),
                )
            })
            .collect::<Vec<_>>()
    };
    let mut work = if let Some(threads) = options.threads {
        ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .map_err(|error| FixRunError::ThreadPool(error.to_string()))?
            .install(execute)
    } else {
        execute()
    };
    work.sort_by(|left, right| left.absolute_path.cmp(&right.absolute_path));
    append_function_policy_diagnostics(&mut work, policy_plan.proof.as_ref(), &options.cwd);

    let commit_succeeded = options.mode != ReportMode::Fix
        || commit_work(&mut work, options, &dependent_destructive_bundle);
    let quarantine_candidates = if options.quarantine_unexpected {
        discovered_unexpected
            .iter()
            .filter_map(|path| report_path(path, &options.cwd).ok())
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let (quarantined, quarantine_errors) =
        if options.mode == ReportMode::Fix && options.quarantine_unexpected && commit_succeeded {
            quarantine_unexpected_files(&discovered_unexpected, options)
        } else {
            (Vec::new(), Vec::new())
        };

    let identity = report_identity(options);
    let discovery_errors = discovery
        .errors
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    let quarantined_set = quarantined.iter().cloned().collect::<BTreeSet<_>>();
    let unexpected_files = discovered_unexpected
        .iter()
        .filter_map(|path| report_path(path, &options.cwd).ok())
        .filter(|path| !quarantined_set.contains(path))
        .collect::<Vec<_>>();
    let files = work.into_iter().map(|item| item.report).collect();
    let mut report = RunReport::new(
        env!("CARGO_PKG_VERSION"),
        options.mode,
        identity,
        discovery_errors,
        unexpected_files,
        files,
        started.elapsed(),
    );
    report.set_quarantine_outcome(quarantine_candidates, quarantined, quarantine_errors);
    if options.preflight {
        report.enable_preflight_evaluation();
    }
    Ok(report)
}

fn build_oracle_context(
    options: &FixOptions,
    files: &[DiscoveredFile],
) -> Result<Option<OracleContext>, FixRunError> {
    let has_c_family = files.iter().any(|file| {
        matches!(
            file.kind,
            ProjectFileKind::CSource | ProjectFileKind::CHeader
        )
    });
    if !has_c_family {
        return Ok(None);
    }
    let oracle = NorminetteOracle::locate(NorminetteConfig {
        executable: options.norminette_executable.clone(),
        expected_version: normfix_oracle::SUPPORTED_NORMINETTE_VERSION.to_owned(),
        strict_version: options.strict_norminette_version,
        limits: ProcessLimits {
            timeout: options.timeout,
            output_bytes: 1024 * 1024,
        },
    })?;
    let cache = options
        .cache
        .then(|| CachePaths::for_project(&options.cwd).ok())
        .flatten()
        .map(PersistentCache::open);
    let norminette_notice_path = files
        .iter()
        .find(|file| {
            matches!(
                file.kind,
                ProjectFileKind::CSource | ProjectFileKind::CHeader
            )
        })
        .map(|file| file.path.clone())
        .expect("a C-family file was established above");
    let compiler_notice_path = files
        .iter()
        .find(|file| file.kind == ProjectFileKind::CSource)
        .map(|file| file.path.clone());
    let has_c_source = compiler_notice_path.is_some();
    let (compiler, compiler_unavailable) =
        if has_c_source && (options.compiler_preflight || options.analyzer || options.preflight) {
            match CompilerValidator::locate(CompilerConfig {
                executable: options.compiler_executable.clone(),
                limits: ProcessLimits {
                    timeout: options.timeout.max(Duration::from_secs(10)),
                    output_bytes: 2 * 1024 * 1024,
                },
            }) {
                Ok(compiler) => (Some(compiler), None),
                Err(error) => (None, Some(error.to_string())),
            }
        } else {
            (None, None)
        };
    let compiler_project = if compiler.is_some() {
        compiler_project_context(&options.cwd)
    } else {
        CompilerProjectContext::default()
    };
    Ok(Some(OracleContext {
        oracle,
        norminette_notice_path,
        compiler,
        compiler_unavailable,
        compiler_notice_path,
        compiler_project_fingerprint: compiler_project.fingerprint,
        compiler_include_directories: compiler_project.include_directories,
        cache,
        project_root: absolute_lexical(&options.cwd),
    }))
}

// This is the single audit boundary that sequences complete discovery, scoped
// authorization, closed snapshots, and both destructive C planners.
#[allow(clippy::too_many_lines)]
fn plan_destructive_preludes(
    _inputs: &[PathBuf],
    selected: &[DiscoveredFile],
    options: &FixOptions,
) -> BTreeMap<PathBuf, DestructivePrelude> {
    let selected_c = selected
        .iter()
        .filter(|file| {
            matches!(
                file.kind,
                ProjectFileKind::CSource | ProjectFileKind::CHeader
            )
        })
        .map(|file| file.path.clone())
        .collect::<BTreeSet<_>>();
    if selected_c.is_empty() {
        return BTreeMap::new();
    }
    let complete = discover(
        &[],
        &DiscoveryOptions::new(&options.cwd)
            .with_respect_gitignore(false)
            .with_respect_normfixignore(false),
    );
    let complete_c = complete
        .processable_files
        .iter()
        .filter(|file| {
            matches!(
                file.kind,
                ProjectFileKind::CSource | ProjectFileKind::CHeader
            )
        })
        .map(|file| file.path.clone())
        .collect::<BTreeSet<_>>();
    let mut preludes = BTreeMap::new();
    let destructive_requested = options.remove_unused_static || options.remove_orphan_prototypes;
    if !complete.errors.is_empty() {
        if !destructive_requested {
            return preludes;
        }
        attach_destructive_diagnostic(
            &mut preludes,
            selected_c.iter().next().expect("non-empty selected set"),
            &options.cwd,
            "PROJECT_CLOSED_SET_INCOMPLETE",
            "Project-wide implementation analysis was skipped because complete C/header discovery failed.",
            "Resolve every discovery error and rerun from the project root.",
        );
        return preludes;
    }
    let closed = match prepare_closed_source_set(&complete_c, &options.cwd) {
        Ok(closed) => closed,
        Err(error) => {
            if !destructive_requested {
                return preludes;
            }
            attach_destructive_diagnostic(
                &mut preludes,
                &error.path,
                &options.cwd,
                "PROJECT_CLOSED_SET_INVALID",
                &error.message,
                error.help,
            );
            return preludes;
        }
    };
    let mut read_preconditions = vec![ReadPrecondition::project_sources(
        absolute_lexical(&options.cwd),
        complete_c.iter().cloned(),
    )];
    read_preconditions.extend(closed.snapshots().iter().map(|snapshot| {
        ReadPrecondition::Matches {
            path: options.cwd.join(snapshot.relative_path().as_std_path()),
            blake3: *snapshot.content_hash().as_bytes(),
        }
    }));

    if options.remove_unused_static {
        if selected_c != complete_c {
            attach_destructive_diagnostic(
                &mut preludes,
                selected_c.iter().next().expect("non-empty selected set"),
                &options.cwd,
                "UNSAFE_STATIC_CLOSED_SET_INCOMPLETE",
                "Unused static functions were not removed because the selected inputs are not the complete project C/header set.",
                "Run from the project root without partial paths or ignored C files.",
            );
        } else if let Some(authorization) = options.destructive_authorization.as_ref() {
            match plan_unused_static_functions(&closed, authorization) {
                Ok(plan) => apply_static_removal_plan(
                    &mut preludes,
                    plan,
                    &options.cwd,
                    &read_preconditions,
                ),
                Err(error) => attach_destructive_diagnostic(
                    &mut preludes,
                    selected_c.iter().next().expect("non-empty selected set"),
                    &options.cwd,
                    "UNSAFE_STATIC_PLAN_FAILED",
                    &format!("Unused static function planning failed: {error}"),
                    "Review the closed project inputs and destructive authorization.",
                ),
            }
        } else {
            attach_destructive_diagnostic(
                &mut preludes,
                selected_c.iter().next().expect("non-empty selected set"),
                &options.cwd,
                "UNSAFE_AUTHORIZATION_REQUIRED",
                "Unused static functions were not removed because destructive authorization is missing.",
                "Confirm the terminal warning or pass --unsafe --force.",
            );
        }
    }

    let can_remove = options.remove_orphan_prototypes
        && selected_c == complete_c
        && options.destructive_authorization.is_some();
    if options.remove_orphan_prototypes && selected_c != complete_c {
        attach_destructive_diagnostic(
            &mut preludes,
            selected_c.iter().next().expect("non-empty selected set"),
            &options.cwd,
            "UNSAFE_ORPHAN_PROTOTYPE_CLOSED_SET_INCOMPLETE",
            "Orphan header prototypes were not removed because the selected inputs are not the complete project C/header set.",
            "Run from the project root so every declaration and use participates in the proof.",
        );
    } else if options.remove_orphan_prototypes && options.destructive_authorization.is_none() {
        attach_destructive_diagnostic(
            &mut preludes,
            selected_c.iter().next().expect("non-empty selected set"),
            &options.cwd,
            "UNSAFE_AUTHORIZATION_REQUIRED",
            "Orphan header prototypes were not removed because destructive authorization is missing.",
            "Confirm the terminal warning or pass --unsafe --force.",
        );
    }
    let transformed =
        prepare_closed_source_set_with_overrides(&complete_c, &options.cwd, &preludes);
    match transformed {
        Ok(transformed) => {
            let authorization = can_remove
                .then_some(options.destructive_authorization.as_ref())
                .flatten();
            match plan_orphan_prototypes(&transformed, authorization) {
                Ok(plan) => apply_orphan_prototype_plan(
                    &mut preludes,
                    plan,
                    &options.cwd,
                    &selected_c,
                    can_remove,
                    &read_preconditions,
                ),
                Err(error) => attach_destructive_diagnostic(
                    &mut preludes,
                    selected_c.iter().next().expect("non-empty selected set"),
                    &options.cwd,
                    "ORPHAN_PROTOTYPE_PLAN_FAILED",
                    &format!("Prototype implementation analysis failed: {error}"),
                    "Review the complete project inputs and destructive authorization.",
                ),
            }
        }
        Err(error) => attach_destructive_diagnostic(
            &mut preludes,
            &error.path,
            &options.cwd,
            "ORPHAN_PROTOTYPE_CLOSED_SET_INVALID",
            &error.message,
            error.help,
        ),
    }
    preludes
}

fn plan_policy_diagnostics(
    selected: &[DiscoveredFile],
    options: &FixOptions,
) -> FunctionPolicyPlan {
    if !selected.iter().any(|file| {
        matches!(
            file.kind,
            ProjectFileKind::CSource | ProjectFileKind::CHeader
        )
    }) {
        return FunctionPolicyPlan::default();
    }
    let policy = match load_project_policy(&options.cwd) {
        Ok(Some(policy)) => policy,
        Ok(None) => {
            if !options.preflight {
                return FunctionPolicyPlan::default();
            }
            let Some(file) = selected.iter().find(|file| {
                matches!(
                    file.kind,
                    ProjectFileKind::CSource | ProjectFileKind::CHeader
                )
            }) else {
                return FunctionPolicyPlan::default();
            };
            let path = report_path(&file.path, &options.cwd)
                .unwrap_or_else(|_| Utf8PathBuf::from(file.path.to_string_lossy().as_ref()));
            return FunctionPolicyPlan {
                proof: None,
                diagnostics: BTreeMap::from([(
                    file.path.clone(),
                    vec![project_diagnostic(
                        path,
                        "FUNCTION_POLICY_NOT_CONFIGURED",
                        "Authorized-function checking is unavailable because this project has no normfix.toml allowlist.",
                        "Create normfix.toml from the subject's exact authorized-function list before relying on preflight.",
                    )],
                )]),
            };
        }
        Err(error) => {
            let Some(file) = selected.first() else {
                return FunctionPolicyPlan::default();
            };
            let path = report_path(&file.path, &options.cwd)
                .unwrap_or_else(|_| Utf8PathBuf::from(file.path.to_string_lossy().as_ref()));
            return FunctionPolicyPlan {
                proof: None,
                diagnostics: BTreeMap::from([(
                    file.path.clone(),
                    vec![project_diagnostic(
                        path,
                        "PROJECT_POLICY_INVALID",
                        &error.to_string(),
                        "Fix normfix.toml; the allowed-function check was skipped for this run.",
                    )],
                )]),
            };
        }
    };
    match build_function_policy_proof(policy, selected, &options.cwd) {
        Ok(proof) => FunctionPolicyPlan {
            proof: Some(proof),
            diagnostics: BTreeMap::new(),
        },
        Err(reason) => function_policy_incomplete_plan(selected, &options.cwd, &reason),
    }
}

fn build_function_policy_proof(
    policy: ProjectPolicy,
    selected: &[DiscoveredFile],
    project_root: &Path,
) -> Result<FunctionPolicyProof, String> {
    let discovery = discover(
        &[],
        &DiscoveryOptions::new(project_root)
            .with_respect_gitignore(false)
            .with_respect_normfixignore(false),
    );
    if let Some(error) = discovery.errors.first() {
        return Err(format!(
            "Complete-project C/header discovery failed: {error}"
        ));
    }
    let project_sources = discovery
        .processable_files
        .iter()
        .filter(|file| {
            matches!(
                file.kind,
                ProjectFileKind::CSource | ProjectFileKind::CHeader
            )
        })
        .collect::<Vec<_>>();
    let discovered_paths = project_sources
        .iter()
        .map(|file| file.path.as_path())
        .collect::<BTreeSet<_>>();
    if let Some(file) = selected.iter().find(|file| {
        matches!(
            file.kind,
            ProjectFileKind::CSource | ProjectFileKind::CHeader
        ) && !discovered_paths.contains(file.path.as_path())
    }) {
        return Err(format!(
            "Selected C/header input `{}` is outside the complete project discovery rooted at `{}`.",
            file.path.display(),
            project_root.display()
        ));
    }
    let mut parser = CParser::new()
        .map_err(|error| format!("The native C parser could not be initialized: {error}"))?;
    let mut external_definitions = BTreeSet::new();
    let mut source_digests = BTreeMap::new();
    for file in project_sources {
        let bytes = std::fs::read(&file.path).map_err(|error| {
            format!(
                "Complete-project source `{}` could not be read: {error}",
                file.path.display()
            )
        })?;
        source_digests.insert(file.path.clone(), *blake3::hash(&bytes).as_bytes());
        let source = String::from_utf8(bytes).map_err(|error| {
            format!(
                "Complete-project source `{}` is not valid UTF-8: {error}",
                file.path.display()
            )
        })?;
        let parsed = parser.parse(&source).map_err(|error| {
            format!(
                "Complete-project source `{}` could not be parsed losslessly: {error}",
                file.path.display()
            )
        })?;
        if !parsed.permits_automatic_edits() || !parsed.tape().is_lossless() {
            return Err(format!(
                "Complete-project source `{}` required ambiguous syntax recovery.",
                file.path.display()
            ));
        }
        external_definitions.extend(
            parsed
                .facts()
                .functions
                .iter()
                .filter(|function| {
                    function.kind == CFunctionKind::Definition && !function.is_static
                })
                .map(|function| function.name.clone()),
        );
    }
    Ok(FunctionPolicyProof {
        policy,
        external_definitions,
        source_digests,
    })
}

fn function_policy_incomplete_plan(
    selected: &[DiscoveredFile],
    project_root: &Path,
    reason: &str,
) -> FunctionPolicyPlan {
    let Some(file) = selected.iter().find(|file| {
        matches!(
            file.kind,
            ProjectFileKind::CSource | ProjectFileKind::CHeader
        )
    }) else {
        return FunctionPolicyPlan::default();
    };
    let path = report_path(&file.path, project_root)
        .unwrap_or_else(|_| Utf8PathBuf::from(file.path.to_string_lossy().as_ref()));
    FunctionPolicyPlan {
        proof: None,
        diagnostics: BTreeMap::from([(
            file.path.clone(),
            vec![project_diagnostic(
                path,
                "FUNCTION_POLICY_PROOF_INCOMPLETE",
                &format!(
                    "Authorized-function findings were disabled because the complete-project proof is incomplete: {reason}"
                ),
                "Make every project C/header input readable and losslessly parseable, then retry from the project root.",
            )],
        )]),
    }
}

fn append_function_policy_diagnostics(
    work: &mut [FileWork],
    proof: Option<&FunctionPolicyProof>,
    project_root: &Path,
) {
    let Some(proof) = proof else {
        return;
    };
    if let Err(reason) = validate_function_policy_snapshot(proof, project_root) {
        append_function_policy_incomplete_diagnostic(work, project_root, &reason);
        return;
    }
    let mut findings = Vec::<(usize, Diagnostic)>::new();
    let mut incomplete = None;
    for (index, item) in work.iter().enumerate().filter(|(_, item)| {
        ProjectFileKind::from_path(&item.absolute_path) == Some(ProjectFileKind::CSource)
    }) {
        let Some(source) = item.report.fixed.as_deref() else {
            incomplete = Some(format!(
                "Final source bytes were unavailable for `{}`.",
                item.absolute_path.display()
            ));
            break;
        };
        let candidates = match analyze_external_calls(item.report.path.as_path(), source) {
            Ok(candidates) => candidates,
            Err(error) => {
                incomplete = Some(format!(
                    "Final source `{}` could not be parsed losslessly: {error}",
                    item.absolute_path.display()
                ));
                break;
            }
        };
        let policy_label = proof.policy.name.as_deref().map_or_else(
            || "this project".to_owned(),
            |name| format!("project `{name}`"),
        );
        for candidate in candidates {
            if proof.external_definitions.contains(&candidate.name)
                || proof.policy.allowed_functions.contains(&candidate.name)
            {
                continue;
            }
            findings.push((
                index,
                Diagnostic {
                    rule_id: "FUNCTION_NOT_ALLOWED".to_owned(),
                    path: candidate.path,
                    range: candidate.name_range,
                    severity: Severity::Warning,
                    message: format!(
                        "External call `{}` is not listed as allowed for {policy_label}.",
                        candidate.name
                    ),
                    source: DiagnosticSource::Project,
                    notes: vec![format!(
                        "Policy source: {}. Same-file definitions, non-static project definitions, and recoverable function-pointer calls were excluded.",
                        proof.policy.path.display()
                    )],
                    help: Some(
                        "Remove the call or add it to [project].allowed only when the 42 subject explicitly permits it."
                            .to_owned(),
                    ),
                },
            ));
        }
    }
    if let Some(reason) = incomplete {
        append_function_policy_incomplete_diagnostic(work, project_root, &reason);
        return;
    }
    for (index, diagnostic) in findings {
        work[index].report.after.push(diagnostic);
    }
    for item in work {
        item.report.after.sort();
        item.report.after.dedup();
    }
}

fn validate_function_policy_snapshot(
    proof: &FunctionPolicyProof,
    project_root: &Path,
) -> Result<(), String> {
    match load_project_policy(project_root) {
        Ok(Some(current)) if current == proof.policy => {}
        Ok(Some(_)) => return Err("normfix.toml changed after policy planning.".to_owned()),
        Ok(None) => return Err("normfix.toml disappeared after policy planning.".to_owned()),
        Err(error) => {
            return Err(format!(
                "normfix.toml could not be revalidated after policy planning: {error}"
            ));
        }
    }
    let discovery = discover(
        &[],
        &DiscoveryOptions::new(project_root)
            .with_respect_gitignore(false)
            .with_respect_normfixignore(false),
    );
    if let Some(error) = discovery.errors.first() {
        return Err(format!(
            "Complete-project C/header discovery changed or failed during the run: {error}"
        ));
    }
    let current_paths = discovery
        .processable_files
        .into_iter()
        .filter(|file| {
            matches!(
                file.kind,
                ProjectFileKind::CSource | ProjectFileKind::CHeader
            )
        })
        .map(|file| file.path)
        .collect::<BTreeSet<_>>();
    let planned_paths = proof
        .source_digests
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();
    if current_paths != planned_paths {
        return Err("The complete project C/header file set changed during the run.".to_owned());
    }
    for (path, expected) in &proof.source_digests {
        let bytes = std::fs::read(path).map_err(|error| {
            format!(
                "Complete-project source `{}` could not be re-read: {error}",
                path.display()
            )
        })?;
        if blake3::hash(&bytes).as_bytes() != expected {
            return Err(format!(
                "Complete-project source `{}` changed during the run.",
                path.display()
            ));
        }
    }
    Ok(())
}

fn append_function_policy_incomplete_diagnostic(
    work: &mut [FileWork],
    project_root: &Path,
    reason: &str,
) {
    let Some(item) = work.iter_mut().find(|item| {
        matches!(
            ProjectFileKind::from_path(&item.absolute_path),
            Some(ProjectFileKind::CSource | ProjectFileKind::CHeader)
        )
    }) else {
        return;
    };
    let path = report_path(&item.absolute_path, project_root)
        .unwrap_or_else(|_| Utf8PathBuf::from(item.absolute_path.to_string_lossy().as_ref()));
    item.report.after.push(project_diagnostic(
        path,
        "FUNCTION_POLICY_PROOF_INCOMPLETE",
        &format!(
            "Authorized-function findings were disabled because the final-source proof is incomplete: {reason}"
        ),
        "Make every selected C source losslessly parseable and retry; no allowlist finding from this run is authoritative.",
    ));
    item.report.after.sort();
    item.report.after.dedup();
}

// Every preflight notice is written here so the complete set of pre-defense
// advisories stays readable as one sequence instead of scattered emitters.
#[allow(clippy::too_many_lines)]
fn append_preflight_diagnostics(
    diagnostics: &mut BTreeMap<PathBuf, Vec<Diagnostic>>,
    selected: &[DiscoveredFile],
    options: &FixOptions,
) {
    if !options.preflight {
        return;
    }
    let Some(notice_file) = selected.iter().find(|file| {
        matches!(
            file.kind,
            ProjectFileKind::CSource | ProjectFileKind::CHeader
        )
    }) else {
        return;
    };
    let selected_makefiles = selected
        .iter()
        .filter(|file| file.kind == ProjectFileKind::Makefile)
        .map(|file| absolute_lexical(&file.path))
        .collect::<BTreeSet<_>>();
    let root_makefiles = root_regular_makefiles(&options.cwd);
    let unevaluated_root_makefile = root_makefiles
        .iter()
        .any(|path| !selected_makefiles.contains(path));
    if selected_makefiles.is_empty() && root_makefiles.is_empty() {
        let path = report_path(&notice_file.path, &options.cwd)
            .unwrap_or_else(|_| Utf8PathBuf::from(notice_file.path.to_string_lossy().as_ref()));
        diagnostics
            .entry(notice_file.path.clone())
            .or_default()
            .push(Diagnostic {
                rule_id: "MAKEFILE_NOT_FOUND".to_owned(),
                path,
                range: TextRange::empty(TextSize::new(0)),
                severity: Severity::Info,
                message:
                    "No regular Makefile was selected or found at the project root; build verification is incomplete."
                        .to_owned(),
                source: DiagnosticSource::Project,
                notes: vec![
                    "Some 42 subjects do not require a Makefile, so absence alone is not a hard fail without subject-specific policy."
                        .to_owned(),
                ],
                help: Some(
                    "Check the current subject and evaluation sheet; add or select the required Makefile before relying on preflight."
                        .to_owned(),
                ),
            });
    } else if unevaluated_root_makefile {
        let path = report_path(&notice_file.path, &options.cwd)
            .unwrap_or_else(|_| Utf8PathBuf::from(notice_file.path.to_string_lossy().as_ref()));
        diagnostics
            .entry(notice_file.path.clone())
            .or_default()
            .push(Diagnostic {
                rule_id: "MAKEFILE_NOT_EVALUATED".to_owned(),
                path,
                range: TextRange::empty(TextSize::new(0)),
                severity: Severity::Warning,
                message:
                    "A regular Makefile exists at the project root but was not selected, so preflight did not evaluate it."
                        .to_owned(),
                source: DiagnosticSource::Project,
                notes: vec![
                    "Its header, targets, recipes, and source references are absent from this report."
                        .to_owned(),
                ],
                help: Some(
                    "Include the root Makefile explicitly, or run preflight from the project root without a partial file scope."
                        .to_owned(),
                ),
            });
    }
    let Some(file) = selected
        .iter()
        .find(|file| file.kind == ProjectFileKind::CSource)
    else {
        return;
    };
    let path = report_path(&file.path, &options.cwd)
        .unwrap_or_else(|_| Utf8PathBuf::from(file.path.to_string_lossy().as_ref()));
    let clang_tidy = executable_on_path("clang-tidy").map_or_else(
        || "clang-tidy was not found on PATH; install it only if the project allows an additional local advisory pass.".to_owned(),
        |executable| {
            format!(
                "clang-tidy is available at `{}`; run it with the project's real include paths and compile flags, and review findings manually.",
                executable.display()
            )
        },
    );
    diagnostics
        .entry(file.path.clone())
        .or_default()
        .push(Diagnostic {
            rule_id: "PREFLIGHT_MANUAL_STEPS".to_owned(),
            path,
            range: TextRange::empty(TextSize::new(0)),
            severity: Severity::Info,
            message:
                "Preflight does not execute project recipes, binaries, interactive tests, or runtime leak tools."
                    .to_owned(),
            source: DiagnosticSource::Project,
            notes: vec![
                "Run the subject's required make/relink sequence and functional tests in the evaluator environment."
                    .to_owned(),
                if options.preflight {
                    "Preflight automatically runs the bounded compiler analyzer; its findings are advisory and are not a runtime leak proof."
                        .to_owned()
                } else if options.analyzer {
                    "A compiler analyzer was requested, but its findings are advisory and are not a runtime leak proof."
                        .to_owned()
                } else {
                    "Use --analyzer for an additional static advisory, then confirm memory ownership at runtime."
                        .to_owned()
                },
                "For a separate local debug build, use AddressSanitizer and UndefinedBehaviorSanitizer (`-fsanitize=address,undefined -fno-omit-frame-pointer -g`) when your compiler supports them; do not silently change the submitted Makefile flags."
                    .to_owned(),
                "LeakSanitizer support varies by compiler and operating system; run the subject's required leak tool as the final runtime check."
                    .to_owned(),
                clang_tidy,
            ],
            help: Some(
                "Complete the subject-specific manual checks shown in the evaluation sheet before defense."
                    .to_owned(),
            ),
        });
}

fn root_regular_makefiles(root: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut makefiles = entries
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.eq_ignore_ascii_case("makefile"))
                && std::fs::symlink_metadata(entry.path())
                    .is_ok_and(|metadata| metadata.is_file() && !metadata.file_type().is_symlink())
        })
        .map(|entry| absolute_lexical(&entry.path()))
        .collect::<Vec<_>>();
    makefiles.sort();
    makefiles.dedup();
    makefiles
}

fn executable_on_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    let executable_name = OsString::from(format!("{name}{}", std::env::consts::EXE_SUFFIX));
    std::env::split_paths(&path)
        .filter(|directory| directory.is_absolute())
        .map(|directory| directory.join(&executable_name))
        .find(|candidate| {
            std::fs::metadata(candidate).is_ok_and(|metadata| is_executable_file(&metadata))
        })
}

fn is_executable_file(metadata: &std::fs::Metadata) -> bool {
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn prepare_closed_source_set(
    selected: &BTreeSet<PathBuf>,
    cwd: &Path,
) -> Result<ClosedCSourceSet, ClosedSourcePreparationError> {
    let mut snapshots = Vec::with_capacity(selected.len());
    for (index, absolute) in selected.iter().enumerate() {
        let bytes = std::fs::read(absolute).map_err(|error| ClosedSourcePreparationError {
            path: absolute.clone(),
            message: format!("A C source could not be read for the closed-world proof: {error}"),
            help: "Make every selected C/header file readable and retry.",
        })?;
        let source = String::from_utf8(bytes).map_err(|error| ClosedSourcePreparationError {
            path: absolute.clone(),
            message: format!("A C source is not valid UTF-8: {error}"),
            help: "Convert the source to UTF-8 before destructive analysis.",
        })?;
        let relative =
            absolute
                .strip_prefix(cwd)
                .map_err(|error| ClosedSourcePreparationError {
                    path: absolute.clone(),
                    message: format!("A selected C source is outside the project root: {error}"),
                    help: "Run destructive analysis from one complete project root.",
                })?;
        let relative = Utf8PathBuf::from_path_buf(relative.to_path_buf()).map_err(|path| {
            ClosedSourcePreparationError {
                path: absolute.clone(),
                message: format!("A selected C path is not valid UTF-8: {}", path.display()),
                help: "Rename the path before destructive analysis.",
            }
        })?;
        let file_id = u32::try_from(index).map_err(|error| ClosedSourcePreparationError {
            path: absolute.clone(),
            message: format!("The closed source set is too large to index safely: {error}"),
            help: "Reduce the project to fewer than 2^32 C/header files.",
        })?;
        let snapshot =
            SourceSnapshot::new(FileId::new(file_id), relative, Arc::<str>::from(source)).map_err(
                |error| ClosedSourcePreparationError {
                    path: absolute.clone(),
                    message: format!("A C source snapshot was rejected: {error}"),
                    help: "Use project-relative UTF-8 paths and sources smaller than 4 GiB.",
                },
            )?;
        snapshots.push(snapshot);
    }
    ClosedCSourceSet::from_complete_discovery(snapshots).map_err(|error| {
        ClosedSourcePreparationError {
            path: selected
                .iter()
                .next()
                .cloned()
                .unwrap_or_else(|| cwd.to_path_buf()),
            message: format!("The closed C source set was rejected: {error}"),
            help: "Run from one complete project root with unique C/header paths.",
        }
    })
}

fn prepare_closed_source_set_with_overrides(
    selected: &BTreeSet<PathBuf>,
    cwd: &Path,
    overrides: &BTreeMap<PathBuf, DestructivePrelude>,
) -> Result<ClosedCSourceSet, ClosedSourcePreparationError> {
    let mut snapshots = Vec::with_capacity(selected.len());
    for (index, absolute) in selected.iter().enumerate() {
        let source = if let Some(source) = overrides
            .get(absolute)
            .and_then(|prelude| prelude.source.clone())
        {
            source
        } else {
            let bytes = std::fs::read(absolute).map_err(|error| ClosedSourcePreparationError {
                path: absolute.clone(),
                message: format!(
                    "A C source could not be read for the closed-world proof: {error}"
                ),
                help: "Make every selected C/header file readable and retry.",
            })?;
            String::from_utf8(bytes).map_err(|error| ClosedSourcePreparationError {
                path: absolute.clone(),
                message: format!("A C source is not valid UTF-8: {error}"),
                help: "Convert the source to UTF-8 before project-wide analysis.",
            })?
        };
        let relative =
            absolute
                .strip_prefix(cwd)
                .map_err(|error| ClosedSourcePreparationError {
                    path: absolute.clone(),
                    message: format!("A selected C source is outside the project root: {error}"),
                    help: "Run project-wide analysis from one complete project root.",
                })?;
        let relative = Utf8PathBuf::from_path_buf(relative.to_path_buf()).map_err(|path| {
            ClosedSourcePreparationError {
                path: absolute.clone(),
                message: format!("A selected C path is not valid UTF-8: {}", path.display()),
                help: "Rename the path before project-wide analysis.",
            }
        })?;
        let file_id = u32::try_from(index).map_err(|error| ClosedSourcePreparationError {
            path: absolute.clone(),
            message: format!("The closed source set is too large to index safely: {error}"),
            help: "Reduce the project to fewer than 2^32 C/header files.",
        })?;
        snapshots.push(
            SourceSnapshot::new(FileId::new(file_id), relative, Arc::<str>::from(source)).map_err(
                |error| ClosedSourcePreparationError {
                    path: absolute.clone(),
                    message: format!("A C source snapshot was rejected: {error}"),
                    help: "Use project-relative UTF-8 paths and sources smaller than 4 GiB.",
                },
            )?,
        );
    }
    ClosedCSourceSet::from_complete_discovery(snapshots).map_err(|error| {
        ClosedSourcePreparationError {
            path: selected
                .iter()
                .next()
                .cloned()
                .unwrap_or_else(|| cwd.to_path_buf()),
            message: format!("The closed C source set was rejected: {error}"),
            help: "Run from one complete project root with unique C/header paths.",
        }
    })
}

fn apply_static_removal_plan(
    preludes: &mut BTreeMap<PathBuf, DestructivePrelude>,
    plan: StaticRemovalPlan,
    cwd: &Path,
    read_preconditions: &[ReadPrecondition],
) {
    for diagnostic in plan.diagnostics {
        let absolute = cwd.join(diagnostic.path.as_std_path());
        let prelude = prelude_entry(preludes, &absolute);
        prelude.diagnostics.push(diagnostic);
    }
    for file_plan in plan.files {
        let absolute = cwd.join(file_plan.path.as_std_path());
        let prelude = prelude_entry(preludes, &absolute);
        if let Err(error) = load_destructive_source(prelude, &absolute) {
            prelude.diagnostics.push(project_diagnostic(
                file_plan.path,
                "UNSAFE_STATIC_SOURCE_UNREADABLE",
                &format!("The source could not be read for destructive planning: {error}"),
                "Make the file readable and retry; no function was removed.",
            ));
            continue;
        }
        let source = prelude
            .source
            .as_deref()
            .expect("loaded destructive source");
        if blake3::hash(source.as_bytes()).to_hex().to_string() != file_plan.original_hash {
            prelude.diagnostics.push(project_diagnostic(
                file_plan.path,
                "UNSAFE_STATIC_SNAPSHOT_CHANGED",
                "The source changed after destructive planning; no function was removed.",
                "Retry the command against an unchanged project.",
            ));
            continue;
        }
        match apply_source_edits(source, &file_plan.edits) {
            Ok(source) => {
                prelude.source = Some(source);
                prelude.fixes.extend(file_plan.fixes);
                prelude
                    .read_preconditions
                    .extend_from_slice(read_preconditions);
            }
            Err(error) => prelude.diagnostics.push(project_diagnostic(
                file_plan.path,
                "UNSAFE_STATIC_EDIT_REJECTED",
                &format!("The destructive edit set was rejected: {error}"),
                "Review the candidate function manually; no deletion was applied.",
            )),
        }
    }
}

fn apply_orphan_prototype_plan(
    preludes: &mut BTreeMap<PathBuf, DestructivePrelude>,
    plan: OrphanPrototypePlan,
    cwd: &Path,
    selected: &BTreeSet<PathBuf>,
    apply_removals: bool,
    read_preconditions: &[ReadPrecondition],
) {
    let mut attached_diagnostic = false;
    let proof_incomplete = apply_removals
        && plan
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.rule_id == "ORPHAN_PROTOTYPE_CLOSED_SET_INVALID");
    for diagnostic in plan.diagnostics {
        if !apply_removals && diagnostic.rule_id == "ORPHAN_PROTOTYPE_CLOSED_SET_INVALID" {
            continue;
        }
        let absolute = cwd.join(diagnostic.path.as_std_path());
        if selected.contains(&absolute) {
            prelude_entry(preludes, &absolute)
                .diagnostics
                .push(diagnostic);
            attached_diagnostic = true;
        }
    }
    if proof_incomplete && !attached_diagnostic {
        if let Some(first) = selected.iter().next() {
            attach_destructive_diagnostic(
                preludes,
                first,
                cwd,
                "ORPHAN_PROTOTYPE_CLOSED_SET_INVALID",
                "Prototype implementation analysis was inconclusive because another project C/header file could not be parsed losslessly.",
                "Fix every project syntax/recovery issue and rerun preflight.",
            );
        }
    }
    if !apply_removals {
        return;
    }
    for file_plan in plan.files {
        let absolute = cwd.join(file_plan.path.as_std_path());
        if !selected.contains(&absolute) {
            continue;
        }
        let prelude = prelude_entry(preludes, &absolute);
        if let Err(error) = load_destructive_source(prelude, &absolute) {
            prelude.diagnostics.push(project_diagnostic(
                file_plan.path,
                "UNSAFE_ORPHAN_PROTOTYPE_SOURCE_UNREADABLE",
                &format!("The header could not be read for destructive planning: {error}"),
                "Make the file readable and retry; no prototype was removed.",
            ));
            continue;
        }
        let source = prelude
            .source
            .as_deref()
            .expect("loaded destructive source");
        if blake3::hash(source.as_bytes()).to_hex().to_string() != file_plan.original_hash {
            prelude.diagnostics.push(project_diagnostic(
                file_plan.path,
                "UNSAFE_ORPHAN_PROTOTYPE_SNAPSHOT_CHANGED",
                "The header changed after orphan-prototype planning; no prototype was removed.",
                "Retry the command against an unchanged project.",
            ));
            continue;
        }
        match apply_source_edits(source, &file_plan.edits) {
            Ok(source) => {
                prelude.source = Some(source);
                prelude.fixes.extend(file_plan.fixes);
                prelude
                    .read_preconditions
                    .extend_from_slice(read_preconditions);
            }
            Err(error) => prelude.diagnostics.push(project_diagnostic(
                file_plan.path,
                "UNSAFE_ORPHAN_PROTOTYPE_EDIT_REJECTED",
                &format!("The prototype deletion set was rejected: {error}"),
                "Review the prototype manually; no deletion was applied.",
            )),
        }
    }
}

fn prelude_entry<'a>(
    preludes: &'a mut BTreeMap<PathBuf, DestructivePrelude>,
    absolute: &Path,
) -> &'a mut DestructivePrelude {
    preludes
        .entry(absolute.to_path_buf())
        .or_insert_with(|| DestructivePrelude {
            source: None,
            original_blake3: None,
            fixes: Vec::new(),
            diagnostics: Vec::new(),
            read_preconditions: Vec::new(),
        })
}

fn load_destructive_source(
    prelude: &mut DestructivePrelude,
    absolute: &Path,
) -> Result<(), String> {
    if prelude.source.is_some() {
        return Ok(());
    }
    let bytes = std::fs::read(absolute).map_err(|error| error.to_string())?;
    let source = String::from_utf8(bytes.clone()).map_err(|error| error.to_string())?;
    prelude.original_blake3 = Some(*blake3::hash(&bytes).as_bytes());
    prelude.source = Some(source);
    Ok(())
}

fn snapshot_preconditions(snapshot: &ProjectSnapshot) -> Vec<ReadPrecondition> {
    snapshot
        .files
        .iter()
        .map(|(path, digest)| ReadPrecondition::Matches {
            path: path.clone(),
            blake3: *digest,
        })
        .collect()
}

fn attach_destructive_diagnostic(
    preludes: &mut BTreeMap<PathBuf, DestructivePrelude>,
    absolute: &Path,
    cwd: &Path,
    rule_id: &str,
    message: &str,
    help: &str,
) {
    let path = report_path(absolute, cwd)
        .unwrap_or_else(|_| Utf8PathBuf::from(absolute.to_string_lossy().as_ref()));
    prelude_entry(preludes, absolute)
        .diagnostics
        .push(project_diagnostic(path, rule_id, message, help));
}

#[allow(clippy::too_many_arguments)]
fn process_file(
    file: &DiscoveredFile,
    options: &FixOptions,
    clock: &RunClock,
    oracle: Option<&OracleContext>,
    guard_approvals: &BTreeMap<PathBuf, GuardApproval>,
    guard_insertions: &BTreeMap<PathBuf, GuardInsertionApproval>,
    guard_failure: Option<&str>,
    destructive_prelude: Option<&DestructivePrelude>,
    policy_diagnostics: Option<&[Diagnostic]>,
) -> FileWork {
    let relative = report_path(&file.path, &options.cwd)
        .unwrap_or_else(|_| Utf8PathBuf::from(file.path.to_string_lossy().as_ref()));
    let original = match std::fs::read(&file.path) {
        Ok(bytes) => bytes,
        Err(error) => {
            return failed_file(file, relative, format!("Could not read this file: {error}"));
        }
    };
    if original.contains(&0) {
        return failed_file(
            file,
            relative,
            "Refused a file containing NUL bytes.".to_owned(),
        );
    }
    let source = match String::from_utf8(original.clone()) {
        Ok(source) => source,
        Err(error) => {
            return failed_file(
                file,
                relative,
                format!("The file is not valid UTF-8: {error}"),
            );
        }
    };
    match file.kind {
        ProjectFileKind::CSource | ProjectFileKind::CHeader => {
            let Some(oracle) = oracle else {
                return failed_source(
                    file,
                    relative,
                    original,
                    source,
                    "The required Norminette context was not initialized for this C file."
                        .to_owned(),
                );
            };
            process_c(
                file,
                relative,
                original,
                source,
                options,
                clock,
                oracle,
                guard_approvals,
                guard_insertions,
                guard_failure,
                destructive_prelude,
                policy_diagnostics,
            )
        }
        ProjectFileKind::Makefile => {
            process_makefile(file, relative, &original, source, options, clock)
        }
        ProjectFileKind::Markdown => process_markdown(file, relative, original, source, options),
    }
}

pub(super) fn failed_file(file: &DiscoveredFile, path: Utf8PathBuf, failure: String) -> FileWork {
    FileWork {
        absolute_path: file.path.clone(),
        report: FileReport {
            path,
            changed: false,
            written: false,
            backup: None,
            failure: Some(failure),
            fixes: Vec::new(),
            before: Vec::new(),
            after: Vec::new(),
            original: None,
            fixed: None,
        },
        plan: None,
        read_preconditions: Vec::new(),
    }
}

/// Bounded number of official-checker rounds for one file.
///
/// One round converges the native actions; a second exposes the rules that only
/// become visible once layout is correct. A third has never been needed in
/// practice and exists so a pathological file cannot spend checker calls
/// forever.
const MAX_ORACLE_ROUNDS: usize = 3;

/// Converts one official report into the diagnostics the action crate consumes.
fn reported_diagnostics(report: &normfix_oracle::NorminetteReport) -> Vec<ReportedDiagnostic> {
    report
        .diagnostics
        .iter()
        .map(|item| {
            ReportedDiagnostic::new(
                item.rule_id.clone(),
                item.line,
                item.column,
                item.message.clone(),
            )
        })
        .collect()
}

// Keeping the C stages in one straight-line function makes the shadow-buffer
// proof boundary and each official-oracle checkpoint visible during review.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn process_c(
    file: &DiscoveredFile,
    path: Utf8PathBuf,
    original_bytes: Vec<u8>,
    original: String,
    options: &FixOptions,
    clock: &RunClock,
    oracle: &OracleContext,
    guard_approvals: &BTreeMap<PathBuf, GuardApproval>,
    guard_insertions: &BTreeMap<PathBuf, GuardInsertionApproval>,
    guard_failure: Option<&str>,
    destructive_prelude: Option<&DestructivePrelude>,
    policy_diagnostics: Option<&[Diagnostic]>,
) -> FileWork {
    let norminette_version = oracle.oracle.fingerprint().version.as_str();
    let before_report = match oracle.lint(&file.path, &original) {
        Ok(report) => report,
        Err(error) => {
            return failed_source(
                file,
                path,
                original_bytes,
                original,
                format!("Official Norminette could not inspect this file: {error}"),
            );
        }
    };
    let mut before_official = before_report.diagnostics.clone();
    let before_semantic_advisories = explain_constant_array_false_positives(
        &path,
        &original,
        &mut before_official,
        norminette_version,
    );
    let before = official_diagnostics(&path, &original, &before_official, norminette_version);
    if options.lint_only {
        let remaining_official = before_official;
        let semantic_advisories = before_semantic_advisories;
        let mut after = match analyze_c(path.as_path(), &original, options.max_columns) {
            Ok(diagnostics) => diagnostics,
            Err(_) => parser_diagnostics(&path, &original),
        };
        merge_official_diagnostics(
            &mut after,
            official_diagnostics(&path, &original, &remaining_official, norminette_version),
            options.preflight,
            norminette_version,
        );
        after.extend(semantic_advisories);
        after.extend(policy_diagnostics.unwrap_or_default().iter().cloned());
        after.extend(untested_norminette_diagnostic(oracle, file, &path));
        if options.emit_budget {
            after.extend(budget_diagnostics(&path, &original));
        }
        if file.kind == ProjectFileKind::CSource {
            after.extend(run_compiler_preflight(
                oracle, options, file, &path, &original, &original,
            ));
        }
        after.sort();
        after.dedup();
        return FileWork {
            absolute_path: file.path.clone(),
            report: FileReport {
                path,
                changed: false,
                written: false,
                backup: None,
                failure: None,
                fixes: Vec::new(),
                before,
                after,
                original: Some(Arc::from(original.clone())),
                fixed: Some(Arc::from(original)),
            },
            plan: None,
            read_preconditions: Vec::new(),
        };
    }
    let confirmed_prelude =
        destructive_prelude.filter(|prelude| prelude.confirmed_source(&original_bytes).is_some());
    let mut current = confirmed_prelude
        .and_then(|prelude| prelude.confirmed_source(&original_bytes))
        .map_or_else(|| original.clone(), str::to_owned);
    let pre_action_source = current.clone();
    let mut fixes = confirmed_prelude.map_or_else(Vec::new, |prelude| prelude.fixes.clone());
    let mut local_diagnostics =
        destructive_prelude.map_or_else(Vec::new, |prelude| prelude.diagnostics.clone());
    let mut read_preconditions =
        confirmed_prelude.map_or_else(Vec::new, |prelude| prelude.read_preconditions.clone());
    if destructive_prelude.is_some_and(|prelude| {
        prelude.source.is_some() && prelude.confirmed_source(&original_bytes).is_none()
    }) {
        local_diagnostics.push(project_diagnostic(
            path.clone(),
            "DESTRUCTIVE_PRELUDE_SNAPSHOT_CHANGED",
            "The file changed after destructive planning; the planned deletion was discarded.",
            "Retry against an unchanged project; no stale source bytes were used.",
        ));
    }
    local_diagnostics.extend(policy_diagnostics.unwrap_or_default().iter().cloned());

    let approval_key = file
        .path
        .canonicalize()
        .unwrap_or_else(|_| file.path.clone());
    let guard_changed = guard_approvals
        .get(&approval_key)
        .and_then(|approval| apply_guard_approval(&current, approval))
        .is_some_and(|updated| {
            current = updated;
            fixes.push(FixRecord {
                rule_id: "HEADER_GUARD_RENAME".to_owned(),
                description:
                    "renamed both canonical inclusion-guard tokens after a closed-project proof"
                        .to_owned(),
                line: None,
                count: 2,
            });
            true
        });
    let guard_inserted = guard_insertions
        .get(&approval_key)
        .and_then(|approval| apply_guard_insertion(&current, approval))
        .is_some_and(|updated| {
            current = updated;
            fixes.push(FixRecord {
                rule_id: "HEADER_GUARD_INSERT".to_owned(),
                description:
                    "added one filename-derived whole-file inclusion guard after a closed-project proof"
                        .to_owned(),
                line: None,
                count: 3,
            });
            true
        });
    if guard_changed {
        if let Some(approval) = guard_approvals.get(&approval_key) {
            read_preconditions.extend(snapshot_preconditions(&approval.snapshot));
        }
    }
    if guard_inserted {
        if let Some(approval) = guard_insertions.get(&approval_key) {
            read_preconditions.extend(snapshot_preconditions(&approval.snapshot));
        }
    }

    let filename = file
        .path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    let ensured = ensure_c_header(&current, filename, options.identity.as_ref(), clock);
    let header_inserted = ensured.inserted;
    append_header_fixes(&mut fixes, &ensured.fixes, &current);
    append_header_issues(&mut local_diagnostics, &path, &ensured.issues);
    current = ensured.output;

    let baseline_report = match oracle.lint(&file.path, &current) {
        Ok(report) => report,
        Err(error) => {
            return failed_source_with_report(
                file,
                path,
                original_bytes,
                original,
                current,
                fixes,
                before,
                local_diagnostics,
                format!("Official Norminette could not validate the header stage: {error}"),
            );
        }
    };
    let action_fix_start = fixes.len();
    let baseline = current.clone();
    let mut reported = reported_diagnostics(&baseline_report);
    let action_options = CActionOptions {
        max_columns: options.max_columns,
        max_passes: options.max_passes,
        remove_invalid_comments: options.remove_invalid_comments,
        format_proven_declarations: true,
        compact_null_checks: options.compact_null_checks,
        reorder_includes: options.reorder_includes,
    };
    let mut action_changed = false;
    // Several actions are driven by official diagnostics, and the oracle only
    // ever sees the bytes in front of it. Correcting indentation can expose an
    // alignment rule that was masked before, so a single pass converges the
    // native actions but not the file. Re-lint and run again until nothing
    // changes, bounded because a round that never settles is a defect rather
    // than a reason to keep spending official checker calls.
    for round in 0..MAX_ORACLE_ROUNDS {
        let changed_this_round;
        match apply_c_actions(path.as_path(), &current, &reported, &action_options) {
            Ok(result) => {
                changed_this_round = result.source != current;
                current = result.source;
                fixes.extend(result.fixes.into_iter().map(|item| FixRecord {
                    rule_id: item.rule_id,
                    description: item.description,
                    line: item.line,
                    count: 1,
                }));
            }
            Err(CActionError::UnsafeSyntax) => {
                local_diagnostics.extend(parser_diagnostics(&path, &current));
                break;
            }
            Err(error) => {
                local_diagnostics.push(point_diagnostic(
                    &path,
                    "FIX_PROOF_REJECTED",
                    Severity::Warning,
                    format!("A native edit batch was rejected: {error}"),
                    DiagnosticSource::NativeNorm41,
                    Some(
                        "Review the reported source issue; the rejected batch was not written."
                            .to_owned(),
                    ),
                ));
                break;
            }
        }
        action_changed |= changed_this_round;
        if !changed_this_round || round + 1 == MAX_ORACLE_ROUNDS {
            break;
        }
        // A failure here is reported by the final validation below.
        let Ok(report) = oracle.lint(&file.path, &current) else {
            break;
        };
        reported = reported_diagnostics(&report);
    }

    let should_update_header = !header_inserted
        && (guard_changed
            || guard_inserted
            || action_changed
            || !c_header_filename_matches(&current, filename));
    if should_update_header {
        let updated = update_c_header(&current, filename, options.identity.as_ref(), clock);
        append_header_fixes(&mut fixes, &updated.fixes, &current);
        append_header_issues(&mut local_diagnostics, &path, &updated.issues);
        current = updated.output;
    }

    let mut final_report = match oracle.lint(&file.path, &current) {
        Ok(report) => report,
        Err(error) => {
            return failed_source_with_report(
                file,
                path,
                original_bytes,
                original,
                current,
                fixes,
                before,
                local_diagnostics,
                format!("Official Norminette could not validate the final shadow buffer: {error}"),
            );
        }
    };
    if action_changed
        && introduces_diagnostics(&baseline_report.diagnostics, &final_report.diagnostics)
    {
        current = baseline;
        fixes.truncate(action_fix_start);
        local_diagnostics.push(point_diagnostic(
            &path,
            "FIX_REJECTED_NEW_DIAGNOSTIC",
            Severity::Info,
            "An optional formatting batch was skipped because it introduced a new Norminette diagnostic."
                .to_owned(),
            DiagnosticSource::NorminetteCompat(
                norminette_version.to_owned(),
            ),
            Some(
                "The complete batch was reverted; the original source remains authoritative."
                    .to_owned(),
            ),
        ));
        final_report = match oracle.lint(&file.path, &current) {
            Ok(report) => report,
            Err(error) => {
                return failed_source_with_report(
                    file,
                    path,
                    original_bytes,
                    original,
                    current,
                    fixes,
                    before,
                    local_diagnostics,
                    format!("Official Norminette could not validate the reverted buffer: {error}"),
                );
            }
        };
    }

    let mut remaining_official = final_report.diagnostics;
    let semantic_advisories = explain_constant_array_false_positives(
        &path,
        &current,
        &mut remaining_official,
        norminette_version,
    );
    local_diagnostics.extend(untested_norminette_diagnostic(oracle, file, &path));
    if file.kind == ProjectFileKind::CSource {
        local_diagnostics.extend(run_compiler_preflight(
            oracle, options, file, &path, &original, &current,
        ));
    }
    let mut after = match analyze_c(path.as_path(), &current, options.max_columns) {
        Ok(diagnostics) => diagnostics,
        Err(_) => parser_diagnostics(&path, &current),
    };
    merge_official_diagnostics(
        &mut after,
        official_diagnostics(&path, &current, &remaining_official, norminette_version),
        options.preflight,
        norminette_version,
    );
    after.extend(semantic_advisories);
    remap_orphan_prototype_diagnostics(&mut local_diagnostics, &pre_action_source, &current);
    after.extend(local_diagnostics);
    if let Some(reason) = guard_failure {
        if file.kind == ProjectFileKind::CHeader
            && remaining_official
                .iter()
                .any(|diagnostic| diagnostic.rule_id.starts_with("HEADER_PROT"))
        {
            after.push(point_diagnostic(
                &path,
                "HEADER_GUARD_PROOF_UNAVAILABLE",
                Severity::Warning,
                format!("The inclusion guard was left unchanged: {reason}"),
                DiagnosticSource::Project,
                Some(
                    "Use a canonical whole-file guard and remove project-wide macro ambiguity."
                        .to_owned(),
                ),
            ));
        }
    }
    after.sort();
    after.dedup();
    let changed = current.as_bytes() != original_bytes;
    let original_arc: Arc<str> = Arc::from(original.clone());
    let fixed_arc: Arc<str> = Arc::from(current.clone());
    let plan = changed.then(|| PlannedFile {
        path: file.path.clone(),
        original: original_bytes.clone(),
        replacement: current.as_bytes().to_vec(),
        fixes: fixes.clone(),
    });
    FileWork {
        absolute_path: file.path.clone(),
        report: FileReport {
            path,
            changed,
            written: false,
            backup: None,
            failure: None,
            fixes,
            before,
            after,
            original: Some(original_arc),
            fixed: Some(fixed_arc),
        },
        plan,
        read_preconditions,
    }
}

fn remap_orphan_prototype_diagnostics(diagnostics: &mut [Diagnostic], before: &str, after: &str) {
    if before == after
        || !diagnostics.iter().any(|diagnostic| {
            matches!(
                diagnostic.rule_id.as_str(),
                "HEADER_PROTOTYPE_IMPLEMENTATION_MISSING"
                    | "HEADER_PROTOTYPE_IMPLEMENTATION_EMPTY"
                    | "UNSAFE_ORPHAN_PROTOTYPE_PROOF_BLOCKED"
            )
        })
    {
        return;
    }
    let parsed = CParser::new().and_then(|mut parser| {
        Ok((
            parser.parse(before)?.facts().clone(),
            parser.parse(after)?.facts().clone(),
        ))
    });
    let Ok((before_facts, after_facts)) = parsed else {
        for diagnostic in diagnostics.iter_mut().filter(|diagnostic| {
            matches!(
                diagnostic.rule_id.as_str(),
                "HEADER_PROTOTYPE_IMPLEMENTATION_MISSING"
                    | "HEADER_PROTOTYPE_IMPLEMENTATION_EMPTY"
                    | "UNSAFE_ORPHAN_PROTOTYPE_PROOF_BLOCKED"
            )
        }) {
            diagnostic.range = TextRange::empty(TextSize::new(0));
        }
        return;
    };
    for diagnostic in diagnostics.iter_mut().filter(|diagnostic| {
        matches!(
            diagnostic.rule_id.as_str(),
            "HEADER_PROTOTYPE_IMPLEMENTATION_MISSING"
                | "HEADER_PROTOTYPE_IMPLEMENTATION_EMPTY"
                | "UNSAFE_ORPHAN_PROTOTYPE_PROOF_BLOCKED"
        )
    }) {
        let before_prototypes = before_facts
            .functions
            .iter()
            .filter(|function| function.kind == CFunctionKind::Prototype)
            .collect::<Vec<_>>();
        let Some((name, ordinal)) = before_prototypes
            .iter()
            .find(|function| function.name_range == diagnostic.range)
            .map(|target| {
                let ordinal = before_prototypes
                    .iter()
                    .filter(|function| function.name == target.name)
                    .position(|function| function.name_range == target.name_range)
                    .unwrap_or(0);
                (target.name.as_str(), ordinal)
            })
        else {
            diagnostic.range = TextRange::empty(TextSize::new(0));
            continue;
        };
        diagnostic.range = after_facts
            .functions
            .iter()
            .filter(|function| function.kind == CFunctionKind::Prototype && function.name == name)
            .nth(ordinal)
            .map_or_else(
                || TextRange::empty(TextSize::new(0)),
                |function| function.name_range,
            );
    }
}

pub(super) fn failed_source(
    file: &DiscoveredFile,
    path: Utf8PathBuf,
    original_bytes: Vec<u8>,
    original: String,
    failure: String,
) -> FileWork {
    failed_source_with_report(
        file,
        path,
        original_bytes,
        original.clone(),
        original,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        failure,
    )
}

#[allow(clippy::too_many_arguments)]
fn failed_source_with_report(
    file: &DiscoveredFile,
    path: Utf8PathBuf,
    _original_bytes: Vec<u8>,
    original: String,
    proposed: String,
    fix_records: Vec<FixRecord>,
    before: Vec<Diagnostic>,
    after: Vec<Diagnostic>,
    failure: String,
) -> FileWork {
    let changed = original != proposed;
    FileWork {
        absolute_path: file.path.clone(),
        report: FileReport {
            path,
            changed,
            written: false,
            backup: None,
            failure: Some(failure),
            fixes: fix_records,
            before,
            after,
            original: Some(Arc::from(original)),
            fixed: Some(Arc::from(proposed)),
        },
        // Operational validation failures never authorize a partial write.
        plan: None,
        read_preconditions: Vec::new(),
    }
}

fn budget_diagnostics(path: &Utf8PathBuf, source: &str) -> Vec<Diagnostic> {
    analyze_budget(path.as_path(), source).map_or_else(
        |_| Vec::new(),
        |budgets| {
            budgets
                .into_iter()
                .map(|budget| Diagnostic {
                    rule_id: "NORM_BUDGET".to_owned(),
                    path: path.clone(),
                    range: line_point_range(source, budget.line),
                    severity: Severity::Info,
                    message: format!(
                        "{}(): lines {}/{} ({} left), variables {}/{} ({} left), parameters {}/{} ({} left).",
                        budget.function,
                        budget.lines,
                        budget.line_limit,
                        budget.line_limit.saturating_sub(budget.lines),
                        budget.variables,
                        budget.variable_limit,
                        budget.variable_limit.saturating_sub(budget.variables),
                        budget.parameters,
                        budget.parameter_limit,
                        budget.parameter_limit.saturating_sub(budget.parameters),
                    ),
                    source: DiagnosticSource::NativeNorm41,
                    notes: Vec::new(),
                    help: Some(
                        "Keep headroom for defense-day changes; limits already exceeded are also reported as warnings."
                            .to_owned(),
                    ),
                })
                .collect()
        },
    )
}

/// Selects the plans to commit, or `None` when interactive approval expired.
///
/// An expired approval marks the affected reports and commits nothing, because
/// the previewed bytes are no longer the bytes on disk.
fn plans_to_commit(
    work: &mut [FileWork],
    options: &FixOptions,
    dependent_destructive_bundle: &BTreeSet<PathBuf>,
) -> Option<Vec<PlannedFile>> {
    let plans = if let Some(approvals) = &options.write_approvals {
        let valid = approvals.iter().all(|(path, approval)| {
            work.iter()
                .find(|item| absolute_lexical(&item.absolute_path) == absolute_lexical(path))
                .and_then(|item| item.plan.as_ref())
                .is_some_and(|plan| approval.permits(plan))
        });
        if !valid {
            let message =
                "Interactive approval expired because the original or proposed bytes changed after preview; no files were written."
                    .to_owned();
            for item in work
                .iter_mut()
                .filter(|item| approvals.contains_key(&absolute_lexical(&item.absolute_path)))
            {
                item.report.failure = Some(message.clone());
            }
            return None;
        }
        work.iter()
            .filter(|item| approvals.contains_key(&absolute_lexical(&item.absolute_path)))
            .filter_map(|item| item.plan.clone())
            .collect::<Vec<_>>()
    } else {
        work.iter().filter_map(|item| item.plan.clone()).collect()
    };

    if dependent_destructive_bundle_is_partial(&plans, dependent_destructive_bundle) {
        let message = "The dependent static-function and orphan-prototype removal bundle became incomplete after validation or approval filtering; no files were written.";
        for item in work.iter_mut().filter(|item| {
            dependent_destructive_bundle.contains(&absolute_lexical(&item.absolute_path))
        }) {
            if item.report.failure.is_none() {
                item.report.failure = Some(message.to_owned());
            }
        }
        return None;
    }
    Some(plans)
}

fn dependent_destructive_bundle_is_partial(
    plans: &[PlannedFile],
    dependent_destructive_bundle: &BTreeSet<PathBuf>,
) -> bool {
    let selected_bundle_paths = plans
        .iter()
        .filter(|plan| {
            plan.fixes.iter().any(|fix| {
                matches!(
                    fix.rule_id.as_str(),
                    "UNSAFE_REMOVE_UNUSED_STATIC" | "UNSAFE_REMOVE_ORPHAN_PROTOTYPE"
                )
            })
        })
        .map(|plan| absolute_lexical(&plan.path))
        .filter(|path| dependent_destructive_bundle.contains(path))
        .collect::<BTreeSet<_>>();
    !selected_bundle_paths.is_empty() && selected_bundle_paths != *dependent_destructive_bundle
}

/// Returns the exact files whose static and orphan removals were planned from
/// one transformed closed-world snapshot. Neither edit family may survive
/// without every other replacement that supplied that snapshot.
fn dependent_destructive_bundle_paths(
    preludes: &BTreeMap<PathBuf, DestructivePrelude>,
) -> BTreeSet<PathBuf> {
    let mut has_static_removal = false;
    let mut has_orphan_removal = false;
    let mut paths = BTreeSet::new();
    for (path, prelude) in preludes {
        let participates = prelude.fixes.iter().any(|fix| {
            if fix.rule_id == "UNSAFE_REMOVE_UNUSED_STATIC" {
                has_static_removal = true;
                true
            } else if fix.rule_id == "UNSAFE_REMOVE_ORPHAN_PROTOTYPE" {
                has_orphan_removal = true;
                true
            } else {
                false
            }
        });
        if participates {
            paths.insert(absolute_lexical(path));
        }
    }
    if has_static_removal && has_orphan_removal {
        paths
    } else {
        BTreeSet::new()
    }
}

/// Records one operational failure on every selected file report.
fn fail_selected(
    work: &mut [FileWork],
    selected_paths: &BTreeSet<PathBuf>,
    message: &str,
    clear_written: bool,
) {
    for item in work
        .iter_mut()
        .filter(|item| selected_paths.contains(&absolute_lexical(&item.absolute_path)))
    {
        item.report.failure = Some(message.to_owned());
        if clear_written {
            item.report.written = false;
        }
    }
}

fn commit_work(
    work: &mut [FileWork],
    options: &FixOptions,
    dependent_destructive_bundle: &BTreeSet<PathBuf>,
) -> bool {
    let Some(plans) = plans_to_commit(work, options, dependent_destructive_bundle) else {
        return false;
    };
    if plans.is_empty() {
        return true;
    }
    let selected_paths = plans
        .iter()
        .map(|plan| absolute_lexical(&plan.path))
        .collect::<BTreeSet<_>>();
    let read_preconditions = work
        .iter()
        .filter(|item| selected_paths.contains(&absolute_lexical(&item.absolute_path)))
        .flat_map(|item| item.read_preconditions.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut transaction_paths = plans
        .iter()
        .map(|plan| absolute_lexical(&plan.path))
        .collect::<Vec<_>>();
    transaction_paths.extend(
        read_preconditions
            .iter()
            .map(|precondition| match precondition {
                ReadPrecondition::Matches { path, .. } | ReadPrecondition::Absent { path } => {
                    absolute_lexical(path)
                }
                ReadPrecondition::ProjectSources { root, .. } => absolute_lexical(root),
            }),
    );
    let cwd = absolute_lexical(&options.cwd);
    let project_root = if transaction_paths.iter().all(|path| path.starts_with(&cwd)) {
        cwd
    } else {
        transaction_root(transaction_paths.iter().map(PathBuf::as_path), &options.cwd)
    };
    let requires_recovery = plans.iter().any(|plan| {
        plan.fixes.iter().any(|fix| {
            matches!(
                fix.rule_id.as_str(),
                "UNSAFE_REMOVE_UNUSED_STATIC"
                    | "REMOVE_INVALID_COMMENT"
                    | "MAKEFILE_REMOVE_MISSING_SOURCE"
                    | "MAKEFILE_REMOVE_EMPTY_SOURCE"
                    | "UNSAFE_REMOVE_ORPHAN_PROTOTYPE"
            )
        })
    });
    let backup_root = match &options.backup {
        BackupPolicy::Automatic => automatic_backup_root(),
        BackupPolicy::Directory(path) => Some(path.clone()),
        BackupPolicy::Disabled if requires_recovery => automatic_backup_root(),
        BackupPolicy::Disabled => None,
    };
    let backup_required = matches!(&options.backup, BackupPolicy::Automatic) || requires_recovery;
    if backup_required && backup_root.is_none() {
        let message = "The write was refused because no external backup directory is available. Configure HOME, XDG_DATA_HOME, or --backup-dir; use --no-backup only for ordinary non-destructive formatting.";
        fail_selected(work, &selected_paths, message, false);
        return false;
    }
    let transaction_options = TransactionOptions {
        project_root,
        run_id: run_id(),
        backup_root,
    };
    match commit_files_guarded(plans, &transaction_options, &read_preconditions) {
        Ok(committed) => {
            let committed = committed
                .files
                .into_iter()
                .map(|file| (file.path, file.backup))
                .collect::<BTreeMap<_, _>>();
            for item in work {
                if let Some(backup) = committed.get(&item.absolute_path) {
                    item.report.written = true;
                    item.report.backup = backup
                        .as_ref()
                        .and_then(|path| Utf8PathBuf::from_path_buf(path.clone()).ok());
                }
            }
            true
        }
        Err(error) => {
            let message = format!("The atomic write transaction failed: {error}");
            fail_selected(work, &selected_paths, &message, true);
            false
        }
    }
}

fn apply_guard_approval(source: &str, approval: &GuardApproval) -> Option<String> {
    if !guard_approval_is_current(approval)
        || *blake3::hash(source.as_bytes()).as_bytes() != approval.rename.header_digest
    {
        return None;
    }
    let mut output = source.to_owned();
    for (range, current) in [
        (
            approval.rename.define_range.clone(),
            approval.rename.define_current.as_str(),
        ),
        (
            approval.rename.ifndef_range.clone(),
            approval.rename.current.as_str(),
        ),
    ] {
        if output.get(range.clone())? != current {
            return None;
        }
        output.replace_range(range, &approval.rename.expected);
    }
    Some(output)
}

fn apply_guard_insertion(source: &str, approval: &GuardInsertionApproval) -> Option<String> {
    if !guard_insertion_approval_is_current(approval)
        || *blake3::hash(source.as_bytes()).as_bytes() != approval.insertion.header_digest
    {
        return None;
    }
    let guard = &approval.insertion.expected;
    let mut body_start = normfix_header::c_header_span(source).map_or(0, |range| range.end);
    while source
        .as_bytes()
        .get(body_start)
        .is_some_and(|byte| matches!(byte, b'\r' | b'\n'))
    {
        body_start += 1;
    }
    let body = source.get(body_start..)?;
    let mut output = String::with_capacity(source.len() + guard.len() * 2 + 40);
    output.push_str(source.get(..body_start)?);
    output.push_str("#ifndef ");
    output.push_str(guard);
    output.push_str("\n# define ");
    output.push_str(guard);
    output.push_str("\n\n");
    output.push_str(body);
    if !body.is_empty() && !body.ends_with('\n') {
        output.push('\n');
    }
    if !body.is_empty() && !output.ends_with("\n\n") {
        output.push('\n');
    }
    output.push_str("#endif\n");
    Some(output)
}

pub(super) fn append_header_fixes(
    output: &mut Vec<FixRecord>,
    fixes: &[normfix_header::Fix],
    source: &str,
) {
    output.extend(fixes.iter().map(|item| FixRecord {
        rule_id: item.code.to_owned(),
        description: item.description.clone(),
        line: line_for_offset(source, item.range.start),
        count: 1,
    }));
}

pub(super) fn append_header_issues(
    output: &mut Vec<Diagnostic>,
    path: &Utf8PathBuf,
    issues: &[normfix_header::Issue],
) {
    output.extend(issues.iter().map(|issue| Diagnostic {
        rule_id: issue.code.to_owned(),
        path: path.clone(),
        range: text_range(issue.range),
        severity: Severity::Warning,
        message: issue.message.clone(),
        source: DiagnosticSource::NativeNorm41,
        notes: Vec::new(),
        help: Some(issue.suggestion.clone()),
    }));
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{BTreeMap, BTreeSet},
        fs,
    };

    use tempfile::TempDir;

    use normfix_core::FixRecord;

    use super::{
        PlannedFile, dependent_destructive_bundle_is_partial, load_destructive_source,
        prelude_entry,
    };

    fn destructive_test_plan(path: &std::path::Path, rule_id: &str) -> PlannedFile {
        PlannedFile {
            path: path.to_path_buf(),
            original: b"old\n".to_vec(),
            replacement: b"new\n".to_vec(),
            fixes: vec![FixRecord {
                rule_id: rule_id.to_owned(),
                description: "test".to_owned(),
                line: None,
                count: 1,
            }],
        }
    }

    #[test]
    fn ordinary_plan_cannot_impersonate_a_surviving_destructive_bundle_member() {
        let project = TempDir::new().expect("project");
        let source = project.path().join("unused.c");
        let header = project.path().join("api.h");
        let bundle = [source.clone(), header.clone()]
            .into_iter()
            .collect::<BTreeSet<_>>();
        let static_plan = destructive_test_plan(&source, "UNSAFE_REMOVE_UNUSED_STATIC");
        let ordinary_header_plan = destructive_test_plan(&header, "HEADER_GUARD_RENAME");

        assert!(dependent_destructive_bundle_is_partial(
            &[static_plan.clone(), ordinary_header_plan],
            &bundle,
        ));
        assert!(!dependent_destructive_bundle_is_partial(
            &[
                static_plan,
                destructive_test_plan(&header, "UNSAFE_REMOVE_ORPHAN_PROTOTYPE"),
            ],
            &bundle,
        ));
    }

    #[test]
    fn diagnostic_preludes_never_invent_or_cache_source_bytes() {
        let project = TempDir::new().expect("project");
        let source = project.path().join("missing.c");
        let mut preludes = BTreeMap::new();

        let prelude = prelude_entry(&mut preludes, &source);
        assert!(prelude.source.is_none());
        assert!(prelude.original_blake3.is_none());
        assert!(load_destructive_source(prelude, &source).is_err());
        assert!(prelude.source.is_none());
        assert!(prelude.original_blake3.is_none());
    }

    #[test]
    fn destructive_source_override_requires_the_authoritative_original_digest() {
        let project = TempDir::new().expect("project");
        let source = project.path().join("main.c");
        fs::write(&source, "static int\tunused(void)\n{\n\treturn (0);\n}\n").expect("source");
        let mut preludes = BTreeMap::new();
        let prelude = prelude_entry(&mut preludes, &source);
        load_destructive_source(prelude, &source).expect("snapshot");

        assert!(
            prelude
                .confirmed_source(b"static int\tunused(void)\n{\n\treturn (0);\n}\n")
                .is_some()
        );
        assert!(
            prelude
                .confirmed_source(b"static int\tused(void)\n{\n\treturn (0);\n}\n")
                .is_none()
        );
    }
}
