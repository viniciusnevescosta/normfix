//! Path vocabulary shared by the pipeline.
//!
//! Every path a report shows, a transaction roots at, or a backup lands in is
//! resolved here, so the rules about what counts as "inside the project" live
//! in one place rather than beside each caller.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use camino::Utf8PathBuf;
use normfix_report::ReportIdentity;

use super::FixOptions;

pub(super) fn report_identity(options: &FixOptions) -> ReportIdentity {
    options.identity.as_ref().map_or_else(
        || ReportIdentity {
            source: options.identity_source.clone(),
            ..ReportIdentity::default()
        },
        |identity| ReportIdentity {
            login: identity.login.clone(),
            email: identity.email.clone(),
            source: identity.source.clone(),
            inferred: identity.inferred(),
            available: true,
        },
    )
}

pub(super) fn report_path(path: &Path, cwd: &Path) -> Result<Utf8PathBuf, PathBuf> {
    let display = path
        .strip_prefix(cwd)
        .ok()
        .filter(|relative| !relative.as_os_str().is_empty())
        .unwrap_or(path);
    Utf8PathBuf::from_path_buf(display.to_path_buf())
}

pub(super) fn transaction_root<'a>(
    paths: impl Iterator<Item = &'a Path>,
    fallback: &Path,
) -> PathBuf {
    let mut paths = paths.map(absolute_lexical);
    let Some(mut common) = paths.next() else {
        return absolute_lexical(fallback);
    };
    common.pop();
    for path in paths {
        while !path.starts_with(&common) {
            if !common.pop() {
                return absolute_lexical(fallback);
            }
        }
    }
    common
}

pub(super) fn absolute_lexical(path: &Path) -> PathBuf {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().map_or_else(|_| path.to_path_buf(), |cwd| cwd.join(path))
    };
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

/// The user-owned directory backups live in, per platform.
///
/// `XDG_DATA_HOME` wins wherever it is set, which keeps a configured location
/// authoritative. Otherwise Windows uses `LOCALAPPDATA` — the same variable the
/// cache already resolves through, and a directory whose ACL is already
/// restricted to that user — and Unix uses the XDG fallback.
///
/// Returning `None` is not a detail: with no external directory the writer
/// refuses to write at all, so a platform without a branch here has a tool that
/// cannot perform its default action.
pub(super) fn automatic_backup_root() -> Option<PathBuf> {
    platform_data_base().map(|base| base.join("normfix/backups"))
}

fn platform_data_base() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("XDG_DATA_HOME").filter(|path| !path.is_empty()) {
        return Some(PathBuf::from(path));
    }
    #[cfg(windows)]
    {
        std::env::var_os("LOCALAPPDATA")
            .filter(|path| !path.is_empty())
            .map(PathBuf::from)
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("HOME")
            .filter(|path| !path.is_empty())
            .map(PathBuf::from)
            .map(|home| home.join(".local/share"))
    }
}

pub(super) fn run_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    format!("run-{nanos}-{}", std::process::id())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::transaction_root;

    /// Absolute paths as each platform writes them.
    ///
    /// A Unix literal such as `/project` is not absolute in the Windows sense:
    /// it names the root of whatever drive is current, so the function resolves
    /// it against one and the assertion is comparing two different ideas.
    #[cfg(unix)]
    const PREFIX: &str = "";
    #[cfg(windows)]
    const PREFIX: &str = r"C:";

    fn path(suffix: &str) -> String {
        let separated = if cfg!(windows) {
            suffix.replace('/', "\\")
        } else {
            suffix.to_owned()
        };
        format!("{PREFIX}{separated}")
    }

    #[test]
    fn transaction_root_is_the_common_ancestor_and_falls_back_to_the_cwd() {
        let cwd = path("/project");
        let cwd = Path::new(&cwd);

        let first = path("/project/src/main.c");
        let second = path("/project/src/util.c");
        let inside = [Path::new(&first), Path::new(&second)];
        assert_eq!(
            transaction_root(inside.iter().copied(), cwd),
            Path::new(&path("/project/src")),
        );

        let here = path("/project/main.c");
        let there = path("/elsewhere/main.c");
        let disjoint = [Path::new(&here), Path::new(&there)];
        assert_eq!(
            transaction_root(disjoint.iter().copied(), cwd),
            Path::new(&path("/")),
        );
    }
}
