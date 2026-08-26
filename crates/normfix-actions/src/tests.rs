use std::{collections::BTreeSet, fs};

use normfix_core::FixRecord;
use tempfile::TempDir;

use super::{
    PlannedFile, ReadPrecondition, TransactionError, TransactionOptions, UndoError, commit_files,
    commit_files_guarded, list_undo_runs, read_journal, sha256_hex, undo_run,
    validate_committed_replacements, validate_read_preconditions_except, write_journal,
};

fn plan(path: &std::path::Path, replacement: &[u8]) -> PlannedFile {
    PlannedFile {
        path: path.to_path_buf(),
        original: fs::read(path).expect("fixture"),
        replacement: replacement.to_vec(),
        fixes: vec![FixRecord {
            rule_id: "TEST".to_owned(),
            description: "test replacement".to_owned(),
            line: Some(1),
            count: 1,
        }],
    }
}

#[test]
fn commits_sorted_files_with_external_backups_and_journal() {
    let project = TempDir::new().expect("project");
    let backups = TempDir::new().expect("backups");
    let a = project.path().join("a.c");
    let b = project.path().join("b.c");
    fs::write(&a, "old a\n").expect("a");
    fs::write(&b, "old b\n").expect("b");
    let options = TransactionOptions {
        project_root: project.path().to_path_buf(),
        run_id: "run-1".to_owned(),
        backup_root: Some(backups.path().to_path_buf()),
    };

    let report =
        commit_files(vec![plan(&b, b"new b\n"), plan(&a, b"new a\n")], &options).expect("commit");

    assert_eq!(fs::read(&a).expect("a"), b"new a\n");
    assert_eq!(fs::read(&b).expect("b"), b"new b\n");
    assert_eq!(report.files[0].path, a);
    assert_eq!(report.files[1].path, b);
    assert!(report.journal.as_ref().is_some_and(|path| path.is_file()));
    for file in &report.files {
        let backup = file.backup.as_ref().expect("backup");
        assert!(backup.is_file());
    }
    assert_eq!(report.files[0].original_sha256, sha256_hex(b"old a\n"));
}

#[test]
fn concurrent_modification_aborts_before_any_write() {
    let project = TempDir::new().expect("project");
    let a = project.path().join("a.c");
    let b = project.path().join("b.c");
    fs::write(&a, "old a\n").expect("a");
    fs::write(&b, "old b\n").expect("b");
    let first = plan(&a, b"new a\n");
    let second = plan(&b, b"new b\n");
    fs::write(&b, "external\n").expect("external change");
    let options = TransactionOptions {
        project_root: project.path().to_path_buf(),
        run_id: "run-2".to_owned(),
        backup_root: None,
    };

    assert!(matches!(
        commit_files(vec![first, second], &options),
        Err(TransactionError::ConcurrentModification(path)) if path == b
    ));
    assert_eq!(fs::read(&a).expect("a"), b"old a\n");
    assert_eq!(fs::read(&b).expect("b"), b"external\n");
}

#[test]
fn changed_read_precondition_aborts_before_backup_or_write() {
    let project = TempDir::new().expect("project");
    let backups = TempDir::new().expect("backups");
    let target = project.path().join("target.c");
    let observed = project.path().join("public.h");
    fs::write(&target, "old\n").expect("target");
    fs::write(&observed, "old declaration\n").expect("observed file");
    let precondition = ReadPrecondition::matches(&observed, b"old declaration\n");
    fs::write(&observed, "new declaration\n").expect("external change");
    let options = TransactionOptions {
        project_root: project.path().to_path_buf(),
        run_id: "run-read-set".to_owned(),
        backup_root: Some(backups.path().join("normfix")),
    };

    assert!(matches!(
        commit_files_guarded(vec![plan(&target, b"new\n")], &options, &[precondition]),
        Err(TransactionError::ConcurrentModification(path)) if path == observed
    ));
    assert_eq!(fs::read(&target).expect("target"), b"old\n");
    assert!(!backups.path().join("normfix").exists());
}

#[test]
fn newly_present_absence_precondition_aborts_before_write() {
    let project = TempDir::new().expect("project");
    let target = project.path().join("Makefile");
    let missing = project.path().join("missing.c");
    fs::write(&target, "SRCS = missing.c\n").expect("Makefile");
    let precondition = ReadPrecondition::absent(&missing);
    fs::write(&missing, "int main(void) { return (0); }\n").expect("new source");
    let options = TransactionOptions {
        project_root: project.path().to_path_buf(),
        run_id: "run-absence".to_owned(),
        backup_root: None,
    };

    assert!(matches!(
        commit_files_guarded(vec![plan(&target, b"SRCS =\n")], &options, &[precondition]),
        Err(TransactionError::ConcurrentModification(path)) if path == missing
    ));
    assert_eq!(fs::read(&target).expect("Makefile"), b"SRCS = missing.c\n");
}

#[test]
fn new_project_source_aborts_a_closed_world_commit() {
    let project = TempDir::new().expect("project");
    let target = project.path().join("main.c");
    let header = project.path().join("public.h");
    fs::write(&target, "int main(void) { return (0); }\n").expect("target");
    fs::write(&header, "int old_api(void);\n").expect("header");
    let precondition =
        ReadPrecondition::project_sources(project.path(), vec![target.clone(), header.clone()]);
    let introduced = project.path().join("late.c");
    fs::write(&introduced, "int old_api(void) { return (1); }\n").expect("late source");
    let options = TransactionOptions {
        project_root: project.path().to_path_buf(),
        run_id: "run-project-membership".to_owned(),
        backup_root: None,
    };

    assert!(matches!(
        commit_files_guarded(
            vec![plan(&header, b"\n")],
            &options,
            &[precondition],
        ),
        Err(TransactionError::ConcurrentModification(path)) if path == introduced
    ));
    assert_eq!(fs::read(&header).expect("header"), b"int old_api(void);\n");
}

#[test]
fn project_source_membership_is_rechecked_after_a_prior_replacement() {
    let project = TempDir::new().expect("project");
    let first = project.path().join("first.c");
    let second = project.path().join("second.h");
    fs::write(&first, "int first(void) { return (1); }\n").expect("first");
    fs::write(&second, "int second(void);\n").expect("second");
    let precondition =
        ReadPrecondition::project_sources(project.path(), vec![first.clone(), second.clone()]);
    let mut prepared = super::prepare_file_at(
        plan(&first, b"int first(void) { return (2); }\n"),
        first.clone(),
        None,
    )
    .expect("prepare first replacement");
    super::persist_staged(&mut prepared).expect("commit first replacement");
    let introduced = project.path().join("late.h");
    fs::write(&introduced, "int late(void);\n").expect("late header");
    let canonical = project.path().canonicalize().expect("canonical project");

    assert!(matches!(
        validate_read_preconditions_except(
            &[precondition],
            project.path(),
            &canonical,
            &BTreeSet::new(),
        ),
        Err(TransactionError::ConcurrentModification(path)) if path == introduced
    ));
}

#[test]
fn project_snapshot_preconditions_allow_a_multi_file_commit() {
    let project = TempDir::new().expect("project");
    let first = project.path().join("first.h");
    let second = project.path().join("second.h");
    fs::write(&first, "#ifndef FIRST\n#define FIRST\n#endif\n").expect("first");
    fs::write(&second, "#ifndef SECOND\n#define SECOND\n#endif\n").expect("second");
    let preconditions = [
        ReadPrecondition::matches(&first, b"#ifndef FIRST\n#define FIRST\n#endif\n"),
        ReadPrecondition::matches(&second, b"#ifndef SECOND\n#define SECOND\n#endif\n"),
    ];
    let options = TransactionOptions {
        project_root: project.path().to_path_buf(),
        run_id: "run-multi-read-set".to_owned(),
        backup_root: None,
    };

    commit_files_guarded(
        vec![
            plan(&first, b"#ifndef FIRST_H\n#define FIRST_H\n#endif\n"),
            plan(&second, b"#ifndef SECOND_H\n#define SECOND_H\n#endif\n"),
        ],
        &options,
        &preconditions,
    )
    .expect("multi-file guarded commit");

    assert_eq!(
        fs::read(&first).expect("first"),
        b"#ifndef FIRST_H\n#define FIRST_H\n#endif\n"
    );
    assert_eq!(
        fs::read(&second).expect("second"),
        b"#ifndef SECOND_H\n#define SECOND_H\n#endif\n"
    );
}

#[test]
fn a_committed_target_must_still_match_before_the_next_replacement() {
    let project = TempDir::new().expect("project");
    let first = project.path().join("first.h");
    fs::write(&first, "old\n").expect("first");
    let mut prepared = super::prepare_file_at(plan(&first, b"replacement\n"), first.clone(), None)
        .expect("prepare replacement");
    super::persist_staged(&mut prepared).expect("commit first replacement");
    fs::write(&first, "concurrent writer\n").expect("concurrent change");

    let error = validate_committed_replacements(&[prepared], &[0])
        .expect_err("the changed committed target must be detected");

    assert_eq!(error.0, first);
    assert!(
        error
            .1
            .to_string()
            .contains("changed after transaction preflight")
    );
}

#[test]
fn rejects_symlinks_and_backup_storage_inside_project() {
    let project = TempDir::new().expect("project");
    let source = project.path().join("main.c");
    fs::write(&source, "old\n").expect("source");
    let options = TransactionOptions {
        project_root: project.path().to_path_buf(),
        run_id: "run-3".to_owned(),
        backup_root: Some(project.path().join("backups")),
    };
    assert!(matches!(
        commit_files(vec![plan(&source, b"new\n")], &options),
        Err(TransactionError::BackupInsideProject(_))
    ));

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;

        let link = project.path().join("link.c");
        symlink(&source, &link).expect("symlink");
        let options = TransactionOptions {
            project_root: project.path().to_path_buf(),
            run_id: "run-4".to_owned(),
            backup_root: None,
        };
        let linked_plan = PlannedFile {
            path: link,
            original: b"old\n".to_vec(),
            replacement: b"new\n".to_vec(),
            fixes: Vec::new(),
        };
        assert!(matches!(
            commit_files(vec![linked_plan], &options),
            Err(TransactionError::Symlink(_))
        ));

        let external = TempDir::new().expect("external backup parent");
        let redirected = project.path().join("redirected-backups");
        fs::create_dir(&redirected).expect("inside-project backup target");
        let backup_link = external.path().join("backup-link");
        symlink(&redirected, &backup_link).expect("backup symlink");
        let options = TransactionOptions {
            project_root: project.path().to_path_buf(),
            run_id: "run-5".to_owned(),
            backup_root: Some(backup_link),
        };
        assert!(matches!(
            commit_files(vec![plan(&source, b"new\n")], &options),
            Err(TransactionError::Symlink(_) | TransactionError::BackupInsideProject(_))
        ));
    }
}

#[cfg(unix)]
#[test]
fn a_symlinked_project_root_uses_one_canonical_target_without_losing_report_paths() {
    use std::os::unix::fs::symlink;

    let container = TempDir::new().expect("container");
    let project = container.path().join("project");
    let alias = container.path().join("project-link");
    fs::create_dir(&project).expect("project");
    symlink(&project, &alias).expect("project alias");
    let canonical_target = project.join("main.c");
    let alias_target = alias.join("main.c");
    fs::write(&canonical_target, "old\n").expect("source");
    let options = TransactionOptions {
        project_root: alias.clone(),
        run_id: "symlink-root".to_owned(),
        backup_root: None,
    };

    let duplicate = commit_files(
        vec![
            plan(&canonical_target, b"canonical\n"),
            plan(&alias_target, b"alias\n"),
        ],
        &options,
    )
    .expect_err("two path vocabularies must not name the same target twice");
    assert!(
        matches!(duplicate, TransactionError::DuplicateTarget(_)),
        "unexpected error: {duplicate:?}"
    );
    assert_eq!(fs::read(&canonical_target).expect("unchanged"), b"old\n");

    let report = commit_files(vec![plan(&alias_target, b"new\n")], &options)
        .expect("commit through the selected root alias");
    assert_eq!(report.files[0].path, alias_target);
    assert_eq!(fs::read(&canonical_target).expect("committed"), b"new\n");
}

#[test]
fn lists_and_undoes_the_latest_intact_transaction() {
    let project = TempDir::new().expect("project");
    let backups = TempDir::new().expect("backups");
    let source = project.path().join("main.c");
    fs::write(&source, "old\n").expect("source");
    let options = TransactionOptions {
        project_root: project.path().to_path_buf(),
        run_id: "run-undo-test".to_owned(),
        backup_root: Some(backups.path().to_path_buf()),
    };
    commit_files(vec![plan(&source, b"new\n")], &options).expect("commit");

    let runs = list_undo_runs(backups.path(), project.path()).expect("runs");
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].run_id, "run-undo-test");
    let report = undo_run(&runs[0], project.path(), backups.path()).expect("undo");

    assert_eq!(fs::read(&source).expect("restored"), b"old\n");
    assert_eq!(report.files, vec![source]);
    assert!(report.journal.is_some());
}

#[test]
fn listing_and_undo_refuse_an_oversized_backup_before_allocating_it() {
    let project = TempDir::new().expect("project");
    let backups = TempDir::new().expect("backups");
    let source = project.path().join("main.c");
    fs::write(&source, "old\n").expect("source");
    let options = TransactionOptions {
        project_root: project.path().to_path_buf(),
        run_id: "run-oversized-backup".to_owned(),
        backup_root: Some(backups.path().to_path_buf()),
    };
    commit_files(vec![plan(&source, b"new\n")], &options).expect("commit");
    let run = list_undo_runs(backups.path(), project.path())
        .expect("runs")
        .pop()
        .expect("run");
    let journal = read_journal(&run.journal).expect("journal");
    let backup = journal.files[0].backup.as_ref().expect("backup").clone();
    fs::OpenOptions::new()
        .write(true)
        .open(&backup)
        .expect("open backup")
        .set_len(super::MAX_UNDO_FILE_BYTES + 1)
        .expect("oversized sparse backup");

    assert!(
        list_undo_runs(backups.path(), project.path())
            .expect("runs")
            .is_empty()
    );
    assert!(matches!(
        undo_run(&run, project.path(), backups.path()),
        Err(UndoError::Inspect { path, .. }) if path == backup
    ));
    assert_eq!(fs::read(&source).expect("unchanged"), b"new\n");
}

#[test]
fn undo_refuses_to_overwrite_changes_made_after_the_run() {
    let project = TempDir::new().expect("project");
    let backups = TempDir::new().expect("backups");
    let source = project.path().join("main.c");
    fs::write(&source, "old\n").expect("source");
    let options = TransactionOptions {
        project_root: project.path().to_path_buf(),
        run_id: "run-changed-test".to_owned(),
        backup_root: Some(backups.path().to_path_buf()),
    };
    commit_files(vec![plan(&source, b"new\n")], &options).expect("commit");
    let run = list_undo_runs(backups.path(), project.path())
        .expect("runs")
        .pop()
        .expect("run");
    fs::write(&source, "student edit\n").expect("external edit");

    assert!(matches!(
        undo_run(&run, project.path(), backups.path()),
        Err(UndoError::ModifiedSinceRun(path)) if path == source
    ));
    assert_eq!(fs::read(&source).expect("unchanged"), b"student edit\n");
}

#[cfg(unix)]
#[test]
fn backup_root_uses_the_canonical_target_of_a_compatibility_symlink() {
    use std::os::unix::fs::symlink;

    let project = TempDir::new().expect("project");
    let storage = TempDir::new().expect("storage");
    let real_backups = storage.path().join("real-backups");
    let compatibility_path = storage.path().join("compat-backups");
    fs::create_dir(&real_backups).expect("real backups");
    symlink(&real_backups, &compatibility_path).expect("compatibility symlink");
    let source = project.path().join("main.c");
    fs::write(&source, "old\n").expect("source");
    let options = TransactionOptions {
        project_root: project.path().to_path_buf(),
        run_id: "run-compat-link".to_owned(),
        backup_root: Some(compatibility_path.clone()),
    };

    let report = commit_files(vec![plan(&source, b"new\n")], &options).expect("commit");
    let canonical_backups = real_backups.canonicalize().expect("canonical backups");
    let journal = report.journal.expect("journal");
    assert!(journal.starts_with(&canonical_backups));

    fs::remove_file(&compatibility_path).expect("remove compatibility symlink");
    symlink(project.path(), &compatibility_path).expect("redirect compatibility path");
    assert!(journal.is_file());
    assert!(!project.path().join("run-compat-link").exists());
}

#[cfg(unix)]
#[test]
fn listing_and_undo_reject_a_symlinked_backup_parent_added_after_confirmation() {
    use std::os::unix::fs::symlink;

    let project = TempDir::new().expect("project");
    let backups = TempDir::new().expect("backups");
    let source = project.path().join("main.c");
    fs::write(&source, "old\n").expect("source");
    let options = TransactionOptions {
        project_root: project.path().to_path_buf(),
        run_id: "run-intermediate-link".to_owned(),
        backup_root: Some(backups.path().to_path_buf()),
    };
    commit_files(vec![plan(&source, b"new\n")], &options).expect("commit");
    let run = list_undo_runs(backups.path(), project.path())
        .expect("runs")
        .pop()
        .expect("run");
    let journal = read_journal(&run.journal).expect("journal");
    let backup = journal.files[0].backup.as_ref().expect("backup");
    let backup_parent = backup.parent().expect("backup parent");
    let relocated = backup_parent.with_file_name("relocated-backup-parent");
    fs::rename(backup_parent, &relocated).expect("relocate backup parent");
    symlink(&relocated, backup_parent).expect("intermediate symlink");

    assert!(
        list_undo_runs(backups.path(), project.path())
            .expect("runs")
            .is_empty()
    );
    assert!(matches!(
        undo_run(&run, project.path(), backups.path()),
        Err(UndoError::InvalidJournal { .. })
    ));
    assert_eq!(fs::read(&source).expect("unchanged"), b"new\n");
}

#[test]
fn undo_rejects_a_confirmed_run_whose_advertised_source_set_changed() {
    let project = TempDir::new().expect("project");
    let backups = TempDir::new().expect("backups");
    let source = project.path().join("main.c");
    fs::write(&source, "old\n").expect("source");
    let options = TransactionOptions {
        project_root: project.path().to_path_buf(),
        run_id: "run-source-set".to_owned(),
        backup_root: Some(backups.path().to_path_buf()),
    };
    commit_files(vec![plan(&source, b"new\n")], &options).expect("commit");
    let mut run = list_undo_runs(backups.path(), project.path())
        .expect("runs")
        .pop()
        .expect("run");
    run.files.push(project.path().join("unconfirmed.c"));

    assert!(matches!(
        undo_run(&run, project.path(), backups.path()),
        Err(UndoError::InvalidJournal { .. })
    ));
    assert_eq!(fs::read(&source).expect("unchanged"), b"new\n");
}

#[test]
fn undo_reloads_the_journal_and_rechecks_backup_confinement_and_hashes() {
    let project = TempDir::new().expect("project");
    let backups = TempDir::new().expect("backups");
    let source = project.path().join("main.c");
    fs::write(&source, "old\n").expect("source");
    let options = TransactionOptions {
        project_root: project.path().to_path_buf(),
        run_id: "run-revalidate".to_owned(),
        backup_root: Some(backups.path().to_path_buf()),
    };
    commit_files(vec![plan(&source, b"new\n")], &options).expect("commit");
    let run = list_undo_runs(backups.path(), project.path())
        .expect("runs")
        .pop()
        .expect("run");
    let mut journal = read_journal(&run.journal).expect("journal");
    let outside = backups.path().join("outside-run.backup");
    fs::write(&outside, "old\n").expect("outside backup");
    journal.files[0].backup = Some(outside);
    write_journal(&run.journal, &journal).expect("replace journal");

    assert!(matches!(
        undo_run(&run, project.path(), backups.path()),
        Err(UndoError::InvalidJournal { .. })
    ));
    assert_eq!(fs::read(&source).expect("unchanged"), b"new\n");
}

#[test]
fn undo_rechecks_backup_digest_after_the_run_was_listed() {
    let project = TempDir::new().expect("project");
    let backups = TempDir::new().expect("backups");
    let source = project.path().join("main.c");
    fs::write(&source, "old\n").expect("source");
    let options = TransactionOptions {
        project_root: project.path().to_path_buf(),
        run_id: "run-backup-hash".to_owned(),
        backup_root: Some(backups.path().to_path_buf()),
    };
    commit_files(vec![plan(&source, b"new\n")], &options).expect("commit");
    let run = list_undo_runs(backups.path(), project.path())
        .expect("runs")
        .pop()
        .expect("run");
    let journal = read_journal(&run.journal).expect("journal");
    let backup = journal.files[0].backup.as_ref().expect("backup");
    fs::write(backup, "tampered\n").expect("tamper backup");

    assert!(matches!(
        undo_run(&run, project.path(), backups.path()),
        Err(UndoError::BackupIntegrity(path)) if path == *backup
    ));
    assert_eq!(fs::read(&source).expect("unchanged"), b"new\n");
}
