//! Validated edit planning, backup and recoverable filesystem transactions.
//!
//! Rule crates never write files. They hand immutable original bytes and
//! validated replacements to this crate, which performs a preflight check,
//! stages every replacement, records a journal and commits in stable path
//! order. A mid-commit error triggers best-effort rollback from exact bytes.

#![forbid(unsafe_code)]

mod filesystem;

use filesystem::{
    absolute_lexical, canonical_directory, make_journal, persist_staged, preflight,
    preflight_bytes, prepare_backup_root, prepare_file_at, regular_file_without_symlink_components,
    reject_symlink_components, resolve_beneath, rollback, sha256_hex, validate_read_preconditions,
    validate_read_preconditions_except, write_journal,
};

use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{self, Read as _};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use normfix_core::FixRecord;
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;
use thiserror::Error;

const MAX_JOURNAL_BYTES: u64 = 16 * 1024 * 1024;
const MAX_UNDO_FILE_BYTES: u64 = 16 * 1024 * 1024;

/// One validated file replacement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlannedFile {
    /// Absolute file path.
    pub path: PathBuf,
    /// Exact bytes observed by analysis.
    pub original: Vec<u8>,
    /// Exact bytes accepted by every shadow-buffer proof.
    pub replacement: Vec<u8>,
    /// User-visible transformations represented by this replacement.
    pub fixes: Vec<FixRecord>,
}

/// One filesystem fact that must remain true for an edit to be sound.
///
/// These observations let project-wide analyses bind a write transaction to
/// files that are not themselves edited, such as every header considered by a
/// guard rename or a missing Makefile source reference.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ReadPrecondition {
    /// The regular file must still have this exact BLAKE3 digest.
    Matches {
        /// Observed project path.
        path: PathBuf,
        /// Digest of the complete observed bytes.
        blake3: [u8; 32],
    },
    /// The path must still be absent.
    Absent {
        /// Project path whose absence authorized an edit.
        path: PathBuf,
    },
    /// The complete non-symlink project `.c`/`.h` path set must stay exact.
    ///
    /// This protects closed-world source proofs against files created or
    /// removed after analysis. The validator follows the same structural
    /// exclusions as project discovery: `.git`, `.claude`, `.codex`, nested
    /// Git repositories and symbolic links are not traversed.
    ProjectSources {
        /// Project directory whose source membership was observed.
        root: PathBuf,
        /// Sorted absolute paths of every observed regular `.c`/`.h` file.
        paths: Vec<PathBuf>,
    },
}

impl ReadPrecondition {
    /// Creates an exact-byte observation for `path`.
    #[must_use]
    pub fn matches(path: impl Into<PathBuf>, bytes: &[u8]) -> Self {
        Self::Matches {
            path: path.into(),
            blake3: *blake3::hash(bytes).as_bytes(),
        }
    }

    /// Creates an absence observation for `path`.
    #[must_use]
    pub fn absent(path: impl Into<PathBuf>) -> Self {
        Self::Absent { path: path.into() }
    }

    /// Creates a closed project C/header membership observation.
    #[must_use]
    pub fn project_sources(
        root: impl Into<PathBuf>,
        paths: impl IntoIterator<Item = PathBuf>,
    ) -> Self {
        let mut paths = paths.into_iter().collect::<Vec<_>>();
        paths.sort();
        paths.dedup();
        Self::ProjectSources {
            root: root.into(),
            paths,
        }
    }
}

impl PlannedFile {
    /// Returns whether this replacement changes any byte.
    #[must_use]
    pub fn changed(&self) -> bool {
        self.original != self.replacement
    }
}

/// Filesystem policy for one transaction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransactionOptions {
    /// Canonical project root that every target must remain inside.
    pub project_root: PathBuf,
    /// Stable run identifier used for backup and journal paths.
    pub run_id: String,
    /// External backup base. `None` opts out for non-destructive edits.
    pub backup_root: Option<PathBuf>,
}

/// Result for one committed path.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CommittedFile {
    /// Original source path.
    pub path: PathBuf,
    /// Backup path, if backups were enabled.
    pub backup: Option<PathBuf>,
    /// Lowercase SHA-256 of the original bytes.
    pub original_sha256: String,
    /// Lowercase BLAKE3 of the replacement bytes.
    pub replacement_blake3: String,
}

/// Successful transaction summary.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CommitReport {
    /// Stable run identifier.
    pub run_id: String,
    /// Changed files in canonical path order.
    pub files: Vec<CommittedFile>,
    /// Journal path when backups were enabled.
    pub journal: Option<PathBuf>,
}

/// Persisted transaction state.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum JournalState {
    Prepared,
    Committing,
    Committed,
    RolledBack,
    RollbackFailed,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct Journal {
    schema_version: u32,
    run_id: String,
    state: JournalState,
    files: Vec<JournalFile>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct JournalFile {
    source: PathBuf,
    backup: Option<PathBuf>,
    original_sha256: String,
    replacement_blake3: String,
    committed: bool,
}

/// One committed backup transaction that can be restored safely.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct UndoRun {
    /// Stable transaction identifier.
    pub run_id: String,
    /// Journal that proves the backup set and replacement hashes.
    pub journal: PathBuf,
    /// Project files represented by the transaction.
    pub files: Vec<PathBuf>,
}

/// Successful restoration of one committed transaction.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct UndoReport {
    /// Original run that was restored.
    pub restored_run_id: String,
    /// Files restored in canonical path order.
    pub files: Vec<PathBuf>,
    /// Journal for the undo transaction, which retains the replaced bytes.
    pub journal: Option<PathBuf>,
}

/// A saved transaction could not be listed or restored safely.
#[derive(Debug, Error)]
pub enum UndoError {
    /// Backup storage could not be inspected.
    #[error("could not inspect undo storage `{path}`: {source}")]
    Inspect {
        /// Path that could not be inspected.
        path: PathBuf,
        /// Operating-system detail.
        #[source]
        source: io::Error,
    },
    /// A journal is malformed, unsupported, incomplete, or outside the project.
    #[error("invalid undo journal `{path}`: {message}")]
    InvalidJournal {
        /// Rejected journal.
        path: PathBuf,
        /// Actionable reason.
        message: String,
    },
    /// A target changed after the saved run and must not be overwritten.
    #[error("refused to undo because `{0}` changed after the selected run")]
    ModifiedSinceRun(PathBuf),
    /// A backup no longer matches the journal digest.
    #[error("backup integrity check failed for `{0}`")]
    BackupIntegrity(PathBuf),
    /// The restoration transaction failed.
    #[error(transparent)]
    Transaction(#[from] TransactionError),
}

/// A filesystem transaction was rejected or failed.
#[derive(Debug, Error)]
pub enum TransactionError {
    /// No changed files were supplied.
    #[error("the transaction contains no changed files")]
    Empty,
    /// A caller supplied a run identifier that could create nested paths.
    #[error("run identifier must contain only ASCII letters, digits, '.', '_' or '-'")]
    InvalidRunId,
    /// A target was repeated.
    #[error("the transaction contains the target more than once: {0}")]
    DuplicateTarget(PathBuf),
    /// A target escaped the project root.
    #[error("target is outside the project root: {0}")]
    OutsideProject(PathBuf),
    /// A target or one of its path components is a symbolic link.
    #[error("refused a path containing a symbolic link: {0}")]
    Symlink(PathBuf),
    /// The target is no longer the same regular file.
    #[error("target is not a regular file: {0}")]
    NotRegularFile(PathBuf),
    /// The file changed after analysis.
    #[error("the file changed after analysis; no write was attempted: {0}")]
    ConcurrentModification(PathBuf),
    /// A configured backup location is inside the scanned project.
    #[error("backup storage must be outside the project root: {0}")]
    BackupInsideProject(PathBuf),
    /// A path could not be canonicalized or inspected.
    #[error("could not inspect `{path}`: {source}")]
    Inspect {
        /// Affected path.
        path: PathBuf,
        /// Filesystem error.
        #[source]
        source: io::Error,
    },
    /// Staging or backup failed before any target was changed.
    #[error("could not prepare `{path}`: {source}")]
    Prepare {
        /// Affected path.
        path: PathBuf,
        /// Filesystem error.
        #[source]
        source: io::Error,
    },
    /// Commit failed and all changed targets were restored.
    #[error("commit failed at `{path}` and the original files were restored: {source}")]
    CommitRolledBack {
        /// Path whose commit failed.
        path: PathBuf,
        /// Original commit error.
        #[source]
        source: io::Error,
    },
    /// Commit failed and at least one rollback also failed.
    #[error(
        "commit failed at `{path}` and rollback was incomplete; inspect journal `{journal}`: {source}"
    )]
    RollbackFailed {
        /// Path whose commit failed.
        path: PathBuf,
        /// Recovery journal.
        journal: PathBuf,
        /// Original commit error.
        #[source]
        source: io::Error,
    },
}

struct PreparedFile {
    plan: PlannedFile,
    target_path: PathBuf,
    staged: NamedTempFile,
    backup: Option<PathBuf>,
    metadata: fs::Metadata,
}

/// Commits a validated set of file replacements.
///
/// All paths and original hashes are checked before a backup or target write.
/// Replacements are staged before the first rename. Targets are committed in
/// canonical path order and directory entries are synced where supported.
///
/// # Errors
///
/// Returns [`TransactionError`] without changing a target on any preflight or
/// staging failure. A mid-commit failure triggers rollback and reports whether
/// recovery was complete.
pub fn commit_files(
    plans: Vec<PlannedFile>,
    options: &TransactionOptions,
) -> Result<CommitReport, TransactionError> {
    commit_files_guarded(plans, options, &[])
}

/// Commits replacements only while every project-wide observation remains true.
///
/// Preconditions are checked before backup/staging and again immediately
/// before each target replacement. If one changes after an earlier target was
/// committed, the complete transaction is rolled back.
///
/// # Errors
///
/// Returns [`TransactionError::ConcurrentModification`] without retaining a
/// target write when any observed file or absence changed.
pub fn commit_files_guarded(
    plans: Vec<PlannedFile>,
    options: &TransactionOptions,
    preconditions: &[ReadPrecondition],
) -> Result<CommitReport, TransactionError> {
    let (mut prepared, backup_run) = prepare_transaction(plans, options, preconditions)?;
    let (mut journal, journal_path) = begin_journal(options, &prepared, backup_run.as_deref())?;
    let committed_indices = commit_prepared_files(
        &mut prepared,
        &mut journal,
        journal_path.as_deref(),
        options,
        preconditions,
    )?;
    finish_journal(
        &prepared,
        &committed_indices,
        &mut journal,
        journal_path.as_deref(),
        options,
    )?;
    Ok(build_commit_report(options, prepared, journal_path))
}

/// Lists committed backup runs whose complete target set belongs to `project_root`.
///
/// Malformed or unrelated journals are ignored so one damaged historical run
/// cannot hide later valid recovery points. An unreadable backup root remains
/// an operational error.
///
/// # Errors
///
/// Returns [`UndoError::Inspect`] when the backup directory itself cannot be read.
pub fn list_undo_runs(backup_root: &Path, project_root: &Path) -> Result<Vec<UndoRun>, UndoError> {
    let root_input = absolute_lexical(project_root);
    let entries = match fs::read_dir(backup_root) {
        Ok(entries) => entries,
        Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => {
            return Err(UndoError::Inspect {
                path: backup_root.to_path_buf(),
                source,
            });
        }
    };
    let root = project_root
        .canonicalize()
        .map_err(|source| UndoError::Inspect {
            path: project_root.to_path_buf(),
            source,
        })?;
    let mut runs = Vec::new();
    for entry in entries.flatten() {
        if entry.file_type().is_ok_and(|kind| kind.is_symlink()) {
            continue;
        }
        let run_directory = entry.path();
        let Ok(canonical_run_directory) = run_directory.canonicalize() else {
            continue;
        };
        let journal_path = run_directory.join("journal.json");
        let Ok(metadata) = fs::symlink_metadata(&journal_path) else {
            continue;
        };
        if !metadata.file_type().is_file() {
            continue;
        }
        let Ok(journal) = read_journal(&journal_path) else {
            continue;
        };
        if journal.schema_version != 1
            || journal.state != JournalState::Committed
            || journal.files.is_empty()
            || entry.file_name() != journal.run_id.as_str()
            || journal.files.iter().any(|file| {
                !journal_file_is_intact(file, &root_input, &root, &canonical_run_directory)
            })
        {
            continue;
        }
        let mut files = journal
            .files
            .iter()
            .map(|file| file.source.clone())
            .collect::<Vec<_>>();
        files.sort();
        let modified = metadata.modified().unwrap_or(UNIX_EPOCH);
        runs.push((
            modified,
            UndoRun {
                run_id: journal.run_id,
                journal: journal_path,
                files,
            },
        ));
    }
    runs.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then_with(|| left.1.run_id.cmp(&right.1.run_id))
    });
    Ok(runs.into_iter().map(|(_, run)| run).collect())
}

fn journal_file_is_intact(
    file: &JournalFile,
    project_root_input: &Path,
    project_root: &Path,
    run_directory: &Path,
) -> bool {
    if !file.committed {
        return false;
    }
    let Some(source) = resolve_beneath(project_root_input, project_root, &file.source) else {
        return false;
    };
    let Some(backup) = file.backup.as_ref() else {
        return false;
    };
    let backup = absolute_lexical(backup);
    if !regular_file_without_symlink_components(project_root, &source)
        || !regular_file_without_symlink_components(run_directory, &backup)
    {
        return false;
    }
    read_regular_file_bounded(&source, MAX_UNDO_FILE_BYTES)
        .is_ok_and(|bytes| blake3::hash(&bytes).to_hex().as_str() == file.replacement_blake3)
        && read_regular_file_bounded(&backup, MAX_UNDO_FILE_BYTES)
            .is_ok_and(|bytes| sha256_hex(&bytes) == file.original_sha256)
}

/// Restores a committed run only while every target still has the exact bytes
/// written by that run. The undo itself is a new backed-up transaction, so the
/// displaced formatted bytes remain recoverable.
///
/// # Errors
///
/// Fails closed on a changed target, corrupt backup, invalid journal, symlink,
/// path escape, concurrent modification, or transactional write failure.
pub fn undo_run(
    run: &UndoRun,
    project_root: &Path,
    backup_root: &Path,
) -> Result<UndoReport, UndoError> {
    let location = resolve_undo_journal(run, backup_root)?;
    let journal = read_journal(&location.journal).map_err(|source| UndoError::InvalidJournal {
        path: run.journal.clone(),
        message: source.to_string(),
    })?;
    let project_root_input = absolute_lexical(project_root);
    let canonical_project_root =
        project_root
            .canonicalize()
            .map_err(|source| UndoError::Inspect {
                path: project_root.to_path_buf(),
                source,
            })?;
    let plans = validate_undo_journal(
        &journal,
        run,
        &project_root_input,
        &canonical_project_root,
        &location.run_directory,
    )?;
    let options = TransactionOptions {
        project_root: project_root.to_path_buf(),
        run_id: undo_run_id(),
        backup_root: Some(location.backup_root),
    };
    let commit = commit_files(plans, &options)?;
    Ok(UndoReport {
        restored_run_id: run.run_id.clone(),
        files: commit.files.into_iter().map(|file| file.path).collect(),
        journal: commit.journal,
    })
}

fn read_journal(path: &Path) -> io::Result<Journal> {
    let bytes = read_regular_file_bounded(path, MAX_JOURNAL_BYTES)?;
    serde_json::from_slice(&bytes).map_err(io::Error::other)
}

fn read_regular_file_bounded(path: &Path, limit: u64) -> io::Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_file() || metadata.len() > limit {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "path is not a bounded regular file",
        ));
    }
    let capacity = usize::try_from(metadata.len())
        .map_err(|_| io::Error::other("file length does not fit in memory"))?;
    let mut bytes = Vec::with_capacity(capacity);
    File::open(path)?
        .take(limit.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if u64::try_from(bytes.len()) != Ok(metadata.len()) {
        return Err(io::Error::other("file changed while it was being read"));
    }
    Ok(bytes)
}

fn validate_undo_journal(
    journal: &Journal,
    run: &UndoRun,
    project_root_input: &Path,
    project_root: &Path,
    run_directory: &Path,
) -> Result<Vec<PlannedFile>, UndoError> {
    let invalid = journal.schema_version != 1
        || journal.state != JournalState::Committed
        || journal.run_id != run.run_id
        || journal.files.is_empty();
    if invalid {
        return Err(invalid_undo_journal(
            run,
            "the journal is not a complete committed run for this project",
        ));
    }
    let mut journal_sources = journal
        .files
        .iter()
        .map(|file| file.source.clone())
        .collect::<Vec<_>>();
    let mut advertised_sources = run.files.clone();
    journal_sources.sort();
    advertised_sources.sort();
    let journal_source_set = journal_sources.iter().collect::<BTreeSet<_>>();
    let advertised_source_set = advertised_sources.iter().collect::<BTreeSet<_>>();
    if journal_sources != advertised_sources
        || journal_source_set.len() != journal_sources.len()
        || advertised_source_set.len() != advertised_sources.len()
    {
        return Err(invalid_undo_journal(
            run,
            "the journal source set no longer matches the confirmed undo run",
        ));
    }
    let mut backups = BTreeSet::new();
    let mut plans = Vec::with_capacity(journal.files.len());
    for file in &journal.files {
        let current = validate_undo_source(file, run, project_root_input, project_root)?;
        let original = validate_undo_backup(file, run, run_directory, &mut backups)?;
        plans.push(PlannedFile {
            path: file.source.clone(),
            original: current,
            replacement: original,
            fixes: vec![FixRecord {
                rule_id: "UNDO".to_owned(),
                description: format!("restored exact bytes from run {}", run.run_id),
                line: None,
                count: 1,
            }],
        });
    }
    Ok(plans)
}

fn validate_undo_source(
    file: &JournalFile,
    run: &UndoRun,
    project_root_input: &Path,
    project_root: &Path,
) -> Result<Vec<u8>, UndoError> {
    if !file.committed {
        return Err(invalid_undo_journal(
            run,
            "the journal contains an uncommitted file",
        ));
    }
    let Some(source) = resolve_beneath(project_root_input, project_root, &file.source) else {
        return Err(invalid_undo_journal(
            run,
            "a journal source is outside the selected project",
        ));
    };
    if !regular_file_without_symlink_components(project_root, &source) {
        return Err(invalid_undo_journal(
            run,
            "a journal source or one of its components is not a regular non-symlink file",
        ));
    }
    let bytes =
        read_regular_file_bounded(&source, MAX_UNDO_FILE_BYTES).map_err(|source_error| {
            UndoError::Inspect {
                path: source.clone(),
                source: source_error,
            }
        })?;
    if blake3::hash(&bytes).to_hex().as_str() != file.replacement_blake3 {
        return Err(UndoError::ModifiedSinceRun(file.source.clone()));
    }
    Ok(bytes)
}

fn validate_undo_backup(
    file: &JournalFile,
    run: &UndoRun,
    run_directory: &Path,
    backups: &mut BTreeSet<PathBuf>,
) -> Result<Vec<u8>, UndoError> {
    let Some(backup) = file.backup.as_ref().map(|path| absolute_lexical(path)) else {
        return Err(invalid_undo_journal(
            run,
            "a committed journal file has no retained backup",
        ));
    };
    if !backups.insert(backup.clone())
        || !regular_file_without_symlink_components(run_directory, &backup)
    {
        return Err(invalid_undo_journal(
            run,
            "a backup is duplicated, outside its run, or behind a symbolic link",
        ));
    }
    let bytes = read_regular_file_bounded(&backup, MAX_UNDO_FILE_BYTES).map_err(|source| {
        UndoError::Inspect {
            path: backup.clone(),
            source,
        }
    })?;
    if sha256_hex(&bytes) != file.original_sha256 {
        return Err(UndoError::BackupIntegrity(backup));
    }
    Ok(bytes)
}

struct UndoJournalLocation {
    journal: PathBuf,
    run_directory: PathBuf,
    backup_root: PathBuf,
}

fn resolve_undo_journal(
    run: &UndoRun,
    backup_root: &Path,
) -> Result<UndoJournalLocation, UndoError> {
    let backup_root_input = absolute_lexical(backup_root);
    let canonical_backup_root =
        backup_root
            .canonicalize()
            .map_err(|source| UndoError::Inspect {
                path: backup_root.to_path_buf(),
                source,
            })?;
    let Some(journal) = resolve_beneath(&backup_root_input, &canonical_backup_root, &run.journal)
    else {
        return Err(invalid_undo_journal(
            run,
            "the journal is outside the selected backup root",
        ));
    };
    let relative = journal
        .strip_prefix(&canonical_backup_root)
        .map_err(|_| invalid_undo_journal(run, "the journal escaped the backup root"))?;
    let components = relative.components().collect::<Vec<_>>();
    if components.len() != 2
        || components[0].as_os_str() != run.run_id.as_str()
        || components[1].as_os_str() != "journal.json"
        || reject_symlink_components(&canonical_backup_root, &journal).is_err()
        || !fs::symlink_metadata(&journal).is_ok_and(|metadata| metadata.file_type().is_file())
    {
        return Err(invalid_undo_journal(
            run,
            "the journal path is not the expected regular non-symlink file",
        ));
    }
    let Some(run_directory) = journal.parent() else {
        return Err(invalid_undo_journal(
            run,
            "the journal has no run directory",
        ));
    };
    let run_directory = run_directory.to_path_buf();
    Ok(UndoJournalLocation {
        journal,
        run_directory,
        backup_root: canonical_backup_root,
    })
}

fn invalid_undo_journal(run: &UndoRun, message: impl Into<String>) -> UndoError {
    UndoError::InvalidJournal {
        path: run.journal.clone(),
        message: message.into(),
    }
}

fn undo_run_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    format!("undo-{nanos}-{}", std::process::id())
}

fn prepare_transaction(
    plans: Vec<PlannedFile>,
    options: &TransactionOptions,
    preconditions: &[ReadPrecondition],
) -> Result<(Vec<PreparedFile>, Option<PathBuf>), TransactionError> {
    let plans = plans
        .into_iter()
        .filter(PlannedFile::changed)
        .collect::<Vec<_>>();
    if plans.is_empty() {
        return Err(TransactionError::Empty);
    }
    if options.run_id.is_empty()
        || !options
            .run_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(TransactionError::InvalidRunId);
    }
    let root_input = absolute_lexical(&options.project_root);
    let root = canonical_directory(&options.project_root)?;
    let mut resolved_plans = Vec::with_capacity(plans.len());
    for plan in plans {
        preflight(&plan, &root_input, &root)?;
        let (_, resolved) = filesystem::resolve_transaction_path(&plan.path, &root_input, &root)?;
        resolved_plans.push((resolved, plan));
    }
    resolved_plans.sort_by(|left, right| left.0.cmp(&right.0));
    if let Some(duplicate) = resolved_plans
        .windows(2)
        .find(|pair| pair[0].0 == pair[1].0)
        .map(|pair| pair[1].1.path.clone())
    {
        return Err(TransactionError::DuplicateTarget(duplicate));
    }
    validate_read_preconditions(preconditions, &root_input, &root)?;
    let backup_run = prepare_backup_root(options, &root_input, &root)?;
    let mut prepared = Vec::with_capacity(resolved_plans.len());
    for (target_path, plan) in resolved_plans {
        prepared.push(prepare_file_at(plan, target_path, backup_run.as_deref())?);
    }
    Ok((prepared, backup_run))
}

fn begin_journal(
    options: &TransactionOptions,
    prepared: &[PreparedFile],
    backup_run: Option<&Path>,
) -> Result<(Journal, Option<PathBuf>), TransactionError> {
    let mut journal = make_journal(options, prepared);
    let journal_path = backup_run.map(|root| root.join("journal.json"));
    if let Some(path) = &journal_path {
        prepare_journal_write(path, &journal)?;
    }
    journal.state = JournalState::Committing;
    if let Some(path) = &journal_path {
        prepare_journal_write(path, &journal)?;
    }
    Ok((journal, journal_path))
}

fn prepare_journal_write(path: &Path, journal: &Journal) -> Result<(), TransactionError> {
    write_journal(path, journal).map_err(|source| TransactionError::Prepare {
        path: path.to_path_buf(),
        source,
    })
}

fn commit_prepared_files(
    prepared: &mut [PreparedFile],
    journal: &mut Journal,
    journal_path: Option<&Path>,
    options: &TransactionOptions,
    preconditions: &[ReadPrecondition],
) -> Result<Vec<usize>, TransactionError> {
    let mut committed_indices: Vec<usize> = Vec::new();
    let root_input = absolute_lexical(&options.project_root);
    let root = canonical_directory(&options.project_root)?;
    for index in 0..prepared.len() {
        let target = prepared[index].plan.path.clone();
        if let Err((changed_path, source)) =
            validate_committed_replacements(prepared, &committed_indices)
        {
            return Err(rollback_error(
                prepared,
                &committed_indices,
                journal,
                journal_path,
                &options.project_root,
                changed_path,
                source,
            ));
        }
        let committed_paths = committed_indices
            .iter()
            .map(|committed| absolute_lexical(&prepared[*committed].target_path))
            .collect::<BTreeSet<_>>();
        if let Err(error) =
            validate_read_preconditions_except(preconditions, &root_input, &root, &committed_paths)
        {
            if committed_indices.is_empty() {
                journal.state = JournalState::RolledBack;
                if let Some(path) = journal_path {
                    let _ = write_journal(path, journal);
                }
                return Err(error);
            }
            return Err(rollback_error(
                prepared,
                &committed_indices,
                journal,
                journal_path,
                &options.project_root,
                target,
                io::Error::other(error.to_string()),
            ));
        }
        if let Err(error) = preflight(&prepared[index].plan, &root_input, &root) {
            if committed_indices.is_empty() {
                return Err(error);
            }
            return Err(rollback_error(
                prepared,
                &committed_indices,
                journal,
                journal_path,
                &options.project_root,
                target,
                io::Error::other(error.to_string()),
            ));
        }
        if let Err(source) = persist_staged(&mut prepared[index]) {
            return Err(rollback_error(
                prepared,
                &committed_indices,
                journal,
                journal_path,
                &options.project_root,
                target,
                source,
            ));
        }
        committed_indices.push(index);
        journal.files[index].committed = true;
        if let Some(path) = journal_path {
            if let Err(source) = write_journal(path, journal) {
                return Err(rollback_error(
                    prepared,
                    &committed_indices,
                    journal,
                    journal_path,
                    &options.project_root,
                    target,
                    source,
                ));
            }
        }
    }
    Ok(committed_indices)
}

fn validate_committed_replacements(
    prepared: &[PreparedFile],
    committed_indices: &[usize],
) -> Result<(), (PathBuf, io::Error)> {
    for index in committed_indices {
        let file = &prepared[*index];
        if let Err(source) = preflight_bytes(&file.target_path, &file.plan.replacement) {
            return Err((file.plan.path.clone(), source));
        }
    }
    Ok(())
}

fn finish_journal(
    prepared: &[PreparedFile],
    committed_indices: &[usize],
    journal: &mut Journal,
    journal_path: Option<&Path>,
    options: &TransactionOptions,
) -> Result<(), TransactionError> {
    journal.state = JournalState::Committed;
    if let Some(path) = journal_path {
        if let Err(source) = write_journal(path, journal) {
            return Err(rollback_error(
                prepared,
                committed_indices,
                journal,
                journal_path,
                &options.project_root,
                path.to_path_buf(),
                source,
            ));
        }
    }
    Ok(())
}

fn rollback_error(
    prepared: &[PreparedFile],
    committed_indices: &[usize],
    journal: &mut Journal,
    journal_path: Option<&Path>,
    project_root: &Path,
    failed_path: PathBuf,
    source: io::Error,
) -> TransactionError {
    let rollback_ok = rollback(prepared, committed_indices);
    journal.state = if rollback_ok {
        JournalState::RolledBack
    } else {
        JournalState::RollbackFailed
    };
    if let Some(path) = journal_path {
        let _ = write_journal(path, journal);
    }
    if rollback_ok {
        TransactionError::CommitRolledBack {
            path: failed_path,
            source,
        }
    } else {
        TransactionError::RollbackFailed {
            path: failed_path,
            journal: journal_path
                .map_or_else(|| project_root.join(".rollback-failed"), Path::to_path_buf),
            source,
        }
    }
}

fn build_commit_report(
    options: &TransactionOptions,
    prepared: Vec<PreparedFile>,
    journal_path: Option<PathBuf>,
) -> CommitReport {
    let files = prepared
        .into_iter()
        .map(|prepared| CommittedFile {
            path: prepared.plan.path,
            backup: prepared.backup,
            original_sha256: sha256_hex(&prepared.plan.original),
            replacement_blake3: blake3::hash(&prepared.plan.replacement)
                .to_hex()
                .to_string(),
        })
        .collect();
    CommitReport {
        run_id: options.run_id.clone(),
        files,
        journal: journal_path,
    }
}

#[cfg(test)]
mod tests;
