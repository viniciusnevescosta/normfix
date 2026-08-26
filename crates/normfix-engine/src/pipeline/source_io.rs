//! Bounded, race-aware reads for files retained in an analysis report.
//!
//! A report keeps both original and proposed text alive until rendering and a
//! fix run may process several files in parallel. Reading arbitrary regular
//! files with `fs::read` therefore lets one accidental or hostile giant file
//! turn a formatting command into an out-of-memory abort. This module is the
//! single limit and revalidation boundary for project source bytes.

use std::fs::{self, File};
use std::io::{self, Read as _};
use std::path::Path;

/// 42 sources and Makefiles are text inputs, not data archives.
pub(super) const MAX_ANALYZED_FILE_BYTES: u64 = 16 * 1024 * 1024;

pub(super) fn read_project_file(path: &Path) -> io::Result<Vec<u8>> {
    let before = fs::symlink_metadata(path)?;
    if before.file_type().is_symlink() || !before.file_type().is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "path is not a regular non-symbolic file",
        ));
    }
    if before.len() > MAX_ANALYZED_FILE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "file contains {} bytes, exceeding the {} MiB analysis limit",
                before.len(),
                MAX_ANALYZED_FILE_BYTES / (1024 * 1024)
            ),
        ));
    }
    let opened = File::open(path)?;
    let opened_metadata = opened.metadata()?;
    if !same_file(&before, &opened_metadata) || opened_metadata.len() != before.len() {
        return Err(io::Error::other(
            "file identity changed while it was being opened",
        ));
    }
    let capacity = usize::try_from(before.len())
        .map_err(|_| io::Error::other("file length does not fit in memory"))?;
    let mut bytes = Vec::with_capacity(capacity);
    opened
        .take(MAX_ANALYZED_FILE_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)?;
    let after = fs::symlink_metadata(path)?;
    if after.file_type().is_symlink()
        || !same_file(&before, &after)
        || after.len() != before.len()
        || u64::try_from(bytes.len()) != Ok(before.len())
    {
        return Err(io::Error::other("file changed while it was being read"));
    }
    Ok(bytes)
}

pub(super) fn project_file_matches(path: &Path, expected: &[u8]) -> io::Result<bool> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink()
        || !metadata.file_type().is_file()
        || metadata.len() != u64::try_from(expected.len()).unwrap_or(u64::MAX)
        || metadata.len() > MAX_ANALYZED_FILE_BYTES
    {
        return Ok(false);
    }
    let mut input = File::open(path)?;
    let opened = input.metadata()?;
    if !same_file(&metadata, &opened) || opened.len() != metadata.len() {
        return Ok(false);
    }
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
    let after = fs::symlink_metadata(path)?;
    Ok(offset == expected.len()
        && !after.file_type().is_symlink()
        && same_file(&metadata, &after)
        && after.len() == metadata.len())
}

#[cfg(unix)]
fn same_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    left.dev() == right.dev() && left.ino() == right.ino()
}

#[cfg(not(unix))]
fn same_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.is_file() == right.is_file()
        && left.len() == right.len()
        && left.modified().ok() == right.modified().ok()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::{MAX_ANALYZED_FILE_BYTES, project_file_matches, read_project_file};

    #[test]
    fn a_sparse_oversized_source_is_refused_before_allocation() {
        let directory = TempDir::new().expect("temporary directory");
        let path = directory.path().join("huge.c");
        fs::File::create(&path)
            .expect("sparse file")
            .set_len(MAX_ANALYZED_FILE_BYTES + 1)
            .expect("sparse length");

        let error = read_project_file(&path).expect_err("oversized source");

        assert!(error.to_string().contains("16 MiB analysis limit"));
    }

    #[test]
    fn streaming_comparison_rejects_same_length_external_edits() {
        let directory = TempDir::new().expect("temporary directory");
        let path = directory.path().join("main.c");
        fs::write(&path, b"old\n").expect("fixture");
        assert!(project_file_matches(&path, b"old\n").expect("match"));

        fs::write(&path, b"new\n").expect("external edit");

        assert!(!project_file_matches(&path, b"old\n").expect("mismatch"));
    }
}
