//! Persistent, content-addressed cache for `normfix`.
//!
//! Cache storage is deliberately outside the analyzed project. Every lookup
//! fails open: lock contention, I/O failures, invalid records and database
//! corruption become misses or bypasses, never lint failures. Corrupt databases
//! are quarantined before a fresh database is created.
//!
//! Only deterministic, validated analysis results belong here. Syntax trees,
//! parser arenas and other transient backend state must never be serialized.

#![forbid(unsafe_code)]

mod database;
mod encoding;
mod key;
mod paths;
mod status;
mod storage;

pub use encoding::{CacheEncodeError, PreparedCacheEntry, fingerprint_serde};
pub use key::CacheKey;
pub use paths::{CachePathError, CachePaths};
pub use status::{CacheAccess, CacheLookup, CacheOpenStatus, CacheWriteStatus};
pub use storage::PersistentCache;

/// Persistent cache schema included in every key and record.
pub const CACHE_SCHEMA_VERSION: u32 = 1;

#[cfg(test)]
mod tests;
