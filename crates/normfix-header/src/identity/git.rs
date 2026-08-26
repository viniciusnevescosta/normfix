//! Bounded Git identity lookup through an explicitly resolved executable.

#[cfg(target_arch = "wasm32")]
use std::path::Path;
#[cfg(target_arch = "wasm32")]
use std::time::Duration;

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use std::env;
    use std::ffi::OsString;
    use std::fs::{self, File, OpenOptions};
    use std::io::{Read as _, Seek as _, SeekFrom};
    use std::path::{Path, PathBuf};
    use std::process::{Child, Command, Stdio};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{Duration, Instant};

    use wait_timeout::ChildExt as _;

    #[cfg(unix)]
    use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};

    const OUTPUT_LIMIT: u64 = 4_096;
    const POLL_INTERVAL: Duration = Duration::from_millis(5);
    static SCRATCH_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    pub(super) fn git_config_email(
        cwd: &Path,
        timeout: Duration,
        path: Option<&str>,
        path_ext: Option<&str>,
    ) -> Option<String> {
        let executable = resolve_git_executable(path?, path_ext)?;
        let mut output = ScratchOutput::create()?;
        let child_output = output.file.try_clone().ok()?;
        let mut child = Command::new(executable)
            .args(["--no-pager", "config", "--get", "user.email"])
            .current_dir(cwd)
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GCM_INTERACTIVE", "never")
            .stdin(Stdio::null())
            .stdout(Stdio::from(child_output))
            .stderr(Stdio::null())
            .spawn()
            .ok()?;

        let status = wait_bounded(&mut child, &output.file, timeout)?;
        if !status.success() {
            return None;
        }
        let length = output.file.metadata().ok()?.len();
        if length == 0 || length > OUTPUT_LIMIT {
            return None;
        }
        output.file.seek(SeekFrom::Start(0)).ok()?;
        let mut bytes = Vec::with_capacity(usize::try_from(length).ok()?);
        output
            .file
            .by_ref()
            .take(OUTPUT_LIMIT.saturating_add(1))
            .read_to_end(&mut bytes)
            .ok()?;
        if u64::try_from(bytes.len()).ok()? > OUTPUT_LIMIT {
            return None;
        }
        let output = String::from_utf8(bytes).ok()?;
        let value = output.trim();
        (!value.is_empty()).then(|| value.to_owned())
    }

    fn wait_bounded(
        child: &mut Child,
        output: &File,
        timeout: Duration,
    ) -> Option<std::process::ExitStatus> {
        let started = Instant::now();
        loop {
            let Ok(metadata) = output.metadata() else {
                terminate_and_reap(child);
                return None;
            };
            if metadata.len() > OUTPUT_LIMIT {
                terminate_and_reap(child);
                return None;
            }
            let elapsed = started.elapsed();
            if elapsed >= timeout {
                terminate_and_reap(child);
                return None;
            }
            let wait_for = POLL_INTERVAL.min(timeout.saturating_sub(elapsed));
            match child.wait_timeout(wait_for) {
                Ok(Some(status)) => return Some(status),
                Ok(None) => {}
                Err(_) => {
                    terminate_and_reap(child);
                    return None;
                }
            }
        }
    }

    fn terminate_and_reap(child: &mut Child) {
        let _ = child.kill();
        let _ = child.wait();
    }

    fn resolve_git_executable(path: &str, path_ext: Option<&str>) -> Option<PathBuf> {
        let names = executable_names(path_ext);
        for directory in env::split_paths(path).filter(|entry| entry.is_absolute()) {
            for name in &names {
                let candidate = directory.join(name);
                let Ok(canonical) = fs::canonicalize(candidate) else {
                    continue;
                };
                if canonical.is_absolute() && is_executable_file(&canonical) {
                    return Some(canonical);
                }
            }
        }
        None
    }

    #[cfg(windows)]
    fn executable_names(path_ext: Option<&str>) -> Vec<OsString> {
        let extensions = path_ext.unwrap_or(".COM;.EXE;.BAT;.CMD");
        let mut names = Vec::new();
        for extension in extensions.split(';').filter_map(valid_windows_extension) {
            let mut name = OsString::from("git");
            name.push(extension);
            if !names.contains(&name) {
                names.push(name);
            }
        }
        names
    }

    #[cfg(windows)]
    fn valid_windows_extension(value: &str) -> Option<String> {
        let trimmed = value.trim();
        (!trimmed.is_empty()
            && trimmed.starts_with('.')
            && trimmed
                .chars()
                .all(|character| character == '.' || character.is_ascii_alphanumeric()))
        .then(|| trimmed.to_ascii_lowercase())
    }

    #[cfg(not(windows))]
    fn executable_names(_path_ext: Option<&str>) -> Vec<OsString> {
        vec![OsString::from("git")]
    }

    fn is_executable_file(path: &Path) -> bool {
        let Ok(metadata) = fs::metadata(path) else {
            return false;
        };
        if !metadata.is_file() {
            return false;
        }
        #[cfg(unix)]
        {
            metadata.permissions().mode() & 0o111 != 0
        }
        #[cfg(not(unix))]
        {
            true
        }
    }

    struct ScratchOutput {
        file: File,
        path: PathBuf,
    }

    impl ScratchOutput {
        fn create() -> Option<Self> {
            let directory = fs::canonicalize(env::temp_dir()).ok()?;
            if !directory.is_absolute() || !directory.is_dir() {
                return None;
            }
            for _ in 0..64 {
                let sequence = SCRATCH_SEQUENCE.fetch_add(1, Ordering::Relaxed);
                let path = directory.join(format!(
                    ".normfix-git-output-{}-{sequence}.tmp",
                    std::process::id()
                ));
                let mut options = OpenOptions::new();
                options.read(true).write(true).create_new(true);
                #[cfg(unix)]
                options.mode(0o600);
                match options.open(&path) {
                    Ok(file) => return Some(Self { file, path }),
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(_) => return None,
                }
            }
            None
        }
    }

    impl Drop for ScratchOutput {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.path);
        }
    }

    #[cfg(all(test, unix))]
    mod tests {
        use std::fs;
        use std::os::unix::fs::PermissionsExt as _;
        use std::path::{Path, PathBuf};
        use std::time::{Duration, Instant};

        use tempfile::TempDir;

        use super::{OUTPUT_LIMIT, git_config_email};

        fn fake_git(directory: &Path, body: &str) -> PathBuf {
            fs::create_dir_all(directory).expect("bin directory");
            let executable = directory.join("git");
            fs::write(&executable, format!("#!/bin/sh\n{body}\n")).expect("fake git");
            fs::set_permissions(&executable, fs::Permissions::from_mode(0o700))
                .expect("executable permissions");
            executable
        }

        #[test]
        fn executes_an_absolute_path_candidate() {
            let temporary = TempDir::new().expect("temporary directory");
            let bin = temporary.path().join("bin");
            fake_git(&bin, "printf '%s\\n' 'absolute@student.42.fr'");

            let email = git_config_email(
                temporary.path(),
                Duration::from_secs(5),
                Some(bin.to_str().expect("utf-8 path")),
                None,
            );

            assert_eq!(email.as_deref(), Some("absolute@student.42.fr"));
        }

        #[test]
        fn ignores_empty_and_relative_path_components() {
            let temporary = TempDir::new().expect("temporary directory");
            let planted_marker = temporary.path().join("planted-ran");
            fake_git(
                temporary.path(),
                &format!("touch '{}'", planted_marker.display()),
            );

            let email = git_config_email(
                temporary.path(),
                Duration::from_millis(100),
                Some(":"),
                None,
            );

            assert!(email.is_none());
            assert!(!planted_marker.exists());
        }

        #[test]
        fn relative_entries_cannot_shadow_a_later_absolute_candidate() {
            let temporary = TempDir::new().expect("temporary directory");
            let planted_marker = temporary.path().join("planted-ran");
            fake_git(
                temporary.path(),
                &format!("touch '{}'", planted_marker.display()),
            );
            let bin = temporary.path().join("trusted-bin");
            fake_git(&bin, "printf '%s\\n' 'trusted@student.42.fr'");
            let search = format!(".:{}", bin.display());

            let email = git_config_email(
                temporary.path(),
                Duration::from_secs(5),
                Some(&search),
                None,
            );

            assert_eq!(email.as_deref(), Some("trusted@student.42.fr"));
            assert!(!planted_marker.exists());
        }

        #[test]
        fn kills_and_reaps_a_timed_out_process() {
            let temporary = TempDir::new().expect("temporary directory");
            let bin = temporary.path().join("bin");
            fake_git(&bin, "exec sleep 5");
            let started = Instant::now();

            let email = git_config_email(
                temporary.path(),
                Duration::from_millis(40),
                Some(bin.to_str().expect("utf-8 path")),
                None,
            );

            assert!(email.is_none());
            assert!(started.elapsed() < Duration::from_secs(2));
        }

        #[test]
        fn rejects_output_larger_than_the_bound() {
            let temporary = TempDir::new().expect("temporary directory");
            let bin = temporary.path().join("bin");
            fake_git(
                &bin,
                &format!(
                    "i=0; while [ \"$i\" -le {} ]; do printf x; i=$((i + 1)); done",
                    OUTPUT_LIMIT + 8
                ),
            );

            let email = git_config_email(
                temporary.path(),
                Duration::from_secs(1),
                Some(bin.to_str().expect("utf-8 path")),
                None,
            );

            assert!(email.is_none());
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn git_config_email(
    cwd: &std::path::Path,
    timeout: std::time::Duration,
    path: Option<&str>,
    path_ext: Option<&str>,
) -> Option<String> {
    native::git_config_email(cwd, timeout, path, path_ext)
}

#[cfg(target_arch = "wasm32")]
pub(super) fn git_config_email(
    _cwd: &Path,
    _timeout: Duration,
    _path: Option<&str>,
    _path_ext: Option<&str>,
) -> Option<String> {
    None
}
