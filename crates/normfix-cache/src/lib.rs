//! Persistent, content-addressed cache for `norminette-fix`.
//!
//! Cache storage is deliberately outside the analyzed project. Every lookup
//! fails open: lock contention, I/O failures, invalid records and database
//! corruption become misses or bypasses, never lint failures. Corrupt databases
//! are quarantined before a fresh database is created.
//!
//! Only deterministic, validated analysis results belong here. Syntax trees,
//! parser arenas and other transient backend state must never be serialized.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use redb::{
    CommitError, Database, DatabaseError, ReadableTableMetadata, StorageError, TableDefinition,
    TableError, TransactionError,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

/// Persistent cache schema included in every key and record.
pub const CACHE_SCHEMA_VERSION: u32 = 1;

const CACHE_TABLE: TableDefinition<&[u8], &[u8]> = TableDefinition::new("validated_results_v1");

/// A deterministic content-addressed cache key.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CacheKey([u8; 32]);

impl CacheKey {
    /// Derives a key from every input that can affect analysis output.
    ///
    /// `namespace` separates result types and rule phases. `relative_path` is
    /// included because headers and file-name rules are path-sensitive.
    #[must_use]
    pub fn derive(
        namespace: &str,
        relative_path: &str,
        content: &[u8],
        config_fingerprint: &[u8],
        tool_fingerprint: &[u8],
    ) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"normfix-cache-key\0");
        hasher.update(&CACHE_SCHEMA_VERSION.to_le_bytes());
        hash_field(&mut hasher, namespace.as_bytes());
        hash_field(&mut hasher, relative_path.as_bytes());
        hash_field(&mut hasher, content);
        hash_field(&mut hasher, config_fingerprint);
        hash_field(&mut hasher, tool_fingerprint);
        Self(*hasher.finalize().as_bytes())
    }

    /// Returns the raw 32-byte key.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Returns the lowercase hexadecimal key.
    #[must_use]
    pub fn to_hex(self) -> String {
        blake3::Hash::from_bytes(self.0).to_hex().to_string()
    }
}

/// Fingerprints a serde value after recursively canonicalizing JSON objects.
///
/// # Errors
///
/// Returns a deterministic serialization error, for example for non-finite
/// floating-point values.
pub fn fingerprint_serde<T: Serialize>(value: &T) -> Result<[u8; 32], CacheEncodeError> {
    canonical_json_bytes(value).map(|bytes| *blake3::hash(&bytes).as_bytes())
}

/// External database location for one canonical project.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CachePaths {
    database: PathBuf,
}

impl CachePaths {
    /// Resolves the platform cache base and derives a project-specific path.
    ///
    /// On Unix, `$XDG_CACHE_HOME` is preferred and `$HOME/.cache` is the
    /// fallback. On Windows, `LOCALAPPDATA` is used.
    ///
    /// # Errors
    ///
    /// Returns [`CachePathError`] when no external cache base is available or
    /// the project root cannot be canonicalized.
    pub fn for_project(project_root: &Path) -> Result<Self, CachePathError> {
        let canonical =
            fs::canonicalize(project_root).map_err(|error| CachePathError::ProjectRoot {
                path: project_root.to_path_buf(),
                message: error.to_string(),
            })?;
        let base = platform_cache_base().ok_or(CachePathError::NoCacheDirectory)?;
        Ok(Self::with_base(base, &canonical))
    }

    /// Derives a project cache below an explicit external base.
    ///
    /// This constructor is useful for hermetic callers and tests. The caller is
    /// responsible for ensuring `base` is outside the analyzed project.
    #[must_use]
    pub fn with_base(base: impl Into<PathBuf>, canonical_project_root: &Path) -> Self {
        let project_id = blake3::hash(&native_path_bytes(canonical_project_root))
            .to_hex()
            .to_string();
        Self {
            database: base
                .into()
                .join("norminette-fix")
                .join(project_id)
                .join(format!("cache-v{CACHE_SCHEMA_VERSION}.redb")),
        }
    }

    /// Returns the redb database path.
    #[must_use]
    pub fn database(&self) -> &Path {
        &self.database
    }
}

/// Failure to derive an external cache location.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CachePathError {
    /// The project root could not be canonicalized.
    ProjectRoot {
        /// Requested root.
        path: PathBuf,
        /// Operating-system detail.
        message: String,
    },
    /// No XDG/platform cache base was available.
    NoCacheDirectory,
}

impl std::fmt::Display for CachePathError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ProjectRoot { path, message } => {
                write!(
                    formatter,
                    "could not resolve project root `{}`: {message}",
                    path.display()
                )
            }
            Self::NoCacheDirectory => {
                formatter.write_str("no external user cache directory is available")
            }
        }
    }
}

impl std::error::Error for CachePathError {}

/// Result of opening the persistent cache.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CacheOpenStatus {
    /// Database opened normally.
    Ready,
    /// Corrupt/incompatible storage was preserved and replaced.
    Recreated {
        /// Quarantined original database.
        quarantined: PathBuf,
        /// Original redb failure.
        reason: String,
    },
    /// Cache is unavailable; analysis must continue without it.
    Disabled {
        /// Actionable reason.
        reason: String,
    },
}

/// Per-operation cache behavior.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CacheAccess {
    /// A validated record was returned.
    Hit,
    /// No record existed for the key.
    Miss,
    /// Cache was unavailable and the caller must compute normally.
    Bypassed {
        /// Non-fatal reason.
        reason: String,
    },
    /// Corruption was recovered during the operation; this access is a miss.
    Recovered {
        /// Quarantined database.
        quarantined: PathBuf,
        /// Recovery reason.
        reason: String,
    },
}

/// Value plus the non-fatal cache outcome.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheLookup<T> {
    /// Cached result on a hit.
    pub value: Option<T>,
    /// Hit, miss, bypass or recovery.
    pub access: CacheAccess,
}

/// Result of a transactional cache write.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CacheWriteStatus {
    /// Every prepared record committed in one redb write transaction.
    Stored {
        /// Number of records committed.
        entries: usize,
    },
    /// Cache was unavailable; no analysis result was affected.
    Bypassed {
        /// Non-fatal reason.
        reason: String,
    },
    /// Storage was recovered after the write failed; callers may continue.
    Recovered {
        /// Quarantined database.
        quarantined: PathBuf,
        /// Recovery reason.
        reason: String,
    },
}

/// Deterministically serialized cache record ready for a transaction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedCacheEntry {
    key: CacheKey,
    encoded: Vec<u8>,
}

impl PreparedCacheEntry {
    /// Canonicalizes and serializes one validated result.
    ///
    /// # Errors
    ///
    /// Returns [`CacheEncodeError`] when serde cannot represent `value`.
    pub fn new<T: Serialize>(key: CacheKey, value: &T) -> Result<Self, CacheEncodeError> {
        let payload = canonical_json_bytes(value)?;
        let envelope = StoredEnvelope {
            schema_version: CACHE_SCHEMA_VERSION,
            key: key.0,
            payload_digest: *blake3::hash(&payload).as_bytes(),
            payload,
        };
        let encoded =
            serde_json::to_vec(&envelope).map_err(|error| CacheEncodeError(error.to_string()))?;
        Ok(Self { key, encoded })
    }

    /// Returns the entry key.
    #[must_use]
    pub const fn key(&self) -> CacheKey {
        self.key
    }

    /// Returns deterministic envelope bytes.
    #[must_use]
    pub fn encoded(&self) -> &[u8] {
        &self.encoded
    }
}

/// Deterministic serialization failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheEncodeError(String);

impl std::fmt::Display for CacheEncodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "could not encode deterministic cache entry: {}",
            self.0
        )
    }
}

impl std::error::Error for CacheEncodeError {}

#[derive(Debug, Serialize, Deserialize)]
struct StoredEnvelope {
    schema_version: u32,
    key: [u8; 32],
    payload_digest: [u8; 32],
    payload: Vec<u8>,
}

#[derive(Debug)]
struct CacheState {
    database: Option<Database>,
    status: CacheOpenStatus,
}

/// Transactional redb-backed cache that always fails open.
#[derive(Debug)]
pub struct PersistentCache {
    paths: CachePaths,
    state: Mutex<CacheState>,
}

impl PersistentCache {
    /// Opens or creates a cache. This function never fails the calling
    /// analysis; failures are represented by [`CacheOpenStatus::Disabled`].
    #[must_use]
    pub fn open(paths: CachePaths) -> Self {
        let (database, status) = open_or_recover(&paths);
        Self {
            paths,
            state: Mutex::new(CacheState { database, status }),
        }
    }

    /// Returns the current cache availability.
    #[must_use]
    pub fn status(&self) -> CacheOpenStatus {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .status
            .clone()
    }

    /// Returns the external database paths.
    #[must_use]
    pub const fn paths(&self) -> &CachePaths {
        &self.paths
    }

    /// Loads and verifies one deterministic serde result.
    ///
    /// Invalid records trigger database quarantine/recreation and return a
    /// miss. Callers must compute the same result they would with cache off.
    #[must_use]
    pub fn lookup<T: DeserializeOwned>(&self, key: CacheKey) -> CacheLookup<T> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(database) = state.database.as_ref() else {
            return CacheLookup {
                value: None,
                access: CacheAccess::Bypassed {
                    reason: disabled_reason(&state.status),
                },
            };
        };
        let encoded = match read_record(database, key) {
            Ok(Some(encoded)) => encoded,
            Ok(None) => {
                return CacheLookup {
                    value: None,
                    access: CacheAccess::Miss,
                };
            }
            Err(failure) => {
                let access = handle_runtime_failure(&self.paths, &mut state, failure);
                return CacheLookup {
                    value: None,
                    access,
                };
            }
        };
        match decode_record::<T>(key, &encoded) {
            Ok(value) => CacheLookup {
                value: Some(value),
                access: CacheAccess::Hit,
            },
            Err(reason) => CacheLookup {
                value: None,
                access: handle_runtime_failure(
                    &self.paths,
                    &mut state,
                    RuntimeFailure::Corrupt(reason),
                ),
            },
        }
    }

    /// Commits all prepared records atomically within one redb transaction.
    ///
    /// An empty batch succeeds without opening a write transaction.
    #[must_use]
    pub fn store_batch(&self, entries: &[PreparedCacheEntry]) -> CacheWriteStatus {
        if entries.is_empty() {
            return CacheWriteStatus::Stored { entries: 0 };
        }
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(database) = state.database.as_ref() else {
            return CacheWriteStatus::Bypassed {
                reason: disabled_reason(&state.status),
            };
        };
        match write_records(database, entries) {
            Ok(()) => CacheWriteStatus::Stored {
                entries: entries.len(),
            },
            Err(failure) => match handle_runtime_failure(&self.paths, &mut state, failure) {
                CacheAccess::Recovered {
                    quarantined,
                    reason,
                } => CacheWriteStatus::Recovered {
                    quarantined,
                    reason,
                },
                CacheAccess::Bypassed { reason } => CacheWriteStatus::Bypassed { reason },
                CacheAccess::Hit | CacheAccess::Miss => unreachable!("recovery result"),
            },
        }
    }

    /// Convenience wrapper for one transactional record.
    #[must_use]
    pub fn store(&self, entry: &PreparedCacheEntry) -> CacheWriteStatus {
        self.store_batch(std::slice::from_ref(entry))
    }

    /// Returns the number of stored records, or a non-fatal bypass.
    #[must_use]
    pub fn len(&self) -> CacheLookup<usize> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(database) = state.database.as_ref() else {
            return CacheLookup {
                value: None,
                access: CacheAccess::Bypassed {
                    reason: disabled_reason(&state.status),
                },
            };
        };
        match record_count(database) {
            Ok(count) => CacheLookup {
                value: Some(count),
                access: CacheAccess::Hit,
            },
            Err(failure) => CacheLookup {
                value: None,
                access: handle_runtime_failure(&self.paths, &mut state, failure),
            },
        }
    }

    /// Returns whether the active cache contains no records.
    #[must_use]
    pub fn is_empty(&self) -> CacheLookup<bool> {
        let count = self.len();
        CacheLookup {
            value: count.value.map(|value| value == 0),
            access: count.access,
        }
    }

    #[cfg(test)]
    fn corrupt_record_for_test(&self, key: CacheKey) {
        let state = self.state.lock().expect("cache mutex");
        let database = state.database.as_ref().expect("active database");
        let transaction = database.begin_write().expect("write transaction");
        {
            let mut table = transaction.open_table(CACHE_TABLE).expect("cache table");
            table
                .insert(key.as_bytes().as_slice(), b"not-json".as_slice())
                .expect("corrupt value");
        }
        transaction.commit().expect("commit corrupt value");
    }
}

fn open_or_recover(paths: &CachePaths) -> (Option<Database>, CacheOpenStatus) {
    let Some(parent) = paths.database.parent() else {
        return (
            None,
            CacheOpenStatus::Disabled {
                reason: "cache database has no parent directory".to_owned(),
            },
        );
    };
    if let Err(error) = fs::create_dir_all(parent) {
        return (
            None,
            CacheOpenStatus::Disabled {
                reason: format!("could not create cache directory: {error}"),
            },
        );
    }
    match create_initialized_database(&paths.database) {
        Ok(database) => (Some(database), CacheOpenStatus::Ready),
        Err(OpenFailure::Busy(reason) | OpenFailure::Io(reason)) => {
            (None, CacheOpenStatus::Disabled { reason })
        }
        Err(OpenFailure::Corrupt(reason)) => match quarantine_and_create(paths, &reason) {
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

enum OpenFailure {
    Busy(String),
    Corrupt(String),
    Io(String),
}

fn create_initialized_database(path: &Path) -> Result<Database, OpenFailure> {
    let database = Database::create(path).map_err(|error| classify_open_error(&error))?;
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

fn quarantine_and_create(paths: &CachePaths, _reason: &str) -> Result<(Database, PathBuf), String> {
    let quarantined = next_quarantine_path(&paths.database);
    fs::rename(&paths.database, &quarantined)
        .map_err(|error| format!("could not quarantine cache: {error}"))?;
    create_initialized_database(&paths.database)
        .map(|database| (database, quarantined))
        .map_err(|error| match error {
            OpenFailure::Busy(reason) | OpenFailure::Corrupt(reason) | OpenFailure::Io(reason) => {
                reason
            }
        })
}

fn next_quarantine_path(database: &Path) -> PathBuf {
    for sequence in 1u32..=u32::MAX {
        let candidate = database.with_extension(format!("redb.corrupt-{sequence}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    database.with_extension("redb.corrupt-overflow")
}

#[derive(Debug)]
enum RuntimeFailure {
    Corrupt(String),
    Bypass(String),
}

fn read_record(database: &Database, key: CacheKey) -> Result<Option<Vec<u8>>, RuntimeFailure> {
    let transaction = database
        .begin_read()
        .map_err(|error| classify_transaction_error(&error))?;
    let table = match transaction.open_table(CACHE_TABLE) {
        Ok(table) => table,
        Err(TableError::TableDoesNotExist(_)) => return Ok(None),
        Err(error) => return Err(classify_table_error(&error)),
    };
    table
        .get(key.as_bytes().as_slice())
        .map(|value| value.map(|guard| guard.value().to_vec()))
        .map_err(|error| classify_storage_error(&error))
}

fn record_count(database: &Database) -> Result<usize, RuntimeFailure> {
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

fn write_records(
    database: &Database,
    entries: &[PreparedCacheEntry],
) -> Result<(), RuntimeFailure> {
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

fn decode_record<T: DeserializeOwned>(key: CacheKey, encoded: &[u8]) -> Result<T, String> {
    let envelope: StoredEnvelope = serde_json::from_slice(encoded)
        .map_err(|error| format!("invalid cache envelope: {error}"))?;
    if envelope.schema_version != CACHE_SCHEMA_VERSION {
        return Err(format!(
            "cache record schema {} does not match {}",
            envelope.schema_version, CACHE_SCHEMA_VERSION
        ));
    }
    if envelope.key != key.0 {
        return Err("cache record key does not match its table key".to_owned());
    }
    if envelope.payload_digest != *blake3::hash(&envelope.payload).as_bytes() {
        return Err("cache record payload checksum mismatch".to_owned());
    }
    serde_json::from_slice(&envelope.payload)
        .map_err(|error| format!("invalid cache payload: {error}"))
}

fn handle_runtime_failure(
    paths: &CachePaths,
    state: &mut CacheState,
    failure: RuntimeFailure,
) -> CacheAccess {
    match failure {
        RuntimeFailure::Corrupt(reason) => recover_runtime_corruption(paths, state, reason),
        RuntimeFailure::Bypass(reason) => {
            let database = state.database.take();
            drop(database);
            state.status = CacheOpenStatus::Disabled {
                reason: reason.clone(),
            };
            CacheAccess::Bypassed { reason }
        }
    }
}

fn recover_runtime_corruption(
    paths: &CachePaths,
    state: &mut CacheState,
    reason: String,
) -> CacheAccess {
    let database = state.database.take();
    drop(database);
    match quarantine_and_create(paths, &reason) {
        Ok((database, quarantined)) => {
            state.database = Some(database);
            state.status = CacheOpenStatus::Recreated {
                quarantined: quarantined.clone(),
                reason: reason.clone(),
            };
            CacheAccess::Recovered {
                quarantined,
                reason,
            }
        }
        Err(recovery_error) => {
            let reason = format!("cache failed ({reason}) and recovery failed: {recovery_error}");
            state.status = CacheOpenStatus::Disabled {
                reason: reason.clone(),
            };
            CacheAccess::Bypassed { reason }
        }
    }
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

fn disabled_reason(status: &CacheOpenStatus) -> String {
    match status {
        CacheOpenStatus::Disabled { reason } => reason.clone(),
        CacheOpenStatus::Ready | CacheOpenStatus::Recreated { .. } => {
            "cache database is not active".to_owned()
        }
    }
}

fn canonical_json_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, CacheEncodeError> {
    let value = serde_json::to_value(value).map_err(|error| CacheEncodeError(error.to_string()))?;
    serde_json::to_vec(&canonicalize_json(value))
        .map_err(|error| CacheEncodeError(error.to_string()))
}

fn canonicalize_json(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Array(values) => {
            serde_json::Value::Array(values.into_iter().map(canonicalize_json).collect())
        }
        serde_json::Value::Object(values) => {
            let mut sorted = BTreeMap::new();
            for (key, value) in values {
                sorted.insert(key, canonicalize_json(value));
            }
            serde_json::Value::Object(sorted.into_iter().collect())
        }
        scalar => scalar,
    }
}

fn hash_field(hasher: &mut blake3::Hasher, bytes: &[u8]) {
    hasher.update(&u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_le_bytes());
    hasher.update(bytes);
}

fn platform_cache_base() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
    {
        return Some(path);
    }
    #[cfg(windows)]
    {
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .map(|home| home.join(".cache"))
    }
}

#[cfg(unix)]
fn native_path_bytes(path: &Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;

    path.as_os_str().as_bytes().to_vec()
}

#[cfg(windows)]
fn native_path_bytes(path: &Path) -> Vec<u8> {
    use std::os::windows::ffi::OsStrExt;

    path.as_os_str()
        .encode_wide()
        .flat_map(u16::to_le_bytes)
        .collect()
}

#[cfg(not(any(unix, windows)))]
fn native_path_bytes(path: &Path) -> Vec<u8> {
    path.as_os_str().to_string_lossy().as_bytes().to_vec()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::Arc;
    use std::thread;

    use serde::{Deserialize, Serialize};
    use tempfile::TempDir;

    use super::{
        CacheAccess, CacheKey, CacheOpenStatus, CachePaths, CacheWriteStatus, PersistentCache,
        PreparedCacheEntry, fingerprint_serde,
    };

    #[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
    struct Analysis {
        clean: bool,
        diagnostics: BTreeMap<String, u32>,
    }

    fn fixture(value: u32) -> Analysis {
        Analysis {
            clean: value == 0,
            diagnostics: BTreeMap::from([("RULE".to_owned(), value)]),
        }
    }

    fn key(content: &[u8]) -> CacheKey {
        CacheKey::derive(
            "test-analysis",
            "src/main.c",
            content,
            b"config-v1",
            b"tool-v1",
        )
    }

    fn cache(directory: &TempDir) -> PersistentCache {
        let project = directory.path().join("project");
        let base = directory.path().join("external-cache");
        std::fs::create_dir(&project).expect("project directory");
        PersistentCache::open(CachePaths::with_base(base, &project))
    }

    #[test]
    fn keys_cover_content_config_tool_path_and_namespace() {
        let baseline = key(b"int main(void);\n");
        assert_eq!(baseline, key(b"int main(void);\n"));
        assert_ne!(
            baseline,
            CacheKey::derive(
                "other",
                "src/main.c",
                b"int main(void);\n",
                b"config-v1",
                b"tool-v1"
            )
        );
        assert_ne!(
            baseline,
            CacheKey::derive(
                "test-analysis",
                "src/other.c",
                b"int main(void);\n",
                b"config-v1",
                b"tool-v1"
            )
        );
        assert_ne!(baseline, key(b"int changed(void);\n"));
        assert_ne!(
            baseline,
            CacheKey::derive(
                "test-analysis",
                "src/main.c",
                b"int main(void);\n",
                b"config-v2",
                b"tool-v1"
            )
        );
        assert_ne!(
            baseline,
            CacheKey::derive(
                "test-analysis",
                "src/main.c",
                b"int main(void);\n",
                b"config-v1",
                b"tool-v2"
            )
        );
    }

    #[test]
    fn explicit_cache_base_stays_outside_the_project() {
        let directory = TempDir::new().expect("temporary directory");
        let project = directory.path().join("project");
        let external = directory.path().join("user-cache");
        std::fs::create_dir(&project).expect("project");

        let paths = CachePaths::with_base(&external, &project);

        assert!(paths.database().starts_with(&external));
        assert!(!paths.database().starts_with(&project));
        assert!(paths.database().ends_with("cache-v1.redb"));
    }

    #[test]
    fn serde_fingerprints_and_entries_are_deterministic() {
        let value = fixture(3);
        assert_eq!(
            fingerprint_serde(&value).expect("fingerprint"),
            fingerprint_serde(&value).expect("fingerprint")
        );
        let first = PreparedCacheEntry::new(key(b"x"), &value).expect("entry");
        let second = PreparedCacheEntry::new(key(b"x"), &value).expect("entry");
        assert_eq!(first.encoded(), second.encoded());
    }

    #[test]
    fn transactionally_round_trips_a_batch() {
        let directory = TempDir::new().expect("temporary directory");
        let cache = cache(&directory);
        let first_key = key(b"first");
        let second_key = key(b"second");
        let entries = [
            PreparedCacheEntry::new(first_key, &fixture(1)).expect("first entry"),
            PreparedCacheEntry::new(second_key, &fixture(2)).expect("second entry"),
        ];

        assert_eq!(
            cache.store_batch(&entries),
            CacheWriteStatus::Stored { entries: 2 }
        );
        assert_eq!(
            cache.lookup::<Analysis>(first_key),
            super::CacheLookup {
                value: Some(fixture(1)),
                access: CacheAccess::Hit,
            }
        );
        assert_eq!(cache.len().value, Some(2));
    }

    #[test]
    fn corrupt_database_is_quarantined_and_recreated_on_open() {
        let directory = TempDir::new().expect("temporary directory");
        let project = directory.path().join("project");
        let base = directory.path().join("external");
        std::fs::create_dir(&project).expect("project");
        let paths = CachePaths::with_base(base, &project);
        std::fs::create_dir_all(paths.database().parent().expect("cache parent"))
            .expect("cache parent");
        std::fs::write(paths.database(), b"not a redb database").expect("corrupt database");

        let cache = PersistentCache::open(paths);
        let status = cache.status();
        let CacheOpenStatus::Recreated { quarantined, .. } = status else {
            panic!("cache should recover, got {status:?}");
        };

        assert!(quarantined.exists());
        assert!(cache.paths().database().exists());
        assert_eq!(
            cache.store(
                &PreparedCacheEntry::new(key(b"after"), &fixture(0)).expect("recovered entry")
            ),
            CacheWriteStatus::Stored { entries: 1 }
        );
    }

    #[test]
    fn corrupt_record_fails_open_and_recreates_the_database() {
        let directory = TempDir::new().expect("temporary directory");
        let cache = cache(&directory);
        let cache_key = key(b"corrupt");
        cache.corrupt_record_for_test(cache_key);

        let lookup = cache.lookup::<Analysis>(cache_key);

        assert!(lookup.value.is_none());
        assert!(matches!(lookup.access, CacheAccess::Recovered { .. }));
        assert_eq!(cache.len().value, Some(0));
    }

    #[test]
    fn lock_contention_disables_second_handle_without_quarantine() {
        let directory = TempDir::new().expect("temporary directory");
        let project = directory.path().join("project");
        let base = directory.path().join("external");
        std::fs::create_dir(&project).expect("project");
        let paths = CachePaths::with_base(base, &project);
        let first = PersistentCache::open(paths.clone());
        let second = PersistentCache::open(paths);

        assert!(matches!(first.status(), CacheOpenStatus::Ready));
        assert!(matches!(second.status(), CacheOpenStatus::Disabled { .. }));
        assert!(matches!(
            second.lookup::<Analysis>(key(b"x")).access,
            CacheAccess::Bypassed { .. }
        ));
    }

    #[test]
    fn cache_on_and_off_produce_the_same_analysis() {
        fn analyze(source: &str) -> Analysis {
            fixture(u32::from(source.contains("bad")))
        }

        let directory = TempDir::new().expect("temporary directory");
        let cache = cache(&directory);
        let source = "bad";
        let expected = analyze(source);
        let cache_key = key(source.as_bytes());
        let miss = cache.lookup::<Analysis>(cache_key);
        assert_eq!(miss.access, CacheAccess::Miss);
        let computed = analyze(source);
        let _ =
            cache.store(&PreparedCacheEntry::new(cache_key, &computed).expect("prepared analysis"));
        let warm = cache.lookup::<Analysis>(cache_key);

        assert_eq!(computed, expected);
        assert_eq!(warm.value, Some(expected));
    }

    #[test]
    fn concurrent_reads_return_identical_values() {
        let directory = TempDir::new().expect("temporary directory");
        let cache = Arc::new(cache(&directory));
        let cache_key = key(b"parallel");
        let _ = cache
            .store(&PreparedCacheEntry::new(cache_key, &fixture(42)).expect("prepared analysis"));
        let handles = (0..8)
            .map(|_| {
                let cache = Arc::clone(&cache);
                thread::spawn(move || cache.lookup::<Analysis>(cache_key))
            })
            .collect::<Vec<_>>();

        for handle in handles {
            assert_eq!(handle.join().expect("reader").value, Some(fixture(42)));
        }
    }
}
