use std::fs::{self, File, Metadata, OpenOptions};
use std::path::{Path, PathBuf};

use redb::{
    CommitError, Database, DatabaseError, ReadableTableMetadata, StorageError, TableDefinition,
    TableError, TransactionError,
};
use serde::de::DeserializeOwned;

use crate::encoding::{decode_record, validate_record_length};
use crate::paths::{CachePaths, paths_overlap};
use crate::{CacheKey, CacheOpenStatus, PreparedCacheEntry};

const CACHE_TABLE: TableDefinition<&[u8], &[u8]> = TableDefinition::new("validated_results_v1");

pub(super) fn open_or_recover(paths: &CachePaths) -> (Option<Database>, CacheOpenStatus) {
    if let Err(reason) = prepare_cache_parent(paths) {
        return (None, CacheOpenStatus::Disabled { reason });
    }
    match create_initialized_database(&paths.database) {
        Ok(database) => (Some(database), CacheOpenStatus::Ready),
        Err(OpenFailure::Busy(reason) | OpenFailure::Io(reason)) => {
            (None, CacheOpenStatus::Disabled { reason })
        }
        Err(OpenFailure::Corrupt(reason)) => match quarantine_and_create(paths) {
            Ok((database, quarantined)) => (
                Some(database),
                CacheOpenStatus::Recreated {
                    quarantined,
                    reason,
                },
            ),
            Err(recovery_error) => (
                None,
                CacheOpenStatus::Disabled {
                    reason: format!(
                        "cache was corrupt ({reason}) and recovery failed: {recovery_error}"
                    ),
                },
            ),
        },
    }
}

fn prepare_cache_parent(paths: &CachePaths) -> Result<(), String> {
    let parent = paths
        .database
        .parent()
        .ok_or_else(|| "cache database has no parent directory".to_owned())?;
    create_private_directories(parent)
        .map_err(|error| format!("could not create cache directory: {error}"))?;
    let resolved = fs::canonicalize(parent)
        .map_err(|error| format!("could not verify cache directory: {error}"))?;
    if resolved != parent {
        return Err(format!(
            "cache directory changed while it was being created: expected `{}`, found `{}`",
            parent.display(),
            resolved.display()
        ));
    }
    if paths_overlap(&resolved, &paths.project_root) {
        return Err(format!(
            "cache directory `{}` overlaps project root `{}`",
            resolved.display(),
            paths.project_root.display()
        ));
    }
    let metadata = fs::symlink_metadata(&resolved)
        .map_err(|error| format!("could not inspect cache directory: {error}"))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(format!(
            "cache directory `{}` is not a real directory",
            resolved.display()
        ));
    }
    Ok(())
}

#[cfg(unix)]
fn create_private_directories(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;

    let mut builder = fs::DirBuilder::new();
    builder.recursive(true).mode(0o700).create(path)
}

#[cfg(not(unix))]
fn create_private_directories(path: &Path) -> std::io::Result<()> {
    fs::create_dir_all(path)
}

enum OpenFailure {
    Busy(String),
    Corrupt(String),
    Io(String),
}

fn create_initialized_database(path: &Path) -> Result<Database, OpenFailure> {
    let file = open_verified_database_file(path)?;
    let database = Database::builder()
        .create_file(file)
        .map_err(|error| classify_open_error(&error))?;
    let transaction = database
        .begin_write()
        .map_err(|error| runtime_to_open_failure(classify_transaction_error(&error)))?;
    transaction
        .open_table(CACHE_TABLE)
        .map_err(|error| runtime_to_open_failure(classify_table_error(&error)))?;
    transaction
        .commit()
        .map_err(|error| runtime_to_open_failure(classify_commit_error(&error)))?;
    Ok(database)
}

fn open_verified_database_file(path: &Path) -> Result<File, OpenFailure> {
    validate_existing_database_target(path)?;
    let file = open_database_file(path).map_err(|error| OpenFailure::Io(error.to_string()))?;
    let opened = file
        .metadata()
        .map_err(|error| OpenFailure::Io(format!("could not inspect open cache file: {error}")))?;
    let linked = fs::symlink_metadata(path).map_err(|error| {
        OpenFailure::Io(format!("could not verify cache database path: {error}"))
    })?;
    if linked.file_type().is_symlink() || !linked.is_file() {
        return Err(OpenFailure::Io(format!(
            "cache database `{}` is not a regular, non-symlink file",
            path.display()
        )));
    }
    if !same_file(&opened, &linked) {
        return Err(OpenFailure::Io(format!(
            "cache database `{}` changed while it was being opened",
            path.display()
        )));
    }
    Ok(file)
}

fn validate_existing_database_target(path: &Path) -> Result<(), OpenFailure> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            Err(OpenFailure::Io(format!(
                "cache database `{}` is not a regular, non-symlink file",
                path.display()
            )))
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(OpenFailure::Io(format!(
            "could not inspect cache database `{}`: {error}",
            path.display()
        ))),
    }
}

#[cfg(unix)]
fn open_database_file(path: &Path) -> std::io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt;

    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(path)
}

#[cfg(windows)]
fn open_database_file(path: &Path) -> std::io::Result<File> {
    use std::os::windows::fs::OpenOptionsExt;

    // Prevent a path replacement race from making this handle follow a
    // reparse point after the pre-open symlink check.
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
}

#[cfg(not(any(unix, windows)))]
fn open_database_file(path: &Path) -> std::io::Result<File> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
}

#[cfg(unix)]
fn same_file(opened: &Metadata, linked: &Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;

    opened.dev() == linked.dev() && opened.ino() == linked.ino()
}

#[cfg(windows)]
fn same_file(opened: &Metadata, linked: &Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    opened.file_attributes() == linked.file_attributes()
        && opened.creation_time() == linked.creation_time()
        && opened.last_write_time() == linked.last_write_time()
        && opened.file_size() == linked.file_size()
}

#[cfg(not(any(unix, windows)))]
fn same_file(opened: &Metadata, linked: &Metadata) -> bool {
    opened.len() == linked.len() && opened.permissions() == linked.permissions()
}

fn classify_open_error(error: &DatabaseError) -> OpenFailure {
    match error {
        DatabaseError::DatabaseAlreadyOpen => OpenFailure::Busy(error.to_string()),
        DatabaseError::Storage(StorageError::Io(source))
            if source.kind() == std::io::ErrorKind::InvalidData =>
        {
            OpenFailure::Corrupt(error.to_string())
        }
        DatabaseError::Storage(StorageError::Io(_)) => OpenFailure::Io(error.to_string()),
        _ => OpenFailure::Corrupt(error.to_string()),
    }
}

fn runtime_to_open_failure(failure: RuntimeFailure) -> OpenFailure {
    match failure {
        RuntimeFailure::Corrupt(reason) => OpenFailure::Corrupt(reason),
        RuntimeFailure::Bypass(reason) => OpenFailure::Io(reason),
    }
}

pub(super) fn quarantine_and_create(paths: &CachePaths) -> Result<(Database, PathBuf), String> {
    let quarantined = quarantine_database(&paths.database)?;
    create_initialized_database(&paths.database)
        .map(|database| (database, quarantined))
        .map_err(|error| match error {
            OpenFailure::Busy(reason) | OpenFailure::Corrupt(reason) | OpenFailure::Io(reason) => {
                reason
            }
        })
}

fn quarantine_database(database: &Path) -> Result<PathBuf, String> {
    let metadata = fs::symlink_metadata(database)
        .map_err(|error| format!("could not inspect corrupt cache: {error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("corrupt cache is not a regular, non-symlink file".to_owned());
    }
    for sequence in 1u32..=u32::MAX {
        let candidate = database.with_extension(format!("redb.corrupt-{sequence}"));
        match fs::hard_link(database, &candidate) {
            Ok(()) => {
                if let Err(error) = fs::remove_file(database) {
                    let _ = fs::remove_file(&candidate);
                    return Err(format!("could not detach corrupt cache: {error}"));
                }
                return Ok(candidate);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(format!("could not quarantine cache: {error}")),
        }
    }
    Err("could not allocate a unique corrupt-cache path".to_owned())
}

#[derive(Debug)]
pub(super) enum RuntimeFailure {
    Corrupt(String),
    Bypass(String),
}

pub(super) fn read_record<T: DeserializeOwned>(
    database: &Database,
    key: CacheKey,
) -> Result<Option<T>, RuntimeFailure> {
    let transaction = database
        .begin_read()
        .map_err(|error| classify_transaction_error(&error))?;
    let table = match transaction.open_table(CACHE_TABLE) {
        Ok(table) => table,
        Err(TableError::TableDoesNotExist(_)) => return Ok(None),
        Err(error) => return Err(classify_table_error(&error)),
    };
    match table.get(key.as_bytes().as_slice()) {
        Ok(Some(guard)) => {
            let value = guard.value();
            validate_record_length(value.len()).map_err(RuntimeFailure::Corrupt)?;
            decode_record(key, value)
                .map(Some)
                .map_err(RuntimeFailure::Corrupt)
        }
        Ok(None) => Ok(None),
        Err(error) => Err(classify_storage_error(&error)),
    }
}

pub(super) fn record_count(database: &Database) -> Result<usize, RuntimeFailure> {
    let transaction = database
        .begin_read()
        .map_err(|error| classify_transaction_error(&error))?;
    let table = match transaction.open_table(CACHE_TABLE) {
        Ok(table) => table,
        Err(TableError::TableDoesNotExist(_)) => return Ok(0),
        Err(error) => return Err(classify_table_error(&error)),
    };
    usize::try_from(
        table
            .len()
            .map_err(|error| classify_storage_error(&error))?,
    )
    .map_err(|_| RuntimeFailure::Bypass("cache record count exceeds usize".to_owned()))
}

pub(super) fn write_records(
    database: &Database,
    entries: &[PreparedCacheEntry],
) -> Result<(), RuntimeFailure> {
    for entry in entries {
        validate_record_length(entry.encoded.len()).map_err(RuntimeFailure::Bypass)?;
    }
    let transaction = database
        .begin_write()
        .map_err(|error| classify_transaction_error(&error))?;
    {
        let mut table = transaction
            .open_table(CACHE_TABLE)
            .map_err(|error| classify_table_error(&error))?;
        for entry in entries {
            table
                .insert(entry.key.as_bytes().as_slice(), entry.encoded.as_slice())
                .map_err(|error| classify_storage_error(&error))?;
        }
    }
    transaction
        .commit()
        .map_err(|error| classify_commit_error(&error))
}

fn classify_transaction_error(error: &TransactionError) -> RuntimeFailure {
    match error {
        TransactionError::Storage(storage) => classify_storage_error(storage),
        _ => RuntimeFailure::Bypass(error.to_string()),
    }
}

fn classify_commit_error(error: &CommitError) -> RuntimeFailure {
    match error {
        CommitError::Storage(storage) => classify_storage_error(storage),
        _ => RuntimeFailure::Bypass(error.to_string()),
    }
}

fn classify_table_error(error: &TableError) -> RuntimeFailure {
    match error {
        TableError::Storage(storage) => classify_storage_error(storage),
        TableError::TableTypeMismatch { .. }
        | TableError::TableIsMultimap(_)
        | TableError::TableIsNotMultimap(_)
        | TableError::TypeDefinitionChanged { .. } => RuntimeFailure::Corrupt(error.to_string()),
        _ => RuntimeFailure::Bypass(error.to_string()),
    }
}

fn classify_storage_error(error: &StorageError) -> RuntimeFailure {
    match error {
        StorageError::Corrupted(_) => RuntimeFailure::Corrupt(error.to_string()),
        StorageError::Io(source) if source.kind() == std::io::ErrorKind::InvalidData => {
            RuntimeFailure::Corrupt(error.to_string())
        }
        _ => RuntimeFailure::Bypass(error.to_string()),
    }
}

#[cfg(test)]
pub(super) fn replace_record_for_test(cache: &Database, key: CacheKey, encoded: &[u8]) {
    let transaction = cache.begin_write().expect("write transaction");
    {
        let mut table = transaction.open_table(CACHE_TABLE).expect("cache table");
        table
            .insert(key.as_bytes().as_slice(), encoded)
            .expect("test value");
    }
    transaction.commit().expect("commit test value");
}
