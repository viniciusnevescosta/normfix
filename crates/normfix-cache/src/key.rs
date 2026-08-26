use serde::{Deserialize, Serialize};

use crate::CACHE_SCHEMA_VERSION;

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

fn hash_field(hasher: &mut blake3::Hasher, bytes: &[u8]) {
    hasher.update(&u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_le_bytes());
    hasher.update(bytes);
}
