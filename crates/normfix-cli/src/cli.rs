//! Command-line grammar and workflow selection.

use std::path::PathBuf;
use std::time::Duration;

use clap::{Args, Parser, Subcommand, ValueEnum};

// Clap represents independent switches as booleans; replacing them with one
// state enum would incorrectly make compatible command-line flags exclusive.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Parser)]
#[command(name = "normfix")]
#[command(version)]
#[command(about = "Safe automatic fixes and actionable diagnostics for the 42 Norm v4.1")]
#[command(subcommand_precedence_over_arg = true)]
#[command(after_help = "With no COMMAND or PATH, the current directory is fixed recursively.")]
pub(crate) struct Cli {
    /// Files or directories; accepts zero, one or many paths.
    pub(crate) paths: Vec<PathBuf>,

    /// Focused workflows; the commandless interface remains backward compatible.
    #[command(subcommand)]
    pub(crate) command: Option<Command>,

    /// Report changes without writing files.
    #[arg(long, global = true, conflicts_with = "diff")]
    pub(crate) check: bool,

    /// Print unified diffs without writing files.
    #[arg(long, global = true)]
    pub(crate) diff: bool,

    /// Respect .gitignore while recursively discovering directory inputs.
    #[arg(long, global = true)]
    pub(crate) use_gitignore: bool,

    /// Verified 42 login; the email remains the source of truth.
    #[arg(long, global = true)]
    pub(crate) login: Option<String>,

    /// Verified 42 student email used by official headers.
    #[arg(long, global = true)]
    pub(crate) email: Option<String>,

    /// Do not retain external backups for ordinary formatting writes.
    #[arg(long, global = true, conflicts_with = "backup_dir")]
    pub(crate) no_backup: bool,

    /// External backup base directory.
    #[arg(long, global = true, value_name = "PATH")]
    pub(crate) backup_dir: Option<PathBuf>,

    /// Select polished terminal output or stable JSON.
    #[arg(long, global = true, value_enum, default_value_t = OutputFormat::Human)]
    pub(crate) format: OutputFormat,

    /// Language for human output: en, pt, es, or fr.
    ///
    /// JSON, rule IDs, flags, and exit codes stay language-neutral. Without
    /// this flag the process locale is used, falling back to English.
    #[arg(long, global = true, value_name = "CODE")]
    pub(crate) lang: Option<String>,

    /// Disable ANSI colors even on an interactive terminal.
    #[arg(long, global = true)]
    pub(crate) no_color: bool,

    /// Show every accepted fix in human output.
    #[arg(long, short, global = true)]
    pub(crate) verbose: bool,

    /// Preview and approve each changed file before a validated write run.
    #[arg(long, global = true)]
    pub(crate) interactive: bool,

    /// Process unstaged tracked changes and untracked, non-ignored files.
    #[arg(long, global = true, conflicts_with = "staged")]
    pub(crate) changed: bool,

    /// Process only files currently recorded in the Git index.
    #[arg(long, global = true)]
    pub(crate) staged: bool,

    /// Per-file official Norminette timeout in seconds.
    #[arg(long, global = true, default_value = "5", value_parser = parse_timeout)]
    pub(crate) timeout: Duration,

    /// Number of parallel workers; defaults to available hardware.
    #[arg(long, global = true, value_parser = parse_worker_count)]
    pub(crate) threads: Option<usize>,

    /// Delete only comments rejected at exact official locations.
    #[arg(long, global = true)]
    pub(crate) remove_invalid_comments: bool,

    /// Remove only unreachable static functions proven in the complete project.
    #[arg(long, global = true)]
    pub(crate) remove_unused: bool,

    /// Move unexpected regular files to external recoverable quarantine.
    #[arg(long, global = true)]
    pub(crate) remove_unexpected: bool,

    /// Enable comment/NULL cleanup, stale Makefile cleanup, unused-static
    /// removal, and unexpected-file quarantine.
    #[arg(long = "unsafe", global = true)]
    pub(crate) unsafe_mode: bool,

    /// Confirm destructive operations or acknowledge a protected system scope.
    #[arg(long, global = true)]
    pub(crate) force: bool,

    /// Legacy no-op: README formatting is enabled by default.
    #[arg(long, global = true, hide = true)]
    pub(crate) format_markdown: bool,

    /// Leave README documents unchanged.
    #[arg(long, global = true, conflicts_with = "format_markdown")]
    pub(crate) no_format_markdown: bool,

    /// Leave contiguous include blocks in their current order.
    #[arg(long, global = true)]
    pub(crate) no_reorder_includes: bool,

    /// Disable the external content-addressed analysis cache.
    #[arg(long, global = true)]
    pub(crate) no_cache: bool,

    /// Use this exact Norminette executable.
    #[arg(long, global = true, value_name = "PATH")]
    pub(crate) norminette: Option<PathBuf>,

    /// Deprecated no-op: untested Norminette releases now run with an advisory.
    #[arg(
        long,
        global = true,
        hide = true,
        conflicts_with = "strict_norminette_version"
    )]
    pub(crate) allow_untested_norminette: bool,

    /// Refuse Norminette releases this normfix version has not verified.
    #[arg(long, global = true)]
    pub(crate) strict_norminette_version: bool,

    /// Disable `cc -fsyntax-only -Wall -Wextra -Werror` diagnostics.
    #[arg(long, global = true)]
    pub(crate) no_compiler_preflight: bool,

    /// Use this exact C compiler for strict preflight and optional analysis.
    #[arg(long, global = true, value_name = "PATH")]
    pub(crate) cc: Option<PathBuf>,

    /// Use this exact clang-tidy for the optional preflight lens.
    #[arg(long, global = true, value_name = "PATH")]
    pub(crate) clang_tidy: Option<PathBuf>,

    /// Run GCC `-fanalyzer` as a slower informational check.
    #[arg(long, global = true)]
    pub(crate) analyzer: bool,

    /// Maximum fixed-point passes for the native formatter.
    #[arg(long, global = true, hide = true, default_value_t = 100, value_parser = parse_pass_count)]
    pub(crate) max_passes: usize,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Apply the canonical layout printer and proven safe fixes.
    Format(PathArguments),
    /// Report source/project problems; edit and removal flags are rejected.
    Lint(PathArguments),
    /// Preview formatting and lint together without writing files.
    Check(PathArguments),
    /// Show function headroom; edit and removal flags are rejected.
    Budget(PathArguments),
    /// Run the read-only checks useful immediately before a 42 evaluation.
    Preflight(PathArguments),
    /// Explain one Norm or native rule offline.
    Explain(ExplainArguments),
    /// Restore an intact backed-up run without overwriting later edits.
    Undo(UndoArguments),
    /// Replace this binary with the newest published release.
    Upgrade(UpgradeArguments),
    /// Remove this binary, and optionally the data it created.
    Uninstall(UninstallArguments),
    /// Run a program you already built under a leak checker.
    ///
    /// This is the one command that executes your code rather than reading it,
    /// so it asks first. normfix never builds the program: point it at one.
    Leaks(LeaksArguments),
}

#[derive(Debug, Args)]
pub(crate) struct LeaksArguments {
    /// The already-built program to run.
    pub(crate) program: PathBuf,
    /// Arguments passed to your program, not to the checker.
    #[arg(last = true)]
    pub(crate) program_arguments: Vec<String>,
}

#[derive(Debug, Args)]
pub(crate) struct UninstallArguments {
    /// Also remove configuration, cache, backups, and quarantined files.
    #[arg(long)]
    pub(crate) purge: bool,
    /// Show exactly what would be removed and stop.
    #[arg(long)]
    pub(crate) dry_run: bool,
}

#[derive(Debug, Args)]
pub(crate) struct UpgradeArguments {
    /// Report whether a newer release exists without installing it.
    #[arg(long)]
    pub(crate) check: bool,
}

#[derive(Debug, Args)]
pub(crate) struct PathArguments {
    /// Files or directories; defaults to the current directory.
    pub(crate) paths: Vec<PathBuf>,
}

#[derive(Debug, Args)]
pub(crate) struct ExplainArguments {
    /// Rule identifier, for example `TOO_MANY_LINES`.
    pub(crate) rule: String,
}

#[derive(Debug, Args)]
pub(crate) struct UndoArguments {
    /// List recovery points without restoring anything.
    #[arg(long, conflicts_with = "run")]
    pub(crate) list: bool,
    /// Restore this exact run instead of the newest intact run.
    #[arg(long, value_name = "RUN_ID", conflicts_with = "list")]
    pub(crate) run: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(crate) enum OutputFormat {
    Human,
    Json,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Workflow {
    Default,
    Format,
    Lint,
    Check,
    Budget,
    Preflight,
}

/// Returns the selected paths and effective workflow.
pub(crate) fn selected_workflow(cli: &Cli) -> (Vec<PathBuf>, Workflow) {
    match &cli.command {
        Some(Command::Format(arguments)) => (arguments.paths.clone(), Workflow::Format),
        Some(Command::Lint(arguments)) => (arguments.paths.clone(), Workflow::Lint),
        Some(Command::Check(arguments)) => (arguments.paths.clone(), Workflow::Check),
        Some(Command::Budget(arguments)) => (arguments.paths.clone(), Workflow::Budget),
        Some(Command::Preflight(arguments)) => (arguments.paths.clone(), Workflow::Preflight),
        Some(
            Command::Explain(_)
            | Command::Undo(_)
            | Command::Upgrade(_)
            | Command::Uninstall(_)
            | Command::Leaks(_),
        ) => (Vec::new(), Workflow::Default),
        None => (cli.paths.clone(), Workflow::Default),
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
