use std::path::PathBuf;

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
