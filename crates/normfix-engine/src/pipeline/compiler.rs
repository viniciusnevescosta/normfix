//! The strict compiler pass and the bounded deep analyzer.
//!
//! Both are diagnostics-only: nothing here can authorize or reject a formatter
//! edit. The compiler is identified by its version banner rather than its
//! command name, because `/usr/bin/gcc` on macOS is Clang.

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fs::File;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use camino::Utf8PathBuf;
use normfix_core::{Diagnostic, DiagnosticSource, Severity, TextRange, TextSize};

use normfix_oracle::CompilerReport;
use normfix_project::{DiscoveredFile, DiscoveryOptions, ProjectFileKind, discover};
use regex::Regex;

use super::diagnostics::{ColumnUnit, diagnostic_range, point_diagnostic};
use super::paths::absolute_lexical;
use super::{FixOptions, OracleContext};

pub(super) const COMPILER_FINGERPRINT_FILE_LIMIT: u64 = 8 * 1024 * 1024;
pub(super) const COMPILER_FINGERPRINT_PROJECT_LIMIT: u64 = 64 * 1024 * 1024;

/// Drops the path-trace note that merely repeats its own finding.
///
/// The Clang analyzer reports a finding twice at the same position: once as a
/// warning tagged with the checker that produced it, and once as the first note
/// of the trace, untagged. Both are useful in a raw log and redundant in a
/// report, so keep the tagged one.
pub(super) fn deduplicate_analyzer_trace(diagnostics: &mut Vec<Diagnostic>) {
    pub(super) fn untagged(message: &str) -> &str {
        message
            .rfind(" [")
            .filter(|_| message.ends_with(']'))
            .map_or(message, |index| &message[..index])
    }

    let tagged = diagnostics
        .iter()
        .filter(|diagnostic| untagged(&diagnostic.message) != diagnostic.message)
        .map(|diagnostic| {
            (
                diagnostic.range.start(),
                untagged(&diagnostic.message).to_owned(),
            )
        })
        .collect::<BTreeSet<_>>();
    diagnostics.retain(|diagnostic| {
        untagged(&diagnostic.message) != diagnostic.message
            || !tagged.contains(&(diagnostic.range.start(), diagnostic.message.clone()))
    });
}

/// The analyzer a compiler actually ships, which is not the same question as
/// what the command is called.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CompilerFamily {
    Gcc,
    Clang,
    Unknown,
}

impl CompilerFamily {
    /// Classifies a compiler from its own version banner.
    ///
    /// Clang is checked first on purpose: `/usr/bin/gcc` on macOS is Clang
    /// wearing another name, and it answers `Apple clang version ...`. Trusting
    /// the command name would send `-fanalyzer` to a compiler that rejects it.
    pub(super) fn from_version(version_output: &str) -> Self {
        let banner = version_output.to_ascii_lowercase();
        if banner.contains("clang") {
            Self::Clang
        } else if banner.contains("gcc") || banner.contains("free software foundation") {
            Self::Gcc
        } else {
            Self::Unknown
        }
    }

    /// Stable cache namespace, because the flags differ per family.
    pub(super) const fn analyzer_namespace(self) -> &'static str {
        match self {
            Self::Clang => "clang-analyze-v1",
            Self::Gcc | Self::Unknown => "gcc-fanalyzer-v3",
        }
    }
}

pub(super) fn compiler_arguments(
    analyzer: bool,
    family: CompilerFamily,
    include_directories: &[PathBuf],
) -> Vec<OsString> {
    let mut arguments = Vec::<OsString>::new();
    if analyzer {
        match family {
            CompilerFamily::Clang => {
                // `--analyze` replaces the syntax-only mode. Passing both makes
                // Clang ignore the analyzer and warn about an unused argument.
                arguments.extend(
                    [
                        "--analyze",
                        "-Xclang",
                        "-analyzer-output=text",
                        "-Wall",
                        "-Wextra",
                    ]
                    .map(OsString::from),
                );
            }
            CompilerFamily::Gcc | CompilerFamily::Unknown => {
                arguments.extend(
                    ["-fsyntax-only", "-Wall", "-Wextra", "-fanalyzer"].map(OsString::from),
                );
            }
        }
    } else {
        arguments.extend(["-fsyntax-only", "-Wall", "-Wextra", "-Werror"].map(OsString::from));
    }
    for directory in include_directories {
        arguments.push(OsString::from("-I"));
        arguments.push(directory.as_os_str().to_owned());
    }
    arguments
}

#[derive(Default)]
pub(super) struct CompilerProjectContext {
    pub(super) fingerprint: Option<[u8; 32]>,
    pub(super) include_directories: Vec<PathBuf>,
}

pub(super) fn compiler_project_context(project_root: &Path) -> CompilerProjectContext {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"normfix-compiler-project-v2\0");
    let absolute_root = absolute_lexical(project_root);
    let discovery = discover(
        &[],
        &DiscoveryOptions::new(&absolute_root)
            .with_respect_gitignore(false)
            .with_respect_normfixignore(false),
    );
    let mut include_directories = discovery
        .processable_files
        .iter()
        .filter(|file| file.kind == ProjectFileKind::CHeader)
        .filter_map(|file| file.path.parent())
        .filter_map(|parent| parent.strip_prefix(&absolute_root).ok())
        .map(|relative| {
            if relative.as_os_str().is_empty() {
                PathBuf::from(".")
            } else {
                relative.to_path_buf()
            }
        })
        .collect::<Vec<_>>();
    include_directories.sort();
    include_directories.dedup();
    let mut paths = discovery
        .processable_files
        .into_iter()
        .map(|file| file.path)
        .chain(discovery.unexpected_files)
        .collect::<Vec<_>>();
    paths.sort();
    paths.dedup();
    let mut remaining_bytes = COMPILER_FINGERPRINT_PROJECT_LIMIT;
    let mut fingerprint_complete = discovery.errors.is_empty();
    for file in paths {
        if fingerprint_complete
            && !hash_compiler_project_file(&mut hasher, &file, &mut remaining_bytes)
        {
            fingerprint_complete = false;
        }
    }
    for error in discovery.errors {
        let detail = error.to_string();
        hasher.update(&u64::MAX.to_le_bytes());
        hasher.update(detail.as_bytes());
    }
    CompilerProjectContext {
        fingerprint: fingerprint_complete.then(|| *hasher.finalize().as_bytes()),
        include_directories,
    }
}

pub(super) fn hash_compiler_project_file(
    hasher: &mut blake3::Hasher,
    file: &Path,
    remaining_bytes: &mut u64,
) -> bool {
    let path = file.to_string_lossy();
    hasher.update(&(path.len() as u64).to_le_bytes());
    hasher.update(path.as_bytes());
    let input = match File::open(file) {
        Ok(input) => input,
        Err(error) => {
            let detail = error.to_string();
            hasher.update(&u64::MAX.to_le_bytes());
            hasher.update(detail.as_bytes());
            return false;
        }
    };
    let metadata = match input.metadata() {
        Ok(metadata) => metadata,
        Err(error) => {
            hasher.update(b"metadata-error\0");
            hasher.update(error.to_string().as_bytes());
            return false;
        }
    };
    let expected_length = metadata.len();
    let read_budget = expected_length.saturating_add(1);
    if !metadata.is_file()
        || expected_length > COMPILER_FINGERPRINT_FILE_LIMIT
        || read_budget > *remaining_bytes
    {
        hasher.update(b"bounded-read-refused\0");
        hasher.update(&expected_length.to_le_bytes());
        return false;
    }
    let mut content = blake3::Hasher::new();
    let mut length = 0_u64;
    let mut buffer = [0_u8; 16 * 1024];
    let mut input = input.take(read_budget);
    loop {
        match input.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => {
                length = length.saturating_add(read as u64);
                *remaining_bytes = remaining_bytes.saturating_sub(read as u64);
                content.update(&buffer[..read]);
            }
            Err(error) => {
                let detail = error.to_string();
                hasher.update(&u64::MAX.to_le_bytes());
                hasher.update(detail.as_bytes());
                return false;
            }
        }
    }
    if length != expected_length {
        hasher.update(b"concurrent-length-change\0");
        hasher.update(&expected_length.to_le_bytes());
        hasher.update(&length.to_le_bytes());
        return false;
    }
    hasher.update(&length.to_le_bytes());
    hasher.update(content.finalize().as_bytes());
    true
}

/// What the optional `clang-tidy` lens saw in this file.
///
/// Every finding is informational and stays that way. The lens reads the file
/// on disk, reports a judgement about how the program behaves, and never takes
/// part in the proof that authorizes an edit — normfix edits on facts about the
/// text, and this is not one. A lens that is absent, that cannot run, or that
/// runs out of its bounds simply shows nothing.
fn clang_tidy_diagnostics(
    oracle: &OracleContext,
    file: &DiscoveredFile,
    path: &Utf8PathBuf,
    original: &str,
    current: &str,
) -> Vec<Diagnostic> {
    let Some(lens) = &oracle.clang_tidy else {
        return Vec::new();
    };
    let Ok(findings) = lens.analyze(&file.path, &oracle.compiler_include_directories) else {
        return Vec::new();
    };
    findings
        .into_iter()
        .filter(|finding| compiler_path_matches(&finding.path, path.as_str()))
        .map(|finding| Diagnostic {
            rule_id: format!("TIDY_{}", normalize_warning_name(&finding.check)),
            path: path.clone(),
            range: remap_compiler_range(original, current, finding.line, finding.column),
            severity: Severity::Info,
            message: finding.message,
            source: DiagnosticSource::Compiler,
            notes: vec![
                "clang-tidy reads the file on disk as an optional lens; its findings never authorize or reject an edit."
                    .to_owned(),
            ],
            help: Some(
                "Follow the ownership or control-flow path it describes; confirm the result at runtime before trusting it."
                    .to_owned(),
            ),
            localized: None,
        })
        .collect()
}

pub(super) fn run_compiler_preflight(
    oracle: &OracleContext,
    options: &FixOptions,
    file: &DiscoveredFile,
    path: &Utf8PathBuf,
    original: &str,
    current: &str,
) -> Vec<Diagnostic> {
    let mut lens = clang_tidy_diagnostics(oracle, file, path, original, current);
    if oracle.compiler.is_none() {
        if oracle.compiler_notice_path.as_deref() == Some(file.path.as_path()) {
            if let Some(reason) = &oracle.compiler_unavailable {
                lens.push(point_diagnostic(
                    path,
                    "CC_PREFLIGHT_UNAVAILABLE",
                    if options.preflight {
                        Severity::Error
                    } else {
                        Severity::Info
                    },
                    format!(
                        "The {} C compiler preflight was skipped: {reason}",
                        if options.preflight {
                            "required"
                        } else {
                            "optional"
                        }
                    ),
                    DiagnosticSource::Compiler,
                    Some(
                        "Install `cc` or provide an exact compiler path; formatting and Norminette validation continued safely."
                            .to_owned(),
                    ),
                ));
                return lens;
            }
        }
        return lens;
    }
    let mut diagnostics = std::mem::take(&mut lens);
    if options.compiler_preflight || options.preflight {
        append_compiler_run(
            &mut diagnostics,
            oracle,
            file,
            path,
            original,
            current,
            false,
            options.preflight,
        );
    }
    if options.analyzer || options.preflight {
        append_compiler_run(
            &mut diagnostics,
            oracle,
            file,
            path,
            original,
            current,
            true,
            false,
        );
    }
    diagnostics
}

#[allow(clippy::too_many_arguments)]
pub(super) fn append_compiler_run(
    diagnostics: &mut Vec<Diagnostic>,
    oracle: &OracleContext,
    file: &DiscoveredFile,
    path: &Utf8PathBuf,
    original: &str,
    current: &str,
    analyzer: bool,
    required: bool,
) {
    match oracle.compiler_preflight(&file.path, original, analyzer) {
        Ok(Some(report)) => diagnostics.extend(compiler_report_diagnostics(
            path, original, current, &report, analyzer, required,
        )),
        Ok(None) => {}
        Err(error) => diagnostics.push(point_diagnostic(
            path,
            if analyzer {
                "CC_ANALYZER_FAILED"
            } else {
                "CC_PREFLIGHT_FAILED"
            },
            if required {
                Severity::Error
            } else {
                Severity::Info
            },
            format!(
                "The {} could not inspect this translation unit: {error}",
                if analyzer {
                    "GCC analyzer"
                } else {
                    "C compiler preflight"
                }
            ),
            DiagnosticSource::Compiler,
            Some(if required {
                "Preflight is incomplete until this compiler failure is resolved; no source edit was authorized by it."
                    .to_owned()
            } else {
                "This operational failure is fail-open and did not authorize or reject any source edit."
                    .to_owned()
            }),
        )),
    }
}

#[allow(clippy::too_many_lines)]
pub(super) fn compiler_report_diagnostics(
    path: &Utf8PathBuf,
    original: &str,
    current: &str,
    report: &CompilerReport,
    analyzer: bool,
    required: bool,
) -> Vec<Diagnostic> {
    static LOCATION: OnceLock<Regex> = OnceLock::new();
    let location = LOCATION.get_or_init(|| {
        Regex::new(
            r"^(?P<path>.*):(?P<line>[0-9]+):(?P<column>[0-9]+):[ \t]*(?P<level>fatal error|error|warning|note):[ \t]*(?P<message>.*)$",
        )
        .expect("constant compiler diagnostic regex")
    });
    let combined = report
        .stdout
        .lines()
        .chain(report.stderr.lines())
        .collect::<Vec<_>>();
    if analyzer
        && !report.accepted
        && combined.iter().any(|line| {
            line.contains("-fanalyzer")
                && (line.contains("unrecognized")
                    || line.contains("unknown argument")
                    || line.contains("unsupported"))
        })
    {
        return vec![point_diagnostic(
            path,
            "CC_ANALYZER_UNAVAILABLE",
            if required {
                Severity::Error
            } else {
                Severity::Info
            },
            "This compiler supports neither GCC `-fanalyzer` nor the Clang analyzer; deep analysis was skipped."
                .to_owned(),
            DiagnosticSource::Compiler,
            Some(
                "Point --cc at a real GCC or Clang, or omit --analyzer.".to_owned(),
            ),
        )];
    }
    if !report.accepted {
        if let Some(detail) = combined
            .iter()
            .map(|line| line.trim())
            .find(|line| compiler_configuration_is_incomplete(line))
        {
            return vec![point_diagnostic(
                path,
                if analyzer {
                    "CC_ANALYZER_CONFIGURATION_INCOMPLETE"
                } else {
                    "CC_PREFLIGHT_CONFIGURATION_INCOMPLETE"
                },
                if required {
                    Severity::Error
                } else {
                    Severity::Info
                },
                format!(
                    "The {} could not resolve the project compilation context: {detail}",
                    if analyzer {
                        "GCC analyzer"
                    } else {
                        "C compiler preflight"
                    }
                ),
                DiagnosticSource::Compiler,
                Some(
                    "normfix added stable -I entries for every discovered project header directory, but deliberately did not infer -D macros, SDK paths, language modes, or execute Make recipes; formatting continued without using this incomplete result."
                        .to_owned(),
                ),
            )];
        }
    }
    let mut diagnostics = combined
        .iter()
        .filter_map(|line| location.captures(line.trim()))
        .map(|captures| {
            let line = captures
                .name("line")
                .and_then(|value| value.as_str().parse::<u32>().ok())
                .unwrap_or(1);
            let column = captures
                .name("column")
                .and_then(|value| value.as_str().parse::<u32>().ok())
                .unwrap_or(1);
            let level = captures
                .name("level")
                .map_or("warning", |value| value.as_str());
            let raw_message = captures
                .name("message")
                .map_or("C compiler diagnostic", |value| value.as_str());
            let (message, warning_name) = compiler_warning_name(raw_message);
            let compiler_path = captures
                .name("path")
                .map_or("", |value| value.as_str());
            let local_location = compiler_path_matches(compiler_path, path.as_str());
            let range = if local_location {
                remap_compiler_range(original, current, line, column)
            } else {
                TextRange::empty(TextSize::new(0))
            };
            let mut notes = vec![
                "Compiler diagnostics inspect the original on-disk translation unit and never authorize or reject formatter edits."
                    .to_owned(),
            ];
            if !local_location {
                notes.push(format!(
                    "Compiler location: {compiler_path}:{line}:{column} (usually an included header)."
                ));
            }
            Diagnostic {
                rule_id: if analyzer {
                    warning_name.map_or_else(
                        || "CC_ANALYZER".to_owned(),
                        |name| {
                            let normalized = normalize_warning_name(name);
                            let normalized = normalized
                                .strip_prefix("ANALYZER_")
                                .unwrap_or(&normalized);
                            format!("CC_ANALYZER_{normalized}")
                        },
                    )
                } else {
                    warning_name.map_or_else(
                        || "CC_STRICT".to_owned(),
                        |name| format!("CC_{}", normalize_warning_name(name)),
                    )
                },
                path: path.clone(),
                range,
                severity: if analyzer || level == "note" {
                    Severity::Info
                } else if level.contains("error") {
                    Severity::Error
                } else {
                    Severity::Warning
                },
                message: message.to_owned(),
                source: DiagnosticSource::Compiler,
                notes,
                help: Some(if analyzer {
                    "Review the analyzer path trace; ownership and control-flow findings are never auto-fixed."
                        .to_owned()
                } else {
                    "Fix this strict -Wall/-Wextra/-Werror compiler diagnostic, then rerun normfix."
                        .to_owned()
                }),
                localized: None,
            }
        })
        .collect::<Vec<_>>();
    if analyzer {
        deduplicate_analyzer_trace(&mut diagnostics);
    }
    if diagnostics.is_empty() && !report.accepted {
        let detail = report
            .stderr
            .lines()
            .chain(report.stdout.lines())
            .find(|line| !line.trim().is_empty())
            .unwrap_or("the compiler returned a nonzero status without a parseable diagnostic");
        diagnostics.push(point_diagnostic(
            path,
            if analyzer {
                "CC_ANALYZER_REJECTED"
            } else {
                "CC_STRICT_REJECTED"
            },
            if analyzer {
                Severity::Info
            } else {
                Severity::Warning
            },
            detail.to_owned(),
            DiagnosticSource::Compiler,
            Some(
                "Inspect the compiler output directly; no formatter decision depended on this preflight."
                    .to_owned(),
            ),
        ));
    }
    diagnostics
}

pub(super) fn compiler_configuration_is_incomplete(line: &str) -> bool {
    let lowercase = line.to_ascii_lowercase();
    let missing_input = lowercase.contains("no such file or directory")
        || lowercase.contains("file not found")
        || lowercase.contains("cannot find")
        || lowercase.contains("could not find");
    missing_input
        && (lowercase.contains("fatal error:") || lowercase.contains("cannot open include file"))
}

pub(super) fn compiler_warning_name(message: &str) -> (&str, Option<&str>) {
    let Some(open) = message.rfind(" [-W") else {
        return (message, None);
    };
    let Some(suffix) = message
        .get(open + 2..)
        .and_then(|tail| tail.strip_suffix(']'))
    else {
        return (message, None);
    };
    let parts = suffix.split(',').map(str::trim).collect::<Vec<_>>();
    let warning = parts
        .iter()
        .find_map(|part| part.strip_prefix("-Werror="))
        .or_else(|| {
            parts.iter().find_map(|part| {
                part.strip_prefix("-W")
                    .filter(|name| *name != "error" && !name.starts_with("error="))
            })
        });
    (message[..open].trim_end(), warning)
}

pub(super) fn normalize_warning_name(name: &str) -> String {
    name.trim_start_matches("error=")
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect()
}

pub(super) fn compiler_path_matches(compiler_path: &str, report_path: &str) -> bool {
    compiler_path == report_path || compiler_path.ends_with(&format!("/{report_path}"))
}

pub(super) fn remap_compiler_range(
    original: &str,
    current: &str,
    line: u32,
    column: u32,
) -> TextRange {
    if original == current {
        return diagnostic_range(current, line, column, ColumnUnit::Byte);
    }
    let Some(original_line) = original.lines().nth(line.saturating_sub(1) as usize) else {
        return TextRange::empty(TextSize::new(0));
    };
    let mut matches = current
        .lines()
        .enumerate()
        .filter_map(|(index, candidate)| (candidate == original_line).then_some(index + 1));
    let Some(mapped) = matches.next() else {
        return TextRange::empty(TextSize::new(0));
    };
    if matches.next().is_some() {
        return TextRange::empty(TextSize::new(0));
    }
    diagnostic_range(
        current,
        u32::try_from(mapped).unwrap_or(u32::MAX),
        column,
        ColumnUnit::Byte,
    )
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::{COMPILER_FINGERPRINT_FILE_LIMIT, compiler_project_context};

    #[test]
    fn a_compiler_is_classified_by_its_banner_not_its_command_name() {
        use super::CompilerFamily;

        // /usr/bin/gcc on macOS answers this, and sending it -fanalyzer fails.
        assert_eq!(
            CompilerFamily::from_version("Apple clang version 17.0.0 (clang-1700.6.4.2)"),
            CompilerFamily::Clang
        );
        assert_eq!(
            CompilerFamily::from_version("gcc (Homebrew GCC 14.2.0) 14.2.0"),
            CompilerFamily::Gcc
        );
        assert_eq!(
            CompilerFamily::from_version("cc (Free Software Foundation) 13"),
            CompilerFamily::Gcc
        );
        assert_eq!(
            CompilerFamily::from_version("tcc version 0.9.27"),
            CompilerFamily::Unknown
        );
    }

    #[test]
    fn each_family_gets_the_analyzer_flags_it_understands() {
        use super::{CompilerFamily, compiler_arguments};

        let clang = compiler_arguments(true, CompilerFamily::Clang, &[]);
        assert!(clang.iter().any(|flag| flag == "--analyze"));
        // Combining the two makes Clang ignore the analyzer entirely.
        assert!(!clang.iter().any(|flag| flag == "-fsyntax-only"));

        let gcc = compiler_arguments(true, CompilerFamily::Gcc, &[]);
        assert!(gcc.iter().any(|flag| flag == "-fanalyzer"));
        assert!(gcc.iter().any(|flag| flag == "-fsyntax-only"));

        // The strict preflight is the same for everyone and keeps -Werror.
        for family in [CompilerFamily::Clang, CompilerFamily::Gcc] {
            let strict = compiler_arguments(false, family, &[]);
            assert!(strict.iter().any(|flag| flag == "-Werror"));
            assert!(!strict.iter().any(|flag| flag == "--analyze"));
            assert!(!strict.iter().any(|flag| flag == "-fanalyzer"));
        }
    }

    #[test]
    fn oversized_unexpected_file_disables_the_compiler_cache_fingerprint() {
        let project = TempDir::new().expect("project");
        fs::write(
            project.path().join("main.c"),
            "int\tmain(void)\n{\n\treturn (0);\n}\n",
        )
        .expect("source");
        let unexpected =
            fs::File::create(project.path().join("recording.bin")).expect("unexpected file");
        unexpected
            .set_len(COMPILER_FINGERPRINT_FILE_LIMIT + 1)
            .expect("sparse oversized unexpected file");

        let context = compiler_project_context(project.path());

        assert!(context.fingerprint.is_none());
    }
}
