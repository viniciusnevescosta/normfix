//! Production command-line interface for the native fixer.

#![forbid(unsafe_code)]

mod execution;
mod rules;
mod scope_guard;
mod upgrade;

use std::collections::BTreeMap;
use std::env;
use std::io::{self, IsTerminal, Write as _};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use clap::{Args, Parser, Subcommand, ValueEnum};
use normfix_actions::{UndoRun, list_undo_runs, undo_run};
use normfix_destructive::{DestructiveAuthorization, DestructiveCapability, DestructiveRequest};
use normfix_engine::{BackupPolicy, FixOptions, WriteApproval, run_fixes};
use normfix_header::{
    IdentityResolution, RunClock, identity_from_email, persist_identity, resolve_identity,
};
use normfix_project::{
    GitScope, GitScopeOptions, ProjectFileKind, is_project_control_file, resolve_git_scope,
};
use normfix_report::{
    FileReport, RenderOptions, ReportMode, RunReport, render_human, unified_diff,
};

use crate::execution::{ExecutionStart, terminal_safe_inline};

// Clap represents independent switches as booleans; replacing them with one
// state enum would incorrectly make compatible command-line flags exclusive.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Parser)]
#[command(name = "normfix")]
#[command(version)]
#[command(about = "Safe automatic fixes and actionable diagnostics for the 42 Norm v4.1")]
#[command(subcommand_precedence_over_arg = true)]
#[command(after_help = "With no COMMAND or PATH, the current directory is fixed recursively.")]
struct Cli {
    /// Files or directories; accepts zero, one or many paths.
    paths: Vec<PathBuf>,

    /// Focused workflows; the commandless interface remains backward compatible.
    #[command(subcommand)]
    command: Option<Command>,

    /// Report changes without writing files.
    #[arg(long, global = true, conflicts_with = "diff")]
    check: bool,

    /// Print unified diffs without writing files.
    #[arg(long, global = true)]
    diff: bool,

    /// Respect .gitignore while recursively discovering directory inputs.
    #[arg(long, global = true)]
    use_gitignore: bool,

    /// Verified 42 login; the email remains the source of truth.
    #[arg(long, global = true)]
    login: Option<String>,

    /// Verified 42 student email used by official headers.
    #[arg(long, global = true)]
    email: Option<String>,

    /// Do not retain external backups for ordinary formatting writes.
    #[arg(long, global = true, conflicts_with = "backup_dir")]
    no_backup: bool,

    /// External backup base directory.
    #[arg(long, global = true, value_name = "PATH")]
    backup_dir: Option<PathBuf>,

    /// Select polished terminal output or stable JSON.
    #[arg(long, global = true, value_enum, default_value_t = OutputFormat::Human)]
    format: OutputFormat,

    /// Disable ANSI colors even on an interactive terminal.
    #[arg(long, global = true)]
    no_color: bool,

    /// Show every accepted fix in human output.
    #[arg(long, short, global = true)]
    verbose: bool,

    /// Preview and approve each changed file before a second validated run writes it.
    #[arg(long, global = true)]
    interactive: bool,

    /// Process unstaged tracked changes and untracked, non-ignored files.
    #[arg(long, global = true, conflicts_with = "staged")]
    changed: bool,

    /// Process only files currently recorded in the Git index.
    #[arg(long, global = true)]
    staged: bool,

    /// Per-file official Norminette timeout in seconds.
    #[arg(long, global = true, default_value = "5", value_parser = parse_timeout)]
    timeout: Duration,

    /// Number of parallel workers; defaults to available hardware.
    #[arg(long, global = true, value_parser = parse_worker_count)]
    threads: Option<usize>,

    /// Delete only comments rejected at exact official locations.
    #[arg(long, global = true)]
    remove_invalid_comments: bool,

    /// Remove only unreachable static functions proven in the complete project.
    #[arg(long, global = true)]
    remove_unused: bool,

    /// Move unexpected regular files to external recoverable quarantine.
    #[arg(long, global = true)]
    remove_unexpected: bool,

    /// Enable comment/NULL cleanup, stale Makefile cleanup, unused-static
    /// removal, and unexpected-file quarantine.
    #[arg(long = "unsafe", global = true)]
    unsafe_mode: bool,

    /// Confirm destructive operations or acknowledge a protected system scope.
    #[arg(long, global = true)]
    force: bool,

    /// Legacy no-op: README formatting is enabled by default.
    #[arg(long, global = true, hide = true)]
    format_markdown: bool,

    /// Leave README documents unchanged.
    #[arg(long, global = true, conflicts_with = "format_markdown")]
    no_format_markdown: bool,

    /// Leave contiguous include blocks in their current order.
    #[arg(long, global = true)]
    no_reorder_includes: bool,

    /// Disable the external content-addressed analysis cache.
    #[arg(long, global = true)]
    no_cache: bool,

    /// Use this exact Norminette executable.
    #[arg(long, global = true, value_name = "PATH")]
    norminette: Option<PathBuf>,

    /// Deprecated no-op: untested Norminette releases now run with an advisory.
    #[arg(
        long,
        global = true,
        hide = true,
        conflicts_with = "strict_norminette_version"
    )]
    allow_untested_norminette: bool,

    /// Refuse Norminette releases this normfix version has not verified.
    #[arg(long, global = true)]
    strict_norminette_version: bool,

    /// Disable `cc -fsyntax-only -Wall -Wextra -Werror` diagnostics.
    #[arg(long, global = true)]
    no_compiler_preflight: bool,

    /// Use this exact C compiler for strict preflight and optional analysis.
    #[arg(long, global = true, value_name = "PATH")]
    cc: Option<PathBuf>,

    /// Run GCC `-fanalyzer` as a slower informational check.
    #[arg(long, global = true)]
    analyzer: bool,

    /// Maximum fixed-point passes for the native formatter.
    #[arg(long, global = true, hide = true, default_value_t = 100, value_parser = parse_pass_count)]
    max_passes: usize,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Apply the canonical layout printer and proven safe fixes.
    Format(PathArguments),
    /// Report source and project problems without proposing edits.
    Lint(PathArguments),
    /// Preview formatting and lint together without writing files.
    Check(PathArguments),
    /// Show function line/variable/parameter headroom.
    Budget(PathArguments),
    /// Run the read-only checks useful immediately before a 42 evaluation.
    Preflight(PathArguments),
    /// Explain one Norm or native rule offline.
    Explain(ExplainArguments),
    /// Restore an intact backed-up run without overwriting later edits.
    Undo(UndoArguments),
    /// Replace this binary with the newest published release.
    Upgrade(UpgradeArguments),
}

#[derive(Debug, Args)]
struct UpgradeArguments {
    /// Report whether a newer release exists without installing it.
    #[arg(long)]
    check: bool,
}

#[derive(Debug, Args)]
struct PathArguments {
    /// Files or directories; defaults to the current directory.
    paths: Vec<PathBuf>,
}

#[derive(Debug, Args)]
struct ExplainArguments {
    /// Rule identifier, for example `TOO_MANY_LINES`.
    rule: String,
}

#[derive(Debug, Args)]
struct UndoArguments {
    /// List recovery points without restoring anything.
    #[arg(long, conflicts_with = "run")]
    list: bool,

    /// Restore this exact run instead of the newest intact run.
    #[arg(long, value_name = "RUN_ID", conflicts_with = "list")]
    run: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum OutputFormat {
    Human,
    Json,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Workflow {
    Default,
    Format,
    Lint,
    Check,
    Budget,
    Preflight,
}

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

fn run(cli: &Cli) -> ExitCode {
    let cwd = match env::current_dir() {
        Ok(cwd) => cwd,
        Err(error) => {
            print_run_error(
                cli.format,
                &format!("Could not determine the current directory: {error}"),
            );
            return ExitCode::from(2);
        }
    };
    if cli.command.is_some() && !cli.paths.is_empty() {
        print_run_error(
            cli.format,
            "paths before a subcommand are ambiguous; place every path after `format`, `lint`, `check`, `budget`, or `preflight`",
        );
        return ExitCode::from(2);
    }
    if let Some(Command::Explain(arguments)) = &cli.command {
        return run_explain(cli.format, &arguments.rule);
    }
    if let Some(Command::Undo(arguments)) = &cli.command {
        return run_undo(cli, arguments, &cwd);
    }
    if let Some(Command::Upgrade(arguments)) = &cli.command {
        return run_upgrade(cli.format, arguments.check);
    }
    let (mut paths, workflow) = selected_workflow(cli);
    let mut git_unexpected = Vec::new();
    let git_scoped = cli.changed || cli.staged;
    if git_scoped {
        if !paths.is_empty() {
            print_run_error(
                cli.format,
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
                print_run_error(cli.format, &message);
                return ExitCode::from(2);
            }
        }
    }
    let destructive = DestructiveFlags::from_cli(cli);
    if let Err(message) = validate_run_scope(cli, workflow, destructive, &cwd, &paths, git_scoped) {
        print_run_error(cli.format, &message);
        return ExitCode::from(2);
    }
    let (identity, identity_persistence) =
        resolve_run_identity(cli, &paths, git_scoped, workflow, &cwd);
    let mut options = build_fix_options(
        cli,
        cwd,
        OptionsInput {
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
        print_run_error(cli.format, &message);
        return ExitCode::from(2);
    }
    options.destructive_authorization =
        match authorize_destructive(destructive, cli.force, cli.format) {
            Ok(authorization) => authorization,
            Err(message) => {
                print_run_error(cli.format, &message);
                return ExitCode::from(2);
            }
        };

    if cli.interactive {
        return run_interactive(cli, &paths, &options);
    }

    finish_run(cli, &paths, &options)
}

/// Runs the pipeline and renders its report.
fn finish_run(cli: &Cli, paths: &[PathBuf], options: &FixOptions) -> ExitCode {
    match run_fixes(paths, options) {
        Ok(report) => {
            if let Err(message) = render_report(cli, &report) {
                print_run_error(cli.format, &message);
                return ExitCode::from(2);
            }
            if cli.format == OutputFormat::Human && io::stderr().is_terminal() {
                upgrade::notify_if_outdated(env!("CARGO_PKG_VERSION"));
            }
            ExitCode::from(report.exit_code())
        }
        Err(error) => {
            print_run_error(cli.format, &error.to_string());
            ExitCode::from(2)
        }
    }
}

/// Replaces the running binary with the newest published release.
fn run_upgrade(format: OutputFormat, check_only: bool) -> ExitCode {
    match upgrade::upgrade(env!("CARGO_PKG_VERSION"), check_only) {
        Ok(upgrade::Outcome::Current(version)) => {
            println!("normfix {version} is already the newest release.");
            ExitCode::SUCCESS
        }
        Ok(upgrade::Outcome::Available { current, latest }) => {
            println!("normfix {latest} is available; this is {current}.");
            println!("Install it with: normfix upgrade");
            ExitCode::SUCCESS
        }
        Ok(upgrade::Outcome::Installed {
            previous,
            installed,
        }) => {
            println!("Upgraded normfix {previous} to {installed}.");
            ExitCode::SUCCESS
        }
        Err(message) => {
            print_run_error(format, &message);
            ExitCode::from(2)
        }
    }
}

/// Splits a Git scope into processable project files and unexpected files.
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
) -> Option<&'static str> {
    if workflow == Workflow::Preflight && cli.no_compiler_preflight {
        return Some(
            "preflight includes the strict compiler check; remove --no-compiler-preflight",
        );
    }
    if cli.force && !destructive.any() && !protected_scope {
        return Some(
            "--force requires --unsafe, --remove-unused, --remove-unexpected, or a protected system scope",
        );
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
            "--interactive is limited to the default or `format` fixing workflow and cannot be combined with --check, --diff, --unsafe, or destructive removal flags",
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
        return Err(message.to_owned());
    }
    if let Some(protected) = protected.as_ref().filter(|_| !cli.force) {
        return Err(format!(
            "refusing to scan or modify protected scope `{}` because {}; inspect the path and pass --force to acknowledge it explicitly",
            protected.resolved.display(),
            protected.reason
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
    options.strict_norminette_version = cli.strict_norminette_version;
    options.compiler_preflight = !cli.no_compiler_preflight;
    options.compiler_executable.clone_from(&cli.cc);
    options.analyzer = cli.analyzer;
    options.timeout = cli.timeout;
    options.cache = !cli.no_cache;
    options.remove_invalid_comments = cli.remove_invalid_comments || cli.unsafe_mode;
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

fn scope_may_need_identity(paths: &[PathBuf], git_scoped: bool) -> bool {
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

const fn identity_prompt_allowed(
    format: OutputFormat,
    stdin_is_terminal: bool,
    stderr_is_terminal: bool,
) -> bool {
    matches!(format, OutputFormat::Human) && stdin_is_terminal && stderr_is_terminal
}

fn resolve_run_identity(
    cli: &Cli,
    paths: &[PathBuf],
    git_scoped: bool,
    workflow: Workflow,
    cwd: &std::path::Path,
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

fn announce_execution(
    cli: &Cli,
    workflow: Workflow,
    paths: &[PathBuf],
    git_scoped: bool,
    options: &FixOptions,
    advisory: Option<String>,
) -> Result<(), String> {
    let scope = execution_scope(cli, paths, git_scoped, &options.cwd);
    let identity = options.identity.as_ref().map_or_else(
        || "unavailable (headers will be reported)".to_owned(),
        |identity| identity.email.clone(),
    );
    let backups = match &options.backup {
        BackupPolicy::Automatic => "automatic external backup".to_owned(),
        BackupPolicy::Directory(path) => format!("external directory {}", path.display()),
        BackupPolicy::Disabled => "disabled for ordinary writes".to_owned(),
    };
    let event = ExecutionStart {
        event: "execution_start",
        action: workflow_name(cli, workflow).to_owned(),
        mode: report_mode_name(options.mode).to_owned(),
        current_directory: options.cwd.display().to_string(),
        scope,
        identity,
        identity_source: options.identity_source.clone(),
        workers: options
            .threads
            .map_or_else(|| "auto".to_owned(), |count| count.to_string()),
        timeout_seconds: options.timeout.as_secs_f64(),
        norminette: options.norminette_executable.as_ref().map_or_else(
            || "automatic PATH discovery".to_owned(),
            |path| path.display().to_string(),
        ),
        norminette_version_policy: if options.strict_norminette_version {
            "strict (tested release required)"
        } else {
            "advisory (other releases continue)"
        }
        .to_owned(),
        compiler_preflight: options.compiler_preflight,
        cache: options.cache,
        respect_gitignore: options.respect_gitignore,
        backups,
        destructive: destructive_description(options),
        forced: cli.force,
        advisory,
    };
    match cli.format {
        OutputFormat::Human => eprint!("{}", event.to_human()),
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
) -> String {
    if git_scoped {
        format!(
            "Git {} in {} ({} selected file(s))",
            if cli.staged { "staged" } else { "changed" },
            cwd.display(),
            paths.len()
        )
    } else if paths.is_empty() {
        format!("{} (recursive)", cwd.display())
    } else {
        let mut selected = paths
            .iter()
            .take(3)
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>();
        if paths.len() > selected.len() {
            selected.push(format!("+{} more", paths.len() - selected.len()));
        }
        selected.join(", ")
    }
}

fn destructive_description(options: &FixOptions) -> String {
    let mut destructive = Vec::new();
    if options.remove_invalid_comments {
        destructive.push("invalid comments");
    }
    if options.compact_null_checks {
        destructive.push("NULL-check compaction");
    }
    if options.remove_missing_makefile_sources {
        destructive.push("missing or trivia-only Makefile entries");
    }
    if options.remove_orphan_prototypes {
        destructive.push("orphan header prototypes");
    }
    if options.remove_unused_static {
        destructive.push("unreachable static functions");
    }
    if options.quarantine_unexpected {
        destructive.push("unexpected-file quarantine");
    }
    if destructive.is_empty() {
        "none".to_owned()
    } else {
        destructive.join(", ")
    }
}

const fn workflow_name(cli: &Cli, workflow: Workflow) -> &'static str {
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

const fn report_mode_name(mode: ReportMode) -> &'static str {
    match mode {
        ReportMode::Fix => "write",
        ReportMode::Check => "read-only check",
        ReportMode::Diff => "read-only diff",
    }
}

fn render_report(cli: &Cli, report: &RunReport) -> Result<(), String> {
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
                    },
                )
            );
            Ok(())
        }
        OutputFormat::Json => report
            .to_pretty_json()
            .map(|json| print!("{json}"))
            .map_err(|error| format!("Could not serialize the run report: {error}")),
    }
}

fn run_interactive(cli: &Cli, paths: &[PathBuf], options: &FixOptions) -> ExitCode {
    if cli.format != OutputFormat::Human
        || !io::stdin().is_terminal()
        || !io::stdout().is_terminal()
        || !io::stderr().is_terminal()
    {
        print_run_error(
            cli.format,
            "--interactive requires a human terminal on standard input, output, and error",
        );
        return ExitCode::from(2);
    }
    if options.mode != ReportMode::Fix || options.lint_only {
        print_run_error(
            cli.format,
            "--interactive is available with the default or `format` workflow, without --check or --diff",
        );
        return ExitCode::from(2);
    }
    if options.remove_invalid_comments
        || options.compact_null_checks
        || options.remove_missing_makefile_sources
        || options.remove_unused_static
        || options.quarantine_unexpected
    {
        print_run_error(
            cli.format,
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
                &format!("Could not capture the interactive run clock: {error}"),
            );
            return ExitCode::from(2);
        }
    };
    let preview = match run_fixes(paths, &preview_options) {
        Ok(report) => report,
        Err(error) => {
            print_run_error(cli.format, &error.to_string());
            return ExitCode::from(2);
        }
    };
    if let Err(message) = render_report(cli, &preview) {
        print_run_error(cli.format, &message);
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
            print_run_error(cli.format, &error.to_string());
            return ExitCode::from(2);
        }
    };
    if let Err(message) = render_report(cli, &report) {
        print_run_error(cli.format, &message);
        return ExitCode::from(2);
    }
    let code = report.exit_code();
    ExitCode::from(if code == 0 && declined { 1 } else { code })
}

/// Collects per-file approvals, or `None` when the run was cancelled.
fn prompt_for_approvals(
    candidates: &[&FileReport],
    cwd: &std::path::Path,
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

fn interactive_absolute_path(cwd: &std::path::Path, path: &std::path::Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    }
}

fn interactive_approval(
    cwd: &std::path::Path,
    file: &normfix_report::FileReport,
) -> Option<(PathBuf, WriteApproval)> {
    let original = file.original.as_deref()?;
    let fixed = file.fixed.as_deref()?;
    Some((
        interactive_absolute_path(cwd, file.path.as_std_path()),
        WriteApproval::new(original.as_bytes(), fixed.as_bytes()),
    ))
}

fn selected_workflow(cli: &Cli) -> (Vec<PathBuf>, Workflow) {
    match &cli.command {
        Some(Command::Format(arguments)) => (arguments.paths.clone(), Workflow::Format),
        Some(Command::Lint(arguments)) => (arguments.paths.clone(), Workflow::Lint),
        Some(Command::Check(arguments)) => (arguments.paths.clone(), Workflow::Check),
        Some(Command::Budget(arguments)) => (arguments.paths.clone(), Workflow::Budget),
        Some(Command::Preflight(arguments)) => (arguments.paths.clone(), Workflow::Preflight),
        Some(Command::Explain(_) | Command::Undo(_) | Command::Upgrade(_)) => {
            (Vec::new(), Workflow::Default)
        }
        None => (cli.paths.clone(), Workflow::Default),
    }
}

fn run_explain(format: OutputFormat, rule: &str) -> ExitCode {
    let canonical = rule.trim().to_ascii_uppercase();
    let Some(explanation) = rules::explain(&canonical) else {
        print_run_error(
            format,
            &format!(
                "No bundled explanation exists for `{canonical}`. The rule remains available in the normal diagnostic report."
            ),
        );
        return ExitCode::from(2);
    };
    match format {
        OutputFormat::Human => print!("{explanation}"),
        OutputFormat::Json => {
            let value = serde_json::json!({
                "schema_version": normfix_report::REPORT_SCHEMA_VERSION,
                "rule_id": canonical,
                "explanation": explanation,
            });
            println!(
                "{}",
                serde_json::to_string_pretty(&value).expect("explanation JSON is serializable")
            );
        }
    }
    ExitCode::SUCCESS
}

fn run_undo(cli: &Cli, arguments: &UndoArguments, cwd: &std::path::Path) -> ExitCode {
    let runs = match collect_undo_runs(cli.backup_dir.as_deref(), cwd) {
        Ok(runs) => runs,
        Err(message) => {
            print_run_error(cli.format, &message);
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
        print_run_error(cli.format, &detail);
        return ExitCode::from(2);
    };
    if !cli.force {
        if let Err(message) = confirm_undo(selected, cli.format) {
            print_run_error(cli.format, &message);
            return ExitCode::from(2);
        }
    }
    let Some(backup_root) = selected.journal.parent().and_then(std::path::Path::parent) else {
        print_run_error(
            cli.format,
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
            print_run_error(cli.format, &error.to_string());
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

fn backup_roots(explicit: Option<&std::path::Path>) -> Vec<PathBuf> {
    if let Some(path) = explicit {
        return vec![path.to_path_buf()];
    }
    let base = env::var_os("XDG_DATA_HOME")
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            env::var_os("HOME")
                .filter(|path| !path.is_empty())
                .map(PathBuf::from)
                .map(|home| home.join(".local/share"))
        });
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
        OutputFormat::Json => println!(
            "{}",
            serde_json::to_string_pretty(runs).expect("undo list JSON is serializable")
        ),
    }
}

fn confirm_undo(run: &UndoRun, format: OutputFormat) -> Result<(), String> {
    if format == OutputFormat::Json || !io::stdin().is_terminal() || !io::stderr().is_terminal() {
        return Err("undo requires an interactive y/N confirmation or --force".to_owned());
    }
    eprintln!(
        "Restore {} file(s) from {}? Later edits are protected and will cause refusal.",
        run.files.len(),
        run.run_id
    );
    eprint!("Continue? [y/N] ");
    let _ = io::stderr().flush();
    let mut answer = String::new();
    let confirmed = io::stdin()
        .read_line(&mut answer)
        .is_ok_and(|_| answer.trim().eq_ignore_ascii_case("y"));
    confirmed
        .then_some(())
        .ok_or_else(|| "undo was cancelled; no files were changed".to_owned())
}

fn authorize_destructive(
    destructive: DestructiveFlags,
    force: bool,
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
        return Err(
            "destructive operations require an interactive y/N confirmation or --force".to_owned(),
        );
    }
    eprintln!(
        "WARNING: this run may remove proven-dead static code, proven-missing or trivia-only Makefile entries, unused missing-implementation header prototypes, and/or move unexpected files."
    );
    eprint!("Continue with recoverable destructive operations? [y/N] ");
    let _ = io::stderr().flush();
    let mut answer = String::new();
    let confirmed = io::stdin()
        .read_line(&mut answer)
        .is_ok_and(|_| answer.trim().eq_ignore_ascii_case("y"));
    request
        .authorize_yes(confirmed)
        .map(Some)
        .map_err(|_| "destructive operations were cancelled; no files were changed".to_owned())
}

fn prompt_for_identity(
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

fn parse_worker_count(value: &str) -> Result<usize, String> {
    let workers = value
        .parse::<usize>()
        .map_err(|_| "worker count must be a positive integer".to_owned())?;
    (workers > 0)
        .then_some(workers)
        .ok_or_else(|| "worker count must be at least one".to_owned())
}

fn parse_pass_count(value: &str) -> Result<usize, String> {
    let passes = value
        .parse::<usize>()
        .map_err(|_| "pass count must be a positive integer".to_owned())?;
    (passes > 0)
        .then_some(passes)
        .ok_or_else(|| "pass count must be at least one".to_owned())
}

fn parse_timeout(value: &str) -> Result<Duration, String> {
    let seconds = value
        .parse::<f64>()
        .map_err(|_| "timeout must be a positive number of seconds".to_owned())?;
    if !seconds.is_finite() || seconds <= 0.0 {
        return Err("timeout must be finite and greater than zero".to_owned());
    }
    Duration::try_from_secs_f64(seconds)
        .map_err(|_| "timeout is outside the supported range".to_owned())
}

fn print_run_error(format: OutputFormat, message: &str) {
    match format {
        OutputFormat::Human => {
            eprintln!("normfix");
            eprintln!("error: {}", terminal_safe_inline(message));
            eprintln!("No unvalidated changes were written.");
        }
        OutputFormat::Json => {
            let value = serde_json::json!({
                "schema_version": normfix_report::REPORT_SCHEMA_VERSION,
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
            authorize_destructive(destructive, cli.force, OutputFormat::Json)
                .expect("no destructive request")
                .is_none()
        );
    }

    #[test]
    fn destructive_noninteractive_runs_still_require_force() {
        let destructive = DestructiveFlags {
            remove_unused: true,
            remove_missing_makefile_sources: false,
            remove_orphan_prototypes: false,
            remove_unexpected: false,
        };
        let error = authorize_destructive(destructive, false, OutputFormat::Json)
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
