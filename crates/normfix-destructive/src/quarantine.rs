//! Read-only planning for recoverable quarantine of unexpected files.

use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::Read as _;
use std::sync::Arc;

use camino::{Utf8Path, Utf8PathBuf};
use normfix_core::{Diagnostic, DiagnosticSource, Severity, TextRange, TextSize};
use thiserror::Error;

use crate::{AuthorizationError, DestructiveAuthorization, DestructiveCapability};

const RULE_REJECTED: &str = "UNSAFE_QUARANTINE_REJECTED";
const MAX_QUARANTINE_ITEMS: usize = 10_000;
const MAX_QUARANTINE_FILE_BYTES: u64 = 32 * 1024 * 1024;
const MAX_QUARANTINE_TOTAL_BYTES: u64 = 256 * 1024 * 1024;

/// Inputs for a deterministic quarantine plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuarantineRequest {
    /// Existing project directory containing every requested relative path.
    pub project_root: Utf8PathBuf,
    /// Existing external directory used for recoverable storage.
    pub recovery_root: Utf8PathBuf,
    /// Safe single path segment separating this run from previous runs.
    pub run_id: String,
    /// Project-relative unexpected paths proposed for quarantine.
    pub paths: Vec<Utf8PathBuf>,
}

/// Immutable bytes and identity captured for one quarantine item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuarantineSnapshot {
    /// Exact file bytes captured without interpretation.
    pub bytes: Arc<[u8]>,
    /// Lowercase BLAKE3 hash of `bytes`.
    pub blake3_hash: String,
    /// Byte length captured from the immutable buffer.
    pub byte_length: u64,
    /// Read-only permission bit as observed during planning.
    pub readonly: bool,
}

/// One recoverable move proposed by the planner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuarantineItem {
    /// Project-relative source path.
    pub relative_path: Utf8PathBuf,
    /// Absolute source path revalidated during execution.
    pub source_path: Utf8PathBuf,
    /// Absolute external destination that must not already exist.
    pub destination_path: Utf8PathBuf,
    /// Original restoration path.
    pub restore_path: Utf8PathBuf,
    /// Content and identity evidence required before execution.
    pub snapshot: QuarantineSnapshot,
}

/// A deterministic, read-only quarantine plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuarantinePlan {
    /// Canonical project root used during planning.
    pub project_root: Utf8PathBuf,
    /// Canonical external recovery root used during planning.
    pub recovery_root: Utf8PathBuf,
    /// Accepted items in relative-path order.
    pub items: Vec<QuarantineItem>,
    /// Per-path rejections; rejected paths never appear in `items`.
    pub diagnostics: Vec<Diagnostic>,
}

/// A quarantine plan could not establish safe global boundaries.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum QuarantinePlanError {
    /// The supplied explicit grant did not authorize quarantine.
    #[error(transparent)]
    Authorization(#[from] AuthorizationError),
    /// A root did not exist, was not a directory, or could not be canonicalized.
    #[error("invalid {kind} root `{path}`: {detail}")]
    InvalidRoot {
        /// Human-readable root role.
        kind: &'static str,
        /// Rejected root path.
        path: Utf8PathBuf,
        /// Filesystem detail.
        detail: String,
    },
    /// Recovery storage is inside the project or is one of its ancestors.
    #[error("recovery root must not overlap the project: {0}")]
    RecoveryOverlapsProject(Utf8PathBuf),
    /// The run identifier was not a safe single component.
    #[error("quarantine run identifier must contain only ASCII letters, digits, `.`, `_` or `-`")]
    InvalidRunId,
    /// The request is too large to snapshot in one recoverable transaction.
    #[error("quarantine request contains more than {limit} paths")]
    TooManyPaths {
        /// Maximum number of paths in one request.
        limit: usize,
    },
}

/// Creates a quarantine plan without changing any file.
///
/// Each accepted source is a regular file with no symlink in its project-local
/// path. Its bytes and BLAKE3 hash are captured in memory. The recovery root
/// must already exist, resolve outside the project, and contain no pre-existing
/// destination for the run. Executors must compare the captured hash and
/// recheck file type immediately before a transactional move.
///
/// # Errors
///
/// Returns a global boundary or authorization error. Individual invalid paths
/// are represented as diagnostics and omitted from the returned items.
pub fn plan_quarantine(
    request: &QuarantineRequest,
    authorization: &DestructiveAuthorization,
) -> Result<QuarantinePlan, QuarantinePlanError> {
    authorization.require(DestructiveCapability::QuarantineUnexpectedFiles)?;
    if !valid_run_id(&request.run_id) {
        return Err(QuarantinePlanError::InvalidRunId);
    }
    if request.paths.len() > MAX_QUARANTINE_ITEMS {
        return Err(QuarantinePlanError::TooManyPaths {
            limit: MAX_QUARANTINE_ITEMS,
        });
    }
    let project_root = canonical_directory("project", &request.project_root)?;
    let recovery_root = canonical_directory("recovery", &request.recovery_root)?;
    if recovery_root.starts_with(&project_root) || project_root.starts_with(&recovery_root) {
        return Err(QuarantinePlanError::RecoveryOverlapsProject(recovery_root));
    }

    let mut paths = request.paths.clone();
    paths.sort();
    paths.dedup();
    let mut items = Vec::new();
    let mut diagnostics = Vec::new();
    let mut seen_destinations = BTreeSet::new();
    let mut captured_bytes = 0_u64;
    for relative_path in paths {
        match capture_item(
            &project_root,
            &recovery_root,
            &request.run_id,
            &relative_path,
        ) {
            Ok(item) if seen_destinations.insert(item.destination_path.clone()) => {
                let next_total = captured_bytes.saturating_add(item.snapshot.byte_length);
                if next_total > MAX_QUARANTINE_TOTAL_BYTES {
                    diagnostics.push(rejected_diagnostic(
                        &relative_path,
                        &format!(
                            "The request exceeds the {} MiB recoverable snapshot limit; split the quarantine into smaller runs.",
                            MAX_QUARANTINE_TOTAL_BYTES / (1024 * 1024)
                        ),
                    ));
                } else {
                    captured_bytes = next_total;
                    items.push(item);
                }
            }
            Ok(_) => diagnostics.push(rejected_diagnostic(
                &relative_path,
                "Two requested paths resolved to the same recovery destination.",
            )),
            Err(detail) => diagnostics.push(rejected_diagnostic(&relative_path, &detail)),
        }
    }
    items.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    diagnostics.sort();
    Ok(QuarantinePlan {
        project_root,
        recovery_root,
        items,
        diagnostics,
    })
}

fn capture_item(
    project_root: &Utf8Path,
    recovery_root: &Utf8Path,
    run_id: &str,
    relative_path: &Utf8Path,
) -> Result<QuarantineItem, String> {
    validate_relative_path(relative_path)?;
    reject_symlink_components(project_root, relative_path)?;
    let source_path = project_root.join(relative_path);
    let before = fs::symlink_metadata(&source_path)
        .map_err(|error| format!("Could not inspect the source: {error}"))?;
    if before.file_type().is_symlink() {
        return Err("The source itself is a symbolic link.".to_owned());
    }
    if !before.is_file() {
        return Err("Only regular files can be quarantined.".to_owned());
    }
    if before.len() > MAX_QUARANTINE_FILE_BYTES {
        return Err(format!(
            "The source is {} bytes; quarantine snapshots are limited to {} MiB per file.",
            before.len(),
            MAX_QUARANTINE_FILE_BYTES / (1024 * 1024)
        ));
    }
    let bytes = read_exact_bounded(&source_path, before.len())
        .map_err(|error| format!("Could not capture the source bytes safely: {error}"))?;
    let after = fs::symlink_metadata(&source_path)
        .map_err(|error| format!("Could not revalidate the source: {error}"))?;
    if after.file_type().is_symlink() || !after.is_file() {
        return Err("The source type changed while it was being captured.".to_owned());
    }
    if before.len() != after.len()
        || before.modified().ok() != after.modified().ok()
        || after.len() != u64::try_from(bytes.len()).unwrap_or(u64::MAX)
    {
        return Err("The source changed while it was being captured; retry the plan.".to_owned());
    }
    let destination_path = recovery_root.join(run_id).join(relative_path);
    reject_destination_symlink_components(recovery_root, run_id, relative_path)?;
    if fs::symlink_metadata(&destination_path).is_ok() {
        return Err(
            "The recovery destination already exists and will not be overwritten.".to_owned(),
        );
    }
    let hash = blake3::hash(&bytes).to_hex().to_string();
    Ok(QuarantineItem {
        relative_path: relative_path.to_owned(),
        source_path: source_path.clone(),
        destination_path,
        restore_path: source_path,
        snapshot: QuarantineSnapshot {
            byte_length: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
            readonly: before.permissions().readonly(),
            bytes: Arc::from(bytes),
            blake3_hash: hash,
        },
    })
}

fn read_exact_bounded(path: &Utf8Path, expected_length: u64) -> std::io::Result<Vec<u8>> {
    if expected_length > MAX_QUARANTINE_FILE_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "file exceeds the quarantine snapshot limit",
        ));
    }
    let capacity = usize::try_from(expected_length)
        .map_err(|_| std::io::Error::other("file length does not fit memory limits"))?;
    let mut bytes = Vec::with_capacity(capacity);
    File::open(path)?
        .take(expected_length.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if u64::try_from(bytes.len()) != Ok(expected_length) {
        return Err(std::io::Error::other(
            "file length changed while it was being read",
        ));
    }
    Ok(bytes)
}

/// Revalidates a snapshot without allocating another file-sized buffer.
///
/// # Errors
///
/// Returns an I/O error when the source metadata or bytes cannot be read.
pub fn quarantine_snapshot_matches(item: &QuarantineItem) -> std::io::Result<bool> {
    let metadata = fs::symlink_metadata(&item.source_path)?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.len() != item.snapshot.byte_length
        || metadata.len() > MAX_QUARANTINE_FILE_BYTES
    {
        return Ok(false);
    }
    let mut input =
        File::open(&item.source_path)?.take(item.snapshot.byte_length.saturating_add(1));
    let mut hasher = blake3::Hasher::new();
    let mut length = 0_u64;
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        let read = input.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        length = length.saturating_add(read as u64);
        hasher.update(&buffer[..read]);
    }
    Ok(length == item.snapshot.byte_length
        && hasher.finalize().to_hex().as_str() == item.snapshot.blake3_hash)
}

fn canonical_directory(
    kind: &'static str,
    path: &Utf8Path,
) -> Result<Utf8PathBuf, QuarantinePlanError> {
    let canonical = fs::canonicalize(path).map_err(|error| QuarantinePlanError::InvalidRoot {
        kind,
        path: path.to_owned(),
        detail: error.to_string(),
    })?;
    let canonical =
        Utf8PathBuf::from_path_buf(canonical).map_err(|path| QuarantinePlanError::InvalidRoot {
            kind,
            path: path.to_string_lossy().into_owned().into(),
            detail: "the canonical path is not valid UTF-8".to_owned(),
        })?;
    let metadata = fs::metadata(&canonical).map_err(|error| QuarantinePlanError::InvalidRoot {
        kind,
        path: canonical.clone(),
        detail: error.to_string(),
    })?;
    if !metadata.is_dir() {
        return Err(QuarantinePlanError::InvalidRoot {
            kind,
            path: canonical,
            detail: "the root is not a directory".to_owned(),
        });
    }
    Ok(canonical)
}

fn validate_relative_path(path: &Utf8Path) -> Result<(), String> {
    if path.as_str().is_empty() || path.is_absolute() {
        return Err("The source path must be non-empty and project-relative.".to_owned());
    }
    if path.components().any(|component| {
        matches!(
            component,
            camino::Utf8Component::CurDir
                | camino::Utf8Component::ParentDir
                | camino::Utf8Component::RootDir
                | camino::Utf8Component::Prefix(_)
        )
    }) {
        return Err(
            "Current-directory, parent-traversal and rooted source components are forbidden."
                .to_owned(),
        );
    }
    Ok(())
}

fn reject_symlink_components(root: &Utf8Path, relative: &Utf8Path) -> Result<(), String> {
    let mut current = root.to_owned();
    for component in relative.components() {
        if matches!(
            component,
            camino::Utf8Component::CurDir | camino::Utf8Component::Prefix(_)
        ) {
            continue;
        }
        current.push(component.as_str());
        let metadata = fs::symlink_metadata(&current)
            .map_err(|error| format!("Could not inspect a source path component: {error}"))?;
        if metadata.file_type().is_symlink() {
            return Err(format!(
                "Symbolic-link path component `{current}` is not allowed."
            ));
        }
    }
    Ok(())
}

fn reject_destination_symlink_components(
    recovery_root: &Utf8Path,
    run_id: &str,
    relative: &Utf8Path,
) -> Result<(), String> {
    let destination = Utf8Path::new(run_id).join(relative);
    let mut current = recovery_root.to_owned();
    for component in destination.components() {
        current.push(component.as_str());
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(format!(
                    "Recovery path component `{current}` is a symbolic link."
                ));
            }
            Ok(metadata) if !metadata.is_dir() && current != recovery_root.join(&destination) => {
                return Err(format!(
                    "Recovery path component `{current}` is not a directory."
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
            Err(error) => {
                return Err(format!(
                    "Could not inspect recovery path component `{current}`: {error}"
                ));
            }
        }
    }
    Ok(())
}

fn valid_run_id(run_id: &str) -> bool {
    !run_id.is_empty()
        && run_id != "."
        && run_id != ".."
        && run_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn rejected_diagnostic(path: &Utf8Path, detail: &str) -> Diagnostic {
    Diagnostic {
        rule_id: RULE_REJECTED.to_owned(),
        path: if path.as_str().is_empty() {
            Utf8PathBuf::from("<empty-path>")
        } else {
            path.to_owned()
        },
        range: TextRange::empty(TextSize::new(0)),
        severity: Severity::Warning,
        message: "The unexpected path was not added to the quarantine plan.".to_owned(),
        source: DiagnosticSource::Project,
        notes: vec![detail.to_owned()],
        help: Some(
            "Use a project-relative regular file and an empty external recovery destination."
                .to_owned(),
        ),
        localized: None,
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use camino::Utf8PathBuf;
    use tempfile::TempDir;

    use crate::{DestructiveCapability, DestructiveRequest, EXACT_CONFIRMATION_PHRASE};

    use super::{
        MAX_QUARANTINE_FILE_BYTES, QuarantinePlanError, QuarantineRequest, plan_quarantine,
        quarantine_snapshot_matches,
    };

    fn authorization() -> crate::DestructiveAuthorization {
        DestructiveRequest::one(DestructiveCapability::QuarantineUnexpectedFiles)
            .authorize_interactively(EXACT_CONFIRMATION_PHRASE)
            .expect("explicit authorization")
    }

    fn fixture() -> (TempDir, Utf8PathBuf, Utf8PathBuf) {
        let temporary = TempDir::new().expect("temporary root");
        let root = Utf8PathBuf::from_path_buf(temporary.path().to_owned()).expect("UTF-8 temp");
        let project = root.join("project");
        let recovery = root.join("recovery");
        fs::create_dir(&project).expect("project");
        fs::create_dir(&recovery).expect("recovery");
        (temporary, project, recovery)
    }

    #[test]
    fn captures_bytes_hash_and_stable_external_destinations_without_writing() {
        let (_temporary, project, recovery) = fixture();
        fs::create_dir(project.join("notes")).expect("notes");
        fs::write(project.join("z.bin"), [0, 1, 2]).expect("binary fixture");
        fs::write(project.join("notes/a.txt"), b"recover me").expect("text fixture");
        let request = QuarantineRequest {
            project_root: project.clone(),
            recovery_root: recovery.clone(),
            run_id: "run-001".to_owned(),
            paths: vec![
                Utf8PathBuf::from("z.bin"),
                Utf8PathBuf::from("notes/a.txt"),
                Utf8PathBuf::from("z.bin"),
            ],
        };
        let plan = plan_quarantine(&request, &authorization()).expect("plan");
        assert_eq!(
            plan.items
                .iter()
                .map(|item| item.relative_path.as_str())
                .collect::<Vec<_>>(),
            ["notes/a.txt", "z.bin"]
        );
        let binary = &plan.items[1];
        assert_eq!(binary.snapshot.bytes.as_ref(), [0, 1, 2]);
        assert_eq!(
            binary.snapshot.blake3_hash,
            blake3::hash(&[0, 1, 2]).to_hex().to_string()
        );
        assert_eq!(
            binary.destination_path,
            plan.recovery_root.join("run-001/z.bin")
        );
        assert!(
            !recovery.join("run-001").exists(),
            "planning must perform no writes"
        );
        assert!(project.join("z.bin").exists());

        let repeated = plan_quarantine(&request, &authorization()).expect("repeated plan");
        assert_eq!(repeated, plan, "read-only planning must be idempotent");
    }

    #[test]
    fn rejects_non_regular_traversal_existing_destination_and_inside_recovery() {
        let (_temporary, project, recovery) = fixture();
        fs::create_dir(project.join("directory")).expect("directory");
        fs::write(project.join("safe.txt"), b"safe").expect("file");
        fs::create_dir(recovery.join("run")).expect("run");
        fs::write(recovery.join("run/safe.txt"), b"occupied").expect("occupied");
        let request = QuarantineRequest {
            project_root: project.clone(),
            recovery_root: recovery,
            run_id: "run".to_owned(),
            paths: vec![
                Utf8PathBuf::from("../escape"),
                Utf8PathBuf::from("directory"),
                Utf8PathBuf::from("safe.txt"),
            ],
        };
        let plan = plan_quarantine(&request, &authorization()).expect("partial plan");
        assert!(plan.items.is_empty());
        assert_eq!(plan.diagnostics.len(), 3);

        let inside = QuarantineRequest {
            project_root: project.clone(),
            recovery_root: project,
            run_id: "run".to_owned(),
            paths: Vec::new(),
        };
        assert!(matches!(
            plan_quarantine(&inside, &authorization()),
            Err(QuarantinePlanError::RecoveryOverlapsProject(_))
        ));
    }

    #[test]
    fn refuses_an_oversized_file_before_allocating_its_contents() {
        let (_temporary, project, recovery) = fixture();
        let oversized = project.join("large.bin");
        fs::File::create(&oversized)
            .expect("sparse fixture")
            .set_len(MAX_QUARANTINE_FILE_BYTES + 1)
            .expect("sparse length");
        let request = QuarantineRequest {
            project_root: project,
            recovery_root: recovery,
            run_id: "run".to_owned(),
            paths: vec![Utf8PathBuf::from("large.bin")],
        };

        let plan = plan_quarantine(&request, &authorization()).expect("bounded plan");

        assert!(plan.items.is_empty());
        assert_eq!(plan.diagnostics.len(), 1);
        assert!(plan.diagnostics[0].notes[0].contains("32 MiB"));
    }

    #[test]
    fn streaming_revalidation_detects_a_changed_snapshot() {
        let (_temporary, project, recovery) = fixture();
        fs::write(project.join("note.txt"), b"before").expect("fixture");
        let request = QuarantineRequest {
            project_root: project.clone(),
            recovery_root: recovery,
            run_id: "run".to_owned(),
            paths: vec![Utf8PathBuf::from("note.txt")],
        };
        let plan = plan_quarantine(&request, &authorization()).expect("plan");
        let item = plan.items.first().expect("captured item");
        assert!(quarantine_snapshot_matches(item).expect("original snapshot"));

        fs::write(project.join("note.txt"), b"changed").expect("mutation");

        assert!(!quarantine_snapshot_matches(item).expect("changed snapshot"));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_a_symlink_or_symlinked_ancestor() {
        use std::os::unix::fs::symlink;

        let (_temporary, project, recovery) = fixture();
        let outside = recovery.join("outside.txt");
        fs::write(&outside, b"outside").expect("outside");
        symlink(&outside, project.join("link.txt")).expect("file symlink");
        fs::create_dir(recovery.join("real")).expect("real directory");
        fs::write(recovery.join("real/nested.txt"), b"nested").expect("nested");
        symlink(recovery.join("real"), project.join("linked-dir")).expect("directory symlink");
        let request = QuarantineRequest {
            project_root: project,
            recovery_root: recovery,
            run_id: "run".to_owned(),
            paths: vec![
                Utf8PathBuf::from("link.txt"),
                Utf8PathBuf::from("linked-dir/nested.txt"),
            ],
        };
        let plan = plan_quarantine(&request, &authorization()).expect("plan");
        assert!(plan.items.is_empty());
        assert_eq!(plan.diagnostics.len(), 2);
    }

    #[cfg(unix)]
    #[test]
    fn rejects_a_symlinked_recovery_component() {
        use std::os::unix::fs::symlink;

        let (temporary, project, recovery) = fixture();
        fs::write(project.join("unexpected.txt"), b"recoverable").expect("unexpected file");
        let redirected = Utf8PathBuf::from_path_buf(temporary.path().join("redirected"))
            .expect("UTF-8 redirected path");
        fs::create_dir(&redirected).expect("redirected directory");
        symlink(&redirected, recovery.join("run")).expect("recovery symlink");
        let request = QuarantineRequest {
            project_root: project,
            recovery_root: recovery,
            run_id: "run".to_owned(),
            paths: vec![Utf8PathBuf::from("unexpected.txt")],
        };
        let plan = plan_quarantine(&request, &authorization()).expect("plan");
        assert!(plan.items.is_empty());
        assert_eq!(plan.diagnostics.len(), 1);
        assert!(!redirected.join("unexpected.txt").exists());
    }
}
