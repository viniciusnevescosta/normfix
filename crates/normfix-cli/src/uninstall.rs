//! Removing normfix from a machine.
//!
//! Uninstalling is the one destructive operation whose target is the tool
//! itself, so it follows the same rule as every other one here: say exactly
//! what will be removed, remove nothing without an explicit acknowledgement,
//! and never touch recovery data unless that was asked for by name.

use std::fs;
use std::path::{Path, PathBuf};

/// One thing an uninstall would delete.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Target {
    /// Stable English label used in the plan and in any failure.
    pub label: &'static str,
    /// Absolute path that would be removed.
    pub path: PathBuf,
    /// Whether the path holds data that cannot be reproduced.
    pub irreplaceable: bool,
}

/// Everything an uninstall would do, computed before anything is removed.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Plan {
    /// The running binary, when it can be located and removed.
    pub binary: Option<PathBuf>,
    /// Configuration, cache, and backup directories selected by `--purge`.
    pub data: Vec<Target>,
    /// Data directories that exist but were deliberately left alone.
    pub kept: Vec<Target>,
}

impl Plan {
    /// Returns whether this plan would delete recovery data.
    pub fn removes_recovery_data(&self) -> bool {
        self.data.iter().any(|target| target.irreplaceable)
    }
}

/// Refuses to delete a binary another package manager is responsible for.
///
/// Removing a Homebrew-managed file leaves the formula describing something
/// that is no longer on disk, and `brew` is then the only thing that can put
/// the machine back in a consistent state.
fn reject_managed_install(executable: &Path) -> Result<(), String> {
    let path = executable.to_string_lossy();
    if path.contains("/Cellar/") || path.contains("/homebrew/") || path.contains("linuxbrew/") {
        return Err(format!(
            "{path} is managed by Homebrew; uninstall it with `brew uninstall viniciusnevescosta/normfix/normfix`"
        ));
    }
    let lowered = path.to_lowercase().replace('\\', "/");
    if lowered.contains("/scoop/apps/") || lowered.contains("/scoop/shims/") {
        return Err(format!(
            "{path} is managed by Scoop; uninstall it with `scoop uninstall normfix`"
        ));
    }
    Ok(())
}

fn existing(label: &'static str, path: PathBuf, irreplaceable: bool) -> Option<Target> {
    // `symlink_metadata` so a link is reported as the link it is rather than
    // followed to something outside the directory being removed.
    fs::symlink_metadata(&path).is_ok().then_some(Target {
        label,
        path,
        irreplaceable,
    })
}

fn base_directory(explicit: &str, fallback: &str) -> Option<PathBuf> {
    if let Some(path) = std::env::var_os(explicit)
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
    {
        return Some(path);
    }
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|home| !home.as_os_str().is_empty())
        .map(|home| home.join(fallback))
}

/// Lists the per-user directories this tool creates, whether or not they exist.
fn data_directories() -> Vec<(&'static str, PathBuf, bool)> {
    let mut directories = Vec::new();
    if let Some(base) = base_directory("XDG_CONFIG_HOME", ".config") {
        directories.push(("configuration", base.join("normfix"), false));
    }
    if let Some(base) = base_directory("XDG_CACHE_HOME", ".cache") {
        directories.push(("cache", base.join("normfix"), false));
    }
    if let Some(base) = base_directory("XDG_DATA_HOME", ".local/share") {
        // Backups are the only thing here that cannot be recreated by running
        // the tool again, which is why they are flagged and why `--purge` has
        // to name them before it removes them.
        directories.push(("backups and quarantine", base.join("normfix"), true));
    }
    directories
}

/// Computes what an uninstall would remove without removing anything.
pub(crate) fn plan(purge: bool) -> Result<Plan, String> {
    let executable = std::env::current_exe()
        .map_err(|error| format!("could not locate the running binary: {error}"))?;
    reject_managed_install(&executable)?;

    let mut plan = Plan {
        binary: Some(executable),
        ..Plan::default()
    };
    for (label, path, irreplaceable) in data_directories() {
        let Some(target) = existing(label, path, irreplaceable) else {
            continue;
        };
        if purge {
            plan.data.push(target);
        } else {
            plan.kept.push(target);
        }
    }
    Ok(plan)
}

/// Executes a plan, removing user data before the binary.
///
/// The order matters: if a directory cannot be removed, the tool that reports
/// the failure and can retry is still on disk.
pub(crate) fn remove(plan: &Plan) -> Result<(), String> {
    for target in &plan.data {
        fs::remove_dir_all(&target.path).map_err(|error| {
            format!(
                "could not remove the {} directory {}: {error}",
                target.label,
                target.path.display()
            )
        })?;
    }

    if let Some(binary) = &plan.binary {
        remove_binary(binary)?;
    }
    Ok(())
}

#[cfg(unix)]
fn remove_binary(binary: &Path) -> Result<(), String> {
    // Unlinking a running executable is safe on Unix: the kernel keeps the
    // inode alive until this process exits, so the command finishes normally.
    fs::remove_file(binary).map_err(|error| {
        format!(
            "could not remove {}: {error}. Check who owns the file; normfix never elevates privileges.",
            binary.display()
        )
    })
}

#[cfg(not(unix))]
fn remove_binary(binary: &Path) -> Result<(), String> {
    Err(format!(
        "this platform cannot delete a running executable, so {} must be removed manually",
        binary.display()
    ))
}

/// Renders the plan as the block shown before anything is removed.
pub(crate) fn describe(plan: &Plan) -> String {
    use std::fmt::Write as _;

    let mut output = String::from("normfix uninstall\n");
    if let Some(binary) = &plan.binary {
        let _ = writeln!(output, "  remove  {}", binary.display());
    }
    for target in &plan.data {
        let _ = writeln!(
            output,
            "  remove  {} ({})",
            target.path.display(),
            target.label
        );
    }
    for target in &plan.kept {
        let _ = writeln!(
            output,
            "  keep    {} ({})",
            target.path.display(),
            target.label
        );
    }
    if !plan.kept.is_empty() {
        output.push_str("Pass --purge to remove the kept directories as well.\n");
    }
    output
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{Plan, Target, describe, reject_managed_install};

    fn target(label: &'static str, path: &str, irreplaceable: bool) -> Target {
        Target {
            label,
            path: PathBuf::from(path),
            irreplaceable,
        }
    }

    #[test]
    fn package_manager_owned_binaries_are_refused_with_the_command_that_works() {
        for (path, manager, command) in [
            (
                "/opt/homebrew/Cellar/normfix/1.0.0/bin/normfix",
                "Homebrew",
                "brew uninstall",
            ),
            (
                "/home/student/.linuxbrew/Cellar/normfix/1.0.0/bin/normfix",
                "Homebrew",
                "brew uninstall",
            ),
            (
                r"C:\Users\student\scoop\apps\normfix\current\normfix.exe",
                "Scoop",
                "scoop uninstall normfix",
            ),
            (
                r"C:\Users\student\scoop\shims\normfix.exe",
                "Scoop",
                "scoop uninstall normfix",
            ),
        ] {
            let error = reject_managed_install(&PathBuf::from(path))
                .expect_err("a managed install must be refused");
            assert!(error.contains(manager), "{path}: {error}");
            assert!(error.contains(command), "{path}: {error}");
            assert!(!error.contains('\n'));
        }

        assert!(reject_managed_install(&PathBuf::from("/usr/local/bin/normfix")).is_ok());
    }

    #[test]
    fn the_default_plan_keeps_user_data_and_says_so() {
        let plan = Plan {
            binary: Some(PathBuf::from("/usr/local/bin/normfix")),
            data: Vec::new(),
            kept: vec![target(
                "backups and quarantine",
                "/home/s/.local/share/normfix",
                true,
            )],
        };
        let described = describe(&plan);

        assert!(described.contains("remove  /usr/local/bin/normfix"));
        assert!(described.contains("keep    /home/s/.local/share/normfix"));
        assert!(described.contains("--purge"));
        assert!(!plan.removes_recovery_data());
    }

    #[test]
    fn a_purge_plan_reports_that_recovery_data_is_included() {
        let plan = Plan {
            binary: Some(PathBuf::from("/usr/local/bin/normfix")),
            data: vec![
                target("configuration", "/home/s/.config/normfix", false),
                target(
                    "backups and quarantine",
                    "/home/s/.local/share/normfix",
                    true,
                ),
            ],
            kept: Vec::new(),
        };
        let described = describe(&plan);

        assert!(plan.removes_recovery_data());
        assert!(described.contains("remove  /home/s/.config/normfix (configuration)"));
        assert!(!described.contains("--purge"));
    }
}
