//! End-to-end native fix pipeline.
//!
//! Analysis and formatting happen in immutable shadow buffers. The only write
//! boundary is the validated multi-file transaction in `normfix-actions`.

mod commit;
mod compiler;
mod destructive;
mod diagnostics;
mod file_processing;
mod makefile;
mod markdown;
mod paths;
mod policy;
mod preflight;
mod quarantine;
mod source_io;

use commit::{commit_work, dependent_destructive_bundle_paths};
use compiler::{
    CompilerFamily, CompilerProjectContext, compiler_arguments, compiler_project_context,
};
use destructive::plan_destructive_preludes;
use diagnostics::{line_for_offset, text_range};
use file_processing::{failed_source, process_file};
use policy::{append_function_policy_diagnostics, plan_policy_diagnostics};
use preflight::append_preflight_diagnostics;
use quarantine::quarantine_unexpected_files;
use source_io::project_file_matches;

use paths::{absolute_lexical, report_identity, report_path};

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use camino::Utf8PathBuf;
use normfix_actions::{PlannedFile, ReadPrecondition};
use normfix_cache::{CacheKey, CachePaths, PersistentCache, PreparedCacheEntry};
use normfix_core::{Diagnostic, DiagnosticSource, FixRecord, Severity};
use normfix_destructive::DestructiveAuthorization;
use normfix_header::{Identity42, RunClock};
use normfix_oracle::{
    ClangTidy, ClangTidyConfig, CompilerConfig, CompilerError, CompilerReport, CompilerValidator,
    NorminetteConfig, NorminetteError, NorminetteOracle, NorminetteReport, ProcessLimits,
};
use normfix_project::{
    DiscoveredFile, DiscoveryOptions, ProjectFileKind, ProjectPolicy, discover,
    plan_guard_insertions, plan_guard_renames,
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
    /// Language for the diagnostics this project authors.
    ///
    /// Findings from the official checker and the C compiler are unaffected:
    /// that text is those tools' own output.
    pub locale: normfix_i18n::Locale,
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
    /// Exact `clang-tidy`, or `None` to search `PATH` for one.
    pub clang_tidy_executable: Option<PathBuf>,
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
    /// Explicitly remove locals the compiler proved unused, when the
    /// declaration carries nothing that runs.
    pub remove_unused_variables: bool,
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
            locale: normfix_i18n::Locale::English,
            identity: None,
            identity_source: "No verified 42 student email was found.".to_owned(),
            backup: BackupPolicy::Automatic,
            norminette_executable: None,
            compiler_preflight: true,
            compiler_executable: None,
            clang_tidy_executable: None,
            analyzer: false,
            strict_norminette_version: false,
            timeout: Duration::from_secs(5),
            cache: true,
            remove_invalid_comments: false,
            remove_unused_variables: false,
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
    clang_tidy: Option<ClangTidy>,
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
        let before_matches = project_file_matches(path, source.as_bytes()).map_err(|error| {
            CompilerError::InvalidProjectSource(format!(
                "could not re-read `{}` before compiler preflight: {error}",
                path.display()
            ))
        })?;
        if !before_matches {
            return Err(CompilerError::InvalidProjectSource(format!(
                "source `{}` changed before compiler preflight",
                path.display()
            )));
        }
        let report = compiler.validate_project_file(&self.project_root, path, &arguments)?;
        let after_matches = project_file_matches(path, source.as_bytes()).map_err(|error| {
            CompilerError::InvalidProjectSource(format!(
                "could not re-read `{}` after compiler preflight: {error}",
                path.display()
            ))
        })?;
        if !after_matches {
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
        // A lens, never a dependency: a machine without it runs as before, and
        // the run never waits on one that cannot answer for itself.
        clang_tidy: if has_c_source && options.preflight {
            ClangTidy::locate(ClangTidyConfig {
                executable: options.clang_tidy_executable.clone(),
                limits: ProcessLimits {
                    timeout: options.timeout.max(Duration::from_secs(30)),
                    output_bytes: 2 * 1024 * 1024,
                },
            })
            .ok()
        } else {
            None
        },
        cache,
        project_root: absolute_lexical(&options.cwd),
    }))
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
        localized: None,
    }));
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, fs};

    use tempfile::TempDir;

    use super::destructive::{load_destructive_source, prelude_entry};

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
