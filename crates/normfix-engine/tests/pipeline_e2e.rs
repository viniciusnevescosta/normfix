//! End-to-end coverage for the native formatting, transaction and reporting pipeline.

#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use normfix_destructive::{DestructiveCapability, DestructiveRequest};
use normfix_engine::{BackupPolicy, FixOptions, run_fixes};
use normfix_header::{Identity42, RunClock, build_c_header};
use normfix_report::{FileStatus, ReportMode};
use tempfile::TempDir;

const CLEAN_SOURCE: &str = "int\tanswer(void)\n{\n\treturn (42);\n}\n";

struct Fixture {
    project: TempDir,
    _tools: TempDir,
    norminette: PathBuf,
}

impl Fixture {
    fn new(script_body: &str) -> Self {
        let project = TempDir::new().expect("temporary project");
        let tools = TempDir::new().expect("temporary tool directory");
        let norminette = tools.path().join("norminette");
        fs::write(&norminette, format!("#!/bin/sh\nset -eu\n{script_body}\n"))
            .expect("write fake Norminette");
        let mut permissions = fs::metadata(&norminette)
            .expect("fake Norminette metadata")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&norminette, permissions).expect("make fake Norminette executable");
        Self {
            project,
            _tools: tools,
            norminette,
        }
    }

    fn clean_oracle() -> Self {
        Self::new(
            r#"
if [ "$1" = "--version" ]; then
    echo "norminette 3.3.59"
    exit 0
fi
echo "$1: OK!"
"#,
        )
    }

    fn options(&self, mode: ReportMode) -> FixOptions {
        let mut options = FixOptions::new(self.project.path());
        options.mode = mode;
        options.identity = Some(identity());
        options.backup = BackupPolicy::Disabled;
        options.norminette_executable = Some(self.norminette.clone());
        options.cache = false;
        options.threads = Some(2);
        options
    }
}

fn identity() -> Identity42 {
    Identity42 {
        login: "student".to_owned(),
        email: "student@student.42.fr".to_owned(),
        source: "end-to-end test fixture".to_owned(),
        inferred_login: false,
        inferred_email: false,
    }
}

#[test]
fn check_mode_plans_the_official_header_without_writing() {
    let fixture = Fixture::clean_oracle();
    let source_path = fixture.project.path().join("answer.c");
    fs::write(&source_path, CLEAN_SOURCE).expect("write source fixture");

    let report = run_fixes(&[], &fixture.options(ReportMode::Check)).expect("check pipeline");

    assert_eq!(
        fs::read_to_string(&source_path).expect("unchanged source"),
        CLEAN_SOURCE
    );
    assert_eq!(report.summary.files, 1);
    assert_eq!(report.summary.changed, 1);
    assert_eq!(report.summary.written, 0);
    assert_eq!(report.exit_code(), 1);
    assert_eq!(report.files[0].status(), FileStatus::WouldFix);
    assert!(!report.files[0].written);
    assert!(report.files[0].backup.is_none());
    assert!(
        report.files[0]
            .fixed
            .as_deref()
            .is_some_and(|source| source.starts_with("/* ********"))
    );
}

#[test]
fn fix_mode_keeps_an_external_backup_and_the_second_run_is_idempotent() {
    let fixture = Fixture::clean_oracle();
    let backups = TempDir::new().expect("external backup root");
    let source_path = fixture.project.path().join("answer.c");
    fs::write(&source_path, CLEAN_SOURCE).expect("write source fixture");
    let mut options = fixture.options(ReportMode::Fix);
    options.backup = BackupPolicy::Directory(backups.path().to_path_buf());

    let first = run_fixes(&[], &options).expect("first fixing pipeline");

    assert_eq!(first.exit_code(), 0);
    assert_eq!(first.summary.changed, 1);
    assert_eq!(first.summary.written, 1);
    assert_eq!(first.files[0].status(), FileStatus::Fixed);
    let backup = first.files[0]
        .backup
        .as_deref()
        .expect("external backup path");
    assert!(backup.starts_with(backups.path()));
    assert_eq!(
        fs::read_to_string(backup).expect("exact backup bytes"),
        CLEAN_SOURCE
    );
    let fixed_once = fs::read(&source_path).expect("first fixed bytes");
    assert!(fixed_once.starts_with(b"/* ********"));

    let second = run_fixes(&[], &options).expect("second fixing pipeline");

    assert_eq!(second.exit_code(), 0);
    assert_eq!(second.summary.changed, 0);
    assert_eq!(second.summary.written, 0);
    assert_eq!(second.summary.fixes, 0);
    assert_eq!(second.files[0].status(), FileStatus::Clean);
    assert!(second.files[0].backup.is_none());
    assert_eq!(
        fs::read(&source_path).expect("second fixed bytes"),
        fixed_once
    );
}

#[test]
fn authorized_quarantine_removes_an_unexpected_file_but_preserves_recovery_bytes() {
    let fixture = Fixture::clean_oracle();
    let backups = TempDir::new().expect("external recovery root");
    let source_path = fixture.project.path().join("answer.c");
    let unexpected_path = fixture.project.path().join("private.data");
    let unexpected_bytes = b"\0recoverable\xffbytes\n";
    fs::write(&source_path, CLEAN_SOURCE).expect("write source fixture");
    fs::write(&unexpected_path, unexpected_bytes).expect("write unexpected fixture");
    let request = DestructiveRequest::one(DestructiveCapability::QuarantineUnexpectedFiles);
    let authorization = request
        .authorize_forced(true, true)
        .expect("explicit destructive authorization");
    let mut options = fixture.options(ReportMode::Fix);
    options.backup = BackupPolicy::Directory(backups.path().to_path_buf());
    options.quarantine_unexpected = true;
    options.destructive_authorization = Some(authorization);

    let report = run_fixes(&[], &options).expect("quarantine pipeline");

    assert_eq!(report.exit_code(), 0);
    assert_eq!(
        report.quarantine_candidates,
        vec![PathBuf::from("private.data")]
    );
    assert_eq!(
        report.quarantined_files,
        vec![PathBuf::from("private.data")]
    );
    assert!(report.unexpected_files.is_empty());
    assert!(report.quarantine_errors.is_empty());
    assert_eq!(report.summary.quarantine_candidates, 1);
    assert_eq!(report.summary.quarantined, 1);
    assert!(!unexpected_path.exists());
    let quarantine_root = backups.path().join("quarantine");
    let run_directory = only_child_directory(&quarantine_root);
    assert_eq!(
        fs::read(run_directory.join("private.data")).expect("recovery bytes"),
        unexpected_bytes
    );
}

#[test]
fn proven_enum_array_bound_becomes_a_non_blocking_vla_advisory() {
    let fixture = Fixture::new(
        r#"
if [ "$1" = "--version" ]; then
    echo "norminette 3.3.59"
    exit 0
fi
line_number=$(awk '/g_values\[OP_TOTAL\]/{ print NR; exit }' "$1")
if [ -n "$line_number" ]; then
    echo "$1: Error!"
    echo "Error: VLA_FORBIDDEN (line: $line_number, col: 5): Variable length array forbidden"
    exit 1
fi
echo "$1: OK!"
"#,
    );
    let source_path = fixture.project.path().join("enum_array.c");
    let header = build_c_header(
        "enum_array.c",
        &identity(),
        &RunClock::fixed("2026/07/23 12:34:56").expect("fixed test clock"),
    )
    .expect("valid official header");
    let source = format!(
        "{header}\n\ntypedef enum e_operation\n{{\n\tOP_ZERO,\n\tOP_TOTAL\n}}\tt_operation;\n\nint\tg_values[OP_TOTAL];\n"
    );
    fs::write(&source_path, source).expect("write enum fixture");

    let report = run_fixes(&[], &fixture.options(ReportMode::Check)).expect("VLA pipeline");

    let after = &report.files[0].after;
    assert!(
        after
            .iter()
            .any(|diagnostic| diagnostic.rule_id == "VLA_COMPAT_FALSE_POSITIVE")
    );
    assert!(
        after
            .iter()
            .all(|diagnostic| diagnostic.rule_id != "VLA_FORBIDDEN")
    );
    assert_eq!(report.summary.remaining, 0);
    assert_eq!(report.summary.advisories, 1);
}

#[test]
fn raw_va_arg_type_recovery_is_a_non_blocking_parser_advisory() {
    let fixture = Fixture::clean_oracle();
    let source_path = fixture.project.path().join("variadic.c");
    let header = build_c_header(
        "variadic.c",
        &identity(),
        &RunClock::fixed("2026/07/23 12:34:56").expect("fixed test clock"),
    )
    .expect("valid official header");
    let source = format!(
        "{header}\n\n#include <stdarg.h>\n\nchar\t*next_string(va_list *args)\n{{\n\treturn (va_arg(*args, char *));\n}}\n"
    );
    fs::write(&source_path, source).expect("write variadic fixture");

    let report = run_fixes(&[], &fixture.options(ReportMode::Check))
        .expect("variadic compatibility pipeline");

    assert_eq!(report.summary.remaining, 0);
    assert!(
        report.files[0]
            .after
            .iter()
            .any(|diagnostic| diagnostic.rule_id == "C_PARSER_VA_ARG_COMPAT")
    );
    assert!(
        report.files[0]
            .after
            .iter()
            .all(|diagnostic| diagnostic.rule_id != "C_SYNTAX_RECOVERY")
    );
}

fn only_child_directory(root: &Path) -> PathBuf {
    let mut children = fs::read_dir(root)
        .expect("read quarantine root")
        .map(|entry| entry.expect("quarantine entry").path())
        .collect::<Vec<_>>();
    children.sort();
    assert_eq!(children.len(), 1);
    assert!(children[0].is_dir());
    children.remove(0)
}
