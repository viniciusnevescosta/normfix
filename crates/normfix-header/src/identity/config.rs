//! Identity configuration discovery, parsing, and atomic persistence.

use std::collections::BTreeSet;
use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};

use thiserror::Error;

#[cfg(unix)]
use std::fs::File;
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};

use super::file::{SymlinkPolicy, path_entry_exists, read_bounded_regular_file};
use super::{CONFIG_ENV, Identity42, IdentityResolver, LEGACY_CONFIG_ENV, SETTINGS_SIZE_LIMIT};

/// A failure to locate or securely persist the global 42 identity.
#[derive(Debug, Error)]
pub enum IdentityConfigError {
    /// No supported per-user configuration directory was available.
    #[error("no per-user configuration directory is available; set NORMFIX_CONFIG explicitly")]
    LocationUnavailable,
    /// The supplied identity no longer satisfies the canonical invariant.
    #[error("refusing to persist an invalid 42 identity")]
    InvalidIdentity,
    /// The destination is not safe to replace as a regular configuration file.
    #[error("unsafe identity configuration path `{path}`: {reason}")]
    UnsafePath {
        /// Rejected path.
        path: PathBuf,
        /// Refusal reason.
        reason: String,
    },
    /// A filesystem operation failed.
    #[error("could not {operation} `{path}`: {source}")]
    Io {
        /// Short operation description.
        operation: &'static str,
        /// Affected path.
        path: PathBuf,
        /// Underlying operating-system error.
        #[source]
        source: std::io::Error,
    },
}

pub(super) fn configured_identity(resolver: &IdentityResolver) -> (Option<String>, Option<String>) {
    let candidates = configuration_candidates(resolver);
    let Some(path) = candidates
        .iter()
        .find(|candidate| path_entry_exists(candidate))
        .or_else(|| candidates.first())
    else {
        return (None, None);
    };
    parse_header_ini(path).unwrap_or((None, None))
}

pub(super) fn configuration_candidates(resolver: &IdentityResolver) -> Vec<PathBuf> {
    if let Some(configured) = resolver
        .environment_value(CONFIG_ENV)
        .or_else(|| resolver.environment_value(LEGACY_CONFIG_ENV))
    {
        return vec![expand_home(configured, resolver.home())];
    }

    let bases = default_configuration_bases(resolver);
    let mut candidates = bases
        .iter()
        .map(|base| base.join("normfix/config.ini"))
        .collect::<Vec<_>>();
    candidates.extend(
        bases
            .iter()
            .map(|base| base.join("norminette-fix/config.ini")),
    );
    candidates
}

pub(super) fn preferred_configuration_path(resolver: &IdentityResolver) -> Option<PathBuf> {
    configuration_candidates(resolver).into_iter().next()
}

fn default_configuration_bases(resolver: &IdentityResolver) -> Vec<PathBuf> {
    if let Some(xdg) = resolver.environment_value("XDG_CONFIG_HOME") {
        return vec![PathBuf::from(xdg)];
    }

    #[cfg(windows)]
    if let Some(app_data) = resolver.environment_value("APPDATA") {
        return vec![PathBuf::from(app_data)];
    }

    let Some(home) = resolver.home() else {
        return Vec::new();
    };
    #[cfg(target_os = "macos")]
    {
        vec![
            home.join("Library/Application Support"),
            home.join(".config"),
        ]
    }
    #[cfg(not(target_os = "macos"))]
    {
        vec![home.join(".config")]
    }
}

/// Persists a validated identity in the platform's per-user configuration.
///
/// `NORMFIX_CONFIG` takes precedence. Otherwise this uses
/// `$XDG_CONFIG_HOME`, `%APPDATA%` on Windows, `~/Library/Application Support`
/// on macOS, or `~/.config` on other Unix platforms. The destination is
/// replaced atomically where the platform permits, with owner-only file and
/// application-directory permissions on Unix.
///
/// # Errors
///
/// Returns an error when the identity is invalid, no user configuration
/// directory can be located, the destination is a symbolic link or non-file,
/// or secure persistence fails.
pub fn persist_identity(identity: &Identity42) -> Result<PathBuf, IdentityConfigError> {
    let resolver = IdentityResolver::from_process();
    let uses_default_directory = resolver.environment_value(CONFIG_ENV).is_none()
        && resolver.environment_value(LEGACY_CONFIG_ENV).is_none();
    let path =
        preferred_configuration_path(&resolver).ok_or(IdentityConfigError::LocationUnavailable)?;
    if !path.is_absolute() {
        return Err(IdentityConfigError::UnsafePath {
            path,
            reason: "the global configuration path must be absolute".to_owned(),
        });
    }
    persist_identity_with_directory_policy(identity, &path, uses_default_directory)?;
    Ok(path)
}

#[cfg(test)]
pub(super) fn persist_identity_at(
    identity: &Identity42,
    path: &Path,
) -> Result<(), IdentityConfigError> {
    persist_identity_with_directory_policy(identity, path, true)
}

#[cfg(test)]
pub(super) fn persist_identity_with_directory_policy_for_test(
    identity: &Identity42,
    path: &Path,
    secure_existing_parent: bool,
) -> Result<(), IdentityConfigError> {
    persist_identity_with_directory_policy(identity, path, secure_existing_parent)
}

fn persist_identity_with_directory_policy(
    identity: &Identity42,
    path: &Path,
    secure_existing_parent: bool,
) -> Result<(), IdentityConfigError> {
    if !identity.is_valid() {
        return Err(IdentityConfigError::InvalidIdentity);
    }
    let parent = identity_config_parent(path)?;
    prepare_identity_config_directory(&parent, secure_existing_parent)?;
    ensure_regular_config_destination(path, "the existing destination is not a regular file")?;
    let contents = format!(
        "[header]\nlogin = {}\nemail = {}\n",
        identity.login, identity.email
    );
    let temporary = TemporaryIdentityConfig::write(&parent, contents.as_bytes())?;
    install_identity_config(temporary, path, &parent)
}

fn identity_config_parent(path: &Path) -> Result<PathBuf, IdentityConfigError> {
    let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    else {
        return Err(IdentityConfigError::UnsafePath {
            path: path.to_path_buf(),
            reason: "the file has no parent directory".to_owned(),
        });
    };
    Ok(parent.to_path_buf())
}

fn ensure_regular_config_destination(
    path: &Path,
    reason: &'static str,
) -> Result<(), IdentityConfigError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            Err(IdentityConfigError::UnsafePath {
                path: path.to_path_buf(),
                reason: reason.to_owned(),
            })
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(IdentityConfigError::Io {
            operation: "inspect identity configuration destination",
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn prepare_identity_config_directory(
    parent: &Path,
    secure_existing_parent: bool,
) -> Result<(), IdentityConfigError> {
    let parent_existed = match fs::symlink_metadata(parent) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err(IdentityConfigError::UnsafePath {
                    path: parent.to_path_buf(),
                    reason: "the destination parent is not a real directory".to_owned(),
                });
            }
            true
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(source) => {
            return Err(IdentityConfigError::Io {
                operation: "inspect identity configuration directory",
                path: parent.to_path_buf(),
                source,
            });
        }
    };
    #[cfg(not(unix))]
    let _ = (parent_existed, secure_existing_parent);
    if !parent_existed {
        fs::create_dir_all(parent).map_err(|source| IdentityConfigError::Io {
            operation: "create identity configuration directory",
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let parent_metadata =
        fs::symlink_metadata(parent).map_err(|source| IdentityConfigError::Io {
            operation: "inspect identity configuration directory",
            path: parent.to_path_buf(),
            source,
        })?;
    if parent_metadata.file_type().is_symlink() || !parent_metadata.is_dir() {
        return Err(IdentityConfigError::UnsafePath {
            path: parent.to_path_buf(),
            reason: "the destination parent is not a real directory".to_owned(),
        });
    }

    #[cfg(unix)]
    if !parent_existed || secure_existing_parent {
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700)).map_err(|source| {
            IdentityConfigError::Io {
                operation: "secure identity configuration directory",
                path: parent.to_path_buf(),
                source,
            }
        })?;
    }
    Ok(())
}

struct TemporaryIdentityConfig {
    path: PathBuf,
    installed: bool,
}

impl TemporaryIdentityConfig {
    fn write(parent: &Path, contents: &[u8]) -> Result<Self, IdentityConfigError> {
        for attempt in 0..32_u8 {
            let candidate = parent.join(format!(
                ".config.ini.normfix-{}-{attempt}.tmp",
                std::process::id()
            ));
            let mut builder = OpenOptions::new();
            builder.write(true).create_new(true);
            #[cfg(unix)]
            builder.mode(0o600);
            match builder.open(&candidate) {
                Ok(mut file) => {
                    let temporary = Self {
                        path: candidate.clone(),
                        installed: false,
                    };
                    file.write_all(contents)
                        .and_then(|()| file.sync_all())
                        .map_err(|source| IdentityConfigError::Io {
                            operation: "write temporary identity configuration",
                            path: candidate.clone(),
                            source,
                        })?;
                    return Ok(temporary);
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(source) => {
                    return Err(IdentityConfigError::Io {
                        operation: "create temporary identity configuration",
                        path: candidate,
                        source,
                    });
                }
            }
        }
        Err(IdentityConfigError::UnsafePath {
            path: parent.to_path_buf(),
            reason: "could not reserve a private temporary filename".to_owned(),
        })
    }
}

impl Drop for TemporaryIdentityConfig {
    fn drop(&mut self) {
        if !self.installed {
            let _ = fs::remove_file(&self.path);
        }
    }
}

fn install_identity_config(
    mut temporary: TemporaryIdentityConfig,
    path: &Path,
    parent: &Path,
) -> Result<(), IdentityConfigError> {
    #[cfg(not(unix))]
    let _ = parent;
    #[cfg(unix)]
    let parent_directory = File::open(parent).map_err(|source| IdentityConfigError::Io {
        operation: "open identity configuration directory",
        path: parent.to_path_buf(),
        source,
    })?;
    ensure_regular_config_destination(path, "the destination changed before it could be replaced")?;
    fs::rename(&temporary.path, path).map_err(|source| IdentityConfigError::Io {
        operation: "install identity configuration",
        path: path.to_path_buf(),
        source,
    })?;
    temporary.installed = true;
    #[cfg(unix)]
    parent_directory
        .sync_all()
        .map_err(|source| IdentityConfigError::Io {
            operation: "synchronize identity configuration directory",
            path: parent.to_path_buf(),
            source,
        })?;
    Ok(())
}

fn parse_header_ini(path: &Path) -> Option<(Option<String>, Option<String>)> {
    let bytes = read_bounded_regular_file(path, SETTINGS_SIZE_LIMIT, SymlinkPolicy::Reject)?;
    let content = String::from_utf8(bytes).ok()?;
    let mut section = None::<String>;
    let mut seen_sections = BTreeSet::new();
    let mut seen_options = BTreeSet::new();
    let mut header_seen = false;
    let mut default_login = None;
    let mut default_email = None;
    let mut header_login = None;
    let mut header_email = None;
    for (index, raw) in content.lines().enumerate() {
        let line = if index == 0 {
            raw.trim_start_matches('\u{feff}').trim()
        } else {
            raw.trim()
        };
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if line.starts_with('[') {
            let close = line.find(']')?;
            let trailing = line[close + 1..].trim();
            if !trailing.is_empty() && !trailing.starts_with('#') && !trailing.starts_with(';') {
                return None;
            }
            let name = line[1..close].trim();
            if name.is_empty() {
                return None;
            }
            let canonical = name.to_ascii_lowercase();
            if !seen_sections.insert(canonical.clone()) {
                return None;
            }
            header_seen |= canonical == "header";
            section = Some(canonical);
            continue;
        }
        let current_section = section.as_deref()?;
        let separator = line.find(['=', ':'])?;
        let key = line[..separator].trim().to_ascii_lowercase();
        let value = line[separator + 1..].trim();
        if key.is_empty() || !seen_options.insert((current_section.to_owned(), key.clone())) {
            return None;
        }
        let target = match (current_section, key.as_str()) {
            ("default", "login") => Some(&mut default_login),
            ("default", "email") => Some(&mut default_email),
            ("header", "login") => Some(&mut header_login),
            ("header", "email") => Some(&mut header_email),
            _ => None,
        };
        if let Some(target) = target {
            *target = (!value.is_empty()).then(|| value.to_owned());
        }
    }
    header_seen.then(|| {
        (
            header_login.or(default_login),
            header_email.or(default_email),
        )
    })
}

fn expand_home(value: &str, home: Option<&Path>) -> PathBuf {
    if value == "~" {
        return home.map_or_else(|| PathBuf::from(value), Path::to_path_buf);
    }
    let remainder = value
        .strip_prefix("~/")
        .or_else(|| value.strip_prefix("~\\"));
    if let (Some(remainder), Some(home)) = (remainder, home) {
        return home.join(remainder);
    }
    PathBuf::from(value)
}
