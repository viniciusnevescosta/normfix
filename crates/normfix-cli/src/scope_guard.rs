//! Early refusal for accidentally broad or operating-system-sensitive scopes.

use std::env;
use std::path::{Component, Path, PathBuf};

// Descendants of these locations are operating-system managed rather than
// ordinary project roots. `/var` and temporary/storage roots are handled
// separately as exact broad scopes so real projects below them still work.
const SYSTEM_TREES: &[&str] = &[
    "/Applications",
    "/Library",
    "/System",
    "/bin",
    "/boot",
    "/dev",
    "/etc",
    "/lib",
    "/lib64",
    "/private/etc",
    "/private/var/db",
    "/private/var/root",
    "/private/var/vm",
    "/proc",
    "/root",
    "/run",
    "/sbin",
    "/sys",
    "/usr",
];

const BROAD_ROOTS: &[&str] = &[
    "/Users",
    "/Volumes",
    "/home",
    "/media",
    "/mnt",
    "/opt",
    "/private",
    "/private/tmp",
    "/private/var",
    "/srv",
    "/tmp",
    "/var",
];

/// One effective scope that requires an explicit `--force` acknowledgement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SensitiveScope {
    /// Path as selected by the invocation.
    pub requested: PathBuf,
    /// Canonical or lexically normalized path used for the decision.
    pub resolved: PathBuf,
    /// Human-readable reason for the protection.
    pub reason: &'static str,
}

/// Returns the first protected path selected by this invocation.
#[must_use]
pub fn sensitive_scope(cwd: &Path, paths: &[PathBuf], git_scoped: bool) -> Option<SensitiveScope> {
    let homes = [env::var_os("HOME"), env::var_os("USERPROFILE")]
        .into_iter()
        .flatten()
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    sensitive_scope_with_homes(cwd, paths, git_scoped, &homes)
}

fn sensitive_scope_with_homes(
    cwd: &Path,
    paths: &[PathBuf],
    git_scoped: bool,
    homes: &[PathBuf],
) -> Option<SensitiveScope> {
    let selected = if git_scoped || paths.is_empty() {
        vec![cwd.to_path_buf()]
    } else {
        paths.to_vec()
    };
    let normalized_homes = homes
        .iter()
        .map(|home| resolve(cwd, home))
        .collect::<Vec<_>>();
    selected.into_iter().find_map(|requested| {
        let resolved = resolve(cwd, &requested);
        protected_reason(&resolved, &normalized_homes).map(|reason| SensitiveScope {
            requested,
            resolved,
            reason,
        })
    })
}

fn resolve(cwd: &Path, path: &Path) -> PathBuf {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    };
    let absolute = normalize_lexically(&absolute);
    let mut ancestor = absolute.clone();
    let mut suffix = Vec::new();
    loop {
        if let Ok(mut resolved) = ancestor.canonicalize() {
            for component in suffix.iter().rev() {
                resolved.push(component);
            }
            return normalize_lexically(&resolved);
        }
        let Some(name) = ancestor.file_name().map(ToOwned::to_owned) else {
            return absolute;
        };
        suffix.push(name);
        if !ancestor.pop() {
            return absolute;
        }
    }
}

fn normalize_lexically(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                normalized.push(component.as_os_str());
            }
            Component::CurDir => {}
            Component::ParentDir => {
                let _ = normalized.pop();
            }
        }
    }
    normalized
}

fn protected_reason(path: &Path, homes: &[PathBuf]) -> Option<&'static str> {
    if path.parent().is_none() {
        return Some("it is a filesystem root");
    }
    if homes.iter().any(|home| path == home) {
        return Some("it is the complete user home directory");
    }

    if SYSTEM_TREES
        .iter()
        .map(Path::new)
        .any(|root| path == root || path.starts_with(root))
        || (cfg!(target_os = "linux") && path.starts_with("/var"))
    {
        return Some("it is inside an operating-system-managed directory");
    }

    BROAD_ROOTS
        .iter()
        .map(Path::new)
        .any(|root| path == root)
        .then_some("it is a broad system or multi-project directory")
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    #[cfg(unix)]
    use std::os::unix::fs::symlink;

    #[cfg(unix)]
    use tempfile::TempDir;

    use super::sensitive_scope_with_homes;

    #[test]
    fn protects_filesystem_system_and_home_scopes() {
        let cwd = Path::new("/work/project");
        let homes = vec![PathBuf::from("/Users/student")];

        assert!(sensitive_scope_with_homes(cwd, &[PathBuf::from("/")], false, &homes).is_some());
        assert!(
            sensitive_scope_with_homes(cwd, &[PathBuf::from("/Users")], false, &homes).is_some()
        );
        assert!(
            sensitive_scope_with_homes(cwd, &[PathBuf::from("/home")], false, &homes).is_some()
        );
        assert!(
            sensitive_scope_with_homes(cwd, &[PathBuf::from("/etc/normfix.c")], false, &homes)
                .is_some()
        );
        assert!(
            sensitive_scope_with_homes(cwd, &[PathBuf::from("/Users/student")], false, &homes,)
                .is_some()
        );
    }

    #[test]
    fn permits_normal_projects_and_temporary_subdirectories() {
        let cwd = Path::new("/Users/student/projects/normfix");
        let homes = vec![PathBuf::from("/Users/student")];

        assert!(sensitive_scope_with_homes(cwd, &[], false, &homes).is_none());
        assert!(
            sensitive_scope_with_homes(cwd, &[PathBuf::from("/tmp/normfix-test")], false, &homes)
                .is_none()
        );
        assert!(
            sensitive_scope_with_homes(
                cwd,
                &[PathBuf::from("/Users/student/main.c")],
                false,
                &homes,
            )
            .is_none()
        );
    }

    #[test]
    fn git_scope_uses_the_repository_root_not_thousands_of_selected_files() {
        let cwd = Path::new("/Users/student");
        let homes = vec![PathBuf::from("/Users/student")];

        let risk =
            sensitive_scope_with_homes(cwd, &[PathBuf::from("project/main.c")], true, &homes)
                .expect("home-scoped Git run must be protected");
        assert_eq!(risk.resolved, PathBuf::from("/Users/student"));
    }

    #[test]
    fn lexical_parent_components_cannot_bypass_a_protected_tree() {
        let cwd = Path::new("/work/project");
        let risk =
            sensitive_scope_with_homes(cwd, &[PathBuf::from("/work/../etc/normfix.c")], false, &[]);
        assert!(risk.is_some());
    }

    #[cfg(unix)]
    #[test]
    fn symbolic_links_are_resolved_before_the_scope_decision() {
        let temporary = TempDir::new().expect("temporary directory");
        let link = temporary.path().join("system");
        let root_link = temporary.path().join("root");
        symlink("/etc", &link).expect("system link");
        symlink("/", &root_link).expect("root link");

        let risk = sensitive_scope_with_homes(
            temporary.path(),
            &[link.join("normfix-does-not-exist.c")],
            false,
            &[],
        );

        assert!(risk.is_some());
        assert!(sensitive_scope_with_homes(temporary.path(), &[root_link], false, &[]).is_some());
    }
}
