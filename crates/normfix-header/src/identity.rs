//! Ambiguity-safe 42 identity discovery.

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::time::Duration;

use regex::Regex;
use wait_timeout::ChildExt;

use crate::{ByteRange, Issue};

const SETTINGS_SIZE_LIMIT: u64 = 1_000_000;
const GIT_TIMEOUT: Duration = Duration::from_secs(2);

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
    fn available(identity: Identity42) -> Self {
        let source = identity.source.clone();
        Self {
            identity: Some(identity),
            source,
            issue: None,
        }
    }

    fn unavailable(code: &'static str, source: String) -> Self {
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
            "XDG_CONFIG_HOME",
            "NORMINETTE_FIX_CONFIG",
            "NORMINETTE_FIX_LOGIN",
            "NORMINETTE_FIX_EMAIL",
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
    pub fn isolated(home: Option<PathBuf>) -> Self {
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
        let env_login = self.environment_value("NORMINETTE_FIX_LOGIN");
        let env_email = self.environment_value("NORMINETTE_FIX_EMAIL");
        let (config_login, config_email) = self.configured_identity();

        if let Some(email) = cli_email {
            return identity_from_candidate(email, cli_login, "command line", false);
        }
        if let Some(email) = env_email {
            return identity_from_candidate(email, cli_login.or(env_login), "environment", false);
        }
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
            let git_email = git_config_email(cwd, self.git_timeout)
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

        let candidates = self.saved_editor_emails();
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

    fn environment_value(&self, key: &str) -> Option<&str> {
        self.environment
            .get(key)
            .map(String::as_str)
            .filter(|value| !value.is_empty())
    }

    fn configured_identity(&self) -> (Option<String>, Option<String>) {
        let path = if let Some(configured) = self.environment_value("NORMINETTE_FIX_CONFIG") {
            expand_home(configured, self.home.as_deref())
        } else {
            let base = self
                .environment_value("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .or_else(|| self.home.as_ref().map(|home| home.join(".config")));
            let Some(base) = base else {
                return (None, None);
            };
            base.join("norminette-fix").join("config.ini")
        };
        parse_header_ini(&path).unwrap_or((None, None))
    }

    fn saved_editor_emails(&self) -> BTreeMap<String, BTreeSet<String>> {
        let Some(home) = self.home.as_deref() else {
            return BTreeMap::new();
        };
        let locations = editor_locations(home);
        let mut candidates = BTreeMap::<String, BTreeSet<String>>::new();
        for (path, pattern, source) in locations {
            let Ok(metadata) = fs::metadata(&path) else {
                continue;
            };
            if !metadata.is_file() || metadata.len() > SETTINGS_SIZE_LIMIT {
                continue;
            }
            let Ok(bytes) = fs::read(&path) else {
                continue;
            };
            let content = String::from_utf8_lossy(&bytes);
            let regex = Regex::new(pattern).expect("editor email regex is constant");
            for captures in regex.captures_iter(&content) {
                let Some(value) = captures.get(1) else {
                    continue;
                };
                let Some(email) = canonical_42_email(value.as_str()) else {
                    continue;
                };
                candidates
                    .entry(email)
                    .or_default()
                    .insert(source.to_owned());
            }
        }
        candidates
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

/// Validates and canonicalizes a supported 42 student email.
///
/// # Panics
///
/// Caller input cannot cause a panic. Initialization would panic only if the
/// built-in regular-expression literal were invalid.
#[must_use]
pub fn canonical_42_email(value: &str) -> Option<String> {
    static EMAIL: OnceLock<Regex> = OnceLock::new();
    let regex = EMAIL.get_or_init(|| {
        Regex::new(
            r"(?i)^([A-Za-z0-9][A-Za-z0-9._-]*)@(42\.fr|student\.42[A-Za-z0-9-]*(?:\.[A-Za-z0-9-]+)+)$",
        )
        .expect("42 email regex is constant")
    });
    let captures = regex.captures(value.trim())?;
    Some(format!(
        "{}@{}",
        captures.get(1)?.as_str().to_ascii_lowercase(),
        captures.get(2)?.as_str().to_ascii_lowercase()
    ))
}

/// Validates one supplied email and derives its matching 42 login.
///
/// This pure helper is suitable for interactive terminal input: it performs no
/// environment, filesystem or Git lookup.
#[must_use]
pub fn identity_from_email(
    email: &str,
    requested_login: Option<&str>,
    source: &str,
) -> IdentityResolution {
    identity_from_candidate(email, requested_login, source, false)
}

fn identity_from_candidate(
    email: &str,
    requested_login: Option<&str>,
    source: &str,
    inferred: bool,
) -> IdentityResolution {
    let Some(canonical) = canonical_42_email(email) else {
        return IdentityResolution::unavailable(
            "IDENTITY_INVALID_EMAIL",
            format!("{source} does not contain a valid 42 student email"),
        );
    };
    let email_login = canonical
        .split_once('@')
        .map_or(canonical.as_str(), |(login, _)| login);
    if requested_login.is_some_and(|login| !login.eq_ignore_ascii_case(email_login)) {
        return IdentityResolution::unavailable(
            "IDENTITY_LOGIN_MISMATCH",
            format!(
                "{source} contains {canonical}, which does not match the configured login {}",
                requested_login.unwrap_or_default()
            ),
        );
    }
    IdentityResolution::available(Identity42 {
        login: email_login.to_owned(),
        email: canonical,
        source: source.to_owned(),
        inferred_login: inferred && requested_login.is_none(),
        inferred_email: inferred,
    })
}

fn select_saved_email(
    candidates: &BTreeMap<String, BTreeSet<String>>,
    requested_login: Option<&str>,
) -> Option<String> {
    if let Some(login) = requested_login {
        let mut matches = candidates.keys().filter(|email| {
            email
                .split_once('@')
                .is_some_and(|(local, _)| local.eq_ignore_ascii_case(login))
        });
        let selected = matches.next()?.clone();
        return matches.next().is_none().then_some(selected);
    }
    (candidates.len() == 1)
        .then(|| candidates.keys().next().cloned())
        .flatten()
}

fn git_config_email(cwd: &Path, timeout: Duration) -> Option<String> {
    let mut child = Command::new("git")
        .args(["config", "--get", "user.email"])
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let Some(status) = child.wait_timeout(timeout).ok()? else {
        let _ = child.kill();
        let _ = child.wait();
        return None;
    };
    if !status.success() {
        return None;
    }
    let mut output = String::new();
    child
        .stdout
        .take()?
        .take(4096)
        .read_to_string(&mut output)
        .ok()?;
    let value = output.trim();
    (!value.is_empty()).then(|| value.to_owned())
}

fn parse_header_ini(path: &Path) -> Option<(Option<String>, Option<String>)> {
    let metadata = fs::metadata(path).ok()?;
    if !metadata.is_file() || metadata.len() > SETTINGS_SIZE_LIMIT {
        return None;
    }
    let content = fs::read_to_string(path).ok()?;
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

fn editor_locations(home: &Path) -> Vec<(PathBuf, &'static str, &'static str)> {
    let vim = r#"\bg:mail42\s*=\s*['"]([^'"]+)['"]"#;
    let lua = r#"\bvim\.g\.mail42\s*=\s*['"]([^'"]+)['"]"#;
    let shell = r#"(?m)^[ \t]*(?:export[ \t]+)?MAIL\s*=\s*['"]?([^'"\s#]+)"#;
    let json = r#""42header\.email"\s*:\s*"([^"]+)""#;
    vec![
        (home.join(".vimrc"), vim, "Vim settings"),
        (home.join(".config/nvim/init.vim"), vim, "Neovim settings"),
        (home.join(".config/nvim/init.lua"), lua, "Neovim settings"),
        (home.join(".zshrc"), shell, "shell settings"),
        (home.join(".zprofile"), shell, "shell settings"),
        (home.join(".bashrc"), shell, "shell settings"),
        (home.join(".bash_profile"), shell, "shell settings"),
        (
            home.join("Library/Application Support/Code/User/settings.json"),
            json,
            "VS Code settings",
        ),
        (
            home.join("Library/Application Support/Cursor/User/settings.json"),
            json,
            "Cursor settings",
        ),
        (
            home.join(".config/Code/User/settings.json"),
            json,
            "VS Code settings",
        ),
        (
            home.join(".config/VSCodium/User/settings.json"),
            json,
            "VSCodium settings",
        ),
        (
            home.join(".config/Cursor/User/settings.json"),
            json,
            "Cursor settings",
        ),
    ]
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::{IdentityResolver, canonical_42_email};

    #[test]
    fn validates_supported_campus_domains_and_canonicalizes_case() {
        assert_eq!(
            canonical_42_email("Student-A@Student.42Berlin.DE"),
            Some("student-a@student.42berlin.de".to_owned())
        );
        assert!(canonical_42_email("student@example.com").is_none());
        assert!(canonical_42_email("bad%login@student.42.fr").is_none());
    }

    #[test]
    fn explicit_email_has_precedence_and_must_match_explicit_login() {
        let temporary = TempDir::new().expect("temporary directory");
        let resolver = IdentityResolver::isolated(Some(temporary.path().to_path_buf()))
            .with_environment("NORMINETTE_FIX_EMAIL", "env@student.42.fr");
        let resolution = resolver.resolve(Some("cli"), Some("cli@student.42.fr"), temporary.path());
        let identity = resolution.identity.expect("valid identity");
        assert_eq!(identity.login, "cli");
        assert!(!identity.inferred());
    }

    #[test]
    fn invalid_explicit_email_never_falls_through_to_a_lower_precedence_source() {
        let temporary = TempDir::new().expect("temporary directory");
        let resolver = IdentityResolver::isolated(Some(temporary.path().to_path_buf()))
            .with_environment("NORMINETTE_FIX_EMAIL", "valid@student.42.fr");
        let resolution = resolver.resolve(None, Some("not-a-42-address"), temporary.path());
        assert!(!resolution.is_available());
        assert_eq!(
            resolution.issue.expect("invalid email issue").code,
            "IDENTITY_INVALID_EMAIL"
        );
    }

    #[test]
    fn config_precedes_editor_settings() {
        let temporary = TempDir::new().expect("temporary directory");
        let config = temporary.path().join("config.ini");
        fs::write(
            &config,
            "[header]\nlogin = configured\nemail = configured@student.42.fr\n",
        )
        .expect("config");
        fs::write(
            temporary.path().join(".vimrc"),
            "let g:mail42 = 'editor@student.42.fr'\n",
        )
        .expect("vimrc");
        let resolver = IdentityResolver::isolated(Some(temporary.path().to_path_buf()))
            .with_environment("NORMINETTE_FIX_CONFIG", config.to_string_lossy());

        let resolution = resolver.resolve(None, None, temporary.path());

        assert_eq!(
            resolution.identity.expect("configured identity").login,
            "configured"
        );
    }

    #[test]
    fn ambiguous_duplicate_config_values_are_rejected_as_a_unit() {
        let temporary = TempDir::new().expect("temporary directory");
        let config = temporary.path().join("config.ini");
        fs::write(
            &config,
            concat!(
                "[header]\n",
                "email = first@student.42.fr\n",
                "email = second@student.42.fr\n"
            ),
        )
        .expect("config");
        fs::write(
            temporary.path().join(".vimrc"),
            "let g:mail42 = 'editor@student.42.fr'\n",
        )
        .expect("vimrc");
        let resolver = IdentityResolver::isolated(Some(temporary.path().to_path_buf()))
            .with_environment("NORMINETTE_FIX_CONFIG", config.to_string_lossy());

        let resolution = resolver.resolve(None, None, temporary.path());

        assert_eq!(
            resolution
                .identity
                .expect("unambiguous editor identity")
                .email,
            "editor@student.42.fr"
        );
    }

    #[test]
    fn ambiguous_editor_emails_are_never_guessed() {
        let temporary = TempDir::new().expect("temporary directory");
        fs::write(
            temporary.path().join(".vimrc"),
            "let g:mail42 = 'first@student.42.fr'\n",
        )
        .expect("vimrc");
        let settings = temporary
            .path()
            .join("Library/Application Support/Code/User/settings.json");
        fs::create_dir_all(settings.parent().expect("settings parent")).expect("settings dir");
        fs::write(settings, r#"{"42header.email":"second@student.42.fr"}"#).expect("settings");
        let resolver = IdentityResolver::isolated(Some(temporary.path().to_path_buf()));

        let resolution = resolver.resolve(None, None, temporary.path());

        assert!(!resolution.is_available());
        assert!(resolution.source.contains("multiple 42 student emails"));
    }

    #[test]
    fn requested_login_selects_one_of_multiple_saved_emails() {
        let temporary = TempDir::new().expect("temporary directory");
        fs::write(
            temporary.path().join(".vimrc"),
            "let g:mail42 = 'first@student.42.fr'\n",
        )
        .expect("vimrc");
        let settings = temporary
            .path()
            .join("Library/Application Support/Code/User/settings.json");
        fs::create_dir_all(settings.parent().expect("settings parent")).expect("settings dir");
        fs::write(settings, r#"{"42header.email":"second@student.42.fr"}"#).expect("settings");
        let resolver = IdentityResolver::isolated(Some(temporary.path().to_path_buf()));

        let resolution = resolver.resolve(Some("second"), None, temporary.path());

        assert_eq!(
            resolution.identity.expect("matched identity").email,
            "second@student.42.fr"
        );
    }

    #[test]
    fn injected_home_and_lossy_editor_read_are_supported() {
        let temporary = TempDir::new().expect("temporary directory");
        let mut vimrc = b"let g:mail42 = 'lossy@student.42.fr'\n".to_vec();
        vimrc.extend_from_slice(&[0xff, b'\n']);
        fs::write(temporary.path().join(".vimrc"), vimrc).expect("vimrc");
        let resolver = IdentityResolver::isolated(None)
            .with_environment("HOME", temporary.path().to_string_lossy());

        let resolution = resolver.resolve(None, None, temporary.path());

        assert_eq!(
            resolution.identity.expect("editor identity").email,
            "lossy@student.42.fr"
        );
    }

    #[test]
    fn mail_environment_precedes_saved_editor_settings() {
        let temporary = TempDir::new().expect("temporary directory");
        fs::write(
            temporary.path().join(".vimrc"),
            "let g:mail42 = 'editor@student.42.fr'\n",
        )
        .expect("vimrc");
        let resolver = IdentityResolver::isolated(Some(temporary.path().to_path_buf()))
            .with_environment("MAIL", "mail@student.42.fr");

        let resolution = resolver.resolve(None, None, temporary.path());

        let identity = resolution.identity.expect("MAIL identity");
        assert_eq!(identity.email, "mail@student.42.fr");
        assert_eq!(identity.source, "MAIL environment variable");
    }
}
