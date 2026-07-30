use std::ffi::{OsStr, OsString};
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
    for directory in std::env::split_paths(&path) {
        for file_name in executable_names(command_name) {
            let candidate = directory.join(file_name);
            if let Ok(path) = validate_executable(&candidate) {
                return Ok(path);
            }
        }
    }
    Err(format!("could not find `{command_name}` on PATH"))
}

fn validate_executable(path: &Path) -> Result<PathBuf, String> {
    let metadata = fs::metadata(path)
        .map_err(|error| format!("cannot inspect `{}`: {error}", path.display()))?;
    if !metadata.is_file() {
        return Err(format!("`{}` is not a regular file", path.display()));
    }
    if !is_executable(&metadata) {
        return Err(format!("`{}` is not executable", path.display()));
    }
    fs::canonicalize(path)
        .map_err(|error| format!("cannot resolve executable `{}`: {error}", path.display()))
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
            let extensions = std::env::var_os("PATHEXT")
                .unwrap_or_else(|| OsString::from(".COM;.EXE;.BAT;.CMD"));
            names.extend(
                extensions
                    .to_string_lossy()
                    .split(';')
                    .filter(|extension| !extension.is_empty())
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
