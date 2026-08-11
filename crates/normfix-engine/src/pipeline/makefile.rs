//! Makefile analysis, reconciliation, and the trivia probe.
//!
//! Norminette does not parse GNU Make, so everything here is this project's own
//! conservative reading. It only ever acts on a wholly literal source list;
//! expansions, patterns, and recipes are left exactly as written.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use camino::Utf8PathBuf;
use normfix_core::{Diagnostic, DiagnosticSource, Severity};
use normfix_destructive::DestructiveCapability;
use normfix_makefile::{
    SourcePathStatus, analyze_makefile, format_makefile, reconcile_source_references,
};

use normfix_header::RunClock;
use normfix_project::DiscoveredFile;
use normfix_report::FileReport;

use super::{FileWork, FixOptions, append_header_fixes, append_header_issues, text_range};
use normfix_actions::{PlannedFile, ReadPrecondition};

pub(super) const MAKEFILE_TRIVIA_PROBE_LIMIT: u64 = 8 * 1024 * 1024;

pub(super) fn process_makefile(
    file: &DiscoveredFile,
    path: Utf8PathBuf,
    original_bytes: &[u8],
    original: String,
    options: &FixOptions,
    clock: &RunClock,
) -> FileWork {
    if options.lint_only {
        let mut after = makefile_diagnostics(file, &path, &original, options);
        after.sort();
        after.dedup();
        let before = after.clone();
        return FileWork {
            absolute_path: file.path.clone(),
            report: FileReport {
                path,
                changed: false,
                written: false,
                backup: None,
                failure: None,
                fixes: Vec::new(),
                before,
                after,
                original: Some(Arc::from(original.clone())),
                fixed: Some(Arc::from(original)),
            },
            plan: None,
            read_preconditions: Vec::new(),
        };
    }
    let filename = file
        .path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Makefile");
    let remove_missing = options.remove_missing_makefile_sources
        && options
            .destructive_authorization
            .as_ref()
            .is_some_and(|authorization| {
                authorization.allows(DestructiveCapability::RemoveMissingMakefileSources)
            });
    let mut before = makefile_diagnostics(file, &path, &original, options);
    before.sort();
    before.dedup();
    let mut empty_source_proofs = BTreeMap::<String, (PathBuf, [u8; 32])>::new();
    let reconciled = reconcile_source_references(&original, remove_missing, |reference| {
        let (status, proof) = makefile_source_probe(&file.path, &options.cwd, reference);
        if let Some(proof) = proof {
            empty_source_proofs.insert(reference.to_owned(), proof);
        }
        status
    });
    let formatted = format_makefile(
        &reconciled.output,
        filename,
        options.identity.as_ref(),
        clock,
    );
    let mut fixes = Vec::new();
    append_header_fixes(&mut fixes, &reconciled.fixes, &original);
    append_header_fixes(&mut fixes, &formatted.fixes, &reconciled.output);
    let mut after = Vec::new();
    append_header_issues(&mut after, &path, &formatted.issues);
    after.extend(makefile_diagnostics(
        file,
        &path,
        &formatted.output,
        options,
    ));
    after.sort();
    after.dedup();
    let changed = formatted.output.as_bytes() != original_bytes;
    let read_preconditions =
        makefile_reconciliation_preconditions(file, options, &reconciled, &empty_source_proofs);
    let plan = changed.then(|| PlannedFile {
        path: file.path.clone(),
        original: original_bytes.to_vec(),
        replacement: formatted.output.as_bytes().to_vec(),
        fixes: fixes.clone(),
    });
    FileWork {
        absolute_path: file.path.clone(),
        report: FileReport {
            path,
            changed,
            written: false,
            backup: None,
            failure: None,
            fixes,
            before,
            after,
            original: Some(Arc::from(original)),
            fixed: Some(Arc::from(formatted.output)),
        },
        plan,
        read_preconditions,
    }
}

pub(super) fn makefile_reconciliation_preconditions(
    file: &DiscoveredFile,
    options: &FixOptions,
    reconciled: &normfix_makefile::SourceReconciliation,
    empty_source_proofs: &BTreeMap<String, (PathBuf, [u8; 32])>,
) -> Vec<ReadPrecondition> {
    let removed_missing_ranges = reconciled
        .fixes
        .iter()
        .filter(|fix| fix.code == "MAKEFILE_REMOVE_MISSING_SOURCE")
        .map(|fix| fix.range)
        .collect::<BTreeSet<_>>();
    let removed_empty_ranges = reconciled
        .fixes
        .iter()
        .filter(|fix| fix.code == "MAKEFILE_REMOVE_EMPTY_SOURCE")
        .map(|fix| fix.range)
        .collect::<BTreeSet<_>>();
    let mut preconditions = reconciled
        .missing
        .iter()
        .filter(|reference| removed_missing_ranges.contains(&reference.range))
        .filter_map(|reference| {
            makefile_source_path(&file.path, &options.cwd, &reference.path)
                .map(ReadPrecondition::absent)
        })
        .collect::<Vec<_>>();
    preconditions.extend(
        reconciled
            .empty
            .iter()
            .filter(|reference| removed_empty_ranges.contains(&reference.range))
            .filter_map(|reference| empty_source_proofs.get(&reference.path))
            .map(|(path, digest)| ReadPrecondition::Matches {
                path: path.clone(),
                blake3: *digest,
            }),
    );
    preconditions
}

pub(super) fn makefile_diagnostics(
    file: &DiscoveredFile,
    path: &Utf8PathBuf,
    source: &str,
    options: &FixOptions,
) -> Vec<Diagnostic> {
    let mut diagnostics = analyze_makefile(source)
        .into_iter()
        .map(|item| Diagnostic {
            rule_id: item.code.to_owned(),
            path: path.clone(),
            range: text_range(item.range),
            severity: Severity::Warning,
            message: item.message,
            source: DiagnosticSource::Makefile,
            notes: (!item.detail.is_empty())
                .then_some(item.detail)
                .into_iter()
                .collect(),
            help: Some(item.suggestion),
        })
        .collect::<Vec<_>>();
    let remaining_sources = reconcile_source_references(source, false, |reference| {
        makefile_source_status(&file.path, &options.cwd, reference)
    });
    diagnostics.extend(remaining_sources.missing.into_iter().map(|reference| {
        Diagnostic {
            rule_id: "MAKEFILE_SOURCE_NOT_FOUND".to_owned(),
            path: path.clone(),
            range: text_range(reference.range),
            severity: Severity::Warning,
            message: format!(
                "The literal Makefile source `{}` does not exist below the project root.",
                reference.path
            ),
            source: DiagnosticSource::Makefile,
            notes: vec![
                "Only a wholly literal SRC/SRCS-style assignment was inspected; Make recipes and expansions were never executed."
                    .to_owned(),
            ],
            help: Some(
                "Create/correct the source path, or use the explicitly authorized unsafe removal mode to remove this exact stale token."
                    .to_owned(),
            ),
        }
    }));
    diagnostics.extend(remaining_sources.empty.into_iter().map(|reference| Diagnostic {
        rule_id: "MAKEFILE_SOURCE_EMPTY".to_owned(),
        path: path.clone(),
        range: text_range(reference.range),
        severity: Severity::Warning,
        message: format!(
            "The literal Makefile source `{}` contains only whitespace or comments.",
            reference.path
        ),
        source: DiagnosticSource::Makefile,
        notes: vec![
            "The file was read as a regular non-symbolic path below the project root; comments and an official header do not count as an implementation."
                .to_owned(),
        ],
        help: Some(
            "Implement the source, remove the stale entry manually, or use explicitly authorized unsafe mode to remove this exact empty token."
                .to_owned(),
        ),
    }));
    diagnostics
}

pub(super) fn makefile_source_status(
    makefile: &Path,
    project_root: &Path,
    reference: &str,
) -> SourcePathStatus {
    makefile_source_probe(makefile, project_root, reference).0
}

pub(super) fn makefile_source_probe(
    makefile: &Path,
    project_root: &Path,
    reference: &str,
) -> (SourcePathStatus, Option<(PathBuf, [u8; 32])>) {
    let Ok(root) = project_root.canonicalize() else {
        return (SourcePathStatus::Unknown, None);
    };
    let Some(parent) = makefile.parent() else {
        return (SourcePathStatus::Unknown, None);
    };
    let Ok(parent) = parent.canonicalize() else {
        return (SourcePathStatus::Unknown, None);
    };
    if !parent.starts_with(&root) {
        return (SourcePathStatus::Unknown, None);
    }
    let relative = Path::new(reference);
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return (SourcePathStatus::Unknown, None);
    }
    let mut candidate = parent;
    for component in relative.components() {
        let std::path::Component::Normal(name) = component else {
            return (SourcePathStatus::Unknown, None);
        };
        candidate.push(name);
        match std::fs::symlink_metadata(&candidate) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return (SourcePathStatus::Unknown, None);
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return (SourcePathStatus::Missing, None);
            }
            Err(_) => return (SourcePathStatus::Unknown, None),
        }
    }
    let Ok(metadata) = std::fs::symlink_metadata(&candidate) else {
        return (SourcePathStatus::Unknown, None);
    };
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return (SourcePathStatus::Unknown, None);
    }
    let Ok(input) = File::open(&candidate) else {
        return (SourcePathStatus::Unknown, None);
    };
    let Ok(opened_metadata) = input.metadata() else {
        return (SourcePathStatus::Unknown, None);
    };
    if !opened_metadata.is_file() || opened_metadata.len() > MAKEFILE_TRIVIA_PROBE_LIMIT {
        return (SourcePathStatus::Unknown, None);
    }
    let expected_length = opened_metadata.len();
    let Ok(capacity) = usize::try_from(expected_length) else {
        return (SourcePathStatus::Unknown, None);
    };
    let mut bytes = Vec::with_capacity(capacity);
    if input
        .take(expected_length.saturating_add(1))
        .read_to_end(&mut bytes)
        .is_err()
        || u64::try_from(bytes.len()) != Ok(expected_length)
    {
        return (SourcePathStatus::Unknown, None);
    }
    if contains_only_c_trivia(&bytes) {
        let digest = *blake3::hash(&bytes).as_bytes();
        let Ok(relative) = candidate.strip_prefix(&root) else {
            return (SourcePathStatus::Unknown, None);
        };
        return (
            SourcePathStatus::Empty,
            Some((project_root.join(relative), digest)),
        );
    }
    (SourcePathStatus::Exists, None)
}

pub(super) fn contains_only_c_trivia(bytes: &[u8]) -> bool {
    let mut index = 0_usize;
    while index < bytes.len() {
        if bytes[index].is_ascii_whitespace() {
            index += 1;
            continue;
        }
        if bytes.get(index..index + 2) == Some(b"//") {
            index += 2;
            while index < bytes.len() && !matches!(bytes[index], b'\r' | b'\n') {
                index += 1;
            }
            continue;
        }
        if bytes.get(index..index + 2) == Some(b"/*") {
            let Some(end) = bytes[index + 2..]
                .windows(2)
                .position(|window| window == b"*/")
            else {
                return false;
            };
            index += end + 4;
            continue;
        }
        return false;
    }
    true
}

pub(super) fn makefile_source_path(
    makefile: &Path,
    project_root: &Path,
    reference: &str,
) -> Option<PathBuf> {
    let root = project_root.canonicalize().ok()?;
    let parent = makefile.parent()?.canonicalize().ok()?;
    if !parent.starts_with(&root) {
        return None;
    }
    let relative = Path::new(reference);
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return None;
    }
    let candidate = parent.join(relative);
    let inside = candidate.strip_prefix(&root).ok()?;
    Some(project_root.join(inside))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use normfix_makefile::SourcePathStatus;
    use tempfile::TempDir;

    use super::{MAKEFILE_TRIVIA_PROBE_LIMIT, makefile_source_path, makefile_source_probe};

    #[test]
    fn oversized_makefile_source_is_not_classified_as_trivia_only() {
        let project = TempDir::new().expect("project");
        let makefile = project.path().join("Makefile");
        fs::write(&makefile, "SRCS = generated.c\n").expect("Makefile");
        let source = fs::File::create(project.path().join("generated.c")).expect("source");
        source
            .set_len(MAKEFILE_TRIVIA_PROBE_LIMIT + 1)
            .expect("sparse oversized source");

        let (status, proof) = makefile_source_probe(&makefile, project.path(), "generated.c");

        assert_eq!(status, SourcePathStatus::Unknown);
        assert!(proof.is_none());
    }

    #[test]
    fn makefile_source_paths_stay_in_the_caller_path_vocabulary() {
        let project = TempDir::new().expect("project");
        let makefile = project.path().join("Makefile");
        fs::write(&makefile, "SRC = main.c\n").expect("makefile");

        let resolved = makefile_source_path(&makefile, project.path(), "main.c")
            .expect("a literal source inside the project resolves");

        // Returning the canonical form here mixed two path vocabularies, so the
        // common transaction ancestor collapsed to `/` on macOS, where /var is
        // a symbolic link, and the write was refused.
        assert!(resolved.starts_with(project.path()), "{resolved:?}");
        assert_eq!(resolved.file_name().expect("file name"), "main.c");
    }

    #[test]
    fn makefile_source_paths_outside_the_project_are_refused() {
        let project = TempDir::new().expect("project");
        let makefile = project.path().join("Makefile");
        fs::write(&makefile, "SRC = main.c\n").expect("makefile");

        assert!(makefile_source_path(&makefile, project.path(), "../escape.c").is_none());
        assert!(makefile_source_path(&makefile, project.path(), "/etc/passwd").is_none());
        assert!(makefile_source_path(&makefile, project.path(), "nested/../../out.c").is_none());
    }
}
