use std::collections::BTreeMap;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::{CACHE_SCHEMA_VERSION, CacheKey};

pub(super) const MAX_CACHE_PAYLOAD_BYTES: usize = 32 * 1024 * 1024;
pub(super) const MAX_CACHE_RECORD_BYTES: usize = 128 * 1024 * 1024;

/// Fingerprints a serde value after recursively canonicalizing JSON objects.
///
/// # Errors
///
/// Returns a deterministic serialization error when serde cannot represent the
/// value.
pub fn fingerprint_serde<T: Serialize>(value: &T) -> Result<[u8; 32], CacheEncodeError> {
    canonical_json_bytes(value).map(|bytes| *blake3::hash(&bytes).as_bytes())
}

/// Deterministically serialized cache record ready for a transaction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedCacheEntry {
    pub(super) key: CacheKey,
    pub(super) encoded: Vec<u8>,
}

impl PreparedCacheEntry {
    /// Canonicalizes and serializes one validated result.
    ///
    /// # Errors
    ///
    /// Returns [`CacheEncodeError`] when serde cannot represent `value` or the
    /// result exceeds the bounded persistent-cache record size.
    pub fn new<T: Serialize>(key: CacheKey, value: &T) -> Result<Self, CacheEncodeError> {
        let payload = canonical_json_bytes(value)?;
        validate_payload_length(payload.len())?;
        let envelope = StoredEnvelope {
            schema_version: CACHE_SCHEMA_VERSION,
            key: *key.as_bytes(),
            payload_digest: *blake3::hash(&payload).as_bytes(),
            payload,
        };
        let encoded =
            serde_json::to_vec(&envelope).map_err(|error| CacheEncodeError(error.to_string()))?;
        validate_size("record", encoded.len(), MAX_CACHE_RECORD_BYTES)?;
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

pub(super) fn decode_record<T: DeserializeOwned>(
    key: CacheKey,
    encoded: &[u8],
) -> Result<T, String> {
    validate_record_length(encoded.len())?;
    let envelope: StoredEnvelope = serde_json::from_slice(encoded)
        .map_err(|error| format!("invalid cache envelope: {error}"))?;
    if envelope.schema_version != CACHE_SCHEMA_VERSION {
        return Err(format!(
            "cache record schema {} does not match {}",
            envelope.schema_version, CACHE_SCHEMA_VERSION
        ));
    }
    if envelope.key != *key.as_bytes() {
        return Err("cache record key does not match its table key".to_owned());
    }
    if envelope.payload.len() > MAX_CACHE_PAYLOAD_BYTES {
        return Err(size_message(
            "payload",
            envelope.payload.len(),
            MAX_CACHE_PAYLOAD_BYTES,
        ));
    }
    if envelope.payload_digest != *blake3::hash(&envelope.payload).as_bytes() {
        return Err("cache record payload checksum mismatch".to_owned());
    }
    serde_json::from_slice(&envelope.payload)
        .map_err(|error| format!("invalid cache payload: {error}"))
}

pub(super) fn validate_record_length(length: usize) -> Result<(), String> {
    if length > MAX_CACHE_RECORD_BYTES {
        return Err(size_message("record", length, MAX_CACHE_RECORD_BYTES));
    }
    Ok(())
}

pub(super) fn validate_payload_length(length: usize) -> Result<(), CacheEncodeError> {
    validate_size("payload", length, MAX_CACHE_PAYLOAD_BYTES)
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

fn validate_size(subject: &str, length: usize, maximum: usize) -> Result<(), CacheEncodeError> {
    if length > maximum {
        return Err(CacheEncodeError(size_message(subject, length, maximum)));
    }
    Ok(())
}

fn size_message(subject: &str, length: usize, maximum: usize) -> String {
    format!("cache {subject} is {length} bytes; the safety limit is {maximum} bytes")
}
