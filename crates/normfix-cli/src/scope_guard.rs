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

/// Why one scope is protected.
///
/// This is a discriminant rather than a sentence so the refusal can be stated
/// in the reader's language without the guard owning any prose.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScopeReason {
    /// The path is a filesystem root.
    FilesystemRoot,
    /// The path is a complete user home directory.
    HomeDirectory,
    /// The path lies inside an operating-system-managed tree.
    OperatingSystemTree,
    /// The path is a broad system or multi-project directory.
    BroadDirectory,
}

impl ScopeReason {
    /// Returns the translated explanation for this protection.
    #[must_use]
    pub const fn describe(self, messages: &normfix_i18n::Messages) -> &'static str {
        match self {
            Self::FilesystemRoot => messages.scope_reason_filesystem_root,
            Self::HomeDirectory => messages.scope_reason_home_directory,
            Self::OperatingSystemTree => messages.scope_reason_system_tree,
            Self::BroadDirectory => messages.scope_reason_broad_directory,
        }
    }
}

/// One effective scope that requires an explicit `--force` acknowledgement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SensitiveScope {
    /// Path as selected by the invocation.
    pub requested: PathBuf,
    /// Canonical or lexically normalized path used for the decision.
    pub resolved: PathBuf,
    /// Why the protection applies.
    pub reason: ScopeReason,
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

/// The form a path is compared in.
///
/// Windows decides two things differently, and both would silently defeat this
/// guard: its paths are case-insensitive, so `C:\\WINDOWS` and `C:\\Windows` are
/// the same directory, and `canonicalize` returns the verbatim `\\\\?\\` form,
/// which does not compare equal to the path a reader typed or that the
/// environment reported. Every comparison below goes through this, so a
/// protected tree is recognized however it was spelled.
fn comparable(path: &Path) -> PathBuf {
    if cfg!(windows) {
        let text = path.to_string_lossy();
        let plain = text.strip_prefix(r"\\?\").unwrap_or(&text);
        PathBuf::from(plain.to_lowercase())
    } else {
        path.to_path_buf()
    }
}

/// Operating-system trees on Windows, read from the environment.
///
/// Hardcoding `C:` would protect the wrong disk: Windows can be installed on
/// any drive, and a machine with the system on `D:` would get a guard that
/// silently protects nothing. These variables are set by the system itself.
#[cfg(windows)]
fn platform_system_trees() -> Vec<PathBuf> {
    [
        "SystemRoot",
        "windir",
        "ProgramFiles",
        "ProgramFiles(x86)",
        "ProgramW6432",
        "ProgramData",
    ]
    .iter()
    .filter_map(|name| env::var_os(name))
    .map(PathBuf::from)
    .collect()
}

/// Broad multi-project directories on Windows, likewise read from the system.
///
/// The directory that holds every user's profile is the Windows counterpart of
/// `/home`: a run started there would walk every account on the machine.
#[cfg(windows)]
fn platform_broad_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(profiles) = env::var_os("PUBLIC").map(PathBuf::from) {
        if let Some(parent) = profiles.parent() {
            roots.push(parent.to_path_buf());
        }
        roots.push(profiles);
    }
    for name in ["TEMP", "TMP"] {
        if let Some(value) = env::var_os(name) {
            roots.push(PathBuf::from(value));
        }
    }
    roots
}

#[cfg(not(windows))]
fn platform_system_trees() -> Vec<PathBuf> {
    Vec::new()
}

#[cfg(not(windows))]
fn platform_broad_roots() -> Vec<PathBuf> {
    Vec::new()
}

fn protected_reason(path: &Path, homes: &[PathBuf]) -> Option<ScopeReason> {
    if path.parent().is_none() {
        return Some(ScopeReason::FilesystemRoot);
    }
    let subject = comparable(path);
    if homes.iter().any(|home| subject == comparable(home)) {
        return Some(ScopeReason::HomeDirectory);
    }

    let system_trees = SYSTEM_TREES
        .iter()
        .map(PathBuf::from)
        .chain(platform_system_trees())
        .collect::<Vec<_>>();
    let broad_roots = BROAD_ROOTS
        .iter()
        .map(PathBuf::from)
        .chain(platform_broad_roots())
        .collect::<Vec<_>>();

    if system_trees.iter().any(|root| {
        let root = comparable(root);
        subject == root || subject.starts_with(&root)
    }) || (cfg!(target_os = "linux") && subject.starts_with("/var"))
    {
        return Some(ScopeReason::OperatingSystemTree);
    }

    broad_roots
        .iter()
        .any(|root| subject == comparable(root))
        .then_some(ScopeReason::BroadDirectory)
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    #[cfg(unix)]
    use std::os::unix::fs::symlink;

    #[cfg(unix)]
    use tempfile::TempDir;

    use super::sensitive_scope_with_homes;

    /// A machine's shapes, so each platform is tested against paths it really
    /// has. Asserting Unix literals on Windows proves nothing about either.
    struct Fixture {
        root: &'static str,
        home: &'static str,
        profiles: &'static str,
        system_file: &'static str,
        escaping: &'static str,
        project_cwd: &'static str,
        project_file: &'static str,
    }

    #[cfg(unix)]
    const FIXTURE: Fixture = Fixture {
        root: "/",
        home: "/Users/student",
        profiles: "/Users",
        system_file: "/etc/normfix.c",
        escaping: "/work/../etc/normfix.c",
        project_cwd: "/Users/student/projects/normfix",
        project_file: "/Users/student/main.c",
    };

    #[cfg(windows)]
    const FIXTURE: Fixture = Fixture {
        root: r"C:\",
        home: r"C:\Users\student",
        profiles: r"C:\Users",
        system_file: r"C:\Windows\System32\normfix.c",
        escaping: r"C:\work\..\Windows\normfix.c",
        project_cwd: r"C:\Users\student\projects\normfix",
        project_file: r"C:\Users\student\main.c",
    };

    /// The environment the production guard reads, restated for the test.
    ///
    /// `protected_reason` derives the Windows trees from the system's own
    /// variables, so a test that does not set them would be asserting against
    /// whatever the runner happens to have.
    #[cfg(windows)]
    fn with_windows_environment<T>(work: impl FnOnce() -> T) -> T {
        // SAFETY: these tests run single-threaded within this module's process
        // and restore nothing, because the values are the real ones a Windows
        // machine reports.
        unsafe {
            std::env::set_var("SystemRoot", r"C:\Windows");
            std::env::set_var("ProgramFiles", r"C:\Program Files");
            std::env::set_var("PUBLIC", r"C:\Users\Public");
        }
        work()
    }

    #[cfg(not(windows))]
    fn with_windows_environment<T>(work: impl FnOnce() -> T) -> T {
        work()
    }

    #[test]
    fn protects_filesystem_system_and_home_scopes() {
        with_windows_environment(|| {
            let cwd = Path::new(FIXTURE.project_cwd);
            let homes = vec![PathBuf::from(FIXTURE.home)];

            for protected in [
                FIXTURE.root,
                FIXTURE.profiles,
                FIXTURE.system_file,
                FIXTURE.home,
            ] {
                assert!(
                    sensitive_scope_with_homes(cwd, &[PathBuf::from(protected)], false, &homes)
                        .is_some(),
                    "{protected} must be protected",
                );
            }
        });
    }

    #[test]
    fn permits_normal_projects_and_their_files() {
        with_windows_environment(|| {
            let cwd = Path::new(FIXTURE.project_cwd);
            let homes = vec![PathBuf::from(FIXTURE.home)];

            assert!(sensitive_scope_with_homes(cwd, &[], false, &homes).is_none());
            assert!(
                sensitive_scope_with_homes(
                    cwd,
                    &[PathBuf::from(FIXTURE.project_file)],
                    false,
                    &homes,
                )
                .is_none()
            );
        });
    }

    #[cfg(unix)]
    #[test]
    fn permits_temporary_subdirectories() {
        let cwd = Path::new(FIXTURE.project_cwd);
        let homes = vec![PathBuf::from(FIXTURE.home)];

        assert!(
            sensitive_scope_with_homes(cwd, &[PathBuf::from("/tmp/normfix-test")], false, &homes)
                .is_none()
        );
    }

    #[test]
    fn git_scope_uses_the_repository_root_not_thousands_of_selected_files() {
        with_windows_environment(|| {
            let cwd = Path::new(FIXTURE.home);
            let homes = vec![PathBuf::from(FIXTURE.home)];

            let risk =
                sensitive_scope_with_homes(cwd, &[PathBuf::from("project/main.c")], true, &homes)
                    .expect("home-scoped Git run must be protected");
            assert_eq!(risk.reason, super::ScopeReason::HomeDirectory);
        });
    }

    #[test]
    fn lexical_parent_components_cannot_bypass_a_protected_tree() {
        with_windows_environment(|| {
            let cwd = Path::new(FIXTURE.project_cwd);
            let risk =
                sensitive_scope_with_homes(cwd, &[PathBuf::from(FIXTURE.escaping)], false, &[]);
            assert!(risk.is_some());
        });
    }

    #[cfg(windows)]
    #[test]
    fn a_protected_tree_is_recognized_however_it_is_spelled() {
        with_windows_environment(|| {
            let cwd = Path::new(FIXTURE.project_cwd);

            // Windows paths are case-insensitive, so a guard that compared them
            // literally would refuse `C:\Windows` and walk `C:\WINDOWS`.
            for spelling in [r"C:\WINDOWS\System32", r"c:\windows\system32"] {
                assert!(
                    sensitive_scope_with_homes(cwd, &[PathBuf::from(spelling)], false, &[])
                        .is_some(),
                    "{spelling} must be protected",
                );
            }
        });
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
