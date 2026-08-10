//! End-to-end native fix pipeline.
//!
//! Analysis and formatting happen in immutable shadow buffers. The only write
//! boundary is the validated multi-file transaction in `normfix-actions`.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fs::{File, OpenOptions};
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt as _;

use camino::Utf8PathBuf;
use normfix_actions::{PlannedFile, ReadPrecondition, TransactionOptions, commit_files_guarded};
use normfix_c_actions::{
    CActionError, CActionOptions, ReportedDiagnostic, analyze_budget, analyze_c,
    analyze_external_calls, apply_c_actions,
};
use normfix_c_semantics::{ArrayBoundKind, analyze as analyze_semantics};
use normfix_c_syntax::{CFunctionKind, CParser};
use normfix_cache::{CacheKey, CachePaths, PersistentCache, PreparedCacheEntry};
use normfix_core::{
    Diagnostic, DiagnosticSource, FileId, FixRecord, Severity, SourceSnapshot, TextRange, TextSize,
    apply_source_edits,
};
use normfix_destructive::{
    ClosedCSourceSet, DestructiveAuthorization, DestructiveCapability, OrphanPrototypePlan,
    QuarantineItem, QuarantineRequest, StaticRemovalPlan, plan_orphan_prototypes, plan_quarantine,
    plan_unused_static_functions,
};
use normfix_header::{
    ByteRange, Identity42, RunClock, c_header_filename_matches, ensure_c_header, update_c_header,
};
use normfix_makefile::{
    SourcePathStatus, analyze_makefile, format_makefile, reconcile_source_references,
};
use normfix_markdown::analyze_markdown;
use normfix_oracle::{
    CompilerConfig, CompilerError, CompilerReport, CompilerValidator, NorminetteConfig,
    NorminetteDiagnostic, NorminetteError, NorminetteOracle, NorminetteReport, ProcessLimits,
};
use normfix_project::{
    DiscoveredFile, DiscoveryOptions, GuardApproval, GuardInsertionApproval, ProjectFileKind,
    ProjectPolicy, ProjectSnapshot, discover, guard_approval_is_current,
    guard_insertion_approval_is_current, load_project_policy, plan_guard_insertions,
    plan_guard_renames,
};
use normfix_report::{FileReport, ReportIdentity, ReportMode, RunReport};
use rayon::ThreadPoolBuilder;
use rayon::prelude::*;
use regex::Regex;
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

struct FileWork {
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

/// Drops the path-trace note that merely repeats its own finding.
///
/// The Clang analyzer reports a finding twice at the same position: once as a
/// warning tagged with the checker that produced it, and once as the first note
/// of the trace, untagged. Both are useful in a raw log and redundant in a
/// report, so keep the tagged one.
fn deduplicate_analyzer_trace(diagnostics: &mut Vec<Diagnostic>) {
    fn untagged(message: &str) -> &str {
        message
            .rfind(" [")
            .filter(|_| message.ends_with(']'))
            .map_or(message, |index| &message[..index])
    }

    let tagged = diagnostics
        .iter()
        .filter(|diagnostic| untagged(&diagnostic.message) != diagnostic.message)
        .map(|diagnostic| {
            (
                diagnostic.range.start(),
                untagged(&diagnostic.message).to_owned(),
            )
        })
        .collect::<BTreeSet<_>>();
    diagnostics.retain(|diagnostic| {
        untagged(&diagnostic.message) != diagnostic.message
            || !tagged.contains(&(diagnostic.range.start(), diagnostic.message.clone()))
    });
}

/// The analyzer a compiler actually ships, which is not the same question as
/// what the command is called.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CompilerFamily {
    Gcc,
    Clang,
    Unknown,
}

impl CompilerFamily {
    /// Classifies a compiler from its own version banner.
    ///
    /// Clang is checked first on purpose: `/usr/bin/gcc` on macOS is Clang
    /// wearing another name, and it answers `Apple clang version ...`. Trusting
    /// the command name would send `-fanalyzer` to a compiler that rejects it.
    fn from_version(version_output: &str) -> Self {
        let banner = version_output.to_ascii_lowercase();
        if banner.contains("clang") {
            Self::Clang
        } else if banner.contains("gcc") || banner.contains("free software foundation") {
            Self::Gcc
        } else {
            Self::Unknown
        }
    }

    /// Stable cache namespace, because the flags differ per family.
    const fn analyzer_namespace(self) -> &'static str {
        match self {
            Self::Clang => "clang-analyze-v1",
            Self::Gcc | Self::Unknown => "gcc-fanalyzer-v3",
        }
    }
}

fn compiler_arguments(
    analyzer: bool,
    family: CompilerFamily,
    include_directories: &[PathBuf],
) -> Vec<OsString> {
    let mut arguments = Vec::<OsString>::new();
    if analyzer {
        match family {
            CompilerFamily::Clang => {
                // `--analyze` replaces the syntax-only mode. Passing both makes
                // Clang ignore the analyzer and warn about an unused argument.
                arguments.extend(
                    [
                        "--analyze",
                        "-Xclang",
                        "-analyzer-output=text",
                        "-Wall",
                        "-Wextra",
                    ]
                    .map(OsString::from),
                );
            }
            CompilerFamily::Gcc | CompilerFamily::Unknown => {
                arguments.extend(
                    ["-fsyntax-only", "-Wall", "-Wextra", "-fanalyzer"].map(OsString::from),
                );
            }
        }
    } else {
        arguments.extend(["-fsyntax-only", "-Wall", "-Wextra", "-Werror"].map(OsString::from));
    }
    for directory in include_directories {
        arguments.push(OsString::from("-I"));
        arguments.push(directory.as_os_str().to_owned());
    }
    arguments
}

#[derive(Default)]
struct CompilerProjectContext {
    fingerprint: Option<[u8; 32]>,
    include_directories: Vec<PathBuf>,
}

const COMPILER_FINGERPRINT_FILE_LIMIT: u64 = 8 * 1024 * 1024;
const COMPILER_FINGERPRINT_PROJECT_LIMIT: u64 = 64 * 1024 * 1024;
const MAKEFILE_TRIVIA_PROBE_LIMIT: u64 = 8 * 1024 * 1024;

fn compiler_project_context(project_root: &Path) -> CompilerProjectContext {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"normfix-compiler-project-v2\0");
    let absolute_root = absolute_lexical(project_root);
    let discovery = discover(
        &[],
        &DiscoveryOptions::new(&absolute_root)
            .with_respect_gitignore(false)
            .with_respect_normfixignore(false),
    );
    let mut include_directories = discovery
        .processable_files
        .iter()
        .filter(|file| file.kind == ProjectFileKind::CHeader)
        .filter_map(|file| file.path.parent())
        .filter_map(|parent| parent.strip_prefix(&absolute_root).ok())
        .map(|relative| {
            if relative.as_os_str().is_empty() {
                PathBuf::from(".")
            } else {
                relative.to_path_buf()
            }
        })
        .collect::<Vec<_>>();
    include_directories.sort();
    include_directories.dedup();
    let mut paths = discovery
        .processable_files
        .into_iter()
        .map(|file| file.path)
        .chain(discovery.unexpected_files)
        .collect::<Vec<_>>();
    paths.sort();
    paths.dedup();
    let mut remaining_bytes = COMPILER_FINGERPRINT_PROJECT_LIMIT;
    let mut fingerprint_complete = discovery.errors.is_empty();
    for file in paths {
        if fingerprint_complete
            && !hash_compiler_project_file(&mut hasher, &file, &mut remaining_bytes)
        {
            fingerprint_complete = false;
        }
    }
    for error in discovery.errors {
        let detail = error.to_string();
        hasher.update(&u64::MAX.to_le_bytes());
        hasher.update(detail.as_bytes());
    }
    CompilerProjectContext {
        fingerprint: fingerprint_complete.then(|| *hasher.finalize().as_bytes()),
        include_directories,
    }
}

fn hash_compiler_project_file(
    hasher: &mut blake3::Hasher,
    file: &Path,
    remaining_bytes: &mut u64,
) -> bool {
    let path = file.to_string_lossy();
    hasher.update(&(path.len() as u64).to_le_bytes());
    hasher.update(path.as_bytes());
    let input = match File::open(file) {
        Ok(input) => input,
        Err(error) => {
            let detail = error.to_string();
            hasher.update(&u64::MAX.to_le_bytes());
            hasher.update(detail.as_bytes());
            return false;
        }
    };
    let metadata = match input.metadata() {
        Ok(metadata) => metadata,
        Err(error) => {
            hasher.update(b"metadata-error\0");
            hasher.update(error.to_string().as_bytes());
            return false;
        }
    };
    let expected_length = metadata.len();
    let read_budget = expected_length.saturating_add(1);
    if !metadata.is_file()
        || expected_length > COMPILER_FINGERPRINT_FILE_LIMIT
        || read_budget > *remaining_bytes
    {
        hasher.update(b"bounded-read-refused\0");
        hasher.update(&expected_length.to_le_bytes());
        return false;
    }
    let mut content = blake3::Hasher::new();
    let mut length = 0_u64;
    let mut buffer = [0_u8; 16 * 1024];
    let mut input = input.take(read_budget);
    loop {
        match input.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => {
                length = length.saturating_add(read as u64);
                *remaining_bytes = remaining_bytes.saturating_sub(read as u64);
                content.update(&buffer[..read]);
            }
            Err(error) => {
                let detail = error.to_string();
                hasher.update(&u64::MAX.to_le_bytes());
                hasher.update(detail.as_bytes());
                return false;
            }
        }
    }
    if length != expected_length {
        hasher.update(b"concurrent-length-change\0");
        hasher.update(&expected_length.to_le_bytes());
        hasher.update(&length.to_le_bytes());
        return false;
    }
    hasher.update(&length.to_le_bytes());
    hasher.update(content.finalize().as_bytes());
    true
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

fn project_diagnostic(path: Utf8PathBuf, rule_id: &str, message: &str, help: &str) -> Diagnostic {
    Diagnostic {
        rule_id: rule_id.to_owned(),
        path,
        range: TextRange::empty(TextSize::new(0)),
        severity: Severity::Warning,
        message: message.to_owned(),
        source: DiagnosticSource::Project,
        notes: Vec::new(),
        help: Some(help.to_owned()),
    }
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

fn failed_file(file: &DiscoveredFile, path: Utf8PathBuf, failure: String) -> FileWork {
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

fn failed_source(
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

fn process_makefile(
    file: &DiscoveredFile,
    path: Utf8PathBuf,
    original_bytes: &[u8],
    original: String,
    options: &FixOptions,
    clock: &RunClock,
) -> FileWork {
    if options.lint_only {
        let mut after = makefile_diagnostics(file, &path, &original, options);
        after.sort();
        after.dedup();
        let before = after.clone();
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
    let filename = file
        .path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Makefile");
    let remove_missing = options.remove_missing_makefile_sources
        && options
            .destructive_authorization
            .as_ref()
            .is_some_and(|authorization| {
                authorization.allows(DestructiveCapability::RemoveMissingMakefileSources)
            });
    let mut before = makefile_diagnostics(file, &path, &original, options);
    before.sort();
    before.dedup();
    let mut empty_source_proofs = BTreeMap::<String, (PathBuf, [u8; 32])>::new();
    let reconciled = reconcile_source_references(&original, remove_missing, |reference| {
        let (status, proof) = makefile_source_probe(&file.path, &options.cwd, reference);
        if let Some(proof) = proof {
            empty_source_proofs.insert(reference.to_owned(), proof);
        }
        status
    });
    let formatted = format_makefile(
        &reconciled.output,
        filename,
        options.identity.as_ref(),
        clock,
    );
    let mut fixes = Vec::new();
    append_header_fixes(&mut fixes, &reconciled.fixes, &original);
    append_header_fixes(&mut fixes, &formatted.fixes, &reconciled.output);
    let mut after = Vec::new();
    append_header_issues(&mut after, &path, &formatted.issues);
    after.extend(makefile_diagnostics(
        file,
        &path,
        &formatted.output,
        options,
    ));
    after.sort();
    after.dedup();
    let changed = formatted.output.as_bytes() != original_bytes;
    let read_preconditions =
        makefile_reconciliation_preconditions(file, options, &reconciled, &empty_source_proofs);
    let plan = changed.then(|| PlannedFile {
        path: file.path.clone(),
        original: original_bytes.to_vec(),
        replacement: formatted.output.as_bytes().to_vec(),
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
            original: Some(Arc::from(original)),
            fixed: Some(Arc::from(formatted.output)),
        },
        plan,
        read_preconditions,
    }
}

fn makefile_reconciliation_preconditions(
    file: &DiscoveredFile,
    options: &FixOptions,
    reconciled: &normfix_makefile::SourceReconciliation,
    empty_source_proofs: &BTreeMap<String, (PathBuf, [u8; 32])>,
) -> Vec<ReadPrecondition> {
    let removed_missing_ranges = reconciled
        .fixes
        .iter()
        .filter(|fix| fix.code == "MAKEFILE_REMOVE_MISSING_SOURCE")
        .map(|fix| fix.range)
        .collect::<BTreeSet<_>>();
    let removed_empty_ranges = reconciled
        .fixes
        .iter()
        .filter(|fix| fix.code == "MAKEFILE_REMOVE_EMPTY_SOURCE")
        .map(|fix| fix.range)
        .collect::<BTreeSet<_>>();
    let mut preconditions = reconciled
        .missing
        .iter()
        .filter(|reference| removed_missing_ranges.contains(&reference.range))
        .filter_map(|reference| {
            makefile_source_path(&file.path, &options.cwd, &reference.path)
                .map(ReadPrecondition::absent)
        })
        .collect::<Vec<_>>();
    preconditions.extend(
        reconciled
            .empty
            .iter()
            .filter(|reference| removed_empty_ranges.contains(&reference.range))
            .filter_map(|reference| empty_source_proofs.get(&reference.path))
            .map(|(path, digest)| ReadPrecondition::Matches {
                path: path.clone(),
                blake3: *digest,
            }),
    );
    preconditions
}

fn makefile_diagnostics(
    file: &DiscoveredFile,
    path: &Utf8PathBuf,
    source: &str,
    options: &FixOptions,
) -> Vec<Diagnostic> {
    let mut diagnostics = analyze_makefile(source)
        .into_iter()
        .map(|item| Diagnostic {
            rule_id: item.code.to_owned(),
            path: path.clone(),
            range: text_range(item.range),
            severity: Severity::Warning,
            message: item.message,
            source: DiagnosticSource::Makefile,
            notes: (!item.detail.is_empty())
                .then_some(item.detail)
                .into_iter()
                .collect(),
            help: Some(item.suggestion),
        })
        .collect::<Vec<_>>();
    let remaining_sources = reconcile_source_references(source, false, |reference| {
        makefile_source_status(&file.path, &options.cwd, reference)
    });
    diagnostics.extend(remaining_sources.missing.into_iter().map(|reference| {
        Diagnostic {
            rule_id: "MAKEFILE_SOURCE_NOT_FOUND".to_owned(),
            path: path.clone(),
            range: text_range(reference.range),
            severity: Severity::Warning,
            message: format!(
                "The literal Makefile source `{}` does not exist below the project root.",
                reference.path
            ),
            source: DiagnosticSource::Makefile,
            notes: vec![
                "Only a wholly literal SRC/SRCS-style assignment was inspected; Make recipes and expansions were never executed."
                    .to_owned(),
            ],
            help: Some(
                "Create/correct the source path, or use the explicitly authorized unsafe removal mode to remove this exact stale token."
                    .to_owned(),
            ),
        }
    }));
    diagnostics.extend(remaining_sources.empty.into_iter().map(|reference| Diagnostic {
        rule_id: "MAKEFILE_SOURCE_EMPTY".to_owned(),
        path: path.clone(),
        range: text_range(reference.range),
        severity: Severity::Warning,
        message: format!(
            "The literal Makefile source `{}` contains only whitespace or comments.",
            reference.path
        ),
        source: DiagnosticSource::Makefile,
        notes: vec![
            "The file was read as a regular non-symbolic path below the project root; comments and an official header do not count as an implementation."
                .to_owned(),
        ],
        help: Some(
            "Implement the source, remove the stale entry manually, or use explicitly authorized unsafe mode to remove this exact empty token."
                .to_owned(),
        ),
    }));
    diagnostics
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

fn makefile_source_status(
    makefile: &Path,
    project_root: &Path,
    reference: &str,
) -> SourcePathStatus {
    makefile_source_probe(makefile, project_root, reference).0
}

fn makefile_source_probe(
    makefile: &Path,
    project_root: &Path,
    reference: &str,
) -> (SourcePathStatus, Option<(PathBuf, [u8; 32])>) {
    let Ok(root) = project_root.canonicalize() else {
        return (SourcePathStatus::Unknown, None);
    };
    let Some(parent) = makefile.parent() else {
        return (SourcePathStatus::Unknown, None);
    };
    let Ok(parent) = parent.canonicalize() else {
        return (SourcePathStatus::Unknown, None);
    };
    if !parent.starts_with(&root) {
        return (SourcePathStatus::Unknown, None);
    }
    let relative = Path::new(reference);
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return (SourcePathStatus::Unknown, None);
    }
    let mut candidate = parent;
    for component in relative.components() {
        let std::path::Component::Normal(name) = component else {
            return (SourcePathStatus::Unknown, None);
        };
        candidate.push(name);
        match std::fs::symlink_metadata(&candidate) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return (SourcePathStatus::Unknown, None);
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return (SourcePathStatus::Missing, None);
            }
            Err(_) => return (SourcePathStatus::Unknown, None),
        }
    }
    let Ok(metadata) = std::fs::symlink_metadata(&candidate) else {
        return (SourcePathStatus::Unknown, None);
    };
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return (SourcePathStatus::Unknown, None);
    }
    let Ok(input) = File::open(&candidate) else {
        return (SourcePathStatus::Unknown, None);
    };
    let Ok(opened_metadata) = input.metadata() else {
        return (SourcePathStatus::Unknown, None);
    };
    if !opened_metadata.is_file() || opened_metadata.len() > MAKEFILE_TRIVIA_PROBE_LIMIT {
        return (SourcePathStatus::Unknown, None);
    }
    let expected_length = opened_metadata.len();
    let Ok(capacity) = usize::try_from(expected_length) else {
        return (SourcePathStatus::Unknown, None);
    };
    let mut bytes = Vec::with_capacity(capacity);
    if input
        .take(expected_length.saturating_add(1))
        .read_to_end(&mut bytes)
        .is_err()
        || u64::try_from(bytes.len()) != Ok(expected_length)
    {
        return (SourcePathStatus::Unknown, None);
    }
    if contains_only_c_trivia(&bytes) {
        let digest = *blake3::hash(&bytes).as_bytes();
        let Ok(relative) = candidate.strip_prefix(&root) else {
            return (SourcePathStatus::Unknown, None);
        };
        return (
            SourcePathStatus::Empty,
            Some((project_root.join(relative), digest)),
        );
    }
    (SourcePathStatus::Exists, None)
}

fn contains_only_c_trivia(bytes: &[u8]) -> bool {
    let mut index = 0_usize;
    while index < bytes.len() {
        if bytes[index].is_ascii_whitespace() {
            index += 1;
            continue;
        }
        if bytes.get(index..index + 2) == Some(b"//") {
            index += 2;
            while index < bytes.len() && !matches!(bytes[index], b'\r' | b'\n') {
                index += 1;
            }
            continue;
        }
        if bytes.get(index..index + 2) == Some(b"/*") {
            let Some(end) = bytes[index + 2..]
                .windows(2)
                .position(|window| window == b"*/")
            else {
                return false;
            };
            index += end + 4;
            continue;
        }
        return false;
    }
    true
}

fn makefile_source_path(makefile: &Path, project_root: &Path, reference: &str) -> Option<PathBuf> {
    let root = project_root.canonicalize().ok()?;
    let parent = makefile.parent()?.canonicalize().ok()?;
    if !parent.starts_with(&root) {
        return None;
    }
    let relative = Path::new(reference);
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return None;
    }
    let candidate = parent.join(relative);
    let inside = candidate.strip_prefix(&root).ok()?;
    Some(project_root.join(inside))
}

fn process_markdown(
    file: &DiscoveredFile,
    path: Utf8PathBuf,
    original_bytes: Vec<u8>,
    original: String,
    options: &FixOptions,
) -> FileWork {
    let result = match analyze_markdown(&original, options.format_markdown && !options.lint_only) {
        Ok(result) => result,
        Err(error) => {
            return failed_source(file, path, original_bytes, original, error.to_string());
        }
    };
    let proposed = result.formatted.unwrap_or_else(|| original.clone());
    let mut after = result
        .issues
        .into_iter()
        .map(|issue| Diagnostic {
            rule_id: issue.rule_id,
            path: path.clone(),
            range: line_point_range(&proposed, issue.line),
            severity: Severity::Info,
            message: issue.message,
            source: DiagnosticSource::Markdown,
            notes: Vec::new(),
            help: Some(issue.help),
        })
        .collect::<Vec<_>>();
    if options.preflight {
        after.push(Diagnostic {
            rule_id: "README_42_CRITERIA_REVIEW".to_owned(),
            path: path.clone(),
            range: TextRange::empty(TextSize::new(0)),
            severity: Severity::Info,
            message: "A README is present, but its subject-specific 42 evaluation criteria cannot be proven automatically."
                .to_owned(),
            source: DiagnosticSource::Markdown,
            notes: vec![
                "README absence is not a normfix preflight failure; this advisory exists only when a README was discovered."
                    .to_owned(),
            ],
            help: Some(
                "Compare the document with the current subject and evaluation sheet: required overview, instructions, resources, attribution, and project-specific sections vary by cursus project."
                    .to_owned(),
            ),
        });
    }
    after.sort();
    after.dedup();
    let changed = proposed.as_bytes() != original_bytes;
    let fix_records = changed
        .then(|| FixRecord {
            rule_id: "MARKDOWN_CANONICAL_FORMAT".to_owned(),
            description: "reprinted the README through the CommonMark syntax tree".to_owned(),
            line: None,
            count: 1,
        })
        .into_iter()
        .collect::<Vec<_>>();
    let plan = changed.then(|| PlannedFile {
        path: file.path.clone(),
        original: original_bytes.clone(),
        replacement: proposed.as_bytes().to_vec(),
        fixes: fix_records.clone(),
    });
    FileWork {
        absolute_path: file.path.clone(),
        report: FileReport {
            path,
            changed,
            written: false,
            backup: None,
            failure: None,
            fixes: fix_records,
            before: Vec::new(),
            after,
            original: Some(Arc::from(original)),
            fixed: Some(Arc::from(proposed)),
        },
        plan,
        read_preconditions: Vec::new(),
    }
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

fn quarantine_unexpected_files(
    unexpected: &[PathBuf],
    options: &FixOptions,
) -> (Vec<Utf8PathBuf>, Vec<String>) {
    let Some(authorization) = options.destructive_authorization.as_ref() else {
        return (
            Vec::new(),
            vec![
                "Unexpected files were not quarantined because destructive authorization is missing."
                    .to_owned(),
            ],
        );
    };
    let (project_root, recovery_root) = match quarantine_roots(options) {
        Ok(roots) => roots,
        Err(error) => return (Vec::new(), vec![error]),
    };
    let mut relative_paths = Vec::new();
    let mut errors = Vec::new();
    for path in unexpected {
        let Ok(relative) = path.strip_prefix(options.cwd.as_path()) else {
            errors.push(format!(
                "Unexpected path is outside the project and was not quarantined: {}",
                path.display()
            ));
            continue;
        };
        match Utf8PathBuf::from_path_buf(relative.to_path_buf()) {
            Ok(relative) => relative_paths.push(relative),
            Err(path) => errors.push(format!(
                "Unexpected path is not valid UTF-8 and was not quarantined: {}",
                path.display()
            )),
        }
    }
    let request = QuarantineRequest {
        project_root,
        recovery_root,
        run_id: run_id(),
        paths: relative_paths,
    };
    let plan = match plan_quarantine(&request, authorization) {
        Ok(plan) => plan,
        Err(error) => {
            errors.push(error.to_string());
            return (Vec::new(), errors);
        }
    };
    errors.extend(plan.diagnostics.into_iter().map(|diagnostic| {
        let detail = diagnostic.notes.join(" ");
        if detail.is_empty() {
            diagnostic.to_string()
        } else {
            format!("{diagnostic}: {detail}")
        }
    }));
    match execute_quarantine(&plan.items) {
        Ok(paths) => (paths, errors),
        Err(error) => {
            errors.push(error);
            (Vec::new(), errors)
        }
    }
}

fn quarantine_roots(options: &FixOptions) -> Result<(Utf8PathBuf, Utf8PathBuf), String> {
    let recovery = match &options.backup {
        BackupPolicy::Directory(path) => path.join("quarantine"),
        BackupPolicy::Automatic | BackupPolicy::Disabled => {
            let root = automatic_backup_root().ok_or_else(|| {
                "No external recovery directory is available for quarantine.".to_owned()
            })?;
            root.join("quarantine")
        }
    };
    let project_root = options
        .cwd
        .canonicalize()
        .map_err(|error| format!("Could not resolve the project root: {error}"))?;
    if !project_root.is_dir() {
        return Err("The project root is not a directory.".to_owned());
    }
    let recovery = prepare_external_recovery_root(&recovery, &project_root)?;
    let project_root = Utf8PathBuf::from_path_buf(project_root)
        .map_err(|path| format!("The project root is not valid UTF-8: {}", path.display()))?;
    let recovery_root = Utf8PathBuf::from_path_buf(recovery)
        .map_err(|path| format!("The recovery path is not valid UTF-8: {}", path.display()))?;
    Ok((project_root, recovery_root))
}

fn prepare_external_recovery_root(
    requested: &Path,
    project_root: &Path,
) -> Result<PathBuf, String> {
    let requested = absolute_lexical(requested);
    if requested.starts_with(project_root) || project_root.starts_with(&requested) {
        return Err(format!(
            "Quarantine recovery storage must not overlap the project: {}",
            requested.display()
        ));
    }
    let mut existing = requested.clone();
    let mut missing = Vec::<OsString>::new();
    loop {
        match std::fs::symlink_metadata(&existing) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() {
                    return Err(format!(
                        "Quarantine recovery path contains a symbolic link: {}",
                        existing.display()
                    ));
                }
                if !metadata.is_dir() {
                    return Err(format!(
                        "Quarantine recovery ancestor is not a directory: {}",
                        existing.display()
                    ));
                }
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let Some(name) = existing.file_name() else {
                    return Err(format!(
                        "Quarantine recovery path has no existing ancestor: {}",
                        requested.display()
                    ));
                };
                missing.push(name.to_os_string());
                if !existing.pop() {
                    return Err(format!(
                        "Quarantine recovery path has no existing ancestor: {}",
                        requested.display()
                    ));
                }
            }
            Err(error) => {
                return Err(format!(
                    "Could not inspect quarantine recovery path `{}`: {error}",
                    existing.display()
                ));
            }
        }
    }
    let mut canonical = existing.canonicalize().map_err(|error| {
        format!(
            "Could not resolve quarantine recovery ancestor `{}`: {error}",
            existing.display()
        )
    })?;
    if canonical.starts_with(project_root) || project_root.starts_with(&canonical) {
        return Err(format!(
            "Quarantine recovery storage must not overlap the project: {}",
            canonical.display()
        ));
    }
    for component in missing.into_iter().rev() {
        let next = canonical.join(component);
        match std::fs::create_dir(&next) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => {
                return Err(format!(
                    "Could not create quarantine recovery directory `{}`: {error}",
                    next.display()
                ));
            }
        }
        let metadata = std::fs::symlink_metadata(&next).map_err(|error| {
            format!(
                "Could not revalidate quarantine recovery directory `{}`: {error}",
                next.display()
            )
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(format!(
                "Quarantine recovery path is not a real directory: {}",
                next.display()
            ));
        }
        canonical = next.canonicalize().map_err(|error| {
            format!(
                "Could not resolve quarantine recovery directory `{}`: {error}",
                next.display()
            )
        })?;
        if canonical.starts_with(project_root) || project_root.starts_with(&canonical) {
            return Err(format!(
                "Quarantine recovery storage must not overlap the project: {}",
                canonical.display()
            ));
        }
    }
    Ok(canonical)
}

fn execute_quarantine(items: &[QuarantineItem]) -> Result<Vec<Utf8PathBuf>, String> {
    for item in items {
        let metadata = std::fs::symlink_metadata(&item.source_path)
            .map_err(|error| format!("Could not revalidate `{}`: {error}", item.source_path))?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(format!(
                "Quarantine source is no longer a regular file: {}",
                item.source_path
            ));
        }
        let bytes = std::fs::read(&item.source_path)
            .map_err(|error| format!("Could not re-read `{}`: {error}", item.source_path))?;
        if bytes.len() as u64 != item.snapshot.byte_length
            || blake3::hash(&bytes).to_hex().to_string() != item.snapshot.blake3_hash
        {
            return Err(format!(
                "Quarantine source changed after planning: {}",
                item.source_path
            ));
        }
        if item.destination_path.exists() {
            return Err(format!(
                "Quarantine destination already exists: {}",
                item.destination_path
            ));
        }
    }

    let mut staged = Vec::<&QuarantineItem>::new();
    for item in items {
        let Some(parent) = item.destination_path.parent() else {
            cleanup_staged_quarantine(&staged);
            return Err(format!(
                "Quarantine destination has no parent: {}",
                item.destination_path
            ));
        };
        if let Err(error) = std::fs::create_dir_all(parent) {
            cleanup_staged_quarantine(&staged);
            return Err(format!(
                "Could not create quarantine directory `{parent}`: {error}"
            ));
        }
        let result = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&item.destination_path)
            .and_then(|mut file| {
                file.write_all(&item.snapshot.bytes)?;
                file.sync_all()
            });
        if let Err(error) = result {
            cleanup_staged_quarantine(&staged);
            return Err(format!(
                "Could not stage quarantine copy `{}`: {error}",
                item.destination_path
            ));
        }
        if item.snapshot.readonly {
            if let Ok(mut permissions) =
                std::fs::metadata(&item.destination_path).map(|metadata| metadata.permissions())
            {
                permissions.set_readonly(true);
                let _ = std::fs::set_permissions(&item.destination_path, permissions);
            }
        }
        staged.push(item);
    }

    let mut removed = Vec::<&QuarantineItem>::new();
    for item in items {
        if let Err(error) = std::fs::remove_file(&item.source_path) {
            let rollback_errors = rollback_quarantine(&removed, &staged);
            let suffix = if rollback_errors.is_empty() {
                "Previously moved files were restored.".to_owned()
            } else {
                format!("Rollback needs review: {}", rollback_errors.join(" | "))
            };
            return Err(format!(
                "Could not remove quarantined source `{}`: {error}. {suffix}",
                item.source_path
            ));
        }
        removed.push(item);
    }
    Ok(items
        .iter()
        .map(|item| item.relative_path.clone())
        .collect())
}

fn cleanup_staged_quarantine(items: &[&QuarantineItem]) {
    for item in items.iter().rev() {
        let _ = std::fs::remove_file(&item.destination_path);
    }
}

fn rollback_quarantine(removed: &[&QuarantineItem], staged: &[&QuarantineItem]) -> Vec<String> {
    let mut errors = Vec::new();
    for item in removed.iter().rev() {
        let restored = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&item.restore_path)
            .and_then(|mut file| {
                file.write_all(&item.snapshot.bytes)?;
                file.sync_all()
            });
        match restored {
            Ok(()) => {
                if let Err(error) = std::fs::remove_file(&item.destination_path) {
                    errors.push(format!(
                        "restored `{}` but could not remove recovery copy: {error}",
                        item.restore_path
                    ));
                }
            }
            Err(error) => errors.push(format!(
                "could not restore `{}`: {error}",
                item.restore_path
            )),
        }
    }
    for item in staged {
        if !removed
            .iter()
            .any(|removed_item| removed_item.source_path == item.source_path)
        {
            let _ = std::fs::remove_file(&item.destination_path);
        }
    }
    errors
}

fn report_identity(options: &FixOptions) -> ReportIdentity {
    options.identity.as_ref().map_or_else(
        || ReportIdentity {
            source: options.identity_source.clone(),
            ..ReportIdentity::default()
        },
        |identity| ReportIdentity {
            login: identity.login.clone(),
            email: identity.email.clone(),
            source: identity.source.clone(),
            inferred: identity.inferred(),
            available: true,
        },
    )
}

fn report_path(path: &Path, cwd: &Path) -> Result<Utf8PathBuf, PathBuf> {
    let display = path
        .strip_prefix(cwd)
        .ok()
        .filter(|relative| !relative.as_os_str().is_empty())
        .unwrap_or(path);
    Utf8PathBuf::from_path_buf(display.to_path_buf())
}

fn transaction_root<'a>(paths: impl Iterator<Item = &'a Path>, fallback: &Path) -> PathBuf {
    let mut paths = paths.map(absolute_lexical);
    let Some(mut common) = paths.next() else {
        return absolute_lexical(fallback);
    };
    common.pop();
    for path in paths {
        while !path.starts_with(&common) {
            if !common.pop() {
                return absolute_lexical(fallback);
            }
        }
    }
    common
}

fn absolute_lexical(path: &Path) -> PathBuf {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().map_or_else(|_| path.to_path_buf(), |cwd| cwd.join(path))
    };
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

fn automatic_backup_root() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("XDG_DATA_HOME").filter(|path| !path.is_empty()) {
        return Some(PathBuf::from(path).join("normfix/backups"));
    }
    std::env::var_os("HOME")
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .map(|home| home.join(".local/share/normfix/backups"))
}

fn run_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    format!("run-{nanos}-{}", std::process::id())
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

fn append_header_fixes(output: &mut Vec<FixRecord>, fixes: &[normfix_header::Fix], source: &str) {
    output.extend(fixes.iter().map(|item| FixRecord {
        rule_id: item.code.to_owned(),
        description: item.description.clone(),
        line: line_for_offset(source, item.range.start),
        count: 1,
    }));
}

fn append_header_issues(
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

fn text_range(range: ByteRange) -> TextRange {
    let start = u32::try_from(range.start).unwrap_or(u32::MAX);
    let end = u32::try_from(range.end).unwrap_or(u32::MAX).max(start);
    TextRange::new(TextSize::new(start), TextSize::new(end))
        .unwrap_or_else(|| TextRange::empty(TextSize::new(start)))
}

fn line_for_offset(source: &str, offset: usize) -> Option<u32> {
    if offset > source.len() || !source.is_char_boundary(offset) {
        return None;
    }
    u32::try_from(
        source[..offset]
            .bytes()
            .filter(|byte| *byte == b'\n')
            .count()
            + 1,
    )
    .ok()
}

fn official_diagnostics(
    path: &Utf8PathBuf,
    source: &str,
    diagnostics: &[NorminetteDiagnostic],
    norminette_version: &str,
) -> Vec<Diagnostic> {
    diagnostics
        .iter()
        .map(|item| Diagnostic {
            rule_id: item.rule_id.clone(),
            path: path.clone(),
            range: diagnostic_range(source, item.line, item.column, ColumnUnit::Display),
            severity: Severity::Warning,
            message: item.message.clone(),
            source: DiagnosticSource::NorminetteCompat(norminette_version.to_owned()),
            notes: Vec::new(),
            help: Some(diagnostic_help(&item.rule_id).to_owned()),
        })
        .collect()
}

/// Merges official findings without allowing one native rule occurrence to
/// hide other official occurrences of that rule at distinct source locations.
fn merge_official_diagnostics(
    diagnostics: &mut Vec<Diagnostic>,
    official: Vec<Diagnostic>,
    corroborate_native: bool,
    norminette_version: &str,
) {
    let represented = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.source == DiagnosticSource::NativeNorm41)
        .filter(|diagnostic| {
            official.iter().any(|candidate| {
                candidate.rule_id == diagnostic.rule_id
                    && candidate.range.start() == diagnostic.range.start()
            })
        })
        .map(|diagnostic| (diagnostic.rule_id.clone(), diagnostic.range.start()))
        .collect::<BTreeSet<_>>();
    if corroborate_native {
        for diagnostic in diagnostics.iter_mut().filter(|diagnostic| {
            represented.contains(&(diagnostic.rule_id.clone(), diagnostic.range.start()))
        }) {
            diagnostic.source = DiagnosticSource::NorminetteCompat(norminette_version.to_owned());
        }
    }
    diagnostics.extend(official.into_iter().filter(|diagnostic| {
        !represented.contains(&(diagnostic.rule_id.clone(), diagnostic.range.start()))
    }));
}

fn diagnostic_help(rule_id: &str) -> &'static str {
    match rule_id {
        "TOO_MANY_LINES" => "Extract one coherent responsibility into a well-named static helper.",
        "TOO_MANY_ARGS" => {
            "Reduce the function contract to four parameters or group genuinely related state."
        }
        "TOO_MANY_VARS_FUNC" => {
            "Split the responsibility or simplify the local declaration block to five variables."
        }
        "TOO_MANY_FUNCS" => {
            "Move a cohesive group of functions to another .c file and update interfaces and the Makefile."
        }
        "LINE_TOO_LONG" => "Shorten a literal/comment manually when no token-safe break exists.",
        "VLA_FORBIDDEN" => {
            "Use a proven integer constant expression or an allowed dynamic-allocation strategy."
        }
        "WRONG_SCOPE_COMMENT" | "COMMENT_ON_INSTR" => {
            "Move the comment to an allowed scope, or rerun with --remove-invalid-comments to delete this exact rejected comment."
        }
        "INVALID_HEADER" => {
            "Configure a verified 42 student email so the official header can be inserted."
        }
        "HEADER_PROT_NAME" | "HEADER_PROT_NODEF" => {
            "Use one canonical filename-derived #ifndef/#define guard around the whole header."
        }
        "MISALIGNED_FUNC_DECL" => {
            "Align this prototype with the complete simple declaration group."
        }
        "MISALIGNED_VAR_DECL" => {
            "Align this declarator with the complete simple declaration group."
        }
        _ => {
            "Review this location and apply the named Norm rule manually; no semantics-preserving automatic edit was proven."
        }
    }
}

/// The unit a reported column is expressed in.
///
/// The two authorities disagree, and the disagreement is invisible until a line
/// is indented with tabs. The official Norminette counts display columns, so a
/// tab advances to the next four-column tab stop. A C compiler counts bytes, so
/// a tab is one column. Reading one convention as the other puts the caret on
/// the wrong character of every indented line, which is most lines of a 42
/// project.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ColumnUnit {
    /// Tab-expanded display column, as the official Norminette reports.
    Display,
    /// One-based byte offset within the physical line, as a C compiler reports.
    Byte,
}

fn diagnostic_range(source: &str, line: u32, column: u32, unit: ColumnUnit) -> TextRange {
    let start = offset_for_line_column(source, line, column, unit);
    let bytes = source.as_bytes();
    let mut end = start;
    if let Some(byte) = bytes.get(start) {
        if *byte == b'_' || byte.is_ascii_alphanumeric() {
            while bytes
                .get(end)
                .is_some_and(|byte| *byte == b'_' || byte.is_ascii_alphanumeric())
            {
                end += 1;
            }
        } else if !byte.is_ascii_whitespace() {
            end += 1;
            while end < source.len() && !source.is_char_boundary(end) {
                end += 1;
            }
        }
    }
    compact_range(start, end)
}

fn offset_for_line_column(source: &str, line: u32, column: u32, unit: ColumnUnit) -> usize {
    let target_line = line.max(1);
    let target_column = column.max(1);
    let mut current_line = 1_u32;
    let mut line_start = 0_usize;
    for (index, byte) in source.bytes().enumerate() {
        if current_line == target_line {
            break;
        }
        if byte == b'\n' {
            current_line = current_line.saturating_add(1);
            line_start = index + 1;
        }
    }
    if current_line != target_line {
        return source.len();
    }
    let line_end = source[line_start..]
        .find('\n')
        .map_or(source.len(), |offset| line_start + offset);
    if unit == ColumnUnit::Byte {
        let mut offset = line_start
            .saturating_add(target_column.saturating_sub(1) as usize)
            .min(line_end);
        // A compiler column can land mid-character only on a malformed report;
        // snapping forward keeps the range sliceable either way.
        while offset < line_end && !source.is_char_boundary(offset) {
            offset += 1;
        }
        return offset;
    }
    let mut column = 1_u32;
    for (offset, character) in source[line_start..line_end].char_indices() {
        if column >= target_column {
            return line_start + offset;
        }
        column = if character == '\t' {
            column.saturating_add(4 - ((column.saturating_sub(1)) % 4))
        } else {
            column.saturating_add(1)
        };
    }
    line_end
}

fn compact_range(start: usize, end: usize) -> TextRange {
    let start = u32::try_from(start).unwrap_or(u32::MAX);
    let end = u32::try_from(end).unwrap_or(u32::MAX).max(start);
    TextRange::new(TextSize::new(start), TextSize::new(end))
        .unwrap_or_else(|| TextRange::empty(TextSize::new(start)))
}

fn line_point_range(source: &str, line: u32) -> TextRange {
    compact_range(
        offset_for_line_column(source, line, 1, ColumnUnit::Display),
        offset_for_line_column(source, line, 1, ColumnUnit::Display),
    )
}

fn introduces_diagnostics(before: &[NorminetteDiagnostic], after: &[NorminetteDiagnostic]) -> bool {
    let counts = |items: &[NorminetteDiagnostic]| {
        let mut counts = BTreeMap::<String, usize>::new();
        for item in items {
            *counts.entry(item.rule_id.clone()).or_default() += 1;
        }
        counts
    };
    let before = counts(before);
    counts(after)
        .into_iter()
        .any(|(rule, count)| count > before.get(&rule).copied().unwrap_or_default())
}

/// Warns once when the run used a Norminette release this version has not been
/// verified against.
fn untested_norminette_diagnostic(
    oracle: &OracleContext,
    file: &DiscoveredFile,
    path: &Utf8PathBuf,
) -> Option<Diagnostic> {
    let fingerprint = oracle.oracle.fingerprint();
    if !fingerprint.untested || oracle.norminette_notice_path.as_path() != file.path.as_path() {
        return None;
    }
    Some(point_diagnostic(
        path,
        "NORMINETTE_VERSION_UNTESTED",
        Severity::Info,
        format!(
            "This run used Norminette {}, which this normfix release has not been verified against; {} is the supported version.",
            fingerprint.version,
            normfix_oracle::SUPPORTED_NORMINETTE_VERSION
        ),
        DiagnosticSource::NorminetteCompat(fingerprint.version.clone()),
        Some(
            "The before/after proof still compares two answers from this same checker, so a run cannot make its own result worse. What is not guaranteed is that the native rules agree with this release; review the diff."
                .to_owned(),
        ),
    ))
}

fn run_compiler_preflight(
    oracle: &OracleContext,
    options: &FixOptions,
    file: &DiscoveredFile,
    path: &Utf8PathBuf,
    original: &str,
    current: &str,
) -> Vec<Diagnostic> {
    if oracle.compiler.is_none() {
        if oracle.compiler_notice_path.as_deref() == Some(file.path.as_path()) {
            if let Some(reason) = &oracle.compiler_unavailable {
                return vec![point_diagnostic(
                    path,
                    "CC_PREFLIGHT_UNAVAILABLE",
                    if options.preflight {
                        Severity::Error
                    } else {
                        Severity::Info
                    },
                    format!(
                        "The {} C compiler preflight was skipped: {reason}",
                        if options.preflight {
                            "required"
                        } else {
                            "optional"
                        }
                    ),
                    DiagnosticSource::Compiler,
                    Some(
                        "Install `cc` or provide an exact compiler path; formatting and Norminette validation continued safely."
                            .to_owned(),
                    ),
                )];
            }
        }
        return Vec::new();
    }
    let mut diagnostics = Vec::new();
    if options.compiler_preflight || options.preflight {
        append_compiler_run(
            &mut diagnostics,
            oracle,
            file,
            path,
            original,
            current,
            false,
            options.preflight,
        );
    }
    if options.analyzer || options.preflight {
        append_compiler_run(
            &mut diagnostics,
            oracle,
            file,
            path,
            original,
            current,
            true,
            false,
        );
    }
    diagnostics
}

#[allow(clippy::too_many_arguments)]
fn append_compiler_run(
    diagnostics: &mut Vec<Diagnostic>,
    oracle: &OracleContext,
    file: &DiscoveredFile,
    path: &Utf8PathBuf,
    original: &str,
    current: &str,
    analyzer: bool,
    required: bool,
) {
    match oracle.compiler_preflight(&file.path, original, analyzer) {
        Ok(Some(report)) => diagnostics.extend(compiler_report_diagnostics(
            path, original, current, &report, analyzer, required,
        )),
        Ok(None) => {}
        Err(error) => diagnostics.push(point_diagnostic(
            path,
            if analyzer {
                "CC_ANALYZER_FAILED"
            } else {
                "CC_PREFLIGHT_FAILED"
            },
            if required {
                Severity::Error
            } else {
                Severity::Info
            },
            format!(
                "The {} could not inspect this translation unit: {error}",
                if analyzer {
                    "GCC analyzer"
                } else {
                    "C compiler preflight"
                }
            ),
            DiagnosticSource::Compiler,
            Some(if required {
                "Preflight is incomplete until this compiler failure is resolved; no source edit was authorized by it."
                    .to_owned()
            } else {
                "This operational failure is fail-open and did not authorize or reject any source edit."
                    .to_owned()
            }),
        )),
    }
}

#[allow(clippy::too_many_lines)]
fn compiler_report_diagnostics(
    path: &Utf8PathBuf,
    original: &str,
    current: &str,
    report: &CompilerReport,
    analyzer: bool,
    required: bool,
) -> Vec<Diagnostic> {
    static LOCATION: OnceLock<Regex> = OnceLock::new();
    let location = LOCATION.get_or_init(|| {
        Regex::new(
            r"^(?P<path>.*):(?P<line>[0-9]+):(?P<column>[0-9]+):[ \t]*(?P<level>fatal error|error|warning|note):[ \t]*(?P<message>.*)$",
        )
        .expect("constant compiler diagnostic regex")
    });
    let combined = report
        .stdout
        .lines()
        .chain(report.stderr.lines())
        .collect::<Vec<_>>();
    if analyzer
        && !report.accepted
        && combined.iter().any(|line| {
            line.contains("-fanalyzer")
                && (line.contains("unrecognized")
                    || line.contains("unknown argument")
                    || line.contains("unsupported"))
        })
    {
        return vec![point_diagnostic(
            path,
            "CC_ANALYZER_UNAVAILABLE",
            if required {
                Severity::Error
            } else {
                Severity::Info
            },
            "This compiler supports neither GCC `-fanalyzer` nor the Clang analyzer; deep analysis was skipped."
                .to_owned(),
            DiagnosticSource::Compiler,
            Some(
                "Point --cc at a real GCC or Clang, or omit --analyzer.".to_owned(),
            ),
        )];
    }
    if !report.accepted {
        if let Some(detail) = combined
            .iter()
            .map(|line| line.trim())
            .find(|line| compiler_configuration_is_incomplete(line))
        {
            return vec![point_diagnostic(
                path,
                if analyzer {
                    "CC_ANALYZER_CONFIGURATION_INCOMPLETE"
                } else {
                    "CC_PREFLIGHT_CONFIGURATION_INCOMPLETE"
                },
                if required {
                    Severity::Error
                } else {
                    Severity::Info
                },
                format!(
                    "The {} could not resolve the project compilation context: {detail}",
                    if analyzer {
                        "GCC analyzer"
                    } else {
                        "C compiler preflight"
                    }
                ),
                DiagnosticSource::Compiler,
                Some(
                    "normfix added stable -I entries for every discovered project header directory, but deliberately did not infer -D macros, SDK paths, language modes, or execute Make recipes; formatting continued without using this incomplete result."
                        .to_owned(),
                ),
            )];
        }
    }
    let mut diagnostics = combined
        .iter()
        .filter_map(|line| location.captures(line.trim()))
        .map(|captures| {
            let line = captures
                .name("line")
                .and_then(|value| value.as_str().parse::<u32>().ok())
                .unwrap_or(1);
            let column = captures
                .name("column")
                .and_then(|value| value.as_str().parse::<u32>().ok())
                .unwrap_or(1);
            let level = captures
                .name("level")
                .map_or("warning", |value| value.as_str());
            let raw_message = captures
                .name("message")
                .map_or("C compiler diagnostic", |value| value.as_str());
            let (message, warning_name) = compiler_warning_name(raw_message);
            let compiler_path = captures
                .name("path")
                .map_or("", |value| value.as_str());
            let local_location = compiler_path_matches(compiler_path, path.as_str());
            let range = if local_location {
                remap_compiler_range(original, current, line, column)
            } else {
                TextRange::empty(TextSize::new(0))
            };
            let mut notes = vec![
                "Compiler diagnostics inspect the original on-disk translation unit and never authorize or reject formatter edits."
                    .to_owned(),
            ];
            if !local_location {
                notes.push(format!(
                    "Compiler location: {compiler_path}:{line}:{column} (usually an included header)."
                ));
            }
            Diagnostic {
                rule_id: if analyzer {
                    warning_name.map_or_else(
                        || "CC_ANALYZER".to_owned(),
                        |name| {
                            let normalized = normalize_warning_name(name);
                            let normalized = normalized
                                .strip_prefix("ANALYZER_")
                                .unwrap_or(&normalized);
                            format!("CC_ANALYZER_{normalized}")
                        },
                    )
                } else {
                    warning_name.map_or_else(
                        || "CC_STRICT".to_owned(),
                        |name| format!("CC_{}", normalize_warning_name(name)),
                    )
                },
                path: path.clone(),
                range,
                severity: if analyzer || level == "note" {
                    Severity::Info
                } else if level.contains("error") {
                    Severity::Error
                } else {
                    Severity::Warning
                },
                message: message.to_owned(),
                source: DiagnosticSource::Compiler,
                notes,
                help: Some(if analyzer {
                    "Review the analyzer path trace; ownership and control-flow findings are never auto-fixed."
                        .to_owned()
                } else {
                    "Fix this strict -Wall/-Wextra/-Werror compiler diagnostic, then rerun normfix."
                        .to_owned()
                }),
            }
        })
        .collect::<Vec<_>>();
    if analyzer {
        deduplicate_analyzer_trace(&mut diagnostics);
    }
    if diagnostics.is_empty() && !report.accepted {
        let detail = report
            .stderr
            .lines()
            .chain(report.stdout.lines())
            .find(|line| !line.trim().is_empty())
            .unwrap_or("the compiler returned a nonzero status without a parseable diagnostic");
        diagnostics.push(point_diagnostic(
            path,
            if analyzer {
                "CC_ANALYZER_REJECTED"
            } else {
                "CC_STRICT_REJECTED"
            },
            if analyzer {
                Severity::Info
            } else {
                Severity::Warning
            },
            detail.to_owned(),
            DiagnosticSource::Compiler,
            Some(
                "Inspect the compiler output directly; no formatter decision depended on this preflight."
                    .to_owned(),
            ),
        ));
    }
    diagnostics
}

fn compiler_configuration_is_incomplete(line: &str) -> bool {
    let lowercase = line.to_ascii_lowercase();
    let missing_input = lowercase.contains("no such file or directory")
        || lowercase.contains("file not found")
        || lowercase.contains("cannot find")
        || lowercase.contains("could not find");
    missing_input
        && (lowercase.contains("fatal error:") || lowercase.contains("cannot open include file"))
}

fn compiler_warning_name(message: &str) -> (&str, Option<&str>) {
    let Some(open) = message.rfind(" [-W") else {
        return (message, None);
    };
    let Some(suffix) = message
        .get(open + 2..)
        .and_then(|tail| tail.strip_suffix(']'))
    else {
        return (message, None);
    };
    let parts = suffix.split(',').map(str::trim).collect::<Vec<_>>();
    let warning = parts
        .iter()
        .find_map(|part| part.strip_prefix("-Werror="))
        .or_else(|| {
            parts.iter().find_map(|part| {
                part.strip_prefix("-W")
                    .filter(|name| *name != "error" && !name.starts_with("error="))
            })
        });
    (message[..open].trim_end(), warning)
}

fn normalize_warning_name(name: &str) -> String {
    name.trim_start_matches("error=")
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect()
}

fn compiler_path_matches(compiler_path: &str, report_path: &str) -> bool {
    compiler_path == report_path || compiler_path.ends_with(&format!("/{report_path}"))
}

fn remap_compiler_range(original: &str, current: &str, line: u32, column: u32) -> TextRange {
    if original == current {
        return diagnostic_range(current, line, column, ColumnUnit::Byte);
    }
    let Some(original_line) = original.lines().nth(line.saturating_sub(1) as usize) else {
        return TextRange::empty(TextSize::new(0));
    };
    let mut matches = current
        .lines()
        .enumerate()
        .filter_map(|(index, candidate)| (candidate == original_line).then_some(index + 1));
    let Some(mapped) = matches.next() else {
        return TextRange::empty(TextSize::new(0));
    };
    if matches.next().is_some() {
        return TextRange::empty(TextSize::new(0));
    }
    diagnostic_range(
        current,
        u32::try_from(mapped).unwrap_or(u32::MAX),
        column,
        ColumnUnit::Byte,
    )
}

fn parser_diagnostics(path: &Utf8PathBuf, source: &str) -> Vec<Diagnostic> {
    let mut parser = match CParser::new() {
        Ok(parser) => parser,
        Err(error) => {
            return vec![point_diagnostic(
                path,
                "C_PARSER_FAILURE",
                Severity::Error,
                error.to_string(),
                DiagnosticSource::Parser,
                Some("Repair the source syntax before running automatic fixes.".to_owned()),
            )];
        }
    };
    match parser.parse(source) {
        Ok(parsed) => parsed
            .issues()
            .iter()
            .map(|issue| {
                let va_arg_compatibility = recovery_is_inside_va_arg(source, issue.range());
                Diagnostic {
                    rule_id: if va_arg_compatibility {
                        "C_PARSER_VA_ARG_COMPAT"
                    } else {
                        "C_SYNTAX_RECOVERY"
                    }
                    .to_owned(),
                    path: path.clone(),
                    range: issue.range(),
                    severity: if va_arg_compatibility {
                        Severity::Info
                    } else {
                        Severity::Warning
                    },
                    message: if va_arg_compatibility {
                        "The native parser preserved a raw `va_arg` type argument through its compatibility path."
                            .to_owned()
                    } else {
                        format!(
                            "The C parser recovered around syntax node `{}`.",
                            issue.syntax_kind()
                        )
                    },
                    source: DiagnosticSource::Parser,
                    notes: vec![
                        if va_arg_compatibility {
                            "This is a tree-sitter-c grammar limitation; the source bytes and official Norminette result remain authoritative."
                        } else {
                            "Automatic syntax-aware edits were disabled for this file."
                        }
                        .to_owned(),
                    ],
                    help: Some(
                        if va_arg_compatibility {
                            "No source change is required; native syntax-aware edits remain disabled for this file."
                        } else {
                            "Repair the malformed or unsupported construct, then rerun normfix."
                        }
                        .to_owned(),
                    ),
                }
            })
            .collect(),
        Err(error) => vec![point_diagnostic(
            path,
            "C_PARSER_FAILURE",
            Severity::Error,
            error.to_string(),
            DiagnosticSource::Parser,
            Some("Repair the source syntax before running automatic fixes.".to_owned()),
        )],
    }
}

fn recovery_is_inside_va_arg(source: &str, range: TextRange) -> bool {
    let Ok(start) = usize::try_from(range.start().get()) else {
        return false;
    };
    if start > source.len() || !source.is_char_boundary(start) {
        return false;
    }
    let line_start = source[..start].rfind('\n').map_or(0, |newline| newline + 1);
    let line_end = source[start..]
        .find('\n')
        .map_or(source.len(), |newline| start + newline);
    source[line_start..line_end]
        .find("va_arg")
        .is_some_and(|offset| {
            source[line_start + offset + "va_arg".len()..line_end]
                .trim_start()
                .starts_with('(')
        })
}

fn point_diagnostic(
    path: &Utf8PathBuf,
    rule_id: &str,
    severity: Severity,
    message: String,
    source: DiagnosticSource,
    help: Option<String>,
) -> Diagnostic {
    Diagnostic {
        rule_id: rule_id.to_owned(),
        path: path.clone(),
        range: TextRange::empty(TextSize::new(0)),
        severity,
        message,
        source,
        notes: Vec::new(),
        help,
    }
}

fn explain_constant_array_false_positives(
    path: &Utf8PathBuf,
    source: &str,
    diagnostics: &mut Vec<NorminetteDiagnostic>,
    norminette_version: &str,
) -> Vec<Diagnostic> {
    if !diagnostics
        .iter()
        .any(|diagnostic| diagnostic.rule_id == "VLA_FORBIDDEN")
    {
        return Vec::new();
    }
    let Ok(mut parser) = CParser::new() else {
        return Vec::new();
    };
    let Ok(parsed) = parser.parse(source) else {
        return Vec::new();
    };
    if parsed.has_syntax_errors() {
        return Vec::new();
    }
    let semantic = analyze_semantics(&parsed);
    let mut advisories = Vec::new();
    diagnostics.retain(|diagnostic| {
        if diagnostic.rule_id != "VLA_FORBIDDEN" {
            return true;
        }
        let offset = u32::try_from(offset_for_line_column(
            source,
            diagnostic.line,
            diagnostic.column,
            ColumnUnit::Display,
        ))
        .map_or(TextSize::new(u32::MAX), TextSize::new);
        let constant = semantic.arrays.iter().find(|array| {
            array.range.contains(offset) && matches!(array.kind, ArrayBoundKind::Constant(_))
        });
        let Some(array) = constant else {
            return true;
        };
        let ArrayBoundKind::Constant(value) = array.kind else {
            return true;
        };
        advisories.push(Diagnostic {
            rule_id: "VLA_COMPAT_FALSE_POSITIVE".to_owned(),
            path: path.clone(),
            range: array.bound_range.unwrap_or(array.range),
            severity: Severity::Info,
            message: format!(
                "Norminette reported a VLA, but `{}` resolves to the integer constant {value}.",
                array.expression.as_deref().unwrap_or("this bound")
            ),
            source: DiagnosticSource::NorminetteCompat(norminette_version.to_owned()),
            notes: vec![
                "The native enum evaluator proved this bound within the current translation unit."
                    .to_owned(),
            ],
            help: Some(
                "No code change is required; keep the enum definition visible before this array."
                    .to_owned(),
            ),
        });
        false
    });
    advisories
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{BTreeMap, BTreeSet},
        fs,
    };

    use tempfile::TempDir;

    use normfix_core::FixRecord;
    use normfix_makefile::SourcePathStatus;

    use super::{
        COMPILER_FINGERPRINT_FILE_LIMIT, MAKEFILE_TRIVIA_PROBE_LIMIT, PlannedFile,
        compiler_project_context, dependent_destructive_bundle_is_partial, load_destructive_source,
        makefile_source_path, makefile_source_probe, prelude_entry, prepare_external_recovery_root,
        transaction_root,
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

    #[test]
    fn each_authority_column_convention_lands_on_the_same_character() {
        use super::{ColumnUnit, offset_for_line_column};

        // Two tabs then the call: the shape of almost every 42 statement.
        let source = "int\tmain(void)\n{\n\t\tsort_medium(ctx);\n}\n";
        let call = source.find("sort_medium").expect("the call");

        // A C compiler counts bytes, so the call starts at column 3.
        assert_eq!(
            offset_for_line_column(source, 3, 3, ColumnUnit::Byte),
            call,
            "a compiler column must be read as a byte offset"
        );
        // The official Norminette counts display columns, so two four-column
        // tab stops put the same character at column 9.
        assert_eq!(
            offset_for_line_column(source, 3, 9, ColumnUnit::Display),
            call,
            "a Norminette column must be read as a tab-expanded display column"
        );
        // Reading a compiler column as a display column is the bug this guards:
        // it stops inside the indentation instead of on the call.
        assert_ne!(
            offset_for_line_column(source, 3, 3, ColumnUnit::Display),
            call
        );
    }

    #[test]
    fn a_byte_column_past_the_line_or_inside_a_character_stays_sliceable() {
        use super::{ColumnUnit, offset_for_line_column};

        let source = "\tchar\t*s = \"caf\u{e9}\"; boom(s);\n";
        // Clang counts both bytes of `é`, so the reported column is the byte
        // offset plus one, not the character count.
        let boom = source.find("boom").expect("the call");
        let reported = u32::try_from(boom).expect("fits") + 1;
        assert_eq!(
            offset_for_line_column(source, 1, reported, ColumnUnit::Byte),
            boom
        );
        assert!(reported > u32::try_from(source[..boom].chars().count()).expect("fits"));

        // A column past the end clamps to the line end rather than running on
        // into the next line.
        let line_end = source.find('\n').expect("the newline");
        assert_eq!(
            offset_for_line_column(source, 1, 9_999, ColumnUnit::Byte),
            line_end
        );

        // A column landing mid-character snaps forward to a boundary, so the
        // range can still be sliced.
        let accent = source.find('\u{e9}').expect("the accent");
        let offset = offset_for_line_column(
            source,
            1,
            u32::try_from(accent).expect("fits") + 2,
            ColumnUnit::Byte,
        );
        assert!(source.is_char_boundary(offset));
    }

    #[test]
    fn a_compiler_is_classified_by_its_banner_not_its_command_name() {
        use super::CompilerFamily;

        // /usr/bin/gcc on macOS answers this, and sending it -fanalyzer fails.
        assert_eq!(
            CompilerFamily::from_version("Apple clang version 17.0.0 (clang-1700.6.4.2)"),
            CompilerFamily::Clang
        );
        assert_eq!(
            CompilerFamily::from_version("gcc (Homebrew GCC 14.2.0) 14.2.0"),
            CompilerFamily::Gcc
        );
        assert_eq!(
            CompilerFamily::from_version("cc (Free Software Foundation) 13"),
            CompilerFamily::Gcc
        );
        assert_eq!(
            CompilerFamily::from_version("tcc version 0.9.27"),
            CompilerFamily::Unknown
        );
    }

    #[test]
    fn each_family_gets_the_analyzer_flags_it_understands() {
        use super::{CompilerFamily, compiler_arguments};

        let clang = compiler_arguments(true, CompilerFamily::Clang, &[]);
        assert!(clang.iter().any(|flag| flag == "--analyze"));
        // Combining the two makes Clang ignore the analyzer entirely.
        assert!(!clang.iter().any(|flag| flag == "-fsyntax-only"));

        let gcc = compiler_arguments(true, CompilerFamily::Gcc, &[]);
        assert!(gcc.iter().any(|flag| flag == "-fanalyzer"));
        assert!(gcc.iter().any(|flag| flag == "-fsyntax-only"));

        // The strict preflight is the same for everyone and keeps -Werror.
        for family in [CompilerFamily::Clang, CompilerFamily::Gcc] {
            let strict = compiler_arguments(false, family, &[]);
            assert!(strict.iter().any(|flag| flag == "-Werror"));
            assert!(!strict.iter().any(|flag| flag == "--analyze"));
            assert!(!strict.iter().any(|flag| flag == "-fanalyzer"));
        }
    }

    #[test]
    fn oversized_unexpected_file_disables_the_compiler_cache_fingerprint() {
        let project = TempDir::new().expect("project");
        fs::write(
            project.path().join("main.c"),
            "int\tmain(void)\n{\n\treturn (0);\n}\n",
        )
        .expect("source");
        let unexpected =
            fs::File::create(project.path().join("recording.bin")).expect("unexpected file");
        unexpected
            .set_len(COMPILER_FINGERPRINT_FILE_LIMIT + 1)
            .expect("sparse oversized unexpected file");

        let context = compiler_project_context(project.path());

        assert!(context.fingerprint.is_none());
    }

    #[test]
    fn oversized_makefile_source_is_not_classified_as_trivia_only() {
        let project = TempDir::new().expect("project");
        let makefile = project.path().join("Makefile");
        fs::write(&makefile, "SRCS = generated.c\n").expect("Makefile");
        let source = fs::File::create(project.path().join("generated.c")).expect("source");
        source
            .set_len(MAKEFILE_TRIVIA_PROBE_LIMIT + 1)
            .expect("sparse oversized source");

        let (status, proof) = makefile_source_probe(&makefile, project.path(), "generated.c");

        assert_eq!(status, SourcePathStatus::Unknown);
        assert!(proof.is_none());
    }

    #[test]
    fn makefile_source_paths_stay_in_the_caller_path_vocabulary() {
        let project = TempDir::new().expect("project");
        let makefile = project.path().join("Makefile");
        fs::write(&makefile, "SRC = main.c\n").expect("makefile");

        let resolved = makefile_source_path(&makefile, project.path(), "main.c")
            .expect("a literal source inside the project resolves");

        // Returning the canonical form here mixed two path vocabularies, so the
        // common transaction ancestor collapsed to `/` on macOS, where /var is
        // a symbolic link, and the write was refused.
        assert!(resolved.starts_with(project.path()), "{resolved:?}");
        assert_eq!(resolved.file_name().expect("file name"), "main.c");
    }

    #[test]
    fn makefile_source_paths_outside_the_project_are_refused() {
        let project = TempDir::new().expect("project");
        let makefile = project.path().join("Makefile");
        fs::write(&makefile, "SRC = main.c\n").expect("makefile");

        assert!(makefile_source_path(&makefile, project.path(), "../escape.c").is_none());
        assert!(makefile_source_path(&makefile, project.path(), "/etc/passwd").is_none());
        assert!(makefile_source_path(&makefile, project.path(), "nested/../../out.c").is_none());
    }

    #[test]
    fn transaction_root_is_the_common_ancestor_and_falls_back_to_the_cwd() {
        let cwd = std::path::Path::new("/project");
        let inside = [
            std::path::Path::new("/project/src/main.c"),
            std::path::Path::new("/project/src/util.c"),
        ];
        assert_eq!(
            transaction_root(inside.iter().copied(), cwd),
            std::path::Path::new("/project/src")
        );

        let disjoint = [
            std::path::Path::new("/project/main.c"),
            std::path::Path::new("/elsewhere/main.c"),
        ];
        assert_eq!(
            transaction_root(disjoint.iter().copied(), cwd),
            std::path::Path::new("/")
        );
    }

    #[test]
    fn recovery_storage_refuses_any_overlap_with_the_project() {
        let project = TempDir::new().expect("project");
        let canonical = project.path().canonicalize().expect("canonical project");

        // Inside the project.
        assert!(
            prepare_external_recovery_root(&project.path().join("quarantine"), &canonical).is_err()
        );
        // An ancestor of the project.
        assert!(
            prepare_external_recovery_root(project.path().parent().expect("parent"), &canonical)
                .is_err()
        );
        // The project itself.
        assert!(prepare_external_recovery_root(project.path(), &canonical).is_err());
    }

    #[test]
    fn recovery_storage_creates_every_missing_level_and_is_repeatable() {
        let project = TempDir::new().expect("project");
        let storage = TempDir::new().expect("storage");
        let canonical = project.path().canonicalize().expect("canonical project");
        let requested = storage.path().join("normfix").join("quarantine");

        let created = prepare_external_recovery_root(&requested, &canonical).expect("first");
        assert!(created.is_dir());

        let again = prepare_external_recovery_root(&requested, &canonical).expect("second");
        assert_eq!(created, again);
    }

    #[cfg(unix)]
    #[test]
    fn recovery_storage_refuses_a_symbolic_link_ancestor() {
        use std::os::unix::fs::symlink;

        let project = TempDir::new().expect("project");
        let storage = TempDir::new().expect("storage");
        let canonical = project.path().canonicalize().expect("canonical project");
        let real = storage.path().join("real");
        let link = storage.path().join("link");
        fs::create_dir(&real).expect("real directory");
        symlink(&real, &link).expect("symbolic link");

        assert!(prepare_external_recovery_root(&link.join("quarantine"), &canonical).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn recovery_storage_refuses_a_link_that_redirects_into_the_project() {
        use std::os::unix::fs::symlink;

        let project = TempDir::new().expect("project");
        let storage = TempDir::new().expect("storage");
        let canonical = project.path().canonicalize().expect("canonical project");
        let inside = project.path().join("backups");
        let link = storage.path().join("link");
        fs::create_dir(&inside).expect("directory inside the project");
        symlink(&inside, &link).expect("symbolic link into the project");

        // The lexical prefix check passes; only resolving the link exposes the
        // overlap, which is why the check runs again after canonicalization.
        assert!(prepare_external_recovery_root(&link, &canonical).is_err());
    }
}
