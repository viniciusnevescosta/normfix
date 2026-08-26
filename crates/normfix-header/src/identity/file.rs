//! Bounded reads for untrusted identity sources.

use std::fs::{self, File, Metadata};
use std::io::Read as _;
use std::path::Path;

#[cfg(unix)]
use std::os::unix::fs::MetadataExt as _;

#[derive(Clone, Copy)]
pub(super) enum SymlinkPolicy {
    Follow,
    Reject,
}

pub(super) fn path_entry_exists(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
}

pub(super) fn read_bounded_regular_file(
    path: &Path,
    limit: u64,
    symlink_policy: SymlinkPolicy,
) -> Option<Vec<u8>> {
    let initial = metadata_for_policy(path, symlink_policy)?;
    if !is_acceptable(&initial, limit) {
        return None;
    }

    let mut file = File::open(path).ok()?;
    let opened = file.metadata().ok()?;
    if !is_acceptable(&opened, limit) {
        return None;
    }
    if matches!(symlink_policy, SymlinkPolicy::Reject) {
        let current = fs::symlink_metadata(path).ok()?;
        if current.file_type().is_symlink()
            || !current.is_file()
            || !same_file(&initial, &opened)
            || !same_file(&current, &opened)
        {
            return None;
        }
    }

    let capacity = usize::try_from(opened.len().min(limit)).unwrap_or(0);
    let mut bytes = Vec::with_capacity(capacity);
    file.by_ref()
        .take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .ok()?;
    (u64::try_from(bytes.len()).ok()? <= limit).then_some(bytes)
}

fn metadata_for_policy(path: &Path, symlink_policy: SymlinkPolicy) -> Option<Metadata> {
    match symlink_policy {
        SymlinkPolicy::Follow => fs::metadata(path).ok(),
        SymlinkPolicy::Reject => {
            let metadata = fs::symlink_metadata(path).ok()?;
            (!metadata.file_type().is_symlink()).then_some(metadata)
        }
    }
}

fn is_acceptable(metadata: &Metadata, limit: u64) -> bool {
    metadata.is_file() && metadata.len() <= limit
}

#[cfg(unix)]
fn same_file(left: &Metadata, right: &Metadata) -> bool {
    left.dev() == right.dev() && left.ino() == right.ino()
}

#[cfg(not(unix))]
fn same_file(left: &Metadata, right: &Metadata) -> bool {
    left.is_file() && right.is_file() && left.len() == right.len()
}

#[cfg(test)]
mod tests {
    use std::fs;

    #[cfg(unix)]
    use std::os::unix::fs::symlink;

    use tempfile::TempDir;

    use super::{SymlinkPolicy, read_bounded_regular_file};

    #[test]
    fn refuses_a_file_that_exceeds_the_limit() {
        let temporary = TempDir::new().expect("temporary directory");
        let path = temporary.path().join("settings");
        fs::write(&path, b"12345").expect("settings");

        assert!(read_bounded_regular_file(&path, 4, SymlinkPolicy::Follow).is_none());
    }

    #[cfg(unix)]
    #[test]
    fn reject_policy_never_follows_a_symbolic_link() {
        let temporary = TempDir::new().expect("temporary directory");
        let target = temporary.path().join("target");
        let link = temporary.path().join("link");
        fs::write(&target, b"identity").expect("target");
        symlink(target, &link).expect("symlink");

        assert!(read_bounded_regular_file(&link, 32, SymlinkPolicy::Reject).is_none());
        assert_eq!(
            read_bounded_regular_file(&link, 32, SymlinkPolicy::Follow),
            Some(b"identity".to_vec())
        );
    }
}
