//! Closed-world safety proof for filename-derived header-guard renames.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use ignore::WalkBuilder;
use thiserror::Error;

const MAX_PROJECT_FILES: usize = 25_000;
const MAX_PROJECT_BYTES: u64 = 512 * 1024 * 1024;

/// A minimal guard pair that may be renamed atomically.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GuardRename {
    /// Header path.
    pub path: PathBuf,
    /// Existing macro.
    pub current: String,
    /// Existing macro in `#define`; differs when the guard pair is mismatched.
    pub define_current: String,
    /// Filename-derived replacement.
    pub expected: String,
    /// UTF-8 byte start/end of the `#ifndef` macro.
    pub ifndef_range: std::ops::Range<usize>,
    /// UTF-8 byte start/end of the `#define` macro.
    pub define_range: std::ops::Range<usize>,
    /// BLAKE3 digest of the exact approved header.
    pub header_digest: [u8; 32],
}

/// Content-addressed project snapshot bound to an approval.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectSnapshot {
    /// Canonical Git project root.
    pub root: PathBuf,
    /// Every regular project file and its content digest.
    pub files: BTreeMap<PathBuf, [u8; 32]>,
}

/// One rename and the closed-world state that proved it safe.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GuardApproval {
    /// Approved local replacement.
    pub rename: GuardRename,
    /// Whole-project content snapshot.
    pub snapshot: ProjectSnapshot,
}

/// Snapshot-bound plan for adding a missing whole-file inclusion guard.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GuardInsertion {
    /// Header path.
    pub path: PathBuf,
    /// Filename-derived macro used by both opening directives.
    pub expected: String,
    /// BLAKE3 digest of the exact unguarded header.
    pub header_digest: [u8; 32],
}

/// One missing-guard insertion and the closed-world state that proved it safe.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GuardInsertionApproval {
    /// Approved insertion.
    pub insertion: GuardInsertion,
    /// Whole-project content snapshot.
    pub snapshot: ProjectSnapshot,
}

/// Header-guard planning failed closed.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum GuardPlanError {
    /// The selected path is not inside a discoverable Git worktree.
    #[error("no Git project root could be proven for `{0}`")]
    NoProjectRoot(PathBuf),
    /// The project could not be traversed without uncertainty.
    #[error("could not establish a closed project snapshot: {0}")]
    ProjectScan(String),
}

/// Plans independently proven guard renames for selected headers.
///
/// A missing or ambiguous proof simply omits that header. Project-scan
/// failures are returned because silently writing after an incomplete scan
/// would violate the closed-world guarantee.
///
/// # Errors
///
/// Returns [`GuardPlanError`] if any candidate project cannot be completely
/// snapshotted.
pub fn plan_guard_renames(
    selected_headers: &[PathBuf],
) -> Result<BTreeMap<PathBuf, GuardApproval>, GuardPlanError> {
    if selected_headers.len() > 256 {
        return Err(GuardPlanError::ProjectScan(
            "more than 256 header-guard candidates were requested".to_owned(),
        ));
    }
    let mut candidates_by_root: BTreeMap<PathBuf, Vec<GuardRename>> = BTreeMap::new();
    for path in selected_headers {
        if path.extension() != Some(OsStr::new("h")) {
            continue;
        }
        let Ok(source) = fs::read_to_string(path) else {
            continue;
        };
        let Some(rename) = guard_candidate(path, &source) else {
            continue;
        };
        let root = find_git_root(path.parent().unwrap_or(Path::new(".")))
            .ok_or_else(|| GuardPlanError::NoProjectRoot(path.clone()))?;
        candidates_by_root.entry(root).or_default().push(rename);
    }

    let mut approvals = BTreeMap::new();
    for (root, candidates) in candidates_by_root {
        let snapshot = scan_project(&root)?;
        let contents = read_snapshot_contents(&snapshot)?;
        let duplicate_guards = duplicate_filename_guards(contents.keys());
        let build_is_dynamic = contents
            .iter()
            .any(|(path, bytes)| is_build_file(path) && has_dynamic_build_definitions(bytes));
        for rename in candidates {
            if build_is_dynamic || duplicate_guards.contains(&rename.expected) {
                continue;
            }
            let current_occurrences = project_identifier_occurrences(&contents, &rename.current);
            let define_occurrences = if rename.define_current == rename.current {
                current_occurrences.clone()
            } else {
                project_identifier_occurrences(&contents, &rename.define_current)
            };
            let expected_occurrences = project_identifier_occurrences(&contents, &rename.expected);
            let target = canonical_or_lexical(&rename.path);
            let expected_local_count = usize::from(rename.current == rename.expected)
                + usize::from(rename.define_current == rename.expected);
            let expected_is_private = expected_occurrences.len() == expected_local_count
                && expected_occurrences.iter().all(|path| path == &target);
            let opening_is_private = current_occurrences.len()
                == usize::from(rename.define_current == rename.current) + 1
                && current_occurrences.iter().all(|path| path == &target);
            let define_is_private = define_occurrences.len()
                == usize::from(rename.define_current == rename.current) + 1
                && define_occurrences.iter().all(|path| path == &target);
            if !expected_is_private || !opening_is_private || !define_is_private {
                continue;
            }
            approvals.insert(
                target,
                GuardApproval {
                    rename,
                    snapshot: snapshot.clone(),
                },
            );
        }
    }
    Ok(approvals)
}

/// Plans whole-file guards for structurally ordinary unguarded headers.
///
/// Headers containing conditional preprocessing, `#pragma once`, `#undef`,
/// duplicate filename-derived guards, dynamic build definitions, or a macro
/// collision are omitted. The returned approval is bound to every project
/// byte and must be rechecked immediately before applying it.
///
/// # Errors
///
/// Returns [`GuardPlanError`] when a candidate project cannot be snapshotted
/// completely.
pub fn plan_guard_insertions(
    selected_headers: &[PathBuf],
) -> Result<BTreeMap<PathBuf, GuardInsertionApproval>, GuardPlanError> {
    if selected_headers.len() > 256 {
        return Err(GuardPlanError::ProjectScan(
            "more than 256 header-guard candidates were requested".to_owned(),
        ));
    }
    let mut candidates_by_root: BTreeMap<PathBuf, Vec<GuardInsertion>> = BTreeMap::new();
    for path in selected_headers {
        if path.extension() != Some(OsStr::new("h")) {
            continue;
        }
        let Ok(source) = fs::read_to_string(path) else {
            continue;
        };
        let Some(insertion) = guard_insertion_candidate(path, &source) else {
            continue;
        };
        let root = find_git_root(path.parent().unwrap_or(Path::new(".")))
            .ok_or_else(|| GuardPlanError::NoProjectRoot(path.clone()))?;
        candidates_by_root.entry(root).or_default().push(insertion);
    }

    let mut approvals = BTreeMap::new();
    for (root, candidates) in candidates_by_root {
        let snapshot = scan_project(&root)?;
        let contents = read_snapshot_contents(&snapshot)?;
        let duplicate_guards = duplicate_filename_guards(contents.keys());
        let build_is_dynamic = contents
            .iter()
            .any(|(path, bytes)| is_build_file(path) && has_dynamic_build_definitions(bytes));
        for insertion in candidates {
            if build_is_dynamic
                || duplicate_guards.contains(&insertion.expected)
                || !project_identifier_occurrences(&contents, &insertion.expected).is_empty()
            {
                continue;
            }
            approvals.insert(
                canonical_or_lexical(&insertion.path),
                GuardInsertionApproval {
                    insertion,
                    snapshot: snapshot.clone(),
                },
            );
        }
    }
    Ok(approvals)
}

/// Recomputes every project content hash before a guard rename is committed.
#[must_use]
pub fn guard_approval_is_current(approval: &GuardApproval) -> bool {
    scan_project(&approval.snapshot.root).is_ok_and(|current| current == approval.snapshot)
        && fs::read(&approval.rename.path)
            .is_ok_and(|bytes| *blake3::hash(&bytes).as_bytes() == approval.rename.header_digest)
}

/// Rechecks every project hash and the exact unguarded header bytes.
#[must_use]
pub fn guard_insertion_approval_is_current(approval: &GuardInsertionApproval) -> bool {
    scan_project(&approval.snapshot.root).is_ok_and(|current| current == approval.snapshot)
        && fs::read(&approval.insertion.path)
            .is_ok_and(|bytes| *blake3::hash(&bytes).as_bytes() == approval.insertion.header_digest)
}

fn guard_insertion_candidate(path: &Path, source: &str) -> Option<GuardInsertion> {
    let expected = expected_guard(path.file_name()?.to_str()?);
    if !is_identifier(&expected) || guard_candidate(path, source).is_some() {
        return None;
    }
    let code = normalized_code(source.as_bytes());
    for line in String::from_utf8_lossy(&code).lines() {
        let directive = line.trim_start().strip_prefix('#').map(str::trim_start);
        if directive.is_some_and(|directive| {
            [
                "if",
                "ifdef",
                "ifndef",
                "elif",
                "else",
                "endif",
                "undef",
                "pragma once",
            ]
            .iter()
            .any(|keyword| {
                directive == *keyword
                    || directive
                        .strip_prefix(keyword)
                        .is_some_and(|tail| tail.chars().next().is_some_and(char::is_whitespace))
            })
        }) {
            return None;
        }
    }
    Some(GuardInsertion {
        path: canonical_or_lexical(path),
        expected,
        header_digest: *blake3::hash(source.as_bytes()).as_bytes(),
    })
}

fn guard_candidate(path: &Path, source: &str) -> Option<GuardRename> {
    let expected = expected_guard(path.file_name()?.to_str()?);
    if !is_identifier(&expected) {
        return None;
    }
    let mut directives = Vec::new();
    let mut offset = 0;
    for line in source.split_inclusive('\n') {
        let raw = line.trim_end_matches(['\r', '\n']);
        if let Some((kind, name, local_start)) = parse_guard_directive(raw) {
            directives.push((
                kind,
                name.to_owned(),
                offset + local_start..offset + local_start + name.len(),
            ));
        }
        offset += line.len();
    }
    let ifndef = directives.iter().find(|(kind, _, _)| *kind == "ifndef")?;
    let define = directives.iter().find(|(kind, _, _)| *kind == "define")?;
    if ifndef.1 == expected && define.1 == expected {
        return None;
    }
    if !whole_file_guard_layout(source, &ifndef.1, &define.1) {
        return None;
    }
    let expected_opening_occurrences = usize::from(ifndef.1 == define.1) + 1;
    if directives
        .iter()
        .filter(|(_, name, _)| name == &ifndef.1)
        .count()
        != expected_opening_occurrences
        || directives
            .iter()
            .filter(|(_, name, _)| name == &define.1)
            .count()
            != expected_opening_occurrences
    {
        return None;
    }
    let endif_count = source
        .lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            trimmed
                .strip_prefix('#')
                .is_some_and(|rest| rest.trim_start().starts_with("endif"))
        })
        .count();
    if endif_count == 0 {
        return None;
    }
    Some(GuardRename {
        path: canonical_or_lexical(path),
        current: ifndef.1.clone(),
        define_current: define.1.clone(),
        expected,
        ifndef_range: ifndef.2.clone(),
        define_range: define.2.clone(),
        header_digest: *blake3::hash(source.as_bytes()).as_bytes(),
    })
}

fn parse_guard_directive(line: &str) -> Option<(&str, &str, usize)> {
    let indent = line.len() - line.trim_start_matches([' ', '\t']).len();
    let after_hash = line.get(indent..)?.strip_prefix('#')?;
    let hash_gap = after_hash.len() - after_hash.trim_start_matches([' ', '\t']).len();
    let directive_start = indent + 1 + hash_gap;
    let tail = line.get(directive_start..)?;
    let (kind, after_kind) = if let Some(rest) = tail.strip_prefix("ifndef") {
        ("ifndef", rest)
    } else {
        ("define", tail.strip_prefix("define")?)
    };
    if after_kind
        .chars()
        .next()
        .is_none_or(|character| !character.is_ascii_whitespace())
    {
        return None;
    }
    let gap = after_kind.len() - after_kind.trim_start_matches([' ', '\t']).len();
    let name = after_kind.get(gap..)?.split_ascii_whitespace().next()?;
    if !is_identifier(name) {
        return None;
    }
    let name_start = directive_start + kind.len() + gap;
    Some((kind, name, name_start))
}

fn whole_file_guard_layout(source: &str, ifndef_guard: &str, define_guard: &str) -> bool {
    let code = normalized_code(source.as_bytes());
    let significant = String::from_utf8_lossy(&code)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let Some(first) = significant.first() else {
        return false;
    };
    let Some(second) = significant.get(1) else {
        return false;
    };
    let Some(last) = significant.last() else {
        return false;
    };
    parse_directive_name(first, "ifndef") == Some(ifndef_guard)
        && parse_directive_name(second, "define") == Some(define_guard)
        && last
            .strip_prefix('#')
            .map(str::trim_start)
            .is_some_and(|line| {
                line == "endif"
                    || line
                        .strip_prefix("endif")
                        .is_some_and(|rest| rest.chars().next().is_some_and(char::is_whitespace))
            })
}

fn parse_directive_name<'a>(line: &'a str, kind: &str) -> Option<&'a str> {
    line.strip_prefix('#')?
        .trim_start()
        .strip_prefix(kind)?
        .trim_start()
        .split_ascii_whitespace()
        .next()
}

fn expected_guard(filename: &str) -> String {
    filename
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

fn is_identifier(value: &str) -> bool {
    let mut bytes = value.bytes();
    bytes
        .next()
        .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn find_git_root(start: &Path) -> Option<PathBuf> {
    let mut current = canonical_or_lexical(start);
    loop {
        if current.join(".git").exists() {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
}

fn scan_project(root: &Path) -> Result<ProjectSnapshot, GuardPlanError> {
    let mut files = BTreeMap::new();
    let mut total_bytes = 0_u64;
    let mut builder = WalkBuilder::new(root);
    let filter_root = root.to_path_buf();
    builder
        .follow_links(false)
        .hidden(false)
        .ignore(false)
        .git_ignore(false)
        .git_global(false)
        .git_exclude(false)
        .sort_by_file_path(std::cmp::Ord::cmp)
        .filter_entry(move |entry| project_entry_allowed(&filter_root, entry));
    for entry in builder.build() {
        let entry = entry.map_err(|error| GuardPlanError::ProjectScan(error.to_string()))?;
        if entry.path_is_symlink() {
            return Err(GuardPlanError::ProjectScan(format!(
                "symbolic link encountered at `{}`",
                entry.path().display()
            )));
        }
        let Some(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_file() {
            continue;
        }
        if files.len() >= MAX_PROJECT_FILES {
            return Err(GuardPlanError::ProjectScan(format!(
                "project exceeds the {MAX_PROJECT_FILES}-file safety limit"
            )));
        }
        let bytes = fs::read(entry.path()).map_err(|error| {
            GuardPlanError::ProjectScan(format!("{}: {error}", entry.path().display()))
        })?;
        total_bytes = total_bytes.saturating_add(bytes.len() as u64);
        if total_bytes > MAX_PROJECT_BYTES {
            return Err(GuardPlanError::ProjectScan(format!(
                "project exceeds the {} MiB safety limit",
                MAX_PROJECT_BYTES / (1024 * 1024)
            )));
        }
        files.insert(
            canonical_or_lexical(entry.path()),
            *blake3::hash(&bytes).as_bytes(),
        );
    }
    Ok(ProjectSnapshot {
        root: canonical_or_lexical(root),
        files,
    })
}

fn project_entry_allowed(root: &Path, entry: &ignore::DirEntry) -> bool {
    let Ok(relative) = entry.path().strip_prefix(root) else {
        return false;
    };
    let components = relative
        .components()
        .map(std::path::Component::as_os_str)
        .collect::<Vec<_>>();
    if components
        .iter()
        .any(|component| *component == OsStr::new(".git"))
    {
        return false;
    }
    !components.windows(2).any(|pair| {
        matches!(
            pair,
            [first, second]
                if (*first == OsStr::new(".codex") || *first == OsStr::new(".claude"))
                    && *second == OsStr::new("worktrees")
        )
    })
}

fn read_snapshot_contents(
    snapshot: &ProjectSnapshot,
) -> Result<BTreeMap<PathBuf, Vec<u8>>, GuardPlanError> {
    snapshot
        .files
        .iter()
        .map(|(path, expected_digest)| {
            let metadata = fs::symlink_metadata(path).map_err(|error| {
                GuardPlanError::ProjectScan(format!("{}: {error}", path.display()))
            })?;
            if !metadata.is_file() || metadata.file_type().is_symlink() {
                return Err(GuardPlanError::ProjectScan(format!(
                    "snapshot path changed type at `{}`",
                    path.display()
                )));
            }
            let bytes = fs::read(path).map_err(|error| {
                GuardPlanError::ProjectScan(format!("{}: {error}", path.display()))
            })?;
            if blake3::hash(&bytes).as_bytes() != expected_digest {
                return Err(GuardPlanError::ProjectScan(format!(
                    "snapshot contents changed while planning at `{}`",
                    path.display()
                )));
            }
            Ok((path.clone(), bytes))
        })
        .collect()
}

fn duplicate_filename_guards<'a>(paths: impl Iterator<Item = &'a PathBuf>) -> BTreeSet<String> {
    let mut counts = BTreeMap::<String, usize>::new();
    for path in paths {
        if path.extension() == Some(OsStr::new("h")) {
            if let Some(name) = path.file_name().and_then(OsStr::to_str) {
                *counts.entry(expected_guard(name)).or_default() += 1;
            }
        }
    }
    counts
        .into_iter()
        .filter_map(|(guard, count)| (count > 1).then_some(guard))
        .collect()
}

fn project_identifier_occurrences(
    contents: &BTreeMap<PathBuf, Vec<u8>>,
    identifier: &str,
) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for (path, bytes) in contents {
        let code = normalized_code(bytes);
        for token in identifier_tokens(&code) {
            if token == identifier.as_bytes() {
                paths.push(path.clone());
            }
        }
    }
    paths
}

fn normalized_code(bytes: &[u8]) -> Vec<u8> {
    let masked = mask_comments_and_literals(bytes);
    let mut output = Vec::with_capacity(masked.len());
    let mut index = 0;
    while index < masked.len() {
        let standard = masked.get(index..index + 2) == Some(b"\\\n");
        let crlf = masked.get(index..index + 3) == Some(b"\\\r\n");
        let trigraph = masked.get(index..index + 4) == Some(b"??/\n");
        let trigraph_crlf = masked.get(index..index + 5) == Some(b"??/\r\n");
        let skipped = if trigraph_crlf {
            5
        } else if trigraph {
            4
        } else if crlf {
            3
        } else if standard {
            2
        } else {
            0
        };
        if skipped > 0 {
            index += skipped;
        } else {
            output.push(masked[index]);
            index += 1;
        }
    }
    output
}

fn identifier_tokens(code: &[u8]) -> impl Iterator<Item = &[u8]> {
    code.split(|byte| !byte.is_ascii_alphanumeric() && *byte != b'_')
        .filter(|token| {
            token
                .first()
                .is_some_and(|byte| byte.is_ascii_alphabetic() || *byte == b'_')
        })
}

fn mask_comments_and_literals(bytes: &[u8]) -> Vec<u8> {
    let mut output = bytes.to_vec();
    let mut index = 0;
    let mut state = b'c';
    while index < bytes.len() {
        let next = bytes.get(index + 1).copied();
        match state {
            b'c' if bytes[index] == b'/' && next == Some(b'/') => {
                output[index] = b' ';
                output[index + 1] = b' ';
                index += 2;
                state = b'l';
                continue;
            }
            b'c' if bytes[index] == b'/' && next == Some(b'*') => {
                output[index] = b' ';
                output[index + 1] = b' ';
                index += 2;
                state = b'b';
                continue;
            }
            b'c' if matches!(bytes[index], b'"' | b'\'') => {
                state = bytes[index];
                output[index] = b' ';
            }
            b'l' if matches!(bytes[index], b'\n' | b'\r') => state = b'c',
            b'b' if bytes[index] == b'*' && next == Some(b'/') => {
                output[index] = b' ';
                output[index + 1] = b' ';
                index += 2;
                state = b'c';
                continue;
            }
            b'"' | b'\'' if bytes[index] == b'\\' && next.is_some() => {
                output[index] = b' ';
                output[index + 1] = b' ';
                index += 2;
                continue;
            }
            quote @ (b'"' | b'\'') if bytes[index] == quote => {
                output[index] = b' ';
                state = b'c';
            }
            b'c' => {}
            _ => {
                if !matches!(bytes[index], b'\n' | b'\r') {
                    output[index] = b' ';
                }
            }
        }
        index += 1;
    }
    output
}

fn is_build_file(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    matches!(
        name.as_str(),
        "makefile"
            | "cmakelists.txt"
            | "meson.build"
            | "build.ninja"
            | "compile_commands.json"
            | "compile_flags.txt"
    ) || matches!(
        path.extension().and_then(OsStr::to_str),
        Some("mk" | "cmake" | "ninja")
    )
}

fn has_dynamic_build_definitions(bytes: &[u8]) -> bool {
    let code = mask_hash_comments(bytes);
    let lowercase = String::from_utf8_lossy(&code).to_ascii_lowercase();
    lowercase.contains("##")
        || lowercase.contains("%:%:")
        || lowercase.contains("target_compile_definitions")
        || lowercase.contains("add_compile_definitions")
        || lowercase.contains("add_definitions")
        || lowercase
            .split_ascii_whitespace()
            .any(|token| token.starts_with("-d") || token.starts_with("/d"))
}

fn mask_hash_comments(bytes: &[u8]) -> Vec<u8> {
    let mut output = bytes.to_vec();
    for line in output.split_mut(|byte| *byte == b'\n') {
        if let Some(index) = line.iter().position(|byte| *byte == b'#') {
            line[index..].fill(b' ');
        }
    }
    output
}

fn canonical_or_lexical(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::{
        guard_approval_is_current, guard_insertion_approval_is_current, plan_guard_insertions,
        plan_guard_renames,
    };

    fn git_project() -> TempDir {
        let project = TempDir::new().expect("project");
        fs::create_dir(project.path().join(".git")).expect("git marker");
        project
    }

    #[test]
    fn isolated_wrong_guard_is_approved_and_snapshot_bound() {
        let project = git_project();
        let header = project.path().join("sample.h");
        fs::write(
            &header,
            "#ifndef OLD_GUARD\n# define OLD_GUARD\n\nint\tx(void);\n\n#endif\n",
        )
        .expect("header");

        let approvals = plan_guard_renames(std::slice::from_ref(&header)).expect("complete scan");
        let approval = approvals
            .get(&header.canonicalize().expect("canonical"))
            .expect("approval");
        assert_eq!(approval.rename.current, "OLD_GUARD");
        assert_eq!(approval.rename.expected, "SAMPLE_H");
        assert!(guard_approval_is_current(approval));

        fs::write(project.path().join("new.txt"), "external change\n").expect("change");
        assert!(!guard_approval_is_current(approval));
    }

    #[test]
    fn reference_or_duplicate_filename_blocks_approval() {
        let project = git_project();
        let header = project.path().join("sample.h");
        fs::create_dir(project.path().join("vendor")).expect("vendor");
        fs::write(&header, "#ifndef OLD_GUARD\n# define OLD_GUARD\n#endif\n").expect("header");
        fs::write(project.path().join("use.c"), "#ifdef OLD_GUARD\n#endif\n").expect("reference");

        assert!(
            plan_guard_renames(std::slice::from_ref(&header))
                .expect("scan")
                .is_empty()
        );

        fs::remove_file(project.path().join("use.c")).expect("remove ref");
        fs::write(project.path().join("vendor/sample.h"), "\n").expect("duplicate");
        assert!(
            plan_guard_renames(std::slice::from_ref(&header))
                .expect("scan")
                .is_empty()
        );
    }

    #[test]
    fn mismatched_ifndef_and_define_are_approved_only_as_one_atomic_pair() {
        let project = git_project();
        let header = project.path().join("get_next_line.h");
        fs::write(
            &header,
            "#ifndef GET_NEXT_LINE_H\n# define GET_NEXT_LINE\n\nint\tgnl(void);\n\n#endif\n",
        )
        .expect("header");

        let approvals = plan_guard_renames(std::slice::from_ref(&header)).expect("complete scan");
        let approval = approvals
            .get(&header.canonicalize().expect("canonical"))
            .expect("approval");
        assert_eq!(approval.rename.current, "GET_NEXT_LINE_H");
        assert_eq!(approval.rename.define_current, "GET_NEXT_LINE");
        assert_eq!(approval.rename.expected, "GET_NEXT_LINE_H");
        assert!(guard_approval_is_current(approval));
    }

    #[test]
    fn token_pasting_build_configuration_fails_closed() {
        let project = git_project();
        let header = project.path().join("sample.h");
        fs::write(&header, "#ifndef OLD_GUARD\n# define OLD_GUARD\n#endif\n").expect("header");
        fs::write(project.path().join("Makefile"), "FLAGS = -DNAME=a##b\n").expect("makefile");

        let result = plan_guard_renames(&[header]).expect("scan");
        assert!(result.is_empty());
    }

    #[test]
    fn ordinary_unguarded_header_gets_a_snapshot_bound_insertion_plan() {
        let project = git_project();
        let header = project.path().join("sample.h");
        fs::write(&header, "int\tsample(void);\n").expect("header");

        let plans = plan_guard_insertions(std::slice::from_ref(&header)).expect("scan");
        let approval = plans
            .get(&header.canonicalize().expect("canonical"))
            .expect("insertion approval");
        assert_eq!(approval.insertion.expected, "SAMPLE_H");
        assert!(guard_insertion_approval_is_current(approval));

        fs::write(&header, "#pragma once\nint\tsample(void);\n").expect("change");
        assert!(!guard_insertion_approval_is_current(approval));
        assert!(plan_guard_insertions(&[header]).expect("scan").is_empty());
    }
}
