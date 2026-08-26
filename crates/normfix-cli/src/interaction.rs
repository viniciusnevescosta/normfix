//! Terminal confirmations and per-file interactive approval.

use std::collections::BTreeMap;
use std::io::{self, IsTerminal as _, Write as _};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use normfix_destructive::{DestructiveAuthorization, DestructiveCapability, DestructiveRequest};
use normfix_engine::{FixOptions, WriteApproval, run_fixes};
use normfix_header::RunClock;
use normfix_report::{FileReport, ReportMode, unified_diff};

use crate::DestructiveFlags;
use crate::cli::{Cli, OutputFormat, Workflow};
use crate::execution::terminal_safe_inline;
use crate::presentation::{cli_messages, print_run_error, render_report};

pub(super) fn run_interactive(cli: &Cli, paths: &[PathBuf], options: &FixOptions) -> ExitCode {
    if cli.format != OutputFormat::Human
        || !io::stdin().is_terminal()
        || !io::stdout().is_terminal()
        || !io::stderr().is_terminal()
    {
        print_run_error(
            cli.format,
            cli_messages(cli),
            "--interactive requires a human terminal on standard input, output, and error",
        );
        return ExitCode::from(2);
    }
    if options.mode != ReportMode::Fix || options.lint_only {
        print_run_error(
            cli.format,
            cli_messages(cli),
            "--interactive is available with the default or `format` workflow, without --check or --diff",
        );
        return ExitCode::from(2);
    }
    if options.remove_invalid_comments
        || options.remove_unused_variables
        || options.compact_null_checks
        || options.remove_missing_makefile_sources
        || options.remove_unused_static
        || options.quarantine_unexpected
    {
        print_run_error(
            cli.format,
            cli_messages(cli),
            "--interactive cannot be combined with destructive or --unsafe operations",
        );
        return ExitCode::from(2);
    }

    let mut preview_options = options.clone();
    preview_options.mode = ReportMode::Check;
    preview_options.write_approvals = None;
    preview_options.run_clock = match RunClock::from_process_environment() {
        Ok(clock) => Some(clock),
        Err(error) => {
            print_run_error(
                cli.format,
                cli_messages(cli),
                &format!("Could not capture the interactive run clock: {error}"),
            );
            return ExitCode::from(2);
        }
    };
    let preview = match run_fixes(paths, &preview_options) {
        Ok(report) => report,
        Err(error) => {
            print_run_error(cli.format, cli_messages(cli), &error.to_string());
            return ExitCode::from(2);
        }
    };
    if let Err(message) = render_report(cli, &preview, Workflow::Check) {
        print_run_error(cli.format, cli_messages(cli), &message);
        return ExitCode::from(2);
    }
    let candidates = preview
        .files
        .iter()
        .filter(|file| file.changed && file.failure.is_none() && unified_diff(file).is_some())
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return ExitCode::from(preview.exit_code());
    }

    let Some(selected) = prompt_for_approvals(&candidates, &options.cwd) else {
        return ExitCode::from(130);
    };
    if selected.is_empty() {
        eprintln!("No files were approved; no files were changed.");
        return ExitCode::from(preview.exit_code().max(1));
    }

    let declined = selected.len() < candidates.len();
    let mut final_options = options.clone();
    final_options.write_approvals = Some(selected);
    final_options.run_clock = preview_options.run_clock;
    let report = match run_fixes(paths, &final_options) {
        Ok(report) => report,
        Err(error) => {
            print_run_error(cli.format, cli_messages(cli), &error.to_string());
            return ExitCode::from(2);
        }
    };
    if let Err(message) = render_report(cli, &report, Workflow::Default) {
        print_run_error(cli.format, cli_messages(cli), &message);
        return ExitCode::from(2);
    }
    let code = report.exit_code();
    ExitCode::from(if code == 0 && declined { 1 } else { code })
}

/// Collects per-file approvals, or `None` when the run was cancelled.
fn prompt_for_approvals(
    candidates: &[&FileReport],
    cwd: &Path,
) -> Option<BTreeMap<PathBuf, WriteApproval>> {
    let mut selected = BTreeMap::new();
    'files: for (index, file) in candidates.iter().enumerate() {
        if let Some(diff) = unified_diff(file) {
            println!("\n{diff}");
        }
        loop {
            eprint!(
                "Apply the validated change to {}? [y/N/a(all)/q(cancel)] ",
                terminal_safe_inline(file.path.as_str())
            );
            let _ = io::stderr().flush();
            let mut answer = String::new();
            if io::stdin().read_line(&mut answer).is_err() {
                eprintln!("Interactive run cancelled; no files were changed.");
                return None;
            }
            match answer.trim().to_ascii_lowercase().as_str() {
                "y" | "yes" => {
                    if let Some((path, approval)) = interactive_approval(cwd, file) {
                        selected.insert(path, approval);
                    }
                    break;
                }
                "" | "n" | "no" => break,
                "a" | "all" => {
                    selected.extend(
                        candidates[index..]
                            .iter()
                            .filter_map(|candidate| interactive_approval(cwd, candidate)),
                    );
                    break 'files;
                }
                "q" | "quit" | "cancel" => {
                    eprintln!("Interactive run cancelled; no files were changed.");
                    return None;
                }
                _ => eprintln!("Please enter y, n, a, or q."),
            }
        }
    }
    Some(selected)
}

fn interactive_absolute_path(cwd: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    }
}

fn interactive_approval(
    cwd: &Path,
    file: &normfix_report::FileReport,
) -> Option<(PathBuf, WriteApproval)> {
    let original = file.original.as_deref()?;
    let fixed = file.fixed.as_deref()?;
    Some((
        interactive_absolute_path(cwd, file.path.as_std_path()),
        WriteApproval::new(original.as_bytes(), fixed.as_bytes()),
    ))
}

pub(super) fn authorize_destructive(
    destructive: DestructiveFlags,
    force: bool,
    messages: &normfix_i18n::Messages,
    format: OutputFormat,
) -> Result<Option<DestructiveAuthorization>, String> {
    let mut capabilities = Vec::new();
    if destructive.remove_unused {
        capabilities.push(DestructiveCapability::RemoveUnreferencedStaticFunctions);
    }
    if destructive.remove_missing_makefile_sources {
        capabilities.push(DestructiveCapability::RemoveMissingMakefileSources);
    }
    if destructive.remove_orphan_prototypes {
        capabilities.push(DestructiveCapability::RemoveOrphanPrototypes);
    }
    if destructive.remove_unexpected {
        capabilities.push(DestructiveCapability::QuarantineUnexpectedFiles);
    }
    if capabilities.is_empty() {
        return Ok(None);
    }
    let request = DestructiveRequest::new(capabilities).map_err(|error| error.to_string())?;
    if force {
        return request
            .authorize_forced(true, true)
            .map(Some)
            .map_err(|error| error.to_string());
    }
    if format == OutputFormat::Json || !io::stdin().is_terminal() || !io::stderr().is_terminal() {
        return Err(messages.destructive_needs_confirmation.to_owned());
    }
    eprintln!("{}", messages.destructive_warning);
    eprint!("{}", messages.destructive_prompt);
    let _ = io::stderr().flush();
    let mut answer = String::new();
    // `y` is the accepted answer in every language; see `confirm_undo`.
    let confirmed = io::stdin()
        .read_line(&mut answer)
        .is_ok_and(|_| answer.trim().eq_ignore_ascii_case("y"));
    request
        .authorize_yes(confirmed)
        .map(Some)
        .map_err(|_| messages.destructive_cancelled.to_owned())
}
