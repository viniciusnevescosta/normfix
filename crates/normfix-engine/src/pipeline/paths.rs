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

pub(super) fn automatic_backup_root() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("XDG_DATA_HOME").filter(|path| !path.is_empty()) {
        return Some(PathBuf::from(path).join("normfix/backups"));
    }
    std::env::var_os("HOME")
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .map(|home| home.join(".local/share/normfix/backups"))
}

pub(super) fn run_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    format!("run-{nanos}-{}", std::process::id())
}

#[cfg(test)]
mod tests {
    use super::transaction_root;

    #[test]
    fn transaction_root_is_the_common_ancestor_and_falls_back_to_the_cwd() {
        let cwd = std::path::Path::new("/project");
        let inside = [
            std::path::Path::new("/project/src/main.c"),
            std::path::Path::new("/project/src/util.c"),
        ];
        assert_eq!(
            transaction_root(inside.iter().copied(), cwd),
            std::path::Path::new("/project/src")
        );

        let disjoint = [
            std::path::Path::new("/project/main.c"),
            std::path::Path::new("/elsewhere/main.c"),
        ];
        assert_eq!(
            transaction_root(disjoint.iter().copied(), cwd),
            std::path::Path::new("/")
        );
    }
}
