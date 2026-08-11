//! End-to-end coverage for the native formatting, transaction and reporting pipeline.

#![cfg(unix)]

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use normfix_core::{DiagnosticSource, Severity};
use normfix_destructive::{DestructiveCapability, DestructiveRequest};
use normfix_engine::{BackupPolicy, FixOptions, FixRunError, WriteApproval, run_fixes};
use normfix_header::{Identity42, RunClock, build_c_header};
use normfix_report::{FileStatus, ReportMode, RunReport};
use tempfile::TempDir;

const CLEAN_SOURCE: &str = "int\tanswer(void)\n{\n\treturn (42);\n}\n";

/// Minimal compiler stub: preflight identifies the compiler before use.
const CC_VERSION_ONLY: &str = r#"
if [ "$1" = "--version" ]; then
    echo "gcc (fixture) 14.1"
fi
exit 0
"#;

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
        options.compiler_preflight = false;
        options
    }
}

fn executable_script(directory: &TempDir, name: &str, body: &str) -> PathBuf {
    let path = directory.path().join(name);
    fs::write(&path, format!("#!/bin/sh\nset -eu\n{body}\n")).expect("write fake tool");
    let mut permissions = fs::metadata(&path)
        .expect("fake tool metadata")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&path, permissions).expect("make fake tool executable");
    path
}

/// Runs the pipeline, waiting out a stub the kernel still reports as busy.
///
/// These tests run on parallel threads, and every one of them spawns the fake
/// Norminette it just wrote. A child forked by another thread still holds a
/// write descriptor to that file until it reaches its own `exec`, and Linux
/// refuses to `execve` a file while any writer exists. The result is an
/// occasional `Text file busy` that has nothing to do with normfix.
///
/// The same window is already waited out inside `normfix-oracle`'s own tests.
/// Retrying here only makes the fixture ready; every other error is returned
/// unchanged, so a test that means to observe a failure still observes it.
fn run_ready(inputs: &[PathBuf], options: &FixOptions) -> Result<RunReport, FixRunError> {
    for _ in 0..100 {
        match run_fixes(inputs, options) {
            Err(error) if error.to_string().contains("Text file busy") => {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            outcome => return outcome,
        }
    }
    run_fixes(inputs, options)
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

    let report = run_ready(&[], &fixture.options(ReportMode::Check)).expect("check pipeline");

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
fn lint_only_reports_original_source_without_planning_a_header() {
    let fixture = Fixture::clean_oracle();
    let source_path = fixture.project.path().join("answer.c");
    fs::write(&source_path, CLEAN_SOURCE).expect("write source fixture");
    let mut options = fixture.options(ReportMode::Check);
    options.lint_only = true;

    let report = run_ready(&[], &options).expect("lint pipeline");

    assert_eq!(
        fs::read_to_string(&source_path).expect("unchanged source"),
        CLEAN_SOURCE
    );
    assert_eq!(report.summary.changed, 0);
    assert!(report.files[0].fixes.is_empty());
}

#[test]
fn allowed_function_diagnostic_range_tracks_the_final_header_inserted_source() {
    let fixture = Fixture::clean_oracle();
    let source_path = fixture.project.path().join("allocate.c");
    let source = "void\t*allocate(void)\n{\n\treturn (malloc(1));\n}\n";
    fs::write(&source_path, source).expect("source fixture");
    fs::write(
        fixture.project.path().join("normfix.toml"),
        "[project]\nname = \"fixture\"\nallowed = []\n",
    )
    .expect("policy fixture");

    let report = run_ready(&[], &fixture.options(ReportMode::Check)).expect("policy pipeline");
    let source_report = report
        .files
        .iter()
        .find(|file| file.path.as_str() == "allocate.c")
        .expect("source report");
    let diagnostic = source_report
        .after
        .iter()
        .find(|diagnostic| diagnostic.rule_id == "FUNCTION_NOT_ALLOWED")
        .expect("disallowed malloc call");
    let fixed = source_report.fixed.as_deref().expect("final source");
    let start = diagnostic.range.start().get() as usize;
    let end = diagnostic.range.end().get() as usize;

    assert!(
        fixed.starts_with("/* ********"),
        "header should be inserted"
    );
    assert_eq!(&fixed[start..end], "malloc");
    assert_eq!(start, fixed.find("malloc").expect("malloc in final source"));
    assert!(start > source.find("malloc").expect("malloc in original source"));
}

#[test]
fn partial_selection_uses_non_static_definitions_from_the_complete_project() {
    let fixture = Fixture::clean_oracle();
    let selected = fixture.project.path().join("main.c");
    let unselected = fixture.project.path().join("helper.c");
    fs::write(
        &selected,
        "int\tmain(void)\n{\n\treturn (shared_helper());\n}\n",
    )
    .expect("selected source");
    fs::write(
        &unselected,
        "int\tshared_helper(void)\n{\n\treturn (42);\n}\n",
    )
    .expect("unselected definition");
    fs::write(
        fixture.project.path().join("normfix.toml"),
        "[project]\nallowed = []\n",
    )
    .expect("policy fixture");

    let report = run_ready(&[selected], &fixture.options(ReportMode::Check))
        .expect("partial policy pipeline");

    assert_eq!(report.files.len(), 1);
    assert!(report.files[0].after.iter().all(|diagnostic| {
        diagnostic.rule_id != "FUNCTION_NOT_ALLOWED"
            && diagnostic.rule_id != "FUNCTION_POLICY_PROOF_INCOMPLETE"
    }));
}

#[test]
fn a_static_definition_in_another_file_does_not_authorize_a_call() {
    let fixture = Fixture::clean_oracle();
    let selected = fixture.project.path().join("main.c");
    fs::write(
        &selected,
        "int\tmain(void)\n{\n\treturn (hidden_helper());\n}\n",
    )
    .expect("selected source");
    fs::write(
        fixture.project.path().join("private.c"),
        "static int\thidden_helper(void)\n{\n\treturn (42);\n}\n",
    )
    .expect("private definition");
    fs::write(
        fixture.project.path().join("normfix.toml"),
        "[project]\nallowed = []\n",
    )
    .expect("policy fixture");

    let report = run_ready(&[selected], &fixture.options(ReportMode::Check))
        .expect("static policy pipeline");

    assert!(
        report.files[0]
            .after
            .iter()
            .any(|diagnostic| diagnostic.rule_id == "FUNCTION_NOT_ALLOWED")
    );
}

#[test]
fn incomplete_complete_project_source_disables_all_allowlist_findings() {
    let fixture = Fixture::clean_oracle();
    let selected = fixture.project.path().join("main.c");
    fs::write(
        &selected,
        "int\tmain(void)\n{\n\treturn (forbidden_call());\n}\n",
    )
    .expect("selected source");
    fs::write(fixture.project.path().join("ambiguous.h"), [0xff, 0xfe])
        .expect("non-UTF-8 project header");
    fs::write(
        fixture.project.path().join("normfix.toml"),
        "[project]\nallowed = []\n",
    )
    .expect("policy fixture");

    let report = run_ready(&[selected], &fixture.options(ReportMode::Check))
        .expect("incomplete policy pipeline");

    assert!(
        report.files[0]
            .after
            .iter()
            .any(|diagnostic| { diagnostic.rule_id == "FUNCTION_POLICY_PROOF_INCOMPLETE" })
    );
    assert!(
        report.files[0]
            .after
            .iter()
            .all(|diagnostic| diagnostic.rule_id != "FUNCTION_NOT_ALLOWED")
    );
}

#[test]
fn a_makefile_only_scope_does_not_load_an_unrelated_function_policy() {
    let fixture = Fixture::clean_oracle();
    let makefile = fixture.project.path().join("Makefile");
    fs::write(&makefile, "NAME = demo\nall:\n").expect("Makefile fixture");
    fs::write(
        fixture.project.path().join("normfix.toml"),
        "[project]\nallowed = definitely-not-an-array\n",
    )
    .expect("invalid policy fixture");

    let report = run_ready(&[makefile], &fixture.options(ReportMode::Check))
        .expect("Makefile-only pipeline");

    assert!(report.files[0].after.iter().all(|diagnostic| {
        diagnostic.rule_id != "PROJECT_POLICY_INVALID"
            && diagnostic.rule_id != "FUNCTION_POLICY_PROOF_INCOMPLETE"
    }));
}

#[test]
fn explicit_empty_scope_does_not_fall_back_to_recursive_discovery() {
    let fixture = Fixture::clean_oracle();
    fs::write(fixture.project.path().join("answer.c"), CLEAN_SOURCE).expect("source fixture");
    let mut options = fixture.options(ReportMode::Check);
    options.empty_input_is_empty = true;
    options.norminette_executable = Some(fixture.project.path().join("not-used"));

    let report = run_ready(&[], &options).expect("empty scope");

    assert_eq!(report.summary.files, 0);
    assert_eq!(report.exit_code(), 0);
}

#[test]
fn fix_mode_can_commit_an_approved_subset_after_full_project_analysis() {
    let fixture = Fixture::clean_oracle();
    let first = fixture.project.path().join("first.c");
    let second = fixture.project.path().join("second.c");
    fs::write(&first, CLEAN_SOURCE).expect("first source fixture");
    fs::write(&second, CLEAN_SOURCE).expect("second source fixture");
    let clock = RunClock::fixed("2026/07/24 12:34:56").expect("fixed run clock");
    let mut preview_options = fixture.options(ReportMode::Check);
    preview_options.run_clock = Some(clock.clone());
    let preview = run_ready(&[], &preview_options).expect("preview pipeline");
    let approved = preview
        .files
        .iter()
        .find(|file| file.path.as_str() == "first.c")
        .expect("first preview");
    let approval = WriteApproval::new(
        approved.original.as_deref().expect("original").as_bytes(),
        approved.fixed.as_deref().expect("replacement").as_bytes(),
    );
    let mut options = fixture.options(ReportMode::Fix);
    options.run_clock = Some(clock);
    options.write_approvals = Some(BTreeMap::from([(first.clone(), approval)]));

    let report = run_ready(&[], &options).expect("selective fix pipeline");

    assert!(
        fs::read_to_string(&first)
            .expect("first source")
            .starts_with("/* ********")
    );
    assert_eq!(
        fs::read_to_string(&second).expect("second source"),
        CLEAN_SOURCE
    );
    assert_eq!(report.summary.changed, 2);
    assert_eq!(report.summary.written, 1);
}

#[test]
fn fix_mode_keeps_an_external_backup_and_the_second_run_is_idempotent() {
    let fixture = Fixture::clean_oracle();
    let backups = TempDir::new().expect("external backup root");
    let source_path = fixture.project.path().join("answer.c");
    fs::write(&source_path, CLEAN_SOURCE).expect("write source fixture");
    let mut options = fixture.options(ReportMode::Fix);
    options.backup = BackupPolicy::Directory(backups.path().to_path_buf());

    let first = run_ready(&[], &options).expect("first fixing pipeline");

    assert_eq!(first.exit_code(), 0);
    assert_eq!(first.summary.changed, 1);
    assert_eq!(first.summary.written, 1);
    assert_eq!(first.files[0].status(), FileStatus::Fixed);
    let backup = first.files[0]
        .backup
        .as_deref()
        .expect("external backup path");
    let canonical_backups = backups
        .path()
        .canonicalize()
        .expect("canonical backup root");
    assert!(backup.starts_with(&canonical_backups));
    assert_eq!(
        fs::read_to_string(backup).expect("exact backup bytes"),
        CLEAN_SOURCE
    );
    let fixed_once = fs::read(&source_path).expect("first fixed bytes");
    assert!(fixed_once.starts_with(b"/* ********"));

    let second = run_ready(&[], &options).expect("second fixing pipeline");

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

    let report = run_ready(&[], &options).expect("quarantine pipeline");

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
    fs::write(
        fixture.project.path().join("normfix.toml"),
        "[project]\nname = \"fixture\"\nallowed = []\n",
    )
    .expect("policy fixture");
    let tools = TempDir::new().expect("compiler tools");
    let compiler = executable_script(&tools, "cc", CC_VERSION_ONLY);
    let mut options = fixture.options(ReportMode::Check);
    options.preflight = true;
    options.compiler_executable = Some(compiler);

    let report = run_ready(&[], &options).expect("VLA pipeline");

    let before = &report.files[0].before;
    let after = &report.files[0].after;
    assert!(
        before
            .iter()
            .all(|diagnostic| diagnostic.rule_id != "VLA_FORBIDDEN")
    );
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
    assert!(
        report
            .evaluation
            .as_ref()
            .expect("preflight evaluation")
            .hard_failures
            .iter()
            .all(|finding| finding.rule_id != "VLA_FORBIDDEN")
    );
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

    let report = run_ready(&[], &fixture.options(ReportMode::Check))
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

#[test]
fn strict_compiler_and_analyzer_are_diagnostics_only_and_keep_source_writes_authorized() {
    let fixture = Fixture::clean_oracle();
    let tools = TempDir::new().expect("compiler tools");
    let compiler = executable_script(
        &tools,
        "cc",
        r#"
if [ "$1" = "--version" ]; then
    echo "gcc (fixture) 14.1"
    exit 0
fi
case " $* " in
    *" -fanalyzer "*)
        echo "$6:2:5: warning: leak of 'value' [-Wanalyzer-malloc-leak]" >&2
        exit 0
        ;;
esac
echo "$6:2:5: error: unused variable 'value' [-Werror=unused-variable]" >&2
exit 1
"#,
    );
    let source_path = fixture.project.path().join("warning.c");
    fs::write(
        &source_path,
        "int\twarning(void)\n{\n\tint\tvalue;\n\n\treturn (0);\n}\n",
    )
    .expect("warning source");
    let backups = TempDir::new().expect("backups");
    let mut options = fixture.options(ReportMode::Fix);
    options.backup = BackupPolicy::Directory(backups.path().to_path_buf());
    options.compiler_preflight = true;
    options.compiler_executable = Some(compiler);
    options.analyzer = true;

    let report = run_ready(&[], &options).expect("compiler diagnostics pipeline");

    assert!(
        report.files[0].written,
        "compiler findings must not gate writes"
    );
    assert!(report.files[0].after.iter().any(|diagnostic| {
        diagnostic.rule_id == "CC_UNUSED_VARIABLE"
            && diagnostic.source == DiagnosticSource::Compiler
            && diagnostic.severity == Severity::Error
    }));
    assert!(report.files[0].after.iter().any(|diagnostic| {
        diagnostic.rule_id == "CC_ANALYZER_MALLOC_LEAK"
            && diagnostic.source == DiagnosticSource::Compiler
            && diagnostic.severity == Severity::Info
    }));
}

#[test]
fn compiler_preflight_receives_stable_project_header_include_directories() {
    let fixture = Fixture::clean_oracle();
    let tools = TempDir::new().expect("compiler tools");
    let compiler = executable_script(
        &tools,
        "cc",
        r#"
if [ "$1" = "--version" ]; then
    echo "cc (fixture) 1.0"
    exit 0
fi
if [ "$#" -eq 10 ] &&
   [ "$1" = "-fsyntax-only" ] && [ "$2" = "-Wall" ] &&
   [ "$3" = "-Wextra" ] && [ "$4" = "-Werror" ] &&
   [ "$5" = "-I" ] && [ "$6" = "include/a" ] &&
   [ "$7" = "-I" ] && [ "$8" = "include/z" ] &&
   [ "$9" = "--" ] && [ "${10}" = "source.c" ]; then
    exit 0
fi
echo "source.c:1:1: error: unstable compiler arguments: $*" >&2
exit 1
"#,
    );
    fs::create_dir_all(fixture.project.path().join("include/a")).expect("first include directory");
    fs::create_dir_all(fixture.project.path().join("include/z")).expect("second include directory");
    fs::write(
        fixture.project.path().join("include/a/a.h"),
        "#define A 1\n",
    )
    .expect("first header");
    fs::write(
        fixture.project.path().join("include/z/z.h"),
        "#define Z 1\n",
    )
    .expect("second header");
    fs::write(fixture.project.path().join("source.c"), CLEAN_SOURCE).expect("source");
    let mut options = fixture.options(ReportMode::Check);
    options.compiler_preflight = true;
    options.compiler_executable = Some(compiler);

    let report = run_ready(&[], &options).expect("compiler include context pipeline");

    assert!(report.files.iter().all(|file| {
        file.after
            .iter()
            .all(|diagnostic| diagnostic.source != DiagnosticSource::Compiler)
    }));
}

#[test]
fn incomplete_compiler_context_is_a_clear_fail_open_advisory() {
    let fixture = Fixture::clean_oracle();
    let tools = TempDir::new().expect("compiler tools");
    let compiler = executable_script(
        &tools,
        "cc",
        r#"
if [ "$1" = "--version" ]; then
    echo "cc (fixture) 1.0"
    exit 0
fi
echo "source.c:1:10: fatal error: generated/config.h: No such file or directory" >&2
exit 1
"#,
    );
    fs::write(fixture.project.path().join("source.c"), CLEAN_SOURCE).expect("source");
    let mut options = fixture.options(ReportMode::Check);
    options.compiler_preflight = true;
    options.compiler_executable = Some(compiler);

    let report = run_ready(&[], &options).expect("incomplete compiler context pipeline");
    let source = report
        .files
        .iter()
        .find(|file| file.path.as_str() == "source.c")
        .expect("source report");

    assert!(source.after.iter().any(|diagnostic| {
        diagnostic.rule_id == "CC_PREFLIGHT_CONFIGURATION_INCOMPLETE"
            && diagnostic.severity == Severity::Info
            && diagnostic.source == DiagnosticSource::Compiler
    }));
    assert!(source.after.iter().all(|diagnostic| {
        diagnostic.rule_id != "CC_STRICT" || diagnostic.severity == Severity::Info
    }));
}

#[test]
fn missing_makefile_source_is_reported_then_removed_only_when_explicitly_enabled() {
    let fixture = Fixture::clean_oracle();
    let makefile = fixture.project.path().join("Makefile");
    let source = concat!(
        "NAME = demo\n",
        "SRC = present.c missing.c\n",
        "all: $(NAME)\n",
        "$(NAME):\n",
        "clean:\n",
        "fclean: clean\n",
        "re: fclean all\n"
    );
    fs::write(&makefile, source).expect("Makefile");
    fs::write(fixture.project.path().join("present.c"), CLEAN_SOURCE).expect("present source");
    let mut check = fixture.options(ReportMode::Check);

    let reported = run_ready(&[], &check).expect("missing source report");

    let make_report = reported
        .files
        .iter()
        .find(|file| file.path.as_str() == "Makefile")
        .expect("Makefile report");
    assert!(
        make_report
            .after
            .iter()
            .any(|diagnostic| diagnostic.rule_id == "MAKEFILE_SOURCE_NOT_FOUND")
    );
    assert_eq!(fs::read_to_string(&makefile).expect("unchanged"), source);

    let backups = TempDir::new().expect("external recovery");
    check.mode = ReportMode::Fix;
    check.backup = BackupPolicy::Directory(backups.path().to_path_buf());
    check.remove_missing_makefile_sources = true;
    check.destructive_authorization = Some(
        DestructiveRequest::one(DestructiveCapability::RemoveMissingMakefileSources)
            .authorize_forced(true, true)
            .expect("explicit Makefile-source removal authorization"),
    );
    let fixed = run_ready(&[], &check).expect("missing source removal");
    let make_report = fixed
        .files
        .iter()
        .find(|file| file.path.as_str() == "Makefile")
        .expect("Makefile report");
    assert!(make_report.written);
    assert!(
        make_report
            .fixes
            .iter()
            .any(|fix| fix.rule_id == "MAKEFILE_REMOVE_MISSING_SOURCE")
    );
    assert!(
        make_report
            .after
            .iter()
            .all(|diagnostic| diagnostic.rule_id != "MAKEFILE_SOURCE_NOT_FOUND")
    );
    assert!(
        !fs::read_to_string(&makefile)
            .expect("fixed")
            .contains("missing.c")
    );
    assert!(
        make_report.backup.is_some(),
        "unsafe removal needs recovery"
    );
}

#[test]
fn nested_makefile_sources_are_resolved_from_the_makefile_directory() {
    let fixture = Fixture::clean_oracle();
    let library = fixture.project.path().join("libft");
    fs::create_dir(&library).expect("library directory");
    let makefile = library.join("Makefile");
    fs::write(
        &makefile,
        "NAME = libft.a\nSRCS = existing.c missing.c\nall: $(NAME)\nclean:\nfclean: clean\nre: fclean all\n",
    )
    .expect("nested Makefile");
    fs::write(library.join("existing.c"), CLEAN_SOURCE).expect("existing nested source");
    let options = fixture.options(ReportMode::Check);

    let report =
        run_ready(std::slice::from_ref(&makefile), &options).expect("nested Makefile check");

    assert!(
        report.files[0]
            .after
            .iter()
            .any(
                |diagnostic| diagnostic.rule_id == "MAKEFILE_SOURCE_NOT_FOUND"
                    && diagnostic.message.contains("missing.c")
            )
    );
}

#[test]
fn makefile_source_removal_refuses_paths_through_symbolic_links() {
    use std::os::unix::fs::symlink;

    let fixture = Fixture::clean_oracle();
    let outside = TempDir::new().expect("outside directory");
    symlink(outside.path(), fixture.project.path().join("linked")).expect("project symlink");
    let makefile = fixture.project.path().join("Makefile");
    fs::write(
        &makefile,
        concat!(
            "NAME = demo\n",
            "SRC = linked/missing.c\n",
            "all: $(NAME)\n",
            "$(NAME):\n",
            "clean:\n",
            "fclean: clean\n",
            "re: fclean all\n"
        ),
    )
    .expect("Makefile");
    let mut options = fixture.options(ReportMode::Check);
    options.remove_missing_makefile_sources = true;
    options.destructive_authorization = Some(
        DestructiveRequest::one(DestructiveCapability::RemoveMissingMakefileSources)
            .authorize_forced(true, true)
            .expect("explicit Makefile-source removal authorization"),
    );

    let report = run_ready(&[makefile], &options).expect("symlink-safe source reconciliation");
    let make_report = &report.files[0];

    assert!(
        make_report
            .fixed
            .as_deref()
            .is_some_and(|fixed| { fixed.contains("linked/missing.c") })
    );
    assert!(
        make_report
            .fixes
            .iter()
            .all(|fix| fix.rule_id != "MAKEFILE_REMOVE_MISSING_SOURCE")
    );
    assert!(
        make_report
            .after
            .iter()
            .all(|diagnostic| diagnostic.rule_id != "MAKEFILE_SOURCE_NOT_FOUND")
    );
}

#[test]
fn untested_norminette_warns_once_for_a_header_only_scope() {
    let fixture = Fixture::new(
        r#"
if [ "$1" = "--version" ]; then
    echo "norminette 3.3.60"
    exit 0
fi
echo "$1: OK!"
"#,
    );
    fs::write(fixture.project.path().join("a.h"), "int\ta(void);\n").expect("first header");
    fs::write(fixture.project.path().join("b.h"), "int\tb(void);\n").expect("second header");
    let mut options = fixture.options(ReportMode::Check);
    options.lint_only = true;

    let report = run_ready(&[], &options).expect("header-only compatibility run");
    let warnings = report
        .files
        .iter()
        .flat_map(|file| &file.after)
        .filter(|diagnostic| diagnostic.rule_id == "NORMINETTE_VERSION_UNTESTED")
        .count();

    assert_eq!(warnings, 1);
}

#[test]
fn official_findings_use_the_detected_untested_norminette_version() {
    let fixture = Fixture::new(
        r#"
if [ "$1" = "--version" ]; then
    echo "norminette 3.3.60"
    exit 0
fi
echo "$1: Error!"
echo "Error: TOO_MANY_LINES (line: 1, col: 1): Function has more than 25 lines"
exit 1
"#,
    );
    fs::write(
        fixture.project.path().join("answer.h"),
        "int\tanswer(void);\n",
    )
    .expect("source fixture");
    let mut options = fixture.options(ReportMode::Check);
    options.lint_only = true;

    let report = run_ready(&[], &options).expect("untested-version lint");
    let source = &report.files[0].after;

    assert!(source.iter().any(|diagnostic| {
        diagnostic.rule_id == "TOO_MANY_LINES"
            && diagnostic.source == DiagnosticSource::NorminetteCompat("3.3.60".to_owned())
    }));
    assert!(source.iter().any(|diagnostic| {
        diagnostic.rule_id == "NORMINETTE_VERSION_UNTESTED"
            && diagnostic.source == DiagnosticSource::NorminetteCompat("3.3.60".to_owned())
    }));
}

#[test]
fn native_rule_overlap_preserves_distinct_official_locations() {
    let fixture = Fixture::new(
        r#"
if [ "$1" = "--version" ]; then
    echo "norminette 3.3.59"
    exit 0
fi
echo "$1: Error!"
echo "Error: TOO_MANY_LINES (line: 1, col: 1): First official occurrence"
echo "Error: TOO_MANY_LINES (line: 20, col: 1): Second official occurrence"
exit 1
"#,
    );
    let tools = TempDir::new().expect("compiler tools");
    let compiler = executable_script(&tools, "cc", "exit 0");
    let source = format!(
        "int\tlong_function(void)\n{{\n{}\treturn (0);\n}}\n",
        "\t(void)0;\n".repeat(26)
    );
    fs::write(fixture.project.path().join("long.c"), source).expect("long function");
    let mut options = fixture.options(ReportMode::Check);
    options.lint_only = true;
    options.preflight = true;
    options.compiler_executable = Some(compiler);

    let report = run_ready(&[], &options).expect("multi-location lint");
    let official = report.files[0]
        .after
        .iter()
        .filter(|diagnostic| {
            diagnostic.rule_id == "TOO_MANY_LINES"
                && matches!(diagnostic.source, DiagnosticSource::NorminetteCompat(_))
        })
        .collect::<Vec<_>>();
    let locations = official
        .iter()
        .map(|diagnostic| diagnostic.range.start())
        .collect::<std::collections::BTreeSet<_>>();

    assert_eq!(official.len(), 2);
    assert_eq!(locations.len(), 2);
}

#[test]
fn unproven_variable_array_bound_remains_an_on_disk_norm_failure() {
    let fixture = Fixture::new(
        r#"
if [ "$1" = "--version" ]; then
    echo "norminette 3.3.59"
    exit 0
fi
line_number=$(awk '/values\[count\]/{ print NR; exit }' "$1")
if [ -n "$line_number" ]; then
    echo "$1: Error!"
    echo "Error: VLA_FORBIDDEN (line: $line_number, col: 5): Variable length array forbidden"
    exit 1
fi
echo "$1: OK!"
"#,
    );
    let source_path = fixture.project.path().join("variable_array.c");
    let source =
        "int\tfirst_value(int count)\n{\n\tint\tvalues[count];\n\n\treturn (values[0]);\n}\n";
    fs::write(&source_path, source).expect("write variable VLA fixture");
    let tools = TempDir::new().expect("compiler tools");
    let compiler = executable_script(&tools, "cc", CC_VERSION_ONLY);
    let mut options = fixture.options(ReportMode::Check);
    options.lint_only = true;
    options.preflight = true;
    options.compiler_executable = Some(compiler);

    let report = run_ready(&[], &options).expect("unproven VLA pipeline");
    let file = &report.files[0];

    assert!(file.before.iter().any(|diagnostic| {
        diagnostic.rule_id == "VLA_FORBIDDEN"
            && matches!(diagnostic.source, DiagnosticSource::NorminetteCompat(_))
    }));
    assert!(
        file.after
            .iter()
            .any(|diagnostic| diagnostic.rule_id == "VLA_FORBIDDEN")
    );
    assert!(
        file.after
            .iter()
            .all(|diagnostic| diagnostic.rule_id != "VLA_COMPAT_FALSE_POSITIVE")
    );
    assert!(
        report
            .evaluation
            .as_ref()
            .expect("preflight evaluation")
            .hard_failures
            .iter()
            .any(|finding| finding.rule_id == "VLA_FORBIDDEN")
    );
}

#[test]
fn preflight_runs_the_bounded_analyzer_without_an_extra_flag() {
    let fixture = Fixture::clean_oracle();
    let tools = TempDir::new().expect("compiler tools");
    let compiler = executable_script(
        &tools,
        "cc",
        r#"
if [ "$1" = "--version" ]; then
    echo "gcc (fixture) 14.1"
    exit 0
fi
case " $* " in
    *" -fanalyzer "*)
        last=""
        for argument in "$@"; do
            last="$argument"
        done
        echo "$last:2:1: warning: leak of 'value' [-Wanalyzer-malloc-leak]" >&2
        ;;
esac
exit 0
"#,
    );
    fs::write(fixture.project.path().join("answer.c"), CLEAN_SOURCE).expect("source fixture");
    let mut options = fixture.options(ReportMode::Check);
    options.preflight = true;
    options.compiler_executable = Some(compiler);

    let report = run_ready(&[], &options).expect("automatic analyzer preflight");

    assert!(report.files[0].after.iter().any(|diagnostic| {
        diagnostic.rule_id == "CC_ANALYZER_MALLOC_LEAK"
            && diagnostic.source == DiagnosticSource::Compiler
            && diagnostic.severity == Severity::Info
    }));
}

#[test]
fn readme_advisory_is_present_only_when_a_readme_exists_and_never_hard_fails() {
    let fixture = Fixture::clean_oracle();
    let readme = fixture.project.path().join("README.md");
    fs::write(&readme, "# Demo\n").expect("README");
    let mut options = fixture.options(ReportMode::Check);
    options.preflight = true;

    let with_readme = run_ready(&[], &options).expect("README preflight");

    assert!(with_readme.files[0].after.iter().any(|diagnostic| {
        diagnostic.rule_id == "README_42_CRITERIA_REVIEW" && diagnostic.severity == Severity::Info
    }));
    assert!(
        with_readme
            .evaluation
            .as_ref()
            .expect("evaluation")
            .hard_failures
            .iter()
            .all(|finding| finding.rule_id != "README_42_CRITERIA_REVIEW")
    );

    fs::remove_file(&readme).expect("remove README fixture");
    let without_readme = run_ready(&[], &options).expect("no README preflight");

    assert!(without_readme.files.is_empty());
}

#[test]
fn missing_makefile_is_an_advisory_until_subject_policy_can_require_one() {
    let fixture = Fixture::clean_oracle();
    fs::write(fixture.project.path().join("answer.c"), CLEAN_SOURCE).expect("source fixture");
    let mut options = fixture.options(ReportMode::Check);
    options.preflight = true;

    let absent = run_ready(&[], &options).expect("preflight without Makefile");

    assert!(
        absent
            .files
            .iter()
            .flat_map(|file| &file.after)
            .any(|diagnostic| diagnostic.rule_id == "MAKEFILE_NOT_FOUND"
                && diagnostic.severity == Severity::Info)
    );
    assert!(
        absent
            .evaluation
            .as_ref()
            .expect("evaluation")
            .hard_failures
            .is_empty()
    );

    fs::write(fixture.project.path().join("Makefile"), "all:\n").expect("Makefile fixture");
    let present = run_ready(&[], &options).expect("preflight with Makefile");

    assert!(
        present
            .files
            .iter()
            .flat_map(|file| &file.after)
            .all(|diagnostic| diagnostic.rule_id != "MAKEFILE_NOT_FOUND")
    );
}

#[test]
fn explicit_header_preflight_reports_an_existing_unselected_root_makefile() {
    let fixture = Fixture::clean_oracle();
    let header = fixture.project.path().join("api.h");
    fs::write(&header, "int\tanswer(void);\n").expect("header");
    let makefile = fixture.project.path().join("Makefile");
    fs::write(&makefile, "all:\n").expect("Makefile fixture");
    let mut options = fixture.options(ReportMode::Check);
    options.preflight = true;

    let partial =
        run_ready(std::slice::from_ref(&header), &options).expect("explicit header-only preflight");

    assert!(partial.files.iter().all(|file| file.path != "Makefile"));
    assert!(
        partial
            .files
            .iter()
            .flat_map(|file| &file.after)
            .any(|diagnostic| diagnostic.rule_id == "MAKEFILE_NOT_EVALUATED"
                && diagnostic.severity == Severity::Warning)
    );

    let complete = run_ready(&[header, makefile], &options).expect("Makefile-selected preflight");

    assert!(complete.files.iter().any(|file| file.path == "Makefile"));
    assert!(
        complete
            .files
            .iter()
            .flat_map(|file| &file.after)
            .all(|diagnostic| diagnostic.rule_id != "MAKEFILE_NOT_EVALUATED")
    );
}

#[test]
fn trivia_only_makefile_source_is_reported_then_removed_only_with_authorization() {
    let fixture = Fixture::clean_oracle();
    let makefile = fixture.project.path().join("Makefile");
    fs::write(
        &makefile,
        "NAME = demo\nSRC = live.c placeholder.c\nall: $(NAME)\n$(NAME):\nclean:\nfclean: clean\nre: fclean all\n",
    )
    .expect("Makefile fixture");
    fs::write(fixture.project.path().join("live.c"), CLEAN_SOURCE).expect("live source");
    fs::write(
        fixture.project.path().join("placeholder.c"),
        "/* TODO: implement this source */\n",
    )
    .expect("placeholder source");

    let report = run_ready(&[], &fixture.options(ReportMode::Check)).expect("empty source check");
    let make_report = report
        .files
        .iter()
        .find(|file| file.path == "Makefile")
        .expect("Makefile report");

    assert!(make_report.after.iter().any(|diagnostic| {
        diagnostic.rule_id == "MAKEFILE_SOURCE_EMPTY"
            && diagnostic.message.contains("placeholder.c")
    }));
    assert!(
        fs::read_to_string(&makefile)
            .expect("unchanged")
            .contains("placeholder.c")
    );

    let mut authorized = fixture.options(ReportMode::Fix);
    authorized.remove_missing_makefile_sources = true;
    authorized.destructive_authorization = Some(
        DestructiveRequest::one(DestructiveCapability::RemoveMissingMakefileSources)
            .authorize_forced(true, true)
            .expect("explicit destructive authorization"),
    );

    let removed = run_ready(&[], &authorized).expect("empty source removal");
    let make_report = removed
        .files
        .iter()
        .find(|file| file.path == "Makefile")
        .expect("Makefile report");

    assert!(
        make_report
            .fixes
            .iter()
            .any(|fix| fix.rule_id == "MAKEFILE_REMOVE_EMPTY_SOURCE")
    );
    assert!(
        !fs::read_to_string(&makefile)
            .expect("fixed")
            .contains("placeholder.c")
    );
}

#[test]
fn default_check_reports_header_prototype_without_project_implementation_at_the_name() {
    let fixture = Fixture::clean_oracle();
    let header = fixture.project.path().join("api.h");
    let header_source = "int\tmissing_api(void);\n";
    fs::write(&header, header_source).expect("header");

    let report = run_ready(&[], &fixture.options(ReportMode::Check))
        .expect("missing implementation warning");
    let header_report = report
        .files
        .iter()
        .find(|file| file.path == "api.h")
        .expect("header report");
    let diagnostic = header_report
        .after
        .iter()
        .find(|diagnostic| diagnostic.rule_id == "HEADER_PROTOTYPE_IMPLEMENTATION_MISSING")
        .expect("missing implementation diagnostic");
    let planned = header_report
        .fixed
        .as_deref()
        .map_or_else(|| header_source.to_owned(), ToOwned::to_owned);
    let name = planned.find("missing_api").expect("prototype name");

    assert_eq!(
        u32::from(diagnostic.range.start()),
        u32::try_from(name).expect("in-range prototype offset")
    );
    assert!(report.evaluation.is_none());
}

#[test]
fn default_check_reports_a_trivia_only_implementation_without_removing_it() {
    let fixture = Fixture::clean_oracle();
    let header = fixture.project.path().join("api.h");
    fs::write(&header, "void\tplaceholder(void);\n").expect("shadow header");
    let implementation = fixture.project.path().join("placeholder.c");
    fs::write(
        &implementation,
        "void\tplaceholder(void)\n{\n\t/* TODO */\n}\n",
    )
    .expect("implementation");

    let report =
        run_ready(&[], &fixture.options(ReportMode::Check)).expect("empty implementation warning");
    let header_report = report
        .files
        .iter()
        .find(|file| file.path == "api.h")
        .expect("header report");

    assert!(
        header_report
            .after
            .iter()
            .any(|diagnostic| diagnostic.rule_id == "HEADER_PROTOTYPE_IMPLEMENTATION_EMPTY")
    );
    assert!(
        header_report
            .fixes
            .iter()
            .all(|fix| fix.rule_id != "UNSAFE_REMOVE_ORPHAN_PROTOTYPE")
    );
    assert!(
        fs::read_to_string(&implementation)
            .expect("unchanged implementation")
            .contains("TODO")
    );
}

#[test]
fn authorized_unsafe_mode_removes_only_an_unused_orphan_header_prototype() {
    let fixture = Fixture::clean_oracle();
    let backups = TempDir::new().expect("external recovery");
    let header = fixture.project.path().join("api.h");
    fs::write(&header, "int\tmissing_api(void);\nint\tanswer(void);\n").expect("header");
    fs::write(fixture.project.path().join("answer.c"), CLEAN_SOURCE).expect("implementation");
    let mut options = fixture.options(ReportMode::Fix);
    options.backup = BackupPolicy::Directory(backups.path().to_path_buf());
    options.remove_orphan_prototypes = true;
    options.destructive_authorization = Some(
        DestructiveRequest::one(DestructiveCapability::RemoveOrphanPrototypes)
            .authorize_forced(true, true)
            .expect("explicit destructive authorization"),
    );

    let report = run_ready(&[], &options).expect("orphan removal");
    let header_report = report
        .files
        .iter()
        .find(|file| file.path == "api.h")
        .expect("header report");

    assert!(header_report.written);
    assert!(
        header_report
            .fixes
            .iter()
            .any(|fix| fix.rule_id == "UNSAFE_REMOVE_ORPHAN_PROTOTYPE")
    );
    assert!(header_report.backup.is_some());
    let fixed = fs::read_to_string(&header).expect("fixed header");
    assert!(!fixed.contains("missing_api"));
    assert!(fixed.contains("answer"));
}

#[test]
fn unsafe_orphan_removal_is_blocked_when_project_code_references_the_api() {
    let fixture = Fixture::clean_oracle();
    let backups = TempDir::new().expect("external recovery");
    let header = fixture.project.path().join("api.h");
    fs::write(&header, "int\tmissing_api(void);\n").expect("header");
    fs::write(
        fixture.project.path().join("main.c"),
        "int\tmain(void)\n{\n\treturn (missing_api());\n}\n",
    )
    .expect("caller");
    let mut options = fixture.options(ReportMode::Fix);
    options.backup = BackupPolicy::Directory(backups.path().to_path_buf());
    options.remove_orphan_prototypes = true;
    options.destructive_authorization = Some(
        DestructiveRequest::one(DestructiveCapability::RemoveOrphanPrototypes)
            .authorize_forced(true, true)
            .expect("explicit destructive authorization"),
    );

    let report = run_ready(&[], &options).expect("blocked orphan removal");
    let header_report = report
        .files
        .iter()
        .find(|file| file.path == "api.h")
        .expect("header report");

    assert!(
        header_report
            .after
            .iter()
            .any(|diagnostic| diagnostic.rule_id == "UNSAFE_ORPHAN_PROTOTYPE_PROOF_BLOCKED")
    );
    assert!(
        header_report
            .fixed
            .as_deref()
            .is_some_and(|source| source.contains("missing_api"))
    );
    assert!(
        fs::read_to_string(&header)
            .expect("unchanged disk source")
            .contains("missing_api")
    );
}

#[test]
fn orphan_prototype_removal_refuses_a_partial_path_scope() {
    let fixture = Fixture::clean_oracle();
    let header = fixture.project.path().join("api.h");
    fs::write(&header, "int\tmissing_api(void);\n").expect("header");
    fs::write(fixture.project.path().join("other.c"), CLEAN_SOURCE).expect("other source");
    let mut options = fixture.options(ReportMode::Check);
    options.remove_orphan_prototypes = true;
    options.destructive_authorization = Some(
        DestructiveRequest::one(DestructiveCapability::RemoveOrphanPrototypes)
            .authorize_forced(true, true)
            .expect("authorization"),
    );

    let report = run_ready(std::slice::from_ref(&header), &options).expect("partial scope");

    assert!(report.files[0].after.iter().any(
        |diagnostic| diagnostic.rule_id == "UNSAFE_ORPHAN_PROTOTYPE_CLOSED_SET_INCOMPLETE"
    ));
    assert!(
        report.files[0]
            .fixes
            .iter()
            .all(|fix| fix.rule_id != "UNSAFE_REMOVE_ORPHAN_PROTOTYPE")
    );
    assert!(
        report.files[0]
            .fixed
            .as_deref()
            .is_some_and(|source| source.contains("missing_api"))
    );
    assert!(
        fs::read_to_string(&header)
            .expect("unchanged disk source")
            .contains("missing_api")
    );
}

#[test]
fn preflight_hard_fails_original_norm_and_makefile_errors_even_when_shadow_fixes_them() {
    let fixture = Fixture::new(
        r#"
if [ "$1" = "--version" ]; then
    echo "norminette 3.3.59"
    exit 0
fi
if [ "$(head -n 1 "$1")" = "/* ************************************************************************** */" ]; then
    echo "$1: OK!"
    exit 0
fi
echo "$1: Error!"
echo "Error: TOO_MANY_LINES (line: 1, col: 1): Function has more than 25 lines"
exit 1
"#,
    );
    let tools = TempDir::new().expect("compiler tools");
    let compiler = executable_script(&tools, "cc", CC_VERSION_ONLY);
    fs::write(fixture.project.path().join("main.c"), CLEAN_SOURCE).expect("source fixture");
    fs::write(
        fixture.project.path().join("Makefile"),
        "NAME = demo\nall: $(NAME)\n$(NAME):\nclean:\nfclean: clean\nre: fclean all\n",
    )
    .expect("Makefile fixture");
    let mut options = fixture.options(ReportMode::Check);
    options.preflight = true;
    options.compiler_executable = Some(compiler);

    let report = run_ready(&[], &options).expect("snapshot-based preflight");
    let evaluation = report.evaluation.as_ref().expect("evaluation");

    assert!(!evaluation.conclusive);
    assert!(evaluation.hard_failures.iter().any(|finding| {
        finding.rule_id == "TOO_MANY_LINES"
            && finding.path == "main.c"
            && (finding.line, finding.column) == (Some(1), Some(1))
    }));
    assert!(evaluation.hard_failures.iter().any(|finding| {
        finding.rule_id == "INVALID_HEADER"
            && finding.path == "Makefile"
            && (finding.line, finding.column) == (Some(1), Some(1))
    }));
    assert!(report.files.iter().all(|file| {
        file.after.iter().all(|diagnostic| {
            diagnostic.rule_id != "TOO_MANY_LINES" && diagnostic.rule_id != "INVALID_HEADER"
        })
    }));
}

#[test]
fn a_piscina_scope_of_loose_c_files_is_a_clean_preflight() {
    // A piscina exercise is expected to contain only `.c` files. Neither a
    // Makefile nor a project header exists, and neither absence may cost a
    // point or produce anything above an advisory.
    let fixture = Fixture::clean_oracle();
    fs::write(
        fixture.project.path().join("normfix.toml"),
        "[project]\nname = \"fixture\"\nallowed = []\n",
    )
    .expect("policy fixture");
    let header = build_c_header(
        "ft_strlen.c",
        &identity(),
        &RunClock::fixed("2026/07/23 12:34:56").expect("fixed test clock"),
    )
    .expect("valid official header");
    fs::write(
        fixture.project.path().join("ft_strlen.c"),
        format!("{header}\n\nint\tft_strlen(void)\n{{\n\treturn (0);\n}}\n"),
    )
    .expect("piscina source");
    let tools = TempDir::new().expect("compiler tools");
    let compiler = executable_script(&tools, "cc", CC_VERSION_ONLY);
    let mut options = fixture.options(ReportMode::Check);
    options.preflight = true;
    options.compiler_executable = Some(compiler);

    let report = run_ready(&[], &options).expect("piscina preflight");
    let evaluation = report.evaluation.as_ref().expect("preflight evaluation");

    assert!(evaluation.hard_failures.is_empty());
    assert_eq!(report.summary.remaining, 0);
    // A perfect score: the missing Makefile is normal here, not a deduction.
    assert_eq!(evaluation.score, 100);

    let makefile_notice = report
        .files
        .iter()
        .flat_map(|file| &file.after)
        .find(|diagnostic| diagnostic.rule_id == "MAKEFILE_NOT_FOUND")
        .expect("the absence is still reported, just not as a problem");
    assert_eq!(makefile_notice.severity, Severity::Info);
    assert!(
        makefile_notice
            .notes
            .iter()
            .any(|note| note.contains("piscina"))
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
