#[cfg(not(windows))]
use std::ffi::OsStr;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) fn resolve_executable(
    explicit: Option<&Path>,
    command_name: &str,
) -> Result<PathBuf, String> {
    if let Some(path) = explicit {
        return validate_executable(path);
    }
    let path = std::env::var_os("PATH").ok_or_else(|| "PATH is not set".to_owned())?;
    resolve_on_search_path(&path, command_name)
}

fn resolve_on_search_path(path: &OsStr, command_name: &str) -> Result<PathBuf, String> {
    let file_names = executable_names(command_name);
    for directory in std::env::split_paths(path) {
        // Empty and relative PATH entries resolve against the process working
        // directory. normfix is normally started inside the project it is
        // auditing, so honoring one would let an untrusted `./cc` or
        // `./norminette` impersonate a system tool. A relative executable is
        // still accepted when the user names it explicitly above.
        if !directory.is_absolute() {
            continue;
        }
        for file_name in &file_names {
            let candidate = directory.join(file_name);
            if let Ok(path) = validate_executable(&candidate) {
                return Ok(path);
            }
        }
    }
    Err(format!("could not find `{command_name}` on PATH"))
}

fn validate_executable(path: &Path) -> Result<PathBuf, String> {
    // Resolve first, then inspect the object that will actually be passed to
    // `Command`. Inspecting a symlink target before canonicalizing it leaves a
    // race where the link can be changed between those two operations and the
    // replacement is returned without ever having been validated.
    let resolved = fs::canonicalize(path)
        .map_err(|error| format!("cannot resolve executable `{}`: {error}", path.display()))?;
    #[cfg(windows)]
    if resolved.extension().is_some_and(|extension| {
        matches!(
            extension.to_string_lossy().to_ascii_lowercase().as_str(),
            "bat" | "cmd"
        )
    }) {
        return Err(format!(
            "`{}` is a command script, not a native executable",
            resolved.display()
        ));
    }
    let metadata = fs::metadata(&resolved)
        .map_err(|error| format!("cannot inspect `{}`: {error}", resolved.display()))?;
    if !metadata.is_file() {
        return Err(format!("`{}` is not a regular file", resolved.display()));
    }
    if !is_executable(&metadata) {
        return Err(format!("`{}` is not executable", resolved.display()));
    }
    Ok(resolved)
}

#[cfg(unix)]
fn is_executable(metadata: &fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;

    metadata.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn is_executable(_metadata: &fs::Metadata) -> bool {
    true
}

fn executable_names(command_name: &str) -> Vec<OsString> {
    #[cfg(windows)]
    {
        let mut names = vec![OsString::from(command_name)];
        if Path::new(command_name).extension().is_none() {
            let extensions =
                std::env::var_os("PATHEXT").unwrap_or_else(|| OsString::from(".COM;.EXE"));
            names.extend(
                extensions
                    .to_string_lossy()
                    .split(';')
                    // Rust cannot safely quote arbitrary arguments for batch
                    // files: Windows runs those through `cmd.exe`, even when
                    // the caller used `Command` rather than a shell. Oracle
                    // executables therefore have to be native programs.
                    .filter(|extension| {
                        matches!(extension.to_ascii_uppercase().as_str(), ".COM" | ".EXE")
                    })
                    .map(|extension| {
                        let mut name = OsString::from(command_name);
                        name.push(extension);
                        name
                    }),
            );
        }
        names
    }
    #[cfg(not(windows))]
    {
        vec![OsStr::new(command_name).to_owned()]
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use tempfile::TempDir;

    use super::{resolve_executable, resolve_on_search_path};

    #[cfg(unix)]
    fn make_executable(path: &Path) {
        use std::os::unix::fs::PermissionsExt as _;

        let mut permissions = std::fs::metadata(path).expect("metadata").permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(path, permissions).expect("executable permissions");
    }

    #[cfg(unix)]
    #[test]
    fn a_relative_path_entry_cannot_select_a_project_file() {
        let current = std::env::current_dir().expect("current directory");
        let directory = TempDir::new_in(&current).expect("temporary directory");
        let executable = directory.path().join("cc");
        std::fs::write(&executable, "#!/bin/sh\nexit 0\n").expect("fake executable");
        make_executable(&executable);
        let relative = directory
            .path()
            .strip_prefix(&current)
            .expect("relative temporary directory");

        let result = resolve_on_search_path(relative.as_os_str(), "cc");

        assert!(result.is_err(), "a project-local PATH entry was accepted");
    }

    #[cfg(unix)]
    #[test]
    fn an_explicit_relative_executable_is_still_user_authorized() {
        let directory = TempDir::new().expect("temporary directory");
        let executable = directory.path().join("tool");
        std::fs::write(&executable, "#!/bin/sh\nexit 0\n").expect("fake executable");
        make_executable(&executable);

        let resolved = resolve_executable(Some(&executable), "ignored").expect("explicit tool");

        assert!(resolved.is_absolute());
        assert_eq!(
            resolved,
            executable.canonicalize().expect("canonical executable")
        );
    }
}
