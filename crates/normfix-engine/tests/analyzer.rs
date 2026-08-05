//! Deep-analyzer coverage against whatever compiler is actually installed.
//!
//! `--analyzer` means different flags per compiler family, and the family is
//! read from the compiler's own version banner rather than its command name.
//! That decision can only be proven against real compilers, so these tests are
//! ignored by default and run in the workflow jobs that have one.
//!
//! Set `NORMFIX_TEST_CC` to test a specific compiler; otherwise `cc` is used.

#![cfg(unix)]

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use normfix_engine::{BackupPolicy, FixOptions, run_fixes};
use normfix_header::Identity42;
use normfix_report::ReportMode;
use tempfile::TempDir;

/// Allocates and returns without freeing, which both analyzers can see.
const LEAKY: &str = concat!(
    "#include <stdlib.h>\n",
    "\n",
    "int\tmain(void)\n",
    "{\n",
    "\tchar\t*buffer;\n",
    "\n",
    "\tbuffer = malloc(10);\n",
    "\tif (!buffer)\n",
    "\t\treturn (1);\n",
    "\treturn (0);\n",
    "}\n",
);

/// Resolves the compiler under test to an absolute path.
///
/// `NORMFIX_TEST_CC` is written as a command name in CI, and the engine expects
/// a path rather than something to look up, so resolve it here.
fn compiler() -> Option<PathBuf> {
    let requested = PathBuf::from(std::env::var_os("NORMFIX_TEST_CC")?);
    if requested.components().count() > 1 {
        return Some(requested);
    }
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|directory| directory.join(&requested))
        .find(|candidate| candidate.is_file())
}

/// A resolved identity, because a run without one fails before the compiler
/// stage and would report a header problem instead of an analyzer result.
fn identity() -> Identity42 {
    Identity42 {
        login: "student".to_owned(),
        email: "student@student.42.fr".to_owned(),
        source: "analyzer test fixture".to_owned(),
        inferred_login: false,
        inferred_email: false,
    }
}

fn version_banner() -> String {
    let command = compiler().unwrap_or_else(|| PathBuf::from("cc"));
    Command::new(command)
        .arg("--version")
        .output()
        .map(|output| String::from_utf8_lossy(&output.stdout).to_ascii_lowercase())
        .unwrap_or_default()
}

fn analyzer_run(project: &TempDir) -> Vec<String> {
    let mut options = FixOptions::new(project.path());
    options.mode = ReportMode::Check;
    options.backup = BackupPolicy::Disabled;
    options.cache = false;
    options.threads = Some(1);
    options.compiler_preflight = true;
    options.analyzer = true;
    options.identity = Some(identity());
    options.compiler_executable = compiler();

    let report = run_fixes(&[], &options).expect("the pipeline must complete");
    report
        .files
        .iter()
        .flat_map(|file| &file.after)
        .map(|diagnostic| diagnostic.rule_id.clone())
        .collect()
}

#[test]
#[ignore = "requires a C compiler with a static analyzer"]
fn the_installed_compiler_analyzer_actually_runs() {
    let banner = version_banner();
    assert!(
        banner.contains("clang") || banner.contains("gcc") || banner.contains("free software"),
        "unrecognized compiler banner, cannot assert analyzer support: {banner}"
    );

    let project = TempDir::new().expect("temporary project");
    fs::write(project.path().join("leak.c"), LEAKY).expect("fixture");

    let rules = analyzer_run(&project);

    assert!(
        !rules.iter().any(|rule| rule == "CC_PREFLIGHT_UNAVAILABLE"),
        "no compiler was reachable, so this proves nothing. rules: {rules:?}"
    );
    assert!(
        !rules.iter().any(|rule| rule == "CC_ANALYZER_UNAVAILABLE"),
        "a real GCC or Clang reported no analyzer support; flags were chosen for the wrong family. rules: {rules:?}"
    );
    assert!(
        rules.iter().any(|rule| rule.starts_with("CC_ANALYZER")),
        "the analyzer produced no finding for an obvious leak. rules: {rules:?}"
    );
}

#[test]
#[ignore = "requires a C compiler with a static analyzer"]
fn a_leak_is_reported_and_never_changes_the_exit_status() {
    let project = TempDir::new().expect("temporary project");
    fs::write(project.path().join("leak.c"), LEAKY).expect("fixture");

    let mut options = FixOptions::new(project.path());
    options.mode = ReportMode::Check;
    options.backup = BackupPolicy::Disabled;
    options.cache = false;
    options.threads = Some(1);
    options.compiler_preflight = true;
    options.analyzer = true;
    options.identity = Some(identity());
    options.compiler_executable = compiler();

    let report = run_fixes(&[], &options).expect("the pipeline must complete");

    // Analyzer findings are advisory: they are informational, so they must not
    // be the reason a run reports remaining work.
    let analyzer_findings = report
        .files
        .iter()
        .flat_map(|file| &file.after)
        .filter(|diagnostic| diagnostic.rule_id.starts_with("CC_ANALYZER"))
        .collect::<Vec<_>>();
    assert!(!analyzer_findings.is_empty(), "expected analyzer findings");
    assert!(
        analyzer_findings
            .iter()
            .all(|diagnostic| diagnostic.severity == normfix_core::Severity::Info),
        "analyzer findings must stay informational"
    );
}
