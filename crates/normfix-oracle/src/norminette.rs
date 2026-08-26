use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tempfile::TempDir;
use thiserror::Error;

use crate::executable::resolve_executable;
use crate::process::{BoundedOutput, ProcessError, ProcessLimits, run_bounded};

/// The memory cache is only a per-run accelerator. Bounding it prevents a
/// large generated tree (or an oracle that emits thousands of diagnostics per
/// file) from turning convenience caching into an unbounded memory sink.
const MEMORY_CACHE_BYTES: usize = 8 * 1024 * 1024;

/// Official Norminette version supported by this compatibility oracle.
pub const SUPPORTED_NORMINETTE_VERSION: &str = "3.3.59";

/// Configuration used to locate and verify Norminette.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NorminetteConfig {
    /// Explicit executable, or `None` to search `PATH`.
    pub executable: Option<PathBuf>,
    /// Exact version required from `norminette --version`.
    pub expected_version: String,
    /// Refuse a different release instead of continuing with an advisory.
    ///
    /// The before/after regression proof compares two answers from the same
    /// executable, so it stays valid whatever version that is. What an untested
    /// release costs is the guarantee that the native rules agree with it, so
    /// strict mode is useful for reproducible CI that pins the official checker.
    pub strict_version: bool,
    /// Limits applied independently to version checks and lint calls.
    pub limits: ProcessLimits,
}

impl Default for NorminetteConfig {
    fn default() -> Self {
        Self {
            executable: None,
            expected_version: SUPPORTED_NORMINETTE_VERSION.to_owned(),
            strict_version: false,
            limits: ProcessLimits::per_file_default(),
        }
    }
}

/// Stable fingerprint of the verified external oracle.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NorminetteFingerprint {
    /// Whether this release is the one the project verifies against.
    pub untested: bool,
    /// Parsed Norminette version.
    pub version: String,
    /// Normalized complete `--version` output.
    pub version_output: String,
    /// BLAKE3 fingerprint of the normalized version response.
    pub digest: [u8; 32],
}

/// One official Norminette diagnostic.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct NorminetteDiagnostic {
    /// Whether the checker called this an error or only a notice.
    ///
    /// A notice is the checker remarking on something it accepts —
    /// `GLOBAL_VAR_DETECTED` asks a reader to confirm a global was deliberate,
    /// and the file still passes. Counting one as an error would make a clean
    /// file look rejected; refusing to read one made the file unprocessable,
    /// which is what this used to do.
    #[serde(default)]
    pub advisory: bool,
    /// One-based physical line.
    pub line: u32,
    /// One-based display column reported by Norminette.
    pub column: u32,
    /// Stable uppercase Norminette rule identifier.
    pub rule_id: String,
    /// English diagnostic text.
    pub message: String,
}

/// Stable result for one in-memory source.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NorminetteReport {
    /// Original basename retained in the temporary file.
    pub file_name: String,
    /// Canonically sorted diagnostics.
    pub diagnostics: Vec<NorminetteDiagnostic>,
}

impl NorminetteReport {
    /// Returns whether the official oracle accepted the source.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        // A notice is the checker remarking on something it accepted. The file
        // passed, so treating one as a rejection would report a clean file as
        // failing.
        self.diagnostics
            .iter()
            .all(|diagnostic| diagnostic.advisory)
    }
}

/// Operational failure distinct from a source diagnostic.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum NorminetteError {
    /// The command could not be located or was not executable.
    #[error("Norminette is unavailable: {0}")]
    Unavailable(String),
    /// The command's version response was not recognized.
    #[error("could not parse Norminette version output: {0}")]
    InvalidVersionOutput(String),
    /// A different Norminette release was found.
    #[error("Norminette version mismatch: expected {expected}, found {found}")]
    VersionMismatch {
        /// Required release.
        expected: String,
        /// Observed release.
        found: String,
    },
    /// The supplied basename was not a C source or header.
    #[error("invalid in-memory source name: {0}")]
    InvalidFileName(String),
    /// The temporary source could not be materialized.
    #[error("could not prepare the in-memory source for Norminette: {0}")]
    TemporarySource(String),
    /// The bounded child-process runner failed.
    #[error(transparent)]
    Process(#[from] ProcessError),
    /// Norminette terminated without a valid lint result.
    #[error("Norminette failed operationally (exit {exit_code:?}): {detail}")]
    ToolFailure {
        /// Numeric process status when available.
        exit_code: Option<i32>,
        /// Bounded English detail.
        detail: String,
    },
    /// Output did not match the official 3.3.59 grammar.
    #[error("could not parse Norminette output: {0}")]
    MalformedOutput(String),
}

/// Verified, bounded compatibility oracle for Norminette 3.3.59.
#[derive(Debug)]
pub struct NorminetteOracle {
    executable: PathBuf,
    fingerprint: NorminetteFingerprint,
    limits: ProcessLimits,
    cache: Mutex<ReportCache>,
}

#[derive(Debug, Default)]
struct ReportCache {
    entries: BTreeMap<[u8; 32], NorminetteReport>,
    bytes: usize,
}

impl ReportCache {
    fn get(&self, key: &[u8; 32]) -> Option<NorminetteReport> {
        self.entries.get(key).cloned()
    }

    fn insert(&mut self, key: [u8; 32], report: NorminetteReport) {
        let report_bytes = report_memory_bytes(&report);
        if report_bytes > MEMORY_CACHE_BYTES {
            return;
        }
        if let Some(replaced) = self.entries.remove(&key) {
            self.bytes = self.bytes.saturating_sub(report_memory_bytes(&replaced));
        }
        while self.bytes.saturating_add(report_bytes) > MEMORY_CACHE_BYTES {
            let Some((_, evicted)) = self.entries.pop_first() else {
                break;
            };
            self.bytes = self.bytes.saturating_sub(report_memory_bytes(&evicted));
        }
        self.bytes = self.bytes.saturating_add(report_bytes);
        self.entries.insert(key, report);
    }

    fn clear(&mut self) {
        self.entries.clear();
        self.bytes = 0;
    }
}

impl NorminetteOracle {
    /// Locates the command, executes `--version` without a shell, and
    /// fingerprints the release. Strict mode additionally requires the tested
    /// release.
    ///
    /// # Errors
    ///
    /// Returns [`NorminetteError`] when the command cannot be safely verified.
    pub fn locate(config: NorminetteConfig) -> Result<Self, NorminetteError> {
        config.limits.validate()?;
        let executable = resolve_executable(config.executable.as_deref(), "norminette")
            .map_err(NorminetteError::Unavailable)?;
        let mut command = Command::new(&executable);
        command.arg("--version");
        configure_english_environment(&mut command);
        let output = run_bounded(&mut command, config.limits)?;
        if !output.success() {
            return Err(tool_failure(&output));
        }
        let version_output = normalized_output(&output);
        let version = parse_version(&version_output)?;
        let untested = version != config.expected_version;
        if untested && config.strict_version {
            return Err(NorminetteError::VersionMismatch {
                expected: config.expected_version,
                found: version,
            });
        }
        let digest = *blake3::hash(version_output.as_bytes()).as_bytes();
        Ok(Self {
            executable,
            fingerprint: NorminetteFingerprint {
                untested,
                version,
                version_output,
                digest,
            },
            limits: config.limits,
            cache: Mutex::new(ReportCache::default()),
        })
    }

    /// Returns the resolved executable path.
    #[must_use]
    pub fn executable(&self) -> &Path {
        &self.executable
    }

    /// Returns the verified tool fingerprint.
    #[must_use]
    pub const fn fingerprint(&self) -> &NorminetteFingerprint {
        &self.fingerprint
    }

    /// Returns the number of successful reports cached in memory.
    #[must_use]
    pub fn memory_cache_len(&self) -> usize {
        self.cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .entries
            .len()
    }

    /// Removes every in-memory result.
    pub fn clear_memory_cache(&self) {
        self.cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clear();
    }

    /// Lints arbitrary UTF-8 C/header text through an isolated temporary file.
    ///
    /// The temporary file retains the supplied basename. Only successful,
    /// parseable tool responses enter the deterministic in-memory cache.
    ///
    /// # Errors
    ///
    /// Returns [`NorminetteError`] for command, timeout, output-limit, I/O or
    /// protocol failures. Norm violations are successful structured results.
    pub fn lint(
        &self,
        requested_name: &Path,
        source: &str,
    ) -> Result<NorminetteReport, NorminetteError> {
        let file_name = validated_basename(requested_name)?;
        let cache_key = lint_cache_key(&self.fingerprint, &file_name, source);
        if let Some(report) = self
            .cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&cache_key)
        {
            return Ok(report);
        }

        let temporary =
            TempDir::new().map_err(|error| NorminetteError::TemporarySource(error.to_string()))?;
        std::fs::write(temporary.path().join(&file_name), source.as_bytes())
            .map_err(|error| NorminetteError::TemporarySource(error.to_string()))?;
        let mut command = Command::new(&self.executable);
        command.current_dir(temporary.path()).arg(&file_name);
        configure_english_environment(&mut command);
        let output = run_bounded(&mut command, self.limits)?;
        let report = parse_lint_output(&file_name, &output)?;
        self.cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(cache_key, report.clone());
        Ok(report)
    }
}

fn report_memory_bytes(report: &NorminetteReport) -> usize {
    // Include allocated capacities and a conservative map-node/key allowance,
    // not only visible string lengths. Otherwise millions of tiny clean
    // reports could stay below the nominal byte budget while their BTree
    // nodes consumed far more memory.
    std::mem::size_of::<NorminetteReport>()
        .saturating_add(std::mem::size_of::<[u8; 32]>())
        .saturating_add(64)
        .saturating_add(report.file_name.capacity())
        .saturating_add(
            report
                .diagnostics
                .capacity()
                .saturating_mul(std::mem::size_of::<NorminetteDiagnostic>()),
        )
        .saturating_add(
            report
                .diagnostics
                .iter()
                .map(|diagnostic| {
                    diagnostic
                        .rule_id
                        .capacity()
                        .saturating_add(diagnostic.message.capacity())
                })
                .fold(0usize, usize::saturating_add),
        )
}

fn validated_basename(requested: &Path) -> Result<String, NorminetteError> {
    let file_name = requested
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .ok_or_else(|| {
            NorminetteError::InvalidFileName(
                "the path must have a non-empty UTF-8 basename".to_owned(),
            )
        })?;
    if file_name.starts_with('-') || file_name.chars().any(char::is_control) {
        return Err(NorminetteError::InvalidFileName(
            "the basename cannot start with '-' or contain control characters".to_owned(),
        ));
    }
    let extension = Path::new(file_name)
        .extension()
        .and_then(|extension| extension.to_str());
    if !matches!(extension, Some("c" | "h")) {
        return Err(NorminetteError::InvalidFileName(format!(
            "`{file_name}` must end in .c or .h"
        )));
    }
    Ok(file_name.to_owned())
}

fn configure_english_environment(command: &mut Command) {
    command
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .env("LANGUAGE", "en")
        .env("PYTHONHASHSEED", "0")
        .env("PYTHONIOENCODING", "utf-8")
        .env("NO_COLOR", "1");
}

fn normalized_output(output: &BoundedOutput) -> String {
    let stdout = strip_terminal_sequences(&output.stdout);
    let stderr = strip_terminal_sequences(&output.stderr);
    stdout
        .lines()
        .chain(stderr.lines())
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("Setting locale to "))
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn strip_terminal_sequences(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == 0x1b {
            index += 1;
            match bytes.get(index).copied() {
                Some(b'[') => {
                    index += 1;
                    while index < bytes.len() {
                        let byte = bytes[index];
                        index += 1;
                        if (0x40..=0x7e).contains(&byte) {
                            break;
                        }
                    }
                }
                Some(b']') => {
                    index += 1;
                    while index < bytes.len() {
                        if bytes[index] == 0x07 {
                            index += 1;
                            break;
                        }
                        if bytes[index] == 0x1b && bytes.get(index + 1) == Some(&b'\\') {
                            index += 2;
                            break;
                        }
                        index += 1;
                    }
                }
                // An ESC followed by an unknown byte is not a terminal
                // sequence we can safely interpret. Drop only ESC. Dropping
                // the following byte used to split a multi-byte UTF-8 scalar
                // and panic in the conversion below (for example `ESC` +
                // `é`), allowing tool output to crash normfix.
                Some(_) | None => {}
            }
            continue;
        }
        let byte = bytes[index];
        if byte.is_ascii_control() && !matches!(byte, b'\n' | b'\t') {
            index += 1;
            continue;
        }
        output.push(byte);
        index += 1;
    }
    String::from_utf8_lossy(&output)
        .chars()
        .filter(|character| {
            !character.is_control()
                && !matches!(
                    character,
                    // Directional formatting controls can reorder a file,
                    // rule, or diagnostic on screen without being visible.
                    '\u{061c}'
                        | '\u{200e}'
                        | '\u{200f}'
                        | '\u{202a}'..='\u{202e}'
                        | '\u{2066}'..='\u{2069}'
                )
                || matches!(character, '\n' | '\t')
        })
        .collect()
}

fn parse_version(output: &str) -> Result<String, NorminetteError> {
    output
        .lines()
        .find_map(|line| {
            line.trim()
                .strip_prefix("norminette ")
                .and_then(|rest| rest.split([',', ' ']).next())
                .filter(|version| {
                    !version.is_empty()
                        && version
                            .split('.')
                            .all(|component| component.parse::<u32>().is_ok())
                })
                .map(str::to_owned)
        })
        .ok_or_else(|| NorminetteError::InvalidVersionOutput(output.to_owned()))
}

fn lint_cache_key(fingerprint: &NorminetteFingerprint, file_name: &str, source: &str) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"normfix-norminette-memory-cache-v1\0");
    hash_field(&mut hasher, &fingerprint.digest);
    hash_field(&mut hasher, file_name.as_bytes());
    hash_field(&mut hasher, source.as_bytes());
    *hasher.finalize().as_bytes()
}

fn hash_field(hasher: &mut blake3::Hasher, bytes: &[u8]) {
    hasher.update(&u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_le_bytes());
    hasher.update(bytes);
}

fn parse_lint_output(
    file_name: &str,
    output: &BoundedOutput,
) -> Result<NorminetteReport, NorminetteError> {
    let normalized = normalized_output(output);
    let mut saw_ok = false;
    let mut saw_error = false;
    let mut diagnostics = Vec::new();
    let mut unrecognized = Vec::new();

    for line in normalized.lines() {
        if line == format!("{file_name}: OK!") {
            saw_ok = true;
        } else if line == format!("{file_name}: Error!") {
            saw_error = true;
        } else if let Some(diagnostic) = parse_diagnostic(line) {
            diagnostics.push(diagnostic);
        } else {
            unrecognized.push(line.to_owned());
        }
    }
    if !unrecognized.is_empty() {
        return Err(NorminetteError::MalformedOutput(format!(
            "unrecognized line(s): {}",
            unrecognized.join(" | ")
        )));
    }
    diagnostics.sort();
    diagnostics.dedup();
    if saw_ok && !saw_error && diagnostics.is_empty() && output.exit_code == Some(0) {
        return Ok(NorminetteReport {
            file_name: file_name.to_owned(),
            diagnostics,
        });
    }
    // A file whose only remark is a notice prints `OK!` and still exits 1. The
    // checker is saying it passed and that something is worth a second look,
    // which is neither a clean exit nor a rejection, and reading it as
    // inconsistent made every file carrying a global unprocessable.
    if saw_ok
        && !saw_error
        && diagnostics.iter().all(|diagnostic| diagnostic.advisory)
        && matches!(output.exit_code, Some(0 | 1))
    {
        return Ok(NorminetteReport {
            file_name: file_name.to_owned(),
            diagnostics,
        });
    }
    if saw_error && !saw_ok && !diagnostics.is_empty() && matches!(output.exit_code, Some(1)) {
        return Ok(NorminetteReport {
            file_name: file_name.to_owned(),
            diagnostics,
        });
    }
    if !matches!(output.exit_code, Some(0 | 1)) {
        return Err(tool_failure(output));
    }
    Err(NorminetteError::MalformedOutput(format!(
        "inconsistent status/output (exit {:?}): {}",
        output.exit_code, normalized
    )))
}

fn parse_diagnostic(line: &str) -> Option<NorminetteDiagnostic> {
    let (advisory, rest) = line.strip_prefix("Error: ").map_or_else(
        || line.strip_prefix("Notice: ").map(|rest| (true, rest)),
        |rest| Some((false, rest)),
    )?;
    let marker = rest.find("(line:")?;
    let rule_id = rest.get(..marker)?.trim();
    if rule_id.is_empty()
        || !rule_id
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
    {
        return None;
    }
    let location = rest.get(marker + "(line:".len()..)?;
    let (line_text, location) = location.split_once(", col:")?;
    let (column_text, message) = location.split_once("):")?;
    let line = line_text.trim().parse().ok()?;
    let column = column_text.trim().parse().ok()?;
    if line == 0 || column == 0 {
        return None;
    }
    let message = strip_terminal_sequences(message).trim().to_owned();
    if message.is_empty() {
        return None;
    }
    Some(NorminetteDiagnostic {
        advisory,
        line,
        column,
        rule_id: rule_id.to_owned(),
        message,
    })
}

fn tool_failure(output: &BoundedOutput) -> NorminetteError {
    let detail = normalized_output(output);
    NorminetteError::ToolFailure {
        exit_code: output.exit_code,
        detail: if detail.is_empty() {
            "the command produced no diagnostic output".to_owned()
        } else {
            detail
        },
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn a_notice_is_read_as_a_remark_rather_than_refused() {
        // `GLOBAL_VAR_DETECTED` is the checker accepting a file and asking a
        // reader to confirm a global was deliberate. Refusing to parse the line
        // made every file carrying one unprocessable, and counting it as an
        // error would report a passing file as rejected.
        let output = crate::process::BoundedOutput {
            stdout: "n.c: OK!\nNotice: GLOBAL_VAR_DETECTED  (line:   1, col:   1):\tGlobal variable present in file.\n".to_owned(),
            stderr: String::new(),
            exit_code: Some(1),
        };
        let report = super::parse_lint_output("n.c", &output).expect("a notice must be readable");

        assert_eq!(report.diagnostics.len(), 1);
        assert!(report.diagnostics[0].advisory);
        assert_eq!(report.diagnostics[0].rule_id, "GLOBAL_VAR_DETECTED");
        assert!(
            report.is_clean(),
            "a file the checker passed is not rejected"
        );
    }

    #[test]
    fn an_error_beside_a_notice_still_rejects() {
        let output = crate::process::BoundedOutput {
            stdout: "n.c: Error!\nNotice: GLOBAL_VAR_DETECTED  (line:   1, col:   1):\tGlobal.\nError: INVALID_HEADER       (line:   1, col:   1):\tMissing or invalid 42 header\n".to_owned(),
            stderr: String::new(),
            exit_code: Some(1),
        };
        let report = super::parse_lint_output("n.c", &output).expect("a readable report");

        assert_eq!(report.diagnostics.len(), 2);
        assert!(!report.is_clean(), "an error is still an error");
    }

    #[test]
    fn an_oversized_report_cannot_make_the_memory_cache_unbounded() {
        let mut cache = super::ReportCache::default();
        let report = super::NorminetteReport {
            file_name: "large.c".to_owned(),
            diagnostics: vec![super::NorminetteDiagnostic {
                advisory: false,
                line: 1,
                column: 1,
                rule_id: "TEST".to_owned(),
                message: "x".repeat(super::MEMORY_CACHE_BYTES),
            }],
        };

        cache.insert([1; 32], report);

        assert!(cache.entries.is_empty());
        assert_eq!(cache.bytes, 0);
    }

    use std::path::{Path, PathBuf};
    use std::time::Duration;

    use tempfile::TempDir;

    use super::{
        NorminetteConfig, NorminetteError, NorminetteOracle, ProcessError, ProcessLimits,
        SUPPORTED_NORMINETTE_VERSION,
    };

    /// The probe argument, answered by a guard ahead of every fake tool's body.
    const READINESS_PROBE: &str = "--normfix-readiness-probe";

    /// Writes a fake tool and does not return until the kernel will run it.
    ///
    /// Linux refuses to `execve` a file while any process holds it open for
    /// writing, and `cargo test` runs these on parallel threads that fork for
    /// their own subprocesses — so a sibling's child, between its `fork` and
    /// its `exec`, briefly counts as a writer for the script this thread just
    /// wrote. Waiting here gives every test a runnable tool instead of an
    /// occasional `Text file busy` that says nothing about normfix.
    ///
    /// This narrows the window rather than closing it: a sibling can fork again
    /// after the probe returns. The retry at the point of use below is the
    /// actual guarantee; the probe is what protects the call sites that do not
    /// have one, and it turns the common case into a wait instead of a failure.
    ///
    /// The probe exits before the body runs, so it cannot disturb a script
    /// that records what it was asked to do.
    #[cfg(unix)]
    fn executable_script(directory: &TempDir, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;

        let path = directory.path().join("norminette");
        std::fs::write(
            &path,
            format!(
                "#!/bin/sh\nset -eu\nif [ \"${{1:-}}\" = \"{READINESS_PROBE}\" ]; then exit 0; fi\n{body}\n"
            ),
        )
        .expect("write fake tool");
        let mut permissions = std::fs::metadata(&path)
            .expect("script metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&path, permissions).expect("make script executable");
        for _ in 0..100 {
            match std::process::Command::new(&path)
                .arg(READINESS_PROBE)
                .output()
            {
                Ok(_) => return path,
                Err(error) if error.to_string().contains("Text file busy") => {
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(error) => panic!("fake tool is not runnable: {error}"),
            }
        }
        panic!("fake tool was still reported as busy after retrying");
    }

    /// Retries while the kernel still reports a freshly written script as busy.
    ///
    /// Tests run in threads, and a child forked by another test inherits the
    /// write descriptor of a script this one just created until that child
    /// reaches its own `exec`. During that window `execve` fails with ETXTBSY.
    /// That is a harness race, not product behavior.
    #[cfg(unix)]
    fn retry_while_text_file_busy<T, E: std::fmt::Debug + std::fmt::Display>(
        what: &str,
        mut attempt: impl FnMut() -> Result<T, E>,
    ) -> T {
        for _ in 0..100 {
            match attempt() {
                Ok(value) => return value,
                Err(error) if error.to_string().contains("Text file busy") => {
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(error) => panic!("{what}: {error:?}"),
            }
        }
        panic!("{what}: still reported as busy after retrying");
    }

    #[cfg(unix)]
    fn oracle(script: &Path, timeout: Duration, output_bytes: usize) -> NorminetteOracle {
        let mut oracle = retry_while_text_file_busy("verified fake oracle", || {
            NorminetteOracle::locate(NorminetteConfig {
                executable: Some(script.to_path_buf()),
                expected_version: SUPPORTED_NORMINETTE_VERSION.to_owned(),
                strict_version: false,
                limits: ProcessLimits {
                    // Version verification is setup, not the per-lint timeout
                    // exercised by these tests. Keep it robust under parallel CI.
                    timeout: timeout.max(Duration::from_secs(5)),
                    output_bytes,
                },
            })
        });
        oracle.limits = ProcessLimits {
            timeout,
            output_bytes,
        };
        oracle
    }

    #[cfg(unix)]
    #[test]
    fn verifies_version_parses_diagnostics_and_retains_only_the_basename() {
        let directory = TempDir::new().expect("temporary script directory");
        let script = executable_script(
            &directory,
            r#"
if [ "$1" = "--version" ]; then
    echo "Setting locale to en_US"
    echo "norminette 3.3.59, Python test"
    exit 0
fi
case "$1" in
    */*) echo "received a path instead of a basename" >&2; exit 9 ;;
esac
test "$(cat "$1")" = "int main(void) { return (0); }"
echo "$1: Error!"
echo "Error: MISALIGNED_FUNC_DECL (line:  7, col:   5):	Misaligned function declaration"
echo "Error: TOO_MANY_TAB          (line:  2, col:   1):	Extra tabs for indent level"
exit 1
"#,
        );
        let oracle = oracle(&script, Duration::from_secs(5), 16 * 1024);

        let report = oracle
            .lint(
                Path::new("nested/example.c"),
                "int main(void) { return (0); }\n",
            )
            .expect("lint diagnostics");

        assert_eq!(report.file_name, "example.c");
        assert_eq!(report.diagnostics.len(), 2);
        assert_eq!(report.diagnostics[0].line, 2);
        assert_eq!(report.diagnostics[1].rule_id, "MISALIGNED_FUNC_DECL");
        assert_eq!(oracle.fingerprint().version, "3.3.59");
    }

    #[cfg(unix)]
    #[test]
    fn successful_results_are_cached_but_failures_are_not() {
        let directory = TempDir::new().expect("temporary script directory");
        let counter = directory.path().join("calls");
        let script = executable_script(
            &directory,
            &format!(
                r#"
if [ "$1" = "--version" ]; then echo "norminette 3.3.59"; exit 0; fi
echo x >> '{}'
echo "$1: OK!"
"#,
                counter.display()
            ),
        );
        let oracle = oracle(&script, Duration::from_secs(5), 16 * 1024);

        let first = oracle
            .lint(Path::new("same.c"), "int same(void);\n")
            .expect("first lint");
        let second = oracle
            .lint(Path::new("same.c"), "int same(void);\n")
            .expect("cached lint");

        assert_eq!(first, second);
        assert_eq!(oracle.memory_cache_len(), 1);
        assert_eq!(
            std::fs::read_to_string(counter)
                .expect("counter")
                .lines()
                .count(),
            1
        );
    }

    #[cfg(unix)]
    #[test]
    fn malformed_results_never_enter_the_memory_cache() {
        let directory = TempDir::new().expect("temporary script directory");
        let counter = directory.path().join("calls");
        let script = executable_script(
            &directory,
            &format!(
                r#"
if [ "$1" = "--version" ]; then echo "norminette 3.3.59"; exit 0; fi
echo x >> '{}'
echo "malformed"
"#,
                counter.display()
            ),
        );
        let oracle = oracle(&script, Duration::from_secs(5), 16 * 1024);

        for _ in 0..2 {
            assert!(
                oracle
                    .lint(Path::new("broken.c"), "int broken(void);\n")
                    .is_err()
            );
        }

        assert_eq!(oracle.memory_cache_len(), 0);
        assert_eq!(
            std::fs::read_to_string(counter)
                .expect("counter")
                .lines()
                .count(),
            2
        );
    }

    #[cfg(unix)]
    #[test]
    fn untested_version_continues_and_is_fingerprinted_by_default() {
        let directory = TempDir::new().expect("temporary script directory");
        let script = executable_script(
            &directory,
            r#"
if [ "$1" = "--version" ]; then echo "norminette 4.0.0"; exit 0; fi
"#,
        );

        let oracle = NorminetteOracle::locate(NorminetteConfig {
            executable: Some(script),
            ..NorminetteConfig::default()
        })
        .expect("untested versions are advisory by default");

        assert!(oracle.fingerprint().untested);
        assert_eq!(oracle.fingerprint().version, "4.0.0");
    }

    #[cfg(unix)]
    #[test]
    fn strict_version_mode_rejects_an_untested_release() {
        let directory = TempDir::new().expect("temporary script directory");
        let script = executable_script(
            &directory,
            r#"
if [ "$1" = "--version" ]; then echo "norminette 4.0.0"; exit 0; fi
"#,
        );

        let error = NorminetteOracle::locate(NorminetteConfig {
            executable: Some(script),
            strict_version: true,
            ..NorminetteConfig::default()
        })
        .expect_err("strict version policy must fail");

        assert!(matches!(error, NorminetteError::VersionMismatch { .. }));
    }

    #[cfg(unix)]
    #[test]
    fn timeout_kills_and_reaps_the_tool() {
        let directory = TempDir::new().expect("temporary script directory");
        let script = executable_script(
            &directory,
            r#"
if [ "$1" = "--version" ]; then echo "norminette 3.3.59"; exit 0; fi
sleep 5
"#,
        );
        let oracle = oracle(&script, Duration::from_millis(40), 16 * 1024);

        let error = oracle
            .lint(Path::new("slow.c"), "int slow(void);\n")
            .expect_err("must time out");

        assert!(matches!(
            error,
            NorminetteError::Process(ProcessError::Timeout { .. })
        ));
        assert_eq!(oracle.memory_cache_len(), 0);
    }

    #[cfg(unix)]
    #[test]
    fn excessive_output_is_bounded_and_not_cached() {
        let directory = TempDir::new().expect("temporary script directory");
        let script = executable_script(
            &directory,
            r#"
if [ "$1" = "--version" ]; then echo "norminette 3.3.59"; exit 0; fi
i=0
while [ "$i" -lt 1000 ]; do
    echo "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"
    i=$((i + 1))
done
"#,
        );
        let oracle = oracle(&script, Duration::from_secs(2), 512);

        let error = oracle
            .lint(Path::new("loud.c"), "int loud(void);\n")
            .expect_err("must enforce cap");

        assert!(matches!(
            error,
            NorminetteError::Process(ProcessError::OutputLimit { limit: 512 })
        ));
        assert_eq!(oracle.memory_cache_len(), 0);
    }

    #[cfg(unix)]
    #[test]
    fn malformed_output_is_distinct_from_norm_diagnostics() {
        let directory = TempDir::new().expect("temporary script directory");
        let script = executable_script(
            &directory,
            r#"
if [ "$1" = "--version" ]; then echo "norminette 3.3.59"; exit 0; fi
echo "unexpected protocol text"
"#,
        );
        let oracle = oracle(&script, Duration::from_secs(5), 16 * 1024);

        let error = oracle
            .lint(Path::new("bad.c"), "int bad(void);\n")
            .expect_err("malformed protocol");

        assert!(matches!(error, NorminetteError::MalformedOutput(_)));
    }

    #[test]
    fn rejects_non_c_file_names_before_spawning() {
        let error = super::validated_basename(Path::new("README.md")).expect_err("must reject");
        assert!(matches!(error, NorminetteError::InvalidFileName(_)));
        let error = super::validated_basename(Path::new("-option.c")).expect_err("must reject");
        assert!(matches!(error, NorminetteError::InvalidFileName(_)));
    }

    #[test]
    fn terminal_escape_sequences_never_reach_diagnostic_messages() {
        let diagnostic = super::parse_diagnostic(
            "Error: MISSING_TAB_VAR (line: 3, col: 2):\t\x1b[97mMissing tab before variable name\x1b[0m",
        )
        .expect("diagnostic");

        assert_eq!(diagnostic.message, "Missing tab before variable name");
        assert!(!diagnostic.message.chars().any(char::is_control));
    }

    #[test]
    fn an_unknown_escape_before_unicode_cannot_panic_or_corrupt_utf8() {
        assert_eq!(
            super::strip_terminal_sequences("before \x1bé after"),
            "before é after"
        );
    }

    #[test]
    fn carriage_returns_and_bidi_overrides_cannot_spoof_a_diagnostic() {
        assert_eq!(
            super::strip_terminal_sequences("safe\rspoof\u{202e}txt"),
            "safespooftxt"
        );
    }

    #[test]
    fn zero_locations_and_empty_messages_are_not_valid_diagnostics() {
        for malformed in [
            "Error: TEST (line: 0, col: 1): bad line",
            "Error: TEST (line: 1, col: 0): bad column",
            "Error: TEST (line: 1, col: 1): \t",
        ] {
            assert!(super::parse_diagnostic(malformed).is_none(), "{malformed}");
        }
    }

    #[test]
    fn unavailable_explicit_command_is_operational() {
        let error = NorminetteOracle::locate(NorminetteConfig {
            executable: Some(PathBuf::from("/definitely/missing/norminette")),
            ..NorminetteConfig::default()
        })
        .expect_err("missing command");

        assert!(matches!(error, NorminetteError::Unavailable(_)));
    }

    #[test]
    #[ignore = "requires the official Norminette 3.3.59 command"]
    fn installed_official_norminette_smoke_test() {
        let oracle =
            NorminetteOracle::locate(NorminetteConfig::default()).expect("official Norminette");
        let report = oracle
            .lint(Path::new("smoke.c"), "int main(void) { return (0); }\n")
            .expect("official lint result");

        assert_eq!(oracle.fingerprint().version, "3.3.59");
        assert_eq!(report.file_name, "smoke.c");
    }
}
