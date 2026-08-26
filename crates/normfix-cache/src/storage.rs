use std::sync::Mutex;

use redb::Database;
use serde::de::DeserializeOwned;

use crate::database::{
    RuntimeFailure, open_or_recover, quarantine_and_create, read_record, record_count,
    write_records,
};
use crate::paths::secure_cache_paths;
use crate::{
    CacheAccess, CacheKey, CacheLookup, CacheOpenStatus, CachePaths, CacheWriteStatus,
    PreparedCacheEntry,
};

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
        let secured = secure_cache_paths(&paths);
        let (paths, database, status) = match secured {
            Ok(paths) => {
                let (database, status) = open_or_recover(&paths);
                (paths, database, status)
            }
            Err(error) => (
                paths,
                None,
                CacheOpenStatus::Disabled {
                    reason: error.to_string(),
                },
            ),
        };
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
        let mut state = self.lock_state();
        let Some(database) = state.database.as_ref() else {
            return CacheLookup {
                value: None,
                access: CacheAccess::Bypassed {
                    reason: disabled_reason(&state.status),
                },
            };
        };
        match read_record::<T>(database, key) {
            Ok(Some(value)) => CacheLookup {
                value: Some(value),
                access: CacheAccess::Hit,
            },
            Ok(None) => CacheLookup {
                value: None,
                access: CacheAccess::Miss,
            },
            Err(failure) => {
                let access = handle_runtime_failure(&self.paths, &mut state, failure);
                CacheLookup {
                    value: None,
                    access,
                }
            }
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
        let mut state = self.lock_state();
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
        let mut state = self.lock_state();
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

    fn lock_state(&self) -> std::sync::MutexGuard<'_, CacheState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    #[cfg(test)]
    pub(super) fn replace_record_for_test(&self, key: CacheKey, encoded: &[u8]) {
        let state = self.state.lock().expect("cache mutex");
        let database = state.database.as_ref().expect("active database");
        crate::database::replace_record_for_test(database, key, encoded);
    }
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
    match quarantine_and_create(paths) {
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

fn disabled_reason(status: &CacheOpenStatus) -> String {
    match status {
        CacheOpenStatus::Disabled { reason } => reason.clone(),
        CacheOpenStatus::Ready | CacheOpenStatus::Recreated { .. } => {
            "cache database is not active".to_owned()
        }
    }
}
