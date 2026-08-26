//! Ambiguity-safe 42 identity discovery.

mod config;
mod discovery;
mod file;
mod git;
mod validation;

use std::collections::BTreeMap;
use std::env;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::{ByteRange, Issue};

pub use config::{IdentityConfigError, persist_identity};
pub use validation::{canonical_42_email, identity_from_email};

use config::configured_identity;
use discovery::saved_editor_emails;
use git::git_config_email;
use validation::{identity_from_candidate, select_saved_email};

const SETTINGS_SIZE_LIMIT: u64 = 1_000_000;
const GIT_TIMEOUT: Duration = Duration::from_secs(2);
const CONFIG_ENV: &str = "NORMFIX_CONFIG";
const LOGIN_ENV: &str = "NORMFIX_LOGIN";
const EMAIL_ENV: &str = "NORMFIX_EMAIL";
const LEGACY_CONFIG_ENV: &str = "NORMINETTE_FIX_CONFIG";
const LEGACY_LOGIN_ENV: &str = "NORMINETTE_FIX_LOGIN";
const LEGACY_EMAIL_ENV: &str = "NORMINETTE_FIX_EMAIL";

/// A validated 42 student identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Identity42 {
    /// Login derived from the local part of the email.
    pub login: String,
    /// Canonical lowercase 42 student email.
    pub email: String,
    /// Human-readable source of the selected email.
    pub source: String,
    /// Whether no explicit login was available for an inferred email.
    pub inferred_login: bool,
    /// Whether the email came from an implicit source.
    pub inferred_email: bool,
}

impl Identity42 {
    /// Returns whether email and login still satisfy the validated invariant.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        canonical_42_email(&self.email).is_some_and(|email| {
            email == self.email
                && email
                    .split_once('@')
                    .is_some_and(|(login, _)| login == self.login)
        })
    }

    /// Returns whether any part of this identity was inferred.
    #[must_use]
    pub const fn inferred(&self) -> bool {
        self.inferred_login || self.inferred_email
    }
}

/// Result of ambiguity-safe identity resolution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IdentityResolution {
    /// Selected identity, or `None` when no candidate was safe.
    pub identity: Option<Identity42>,
    /// Explanation of the selected source or the refusal reason.
    pub source: String,
    /// English issue when no identity could be selected.
    pub issue: Option<Issue>,
}

impl IdentityResolution {
    pub(super) fn available(identity: Identity42) -> Self {
        let source = identity.source.clone();
        Self {
            identity: Some(identity),
            source,
            issue: None,
        }
    }

    pub(super) fn unavailable(code: &'static str, source: String) -> Self {
        Self {
            identity: None,
            source: source.clone(),
            issue: Some(Issue {
                code,
                message: source,
                range: ByteRange::new(0, 0),
                suggestion: concat!(
                    "Pass --email with one verified 42 student address, or configure it ",
                    "in the header settings."
                )
                .to_owned(),
            }),
        }
    }

    /// Returns whether a verified identity is available.
    #[must_use]
    pub const fn is_available(&self) -> bool {
        self.identity.is_some()
    }
}

/// Deterministic identity resolver with injectable environment inputs.
#[derive(Clone, Debug)]
pub struct IdentityResolver {
    environment: BTreeMap<String, String>,
    home: Option<PathBuf>,
    query_git: bool,
    git_timeout: Duration,
}

impl IdentityResolver {
    /// Captures relevant process environment variables.
    #[must_use]
    pub fn from_process() -> Self {
        let relevant = [
            "HOME",
            "USERPROFILE",
            "APPDATA",
            "XDG_CONFIG_HOME",
            "PATH",
            "PATHEXT",
            CONFIG_ENV,
            LOGIN_ENV,
            EMAIL_ENV,
            LEGACY_CONFIG_ENV,
            LEGACY_LOGIN_ENV,
            LEGACY_EMAIL_ENV,
            "MAIL",
        ];
        let environment = relevant
            .into_iter()
            .filter_map(|key| env::var(key).ok().map(|value| (key.to_owned(), value)))
            .collect::<BTreeMap<_, _>>();
        let home = environment
            .get("HOME")
            .or_else(|| environment.get("USERPROFILE"))
            .filter(|value| !value.is_empty())
            .map(PathBuf::from);
        Self {
            environment,
            home,
            query_git: true,
            git_timeout: GIT_TIMEOUT,
        }
    }

    /// Creates an isolated resolver that performs no Git lookup.
    #[must_use]
    pub const fn isolated(home: Option<PathBuf>) -> Self {
        Self {
            environment: BTreeMap::new(),
            home,
            query_git: false,
            git_timeout: GIT_TIMEOUT,
        }
    }

    /// Adds or replaces one captured environment value.
    #[must_use]
    pub fn with_environment(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        let key = key.into();
        let value = value.into();
        if matches!(key.as_str(), "HOME" | "USERPROFILE") && !value.is_empty() {
            self.home = Some(PathBuf::from(&value));
        }
        self.environment.insert(key, value);
        self
    }

    /// Enables or disables effective Git `user.email` lookup.
    #[must_use]
    pub const fn with_git_lookup(mut self, enabled: bool) -> Self {
        self.query_git = enabled;
        self
    }

    /// Resolves identity using CLI, environment, INI, Git, MAIL and editor precedence.
    #[must_use]
    pub fn resolve(
        &self,
        cli_login: Option<&str>,
        cli_email: Option<&str>,
        cwd: &Path,
    ) -> IdentityResolution {
        let env_login = self
            .environment_value(LOGIN_ENV)
            .or_else(|| self.environment_value(LEGACY_LOGIN_ENV));
        let env_email = self
            .environment_value(EMAIL_ENV)
            .or_else(|| self.environment_value(LEGACY_EMAIL_ENV));

        if let Some(email) = cli_email {
            return identity_from_candidate(email, cli_login, "command line", false);
        }
        if let Some(email) = env_email {
            return identity_from_candidate(email, cli_login.or(env_login), "environment", false);
        }
        let (config_login, config_email) = configured_identity(self);
        if let Some(email) = config_email.as_deref() {
            return identity_from_candidate(
                email,
                cli_login.or(env_login).or(config_login.as_deref()),
                "user config",
                false,
            );
        }

        let requested_login = cli_login.or(env_login).or(config_login.as_deref());
        let mut rejected_sources = Vec::new();
        if self.query_git {
            let git_email = git_config_email(
                cwd,
                self.git_timeout,
                self.environment_value("PATH"),
                self.environment_value("PATHEXT"),
            )
            .filter(|email| canonical_42_email(email).is_some());
            if let Some(email) = git_email {
                let resolution =
                    identity_from_candidate(&email, requested_login, "Git config", true);
                if resolution.is_available() {
                    return resolution;
                }
                rejected_sources.push(resolution.source);
            }
        }

        if let Some(email) = self
            .environment_value("MAIL")
            .filter(|email| canonical_42_email(email).is_some())
        {
            let resolution =
                identity_from_candidate(email, requested_login, "MAIL environment variable", true);
            if resolution.is_available() {
                return resolution;
            }
            rejected_sources.push(resolution.source);
        }

        let candidates = self.home().map(saved_editor_emails).unwrap_or_default();
        let selected = select_saved_email(&candidates, requested_login);
        let Some(email) = selected else {
            let reason = if !candidates.is_empty() {
                "saved editor settings contain multiple 42 student emails, but none ".to_owned()
                    + "could be matched safely to the configured login"
            } else if rejected_sources.is_empty() {
                "no 42 student email was found in command settings, environment, Git, ".to_owned()
                    + "Vim/Neovim, or VS Code/Cursor settings"
            } else {
                rejected_sources.join("; ")
            };
            return IdentityResolution::unavailable("IDENTITY_NOT_FOUND", reason);
        };
        let sources = candidates.get(&email).map_or_else(
            || "editor settings".to_owned(),
            |items| items.iter().cloned().collect::<Vec<_>>().join(", "),
        );
        identity_from_candidate(&email, requested_login, &sources, true)
    }

    pub(super) fn environment_value(&self, key: &str) -> Option<&str> {
        self.environment
            .get(key)
            .map(String::as_str)
            .filter(|value| !value.is_empty())
    }

    pub(super) fn home(&self) -> Option<&Path> {
        self.home.as_deref()
    }
}

/// Resolves identity from the current process environment.
#[must_use]
pub fn resolve_identity(
    login: Option<&str>,
    email: Option<&str>,
    cwd: &Path,
) -> IdentityResolution {
    IdentityResolver::from_process().resolve(login, email, cwd)
}

#[cfg(test)]
mod tests;
