//! Deterministic, read-only project discovery.
//!
//! Directory inputs are traversed without following symbolic links. C sources,
//! C headers, makefiles and README documents are returned as processable files,
//! while other regular files are reported separately as unexpected.
//!
//! Discovery never enters a `.git` directory. Git ignore rules can be enabled
//! for directory traversal through [`DiscoveryOptions::respect_gitignore`].
//! Explicit file inputs remain explicit and are therefore not filtered by
//! `.gitignore`.

#![forbid(unsafe_code)]

mod git_scope;
mod guard;
mod policy;

pub use git_scope::{GitScope, GitScopeError, GitScopeOptions, resolve_git_scope};
pub use guard::{
    GuardApproval, GuardInsertion, GuardInsertionApproval, GuardPlanError, GuardRename,
    ProjectSnapshot, guard_approval_is_current, guard_insertion_approval_is_current,
    plan_guard_insertions, plan_guard_renames,
};
pub use policy::{PolicyError, ProjectPolicy, load_project_policy};

use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use ignore::WalkBuilder;
use thiserror::Error;

/// Options controlling one discovery operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoveryOptions {
    /// Directory used to resolve relative inputs and as the zero-input target.
    pub cwd: PathBuf,
    /// Whether directory walks should honor `.gitignore` files.
    pub respect_gitignore: bool,
    /// Whether directory walks honor `.normfixignore` and its legacy alias.
    pub respect_normfixignore: bool,
}

impl DiscoveryOptions {
    /// Creates options rooted at `cwd`, with Git ignore handling disabled.
    #[must_use]
    pub fn new(cwd: impl Into<PathBuf>) -> Self {
        Self {
            cwd: cwd.into(),
            respect_gitignore: false,
            respect_normfixignore: true,
        }
    }

    /// Enables or disables `.gitignore` handling for directory walks.
    #[must_use]
    pub const fn with_respect_gitignore(mut self, respect_gitignore: bool) -> Self {
        self.respect_gitignore = respect_gitignore;
        self
    }

    /// Enables or disables project-local `.normfixignore` handling.
    ///
    /// The legacy `.norminetteignore` filename follows the same switch so a
    /// closed-worktree proof can disable both sources of hidden inputs.
    #[must_use]
    pub const fn with_respect_normfixignore(mut self, respect_normfixignore: bool) -> Self {
        self.respect_normfixignore = respect_normfixignore;
        self
    }
}

/// A processable project file.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct DiscoveredFile {
    /// Absolute, lexically normalized path.
    pub path: PathBuf,
    /// Kind inferred from the file name.
    pub kind: ProjectFileKind,
}

/// Kinds of files handled by the first native migration slice.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ProjectFileKind {
    /// A file whose extension is exactly `.c`.
    CSource,
    /// A file whose extension is exactly `.h`.
    CHeader,
    /// A file named `Makefile`, compared case-insensitively.
    Makefile,
    /// A README document, with or without an extension.
    Markdown,
}

impl ProjectFileKind {
    /// Classifies `path` as a processable project file.
    #[must_use]
    pub fn from_path(path: &Path) -> Option<Self> {
        if path.extension() == Some(OsStr::new("c")) {
            return Some(Self::CSource);
        }
        if path.extension() == Some(OsStr::new("h")) {
            return Some(Self::CHeader);
        }
        let name = path.file_name().and_then(OsStr::to_str)?;
        if name.eq_ignore_ascii_case("makefile") {
            return Some(Self::Makefile);
        }
        let lowercase = name.to_ascii_lowercase();
        (lowercase == "readme" || lowercase.starts_with("readme.")).then_some(Self::Markdown)
    }
}

/// Complete output from a discovery operation.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DiscoveryResult {
    /// Supported files, deduplicated and sorted by absolute path.
    pub processable_files: Vec<DiscoveredFile>,
    /// Unsupported regular files found while recursively scanning directories.
    pub unexpected_files: Vec<PathBuf>,
    /// Non-fatal discovery errors, sorted into a stable order.
    pub errors: Vec<DiscoveryError>,
}

/// A non-fatal error associated with an input or walk entry.
#[derive(Clone, Debug, Eq, Error, Ord, PartialEq, PartialOrd)]
#[error("could not discover `{input}` at `{path}`: {kind}")]
pub struct DiscoveryError {
    /// Resolved input that started the failed operation.
    pub input: PathBuf,
    /// Most specific path available for the failure.
    pub path: PathBuf,
    /// Machine-readable category and category-specific context.
    pub kind: DiscoveryErrorKind,
}

/// Machine-readable discovery failure categories.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DiscoveryErrorKind {
    /// The requested input does not exist.
    InputNotFound,
    /// The explicit path traverses or names a symbolic link.
    SymbolicLinkTraversal {
        /// First symbolic-link component found in the explicit path.
        component: PathBuf,
    },
    /// The explicit file is neither C, a header nor a makefile.
    UnsupportedExplicitFile,
    /// The input exists but is not a regular file or directory.
    UnsupportedFileType,
    /// Filesystem metadata could not be read.
    Metadata {
        /// Filesystem operation that failed.
        operation: &'static str,
        /// Operating-system error text.
        message: String,
    },
    /// The recursive directory walker could not inspect an entry.
    Walk {
        /// Error text reported by the walker.
        message: String,
    },
    /// The `.git` metadata directory was supplied as an explicit input.
    GitMetadataInput,
}

impl fmt::Display for DiscoveryErrorKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InputNotFound => formatter.write_str("input does not exist"),
            Self::SymbolicLinkTraversal { component } => {
                write!(
                    formatter,
                    "path traverses symbolic link `{}`",
                    component.display()
                )
            }
            Self::UnsupportedExplicitFile => {
                formatter.write_str("explicit file is not a .c, .h or Makefile")
            }
            Self::UnsupportedFileType => {
                formatter.write_str("input is not a regular file or directory")
            }
            Self::Metadata { operation, message } => {
                write!(formatter, "{operation} failed: {message}")
            }
            Self::Walk { message } => write!(formatter, "directory walk failed: {message}"),
            Self::GitMetadataInput => {
                formatter.write_str("explicit .git metadata inputs are never traversed")
            }
        }
    }
}

/// Discovers supported and unexpected files below zero or more inputs.
///
/// An empty `inputs` slice scans [`DiscoveryOptions::cwd`]. Multiple and
/// overlapping inputs are accepted; results are deduplicated. Every returned
/// path is absolute and ordered lexicographically. Discovery is read-only.
#[must_use]
pub fn discover(inputs: &[PathBuf], options: &DiscoveryOptions) -> DiscoveryResult {
    let mut processable = BTreeSet::new();
    let mut unexpected = BTreeSet::new();
    let mut errors = BTreeSet::new();
    let requested = if inputs.is_empty() {
        vec![PathBuf::from(".")]
    } else {
        inputs.to_vec()
    };
    let cwd = make_absolute(&options.cwd, None);
    let roots = requested
        .into_iter()
        .map(|input| make_absolute(&input, Some(&cwd)))
        .collect::<BTreeSet<_>>();

    for root in roots {
        discover_root(
            &root,
            options,
            &mut processable,
            &mut unexpected,
            &mut errors,
        );
    }

    DiscoveryResult {
        processable_files: processable.into_iter().collect(),
        unexpected_files: unexpected.into_iter().collect(),
        errors: errors.into_iter().collect(),
    }
}

fn discover_root(
    requested_root: &Path,
    options: &DiscoveryOptions,
    processable: &mut BTreeSet<DiscoveredFile>,
    unexpected: &mut BTreeSet<PathBuf>,
    errors: &mut BTreeSet<DiscoveryError>,
) {
    if contains_git_metadata_component(requested_root) {
        errors.insert(error(
            requested_root,
            requested_root,
            DiscoveryErrorKind::GitMetadataInput,
        ));
        return;
    }

    match first_symlink_component(requested_root) {
        Ok(Some(component)) => {
            errors.insert(error(
                requested_root,
                requested_root,
                DiscoveryErrorKind::SymbolicLinkTraversal { component },
            ));
            return;
        }
        Ok(None) => {}
        Err(source) => {
            errors.insert(error(
                requested_root,
                requested_root,
                DiscoveryErrorKind::Metadata {
                    operation: "symlink metadata",
                    message: source.to_string(),
                },
            ));
            return;
        }
    }

    // Inspect the path before normalization. Besides preserving the symlink
    // proof above, this ensures `missing/../file.c` remains a missing input
    // instead of being silently reinterpreted as `file.c`.
    let metadata = match fs::symlink_metadata(requested_root) {
        Ok(metadata) => metadata,
        Err(source) if source.kind() == io::ErrorKind::NotFound => {
            errors.insert(error(
                requested_root,
                requested_root,
                DiscoveryErrorKind::InputNotFound,
            ));
            return;
        }
        Err(source) => {
            errors.insert(error(
                requested_root,
                requested_root,
                DiscoveryErrorKind::Metadata {
                    operation: "input metadata",
                    message: source.to_string(),
                },
            ));
            return;
        }
    };
    // Successful metadata and the absence of symlinks prove that lexical
    // normalization identifies the same filesystem object.
    let root = lexical_normalize(requested_root);

    if metadata.is_file() {
        match ProjectFileKind::from_path(&root) {
            Some(kind) => {
                processable.insert(DiscoveredFile {
                    path: root.clone(),
                    kind,
                });
            }
            None => {
                errors.insert(error(
                    requested_root,
                    &root,
                    DiscoveryErrorKind::UnsupportedExplicitFile,
                ));
            }
        }
        return;
    }
    if !metadata.is_dir() {
        errors.insert(error(
            requested_root,
            &root,
            DiscoveryErrorKind::UnsupportedFileType,
        ));
        return;
    }

    walk_directory(&root, options, processable, unexpected, errors);
}

fn walk_directory(
    root: &Path,
    options: &DiscoveryOptions,
    processable: &mut BTreeSet<DiscoveredFile>,
    unexpected: &mut BTreeSet<PathBuf>,
    errors: &mut BTreeSet<DiscoveryError>,
) {
    let mut builder = WalkBuilder::new(root);
    builder
        .follow_links(false)
        .hidden(false)
        .ignore(false)
        .git_ignore(options.respect_gitignore)
        .git_global(false)
        .git_exclude(false)
        .parents(options.respect_gitignore)
        .require_git(false)
        .sort_by_file_path(std::cmp::Ord::cmp)
        .filter_entry(walk_entry_is_allowed);
    if options.respect_normfixignore {
        builder.add_custom_ignore_filename(".normfixignore");
        builder.add_custom_ignore_filename(".norminetteignore");
    }

    for entry in builder.build() {
        let entry = match entry {
            Ok(entry) => entry,
            Err(source) => {
                let path = walk_error_path(&source).unwrap_or(root).to_path_buf();
                errors.insert(error(
                    root,
                    &path,
                    DiscoveryErrorKind::Walk {
                        message: source.to_string(),
                    },
                ));
                continue;
            }
        };
        if let Some(source) = entry.error() {
            errors.insert(error(
                root,
                entry.path(),
                DiscoveryErrorKind::Walk {
                    message: source.to_string(),
                },
            ));
        }
        let Some(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_file() {
            continue;
        }
        let path = lexical_normalize(entry.path());
        if let Some(kind) = ProjectFileKind::from_path(&path) {
            processable.insert(DiscoveredFile { path, kind });
        } else if !is_project_control_file(&path) {
            unexpected.insert(path);
        }
    }
}

/// Returns whether `path` is normfix project metadata rather than a source or
/// an unexpected deliverable.
#[must_use]
pub fn is_project_control_file(path: &Path) -> bool {
    matches!(
        path.file_name(),
        Some(name) if name == OsStr::new(".normfixignore")
            || name == OsStr::new(".norminetteignore")
            || name == OsStr::new("normfix.toml")
    )
}

fn walk_entry_is_allowed(entry: &ignore::DirEntry) -> bool {
    if entry.path_is_symlink() || entry.file_name() == OsStr::new(".git") {
        return false;
    }
    if entry.depth() > 0
        && entry.file_type().is_some_and(|kind| kind.is_dir())
        && (matches!(entry.file_name().to_str(), Some(".claude" | ".codex"))
            || nested_git_repository(entry.path()))
    {
        return false;
    }
    true
}

fn nested_git_repository(path: &Path) -> bool {
    fs::symlink_metadata(path.join(".git")).is_ok()
}

fn walk_error_path(error: &ignore::Error) -> Option<&Path> {
    match error {
        ignore::Error::WithPath { path, .. } => Some(path),
        ignore::Error::Loop { child, .. } => Some(child),
        ignore::Error::Partial(errors) => errors.iter().find_map(walk_error_path),
        ignore::Error::WithLineNumber { err, .. } | ignore::Error::WithDepth { err, .. } => {
            walk_error_path(err)
        }
        _ => None,
    }
}

fn contains_git_metadata_component(path: &Path) -> bool {
    path.components()
        .any(|component| component.as_os_str() == OsStr::new(".git"))
}

fn first_symlink_component(path: &Path) -> io::Result<Option<PathBuf>> {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                // macOS and a few Unix layouts expose `/var`, `/tmp` or `/home`
                // through a root-level compatibility link. Trust only that
                // filesystem prefix; links below it remain forbidden.
                if current.parent() != Some(Path::new(std::path::MAIN_SEPARATOR_STR)) {
                    return Ok(Some(current));
                }
            }
            Ok(_) => {}
            Err(source) if source.kind() == io::ErrorKind::NotFound => {}
            Err(source) => return Err(source),
        }
    }
    Ok(None)
}

fn make_absolute(path: &Path, base: Option<&Path>) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else if let Some(base) = base {
        base.join(path)
    } else {
        match std::env::current_dir() {
            Ok(current) => current.join(path),
            Err(_) => path.to_path_buf(),
        }
    }
}

fn lexical_normalize(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => match normalized.components().next_back() {
                Some(Component::Normal(_)) => {
                    normalized.pop();
                }
                Some(Component::ParentDir) | None => normalized.push(".."),
                Some(Component::Prefix(_) | Component::RootDir | Component::CurDir) => {}
            },
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                normalized.push(component.as_os_str());
            }
        }
    }
    normalized
}

fn error(input: &Path, path: &Path, kind: DiscoveryErrorKind) -> DiscoveryError {
    DiscoveryError {
        input: input.to_path_buf(),
        path: path.to_path_buf(),
        kind,
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use tempfile::TempDir;

    use super::{DiscoveredFile, DiscoveryErrorKind, DiscoveryOptions, ProjectFileKind, discover};

    fn options(directory: &Path) -> DiscoveryOptions {
        DiscoveryOptions::new(directory)
    }

    fn paths(files: &[DiscoveredFile]) -> Vec<PathBuf> {
        files.iter().map(|file| file.path.clone()).collect()
    }

    #[test]
    fn zero_inputs_recurses_deduplicates_and_sorts_paths() {
        let temporary = TempDir::new().expect("temporary directory");
        let root = temporary.path();
        fs::create_dir(root.join("src")).expect("src directory");
        fs::create_dir(root.join("include")).expect("include directory");
        fs::write(root.join("src/z.c"), "").expect("C source");
        fs::write(root.join("include/a.h"), "").expect("C header");
        fs::write(root.join("Makefile"), "all:\n").expect("makefile");
        fs::write(root.join("README.md"), "# Project\n").expect("readme");

        let result = discover(&[], &options(root));

        assert_eq!(
            paths(&result.processable_files),
            vec![
                root.join("Makefile"),
                root.join("README.md"),
                root.join("include/a.h"),
                root.join("src/z.c"),
            ]
        );
        assert!(result.unexpected_files.is_empty());
        assert!(result.errors.is_empty());
    }

    #[test]
    fn accepts_multiple_overlapping_file_and_directory_inputs() {
        let temporary = TempDir::new().expect("temporary directory");
        let root = temporary.path();
        fs::create_dir(root.join("lib")).expect("lib directory");
        let source = root.join("main.c");
        let header = root.join("lib/lib.h");
        fs::write(&source, "").expect("C source");
        fs::write(&header, "").expect("C header");

        let result = discover(
            &[
                source.clone(),
                root.join("lib"),
                source.clone(),
                root.to_path_buf(),
            ],
            &options(root),
        );

        assert_eq!(
            paths(&result.processable_files),
            vec![header, source]
                .into_iter()
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>()
        );
        assert!(result.errors.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn neither_walks_symlinks_nor_accepts_an_explicit_symlink_escape() {
        use std::os::unix::fs::symlink;

        let temporary = TempDir::new().expect("temporary directory");
        let root = temporary.path();
        let project = root.join("project");
        let outside = root.join("outside");
        fs::create_dir(&project).expect("project directory");
        fs::create_dir(&outside).expect("outside directory");
        let victim = outside.join("victim.c");
        fs::write(&victim, "int victim(void);\n").expect("outside source");
        let link = project.join("escape");
        symlink(&outside, &link).expect("directory symlink");

        let walked = discover(std::slice::from_ref(&project), &options(&project));
        assert!(walked.processable_files.is_empty());
        assert!(walked.unexpected_files.is_empty());
        assert!(walked.errors.is_empty());

        let explicit = discover(&[link.join("victim.c")], &options(&project));
        assert!(explicit.processable_files.is_empty());
        assert!(matches!(
            explicit.errors.as_slice(),
            [error]
                if error.kind
                    == DiscoveryErrorKind::SymbolicLinkTraversal {
                        component: link
                    }
        ));
    }

    #[test]
    fn gitignore_is_optional_and_prunes_the_directory_walk() {
        let temporary = TempDir::new().expect("temporary directory");
        let root = temporary.path();
        let kept = root.join("kept.c");
        let ignored = root.join("ignored.c");
        fs::write(&kept, "").expect("kept source");
        fs::write(&ignored, "").expect("ignored source");
        fs::write(root.join(".gitignore"), "ignored.c\n").expect("gitignore");

        let without_ignore = discover(&[], &options(root));
        let with_ignore = discover(&[], &options(root).with_respect_gitignore(true));

        assert_eq!(
            paths(&without_ignore.processable_files),
            vec![ignored, kept.clone()]
        );
        assert_eq!(paths(&with_ignore.processable_files), vec![kept]);
    }

    #[test]
    fn project_ignore_is_default_and_can_be_disabled_for_closed_world_proofs() {
        let temporary = TempDir::new().expect("temporary directory");
        let root = temporary.path();
        let kept = root.join("kept.c");
        let ignored = root.join("generated.c");
        fs::write(&kept, "").expect("kept source");
        fs::write(&ignored, "").expect("ignored source");
        fs::write(root.join(".normfixignore"), "generated.c\n").expect("project ignore");

        let normal = discover(&[], &options(root));
        let closed = discover(&[], &options(root).with_respect_normfixignore(false));

        assert_eq!(paths(&normal.processable_files), vec![kept.clone()]);
        assert_eq!(paths(&closed.processable_files), vec![ignored, kept]);
        assert!(normal.unexpected_files.is_empty());
    }

    #[test]
    fn legacy_project_ignore_remains_supported_and_is_not_unexpected() {
        let temporary = TempDir::new().expect("temporary directory");
        let root = temporary.path();
        let kept = root.join("kept.c");
        let ignored = root.join("generated.c");
        fs::write(&kept, "").expect("kept source");
        fs::write(&ignored, "").expect("ignored source");
        fs::write(root.join(".norminetteignore"), "generated.c\n").expect("legacy ignore");

        let normal = discover(&[], &options(root));
        let closed = discover(&[], &options(root).with_respect_normfixignore(false));

        assert_eq!(paths(&normal.processable_files), vec![kept.clone()]);
        assert_eq!(paths(&closed.processable_files), vec![ignored, kept]);
        assert!(normal.unexpected_files.is_empty());
    }

    #[test]
    fn reports_dotfiles_as_unexpected_but_not_readmes() {
        let temporary = TempDir::new().expect("temporary directory");
        let root = temporary.path();
        let finder_metadata = root.join(".DS_Store");
        let gitignore = root.join(".gitignore");
        fs::write(&finder_metadata, "metadata").expect("finder metadata");
        fs::write(&gitignore, "*.o\n").expect("gitignore");
        fs::write(root.join("README"), "Project\n").expect("plain readme");
        fs::write(root.join("README.en.md"), "Project\n").expect("localized readme");

        let result = discover(&[], &options(root));

        assert_eq!(
            result.unexpected_files,
            vec![finder_metadata, gitignore]
                .into_iter()
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>()
        );
        assert!(result.errors.is_empty());
    }

    #[test]
    fn never_enters_git_metadata() {
        let temporary = TempDir::new().expect("temporary directory");
        let root = temporary.path();
        fs::create_dir(root.join(".git")).expect("git metadata directory");
        fs::write(root.join(".git/secret.c"), "").expect("git object-like file");
        fs::write(root.join("visible.c"), "").expect("visible source");

        let result = discover(&[], &options(root));

        assert_eq!(
            result.processable_files,
            vec![DiscoveredFile {
                path: root.join("visible.c"),
                kind: ProjectFileKind::CSource,
            }]
        );
        assert!(result.unexpected_files.is_empty());
        assert!(result.errors.is_empty());
    }

    #[test]
    fn never_enters_nested_tool_or_git_worktrees() {
        let temporary = TempDir::new().expect("temporary directory");
        let root = temporary.path();
        let claude = root.join(".claude/worktrees/review");
        let codex = root.join(".codex/worktrees/review");
        let nested = root.join("vendor/nested-repository");
        let clone = root.join("vendor/nested-clone");
        fs::create_dir_all(&claude).expect("Claude worktree");
        fs::create_dir_all(&codex).expect("Codex worktree");
        fs::create_dir_all(&nested).expect("nested repository");
        fs::create_dir_all(clone.join(".git")).expect("nested Git metadata");
        fs::write(claude.join("copy.c"), "").expect("Claude source copy");
        fs::write(codex.join("copy.c"), "").expect("Codex source copy");
        fs::write(nested.join(".git"), "gitdir: elsewhere\n").expect("worktree marker");
        fs::write(nested.join("copy.c"), "").expect("nested source copy");
        fs::write(clone.join("copy.c"), "").expect("nested clone source copy");
        fs::write(root.join("visible.c"), "").expect("visible source");

        let result = discover(&[], &options(root));

        assert_eq!(
            paths(&result.processable_files),
            vec![root.join("visible.c")]
        );
        assert!(result.unexpected_files.is_empty());
        assert!(result.errors.is_empty());
    }

    #[test]
    fn unsupported_explicit_file_is_a_structured_error() {
        let temporary = TempDir::new().expect("temporary directory");
        let root = temporary.path();
        let notes = root.join("notes.txt");
        fs::write(&notes, "notes\n").expect("notes");

        let result = discover(std::slice::from_ref(&notes), &options(root));

        assert!(result.processable_files.is_empty());
        assert!(result.unexpected_files.is_empty());
        assert!(matches!(
            result.errors.as_slice(),
            [error]
                if error.path == notes
                    && error.kind == DiscoveryErrorKind::UnsupportedExplicitFile
        ));
    }
}
