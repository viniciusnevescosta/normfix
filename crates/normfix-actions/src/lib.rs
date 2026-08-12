//! Validated edit planning, backup and recoverable filesystem transactions.
//!
//! Rule crates never write files. They hand immutable original bytes and
//! validated replacements to this crate, which performs a preflight check,
//! stages every replacement, records a journal and commits in stable path
//! order. A mid-commit error triggers best-effort rollback from exact bytes.

#![forbid(unsafe_code)]

use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use filetime::{FileTime, set_file_times};
use normfix_core::FixRecord;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tempfile::NamedTempFile;
use thiserror::Error;

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
    fs::read(source)
        .is_ok_and(|bytes| blake3::hash(&bytes).to_hex().to_string() == file.replacement_blake3)
        && fs::read(&backup).is_ok_and(|bytes| sha256_hex(&bytes) == file.original_sha256)
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
    let bytes = fs::read(path)?;
    serde_json::from_slice(&bytes).map_err(io::Error::other)
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
        let current = fs::read(&source).map_err(|source_error| UndoError::Inspect {
            path: source.clone(),
            source: source_error,
        })?;
        if blake3::hash(&current).to_hex().to_string() != file.replacement_blake3 {
            return Err(UndoError::ModifiedSinceRun(file.source.clone()));
        }
        let original = fs::read(&backup).map_err(|source| UndoError::Inspect {
            path: backup.clone(),
            source,
        })?;
        if sha256_hex(&original) != file.original_sha256 {
            return Err(UndoError::BackupIntegrity(backup));
        }
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
    let mut plans = plans
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
    plans.sort_by(|left, right| left.path.cmp(&right.path));
    reject_duplicate_targets(&plans)?;
    let root_input = absolute_lexical(&options.project_root);
    let root = canonical_directory(&options.project_root)?;
    for plan in &plans {
        preflight(plan, &root_input, &root)?;
    }
    validate_read_preconditions(preconditions, &root_input, &root)?;
    let backup_run = prepare_backup_root(options, &root_input, &root)?;
    let mut prepared = Vec::with_capacity(plans.len());
    for plan in plans {
        prepared.push(prepare_file(plan, backup_run.as_deref())?);
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
            .map(|committed| absolute_lexical(&prepared[*committed].plan.path))
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
        if let Err(source) = preflight_bytes(&file.plan.path, &file.plan.replacement) {
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

fn reject_duplicate_targets(plans: &[PlannedFile]) -> Result<(), TransactionError> {
    let mut seen = BTreeSet::new();
    for plan in plans {
        if !seen.insert(&plan.path) {
            return Err(TransactionError::DuplicateTarget(plan.path.clone()));
        }
    }
    Ok(())
}

fn canonical_directory(path: &Path) -> Result<PathBuf, TransactionError> {
    path.canonicalize()
        .map_err(|source| TransactionError::Inspect {
            path: path.to_path_buf(),
            source,
        })
}

fn prepare_backup_root(
    options: &TransactionOptions,
    project_root_input: &Path,
    project_root: &Path,
) -> Result<Option<PathBuf>, TransactionError> {
    let Some(base) = &options.backup_root else {
        return Ok(None);
    };
    let absolute = absolute_lexical(base);
    if absolute.starts_with(project_root_input) || absolute.starts_with(project_root) {
        return Err(TransactionError::BackupInsideProject(absolute));
    }
    let canonical_base = create_canonical_backup_base(&absolute, project_root)?;
    let run = canonical_base.join(&options.run_id);
    fs::create_dir(&run).map_err(|source| TransactionError::Prepare {
        path: run.clone(),
        source,
    })?;
    set_private_directory_permissions(&run).map_err(|source| TransactionError::Prepare {
        path: run.clone(),
        source,
    })?;
    let resolved = run
        .canonicalize()
        .map_err(|source| TransactionError::Inspect {
            path: run.clone(),
            source,
        })?;
    if resolved.starts_with(project_root) || resolved.parent() != Some(canonical_base.as_path()) {
        return Err(TransactionError::BackupInsideProject(resolved));
    }
    Ok(Some(resolved))
}

fn create_canonical_backup_base(
    absolute: &Path,
    project_root: &Path,
) -> Result<PathBuf, TransactionError> {
    let mut existing = absolute.to_path_buf();
    let mut missing = Vec::new();
    loop {
        match fs::symlink_metadata(&existing) {
            Ok(_) => break,
            Err(source) if source.kind() == io::ErrorKind::NotFound => {
                let Some(name) = existing.file_name() else {
                    return Err(TransactionError::Inspect {
                        path: existing,
                        source,
                    });
                };
                missing.push(name.to_os_string());
                if !existing.pop() {
                    return Err(TransactionError::Inspect {
                        path: absolute.to_path_buf(),
                        source: io::Error::new(
                            io::ErrorKind::NotFound,
                            "backup path has no existing ancestor",
                        ),
                    });
                }
            }
            Err(source) => {
                return Err(TransactionError::Inspect {
                    path: existing,
                    source,
                });
            }
        }
    }
    let mut canonical = existing
        .canonicalize()
        .map_err(|source| TransactionError::Inspect {
            path: existing.clone(),
            source,
        })?;
    if !canonical.is_dir() {
        return Err(TransactionError::Inspect {
            path: canonical,
            source: io::Error::other("backup path ancestor is not a directory"),
        });
    }
    if canonical.starts_with(project_root) {
        return Err(TransactionError::BackupInsideProject(canonical));
    }
    for component in missing.into_iter().rev() {
        let next = canonical.join(component);
        match fs::create_dir(&next) {
            Ok(()) => {
                set_private_directory_permissions(&next).map_err(|source| {
                    TransactionError::Prepare {
                        path: next.clone(),
                        source,
                    }
                })?;
            }
            Err(source) if source.kind() == io::ErrorKind::AlreadyExists => {}
            Err(source) => {
                return Err(TransactionError::Prepare { path: next, source });
            }
        }
        let metadata = fs::symlink_metadata(&next).map_err(|source| TransactionError::Inspect {
            path: next.clone(),
            source,
        })?;
        if metadata.file_type().is_symlink() {
            return Err(TransactionError::Symlink(next));
        }
        if !metadata.file_type().is_dir() {
            return Err(TransactionError::Inspect {
                path: next,
                source: io::Error::other("backup path component is not a directory"),
            });
        }
        canonical = next
            .canonicalize()
            .map_err(|source| TransactionError::Inspect { path: next, source })?;
        if canonical.starts_with(project_root) {
            return Err(TransactionError::BackupInsideProject(canonical));
        }
    }
    Ok(canonical)
}

fn preflight(
    plan: &PlannedFile,
    root_input: &Path,
    canonical_root: &Path,
) -> Result<(), TransactionError> {
    let absolute = absolute_lexical(&plan.path);
    let resolved = if let Ok(relative) = absolute.strip_prefix(root_input) {
        canonical_root.join(relative)
    } else if absolute.starts_with(canonical_root) {
        absolute.clone()
    } else {
        return Err(TransactionError::OutsideProject(absolute));
    };
    reject_symlink_components(canonical_root, &resolved)?;
    let metadata = fs::symlink_metadata(&absolute).map_err(|source| TransactionError::Inspect {
        path: absolute.clone(),
        source,
    })?;
    if !metadata.file_type().is_file() {
        return Err(TransactionError::NotRegularFile(absolute));
    }
    let current = fs::read(&absolute).map_err(|source| TransactionError::Inspect {
        path: absolute.clone(),
        source,
    })?;
    if current != plan.original {
        return Err(TransactionError::ConcurrentModification(absolute));
    }
    Ok(())
}

fn validate_read_preconditions(
    preconditions: &[ReadPrecondition],
    root_input: &Path,
    canonical_root: &Path,
) -> Result<(), TransactionError> {
    validate_read_preconditions_except(preconditions, root_input, canonical_root, &BTreeSet::new())
}

fn validate_read_preconditions_except(
    preconditions: &[ReadPrecondition],
    root_input: &Path,
    canonical_root: &Path,
    ignored_paths: &BTreeSet<PathBuf>,
) -> Result<(), TransactionError> {
    for precondition in preconditions {
        let path = match precondition {
            ReadPrecondition::Matches { path, .. } | ReadPrecondition::Absent { path } => path,
            ReadPrecondition::ProjectSources { root, .. } => root,
        };
        if ignored_paths.contains(&absolute_lexical(path)) {
            continue;
        }
        match precondition {
            ReadPrecondition::Matches { path, blake3 } => {
                let (absolute, resolved) =
                    resolve_transaction_path(path, root_input, canonical_root)?;
                reject_symlink_components(canonical_root, &resolved)?;
                let metadata = fs::symlink_metadata(&absolute).map_err(|source| {
                    if source.kind() == io::ErrorKind::NotFound {
                        TransactionError::ConcurrentModification(absolute.clone())
                    } else {
                        TransactionError::Inspect {
                            path: absolute.clone(),
                            source,
                        }
                    }
                })?;
                if !metadata.file_type().is_file() {
                    return Err(TransactionError::ConcurrentModification(absolute));
                }
                let bytes = fs::read(&absolute).map_err(|source| TransactionError::Inspect {
                    path: absolute.clone(),
                    source,
                })?;
                if blake3::hash(&bytes).as_bytes() != blake3 {
                    return Err(TransactionError::ConcurrentModification(absolute));
                }
            }
            ReadPrecondition::Absent { path } => {
                let (absolute, resolved) =
                    resolve_transaction_path(path, root_input, canonical_root)?;
                reject_symlink_components_allow_missing(canonical_root, &resolved)?;
                match fs::symlink_metadata(&absolute) {
                    Err(source) if source.kind() == io::ErrorKind::NotFound => {}
                    Ok(_) => return Err(TransactionError::ConcurrentModification(absolute)),
                    Err(source) => {
                        return Err(TransactionError::Inspect {
                            path: absolute,
                            source,
                        });
                    }
                }
            }
            ReadPrecondition::ProjectSources { root, paths } => {
                validate_project_source_membership(root, paths, root_input, canonical_root)?;
            }
        }
    }
    Ok(())
}

const MAX_PROJECT_TREE_ENTRIES: usize = 1_000_000;

fn validate_project_source_membership(
    observed_root: &Path,
    expected_paths: &[PathBuf],
    transaction_root_input: &Path,
    canonical_transaction_root: &Path,
) -> Result<(), TransactionError> {
    let (absolute_root, resolved_root) = resolve_transaction_path(
        observed_root,
        transaction_root_input,
        canonical_transaction_root,
    )?;
    reject_symlink_components(canonical_transaction_root, &resolved_root)?;
    let metadata = fs::symlink_metadata(&absolute_root).map_err(|source| {
        if source.kind() == io::ErrorKind::NotFound {
            TransactionError::ConcurrentModification(absolute_root.clone())
        } else {
            TransactionError::Inspect {
                path: absolute_root.clone(),
                source,
            }
        }
    })?;
    if !metadata.file_type().is_dir() {
        return Err(TransactionError::ConcurrentModification(absolute_root));
    }

    let mut expected = BTreeSet::new();
    for path in expected_paths {
        let (absolute, resolved) =
            resolve_transaction_path(path, transaction_root_input, canonical_transaction_root)?;
        if !resolved.starts_with(&resolved_root)
            || !matches!(
                absolute.extension().and_then(std::ffi::OsStr::to_str),
                Some("c" | "h")
            )
            || !expected.insert(absolute_lexical(&absolute))
        {
            return Err(TransactionError::ConcurrentModification(absolute));
        }
    }
    let actual =
        collect_project_sources(&absolute_root).map_err(|source| TransactionError::Inspect {
            path: absolute_root.clone(),
            source,
        })?;
    if actual != expected {
        let changed = actual
            .symmetric_difference(&expected)
            .next()
            .cloned()
            .unwrap_or(absolute_root);
        return Err(TransactionError::ConcurrentModification(changed));
    }
    Ok(())
}

fn collect_project_sources(root: &Path) -> io::Result<BTreeSet<PathBuf>> {
    let mut sources = BTreeSet::new();
    let mut pending = vec![root.to_path_buf()];
    let mut visited = 0_usize;
    while let Some(directory) = pending.pop() {
        let mut entries = fs::read_dir(&directory)?.collect::<Result<Vec<_>, _>>()?;
        entries.sort_by_key(fs::DirEntry::path);
        for entry in entries.into_iter().rev() {
            visited = visited.saturating_add(1);
            if visited > MAX_PROJECT_TREE_ENTRIES {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "project source membership exceeds the transaction safety limit",
                ));
            }
            let path = entry.path();
            let file_type = entry.file_type()?;
            if file_type.is_symlink() {
                continue;
            }
            if file_type.is_dir() {
                let name = entry.file_name();
                if name == ".git"
                    || matches!(name.to_str(), Some(".claude" | ".codex"))
                    || fs::symlink_metadata(path.join(".git")).is_ok()
                {
                    continue;
                }
                pending.push(path);
            } else if file_type.is_file()
                && matches!(
                    path.extension().and_then(std::ffi::OsStr::to_str),
                    Some("c" | "h")
                )
            {
                sources.insert(absolute_lexical(&path));
            }
        }
    }
    Ok(sources)
}

fn resolve_transaction_path(
    path: &Path,
    root_input: &Path,
    canonical_root: &Path,
) -> Result<(PathBuf, PathBuf), TransactionError> {
    let absolute = absolute_lexical(path);
    let resolved = if let Ok(relative) = absolute.strip_prefix(root_input) {
        canonical_root.join(relative)
    } else if absolute.starts_with(canonical_root) {
        absolute.clone()
    } else {
        return Err(TransactionError::OutsideProject(absolute));
    };
    Ok((absolute, resolved))
}

fn prepare_file(
    plan: PlannedFile,
    backup_run: Option<&Path>,
) -> Result<PreparedFile, TransactionError> {
    let metadata = fs::metadata(&plan.path).map_err(|source| TransactionError::Prepare {
        path: plan.path.clone(),
        source,
    })?;
    let backup = backup_run
        .map(|root| backup_path(root, &plan.path))
        .transpose()
        .map_err(|source| TransactionError::Prepare {
            path: plan.path.clone(),
            source,
        })?;
    if let Some(path) = &backup {
        write_exact_file(path, &plan.original, &metadata).map_err(|source| {
            TransactionError::Prepare {
                path: path.clone(),
                source,
            }
        })?;
    }
    let parent = plan
        .path
        .parent()
        .ok_or_else(|| TransactionError::OutsideProject(plan.path.clone()))?;
    let mut staged = NamedTempFile::new_in(parent).map_err(|source| TransactionError::Prepare {
        path: plan.path.clone(),
        source,
    })?;
    staged
        .write_all(&plan.replacement)
        .and_then(|()| staged.as_file_mut().sync_all())
        .map_err(|source| TransactionError::Prepare {
            path: plan.path.clone(),
            source,
        })?;
    apply_metadata(staged.path(), &metadata).map_err(|source| TransactionError::Prepare {
        path: plan.path.clone(),
        source,
    })?;
    Ok(PreparedFile {
        plan,
        staged,
        backup,
        metadata,
    })
}

fn persist_staged(prepared: &mut PreparedFile) -> io::Result<()> {
    preflight_bytes(&prepared.plan.path, &prepared.plan.original)?;
    let parent = prepared
        .plan
        .path
        .parent()
        .ok_or_else(|| io::Error::other("target has no parent directory"))?;
    let staged = std::mem::replace(&mut prepared.staged, NamedTempFile::new_in(parent)?);
    staged
        .persist(&prepared.plan.path)
        .map_err(|error| error.error)?;
    sync_directory(parent)
}

fn rollback(prepared: &[PreparedFile], committed_indices: &[usize]) -> bool {
    let mut complete = true;
    for index in committed_indices.iter().rev() {
        let file = &prepared[*index];
        if preflight_bytes(&file.plan.path, &file.plan.replacement).is_err()
            || restore_exact(&file.plan.path, &file.plan.original, &file.metadata).is_err()
        {
            complete = false;
        }
    }
    complete
}

fn restore_exact(path: &Path, bytes: &[u8], metadata: &fs::Metadata) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("target has no parent directory"))?;
    let mut temporary = NamedTempFile::new_in(parent)?;
    temporary.write_all(bytes)?;
    temporary.as_file_mut().sync_all()?;
    apply_metadata(temporary.path(), metadata)?;
    temporary.persist(path).map_err(|error| error.error)?;
    sync_directory(parent)
}

fn preflight_bytes(path: &Path, expected: &[u8]) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_file() {
        return Err(io::Error::other("target is no longer a regular file"));
    }
    if fs::read(path)? != expected {
        return Err(io::Error::other(
            "target changed after transaction preflight",
        ));
    }
    Ok(())
}

fn backup_path(root: &Path, source: &Path) -> io::Result<PathBuf> {
    let parent = source
        .parent()
        .ok_or_else(|| io::Error::other("source has no parent"))?;
    let parent_hash = blake3::hash(parent.as_os_str().as_encoded_bytes())
        .to_hex()
        .to_string();
    let name = source
        .file_name()
        .ok_or_else(|| io::Error::other("source has no file name"))?;
    Ok(root.join(&parent_hash[..16]).join(name))
}

fn write_exact_file(path: &Path, bytes: &[u8], metadata: &fs::Metadata) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("backup has no parent"))?;
    fs::create_dir_all(parent)?;
    set_private_directory_permissions(parent)?;
    let mut options = OpenOptions::new();
    options.create_new(true).write(true);
    let mut file = options.open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    apply_metadata(path, metadata)?;
    sync_directory(parent)
}

fn apply_metadata(path: &Path, metadata: &fs::Metadata) -> io::Result<()> {
    fs::set_permissions(path, metadata.permissions())?;
    let accessed = FileTime::from_last_access_time(metadata);
    let modified = FileTime::from_last_modification_time(metadata);
    set_file_times(path, accessed, modified)
}

fn make_journal(options: &TransactionOptions, prepared: &[PreparedFile]) -> Journal {
    Journal {
        schema_version: 1,
        run_id: options.run_id.clone(),
        state: JournalState::Prepared,
        files: prepared
            .iter()
            .map(|file| JournalFile {
                source: file.plan.path.clone(),
                backup: file.backup.clone(),
                original_sha256: sha256_hex(&file.plan.original),
                replacement_blake3: blake3::hash(&file.plan.replacement).to_hex().to_string(),
                committed: false,
            })
            .collect(),
    }
}

fn write_journal(path: &Path, journal: &Journal) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("journal has no parent"))?;
    let mut temporary = NamedTempFile::new_in(parent)?;
    serde_json::to_writer_pretty(temporary.as_file_mut(), journal)?;
    temporary.write_all(b"\n")?;
    temporary.as_file_mut().sync_all()?;
    temporary.persist(path).map_err(|error| error.error)?;
    sync_directory(parent)
}

fn reject_symlink_components(root: &Path, path: &Path) -> Result<(), TransactionError> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| TransactionError::OutsideProject(path.to_path_buf()))?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        match component {
            Component::CurDir => {}
            Component::Prefix(_) | Component::RootDir | Component::ParentDir => {
                return Err(TransactionError::OutsideProject(path.to_path_buf()));
            }
            Component::Normal(value) => {
                current.push(value);
                let metadata =
                    fs::symlink_metadata(&current).map_err(|source| TransactionError::Inspect {
                        path: current.clone(),
                        source,
                    })?;
                if metadata.file_type().is_symlink() {
                    return Err(TransactionError::Symlink(current));
                }
            }
        }
    }
    Ok(())
}

fn reject_symlink_components_allow_missing(
    root: &Path,
    path: &Path,
) -> Result<(), TransactionError> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| TransactionError::OutsideProject(path.to_path_buf()))?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(value) = component else {
            if matches!(component, Component::CurDir) {
                continue;
            }
            return Err(TransactionError::OutsideProject(path.to_path_buf()));
        };
        current.push(value);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(TransactionError::Symlink(current));
            }
            Ok(_) => {}
            Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(source) => {
                return Err(TransactionError::Inspect {
                    path: current,
                    source,
                });
            }
        }
    }
    Ok(())
}

fn regular_file_without_symlink_components(root: &Path, path: &Path) -> bool {
    path.starts_with(root)
        && reject_symlink_components(root, path).is_ok()
        && fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_file())
}

fn resolve_beneath(input_root: &Path, canonical_root: &Path, path: &Path) -> Option<PathBuf> {
    let absolute = absolute_lexical(path);
    if let Ok(relative) = absolute.strip_prefix(input_root) {
        return Some(canonical_root.join(relative));
    }
    absolute.starts_with(canonical_root).then_some(absolute)
}

fn absolute_lexical(path: &Path) -> PathBuf {
    if path.is_absolute() {
        lexical_normalize(path)
    } else {
        lexical_normalize(
            &std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join(path),
        )
    }
}

fn lexical_normalize(path: &Path) -> PathBuf {
    let mut output = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                let _ = output.pop();
            }
            other => output.push(other.as_os_str()),
        }
    }
    output
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Makes a newly created or replaced entry durable.
///
/// POSIX requires the parent directory to be flushed before a create or rename
/// survives a crash, which is why this exists at all.
///
/// Windows has no counterpart. A directory handle cannot even be opened without
/// `FILE_FLAG_BACKUP_SEMANTICS`, and flushing one is not a supported operation —
/// `File::open` on a directory is what made every commit and every backup fail
/// there with "Access is denied". The file's own `sync_all` has already run, and
/// NTFS journals the metadata, so nothing is skipped here that Windows would
/// otherwise have done. What remains different is that a rename is not written
/// through, which `docs/COMPATIBILITY.md` states rather than papers over.
#[cfg(unix)]
fn sync_directory(path: &Path) -> io::Result<()> {
    File::open(path)?.sync_all()
}

#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> io::Result<()> {
    Ok(())
}

#[cfg(unix)]
fn set_private_directory_permissions(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
}

#[cfg(not(unix))]
fn set_private_directory_permissions(_path: &Path) -> io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeSet, fs};

    use normfix_core::FixRecord;
    use tempfile::TempDir;

    use super::{
        PlannedFile, ReadPrecondition, TransactionError, TransactionOptions, UndoError,
        commit_files, commit_files_guarded, list_undo_runs, read_journal, sha256_hex, undo_run,
        validate_committed_replacements, validate_read_preconditions_except, write_journal,
    };

    fn plan(path: &std::path::Path, replacement: &[u8]) -> PlannedFile {
        PlannedFile {
            path: path.to_path_buf(),
            original: fs::read(path).expect("fixture"),
            replacement: replacement.to_vec(),
            fixes: vec![FixRecord {
                rule_id: "TEST".to_owned(),
                description: "test replacement".to_owned(),
                line: Some(1),
                count: 1,
            }],
        }
    }

    #[test]
    fn commits_sorted_files_with_external_backups_and_journal() {
        let project = TempDir::new().expect("project");
        let backups = TempDir::new().expect("backups");
        let a = project.path().join("a.c");
        let b = project.path().join("b.c");
        fs::write(&a, "old a\n").expect("a");
        fs::write(&b, "old b\n").expect("b");
        let options = TransactionOptions {
            project_root: project.path().to_path_buf(),
            run_id: "run-1".to_owned(),
            backup_root: Some(backups.path().to_path_buf()),
        };

        let report = commit_files(vec![plan(&b, b"new b\n"), plan(&a, b"new a\n")], &options)
            .expect("commit");

        assert_eq!(fs::read(&a).expect("a"), b"new a\n");
        assert_eq!(fs::read(&b).expect("b"), b"new b\n");
        assert_eq!(report.files[0].path, a);
        assert_eq!(report.files[1].path, b);
        assert!(report.journal.as_ref().is_some_and(|path| path.is_file()));
        for file in &report.files {
            let backup = file.backup.as_ref().expect("backup");
            assert!(backup.is_file());
        }
        assert_eq!(report.files[0].original_sha256, sha256_hex(b"old a\n"));
    }

    #[test]
    fn concurrent_modification_aborts_before_any_write() {
        let project = TempDir::new().expect("project");
        let a = project.path().join("a.c");
        let b = project.path().join("b.c");
        fs::write(&a, "old a\n").expect("a");
        fs::write(&b, "old b\n").expect("b");
        let first = plan(&a, b"new a\n");
        let second = plan(&b, b"new b\n");
        fs::write(&b, "external\n").expect("external change");
        let options = TransactionOptions {
            project_root: project.path().to_path_buf(),
            run_id: "run-2".to_owned(),
            backup_root: None,
        };

        assert!(matches!(
            commit_files(vec![first, second], &options),
            Err(TransactionError::ConcurrentModification(path)) if path == b
        ));
        assert_eq!(fs::read(&a).expect("a"), b"old a\n");
        assert_eq!(fs::read(&b).expect("b"), b"external\n");
    }

    #[test]
    fn changed_read_precondition_aborts_before_backup_or_write() {
        let project = TempDir::new().expect("project");
        let backups = TempDir::new().expect("backups");
        let target = project.path().join("target.c");
        let observed = project.path().join("public.h");
        fs::write(&target, "old\n").expect("target");
        fs::write(&observed, "old declaration\n").expect("observed file");
        let precondition = ReadPrecondition::matches(&observed, b"old declaration\n");
        fs::write(&observed, "new declaration\n").expect("external change");
        let options = TransactionOptions {
            project_root: project.path().to_path_buf(),
            run_id: "run-read-set".to_owned(),
            backup_root: Some(backups.path().join("normfix")),
        };

        assert!(matches!(
            commit_files_guarded(vec![plan(&target, b"new\n")], &options, &[precondition]),
            Err(TransactionError::ConcurrentModification(path)) if path == observed
        ));
        assert_eq!(fs::read(&target).expect("target"), b"old\n");
        assert!(!backups.path().join("normfix").exists());
    }

    #[test]
    fn newly_present_absence_precondition_aborts_before_write() {
        let project = TempDir::new().expect("project");
        let target = project.path().join("Makefile");
        let missing = project.path().join("missing.c");
        fs::write(&target, "SRCS = missing.c\n").expect("Makefile");
        let precondition = ReadPrecondition::absent(&missing);
        fs::write(&missing, "int main(void) { return (0); }\n").expect("new source");
        let options = TransactionOptions {
            project_root: project.path().to_path_buf(),
            run_id: "run-absence".to_owned(),
            backup_root: None,
        };

        assert!(matches!(
            commit_files_guarded(vec![plan(&target, b"SRCS =\n")], &options, &[precondition]),
            Err(TransactionError::ConcurrentModification(path)) if path == missing
        ));
        assert_eq!(fs::read(&target).expect("Makefile"), b"SRCS = missing.c\n");
    }

    #[test]
    fn new_project_source_aborts_a_closed_world_commit() {
        let project = TempDir::new().expect("project");
        let target = project.path().join("main.c");
        let header = project.path().join("public.h");
        fs::write(&target, "int main(void) { return (0); }\n").expect("target");
        fs::write(&header, "int old_api(void);\n").expect("header");
        let precondition =
            ReadPrecondition::project_sources(project.path(), vec![target.clone(), header.clone()]);
        let introduced = project.path().join("late.c");
        fs::write(&introduced, "int old_api(void) { return (1); }\n").expect("late source");
        let options = TransactionOptions {
            project_root: project.path().to_path_buf(),
            run_id: "run-project-membership".to_owned(),
            backup_root: None,
        };

        assert!(matches!(
            commit_files_guarded(
                vec![plan(&header, b"\n")],
                &options,
                &[precondition],
            ),
            Err(TransactionError::ConcurrentModification(path)) if path == introduced
        ));
        assert_eq!(fs::read(&header).expect("header"), b"int old_api(void);\n");
    }

    #[test]
    fn project_source_membership_is_rechecked_after_a_prior_replacement() {
        let project = TempDir::new().expect("project");
        let first = project.path().join("first.c");
        let second = project.path().join("second.h");
        fs::write(&first, "int first(void) { return (1); }\n").expect("first");
        fs::write(&second, "int second(void);\n").expect("second");
        let precondition =
            ReadPrecondition::project_sources(project.path(), vec![first.clone(), second.clone()]);
        let mut prepared =
            super::prepare_file(plan(&first, b"int first(void) { return (2); }\n"), None)
                .expect("prepare first replacement");
        super::persist_staged(&mut prepared).expect("commit first replacement");
        let introduced = project.path().join("late.h");
        fs::write(&introduced, "int late(void);\n").expect("late header");
        let canonical = project.path().canonicalize().expect("canonical project");

        assert!(matches!(
            validate_read_preconditions_except(
                &[precondition],
                project.path(),
                &canonical,
                &BTreeSet::new(),
            ),
            Err(TransactionError::ConcurrentModification(path)) if path == introduced
        ));
    }

    #[test]
    fn project_snapshot_preconditions_allow_a_multi_file_commit() {
        let project = TempDir::new().expect("project");
        let first = project.path().join("first.h");
        let second = project.path().join("second.h");
        fs::write(&first, "#ifndef FIRST\n#define FIRST\n#endif\n").expect("first");
        fs::write(&second, "#ifndef SECOND\n#define SECOND\n#endif\n").expect("second");
        let preconditions = [
            ReadPrecondition::matches(&first, b"#ifndef FIRST\n#define FIRST\n#endif\n"),
            ReadPrecondition::matches(&second, b"#ifndef SECOND\n#define SECOND\n#endif\n"),
        ];
        let options = TransactionOptions {
            project_root: project.path().to_path_buf(),
            run_id: "run-multi-read-set".to_owned(),
            backup_root: None,
        };

        commit_files_guarded(
            vec![
                plan(&first, b"#ifndef FIRST_H\n#define FIRST_H\n#endif\n"),
                plan(&second, b"#ifndef SECOND_H\n#define SECOND_H\n#endif\n"),
            ],
            &options,
            &preconditions,
        )
        .expect("multi-file guarded commit");

        assert_eq!(
            fs::read(&first).expect("first"),
            b"#ifndef FIRST_H\n#define FIRST_H\n#endif\n"
        );
        assert_eq!(
            fs::read(&second).expect("second"),
            b"#ifndef SECOND_H\n#define SECOND_H\n#endif\n"
        );
    }

    #[test]
    fn a_committed_target_must_still_match_before_the_next_replacement() {
        let project = TempDir::new().expect("project");
        let first = project.path().join("first.h");
        fs::write(&first, "old\n").expect("first");
        let mut prepared =
            super::prepare_file(plan(&first, b"replacement\n"), None).expect("prepare replacement");
        super::persist_staged(&mut prepared).expect("commit first replacement");
        fs::write(&first, "concurrent writer\n").expect("concurrent change");

        let error = validate_committed_replacements(&[prepared], &[0])
            .expect_err("the changed committed target must be detected");

        assert_eq!(error.0, first);
        assert!(
            error
                .1
                .to_string()
                .contains("changed after transaction preflight")
        );
    }

    #[test]
    fn rejects_symlinks_and_backup_storage_inside_project() {
        let project = TempDir::new().expect("project");
        let source = project.path().join("main.c");
        fs::write(&source, "old\n").expect("source");
        let options = TransactionOptions {
            project_root: project.path().to_path_buf(),
            run_id: "run-3".to_owned(),
            backup_root: Some(project.path().join("backups")),
        };
        assert!(matches!(
            commit_files(vec![plan(&source, b"new\n")], &options),
            Err(TransactionError::BackupInsideProject(_))
        ));

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;

            let link = project.path().join("link.c");
            symlink(&source, &link).expect("symlink");
            let options = TransactionOptions {
                project_root: project.path().to_path_buf(),
                run_id: "run-4".to_owned(),
                backup_root: None,
            };
            let linked_plan = PlannedFile {
                path: link,
                original: b"old\n".to_vec(),
                replacement: b"new\n".to_vec(),
                fixes: Vec::new(),
            };
            assert!(matches!(
                commit_files(vec![linked_plan], &options),
                Err(TransactionError::Symlink(_))
            ));

            let external = TempDir::new().expect("external backup parent");
            let redirected = project.path().join("redirected-backups");
            fs::create_dir(&redirected).expect("inside-project backup target");
            let backup_link = external.path().join("backup-link");
            symlink(&redirected, &backup_link).expect("backup symlink");
            let options = TransactionOptions {
                project_root: project.path().to_path_buf(),
                run_id: "run-5".to_owned(),
                backup_root: Some(backup_link),
            };
            assert!(matches!(
                commit_files(vec![plan(&source, b"new\n")], &options),
                Err(TransactionError::Symlink(_) | TransactionError::BackupInsideProject(_))
            ));
        }
    }

    #[test]
    fn lists_and_undoes_the_latest_intact_transaction() {
        let project = TempDir::new().expect("project");
        let backups = TempDir::new().expect("backups");
        let source = project.path().join("main.c");
        fs::write(&source, "old\n").expect("source");
        let options = TransactionOptions {
            project_root: project.path().to_path_buf(),
            run_id: "run-undo-test".to_owned(),
            backup_root: Some(backups.path().to_path_buf()),
        };
        commit_files(vec![plan(&source, b"new\n")], &options).expect("commit");

        let runs = list_undo_runs(backups.path(), project.path()).expect("runs");
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].run_id, "run-undo-test");
        let report = undo_run(&runs[0], project.path(), backups.path()).expect("undo");

        assert_eq!(fs::read(&source).expect("restored"), b"old\n");
        assert_eq!(report.files, vec![source]);
        assert!(report.journal.is_some());
    }

    #[test]
    fn undo_refuses_to_overwrite_changes_made_after_the_run() {
        let project = TempDir::new().expect("project");
        let backups = TempDir::new().expect("backups");
        let source = project.path().join("main.c");
        fs::write(&source, "old\n").expect("source");
        let options = TransactionOptions {
            project_root: project.path().to_path_buf(),
            run_id: "run-changed-test".to_owned(),
            backup_root: Some(backups.path().to_path_buf()),
        };
        commit_files(vec![plan(&source, b"new\n")], &options).expect("commit");
        let run = list_undo_runs(backups.path(), project.path())
            .expect("runs")
            .pop()
            .expect("run");
        fs::write(&source, "student edit\n").expect("external edit");

        assert!(matches!(
            undo_run(&run, project.path(), backups.path()),
            Err(UndoError::ModifiedSinceRun(path)) if path == source
        ));
        assert_eq!(fs::read(&source).expect("unchanged"), b"student edit\n");
    }

    #[cfg(unix)]
    #[test]
    fn backup_root_uses_the_canonical_target_of_a_compatibility_symlink() {
        use std::os::unix::fs::symlink;

        let project = TempDir::new().expect("project");
        let storage = TempDir::new().expect("storage");
        let real_backups = storage.path().join("real-backups");
        let compatibility_path = storage.path().join("compat-backups");
        fs::create_dir(&real_backups).expect("real backups");
        symlink(&real_backups, &compatibility_path).expect("compatibility symlink");
        let source = project.path().join("main.c");
        fs::write(&source, "old\n").expect("source");
        let options = TransactionOptions {
            project_root: project.path().to_path_buf(),
            run_id: "run-compat-link".to_owned(),
            backup_root: Some(compatibility_path.clone()),
        };

        let report = commit_files(vec![plan(&source, b"new\n")], &options).expect("commit");
        let canonical_backups = real_backups.canonicalize().expect("canonical backups");
        let journal = report.journal.expect("journal");
        assert!(journal.starts_with(&canonical_backups));

        fs::remove_file(&compatibility_path).expect("remove compatibility symlink");
        symlink(project.path(), &compatibility_path).expect("redirect compatibility path");
        assert!(journal.is_file());
        assert!(!project.path().join("run-compat-link").exists());
    }

    #[cfg(unix)]
    #[test]
    fn listing_and_undo_reject_a_symlinked_backup_parent_added_after_confirmation() {
        use std::os::unix::fs::symlink;

        let project = TempDir::new().expect("project");
        let backups = TempDir::new().expect("backups");
        let source = project.path().join("main.c");
        fs::write(&source, "old\n").expect("source");
        let options = TransactionOptions {
            project_root: project.path().to_path_buf(),
            run_id: "run-intermediate-link".to_owned(),
            backup_root: Some(backups.path().to_path_buf()),
        };
        commit_files(vec![plan(&source, b"new\n")], &options).expect("commit");
        let run = list_undo_runs(backups.path(), project.path())
            .expect("runs")
            .pop()
            .expect("run");
        let journal = read_journal(&run.journal).expect("journal");
        let backup = journal.files[0].backup.as_ref().expect("backup");
        let backup_parent = backup.parent().expect("backup parent");
        let relocated = backup_parent.with_file_name("relocated-backup-parent");
        fs::rename(backup_parent, &relocated).expect("relocate backup parent");
        symlink(&relocated, backup_parent).expect("intermediate symlink");

        assert!(
            list_undo_runs(backups.path(), project.path())
                .expect("runs")
                .is_empty()
        );
        assert!(matches!(
            undo_run(&run, project.path(), backups.path()),
            Err(UndoError::InvalidJournal { .. })
        ));
        assert_eq!(fs::read(&source).expect("unchanged"), b"new\n");
    }

    #[test]
    fn undo_rejects_a_confirmed_run_whose_advertised_source_set_changed() {
        let project = TempDir::new().expect("project");
        let backups = TempDir::new().expect("backups");
        let source = project.path().join("main.c");
        fs::write(&source, "old\n").expect("source");
        let options = TransactionOptions {
            project_root: project.path().to_path_buf(),
            run_id: "run-source-set".to_owned(),
            backup_root: Some(backups.path().to_path_buf()),
        };
        commit_files(vec![plan(&source, b"new\n")], &options).expect("commit");
        let mut run = list_undo_runs(backups.path(), project.path())
            .expect("runs")
            .pop()
            .expect("run");
        run.files.push(project.path().join("unconfirmed.c"));

        assert!(matches!(
            undo_run(&run, project.path(), backups.path()),
            Err(UndoError::InvalidJournal { .. })
        ));
        assert_eq!(fs::read(&source).expect("unchanged"), b"new\n");
    }

    #[test]
    fn undo_reloads_the_journal_and_rechecks_backup_confinement_and_hashes() {
        let project = TempDir::new().expect("project");
        let backups = TempDir::new().expect("backups");
        let source = project.path().join("main.c");
        fs::write(&source, "old\n").expect("source");
        let options = TransactionOptions {
            project_root: project.path().to_path_buf(),
            run_id: "run-revalidate".to_owned(),
            backup_root: Some(backups.path().to_path_buf()),
        };
        commit_files(vec![plan(&source, b"new\n")], &options).expect("commit");
        let run = list_undo_runs(backups.path(), project.path())
            .expect("runs")
            .pop()
            .expect("run");
        let mut journal = read_journal(&run.journal).expect("journal");
        let outside = backups.path().join("outside-run.backup");
        fs::write(&outside, "old\n").expect("outside backup");
        journal.files[0].backup = Some(outside);
        write_journal(&run.journal, &journal).expect("replace journal");

        assert!(matches!(
            undo_run(&run, project.path(), backups.path()),
            Err(UndoError::InvalidJournal { .. })
        ));
        assert_eq!(fs::read(&source).expect("unchanged"), b"new\n");
    }

    #[test]
    fn undo_rechecks_backup_digest_after_the_run_was_listed() {
        let project = TempDir::new().expect("project");
        let backups = TempDir::new().expect("backups");
        let source = project.path().join("main.c");
        fs::write(&source, "old\n").expect("source");
        let options = TransactionOptions {
            project_root: project.path().to_path_buf(),
            run_id: "run-backup-hash".to_owned(),
            backup_root: Some(backups.path().to_path_buf()),
        };
        commit_files(vec![plan(&source, b"new\n")], &options).expect("commit");
        let run = list_undo_runs(backups.path(), project.path())
            .expect("runs")
            .pop()
            .expect("run");
        let journal = read_journal(&run.journal).expect("journal");
        let backup = journal.files[0].backup.as_ref().expect("backup");
        fs::write(backup, "tampered\n").expect("tamper backup");

        assert!(matches!(
            undo_run(&run, project.path(), backups.path()),
            Err(UndoError::BackupIntegrity(path)) if path == *backup
        ));
        assert_eq!(fs::read(&source).expect("unchanged"), b"new\n");
    }
}
