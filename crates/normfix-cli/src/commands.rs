//! Standalone commands that do not enter the project formatting pipeline.

use std::env;
use std::io::{self, IsTerminal as _, Write as _};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use camino::Utf8PathBuf;
use normfix_actions::{UndoRun, list_undo_runs, undo_run};

use crate::cli::{Cli, Command, LeaksArguments, OutputFormat, UndoArguments, UninstallArguments};
use crate::presentation::{
    cli_locale, cli_messages, print_json_outcome, print_run_error, resolve_locale,
};
use crate::{rules, uninstall, upgrade};

pub(super) fn run_standalone_command(cli: &Cli, cwd: &Path) -> Option<ExitCode> {
    match &cli.command {
        Some(Command::Explain(arguments)) => Some(run_explain(
            cli.format,
            cli_locale(cli),
            cli_messages(cli),
            &arguments.rule,
        )),
        Some(Command::Undo(arguments)) => Some(run_undo(cli, arguments, cwd)),
        Some(Command::Upgrade(arguments)) => {
            Some(run_upgrade(cli.format, cli_messages(cli), arguments.check))
        }
        Some(Command::Uninstall(arguments)) => Some(run_uninstall(cli, arguments)),
        Some(Command::Leaks(arguments)) => Some(run_leaks(cli, arguments)),
        _ => None,
    }
}
fn run_uninstall(cli: &Cli, arguments: &UninstallArguments) -> ExitCode {
    let messages = cli_messages(cli);
    let plan = match uninstall::plan(arguments.purge) {
        Ok(plan) => plan,
        Err(message) => {
            print_run_error(cli.format, messages, &message);
            return ExitCode::from(2);
        }
    };

    if cli.format == OutputFormat::Json {
        print_json_outcome(
            "uninstall",
            if arguments.dry_run {
                "planned"
            } else {
                "success"
            },
            &serde_json::json!({
                "dry_run": arguments.dry_run,
                "purge": arguments.purge,
                "removes_recovery_data": plan.removes_recovery_data(),
                "plan": uninstall::describe(&plan),
            }),
        );
    } else {
        eprint!("{}", uninstall::describe(&plan));
        if plan.removes_recovery_data() {
            eprintln!("{}", messages.uninstall_recovery_warning);
        }
    }
    if arguments.dry_run {
        return ExitCode::SUCCESS;
    }

    if let Err(message) = confirm_uninstall(cli, messages) {
        print_run_error(cli.format, messages, &message);
        return ExitCode::from(2);
    }

    match uninstall::remove(&plan) {
        Ok(()) => {
            if cli.format != OutputFormat::Json {
                eprintln!("{}", messages.uninstall_done);
            }
            ExitCode::SUCCESS
        }
        Err(message) => {
            print_run_error(cli.format, messages, &message);
            ExitCode::from(2)
        }
    }
}

fn confirm_uninstall(cli: &Cli, messages: &normfix_i18n::Messages) -> Result<(), String> {
    if cli.force {
        return Ok(());
    }
    if cli.format == OutputFormat::Json || !io::stdin().is_terminal() || !io::stderr().is_terminal()
    {
        return Err(messages.uninstall_needs_confirmation.to_owned());
    }
    eprint!("{}", messages.uninstall_prompt);
    let _ = io::stderr().flush();
    let mut answer = String::new();
    // `y` is the accepted answer in every language; see `confirm_undo`.
    let confirmed = io::stdin()
        .read_line(&mut answer)
        .is_ok_and(|_| answer.trim().eq_ignore_ascii_case("y"));
    confirmed
        .then_some(())
        .ok_or_else(|| messages.uninstall_cancelled.to_owned())
}

/// Runs a program the student already built, under the leak checker.
///
/// The confirmation is not ceremony. Every other command reads source, and
/// reading source cannot delete a file or open a socket; this one executes a
/// binary, which can do both. So it says which program, and waits — unless
/// `--force` was given, which is how a script says it meant it.
fn run_leaks(cli: &Cli, arguments: &LeaksArguments) -> ExitCode {
    let messages = cli_messages(cli);
    let checker =
        match normfix_oracle::ValgrindChecker::locate(normfix_oracle::ValgrindConfig::default()) {
            Ok(checker) => checker,
            Err(error) => {
                print_run_error(
                    cli.format,
                    messages,
                    &format!(
                        "{} {} ({error})",
                        messages.leaks_unavailable,
                        leaks_install_hint(messages)
                    ),
                );
                return ExitCode::from(2);
            }
        };

    if let Err(refusal) = confirm_leaks(cli, messages, &arguments.program) {
        print_run_error(cli.format, messages, &refusal);
        return ExitCode::from(2);
    }

    let report = match checker.check(&arguments.program, &arguments.program_arguments) {
        Ok(report) => report,
        Err(error) => {
            print_run_error(cli.format, messages, &error.to_string());
            return ExitCode::from(2);
        }
    };

    if cli.format == OutputFormat::Json {
        match serde_json::to_value(&report) {
            Ok(payload) => print_json_outcome(
                "leaks",
                if report.lost_anything() {
                    "findings"
                } else {
                    "success"
                },
                &payload,
            ),
            Err(error) => {
                print_run_error(cli.format, messages, &error.to_string());
                return ExitCode::from(2);
            }
        }
    } else {
        if report.lost_anything() {
            println!(
                "{}",
                normfix_i18n::fill(
                    messages.leaks_lost,
                    &[
                        ("definite", &report.definitely_lost_bytes.to_string()),
                        ("indirect", &report.indirectly_lost_bytes.to_string()),
                    ],
                )
            );
        } else {
            println!("{}", messages.leaks_none);
        }
        print_leak_findings(&report, cli, messages);
        // The checker's own total can exceed what could be placed in a file,
        // and repeating a number the list above already showed reads as a
        // second finding.
        let unlisted = report
            .error_count
            .saturating_sub(report.errors.len().try_into().unwrap_or(u64::MAX));
        if unlisted > 0 {
            println!(
                "{}",
                normfix_i18n::fill_plural(
                    cli_locale(cli),
                    unlisted,
                    messages.leaks_errors_one,
                    messages.leaks_errors_other,
                    &[("count", &unlisted.to_string())],
                )
            );
        }
        println!("{}", messages.leaks_not_a_proof);
    }

    // A leak is a finding about the project, which is what exit 1 means
    // everywhere else in this tool. It is not an operational failure.
    if report.lost_anything() {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

/// Shows every finding under the line of source that produced it.
///
/// The checker names a file and a line; this reads that file and hands the
/// finding to the same renderer every other diagnostic goes through, so a leak
/// gets the caret a Norm error gets. A file the checker named but this process
/// cannot read falls back to the plain list rather than losing the finding.
fn print_leak_findings(
    report: &normfix_oracle::ValgrindReport,
    cli: &Cli,
    messages: &normfix_i18n::Messages,
) {
    let mut sources: std::collections::BTreeMap<Utf8PathBuf, String> =
        std::collections::BTreeMap::new();
    let mut diagnostics: Vec<(Utf8PathBuf, normfix_core::Diagnostic)> = Vec::new();
    let mut unplaced = Vec::new();

    for site in &report.sites {
        let text = normfix_i18n::fill(
            if site.indirect {
                messages.leaks_site_indirect
            } else {
                messages.leaks_site_direct
            },
            &[
                ("bytes", &site.bytes.to_string()),
                ("function", &site.function),
            ],
        );
        match leak_diagnostic(
            site.location.as_ref(),
            if site.indirect {
                "LEAK_INDIRECTLY_LOST"
            } else {
                "LEAK_DEFINITELY_LOST"
            },
            &text,
            messages.leaks_site_help,
            &mut sources,
        ) {
            Some(placed) => diagnostics.push(placed),
            None => unplaced.push(text),
        }
    }
    for error in &report.errors {
        let text = normfix_i18n::fill(
            messages.leaks_error_at,
            &[("kind", &error.kind), ("function", &error.function)],
        );
        match leak_diagnostic(
            error.location.as_ref(),
            "MEMORY_ERROR",
            &text,
            messages.leaks_error_help,
            &mut sources,
        ) {
            Some(placed) => diagnostics.push(placed),
            None => unplaced.push(text),
        }
    }

    if diagnostics.is_empty() && unplaced.is_empty() {
        return;
    }
    if !unplaced.is_empty() {
        println!("{}", messages.leaks_sites);
    }
    if !diagnostics.is_empty() {
        let files = sources
            .into_iter()
            .map(|(path, source)| normfix_report::FileReport {
                budget: Vec::new(),
                after: diagnostics
                    .iter()
                    .filter(|(owner, _)| *owner == path)
                    .map(|(_, diagnostic)| diagnostic.clone())
                    .collect(),
                original: Some(std::sync::Arc::from(source.as_str())),
                path,
                changed: false,
                written: false,
                backup: None,
                failure: None,
                fixes: Vec::new(),
                before: Vec::new(),
                fixed: None,
            })
            .collect::<Vec<_>>();
        print!(
            "{}",
            normfix_report::render_findings(
                &files,
                normfix_report::RenderOptions {
                    color: !cli.no_color
                        && env::var_os("NO_COLOR").is_none()
                        && io::stdout().is_terminal(),
                    verbose: false,
                    show_diff: false,
                    locale: resolve_locale(cli).locale,
                },
            )
        );
    }
    for line in &unplaced {
        println!("  {line}");
    }
    if report.sites.iter().all(|site| site.location.is_none())
        && report.errors.iter().all(|error| error.location.is_none())
    {
        println!("{}", messages.leaks_no_debug_info);
    }
}

/// Builds a diagnostic pointing at the line the checker named.
fn leak_diagnostic(
    location: Option<&normfix_oracle::LeakLocation>,
    rule_id: &str,
    message: &str,
    help: &str,
    sources: &mut std::collections::BTreeMap<Utf8PathBuf, String>,
) -> Option<(Utf8PathBuf, normfix_core::Diagnostic)> {
    let location = location?;
    let path = Utf8PathBuf::from(&location.file);
    if !sources.contains_key(&path) {
        let text = std::fs::read_to_string(&path).ok()?;
        sources.insert(path.clone(), text);
    }
    let source = sources.get(&path)?;
    let range = line_range(source, location.line)?;
    Some((
        path.clone(),
        normfix_core::Diagnostic {
            rule_id: rule_id.to_owned(),
            path,
            range,
            severity: normfix_core::Severity::Error,
            message: message.to_owned(),
            source: normfix_core::DiagnosticSource::LeakChecker,
            notes: Vec::new(),
            help: Some(help.to_owned()),
            localized: None,
        },
    ))
}

/// The range covering one one-based line, without its leading whitespace.
fn line_range(source: &str, line: u32) -> Option<normfix_core::TextRange> {
    let mut offset = 0_usize;
    for (index, text) in source.split_inclusive('\n').enumerate() {
        if u32::try_from(index).ok()? + 1 == line {
            let lead = text.len() - text.trim_start().len();
            let content = text.trim_end();
            let start = u32::try_from(offset + lead).ok()?;
            let end = u32::try_from(offset + content.len().max(lead)).ok()?;
            return normfix_core::TextRange::new(
                normfix_core::TextSize::new(start),
                normfix_core::TextSize::new(end.max(start)),
            );
        }
        offset += text.len();
    }
    None
}

/// What to tell a reader whose platform has no checker.
///
/// The answer is different in kind on each one: a package manager away on Linux
/// and FreeBSD, a community port on macOS because upstream does not build
/// there, and another operating system on Windows because Valgrind does not
/// exist for it at all. One generic sentence would be wrong on two of the three.
const fn leaks_install_hint(messages: &normfix_i18n::Messages) -> &'static str {
    if cfg!(target_os = "macos") {
        messages.leaks_install_hint_macos
    } else if cfg!(windows) {
        messages.leaks_install_hint_windows
    } else {
        messages.leaks_install_hint
    }
}

fn confirm_leaks(
    cli: &Cli,
    messages: &normfix_i18n::Messages,
    program: &std::path::Path,
) -> Result<(), String> {
    if cli.force {
        return Ok(());
    }
    if cli.format == OutputFormat::Json || !io::stdin().is_terminal() || !io::stderr().is_terminal()
    {
        return Err(messages.leaks_needs_confirmation.to_owned());
    }
    eprint!(
        "{}",
        normfix_i18n::fill(
            messages.leaks_prompt,
            &[("program", &program.display().to_string())],
        )
    );
    let _ = io::stderr().flush();
    let mut answer = String::new();
    // `y` is the accepted answer in every language; see `confirm_undo`.
    let confirmed = io::stdin()
        .read_line(&mut answer)
        .is_ok_and(|_| answer.trim().eq_ignore_ascii_case("y"));
    confirmed
        .then_some(())
        .ok_or_else(|| messages.leaks_cancelled.to_owned())
}

fn run_upgrade(
    format: OutputFormat,
    messages: &normfix_i18n::Messages,
    check_only: bool,
) -> ExitCode {
    let json = format == OutputFormat::Json;
    match upgrade::upgrade(env!("CARGO_PKG_VERSION"), check_only) {
        Ok(upgrade::Outcome::Current(version)) => {
            if json {
                print_json_outcome(
                    "upgrade",
                    "success",
                    &serde_json::json!({
                        "state": "current",
                        "current_version": version,
                        "latest_version": version,
                        "installed": false,
                    }),
                );
            } else {
                println!("normfix {version} is already the newest release.");
            }
            ExitCode::SUCCESS
        }
        Ok(upgrade::Outcome::Available { current, latest }) => {
            if json {
                print_json_outcome(
                    "upgrade",
                    "success",
                    &serde_json::json!({
                        "state": "available",
                        "current_version": current,
                        "latest_version": latest,
                        "installed": false,
                    }),
                );
            } else {
                println!("normfix {latest} is available; this is {current}.");
                println!("Install it with: normfix upgrade");
            }
            ExitCode::SUCCESS
        }
        Ok(upgrade::Outcome::Installed {
            previous,
            installed,
        }) => {
            if json {
                print_json_outcome(
                    "upgrade",
                    "success",
                    &serde_json::json!({
                        "state": "installed",
                        "current_version": previous,
                        "latest_version": installed,
                        "installed": true,
                    }),
                );
            } else {
                println!("Upgraded normfix {previous} to {installed}.");
            }
            ExitCode::SUCCESS
        }
        Err(message) => {
            print_run_error(format, messages, &message);
            ExitCode::from(2)
        }
    }
}

/// Splits a Git scope into processable project files and unexpected files.
fn run_explain(
    format: OutputFormat,
    locale: normfix_i18n::Locale,
    messages: &normfix_i18n::Messages,
    rule: &str,
) -> ExitCode {
    let canonical = rule.trim().to_ascii_uppercase();
    let Some(explanation) = rules::explain(&canonical, locale, messages) else {
        print_run_error(
            format,
            messages,
            &normfix_i18n::fill(messages.explain_unknown_rule, &[("rule", &canonical)]),
        );
        return ExitCode::from(2);
    };
    match format {
        OutputFormat::Human => print!("{explanation}"),
        OutputFormat::Json => print_json_outcome(
            "explain",
            "success",
            &serde_json::json!({
                "rule_id": canonical,
                "explanation": explanation,
            }),
        ),
    }
    ExitCode::SUCCESS
}

fn run_undo(cli: &Cli, arguments: &UndoArguments, cwd: &std::path::Path) -> ExitCode {
    let runs = match collect_undo_runs(cli.backup_dir.as_deref(), cwd) {
        Ok(runs) => runs,
        Err(message) => {
            print_run_error(cli.format, cli_messages(cli), &message);
            return ExitCode::from(2);
        }
    };
    if arguments.list {
        render_undo_list(cli.format, &runs);
        return ExitCode::SUCCESS;
    }
    let selected = arguments.run.as_ref().map_or_else(
        || runs.last(),
        |run_id| runs.iter().rev().find(|run| &run.run_id == run_id),
    );
    let Some(selected) = selected else {
        let detail = arguments.run.as_ref().map_or_else(
            || "No intact backup run exists for this project.".to_owned(),
            |run_id| format!("No intact backup run named `{run_id}` exists for this project."),
        );
        print_run_error(cli.format, cli_messages(cli), &detail);
        return ExitCode::from(2);
    };
    if !cli.force {
        if let Err(message) = confirm_undo(selected, cli) {
            print_run_error(cli.format, cli_messages(cli), &message);
            return ExitCode::from(2);
        }
    }
    let Some(backup_root) = selected.journal.parent().and_then(std::path::Path::parent) else {
        print_run_error(
            cli.format,
            cli_messages(cli),
            "The selected recovery journal has no backup root.",
        );
        return ExitCode::from(2);
    };
    match undo_run(selected, cwd, backup_root) {
        Ok(report) => {
            match cli.format {
                OutputFormat::Human => {
                    println!(
                        "normfix undo\nRestored {} file(s) from {}.",
                        report.files.len(),
                        report.restored_run_id
                    );
                    for path in &report.files {
                        println!("  {}", path.display());
                    }
                    if let Some(journal) = report.journal {
                        println!(
                            "The displaced bytes remain recoverable through {}.",
                            journal.display()
                        );
                    }
                }
                OutputFormat::Json => println!(
                    "{}",
                    serde_json::to_string_pretty(&report)
                        .expect("undo report JSON is serializable")
                ),
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            print_run_error(cli.format, cli_messages(cli), &error.to_string());
            ExitCode::from(2)
        }
    }
}

fn collect_undo_runs(
    explicit: Option<&std::path::Path>,
    project_root: &std::path::Path,
) -> Result<Vec<UndoRun>, String> {
    let mut runs = Vec::new();
    for root in backup_roots(explicit) {
        runs.extend(list_undo_runs(&root, project_root).map_err(|error| error.to_string())?);
    }
    runs.sort_by(|left, right| {
        let left_time = std::fs::metadata(&left.journal)
            .and_then(|metadata| metadata.modified())
            .unwrap_or(std::time::UNIX_EPOCH);
        let right_time = std::fs::metadata(&right.journal)
            .and_then(|metadata| metadata.modified())
            .unwrap_or(std::time::UNIX_EPOCH);
        left_time
            .cmp(&right_time)
            .then_with(|| left.run_id.cmp(&right.run_id))
    });
    runs.dedup_by(|left, right| left.journal == right.journal);
    Ok(runs)
}

/// The user-owned data directory this platform reports.
///
/// Windows resolves through `LOCALAPPDATA`, which is where the cache already
/// looks and whose ACL is already restricted to that user. Without a branch
/// here the undo listing would look in a directory that does not exist on the
/// platform, and report no recoverable runs when there are some.
#[cfg(windows)]
fn platform_data_base() -> Option<PathBuf> {
    env::var_os("LOCALAPPDATA")
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
}

#[cfg(not(windows))]
fn platform_data_base() -> Option<PathBuf> {
    env::var_os("HOME")
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .map(|home| home.join(".local/share"))
}

fn backup_roots(explicit: Option<&std::path::Path>) -> Vec<PathBuf> {
    if let Some(path) = explicit {
        return vec![path.to_path_buf()];
    }
    let base = env::var_os("XDG_DATA_HOME")
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .or_else(platform_data_base);
    base.map_or_else(Vec::new, |base| {
        vec![
            base.join("normfix/backups"),
            base.join("norminette-fix/backups"),
        ]
    })
}

fn render_undo_list(format: OutputFormat, runs: &[UndoRun]) {
    match format {
        OutputFormat::Human => {
            println!("normfix undo: {} recovery point(s)", runs.len());
            for run in runs.iter().rev() {
                println!("  {}  {} file(s)", run.run_id, run.files.len());
            }
        }
        OutputFormat::Json => print_json_outcome(
            "undo",
            "success",
            &serde_json::json!({
                "recovery_points": runs,
                "count": runs.len(),
            }),
        ),
    }
}

fn confirm_undo(run: &UndoRun, cli: &Cli) -> Result<(), String> {
    let messages = cli_messages(cli);
    if cli.format == OutputFormat::Json || !io::stdin().is_terminal() || !io::stderr().is_terminal()
    {
        return Err(messages.undo_needs_confirmation.to_owned());
    }
    eprintln!(
        "{}",
        normfix_i18n::fill_plural(
            cli_locale(cli),
            run.files.len() as u64,
            messages.undo_question_one,
            messages.undo_question_other,
            &[
                ("count", &run.files.len().to_string()),
                ("run", &run.run_id),
            ],
        )
    );
    eprint!("{}", messages.undo_prompt);
    let _ = io::stderr().flush();
    let mut answer = String::new();
    // The accepted answer stays `y` in every language: it is a protocol token,
    // like a flag. A prompt that offered a translated letter and then rejected
    // it would be a trap in exactly the place that must not have one.
    let confirmed = io::stdin()
        .read_line(&mut answer)
        .is_ok_and(|_| answer.trim().eq_ignore_ascii_case("y"));
    confirmed
        .then_some(())
        .ok_or_else(|| messages.undo_cancelled.to_owned())
}
