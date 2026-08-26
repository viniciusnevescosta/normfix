//! Human and machine-readable command output.

use std::env;
use std::io::{self, IsTerminal as _};

use normfix_report::{RenderOptions, ReportMode, RunReport, render_human};

use crate::cli::{Cli, OutputFormat, Workflow};
use crate::execution::terminal_safe_inline;

pub(super) fn resolve_locale(cli: &Cli) -> normfix_i18n::Resolution {
    normfix_i18n::resolve(cli.lang.as_deref(), |name| std::env::var(name).ok())
}

/// Returns the language this invocation's human output uses.
///
/// JSON is machine output and stays English whatever the reader's language is.
pub(super) fn cli_locale(cli: &Cli) -> normfix_i18n::Locale {
    match cli.format {
        OutputFormat::Human => resolve_locale(cli).locale,
        OutputFormat::Json => normfix_i18n::Locale::English,
    }
}

/// Returns the catalogue for this invocation's output.
pub(super) fn cli_messages(cli: &Cli) -> &'static normfix_i18n::Messages {
    normfix_i18n::messages(cli_locale(cli))
}

pub(super) const fn workflow_name(cli: &Cli, workflow: Workflow) -> &'static str {
    match workflow {
        Workflow::Default | Workflow::Format if cli.diff => "diff",
        Workflow::Default | Workflow::Format if cli.check => "check",
        Workflow::Default | Workflow::Format => "format",
        Workflow::Lint => "lint",
        Workflow::Check => "check",
        Workflow::Budget => "budget",
        Workflow::Preflight => "preflight",
    }
}

pub(super) const fn report_mode_name(
    mode: ReportMode,
    messages: &normfix_i18n::Messages,
) -> &'static str {
    match mode {
        ReportMode::Fix => messages.mode_write,
        ReportMode::Check => messages.mode_check,
        ReportMode::Diff => messages.mode_diff,
    }
}

pub(super) fn render_report(
    cli: &Cli,
    report: &RunReport,
    workflow: Workflow,
) -> Result<(), String> {
    match cli.format {
        OutputFormat::Human => {
            let color =
                !cli.no_color && env::var_os("NO_COLOR").is_none() && io::stdout().is_terminal();
            print!(
                "{}",
                render_human(
                    report,
                    RenderOptions {
                        color,
                        verbose: cli.verbose,
                        show_diff: cli.diff,
                        locale: resolve_locale(cli).locale,
                    },
                )
            );
            Ok(())
        }
        OutputFormat::Json => {
            let mut value = serde_json::to_value(report)
                .map_err(|error| format!("Could not serialize the run report: {error}"))?;
            // Asking for a diff and being handed a mode label is not an answer.
            // The report leaves diffs out by default because they double its
            // size for a reader who did not ask; `--diff` is the reader asking.
            if cli.diff {
                attach_unified_diffs(&mut value, report);
            }
            attach_command(&mut value, cli, workflow);
            attach_granted_capabilities(&mut value, cli);
            attach_scope(&mut value, cli);
            let json = serde_json::to_string_pretty(&value)
                .map_err(|error| format!("Could not serialize the run report: {error}"))?;
            println!("{json}");
            Ok(())
        }
    }
}

/// Puts each file's unified diff beside its entry, when one was asked for.
fn attach_unified_diffs(value: &mut serde_json::Value, report: &RunReport) {
    let Some(files) = value
        .get_mut("files")
        .and_then(serde_json::Value::as_array_mut)
    else {
        return;
    };
    for (entry, file) in files.iter_mut().zip(&report.files) {
        let Some(object) = entry.as_object_mut() else {
            continue;
        };
        object.insert(
            "diff".to_owned(),
            normfix_report::unified_diff(file).map_or(serde_json::Value::Null, |diff| {
                serde_json::Value::String(diff)
            }),
        );
    }
}

/// Names which command produced this answer.
///
/// `mode` says whether a run wrote, checked, or diffed; it does not say whether
/// `budget` or `lint` asked. A caller holding a payload should be able to tell
/// what produced it without having kept the command line that did.
fn attach_command(value: &mut serde_json::Value, cli: &Cli, workflow: Workflow) {
    if let Some(object) = value.as_object_mut() {
        object.insert(
            "command".to_owned(),
            serde_json::Value::String(workflow_name(cli, workflow).to_owned()),
        );
    }
}

/// Names how the run chose the files it worked on.
///
/// The file list alone does not say whether Git selected it or a directory
/// walk did, and the two mean different things to a caller deciding what a
/// clean result covers. The human banner has always said this; the JSON is
/// where a caller reads it.
fn attach_scope(value: &mut serde_json::Value, cli: &Cli) {
    let Some(object) = value.as_object_mut() else {
        return;
    };
    let selection = if cli.staged {
        "git_staged"
    } else if cli.changed {
        "git_changed"
    } else if cli.paths.is_empty() && cli.command.is_none() {
        "working_directory"
    } else {
        "explicit_paths"
    };
    object.insert(
        "scope".to_owned(),
        serde_json::json!({
            "selection": selection,
            "respects_gitignore": cli.use_gitignore,
        }),
    );
}

/// Names the destructive capabilities this run was granted.
///
/// A caller deciding whether to trust a result has to know what the run was
/// allowed to do, and reading that back from the flags it passed is only
/// possible for the caller that passed them. An empty list is the answer for
/// an ordinary run, and is present rather than omitted so its absence never
/// has to be interpreted.
fn attach_granted_capabilities(value: &mut serde_json::Value, cli: &Cli) {
    let Some(object) = value.as_object_mut() else {
        return;
    };
    let mut granted = Vec::new();
    if cli.unsafe_mode {
        granted.push("unsafe");
    }
    if cli.remove_invalid_comments {
        granted.push("remove_invalid_comments");
    }
    if cli.remove_unused {
        granted.push("remove_unused");
    }
    if cli.remove_unexpected {
        granted.push("remove_unexpected");
    }
    if cli.force {
        granted.push("force");
    }
    object.insert(
        "granted_capabilities".to_owned(),
        serde_json::Value::Array(
            granted
                .into_iter()
                .map(|name| serde_json::Value::String(name.to_owned()))
                .collect(),
        ),
    );
}

pub(super) fn print_json_outcome(command: &str, outcome: &str, payload: &serde_json::Value) {
    let value = serde_json::json!({
        "schema_version": normfix_report::REPORT_SCHEMA_VERSION,
        "command": command,
        "outcome": outcome,
        "result": payload,
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&value).expect("the command outcome schema is serializable")
    );
}

pub(super) fn print_run_error(
    format: OutputFormat,
    messages: &normfix_i18n::Messages,
    message: &str,
) {
    match format {
        OutputFormat::Human => {
            eprintln!("normfix");
            eprintln!("error: {}", terminal_safe_inline(message));
            eprintln!("{}", messages.error_nothing_written);
        }
        OutputFormat::Json => {
            // The same envelope a successful command answers with, so a caller
            // reads `outcome` in one place instead of learning two shapes.
            let value = serde_json::json!({
                "schema_version": normfix_report::REPORT_SCHEMA_VERSION,
                "outcome": "failure",
                "error": {
                    "code": "run_error",
                    "message": message,
                }
            });
            println!(
                "{}",
                serde_json::to_string_pretty(&value)
                    .expect("the static error schema is serializable")
            );
        }
    }
}
