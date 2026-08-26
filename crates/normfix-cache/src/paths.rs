use std::fs;
use std::path::{Path, PathBuf};

use crate::CACHE_SCHEMA_VERSION;

/// External database location for one canonical project.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CachePaths {
    pub(super) database: PathBuf,
    pub(super) project_root: PathBuf,
}

impl CachePaths {
    /// Resolves the platform cache base and derives a project-specific path.
    ///
    /// On Unix, `$XDG_CACHE_HOME` is preferred and `$HOME/.cache` is the
    /// fallback. On Windows, `LOCALAPPDATA` is used.
    ///
    /// # Errors
    ///
    /// Returns [`CachePathError`] when no external cache base is available or
    /// the project root cannot be canonicalized.
    pub fn for_project(project_root: &Path) -> Result<Self, CachePathError> {
        let canonical =
            fs::canonicalize(project_root).map_err(|error| CachePathError::ProjectRoot {
                path: project_root.to_path_buf(),
                message: error.to_string(),
            })?;
        let base = platform_cache_base().ok_or(CachePathError::NoCacheDirectory)?;
        secure_cache_paths(&Self::with_base(base, &canonical))
    }

    /// Derives a project cache below an explicit external base.
    ///
    /// This constructor is useful for hermetic callers and tests. The caller is
    /// responsible for ensuring `base` is outside the analyzed project.
    #[must_use]
    pub fn with_base(base: impl Into<PathBuf>, canonical_project_root: &Path) -> Self {
        let project_id = blake3::hash(&native_path_bytes(canonical_project_root))
            .to_hex()
            .to_string();
        Self {
            database: base
                .into()
                .join("normfix")
                .join(project_id)
                .join(format!("cache-v{CACHE_SCHEMA_VERSION}.redb")),
            project_root: canonical_project_root.to_path_buf(),
        }
    }

    /// Returns the redb database path.
    #[must_use]
    pub fn database(&self) -> &Path {
        &self.database
    }
}

/// Failure to derive an external cache location.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CachePathError {
    /// The project root could not be canonicalized.
    ProjectRoot {
        /// Requested root.
        path: PathBuf,
        /// Operating-system detail.
        message: String,
    },
    /// No XDG/platform cache base was available.
    NoCacheDirectory,
    /// The resolved cache path overlaps the analyzed project or is unsafe.
    UnsafeCacheDirectory {
        /// Rejected cache path.
        path: PathBuf,
        /// Resolution or overlap detail.
        message: String,
    },
}

impl std::fmt::Display for CachePathError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ProjectRoot { path, message } => {
                write!(
                    formatter,
                    "could not resolve project root `{}`: {message}",
                    path.display()
                )
            }
            Self::NoCacheDirectory => {
                formatter.write_str("no external user cache directory is available")
            }
            Self::UnsafeCacheDirectory { path, message } => write!(
                formatter,
                "cache directory `{}` is not safely external: {message}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for CachePathError {}

pub(super) fn secure_cache_paths(paths: &CachePaths) -> Result<CachePaths, CachePathError> {
    let project_root =
        fs::canonicalize(&paths.project_root).map_err(|error| CachePathError::ProjectRoot {
            path: paths.project_root.clone(),
            message: error.to_string(),
        })?;
    let Some(parent) = paths.database.parent() else {
        return Err(CachePathError::UnsafeCacheDirectory {
            path: paths.database.clone(),
            message: "database path has no parent directory".to_owned(),
        });
    };
    let resolved_parent = resolve_path_for_creation(parent).map_err(|message| {
        CachePathError::UnsafeCacheDirectory {
            path: parent.to_path_buf(),
            message,
        }
    })?;
    if paths_overlap(&resolved_parent, &project_root) {
        return Err(CachePathError::UnsafeCacheDirectory {
            path: resolved_parent,
            message: format!("it overlaps project root `{}`", project_root.display()),
        });
    }
    let Some(file_name) = paths.database.file_name() else {
        return Err(CachePathError::UnsafeCacheDirectory {
            path: paths.database.clone(),
            message: "database path has no filename".to_owned(),
        });
    };
    Ok(CachePaths {
        database: resolved_parent.join(file_name),
        project_root,
    })
}

pub(super) fn paths_overlap(first: &Path, second: &Path) -> bool {
    first.starts_with(second) || second.starts_with(first)
}

fn resolve_path_for_creation(path: &Path) -> Result<PathBuf, String> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| format!("could not resolve the current directory: {error}"))?
            .join(path)
    };
    let mut existing = absolute.as_path();
    let mut suffix = Vec::new();
    loop {
        match fs::symlink_metadata(existing) {
            Ok(_) => break,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let Some(name) = existing.file_name() else {
                    return Err(format!("no existing ancestor for `{}`", absolute.display()));
                };
                suffix.push(name.to_os_string());
                existing = existing
                    .parent()
                    .ok_or_else(|| format!("no existing ancestor for `{}`", absolute.display()))?;
            }
            Err(error) => {
                return Err(format!(
                    "could not inspect `{}`: {error}",
                    existing.display()
                ));
            }
        }
    }
    let mut resolved = fs::canonicalize(existing)
        .map_err(|error| format!("could not canonicalize `{}`: {error}", existing.display()))?;
    for component in suffix.into_iter().rev() {
        if component == "." || component == ".." {
            return Err("cache paths cannot contain dot components".to_owned());
        }
        resolved.push(component);
    }
    Ok(resolved)
}

fn platform_cache_base() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
    {
        return Some(path);
    }
    #[cfg(windows)]
    {
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .map(|home| home.join(".cache"))
    }
}

#[cfg(unix)]
fn native_path_bytes(path: &Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;

    path.as_os_str().as_bytes().to_vec()
}

#[cfg(windows)]
fn native_path_bytes(path: &Path) -> Vec<u8> {
    use std::os::windows::ffi::OsStrExt;

    path.as_os_str()
        .encode_wide()
        .flat_map(u16::to_le_bytes)
        .collect()
}

#[cfg(not(any(unix, windows)))]
fn native_path_bytes(path: &Path) -> Vec<u8> {
    path.as_os_str().to_string_lossy().as_bytes().to_vec()
}
