//! Filesystem confinement, bounded validation, staging, and durability helpers.

use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read as _, Write as _};
use std::path::{Component, Path, PathBuf};

use filetime::{FileTime, set_file_times};
use sha2::{Digest, Sha256};
use tempfile::NamedTempFile;

use super::{
    Journal, JournalFile, JournalState, PlannedFile, PreparedFile, ReadPrecondition,
    TransactionError, TransactionOptions,
};

pub(super) fn canonical_directory(path: &Path) -> Result<PathBuf, TransactionError> {
    path.canonicalize()
        .map_err(|source| TransactionError::Inspect {
            path: path.to_path_buf(),
            source,
        })
}

pub(super) fn prepare_backup_root(
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

pub(super) fn create_canonical_backup_base(
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

pub(super) fn preflight(
    plan: &PlannedFile,
    root_input: &Path,
    canonical_root: &Path,
) -> Result<(), TransactionError> {
    let (absolute, resolved) = resolve_transaction_path(&plan.path, root_input, canonical_root)?;
    reject_symlink_components(canonical_root, &resolved)?;
    let metadata = fs::symlink_metadata(&resolved).map_err(|source| TransactionError::Inspect {
        path: absolute.clone(),
        source,
    })?;
    if !metadata.file_type().is_file() {
        return Err(TransactionError::NotRegularFile(absolute));
    }
    let matches = file_equals_bytes(&resolved, &plan.original).map_err(|source| {
        TransactionError::Inspect {
            path: absolute.clone(),
            source,
        }
    })?;
    if !matches {
        return Err(TransactionError::ConcurrentModification(absolute));
    }
    Ok(())
}

pub(super) fn validate_read_preconditions(
    preconditions: &[ReadPrecondition],
    root_input: &Path,
    canonical_root: &Path,
) -> Result<(), TransactionError> {
    validate_read_preconditions_except(preconditions, root_input, canonical_root, &BTreeSet::new())
}

pub(super) fn validate_read_preconditions_except(
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
        let (_, resolved_precondition) =
            resolve_transaction_path(path, root_input, canonical_root)?;
        if ignored_paths.contains(&absolute_lexical(&resolved_precondition)) {
            continue;
        }
        match precondition {
            ReadPrecondition::Matches { path, blake3 } => {
                let (absolute, resolved) =
                    resolve_transaction_path(path, root_input, canonical_root)?;
                reject_symlink_components(canonical_root, &resolved)?;
                let metadata = fs::symlink_metadata(&resolved).map_err(|source| {
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
                let digest =
                    file_blake3(&resolved).map_err(|source| TransactionError::Inspect {
                        path: absolute.clone(),
                        source,
                    })?;
                if digest.as_bytes() != blake3 {
                    return Err(TransactionError::ConcurrentModification(absolute));
                }
            }
            ReadPrecondition::Absent { path } => {
                let (absolute, resolved) =
                    resolve_transaction_path(path, root_input, canonical_root)?;
                reject_symlink_components_allow_missing(canonical_root, &resolved)?;
                match fs::symlink_metadata(&resolved) {
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

pub(super) fn validate_project_source_membership(
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
    let metadata = fs::symlink_metadata(&resolved_root).map_err(|source| {
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
            || !expected.insert(absolute_lexical(&resolved))
        {
            return Err(TransactionError::ConcurrentModification(absolute));
        }
    }
    let actual =
        collect_project_sources(&resolved_root).map_err(|source| TransactionError::Inspect {
            path: absolute_root.clone(),
            source,
        })?;
    if actual != expected {
        let changed = actual
            .symmetric_difference(&expected)
            .next()
            .cloned()
            .and_then(|path| {
                path.strip_prefix(&resolved_root)
                    .ok()
                    .map(|relative| absolute_root.join(relative))
            })
            .unwrap_or(absolute_root);
        return Err(TransactionError::ConcurrentModification(changed));
    }
    Ok(())
}

pub(super) fn collect_project_sources(root: &Path) -> io::Result<BTreeSet<PathBuf>> {
    let mut sources = BTreeSet::new();
    let mut pending = vec![root.to_path_buf()];
    let mut visited = 0_usize;
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory)? {
            let entry = entry?;
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

pub(super) fn resolve_transaction_path(
    path: &Path,
    root_input: &Path,
    canonical_root: &Path,
) -> Result<(PathBuf, PathBuf), TransactionError> {
    let absolute = absolute_lexical(path);
    let resolved = if let Ok(relative) = absolute.strip_prefix(root_input) {
        canonical_root.join(relative)
    } else if absolute.starts_with(canonical_root) {
        absolute.clone()
    } else if let Some(mapped) = map_from_equivalent_root(&absolute, canonical_root) {
        mapped
    } else {
        return Err(TransactionError::OutsideProject(absolute));
    };
    Ok((absolute, resolved))
}

/// Maps a path written through an operating-system alias of the project root
/// (for example macOS `/var` versus `/private/var`) without following any
/// component below that root.
fn map_from_equivalent_root(path: &Path, canonical_root: &Path) -> Option<PathBuf> {
    let mut ancestor = path.to_path_buf();
    let mut suffix = Vec::new();
    loop {
        if ancestor
            .canonicalize()
            .is_ok_and(|resolved| resolved == canonical_root)
        {
            let mut mapped = canonical_root.to_path_buf();
            for component in suffix.iter().rev() {
                mapped.push(component);
            }
            return Some(mapped);
        }
        let name = ancestor.file_name()?.to_os_string();
        suffix.push(name);
        if !ancestor.pop() {
            return None;
        }
    }
}

pub(super) fn prepare_file_at(
    plan: PlannedFile,
    target_path: PathBuf,
    backup_run: Option<&Path>,
) -> Result<PreparedFile, TransactionError> {
    let metadata = fs::metadata(&target_path).map_err(|source| TransactionError::Prepare {
        path: plan.path.clone(),
        source,
    })?;
    let backup = backup_run
        .map(|root| backup_path(root, &target_path))
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
    let parent = target_path
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
        target_path,
        staged,
        backup,
        metadata,
    })
}

pub(super) fn persist_staged(prepared: &mut PreparedFile) -> io::Result<()> {
    preflight_bytes(&prepared.target_path, &prepared.plan.original)?;
    let parent = prepared
        .target_path
        .parent()
        .ok_or_else(|| io::Error::other("target has no parent directory"))?;
    let staged = std::mem::replace(&mut prepared.staged, NamedTempFile::new_in(parent)?);
    staged
        .persist(&prepared.target_path)
        .map_err(|error| error.error)?;
    sync_directory(parent)
}

pub(super) fn rollback(prepared: &[PreparedFile], committed_indices: &[usize]) -> bool {
    let mut complete = true;
    for index in committed_indices.iter().rev() {
        let file = &prepared[*index];
        if preflight_bytes(&file.target_path, &file.plan.replacement).is_err()
            || restore_exact(&file.target_path, &file.plan.original, &file.metadata).is_err()
        {
            complete = false;
        }
    }
    complete
}

pub(super) fn restore_exact(path: &Path, bytes: &[u8], metadata: &fs::Metadata) -> io::Result<()> {
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

pub(super) fn preflight_bytes(path: &Path, expected: &[u8]) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_file() {
        return Err(io::Error::other("target is no longer a regular file"));
    }
    if !file_equals_bytes(path, expected)? {
        return Err(io::Error::other(
            "target changed after transaction preflight",
        ));
    }
    Ok(())
}

pub(super) fn backup_path(root: &Path, source: &Path) -> io::Result<PathBuf> {
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

pub(super) fn write_exact_file(
    path: &Path,
    bytes: &[u8],
    metadata: &fs::Metadata,
) -> io::Result<()> {
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

pub(super) fn apply_metadata(path: &Path, metadata: &fs::Metadata) -> io::Result<()> {
    fs::set_permissions(path, metadata.permissions())?;
    let accessed = FileTime::from_last_access_time(metadata);
    let modified = FileTime::from_last_modification_time(metadata);
    set_file_times(path, accessed, modified)
}

pub(super) fn make_journal(options: &TransactionOptions, prepared: &[PreparedFile]) -> Journal {
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

pub(super) fn write_journal(path: &Path, journal: &Journal) -> io::Result<()> {
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

pub(super) fn reject_symlink_components(root: &Path, path: &Path) -> Result<(), TransactionError> {
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

pub(super) fn reject_symlink_components_allow_missing(
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

pub(super) fn regular_file_without_symlink_components(root: &Path, path: &Path) -> bool {
    path.starts_with(root)
        && reject_symlink_components(root, path).is_ok()
        && fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_file())
}

pub(super) fn resolve_beneath(
    input_root: &Path,
    canonical_root: &Path,
    path: &Path,
) -> Option<PathBuf> {
    let absolute = absolute_lexical(path);
    if let Ok(relative) = absolute.strip_prefix(input_root) {
        return Some(canonical_root.join(relative));
    }
    absolute.starts_with(canonical_root).then_some(absolute)
}

pub(super) fn absolute_lexical(path: &Path) -> PathBuf {
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

pub(super) fn lexical_normalize(path: &Path) -> PathBuf {
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

pub(super) fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub(super) fn file_blake3(path: &Path) -> io::Result<blake3::Hash> {
    let mut input = File::open(path)?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        let read = input.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher.finalize())
}

pub(super) fn file_sha256_hex(path: &Path) -> io::Result<String> {
    let mut input = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        let read = input.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

pub(super) fn file_equals_bytes(path: &Path, expected: &[u8]) -> io::Result<bool> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_file()
        || metadata.len() != u64::try_from(expected.len()).unwrap_or(u64::MAX)
    {
        return Ok(false);
    }
    let mut input = File::open(path)?;
    let mut offset = 0_usize;
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        let read = input.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        let Some(end) = offset.checked_add(read) else {
            return Ok(false);
        };
        if expected.get(offset..end) != Some(&buffer[..read]) {
            return Ok(false);
        }
        offset = end;
    }
    Ok(offset == expected.len())
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
pub(super) fn sync_directory(path: &Path) -> io::Result<()> {
    File::open(path)?.sync_all()
}

#[cfg(not(unix))]
pub(super) fn sync_directory(_path: &Path) -> io::Result<()> {
    Ok(())
}

#[cfg(unix)]
pub(super) fn set_private_directory_permissions(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
}

#[cfg(not(unix))]
pub(super) fn set_private_directory_permissions(_path: &Path) -> io::Result<()> {
    Ok(())
}
