//! End-to-end native fix pipeline.
//!
//! Analysis and formatting happen in immutable shadow buffers. The only write
//! boundary is the validated multi-file transaction in `normfix-actions`.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::OpenOptions;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use camino::Utf8PathBuf;
use normfix_actions::{PlannedFile, TransactionOptions, commit_files};
use normfix_c_actions::{
    CActionError, CActionOptions, ReportedDiagnostic, analyze_c, apply_c_actions,
};
use normfix_c_semantics::{ArrayBoundKind, analyze as analyze_semantics};
use normfix_c_syntax::CParser;
use normfix_cache::{CacheKey, CachePaths, PersistentCache, PreparedCacheEntry};
use normfix_core::{
    Diagnostic, DiagnosticSource, FileId, FixRecord, Severity, SourceSnapshot, TextRange, TextSize,
    apply_source_edits,
};
use normfix_destructive::{
    ClosedCSourceSet, DestructiveAuthorization, QuarantineItem, QuarantineRequest,
    StaticRemovalPlan, plan_quarantine, plan_unused_static_functions,
};
use normfix_header::{
    ByteRange, Identity42, RunClock, c_header_filename_matches, ensure_c_header, update_c_header,
};
use normfix_makefile::{analyze_makefile, format_makefile};
use normfix_markdown::analyze_markdown;
use normfix_oracle::{
    NorminetteConfig, NorminetteDiagnostic, NorminetteError, NorminetteOracle, NorminetteReport,
    ProcessLimits,
};
use normfix_project::{
    DiscoveredFile, DiscoveryOptions, GuardApproval, ProjectFileKind, discover,
    guard_approval_is_current, plan_guard_renames,
};
use normfix_report::{FileReport, ReportIdentity, ReportMode, RunReport};
use rayon::ThreadPoolBuilder;
use rayon::prelude::*;
use thiserror::Error;

/// Backup behavior for one fixing run.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BackupPolicy {
    /// Use the platform's external norminette-fix data directory.
    Automatic,
    /// Use this external directory as the backup base.
    Directory(PathBuf),
    /// Do not retain original copies for ordinary formatting edits.
    Disabled,
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
    /// Per-file official-tool timeout.
    pub timeout: Duration,
    /// Enable the external content-addressed cache.
    pub cache: bool,
    /// Explicitly remove only comments rejected at exact official locations.
    pub remove_invalid_comments: bool,
    /// Remove only unreachable `static` functions under explicit authorization.
    pub remove_unused_static: bool,
    /// Quarantine unexpected files under explicit authorization.
    pub quarantine_unexpected: bool,
    /// Capability-scoped grant for destructive operations.
    pub destructive_authorization: Option<DestructiveAuthorization>,
    /// Opt in to canonical `CommonMark` formatting of README files.
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
            respect_gitignore: false,
            threads: None,
            identity: None,
            identity_source: "No verified 42 student email was found.".to_owned(),
            backup: BackupPolicy::Automatic,
            norminette_executable: None,
            timeout: Duration::from_secs(5),
            cache: true,
            remove_invalid_comments: false,
            remove_unused_static: false,
            quarantine_unexpected: false,
            destructive_authorization: None,
            format_markdown: false,
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
            "norminette-3.3.59",
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
}

struct FileWork {
    absolute_path: PathBuf,
    report: FileReport,
    plan: Option<PlannedFile>,
}

#[derive(Clone, Debug)]
struct DestructivePrelude {
    source: String,
    fixes: Vec<FixRecord>,
    diagnostics: Vec<Diagnostic>,
}

struct ClosedSourcePreparationError {
    path: PathBuf,
    message: String,
    help: &'static str,
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
pub fn run_fixes(inputs: &[PathBuf], options: &FixOptions) -> Result<RunReport, FixRunError> {
    let started = Instant::now();
    if options.threads == Some(0) {
        return Err(FixRunError::ZeroThreads);
    }
    let clock = RunClock::from_process_environment()
        .map_err(|error| FixRunError::Clock(error.to_string()))?;
    let discovery_options =
        DiscoveryOptions::new(&options.cwd).with_respect_gitignore(options.respect_gitignore);
    let discovery = discover(inputs, &discovery_options);
    let oracle = build_oracle_context(options)?;

    let header_paths = discovery
        .processable_files
        .iter()
        .filter(|file| file.kind == ProjectFileKind::CHeader)
        .map(|file| file.path.clone())
        .collect::<Vec<_>>();
    let (guard_approvals, guard_failure) = match plan_guard_renames(&header_paths) {
        Ok(approvals) => (approvals, None),
        Err(error) => (BTreeMap::new(), Some(error.to_string())),
    };
    let destructive_preludes =
        plan_destructive_preludes(inputs, &discovery.processable_files, options);

    let execute = || {
        discovery
            .processable_files
            .par_iter()
            .map(|file| {
                process_file(
                    file,
                    options,
                    &clock,
                    &oracle,
                    &guard_approvals,
                    guard_failure.as_deref(),
                    destructive_preludes.get(&file.path),
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

    let commit_succeeded = options.mode != ReportMode::Fix || commit_work(&mut work, options);
    let quarantine_candidates = if options.quarantine_unexpected {
        discovery
            .unexpected_files
            .iter()
            .filter_map(|path| report_path(path, &options.cwd).ok())
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let (quarantined, quarantine_errors) =
        if options.mode == ReportMode::Fix && options.quarantine_unexpected && commit_succeeded {
            quarantine_unexpected_files(&discovery.unexpected_files, options)
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
    let unexpected_files = discovery
        .unexpected_files
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
    Ok(report)
}

fn build_oracle_context(options: &FixOptions) -> Result<OracleContext, FixRunError> {
    let oracle = NorminetteOracle::locate(NorminetteConfig {
        executable: options.norminette_executable.clone(),
        expected_version: normfix_oracle::SUPPORTED_NORMINETTE_VERSION.to_owned(),
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
    Ok(OracleContext {
        oracle,
        cache,
        project_root: absolute_lexical(&options.cwd),
    })
}

fn plan_destructive_preludes(
    _inputs: &[PathBuf],
    selected: &[DiscoveredFile],
    options: &FixOptions,
) -> BTreeMap<PathBuf, DestructivePrelude> {
    if !options.remove_unused_static {
        return BTreeMap::new();
    }
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
            .with_respect_norminetteignore(false),
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
    if !complete.errors.is_empty() || selected_c != complete_c {
        attach_destructive_diagnostic(
            &mut preludes,
            selected_c.iter().next().expect("non-empty selected set"),
            &options.cwd,
            "UNSAFE_STATIC_CLOSED_SET_INCOMPLETE",
            "Unused static functions were not removed because the selected inputs are not the complete project C/header set.",
            "Run from the project root without partial paths or ignored C files.",
        );
        return preludes;
    }
    let Some(authorization) = options.destructive_authorization.as_ref() else {
        attach_destructive_diagnostic(
            &mut preludes,
            selected_c.iter().next().expect("non-empty selected set"),
            &options.cwd,
            "UNSAFE_AUTHORIZATION_REQUIRED",
            "Unused static functions were not removed because destructive authorization is missing.",
            "Confirm the terminal warning or pass --unsafe --force.",
        );
        return preludes;
    };

    let closed = match prepare_closed_source_set(&selected_c, &options.cwd) {
        Ok(closed) => closed,
        Err(error) => {
            attach_destructive_diagnostic(
                &mut preludes,
                &error.path,
                &options.cwd,
                "UNSAFE_STATIC_CLOSED_SET_INVALID",
                &error.message,
                error.help,
            );
            return preludes;
        }
    };
    let plan = match plan_unused_static_functions(&closed, authorization) {
        Ok(plan) => plan,
        Err(error) => {
            attach_destructive_diagnostic(
                &mut preludes,
                selected_c.iter().next().expect("non-empty selected set"),
                &options.cwd,
                "UNSAFE_STATIC_PLAN_FAILED",
                &format!("Unused static function planning failed: {error}"),
                "Review the closed project inputs and destructive authorization.",
            );
            return preludes;
        }
    };
    apply_static_removal_plan(&mut preludes, plan, &options.cwd);
    preludes
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

fn apply_static_removal_plan(
    preludes: &mut BTreeMap<PathBuf, DestructivePrelude>,
    plan: StaticRemovalPlan,
    cwd: &Path,
) {
    for diagnostic in plan.diagnostics {
        let absolute = cwd.join(diagnostic.path.as_std_path());
        let prelude = prelude_entry(preludes, &absolute);
        prelude.diagnostics.push(diagnostic);
    }
    for file_plan in plan.files {
        let absolute = cwd.join(file_plan.path.as_std_path());
        let prelude = prelude_entry(preludes, &absolute);
        if blake3::hash(prelude.source.as_bytes()).to_hex().to_string() != file_plan.original_hash {
            prelude.diagnostics.push(project_diagnostic(
                file_plan.path,
                "UNSAFE_STATIC_SNAPSHOT_CHANGED",
                "The source changed after destructive planning; no function was removed.",
                "Retry the command against an unchanged project.",
            ));
            continue;
        }
        match apply_source_edits(&prelude.source, &file_plan.edits) {
            Ok(source) => {
                prelude.source = source;
                prelude.fixes.extend(file_plan.fixes);
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

fn prelude_entry<'a>(
    preludes: &'a mut BTreeMap<PathBuf, DestructivePrelude>,
    absolute: &Path,
) -> &'a mut DestructivePrelude {
    preludes
        .entry(absolute.to_path_buf())
        .or_insert_with(|| DestructivePrelude {
            source: std::fs::read_to_string(absolute).unwrap_or_default(),
            fixes: Vec::new(),
            diagnostics: Vec::new(),
        })
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

fn process_file(
    file: &DiscoveredFile,
    options: &FixOptions,
    clock: &RunClock,
    oracle: &OracleContext,
    guard_approvals: &BTreeMap<PathBuf, GuardApproval>,
    guard_failure: Option<&str>,
    destructive_prelude: Option<&DestructivePrelude>,
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
        ProjectFileKind::CSource | ProjectFileKind::CHeader => process_c(
            file,
            relative,
            original,
            source,
            options,
            clock,
            oracle,
            guard_approvals,
            guard_failure,
            destructive_prelude,
        ),
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
    }
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
    guard_failure: Option<&str>,
    destructive_prelude: Option<&DestructivePrelude>,
) -> FileWork {
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
    let before = official_diagnostics(&path, &original, &before_report.diagnostics);
    let mut current =
        destructive_prelude.map_or_else(|| original.clone(), |prelude| prelude.source.clone());
    let mut fixes = destructive_prelude.map_or_else(Vec::new, |prelude| prelude.fixes.clone());
    let mut local_diagnostics =
        destructive_prelude.map_or_else(Vec::new, |prelude| prelude.diagnostics.clone());

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
    let reported = baseline_report
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
        .collect::<Vec<_>>();
    let action_options = CActionOptions {
        max_columns: options.max_columns,
        max_passes: options.max_passes,
        remove_invalid_comments: options.remove_invalid_comments,
        format_proven_declarations: true,
    };
    let mut action_changed = false;
    match apply_c_actions(path.as_path(), &current, &reported, &action_options) {
        Ok(result) => {
            action_changed = result.source != current;
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
        }
    }

    let should_update_header = !header_inserted
        && (guard_changed || action_changed || !c_header_filename_matches(&current, filename));
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
                normfix_oracle::SUPPORTED_NORMINETTE_VERSION.to_owned(),
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
    let semantic_advisories =
        explain_constant_array_false_positives(&path, &current, &mut remaining_official);
    let mut after = match analyze_c(path.as_path(), &current, options.max_columns) {
        Ok(diagnostics) => diagnostics,
        Err(_) => parser_diagnostics(&path, &current),
    };
    let native_rules = after
        .iter()
        .map(|diagnostic| diagnostic.rule_id.clone())
        .collect::<BTreeSet<_>>();
    after.extend(
        official_diagnostics(&path, &current, &remaining_official)
            .into_iter()
            .filter(|diagnostic| !native_rules.contains(&diagnostic.rule_id)),
    );
    after.extend(semantic_advisories);
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
    let filename = file
        .path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Makefile");
    let formatted = format_makefile(&original, filename, options.identity.as_ref(), clock);
    let mut fixes = Vec::new();
    append_header_fixes(&mut fixes, &formatted.fixes, &original);
    let mut after = Vec::new();
    append_header_issues(&mut after, &path, &formatted.issues);
    after.extend(analyze_makefile(&formatted.output).into_iter().map(|item| {
        Diagnostic {
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
        }
    }));
    after.sort();
    after.dedup();
    let changed = formatted.output.as_bytes() != original_bytes;
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
            before: Vec::new(),
            after,
            original: Some(Arc::from(original)),
            fixed: Some(Arc::from(formatted.output)),
        },
        plan,
    }
}

fn process_markdown(
    file: &DiscoveredFile,
    path: Utf8PathBuf,
    original_bytes: Vec<u8>,
    original: String,
    options: &FixOptions,
) -> FileWork {
    let result = match analyze_markdown(&original, options.format_markdown) {
        Ok(result) => result,
        Err(error) => {
            return failed_source(file, path, original_bytes, original, error.to_string());
        }
    };
    let proposed = result.formatted.unwrap_or_else(|| original.clone());
    let after = result
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
    }
}

fn commit_work(work: &mut [FileWork], options: &FixOptions) -> bool {
    let plans = work
        .iter()
        .filter_map(|item| item.plan.clone())
        .collect::<Vec<_>>();
    if plans.is_empty() {
        return true;
    }
    let project_root = transaction_root(plans.iter().map(|plan| plan.path.as_path()), &options.cwd);
    let requires_recovery = plans.iter().any(|plan| {
        plan.fixes.iter().any(|fix| {
            matches!(
                fix.rule_id.as_str(),
                "UNSAFE_REMOVE_UNUSED_STATIC" | "REMOVE_INVALID_COMMENT"
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
        let message =
            "The write was refused because no external backup directory is available. Configure HOME, XDG_DATA_HOME, or --backup-dir; use --no-backup only for ordinary non-destructive formatting."
                .to_owned();
        for item in work.iter_mut().filter(|item| item.plan.is_some()) {
            item.report.failure = Some(message.clone());
        }
        return false;
    }
    let transaction_options = TransactionOptions {
        project_root,
        run_id: run_id(),
        backup_root,
    };
    match commit_files(plans, &transaction_options) {
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
            for item in work.iter_mut().filter(|item| item.plan.is_some()) {
                item.report.failure = Some(message.clone());
                item.report.written = false;
            }
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
    std::fs::create_dir_all(&recovery).map_err(|error| {
        format!(
            "Could not create external quarantine storage `{}`: {error}",
            recovery.display()
        )
    })?;
    let project_root = options
        .cwd
        .canonicalize()
        .map_err(|error| format!("Could not resolve the project root: {error}"))?;
    let project_root = Utf8PathBuf::from_path_buf(project_root)
        .map_err(|path| format!("The project root is not valid UTF-8: {}", path.display()))?;
    let recovery_root = Utf8PathBuf::from_path_buf(recovery)
        .map_err(|path| format!("The recovery path is not valid UTF-8: {}", path.display()))?;
    Ok((project_root, recovery_root))
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
        if item.snapshot.readonly
            && let Ok(mut permissions) =
                std::fs::metadata(&item.destination_path).map(|metadata| metadata.permissions())
        {
            permissions.set_readonly(true);
            let _ = std::fs::set_permissions(&item.destination_path, permissions);
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
        return Some(PathBuf::from(path).join("norminette-fix/backups"));
    }
    std::env::var_os("HOME")
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .map(|home| home.join(".local/share/norminette-fix/backups"))
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
    for range in [
        approval.rename.define_range.clone(),
        approval.rename.ifndef_range.clone(),
    ] {
        if output.get(range.clone())? != approval.rename.current {
            return None;
        }
        output.replace_range(range, &approval.rename.expected);
    }
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
) -> Vec<Diagnostic> {
    diagnostics
        .iter()
        .map(|item| Diagnostic {
            rule_id: item.rule_id.clone(),
            path: path.clone(),
            range: diagnostic_range(source, item.line, item.column),
            severity: Severity::Warning,
            message: item.message.clone(),
            source: DiagnosticSource::NorminetteCompat(
                normfix_oracle::SUPPORTED_NORMINETTE_VERSION.to_owned(),
            ),
            notes: Vec::new(),
            help: Some(diagnostic_help(&item.rule_id).to_owned()),
        })
        .collect()
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

fn diagnostic_range(source: &str, line: u32, visual_column: u32) -> TextRange {
    let start = offset_for_line_column(source, line, visual_column);
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

fn offset_for_line_column(source: &str, line: u32, visual_column: u32) -> usize {
    let target_line = line.max(1);
    let target_column = visual_column.max(1);
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
        offset_for_line_column(source, line, 1),
        offset_for_line_column(source, line, 1),
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
                            "Repair the malformed or unsupported construct, then rerun norminette-fix."
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
) -> Vec<Diagnostic> {
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
            source: DiagnosticSource::NorminetteCompat(
                normfix_oracle::SUPPORTED_NORMINETTE_VERSION.to_owned(),
            ),
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
