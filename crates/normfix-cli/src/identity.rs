//! 42 identity resolution, prompting, and persistence.

use std::io::{self, IsTerminal as _, Write as _};
use std::path::{Path, PathBuf};

use normfix_header::{IdentityResolution, identity_from_email, persist_identity, resolve_identity};
use normfix_project::ProjectFileKind;

use crate::cli::{Cli, OutputFormat, Workflow};
use crate::execution::terminal_safe_inline;

pub(super) fn scope_may_need_identity(paths: &[PathBuf], git_scoped: bool) -> bool {
    if paths.is_empty() {
        return !git_scoped;
    }
    paths.iter().any(|path| {
        if path.is_dir() {
            return true;
        }
        matches!(
            ProjectFileKind::from_path(path),
            Some(ProjectFileKind::CSource | ProjectFileKind::CHeader | ProjectFileKind::Makefile)
        )
    })
}

pub(super) const fn identity_prompt_allowed(
    format: OutputFormat,
    stdin_is_terminal: bool,
    stderr_is_terminal: bool,
) -> bool {
    matches!(format, OutputFormat::Human) && stdin_is_terminal && stderr_is_terminal
}

pub(super) fn resolve_run_identity(
    cli: &Cli,
    paths: &[PathBuf],
    git_scoped: bool,
    workflow: Workflow,
    cwd: &Path,
) -> (IdentityResolution, Option<String>) {
    let mut identity = resolve_identity(cli.login.as_deref(), cli.email.as_deref(), cwd);
    if identity.identity.is_none()
        && scope_may_need_identity(paths, git_scoped)
        && !matches!(workflow, Workflow::Lint | Workflow::Budget)
        && identity_prompt_allowed(
            cli.format,
            io::stdin().is_terminal(),
            io::stderr().is_terminal(),
        )
    {
        identity = prompt_for_identity(cli.login.as_deref(), identity);
    }
    let persistence = persist_requested_identity(cli, &identity);
    (identity, persistence)
}

fn persist_requested_identity(cli: &Cli, resolution: &IdentityResolution) -> Option<String> {
    let supplied_or_prompted = cli.email.is_some()
        || cli.login.is_some()
        || resolution
            .identity
            .as_ref()
            .is_some_and(|identity| identity.source == "interactive terminal");
    if !supplied_or_prompted {
        return None;
    }
    let identity = resolution.identity.as_ref()?;
    Some(match persist_identity(identity) {
        Ok(path) => format!(
            "42 identity saved with private file permissions for future runs at {}",
            path.display()
        ),
        Err(error) => format!("42 identity is valid for this run but could not be saved: {error}"),
    })
}

/// Asks for a valid 42 identity only on an interactive terminal.
pub(super) fn prompt_for_identity(
    requested_login: Option<&str>,
    fallback: IdentityResolution,
) -> IdentityResolution {
    loop {
        eprint!(
            "No verified 42 student email was found.\n\
             Enter your 42 email (Enter, cancel, or q to skip the header): "
        );
        let _ = io::stderr().flush();
        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_err() {
            eprintln!("\nHeader skipped; all other safe fixes will continue.");
            return fallback;
        }
        let value = input.trim();
        if value.is_empty()
            || value.eq_ignore_ascii_case("cancel")
            || value.eq_ignore_ascii_case("q")
        {
            eprintln!("Header skipped; its absence will be included in the report.");
            return fallback;
        }
        let resolution = identity_from_email(value, requested_login, "interactive terminal");
        if resolution.identity.is_some() {
            return resolution;
        }
        eprintln!(
            "{}. Use an address such as login@student.42.fr, or cancel.",
            terminal_safe_inline(&resolution.source)
        );
    }
}
