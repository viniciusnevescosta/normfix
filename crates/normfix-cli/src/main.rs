//! Production command-line interface for the native fixer.

#![forbid(unsafe_code)]

mod cli;
mod commands;
mod execution;
mod identity;
mod interaction;
mod presentation;
mod rules;
mod scope_guard;
mod uninstall;
mod upgrade;

use std::env;
use std::io::{self, IsTerminal as _};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use normfix_engine::{BackupPolicy, FixOptions, run_fixes};
use normfix_header::IdentityResolution;
use normfix_project::{
    GitScope, GitScopeOptions, ProjectFileKind, is_project_control_file, resolve_git_scope,
};
use normfix_report::ReportMode;

#[cfg(test)]
use crate::cli::Command;
use crate::cli::{Cli, OutputFormat, Workflow, selected_workflow};
use crate::execution::ExecutionStart;
#[cfg(test)]
use crate::identity::identity_prompt_allowed;
use crate::identity::resolve_run_identity;
use crate::interaction::{authorize_destructive, run_interactive};
use crate::presentation::{
    cli_locale, cli_messages, print_run_error, render_report, report_mode_name, workflow_name,
};

/// Recoverable removals requested for one run.
// Clap exposes independent destructive switches; each may be enabled together.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Copy, Debug)]
struct DestructiveFlags {
    remove_unused: bool,
    remove_missing_makefile_sources: bool,
    remove_orphan_prototypes: bool,
    remove_unexpected: bool,
}

impl DestructiveFlags {
    fn from_cli(cli: &Cli) -> Self {
        Self {
            remove_unused: cli.remove_unused || cli.unsafe_mode,
            remove_missing_makefile_sources: cli.unsafe_mode,
            remove_orphan_prototypes: cli.unsafe_mode,
            remove_unexpected: cli.remove_unexpected || cli.unsafe_mode,
        }
    }

    fn any(self) -> bool {
        self.remove_unused
            || self.remove_missing_makefile_sources
            || self.remove_orphan_prototypes
            || self.remove_unexpected
    }
}

/// Everything resolved before the engine options are assembled.
struct OptionsInput {
    locale: normfix_i18n::Locale,
    workflow: Workflow,
    git_scoped: bool,
    scope_is_empty: bool,
    git_unexpected: Vec<PathBuf>,
    identity: IdentityResolution,
    destructive: DestructiveFlags,
}

fn main() -> ExitCode {
    run(&Cli::parse())
}

/// Resolves one filesystem-backed workflow and hands it to the engine.
fn run(cli: &Cli) -> ExitCode {
    let cwd = match env::current_dir() {
        Ok(cwd) => cwd,
        Err(error) => {
            print_run_error(
                cli.format,
                cli_messages(cli),
                &format!("Could not determine the current directory: {error}"),
            );
            return ExitCode::from(2);
        }
    };
    if cli.command.is_some() && !cli.paths.is_empty() {
        print_run_error(
            cli.format,
            cli_messages(cli),
            "paths before a subcommand are ambiguous; place every path after `format`, `lint`, `check`, `budget`, or `preflight`",
        );
        return ExitCode::from(2);
    }
    if let Some(exit) = commands::run_standalone_command(cli, &cwd) {
        return exit;
    }
    let (mut paths, workflow) = selected_workflow(cli);
    let mut git_unexpected = Vec::new();
    let git_scoped = cli.changed || cli.staged;
    if git_scoped {
        if !paths.is_empty() {
            print_run_error(
                cli.format,
                cli_messages(cli),
                "--changed and --staged select paths themselves and cannot be combined with PATH arguments",
            );
            return ExitCode::from(2);
        }
        match git_scoped_paths(&cwd, cli.staged) {
            Ok((selected, unexpected)) => {
                paths.extend(selected);
                git_unexpected = unexpected;
            }
            Err(message) => {
                print_run_error(cli.format, cli_messages(cli), &message);
                return ExitCode::from(2);
            }
        }
    }
    let destructive = DestructiveFlags::from_cli(cli);
    if let Err(message) = validate_run_scope(cli, workflow, destructive, &cwd, &paths, git_scoped) {
        print_run_error(cli.format, cli_messages(cli), &message);
        return ExitCode::from(2);
    }
    let (identity, identity_persistence) =
        resolve_run_identity(cli, &paths, git_scoped, workflow, &cwd);
    let mut options = build_fix_options(
        cli,
        cwd,
        OptionsInput {
            // JSON output stays English, so the engine follows the same rule
            // the report and the announcement already do.
            locale: cli_locale(cli),
            workflow,
            git_scoped,
            scope_is_empty: paths.is_empty(),
            git_unexpected,
            identity,
            destructive,
        },
    );

    if let Err(message) = announce_execution(
        cli,
        workflow,
        &paths,
        git_scoped,
        &options,
        identity_persistence,
    ) {
        print_run_error(cli.format, cli_messages(cli), &message);
        return ExitCode::from(2);
    }
    options.destructive_authorization =
        match authorize_destructive(destructive, cli.force, cli_messages(cli), cli.format) {
            Ok(authorization) => authorization,
            Err(message) => {
                print_run_error(cli.format, cli_messages(cli), &message);
                return ExitCode::from(2);
            }
        };

    if cli.interactive {
        return run_interactive(cli, &paths, &options);
    }

    finish_run(cli, &paths, &options, workflow)
}

/// Runs the pipeline and renders its report.
fn finish_run(cli: &Cli, paths: &[PathBuf], options: &FixOptions, workflow: Workflow) -> ExitCode {
    match run_fixes(paths, options) {
        Ok(report) => {
            if let Err(message) = render_report(cli, &report, workflow) {
                print_run_error(cli.format, cli_messages(cli), &message);
                return ExitCode::from(2);
            }
            if cli.format == OutputFormat::Human && io::stderr().is_terminal() {
                upgrade::notify_if_outdated(env!("CARGO_PKG_VERSION"));
            }
            ExitCode::from(report.exit_code())
        }
        Err(error) => {
            print_run_error(cli.format, cli_messages(cli), &error.to_string());
            ExitCode::from(2)
        }
    }
}

/// Replaces the running binary with the newest published release.
/// Removes this binary, and under `--purge` the data it created.
///
/// The plan is printed before anything is deleted, for the same reason a run
/// announces its scope: the destructive step must be visible while it can still
/// be refused.
fn git_scoped_paths(
    cwd: &std::path::Path,
    staged: bool,
) -> Result<(Vec<PathBuf>, Vec<PathBuf>), String> {
    let scope = if staged {
        GitScope::Staged
    } else {
        GitScope::Changed
    };
    let scoped = resolve_git_scope(cwd, scope, &GitScopeOptions::default())
        .map_err(|error| error.to_string())?;
    let mut selected = Vec::new();
    let mut unexpected = Vec::new();
    for path in scoped {
        if ProjectFileKind::from_path(&path).is_some() {
            selected.push(path);
        } else if !is_project_control_file(&path) {
            unexpected.push(path);
        }
    }
    Ok((selected, unexpected))
}

/// Returns the first invalid flag combination for this invocation.
fn invalid_invocation(
    cli: &Cli,
    workflow: Workflow,
    destructive: DestructiveFlags,
    protected_scope: bool,
) -> Option<String> {
    let diagnostic_only = match workflow {
        Workflow::Lint => Some("lint"),
        Workflow::Budget => Some("budget"),
        _ => None,
    };
    if let Some(command) = diagnostic_only {
        if cli.check {
            return Some(format!(
                "`normfix {command}` is already read-only; remove the redundant --check flag"
            ));
        }
        if cli.diff {
            return Some(format!(
                "`normfix {command}` never proposes edits, so --diff has nothing to show; use `normfix check --diff` to preview proven changes"
            ));
        }
        if cli.login.is_some() || cli.email.is_some() {
            return Some(format!(
                "`normfix {command}` does not create headers; remove --login/--email, or use `normfix check` to preview the official header"
            ));
        }
        if cli.remove_invalid_comments
            || destructive.any()
            || cli.no_format_markdown
            || cli.no_reorder_includes
            || cli.max_passes != 100
        {
            return Some(format!(
                "`normfix {command}` diagnoses the bytes on disk and never plans edits; remove formatting/removal flags, or use `normfix check` to preview changes"
            ));
        }
    }
    let read_only = cli.check
        || cli.diff
        || matches!(
            workflow,
            Workflow::Lint | Workflow::Check | Workflow::Budget | Workflow::Preflight
        );
    if read_only && (cli.no_backup || cli.backup_dir.is_some()) {
        return Some(
            "this is a read-only run, so backup flags would have no effect; remove --no-backup/--backup-dir"
                .to_owned(),
        );
    }
    if cli.check && matches!(workflow, Workflow::Check | Workflow::Preflight) {
        return Some(format!(
            "`normfix {}` is already read-only; remove the redundant --check flag",
            if workflow == Workflow::Check {
                "check"
            } else {
                "preflight"
            }
        ));
    }
    if workflow == Workflow::Preflight && cli.no_compiler_preflight {
        return Some(
            "preflight includes the strict compiler check; remove --no-compiler-preflight"
                .to_owned(),
        );
    }
    if cli.force && !destructive.any() && !protected_scope {
        return Some(cli_messages(cli).force_without_target.to_owned());
    }
    if cli.interactive
        && (cli.format != OutputFormat::Human
            || cli.check
            || cli.diff
            || !matches!(workflow, Workflow::Default | Workflow::Format)
            || cli.remove_invalid_comments
            || destructive.any())
    {
        return Some(
            "--interactive is limited to the default or `format` fixing workflow and cannot be combined with --check, --diff, --unsafe, or destructive removal flags"
                .to_owned(),
        );
    }
    None
}

fn validate_run_scope(
    cli: &Cli,
    workflow: Workflow,
    destructive: DestructiveFlags,
    cwd: &std::path::Path,
    paths: &[PathBuf],
    git_scoped: bool,
) -> Result<(), String> {
    let protected = scope_guard::sensitive_scope(cwd, paths, git_scoped);
    if let Some(message) = invalid_invocation(cli, workflow, destructive, protected.is_some()) {
        return Err(message);
    }
    if let Some(protected) = protected.as_ref().filter(|_| !cli.force) {
        let messages = cli_messages(cli);
        return Err(normfix_i18n::fill(
            messages.scope_refusal,
            &[
                ("scope", &protected.resolved.display().to_string()),
                ("reason", protected.reason.describe(messages)),
            ],
        ));
    }
    Ok(())
}

fn build_fix_options(cli: &Cli, cwd: PathBuf, input: OptionsInput) -> FixOptions {
    let mut options = FixOptions::new(cwd);
    options.mode = if cli.diff {
        ReportMode::Diff
    } else if cli.check
        || matches!(
            input.workflow,
            Workflow::Lint | Workflow::Check | Workflow::Budget | Workflow::Preflight
        )
    {
        ReportMode::Check
    } else {
        ReportMode::Fix
    };
    options.respect_gitignore = cli.use_gitignore;
    options.empty_input_is_empty = input.git_scoped && input.scope_is_empty;
    options.additional_unexpected_files = input.git_unexpected;
    options.lint_only = matches!(input.workflow, Workflow::Lint | Workflow::Budget);
    options.emit_budget = input.workflow == Workflow::Budget;
    options.preflight = input.workflow == Workflow::Preflight;
    options.threads = cli.threads;
    options.identity_source = input.identity.source;
    options.identity = input.identity.identity;
    options.backup = if cli.no_backup {
        BackupPolicy::Disabled
    } else if let Some(directory) = cli.backup_dir.clone() {
        BackupPolicy::Directory(directory)
    } else {
        BackupPolicy::Automatic
    };
    options.norminette_executable.clone_from(&cli.norminette);
    options.locale = input.locale;
    options.strict_norminette_version = cli.strict_norminette_version;
    options.compiler_preflight = !cli.no_compiler_preflight;
    options.compiler_executable.clone_from(&cli.cc);
    options.clang_tidy_executable.clone_from(&cli.clang_tidy);
    options.analyzer = cli.analyzer;
    options.timeout = cli.timeout;
    options.cache = !cli.no_cache;
    options.remove_invalid_comments = cli.remove_invalid_comments || cli.unsafe_mode;
    options.remove_unused_variables = cli.unsafe_mode;
    options.compact_null_checks = cli.unsafe_mode;
    options.remove_missing_makefile_sources = input.destructive.remove_missing_makefile_sources;
    options.remove_orphan_prototypes = input.destructive.remove_orphan_prototypes;
    options.remove_unused_static = input.destructive.remove_unused;
    options.quarantine_unexpected = input.destructive.remove_unexpected;
    options.reorder_includes = !cli.no_reorder_includes;
    options.format_markdown = !cli.no_format_markdown;
    options.max_passes = cli.max_passes;
    options
}

fn announce_execution(
    cli: &Cli,
    workflow: Workflow,
    paths: &[PathBuf],
    git_scoped: bool,
    options: &FixOptions,
    advisory: Option<String>,
) -> Result<(), String> {
    // Only the human block is localized. The JSON event keeps English values so
    // a script never has to select a language to parse the same run.
    let messages = cli_messages(cli);
    let scope = execution_scope(cli, paths, git_scoped, &options.cwd, messages);
    let identity = options.identity.as_ref().map_or_else(
        || messages.identity_unavailable.to_owned(),
        |identity| identity.email.clone(),
    );
    let backups = match (options.mode, &options.backup) {
        (ReportMode::Check | ReportMode::Diff, _) => messages.backups_read_only.to_owned(),
        (ReportMode::Fix, BackupPolicy::Automatic) => messages.backups_automatic.to_owned(),
        (ReportMode::Fix, BackupPolicy::Directory(path)) => normfix_i18n::fill(
            messages.backups_directory,
            &[("path", &path.display().to_string())],
        ),
        (ReportMode::Fix, BackupPolicy::Disabled) => messages.backups_disabled.to_owned(),
    };
    let event = ExecutionStart {
        event: "execution_start",
        action: workflow_name(cli, workflow).to_owned(),
        mode: report_mode_name(options.mode, messages).to_owned(),
        current_directory: options.cwd.display().to_string(),
        scope,
        identity,
        identity_source: options.identity_source.clone(),
        workers: options.threads.map_or_else(
            || messages.workers_automatic.to_owned(),
            |count| count.to_string(),
        ),
        timeout_seconds: options.timeout.as_secs_f64(),
        norminette: options.norminette_executable.as_ref().map_or_else(
            || messages.norminette_path_discovery.to_owned(),
            |path| path.display().to_string(),
        ),
        norminette_version_policy: if options.strict_norminette_version {
            messages.version_policy_strict
        } else {
            messages.version_policy_advisory
        }
        .to_owned(),
        compiler_preflight: options.compiler_preflight,
        cache: options.cache,
        respect_gitignore: options.respect_gitignore,
        backups,
        destructive: destructive_description(options, messages),
        forced: cli.force,
        advisory,
    };
    match cli.format {
        OutputFormat::Human => eprint!("{}", event.to_human(messages)),
        OutputFormat::Json => eprintln!(
            "{}",
            event
                .to_json_line()
                .map_err(|error| format!("Could not serialize execution settings: {error}"))?
        ),
    }
    Ok(())
}

fn execution_scope(
    cli: &Cli,
    paths: &[PathBuf],
    git_scoped: bool,
    cwd: &std::path::Path,
    messages: &normfix_i18n::Messages,
) -> String {
    if git_scoped {
        normfix_i18n::fill_plural(
            cli_locale(cli),
            paths.len() as u64,
            messages.scope_git_one,
            messages.scope_git_other,
            &[
                (
                    "kind",
                    if cli.staged {
                        messages.scope_git_staged
                    } else {
                        messages.scope_git_changed
                    },
                ),
                ("directory", &cwd.display().to_string()),
                ("count", &paths.len().to_string()),
            ],
        )
    } else if paths.is_empty() {
        normfix_i18n::fill(
            messages.scope_recursive,
            &[("directory", &cwd.display().to_string())],
        )
    } else {
        let mut selected = paths
            .iter()
            .take(3)
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>();
        if paths.len() > selected.len() {
            selected.push(normfix_i18n::fill(
                messages.scope_more_paths,
                &[("count", &(paths.len() - selected.len()).to_string())],
            ));
        }
        selected.join(", ")
    }
}

fn destructive_description(options: &FixOptions, messages: &normfix_i18n::Messages) -> String {
    let mut destructive = Vec::new();
    if options.remove_invalid_comments {
        destructive.push(messages.destructive_invalid_comments);
    }
    if options.remove_unused_variables {
        destructive.push(messages.destructive_unused_variables);
    }
    if options.compact_null_checks {
        destructive.push(messages.destructive_null_checks);
    }
    if options.remove_missing_makefile_sources {
        destructive.push(messages.destructive_makefile_entries);
    }
    if options.remove_orphan_prototypes {
        destructive.push(messages.destructive_orphan_prototypes);
    }
    if options.remove_unused_static {
        destructive.push(messages.destructive_unused_statics);
    }
    if options.quarantine_unexpected {
        destructive.push(messages.destructive_quarantine);
    }
    if destructive.is_empty() {
        messages.destructive_none.to_owned()
    } else {
        destructive.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::process::ExitCode;
    use std::time::Duration;

    use clap::Parser;

    use super::{
        Cli, Command, DestructiveFlags, OutputFormat, Workflow, authorize_destructive,
        identity_prompt_allowed, invalid_invocation, run, selected_workflow, workflow_name,
    };

    #[test]
    fn accepts_zero_one_or_many_paths_like_norminette() {
        for expected in 0..=2 {
            let arguments = match expected {
                0 => vec!["normfix"],
                1 => vec!["normfix", "main.c"],
                _ => vec!["normfix", "src", "include/demo.h"],
            };
            let parsed = Cli::try_parse_from(arguments).expect("valid CLI");
            assert_eq!(parsed.paths.len(), expected);
        }
    }

    #[test]
    fn parses_preview_identity_performance_and_output_flags() {
        let parsed = Cli::try_parse_from([
            "normfix",
            "--diff",
            "--use-gitignore",
            "--login",
            "student",
            "--email",
            "student@student.42.fr",
            "--format",
            "json",
            "--timeout",
            "1.5",
            "--threads",
            "4",
            "--remove-invalid-comments",
            "--remove-unused",
            "--remove-unexpected",
            "--force",
            "--format-markdown",
            "--no-cache",
            "--strict-norminette-version",
            "src",
        ])
        .expect("valid CLI");

        assert!(parsed.diff);
        assert_eq!(parsed.format, OutputFormat::Json);
        assert_eq!(parsed.timeout, Duration::from_millis(1500));
        assert_eq!(parsed.threads, Some(4));
        assert!(parsed.remove_invalid_comments);
        assert!(parsed.remove_unused);
        assert!(parsed.remove_unexpected);
        assert!(parsed.force);
        assert!(parsed.format_markdown);
        assert!(!parsed.no_format_markdown);
        assert!(parsed.no_cache);
        assert!(parsed.strict_norminette_version);
        assert_eq!(parsed.paths, vec![PathBuf::from("src")]);
    }

    #[test]
    fn rejects_conflicting_previews_and_invalid_limits() {
        assert!(Cli::try_parse_from(["normfix", "--check", "--diff"]).is_err());
        assert!(Cli::try_parse_from(["normfix", "--changed", "--staged"]).is_err());
        assert!(Cli::try_parse_from(["normfix", "--threads", "0"]).is_err());
        assert!(Cli::try_parse_from(["normfix", "--timeout", "nan"]).is_err());
        let ambiguous = Cli::try_parse_from(["normfix", "src", "format", "include"])
            .expect("Clap parses before semantic validation");
        assert_eq!(run(&ambiguous), ExitCode::from(2));
        let explain = Cli::try_parse_from(["normfix", "ignored.c", "explain", "LINE_TOO_LONG"])
            .expect("Clap parses before semantic validation");
        assert_eq!(run(&explain), ExitCode::from(2));
    }

    #[test]
    fn parses_workflow_subcommands_and_markdown_opt_out() {
        let check = Cli::try_parse_from([
            "normfix",
            "check",
            "src",
            "include/project.h",
            "--no-format-markdown",
        ])
        .expect("check workflow");
        let Some(Command::Check(arguments)) = check.command else {
            panic!("expected check workflow");
        };
        assert_eq!(
            arguments.paths,
            vec![PathBuf::from("src"), PathBuf::from("include/project.h")]
        );
        assert!(check.no_format_markdown);

        let undo =
            Cli::try_parse_from(["normfix", "undo", "--list", "--force"]).expect("undo workflow");
        assert!(matches!(
            undo.command,
            Some(Command::Undo(arguments)) if arguments.list
        ));
        assert!(undo.force);

        let preflight = Cli::try_parse_from(["normfix", "preflight", "--changed", "--interactive"])
            .expect("preflight workflow flags parse before semantic validation");
        assert!(matches!(preflight.command, Some(Command::Preflight(_))));
        assert!(preflight.changed);
        assert!(preflight.interactive);
    }

    #[test]
    fn identity_prompt_is_never_available_to_json_or_noninteractive_runs() {
        assert!(!identity_prompt_allowed(OutputFormat::Json, true, true));
        assert!(!identity_prompt_allowed(OutputFormat::Human, false, true));
        assert!(!identity_prompt_allowed(OutputFormat::Human, true, false));
        assert!(identity_prompt_allowed(OutputFormat::Human, true, true));
    }

    #[test]
    fn global_preview_shortcuts_name_the_real_action_in_the_start_banner() {
        let check = Cli::try_parse_from(["normfix", "--check"]).expect("check shortcut");
        let (_, check_workflow) = selected_workflow(&check);
        assert_eq!(workflow_name(&check, check_workflow), "check");

        let diff = Cli::try_parse_from(["normfix", "--diff"]).expect("diff shortcut");
        let (_, diff_workflow) = selected_workflow(&diff);
        assert_eq!(workflow_name(&diff, diff_workflow), "diff");
    }

    #[test]
    fn protected_scope_force_does_not_invent_destructive_capabilities() {
        let cli = Cli::try_parse_from(["normfix", "--force", "/"]).expect("forced root scope");
        let destructive = DestructiveFlags::from_cli(&cli);

        assert!(invalid_invocation(&cli, Workflow::Default, destructive, true).is_none());
        assert!(
            authorize_destructive(
                destructive,
                cli.force,
                normfix_i18n::messages(normfix_i18n::Locale::English),
                OutputFormat::Json,
            )
            .expect("no destructive request")
            .is_none()
        );
    }

    #[test]
    fn diagnostic_only_commands_reject_flags_that_cannot_affect_their_result() {
        for (arguments, workflow, expected) in [
            (
                vec!["normfix", "lint", "--diff"],
                Workflow::Lint,
                "never proposes edits",
            ),
            (
                vec!["normfix", "budget", "--unsafe", "--force"],
                Workflow::Budget,
                "never plans edits",
            ),
            (
                vec!["normfix", "lint", "--email", "student@student.42.fr"],
                Workflow::Lint,
                "does not create headers",
            ),
            (
                vec!["normfix", "budget", "--no-reorder-includes"],
                Workflow::Budget,
                "never plans edits",
            ),
        ] {
            let cli = Cli::try_parse_from(arguments).expect("the grammar remains compatible");
            let error = invalid_invocation(&cli, workflow, DestructiveFlags::from_cli(&cli), false)
                .expect("ignored flags must be rejected");
            assert!(error.contains(expected), "{error}");
        }
    }

    #[test]
    fn read_only_commands_reject_backup_flags_and_redundant_check() {
        for (arguments, workflow, expected) in [
            (
                vec!["normfix", "check", "--no-backup"],
                Workflow::Check,
                "backup flags would have no effect",
            ),
            (
                vec!["normfix", "preflight", "--backup-dir", "saved"],
                Workflow::Preflight,
                "backup flags would have no effect",
            ),
            (
                vec!["normfix", "check", "--check"],
                Workflow::Check,
                "already read-only",
            ),
        ] {
            let cli = Cli::try_parse_from(arguments).expect("the grammar remains compatible");
            let error = invalid_invocation(&cli, workflow, DestructiveFlags::from_cli(&cli), false)
                .expect("ignored flags must be rejected");
            assert!(error.contains(expected), "{error}");
        }
    }

    #[test]
    fn destructive_noninteractive_runs_still_require_force() {
        let destructive = DestructiveFlags {
            remove_unused: true,
            remove_missing_makefile_sources: false,
            remove_orphan_prototypes: false,
            remove_unexpected: false,
        };
        let error = authorize_destructive(
            destructive,
            false,
            normfix_i18n::messages(normfix_i18n::Locale::English),
            OutputFormat::Json,
        )
        .expect_err("JSON may not prompt");
        assert!(error.contains("require an interactive"));
    }

    #[test]
    fn deprecated_allow_flag_is_hidden_and_conflicts_with_strict_policy() {
        assert!(
            Cli::try_parse_from([
                "normfix",
                "--allow-untested-norminette",
                "--strict-norminette-version",
            ])
            .is_err()
        );
        let help = Cli::try_parse_from(["normfix", "--help"])
            .expect_err("help exits through clap")
            .to_string();
        assert!(!help.contains("allow-untested-norminette"));
        assert!(help.contains("strict-norminette-version"));
    }
}
