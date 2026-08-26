//! Recoverable quarantine for files a 42 project is not expected to contain.
//!
//! Nothing here deletes: a quarantined file is moved to external recovery
//! storage with its relative path preserved. The staging, rollback, and
//! overlap checks exist so a partial failure leaves the project as it was.

use std::ffi::OsString;
use std::fs::OpenOptions;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use camino::Utf8PathBuf;

use normfix_destructive::{
    QuarantineItem, QuarantineRequest, plan_quarantine, quarantine_snapshot_matches,
};

use super::paths::{absolute_lexical, automatic_backup_root, run_id};
use super::{BackupPolicy, FixOptions};

pub(super) fn quarantine_unexpected_files(
    unexpected: &[PathBuf],
    options: &FixOptions,
) -> (Vec<Utf8PathBuf>, Vec<String>) {
    let Some(authorization) = options.destructive_authorization.as_ref() else {
        return (
            Vec::new(),
            vec![
                "Unexpected files were not quarantined because destructive authorization is missing."
                    .to_owned(),
            ],
        );
    };
    let (project_root, recovery_root) = match quarantine_roots(options) {
        Ok(roots) => roots,
        Err(error) => return (Vec::new(), vec![error]),
    };
    let mut relative_paths = Vec::new();
    let mut errors = Vec::new();
    for path in unexpected {
        let Ok(relative) = path.strip_prefix(options.cwd.as_path()) else {
            errors.push(format!(
                "Unexpected path is outside the project and was not quarantined: {}",
                path.display()
            ));
            continue;
        };
        match Utf8PathBuf::from_path_buf(relative.to_path_buf()) {
            Ok(relative) => relative_paths.push(relative),
            Err(path) => errors.push(format!(
                "Unexpected path is not valid UTF-8 and was not quarantined: {}",
                path.display()
            )),
        }
    }
    let request = QuarantineRequest {
        project_root,
        recovery_root,
        run_id: run_id(),
        paths: relative_paths,
    };
    let plan = match plan_quarantine(&request, authorization) {
        Ok(plan) => plan,
        Err(error) => {
            errors.push(error.to_string());
            return (Vec::new(), errors);
        }
    };
    errors.extend(plan.diagnostics.into_iter().map(|diagnostic| {
        let detail = diagnostic.notes.join(" ");
        if detail.is_empty() {
            diagnostic.to_string()
        } else {
            format!("{diagnostic}: {detail}")
        }
    }));
    match execute_quarantine(&plan.items) {
        Ok(paths) => (paths, errors),
        Err(error) => {
            errors.push(error);
            (Vec::new(), errors)
        }
    }
}

pub(super) fn quarantine_roots(options: &FixOptions) -> Result<(Utf8PathBuf, Utf8PathBuf), String> {
    let recovery = match &options.backup {
        BackupPolicy::Directory(path) => path.join("quarantine"),
        BackupPolicy::Automatic | BackupPolicy::Disabled => {
            let root = automatic_backup_root().ok_or_else(|| {
                "No external recovery directory is available for quarantine.".to_owned()
            })?;
            root.join("quarantine")
        }
    };
    let project_root = options
        .cwd
        .canonicalize()
        .map_err(|error| format!("Could not resolve the project root: {error}"))?;
    if !project_root.is_dir() {
        return Err("The project root is not a directory.".to_owned());
    }
    let recovery = prepare_external_recovery_root(&recovery, &project_root)?;
    let project_root = Utf8PathBuf::from_path_buf(project_root)
        .map_err(|path| format!("The project root is not valid UTF-8: {}", path.display()))?;
    let recovery_root = Utf8PathBuf::from_path_buf(recovery)
        .map_err(|path| format!("The recovery path is not valid UTF-8: {}", path.display()))?;
    Ok((project_root, recovery_root))
}

pub(super) fn prepare_external_recovery_root(
    requested: &Path,
    project_root: &Path,
) -> Result<PathBuf, String> {
    let requested = absolute_lexical(requested);
    if requested.starts_with(project_root) || project_root.starts_with(&requested) {
        return Err(format!(
            "Quarantine recovery storage must not overlap the project: {}",
            requested.display()
        ));
    }
    let mut existing = requested.clone();
    let mut missing = Vec::<OsString>::new();
    loop {
        match std::fs::symlink_metadata(&existing) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() {
                    return Err(format!(
                        "Quarantine recovery path contains a symbolic link: {}",
                        existing.display()
                    ));
                }
                if !metadata.is_dir() {
                    return Err(format!(
                        "Quarantine recovery ancestor is not a directory: {}",
                        existing.display()
                    ));
                }
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let Some(name) = existing.file_name() else {
                    return Err(format!(
                        "Quarantine recovery path has no existing ancestor: {}",
                        requested.display()
                    ));
                };
                missing.push(name.to_os_string());
                if !existing.pop() {
                    return Err(format!(
                        "Quarantine recovery path has no existing ancestor: {}",
                        requested.display()
                    ));
                }
            }
            Err(error) => {
                return Err(format!(
                    "Could not inspect quarantine recovery path `{}`: {error}",
                    existing.display()
                ));
            }
        }
    }
    let mut canonical = existing.canonicalize().map_err(|error| {
        format!(
            "Could not resolve quarantine recovery ancestor `{}`: {error}",
            existing.display()
        )
    })?;
    if canonical.starts_with(project_root) || project_root.starts_with(&canonical) {
        return Err(format!(
            "Quarantine recovery storage must not overlap the project: {}",
            canonical.display()
        ));
    }
    for component in missing.into_iter().rev() {
        let next = canonical.join(component);
        match std::fs::create_dir(&next) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => {
                return Err(format!(
                    "Could not create quarantine recovery directory `{}`: {error}",
                    next.display()
                ));
            }
        }
        let metadata = std::fs::symlink_metadata(&next).map_err(|error| {
            format!(
                "Could not revalidate quarantine recovery directory `{}`: {error}",
                next.display()
            )
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(format!(
                "Quarantine recovery path is not a real directory: {}",
                next.display()
            ));
        }
        canonical = next.canonicalize().map_err(|error| {
            format!(
                "Could not resolve quarantine recovery directory `{}`: {error}",
                next.display()
            )
        })?;
        if canonical.starts_with(project_root) || project_root.starts_with(&canonical) {
            return Err(format!(
                "Quarantine recovery storage must not overlap the project: {}",
                canonical.display()
            ));
        }
    }
    Ok(canonical)
}

pub(super) fn execute_quarantine(items: &[QuarantineItem]) -> Result<Vec<Utf8PathBuf>, String> {
    for item in items {
        let metadata = std::fs::symlink_metadata(&item.source_path)
            .map_err(|error| format!("Could not revalidate `{}`: {error}", item.source_path))?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(format!(
                "Quarantine source is no longer a regular file: {}",
                item.source_path
            ));
        }
        if !quarantine_snapshot_matches(item)
            .map_err(|error| format!("Could not re-read `{}`: {error}", item.source_path))?
        {
            return Err(format!(
                "Quarantine source changed after planning: {}",
                item.source_path
            ));
        }
        if item.destination_path.exists() {
            return Err(format!(
                "Quarantine destination already exists: {}",
                item.destination_path
            ));
        }
    }

    let mut staged = Vec::<&QuarantineItem>::new();
    for item in items {
        let Some(parent) = item.destination_path.parent() else {
            cleanup_staged_quarantine(&staged);
            return Err(format!(
                "Quarantine destination has no parent: {}",
                item.destination_path
            ));
        };
        if let Err(error) = std::fs::create_dir_all(parent) {
            cleanup_staged_quarantine(&staged);
            return Err(format!(
                "Could not create quarantine directory `{parent}`: {error}"
            ));
        }
        let result = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&item.destination_path)
            .and_then(|mut file| {
                file.write_all(&item.snapshot.bytes)?;
                file.sync_all()
            });
        if let Err(error) = result {
            cleanup_staged_quarantine(&staged);
            return Err(format!(
                "Could not stage quarantine copy `{}`: {error}",
                item.destination_path
            ));
        }
        if item.snapshot.readonly {
            if let Ok(mut permissions) =
                std::fs::metadata(&item.destination_path).map(|metadata| metadata.permissions())
            {
                permissions.set_readonly(true);
                let _ = std::fs::set_permissions(&item.destination_path, permissions);
            }
        }
        staged.push(item);
    }

    let mut removed = Vec::<&QuarantineItem>::new();
    for item in items {
        if let Err(error) = std::fs::remove_file(&item.source_path) {
            let rollback_errors = rollback_quarantine(&removed, &staged);
            let suffix = if rollback_errors.is_empty() {
                "Previously moved files were restored.".to_owned()
            } else {
                format!("Rollback needs review: {}", rollback_errors.join(" | "))
            };
            return Err(format!(
                "Could not remove quarantined source `{}`: {error}. {suffix}",
                item.source_path
            ));
        }
        removed.push(item);
    }
    Ok(items
        .iter()
        .map(|item| item.relative_path.clone())
        .collect())
}

pub(super) fn cleanup_staged_quarantine(items: &[&QuarantineItem]) {
    for item in items.iter().rev() {
        let _ = std::fs::remove_file(&item.destination_path);
    }
}

pub(super) fn rollback_quarantine(
    removed: &[&QuarantineItem],
    staged: &[&QuarantineItem],
) -> Vec<String> {
    let mut errors = Vec::new();
    for item in removed.iter().rev() {
        let restored = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&item.restore_path)
            .and_then(|mut file| {
                file.write_all(&item.snapshot.bytes)?;
                file.sync_all()
            });
        match restored {
            Ok(()) => {
                if let Err(error) = std::fs::remove_file(&item.destination_path) {
                    errors.push(format!(
                        "restored `{}` but could not remove recovery copy: {error}",
                        item.restore_path
                    ));
                }
            }
            Err(error) => errors.push(format!(
                "could not restore `{}`: {error}",
                item.restore_path
            )),
        }
    }
    for item in staged {
        if !removed
            .iter()
            .any(|removed_item| removed_item.source_path == item.source_path)
        {
            let _ = std::fs::remove_file(&item.destination_path);
        }
    }
    errors
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::prepare_external_recovery_root;

    #[test]
    fn recovery_storage_refuses_any_overlap_with_the_project() {
        let project = TempDir::new().expect("project");
        let canonical = project.path().canonicalize().expect("canonical project");

        // Inside the project.
        assert!(
            prepare_external_recovery_root(&project.path().join("quarantine"), &canonical).is_err()
        );
        // An ancestor of the project.
        assert!(
            prepare_external_recovery_root(project.path().parent().expect("parent"), &canonical)
                .is_err()
        );
        // The project itself.
        assert!(prepare_external_recovery_root(project.path(), &canonical).is_err());
    }

    #[test]
    fn recovery_storage_creates_every_missing_level_and_is_repeatable() {
        let project = TempDir::new().expect("project");
        let storage = TempDir::new().expect("storage");
        let canonical = project.path().canonicalize().expect("canonical project");
        let requested = storage.path().join("normfix").join("quarantine");

        let created = prepare_external_recovery_root(&requested, &canonical).expect("first");
        assert!(created.is_dir());

        let again = prepare_external_recovery_root(&requested, &canonical).expect("second");
        assert_eq!(created, again);
    }

    #[cfg(unix)]
    #[test]
    fn recovery_storage_refuses_a_symbolic_link_ancestor() {
        use std::os::unix::fs::symlink;

        let project = TempDir::new().expect("project");
        let storage = TempDir::new().expect("storage");
        let canonical = project.path().canonicalize().expect("canonical project");
        let real = storage.path().join("real");
        let link = storage.path().join("link");
        fs::create_dir(&real).expect("real directory");
        symlink(&real, &link).expect("symbolic link");

        assert!(prepare_external_recovery_root(&link.join("quarantine"), &canonical).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn recovery_storage_refuses_a_link_that_redirects_into_the_project() {
        use std::os::unix::fs::symlink;

        let project = TempDir::new().expect("project");
        let storage = TempDir::new().expect("storage");
        let canonical = project.path().canonicalize().expect("canonical project");
        let inside = project.path().join("backups");
        let link = storage.path().join("link");
        fs::create_dir(&inside).expect("directory inside the project");
        symlink(&inside, &link).expect("symbolic link into the project");

        // The lexical prefix check passes; only resolving the link exposes the
        // overlap, which is why the check runs again after canonicalization.
        assert!(prepare_external_recovery_root(&link, &canonical).is_err());
    }
}
