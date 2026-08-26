//! Pure validation and ambiguity-safe candidate selection.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use regex::Regex;

use super::{Identity42, IdentityResolution};

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

pub(super) fn identity_from_candidate(
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

pub(super) fn select_saved_email(
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
