//! Selection and atomic commit of shadow-buffer plans.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use camino::Utf8PathBuf;
use normfix_actions::{PlannedFile, ReadPrecondition, TransactionOptions, commit_files_guarded};

use super::destructive::DestructivePrelude;
use super::paths::{absolute_lexical, automatic_backup_root, run_id, transaction_root};
use super::{BackupPolicy, FileWork, FixOptions};

/// Selects plans to commit, or `None` when interactive approval expired.
fn plans_to_commit(
    work: &mut [FileWork],
    options: &FixOptions,
    dependent_destructive_bundle: &BTreeSet<PathBuf>,
) -> Option<Vec<PlannedFile>> {
    let plans = if let Some(approvals) = &options.write_approvals {
        let valid = approvals.iter().all(|(path, approval)| {
            work.iter()
                .find(|item| absolute_lexical(&item.absolute_path) == absolute_lexical(path))
                .and_then(|item| item.plan.as_ref())
                .is_some_and(|plan| approval.permits(plan))
        });
        if !valid {
            let message =
                "Interactive approval expired because the original or proposed bytes changed after preview; no files were written."
                    .to_owned();
            for item in work
                .iter_mut()
                .filter(|item| approvals.contains_key(&absolute_lexical(&item.absolute_path)))
            {
                item.report.failure = Some(message.clone());
            }
            return None;
        }
        work.iter()
            .filter(|item| approvals.contains_key(&absolute_lexical(&item.absolute_path)))
            .filter_map(|item| item.plan.clone())
            .collect::<Vec<_>>()
    } else {
        work.iter().filter_map(|item| item.plan.clone()).collect()
    };

    if dependent_destructive_bundle_is_partial(&plans, dependent_destructive_bundle) {
        let message = "The dependent static-function and orphan-prototype removal bundle became incomplete after validation or approval filtering; no files were written.";
        for item in work.iter_mut().filter(|item| {
            dependent_destructive_bundle.contains(&absolute_lexical(&item.absolute_path))
        }) {
            if item.report.failure.is_none() {
                item.report.failure = Some(message.to_owned());
            }
        }
        return None;
    }
    Some(plans)
}

pub(super) fn dependent_destructive_bundle_is_partial(
    plans: &[PlannedFile],
    dependent_destructive_bundle: &BTreeSet<PathBuf>,
) -> bool {
    let selected_bundle_paths = plans
        .iter()
        .filter(|plan| {
            plan.fixes.iter().any(|fix| {
                matches!(
                    fix.rule_id.as_str(),
                    "UNSAFE_REMOVE_UNUSED_STATIC" | "UNSAFE_REMOVE_ORPHAN_PROTOTYPE"
                )
            })
        })
        .map(|plan| absolute_lexical(&plan.path))
        .filter(|path| dependent_destructive_bundle.contains(path))
        .collect::<BTreeSet<_>>();
    !selected_bundle_paths.is_empty() && selected_bundle_paths != *dependent_destructive_bundle
}

/// Returns the exact files whose dependent destructive edits form one proof.
pub(super) fn dependent_destructive_bundle_paths(
    preludes: &BTreeMap<PathBuf, DestructivePrelude>,
) -> BTreeSet<PathBuf> {
    let mut has_static_removal = false;
    let mut has_orphan_removal = false;
    let mut paths = BTreeSet::new();
    for (path, prelude) in preludes {
        let participates = prelude.fixes.iter().any(|fix| {
            if fix.rule_id == "UNSAFE_REMOVE_UNUSED_STATIC" {
                has_static_removal = true;
                true
            } else if fix.rule_id == "UNSAFE_REMOVE_ORPHAN_PROTOTYPE" {
                has_orphan_removal = true;
                true
            } else {
                false
            }
        });
        if participates {
            paths.insert(absolute_lexical(path));
        }
    }
    if has_static_removal && has_orphan_removal {
        paths
    } else {
        BTreeSet::new()
    }
}

fn fail_selected(
    work: &mut [FileWork],
    selected_paths: &BTreeSet<PathBuf>,
    message: &str,
    clear_written: bool,
) {
    for item in work
        .iter_mut()
        .filter(|item| selected_paths.contains(&absolute_lexical(&item.absolute_path)))
    {
        item.report.failure = Some(message.to_owned());
        if clear_written {
            item.report.written = false;
        }
    }
}

pub(super) fn commit_work(
    work: &mut [FileWork],
    options: &FixOptions,
    dependent_destructive_bundle: &BTreeSet<PathBuf>,
) -> bool {
    let Some(plans) = plans_to_commit(work, options, dependent_destructive_bundle) else {
        return false;
    };
    if plans.is_empty() {
        return true;
    }
    let selected_paths = plans
        .iter()
        .map(|plan| absolute_lexical(&plan.path))
        .collect::<BTreeSet<_>>();
    let read_preconditions = work
        .iter()
        .filter(|item| selected_paths.contains(&absolute_lexical(&item.absolute_path)))
        .flat_map(|item| item.read_preconditions.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut transaction_paths = plans
        .iter()
        .map(|plan| absolute_lexical(&plan.path))
        .collect::<Vec<_>>();
    transaction_paths.extend(
        read_preconditions
            .iter()
            .map(|precondition| match precondition {
                ReadPrecondition::Matches { path, .. } | ReadPrecondition::Absent { path } => {
                    absolute_lexical(path)
                }
                ReadPrecondition::ProjectSources { root, .. } => absolute_lexical(root),
            }),
    );
    let cwd = absolute_lexical(&options.cwd);
    let project_root = if transaction_paths.iter().all(|path| path.starts_with(&cwd)) {
        cwd
    } else {
        transaction_root(transaction_paths.iter().map(PathBuf::as_path), &options.cwd)
    };
    let requires_recovery = plans.iter().any(|plan| {
        plan.fixes.iter().any(|fix| {
            matches!(
                fix.rule_id.as_str(),
                "UNSAFE_REMOVE_UNUSED_STATIC"
                    | "REMOVE_INVALID_COMMENT"
                    | "MAKEFILE_REMOVE_MISSING_SOURCE"
                    | "MAKEFILE_REMOVE_EMPTY_SOURCE"
                    | "UNSAFE_REMOVE_ORPHAN_PROTOTYPE"
            )
        })
    });
    let backup_root = match &options.backup {
        BackupPolicy::Automatic => automatic_backup_root(),
        BackupPolicy::Directory(path) => Some(path.clone()),
        BackupPolicy::Disabled if requires_recovery => automatic_backup_root(),
        BackupPolicy::Disabled => None,
    };
    let backup_required = matches!(&options.backup, BackupPolicy::Automatic) || requires_recovery;
    if backup_required && backup_root.is_none() {
        let message = "The write was refused because no external backup directory is available. Configure HOME, XDG_DATA_HOME, or --backup-dir; use --no-backup only for ordinary non-destructive formatting.";
        fail_selected(work, &selected_paths, message, false);
        return false;
    }
    let transaction_options = TransactionOptions {
        project_root,
        run_id: run_id(),
        backup_root,
    };
    match commit_files_guarded(plans, &transaction_options, &read_preconditions) {
        Ok(committed) => {
            let committed = committed
                .files
                .into_iter()
                .map(|file| (file.path, file.backup))
                .collect::<BTreeMap<_, _>>();
            for item in work {
                if let Some(backup) = committed.get(&item.absolute_path) {
                    item.report.written = true;
                    item.report.backup = backup
                        .as_ref()
                        .and_then(|path| Utf8PathBuf::from_path_buf(path.clone()).ok());
                }
            }
            true
        }
        Err(error) => {
            let message = format!("The atomic write transaction failed: {error}");
            fail_selected(work, &selected_paths, &message, true);
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use normfix_actions::PlannedFile;
    use normfix_core::FixRecord;
    use tempfile::TempDir;

    use super::dependent_destructive_bundle_is_partial;

    fn destructive_test_plan(path: &std::path::Path, rule_id: &str) -> PlannedFile {
        PlannedFile {
            path: path.to_path_buf(),
            original: b"old\n".to_vec(),
            replacement: b"new\n".to_vec(),
            fixes: vec![FixRecord {
                rule_id: rule_id.to_owned(),
                description: "test".to_owned(),
                line: None,
                count: 1,
            }],
        }
    }

    #[test]
    fn ordinary_plan_cannot_impersonate_a_surviving_destructive_bundle_member() {
        let project = TempDir::new().expect("project");
        let source = project.path().join("unused.c");
        let header = project.path().join("api.h");
        let bundle = [source.clone(), header.clone()]
            .into_iter()
            .collect::<BTreeSet<_>>();
        let static_plan = destructive_test_plan(&source, "UNSAFE_REMOVE_UNUSED_STATIC");
        let ordinary_header_plan = destructive_test_plan(&header, "HEADER_GUARD_RENAME");

        assert!(dependent_destructive_bundle_is_partial(
            &[static_plan.clone(), ordinary_header_plan],
            &bundle,
        ));
        assert!(!dependent_destructive_bundle_is_partial(
            &[
                static_plan,
                destructive_test_plan(&header, "UNSAFE_REMOVE_ORPHAN_PROTOTYPE"),
            ],
            &bundle,
        ));
    }
}
