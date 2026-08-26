use std::collections::BTreeMap;
use std::sync::Arc;
use std::thread;

use serde::{Deserialize, Serialize};
use tempfile::TempDir;

use super::encoding::{
    MAX_CACHE_PAYLOAD_BYTES, MAX_CACHE_RECORD_BYTES, validate_payload_length,
    validate_record_length,
};
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
fn external_cache_uses_the_normfix_namespace() {
    let directory = TempDir::new().expect("temporary directory");
    let project = directory.path().join("project");
    let base = directory.path().join("external-cache");
    std::fs::create_dir(&project).expect("project directory");

    let paths = CachePaths::with_base(&base, &project);

    assert!(paths.database().starts_with(base.join("normfix")));
    assert!(!paths.database().starts_with(base.join("norminette-fix")));
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
    let fingerprint = fingerprint_serde(&value).expect("fingerprint");
    assert_eq!(fingerprint, fingerprint_serde(&value).expect("fingerprint"));
    let first = PreparedCacheEntry::new(key(b"x"), &value).expect("entry");
    let second = PreparedCacheEntry::new(key(b"x"), &value).expect("entry");
    assert_eq!(first.encoded(), second.encoded());
    assert_eq!(
        key(b"x").to_hex(),
        "f172181958c4b362d4dcbc06fca82cc104096f462c3ca62032f0ad24a990b4f5"
    );
    assert_eq!(
        blake3::Hash::from_bytes(fingerprint).to_hex().as_str(),
        "68246fabd162b6b0d6408292b2a2e2df7960267a50877455dd830b0da4053c1b"
    );
    assert_eq!(
        blake3::hash(first.encoded()).to_hex().as_str(),
        "4e27aaf590c6a4a3f1c1deeb996a572c5236476db4edc835dbecc5d7e8e927b3"
    );
    assert_eq!(first.encoded().len(), 425);
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

    assert_eq!(
        std::fs::read(&quarantined).expect("quarantined bytes"),
        b"not a redb database"
    );
    assert!(cache.paths().database().exists());
    assert_eq!(
        cache.store(&PreparedCacheEntry::new(key(b"after"), &fixture(0)).expect("recovered entry")),
        CacheWriteStatus::Stored { entries: 1 }
    );
}

#[test]
fn corrupt_record_fails_open_and_recreates_the_database() {
    let directory = TempDir::new().expect("temporary directory");
    let cache = cache(&directory);
    let cache_key = key(b"corrupt");
    cache.replace_record_for_test(cache_key, b"not-json");

    let lookup = cache.lookup::<Analysis>(cache_key);

    assert!(lookup.value.is_none());
    assert!(matches!(lookup.access, CacheAccess::Recovered { .. }));
    assert_eq!(cache.len().value, Some(0));
}

#[test]
fn oversized_record_is_rejected_before_copying() {
    let error = validate_record_length(MAX_CACHE_RECORD_BYTES + 1).expect_err("oversized record");

    assert!(error.contains("record"));
    assert!(error.contains("safety limit"));
}

#[test]
fn oversized_payload_is_rejected_before_envelope_expansion() {
    let error =
        validate_payload_length(MAX_CACHE_PAYLOAD_BYTES + 1).expect_err("oversized payload");

    assert!(error.to_string().contains("payload"));
    assert!(error.to_string().contains("safety limit"));
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
    let _ = cache.store(&PreparedCacheEntry::new(cache_key, &computed).expect("prepared analysis"));
    let warm = cache.lookup::<Analysis>(cache_key);

    assert_eq!(computed, expected);
    assert_eq!(warm.value, Some(expected));
}

#[test]
fn concurrent_reads_return_identical_values() {
    let directory = TempDir::new().expect("temporary directory");
    let cache = Arc::new(cache(&directory));
    let cache_key = key(b"parallel");
    let _ =
        cache.store(&PreparedCacheEntry::new(cache_key, &fixture(42)).expect("prepared analysis"));
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

#[cfg(unix)]
#[test]
fn database_symlink_is_rejected_without_touching_its_target() {
    use std::os::unix::fs::symlink;

    let directory = TempDir::new().expect("temporary directory");
    let project = directory.path().join("project");
    let base = directory.path().join("external");
    let victim = project.join("victim.c");
    std::fs::create_dir(&project).expect("project");
    std::fs::write(&victim, b"int victim;\n").expect("victim");
    let paths = CachePaths::with_base(base, &project);
    std::fs::create_dir_all(paths.database().parent().expect("cache parent"))
        .expect("cache parent");
    symlink(&victim, paths.database()).expect("database symlink");

    let cache = PersistentCache::open(paths);

    assert!(matches!(cache.status(), CacheOpenStatus::Disabled { .. }));
    assert_eq!(
        std::fs::read(&victim).expect("victim bytes"),
        b"int victim;\n"
    );
}

#[cfg(unix)]
#[test]
fn newly_created_database_is_private() {
    use std::os::unix::fs::PermissionsExt;

    let directory = TempDir::new().expect("temporary directory");
    let cache = cache(&directory);
    let mode = std::fs::metadata(cache.paths().database())
        .expect("database metadata")
        .permissions()
        .mode();

    assert_eq!(mode & 0o077, 0);
}
