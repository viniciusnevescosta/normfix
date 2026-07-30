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
    let (mut prepared, backup_run) = prepare_transaction(plans, options)?;
    let (mut journal, journal_path) = begin_journal(options, &prepared, backup_run.as_deref())?;
    let committed_indices = commit_prepared_files(
        &mut prepared,
        &mut journal,
        journal_path.as_deref(),
        options,
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

fn prepare_transaction(
    plans: Vec<PlannedFile>,
    options: &TransactionOptions,
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
    let backup_run = prepare_backup_root(options, &root_input, &root)?;
    for plan in &plans {
        preflight(plan, &root_input, &root)?;
    }
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
) -> Result<Vec<usize>, TransactionError> {
    let mut committed_indices = Vec::new();
    for index in 0..prepared.len() {
        let target = prepared[index].plan.path.clone();
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
        if let Some(path) = journal_path
            && let Err(source) = write_journal(path, journal)
        {
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
    Ok(committed_indices)
}

fn finish_journal(
    prepared: &[PreparedFile],
    committed_indices: &[usize],
    journal: &mut Journal,
    journal_path: Option<&Path>,
    options: &TransactionOptions,
) -> Result<(), TransactionError> {
    journal.state = JournalState::Committed;
    if let Some(path) = journal_path
        && let Err(source) = write_journal(path, journal)
    {
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
    let run = absolute.join(&options.run_id);
    fs::create_dir_all(&run).map_err(|source| TransactionError::Prepare {
        path: run.clone(),
        source,
    })?;
    set_private_directory_permissions(&run).map_err(|source| TransactionError::Prepare {
        path: run.clone(),
        source,
    })?;
    Ok(Some(run))
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
        if restore_exact(&file.plan.path, &file.plan.original, &file.metadata).is_err() {
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

fn sync_directory(path: &Path) -> io::Result<()> {
    File::open(path)?.sync_all()
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
    use std::fs;

    use normfix_core::FixRecord;
    use tempfile::TempDir;

    use super::{PlannedFile, TransactionError, TransactionOptions, commit_files, sha256_hex};

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
        }
    }
}
