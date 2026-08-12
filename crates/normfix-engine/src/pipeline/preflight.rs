//! Pre-defense advisories.
//!
//! Every entry here states a coverage limit rather than a source problem: what
//! preflight could not check is as important as what it could.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use camino::Utf8PathBuf;
use normfix_core::{Diagnostic, DiagnosticSource, Severity, TextRange, TextSize};
use normfix_i18n::DiagnosticKey;
use normfix_project::{DiscoveredFile, ProjectFileKind};

use super::FixOptions;
use super::diagnostics::localized_text;
use super::paths::{absolute_lexical, report_path};

// Every preflight notice is written here so the complete set of pre-defense
// advisories stays readable as one sequence instead of scattered emitters.
#[allow(clippy::too_many_lines)]
pub(super) fn append_preflight_diagnostics(
    diagnostics: &mut BTreeMap<PathBuf, Vec<Diagnostic>>,
    selected: &[DiscoveredFile],
    options: &FixOptions,
) {
    if !options.preflight {
        return;
    }
    let Some(notice_file) = selected.iter().find(|file| {
        matches!(
            file.kind,
            ProjectFileKind::CSource | ProjectFileKind::CHeader
        )
    }) else {
        return;
    };
    let selected_makefiles = selected
        .iter()
        .filter(|file| file.kind == ProjectFileKind::Makefile)
        .map(|file| absolute_lexical(&file.path))
        .collect::<BTreeSet<_>>();
    let root_makefiles = root_regular_makefiles(&options.cwd);
    let unevaluated_root_makefile = root_makefiles
        .iter()
        .any(|path| !selected_makefiles.contains(path));
    if selected_makefiles.is_empty() && root_makefiles.is_empty() {
        let path = report_path(&notice_file.path, &options.cwd)
            .unwrap_or_else(|_| Utf8PathBuf::from(notice_file.path.to_string_lossy().as_ref()));
        diagnostics
            .entry(notice_file.path.clone())
            .or_default()
            .push({
                let text = localized_text(options.locale, DiagnosticKey::MakefileNotFound, &[]);
                Diagnostic {
                    rule_id: "MAKEFILE_NOT_FOUND".to_owned(),
                    path,
                    range: TextRange::empty(TextSize::new(0)),
                    severity: Severity::Info,
                    message: text.message,
                    source: DiagnosticSource::Project,
                    notes: text.notes,
                    help: text.help,
                    localized: text.localized,
                }
            });
    } else if unevaluated_root_makefile {
        let path = report_path(&notice_file.path, &options.cwd)
            .unwrap_or_else(|_| Utf8PathBuf::from(notice_file.path.to_string_lossy().as_ref()));
        diagnostics
            .entry(notice_file.path.clone())
            .or_default()
            .push({
                let text = localized_text(options.locale, DiagnosticKey::MakefileNotEvaluated, &[]);
                Diagnostic {
                    rule_id: "MAKEFILE_NOT_EVALUATED".to_owned(),
                    path,
                    range: TextRange::empty(TextSize::new(0)),
                    severity: Severity::Warning,
                    message: text.message,
                    source: DiagnosticSource::Project,
                    notes: text.notes,
                    help: text.help,
                    localized: text.localized,
                }
            });
    }
    let Some(file) = selected
        .iter()
        .find(|file| file.kind == ProjectFileKind::CSource)
    else {
        return;
    };
    let path = report_path(&file.path, &options.cwd)
        .unwrap_or_else(|_| Utf8PathBuf::from(file.path.to_string_lossy().as_ref()));
    let clang_tidy = executable_on_path("clang-tidy").map_or_else(
        || "clang-tidy was not found on PATH; install it only if the project allows an additional local advisory pass.".to_owned(),
        |executable| {
            format!(
                "clang-tidy is available at `{}`; run it with the project's real include paths and compile flags, and review findings manually.",
                executable.display()
            )
        },
    );
    let leak_checker = leak_checker_note();
    diagnostics
        .entry(file.path.clone())
        .or_default()
        .push(Diagnostic {
            rule_id: "PREFLIGHT_MANUAL_STEPS".to_owned(),
            path,
            range: TextRange::empty(TextSize::new(0)),
            severity: Severity::Info,
            message:
                "Preflight does not execute project recipes, binaries, interactive tests, or runtime leak tools."
                    .to_owned(),
            source: DiagnosticSource::Project,
            notes: vec![
                "Run the subject's required make/relink sequence and functional tests in the evaluator environment."
                    .to_owned(),
                if options.preflight {
                    "Preflight automatically runs the bounded compiler analyzer; its findings are advisory and are not a runtime leak proof."
                        .to_owned()
                } else if options.analyzer {
                    "A compiler analyzer was requested, but its findings are advisory and are not a runtime leak proof."
                        .to_owned()
                } else {
                    "Use --analyzer for an additional static advisory, then confirm memory ownership at runtime."
                        .to_owned()
                },
                "For a separate local debug build, use AddressSanitizer and UndefinedBehaviorSanitizer (`-fsanitize=address,undefined -fno-omit-frame-pointer -g`) when your compiler supports them; do not silently change the submitted Makefile flags."
                    .to_owned(),
                "LeakSanitizer support varies by compiler and operating system; run the subject's required leak tool as the final runtime check."
                    .to_owned(),
                leak_checker,
                clang_tidy,
            ],
            help: Some(
                "Complete the subject-specific manual checks shown in the evaluation sheet before defense."
                    .to_owned(),
            ),
            localized: None,
        });
}

pub(super) fn root_regular_makefiles(root: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut makefiles = entries
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.eq_ignore_ascii_case("makefile"))
                && std::fs::symlink_metadata(entry.path())
                    .is_ok_and(|metadata| metadata.is_file() && !metadata.file_type().is_symlink())
        })
        .map(|entry| absolute_lexical(&entry.path()))
        .collect::<Vec<_>>();
    makefiles.sort();
    makefiles.dedup();
    makefiles
}

/// Whether a leak checker is available, and what to do about it either way.
///
/// Preflight looks for the tool rather than running it. Checking a `PATH` entry
/// reads a directory; running a leak checker runs the student's program, which
/// is the one thing preflight promises not to do — and it could not do it
/// anyway, because it reads source and has no way to know which binary is the
/// project's output or what arguments would exercise anything.
///
/// So it answers the question a reader has the evening before a defense: is the
/// tool here, and what exactly do I type.
fn leak_checker_note() -> String {
    if let Some(executable) = executable_on_path("valgrind") {
        return format!(
            "A leak checker is available at `{}`; check a built binary with `normfix leaks ./your_program`, which runs it and reports what one run observed.",
            executable.display()
        );
    }
    let install = if cfg!(target_os = "macos") {
        "upstream Valgrind does not build for macOS, but the LouisBrunner/valgrind-macos port does (`brew install LouisBrunner/valgrind/valgrind`), with limited Apple Silicon support"
    } else if cfg!(windows) {
        "Valgrind does not exist for Windows; run normfix inside WSL, where the Linux checker works normally"
    } else {
        "install Valgrind from your package manager"
    };
    format!(
        "No leak checker was found on PATH; to check a built binary with `normfix leaks`, {install}."
    )
}

pub(super) fn executable_on_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    let executable_name = OsString::from(format!("{name}{}", std::env::consts::EXE_SUFFIX));
    std::env::split_paths(&path)
        .filter(|directory| directory.is_absolute())
        .map(|directory| directory.join(&executable_name))
        .find(|candidate| {
            std::fs::metadata(candidate).is_ok_and(|metadata| is_executable_file(&metadata))
        })
}

pub(super) fn is_executable_file(metadata: &std::fs::Metadata) -> bool {
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}
