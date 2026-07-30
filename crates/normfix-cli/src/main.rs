//! Production command-line interface for the native fixer.

#![forbid(unsafe_code)]

use std::env;
use std::io::{self, IsTerminal, Write as _};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use clap::{Parser, ValueEnum};
use normfix_destructive::{DestructiveAuthorization, DestructiveCapability, DestructiveRequest};
use normfix_engine::{BackupPolicy, FixOptions, run_fixes};
use normfix_header::{IdentityResolution, identity_from_email, resolve_identity};
use normfix_report::{RenderOptions, ReportMode, render_human};

// Clap represents independent switches as booleans; replacing them with one
// state enum would incorrectly make compatible command-line flags exclusive.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Parser)]
#[command(name = "norminette-fix")]
#[command(version)]
#[command(about = "Safe automatic fixes and actionable diagnostics for the 42 Norm v4.1")]
#[command(after_help = "With no PATH, the current directory is processed recursively.")]
struct Cli {
    /// Files or directories; accepts zero, one or many paths.
    paths: Vec<PathBuf>,

    /// Report changes without writing files.
    #[arg(long, conflicts_with = "diff")]
    check: bool,

    /// Print unified diffs without writing files.
    #[arg(long)]
    diff: bool,

    /// Respect .gitignore while recursively discovering directory inputs.
    #[arg(long)]
    use_gitignore: bool,

    /// Verified 42 login; the email remains the source of truth.
    #[arg(long)]
    login: Option<String>,

    /// Verified 42 student email used by official headers.
    #[arg(long)]
    email: Option<String>,

    /// Do not retain external backups for ordinary formatting writes.
    #[arg(long, conflicts_with = "backup_dir")]
    no_backup: bool,

    /// External backup base directory.
    #[arg(long, value_name = "PATH")]
    backup_dir: Option<PathBuf>,

    /// Select polished terminal output or stable JSON.
    #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
    format: OutputFormat,

    /// Disable ANSI colors even on an interactive terminal.
    #[arg(long)]
    no_color: bool,

    /// Show every accepted fix in human output.
    #[arg(long, short)]
    verbose: bool,

    /// Per-file official Norminette timeout in seconds.
    #[arg(long, default_value = "5", value_parser = parse_timeout)]
    timeout: Duration,

    /// Number of parallel workers; defaults to available hardware.
    #[arg(long, value_parser = parse_worker_count)]
    threads: Option<usize>,

    /// Delete only comments rejected at exact official locations.
    #[arg(long)]
    remove_invalid_comments: bool,

    /// Remove only unreachable static functions proven in the complete project.
    #[arg(long)]
    remove_unused: bool,

    /// Move unexpected regular files to external recoverable quarantine.
    #[arg(long)]
    remove_unexpected: bool,

    /// Enable comment removal, unused-static removal and file quarantine.
    #[arg(long = "unsafe")]
    unsafe_mode: bool,

    /// Confirm destructive operations non-interactively.
    #[arg(long)]
    force: bool,

    /// Canonically format README documents through a `CommonMark` syntax tree.
    #[arg(long)]
    format_markdown: bool,

    /// Disable the external content-addressed analysis cache.
    #[arg(long)]
    no_cache: bool,

    /// Use this exact Norminette executable.
    #[arg(long, value_name = "PATH")]
    norminette: Option<PathBuf>,

    /// Maximum fixed-point passes for the native formatter.
    #[arg(long, hide = true, default_value_t = 100, value_parser = parse_pass_count)]
    max_passes: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum OutputFormat {
    Human,
    Json,
}

fn main() -> ExitCode {
    run(Cli::parse())
}

fn run(cli: Cli) -> ExitCode {
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
    let mut identity = resolve_identity(cli.login.as_deref(), cli.email.as_deref(), &cwd);
    if identity.identity.is_none()
        && cli.format == OutputFormat::Human
        && io::stdin().is_terminal()
        && io::stderr().is_terminal()
    {
        identity = prompt_for_identity(cli.login.as_deref(), identity);
    }
    let remove_unused = cli.remove_unused || cli.unsafe_mode;
    let remove_unexpected = cli.remove_unexpected || cli.unsafe_mode;
    let destructive_authorization =
        match authorize_destructive(remove_unused, remove_unexpected, cli.force, cli.format) {
            Ok(authorization) => authorization,
            Err(message) => {
                print_run_error(cli.format, &message);
                return ExitCode::from(2);
            }
        };
    if cli.force && destructive_authorization.is_none() {
        print_run_error(
            cli.format,
            "--force requires --unsafe, --remove-unused, or --remove-unexpected",
        );
        return ExitCode::from(2);
    }

    let mut options = FixOptions::new(cwd);
    options.mode = if cli.diff {
        ReportMode::Diff
    } else if cli.check {
        ReportMode::Check
    } else {
        ReportMode::Fix
    };
    options.respect_gitignore = cli.use_gitignore;
    options.threads = cli.threads;
    options.identity_source = identity.source;
    options.identity = identity.identity;
    options.backup = if cli.no_backup {
        BackupPolicy::Disabled
    } else if let Some(directory) = cli.backup_dir {
        BackupPolicy::Directory(directory)
    } else {
        BackupPolicy::Automatic
    };
    options.norminette_executable = cli.norminette;
    options.timeout = cli.timeout;
    options.cache = !cli.no_cache;
    options.remove_invalid_comments = cli.remove_invalid_comments || cli.unsafe_mode;
    options.remove_unused_static = remove_unused;
    options.quarantine_unexpected = remove_unexpected;
    options.destructive_authorization = destructive_authorization;
    options.format_markdown = cli.format_markdown;
    options.max_passes = cli.max_passes;

    match run_fixes(&cli.paths, &options) {
        Ok(report) => {
            match cli.format {
                OutputFormat::Human => {
                    let color = !cli.no_color
                        && env::var_os("NO_COLOR").is_none()
                        && io::stdout().is_terminal();
                    print!(
                        "{}",
                        render_human(
                            &report,
                            RenderOptions {
                                color,
                                verbose: cli.verbose,
                                show_diff: cli.diff,
                            },
                        )
                    );
                }
                OutputFormat::Json => match report.to_pretty_json() {
                    Ok(json) => print!("{json}"),
                    Err(error) => {
                        print_run_error(
                            OutputFormat::Json,
                            &format!("Could not serialize the run report: {error}"),
                        );
                        return ExitCode::from(2);
                    }
                },
            }
            ExitCode::from(report.exit_code())
        }
        Err(error) => {
            print_run_error(cli.format, &error.to_string());
            ExitCode::from(2)
        }
    }
}

fn authorize_destructive(
    remove_unused: bool,
    remove_unexpected: bool,
    force: bool,
    format: OutputFormat,
) -> Result<Option<DestructiveAuthorization>, String> {
    let mut capabilities = Vec::new();
    if remove_unused {
        capabilities.push(DestructiveCapability::RemoveUnreferencedStaticFunctions);
    }
    if remove_unexpected {
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
    eprintln!("WARNING: this run may delete unreachable static code and/or move unexpected files.");
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
            resolution.source
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
            eprintln!("norminette-fix");
            eprintln!("error: {message}");
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
    use std::time::Duration;

    use clap::Parser;

    use super::{Cli, OutputFormat};

    #[test]
    fn accepts_zero_one_or_many_paths_like_norminette() {
        for expected in 0..=2 {
            let arguments = match expected {
                0 => vec!["norminette-fix"],
                1 => vec!["norminette-fix", "main.c"],
                _ => vec!["norminette-fix", "src", "include/demo.h"],
            };
            let parsed = Cli::try_parse_from(arguments).expect("valid CLI");
            assert_eq!(parsed.paths.len(), expected);
        }
    }

    #[test]
    fn parses_preview_identity_performance_and_output_flags() {
        let parsed = Cli::try_parse_from([
            "norminette-fix",
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
        assert!(parsed.no_cache);
        assert_eq!(parsed.paths, vec![PathBuf::from("src")]);
    }

    #[test]
    fn rejects_conflicting_previews_and_invalid_limits() {
        assert!(Cli::try_parse_from(["norminette-fix", "--check", "--diff"]).is_err());
        assert!(Cli::try_parse_from(["norminette-fix", "--threads", "0"]).is_err());
        assert!(Cli::try_parse_from(["norminette-fix", "--timeout", "nan"]).is_err());
    }
}
