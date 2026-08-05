//! Differential coverage against the official checker.
//!
//! Every other test uses a fake Norminette so it can run anywhere. These tests
//! use the real one, because the product promise is stated relative to it: a
//! run must never leave a file with more official diagnostics than it started
//! with, and must never break a file that compiled.
//!
//! They are ignored by default and run in the workflow job that installs the
//! exact official checker and a C compiler.

#![cfg(unix)]

use std::fs;
use std::path::Path;
use std::process::Command;

use normfix_engine::{BackupPolicy, FixOptions, run_fixes};
use normfix_header::Identity42;
use normfix_report::ReportMode;
use tempfile::TempDir;

/// Sources that exercise the layout rules a 42 project actually trips over.
/// Each one is valid C that the official checker rejects for formatting.
const CORPUS: &[(&str, &str)] = &[
    ("spacing.c", "int add(int a,int b){\nreturn a+b;\n}\n"),
    (
        "indent.c",
        "int\tvalue(void)\n{\n  int\tresult;\n\n  result = 1;\n  return (result);\n}\n",
    ),
    (
        "braces.c",
        "int\tpick(int flag)\n{\n\tif (flag) {\n\t\treturn (1);\n\t} else {\n\t\treturn (0);\n\t}\n}\n",
    ),
    (
        "includes.c",
        "# include \"zeta.h\"\n# include <stdlib.h>\n# include <limits.h>\n\nint\tmain(void)\n{\n\treturn (0);\n}\n",
    ),
    (
        "declarations.c",
        "int\tcount(void)\n{\n\tint a;\n\tchar\t*text;\n\ta = 0;\n\ttext = 0;\n\treturn (a);\n}\n",
    ),
    ("returns.c", "int\tanswer(void)\n{\n\treturn 42;\n}\n"),
    // Prototype alignment is only reported once the surrounding layout is
    // correct, so this file needed a second run before the pipeline learned to
    // re-consult the official checker within one invocation.
    (
        "masked.h",
        concat!(
            "#ifndef MASKED_H\n",
            "# define MASKED_H\n",
            "\n",
            "int ft_isalpha(int c);\n",
            "int ft_isdigit(int c);\n",
            "char *ft_strdup(const char *s);\n",
            "size_t ft_strlen(const char *s);\n",
            "\n",
            "#endif\n",
        ),
    ),
];

fn identity() -> Identity42 {
    Identity42 {
        login: "student".to_owned(),
        email: "student@student.42.fr".to_owned(),
        source: "differential test fixture".to_owned(),
        inferred_login: false,
        inferred_email: false,
    }
}

/// Counts the official diagnostics reported for one file.
fn norminette_errors(path: &Path) -> usize {
    let output = Command::new("norminette")
        .arg(path)
        .output()
        .expect("the official Norminette must be on PATH");
    let text = String::from_utf8_lossy(&output.stdout);
    text.lines()
        .filter(|line| line.trim_start().starts_with("Error:"))
        .count()
}

fn compiles(path: &Path) -> bool {
    Command::new("cc")
        .args(["-fsyntax-only", "-Wall", "-Wextra"])
        .arg(path)
        .output()
        .is_ok_and(|output| output.status.success())
}

fn fix_in_place(project: &TempDir) {
    let mut options = FixOptions::new(project.path());
    options.mode = ReportMode::Fix;
    options.identity = Some(identity());
    options.backup = BackupPolicy::Disabled;
    options.cache = false;
    options.threads = Some(2);
    options.compiler_preflight = false;

    run_fixes(&[], &options).expect("the pipeline must complete");
}

#[test]
#[ignore = "requires the official Norminette 3.3.59 command"]
fn a_run_never_increases_official_diagnostics() {
    for (name, source) in CORPUS {
        let project = TempDir::new().expect("temporary project");
        let path = project.path().join(name);
        fs::write(&path, source).expect("corpus source");

        let before = norminette_errors(&path);
        fix_in_place(&project);
        let after = norminette_errors(&path);

        assert!(
            after <= before,
            "{name}: official diagnostics rose from {before} to {after}\n{}",
            fs::read_to_string(&path).unwrap_or_default()
        );
    }
}

#[test]
#[ignore = "requires the official Norminette 3.3.59 command"]
fn a_run_is_idempotent_against_the_official_checker() {
    for (name, source) in CORPUS {
        let project = TempDir::new().expect("temporary project");
        let path = project.path().join(name);
        fs::write(&path, source).expect("corpus source");

        fix_in_place(&project);
        let once = fs::read_to_string(&path).expect("first result");
        fix_in_place(&project);
        let twice = fs::read_to_string(&path).expect("second result");

        assert_eq!(once, twice, "{name}: a second run kept changing the file");
    }
}

#[test]
#[ignore = "requires the official Norminette 3.3.59 command and a C compiler"]
fn a_run_never_breaks_a_file_that_compiled() {
    for (name, source) in CORPUS {
        let project = TempDir::new().expect("temporary project");
        let path = project.path().join(name);
        fs::write(&path, source).expect("corpus source");

        if !compiles(&path) {
            // The corpus entry needs project headers; the diagnostic and
            // idempotence properties still cover it.
            continue;
        }
        fix_in_place(&project);

        assert!(
            compiles(&path),
            "{name}: the file no longer compiles after formatting\n{}",
            fs::read_to_string(&path).unwrap_or_default()
        );
    }
}
