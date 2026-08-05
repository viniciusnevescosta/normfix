//! End-to-end coverage for the native formatting, transaction and reporting pipeline.

#![cfg(unix)]

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use normfix_core::{DiagnosticSource, Severity};
use normfix_destructive::{DestructiveCapability, DestructiveRequest};
use normfix_engine::{BackupPolicy, FixOptions, WriteApproval, run_fixes};
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
fn lint_only_reports_original_source_without_planning_a_header() {
    let fixture = Fixture::clean_oracle();
    let source_path = fixture.project.path().join("answer.c");
    fs::write(&source_path, CLEAN_SOURCE).expect("write source fixture");
    let mut options = fixture.options(ReportMode::Check);
    options.lint_only = true;

    let report = run_fixes(&[], &options).expect("lint pipeline");

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

    let report = run_fixes(&[], &fixture.options(ReportMode::Check)).expect("policy pipeline");
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

    let report = run_fixes(&[selected], &fixture.options(ReportMode::Check))
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

    let report = run_fixes(&[selected], &fixture.options(ReportMode::Check))
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

    let report = run_fixes(&[selected], &fixture.options(ReportMode::Check))
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

    let report = run_fixes(&[makefile], &fixture.options(ReportMode::Check))
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

    let report = run_fixes(&[], &options).expect("empty scope");

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
    let preview = run_fixes(&[], &preview_options).expect("preview pipeline");
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

    let report = run_fixes(&[], &options).expect("selective fix pipeline");

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

    let first = run_fixes(&[], &options).expect("first fixing pipeline");

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

    let report = run_fixes(&[], &options).expect("compiler diagnostics pipeline");

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

    let report = run_fixes(&[], &options).expect("compiler include context pipeline");

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

    let report = run_fixes(&[], &options).expect("incomplete compiler context pipeline");
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

    let reported = run_fixes(&[], &check).expect("missing source report");

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
    let fixed = run_fixes(&[], &check).expect("missing source removal");
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
        run_fixes(std::slice::from_ref(&makefile), &options).expect("nested Makefile check");

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

    let report = run_fixes(&[makefile], &options).expect("symlink-safe source reconciliation");
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
