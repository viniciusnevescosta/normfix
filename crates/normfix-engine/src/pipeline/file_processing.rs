//! Per-file loading, shadow formatting, official validation, and reporting.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use camino::Utf8PathBuf;
use normfix_actions::PlannedFile;
use normfix_c_actions::{
    CActionError, CActionOptions, ReportedDiagnostic, analyze_budget, analyze_c, apply_c_actions,
};
use normfix_c_syntax::{CFunctionKind, CParser};
use normfix_core::{Diagnostic, DiagnosticSource, FixRecord, Severity, TextRange, TextSize};
use normfix_header::{RunClock, c_header_filename_matches, ensure_c_header, update_c_header};
use normfix_project::{
    DiscoveredFile, GuardApproval, GuardInsertionApproval, ProjectFileKind,
    guard_approval_is_current, guard_insertion_approval_is_current,
};
use normfix_report::FileReport;

use super::compiler::run_compiler_preflight;
use super::destructive::{DestructivePrelude, snapshot_preconditions};
use super::diagnostics::{
    explain_constant_array_false_positives, introduces_diagnostics, line_point_range,
    merge_official_diagnostics, official_diagnostics, parser_diagnostics, point_diagnostic,
    project_diagnostic, suppress_official_structural_duplicates, untested_norminette_diagnostic,
};
use super::makefile::process_makefile;
use super::markdown::process_markdown;
use super::paths::report_path;
use super::source_io::read_project_file;
use super::{FileWork, FixOptions, OracleContext, append_header_fixes, append_header_issues};

#[allow(clippy::too_many_arguments)]
pub(super) fn process_file(
    file: &DiscoveredFile,
    options: &FixOptions,
    clock: &RunClock,
    oracle: Option<&OracleContext>,
    guard_approvals: &BTreeMap<PathBuf, GuardApproval>,
    guard_insertions: &BTreeMap<PathBuf, GuardInsertionApproval>,
    guard_failure: Option<&str>,
    destructive_prelude: Option<&DestructivePrelude>,
    policy_diagnostics: Option<&[Diagnostic]>,
) -> FileWork {
    let relative = report_path(&file.path, &options.cwd)
        .unwrap_or_else(|_| Utf8PathBuf::from(file.path.to_string_lossy().as_ref()));
    let original = match read_project_file(&file.path) {
        Ok(bytes) => bytes,
        Err(error) => {
            return failed_file(file, relative, format!("Could not read this file: {error}"));
        }
    };
    if original.contains(&0) {
        return failed_file(
            file,
            relative,
            "Refused a file containing NUL bytes.".to_owned(),
        );
    }
    let source = match String::from_utf8(original.clone()) {
        Ok(source) => source,
        Err(error) => {
            return failed_file(
                file,
                relative,
                format!("The file is not valid UTF-8: {error}"),
            );
        }
    };
    match file.kind {
        ProjectFileKind::CSource | ProjectFileKind::CHeader => {
            let Some(oracle) = oracle else {
                return failed_source(
                    file,
                    relative,
                    original,
                    source,
                    "The required Norminette context was not initialized for this C file."
                        .to_owned(),
                );
            };
            process_c(
                file,
                relative,
                original,
                source,
                options,
                clock,
                oracle,
                guard_approvals,
                guard_insertions,
                guard_failure,
                destructive_prelude,
                policy_diagnostics,
            )
        }
        ProjectFileKind::Makefile => {
            process_makefile(file, relative, &original, source, options, clock)
        }
        ProjectFileKind::Markdown => process_markdown(file, relative, original, source, options),
    }
}

pub(super) fn failed_file(file: &DiscoveredFile, path: Utf8PathBuf, failure: String) -> FileWork {
    FileWork {
        absolute_path: file.path.clone(),
        report: FileReport {
            budget: Vec::new(),
            path,
            changed: false,
            written: false,
            backup: None,
            failure: Some(failure),
            fixes: Vec::new(),
            before: Vec::new(),
            after: Vec::new(),
            original: None,
            fixed: None,
        },
        plan: None,
        read_preconditions: Vec::new(),
    }
}

/// Bounded number of official-checker rounds for one file.
///
/// One round converges the native actions; a second exposes the rules that only
/// become visible once layout is correct. A third has never been needed in
/// practice and exists so a pathological file cannot spend checker calls
/// forever.
const MAX_ORACLE_ROUNDS: usize = 3;

/// Converts one official report into the diagnostics the action crate consumes.
fn reported_diagnostics(report: &normfix_oracle::NorminetteReport) -> Vec<ReportedDiagnostic> {
    report
        .diagnostics
        .iter()
        .map(|item| {
            ReportedDiagnostic::new(
                item.rule_id.clone(),
                item.line,
                item.column,
                item.message.clone(),
            )
        })
        .collect()
}

// Keeping the C stages in one straight-line function makes the shadow-buffer
// proof boundary and each official-oracle checkpoint visible during review.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn process_c(
    file: &DiscoveredFile,
    path: Utf8PathBuf,
    original_bytes: Vec<u8>,
    original: String,
    options: &FixOptions,
    clock: &RunClock,
    oracle: &OracleContext,
    guard_approvals: &BTreeMap<PathBuf, GuardApproval>,
    guard_insertions: &BTreeMap<PathBuf, GuardInsertionApproval>,
    guard_failure: Option<&str>,
    destructive_prelude: Option<&DestructivePrelude>,
    policy_diagnostics: Option<&[Diagnostic]>,
) -> FileWork {
    let norminette_version = oracle.oracle.fingerprint().version.as_str();
    let before_report = match oracle.lint(&file.path, &original) {
        Ok(report) => report,
        Err(error) => {
            return failed_source(
                file,
                path,
                original_bytes,
                original,
                format!("Official Norminette could not inspect this file: {error}"),
            );
        }
    };
    let mut before_official = before_report.diagnostics.clone();
    let before_semantic_advisories = explain_constant_array_false_positives(
        &path,
        &original,
        &mut before_official,
        norminette_version,
    );
    let before = official_diagnostics(&path, &original, &before_official, norminette_version);
    if options.lint_only {
        let remaining_official = before_official;
        let semantic_advisories = before_semantic_advisories;
        let mut after = match analyze_c(path.as_path(), &original, options.max_columns) {
            Ok(diagnostics) => diagnostics,
            Err(_) => parser_diagnostics(&path, &original),
        };
        merge_official_diagnostics(
            &mut after,
            official_diagnostics(&path, &original, &remaining_official, norminette_version),
            options.preflight,
            norminette_version,
        );
        after.extend(semantic_advisories);
        after.extend(policy_diagnostics.unwrap_or_default().iter().cloned());
        after.extend(untested_norminette_diagnostic(oracle, file, &path));
        let budget = if options.emit_budget {
            after.extend(budget_diagnostics(&path, &original));
            function_budgets(&path, &original)
        } else {
            Vec::new()
        };
        if file.kind == ProjectFileKind::CSource {
            after.extend(run_compiler_preflight(
                oracle, options, file, &path, &original, &original,
            ));
        }
        after.sort();
        after.dedup();
        return FileWork {
            absolute_path: file.path.clone(),
            report: FileReport {
                budget,
                path,
                changed: false,
                written: false,
                backup: None,
                failure: None,
                fixes: Vec::new(),
                before,
                after,
                original: Some(Arc::from(original.clone())),
                fixed: Some(Arc::from(original)),
            },
            plan: None,
            read_preconditions: Vec::new(),
        };
    }
    let confirmed_prelude =
        destructive_prelude.filter(|prelude| prelude.confirmed_source(&original_bytes).is_some());
    let mut current = confirmed_prelude
        .and_then(|prelude| prelude.confirmed_source(&original_bytes))
        .map_or_else(|| original.clone(), str::to_owned);
    let pre_action_source = current.clone();
    let mut fixes = confirmed_prelude.map_or_else(Vec::new, |prelude| prelude.fixes.clone());
    let mut local_diagnostics =
        destructive_prelude.map_or_else(Vec::new, |prelude| prelude.diagnostics.clone());
    let mut read_preconditions =
        confirmed_prelude.map_or_else(Vec::new, |prelude| prelude.read_preconditions.clone());
    if destructive_prelude.is_some_and(|prelude| {
        prelude.source.is_some() && prelude.confirmed_source(&original_bytes).is_none()
    }) {
        local_diagnostics.push(project_diagnostic(
            path.clone(),
            "DESTRUCTIVE_PRELUDE_SNAPSHOT_CHANGED",
            "The file changed after destructive planning; the planned deletion was discarded.",
            "Retry against an unchanged project; no stale source bytes were used.",
        ));
    }
    local_diagnostics.extend(policy_diagnostics.unwrap_or_default().iter().cloned());

    let approval_key = file
        .path
        .canonicalize()
        .unwrap_or_else(|_| file.path.clone());
    let guard_changed = guard_approvals
        .get(&approval_key)
        .and_then(|approval| apply_guard_approval(&current, approval))
        .is_some_and(|updated| {
            current = updated;
            fixes.push(FixRecord {
                rule_id: "HEADER_GUARD_RENAME".to_owned(),
                description:
                    "renamed both canonical inclusion-guard tokens after a closed-project proof"
                        .to_owned(),
                line: None,
                count: 2,
            });
            true
        });
    let guard_inserted = guard_insertions
        .get(&approval_key)
        .and_then(|approval| apply_guard_insertion(&current, approval))
        .is_some_and(|updated| {
            current = updated;
            fixes.push(FixRecord {
                rule_id: "HEADER_GUARD_INSERT".to_owned(),
                description:
                    "added one filename-derived whole-file inclusion guard after a closed-project proof"
                        .to_owned(),
                line: None,
                count: 3,
            });
            true
        });
    if guard_changed {
        if let Some(approval) = guard_approvals.get(&approval_key) {
            read_preconditions.extend(snapshot_preconditions(&approval.snapshot));
        }
    }
    if guard_inserted {
        if let Some(approval) = guard_insertions.get(&approval_key) {
            read_preconditions.extend(snapshot_preconditions(&approval.snapshot));
        }
    }

    let filename = file
        .path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    // A byte-order mark has to go before the header is written, not after. The
    // hygiene pass that removes it only looks at the first byte, so a header
    // inserted first pushed the mark into the middle of the file, where the
    // official checker reads it as a stray lexeme on the same line as the first
    // instruction. The run finished, reported success, and left it there.
    if current.starts_with('\u{feff}') {
        current.remove(0);
        fixes.push(FixRecord {
            rule_id: "REMOVE_BOM".to_owned(),
            description: "removed the UTF-8 byte-order mark".to_owned(),
            line: Some(1),
            count: 1,
        });
    }
    let ensured = ensure_c_header(&current, filename, options.identity.as_ref(), clock);
    let header_inserted = ensured.inserted;
    append_header_fixes(&mut fixes, &ensured.fixes, &current);
    append_header_issues(&mut local_diagnostics, &path, &ensured.issues);
    current = ensured.output;

    let baseline_report = match oracle.lint(&file.path, &current) {
        Ok(report) => report,
        Err(error) => {
            return failed_source_with_report(
                file,
                path,
                original_bytes,
                original,
                current,
                fixes,
                before,
                local_diagnostics,
                format!("Official Norminette could not validate the header stage: {error}"),
            );
        }
    };
    let action_fix_start = fixes.len();
    let baseline = current.clone();
    let mut reported = reported_diagnostics(&baseline_report);
    let action_options = CActionOptions {
        max_columns: options.max_columns,
        max_passes: options.max_passes,
        remove_invalid_comments: options.remove_invalid_comments,
        remove_unused_variables: options.remove_unused_variables,
        format_proven_declarations: true,
        compact_null_checks: options.compact_null_checks,
        reorder_includes: options.reorder_includes,
    };
    let mut action_changed = false;
    // Several actions are driven by official diagnostics, and the oracle only
    // ever sees the bytes in front of it. Correcting indentation can expose an
    // alignment rule that was masked before, so a single pass converges the
    // native actions but not the file. Re-lint and run again until nothing
    // changes, bounded because a round that never settles is a defect rather
    // than a reason to keep spending official checker calls.
    for round in 0..MAX_ORACLE_ROUNDS {
        let changed_this_round;
        match apply_c_actions(path.as_path(), &current, &reported, &action_options) {
            Ok(result) => {
                changed_this_round = result.source != current;
                current = result.source;
                fixes.extend(result.fixes.into_iter().map(|item| FixRecord {
                    rule_id: item.rule_id,
                    description: item.description,
                    line: item.line,
                    count: 1,
                }));
            }
            Err(CActionError::UnsafeSyntax) => {
                local_diagnostics.extend(parser_diagnostics(&path, &current));
                break;
            }
            Err(error) => {
                local_diagnostics.push(point_diagnostic(
                    &path,
                    "FIX_PROOF_REJECTED",
                    Severity::Warning,
                    format!("A native edit batch was rejected: {error}"),
                    DiagnosticSource::NativeNorm41,
                    Some(
                        "Review the reported source issue; the rejected batch was not written."
                            .to_owned(),
                    ),
                ));
                break;
            }
        }
        action_changed |= changed_this_round;
        if !changed_this_round || round + 1 == MAX_ORACLE_ROUNDS {
            break;
        }
        // A failure here is reported by the final validation below.
        let Ok(report) = oracle.lint(&file.path, &current) else {
            break;
        };
        reported = reported_diagnostics(&report);
    }

    let should_update_header = !header_inserted
        && (guard_changed
            || guard_inserted
            || action_changed
            || !c_header_filename_matches(&current, filename));
    if should_update_header {
        let updated = update_c_header(&current, filename, options.identity.as_ref(), clock);
        append_header_fixes(&mut fixes, &updated.fixes, &current);
        append_header_issues(&mut local_diagnostics, &path, &updated.issues);
        current = updated.output;
    }

    let mut final_report = match oracle.lint(&file.path, &current) {
        Ok(report) => report,
        Err(error) => {
            return failed_source_with_report(
                file,
                path,
                original_bytes,
                original,
                current,
                fixes,
                before,
                local_diagnostics,
                format!("Official Norminette could not validate the final shadow buffer: {error}"),
            );
        }
    };
    if action_changed
        && introduces_diagnostics(&baseline_report.diagnostics, &final_report.diagnostics)
    {
        current = baseline;
        fixes.truncate(action_fix_start);
        local_diagnostics.push(point_diagnostic(
            &path,
            "FIX_REJECTED_NEW_DIAGNOSTIC",
            Severity::Info,
            "An optional formatting batch was skipped because it introduced a new Norminette diagnostic."
                .to_owned(),
            DiagnosticSource::NorminetteCompat(
                norminette_version.to_owned(),
            ),
            Some(
                "The complete batch was reverted; the original source remains authoritative."
                    .to_owned(),
            ),
        ));
        final_report = match oracle.lint(&file.path, &current) {
            Ok(report) => report,
            Err(error) => {
                return failed_source_with_report(
                    file,
                    path,
                    original_bytes,
                    original,
                    current,
                    fixes,
                    before,
                    local_diagnostics,
                    format!("Official Norminette could not validate the reverted buffer: {error}"),
                );
            }
        };
    }

    let mut remaining_official = final_report.diagnostics;
    let semantic_advisories = explain_constant_array_false_positives(
        &path,
        &current,
        &mut remaining_official,
        norminette_version,
    );
    local_diagnostics.extend(untested_norminette_diagnostic(oracle, file, &path));
    if file.kind == ProjectFileKind::CSource {
        local_diagnostics.extend(run_compiler_preflight(
            oracle, options, file, &path, &original, &current,
        ));
    }
    let mut after = match analyze_c(path.as_path(), &current, options.max_columns) {
        Ok(diagnostics) => diagnostics,
        Err(_) => parser_diagnostics(&path, &current),
    };
    suppress_official_structural_duplicates(&mut after, &remaining_official);
    merge_official_diagnostics(
        &mut after,
        official_diagnostics(&path, &current, &remaining_official, norminette_version),
        options.preflight,
        norminette_version,
    );
    after.extend(semantic_advisories);
    remap_orphan_prototype_diagnostics(&mut local_diagnostics, &pre_action_source, &current);
    after.extend(local_diagnostics);
    if let Some(reason) = guard_failure {
        if file.kind == ProjectFileKind::CHeader
            && remaining_official
                .iter()
                .any(|diagnostic| diagnostic.rule_id.starts_with("HEADER_PROT"))
        {
            after.push(point_diagnostic(
                &path,
                "HEADER_GUARD_PROOF_UNAVAILABLE",
                Severity::Warning,
                format!("The inclusion guard was left unchanged: {reason}"),
                DiagnosticSource::Project,
                Some(
                    "Use a canonical whole-file guard and remove project-wide macro ambiguity."
                        .to_owned(),
                ),
            ));
        }
    }
    after.sort();
    after.dedup();
    let changed = current.as_bytes() != original_bytes;
    let original_arc: Arc<str> = Arc::from(original.clone());
    let fixed_arc: Arc<str> = Arc::from(current.clone());
    let plan = changed.then(|| PlannedFile {
        path: file.path.clone(),
        original: original_bytes.clone(),
        replacement: current.as_bytes().to_vec(),
        fixes: fixes.clone(),
    });
    FileWork {
        absolute_path: file.path.clone(),
        report: FileReport {
            budget: Vec::new(),
            path,
            changed,
            written: false,
            backup: None,
            failure: None,
            fixes,
            before,
            after,
            original: Some(original_arc),
            fixed: Some(fixed_arc),
        },
        plan,
        read_preconditions,
    }
}

fn remap_orphan_prototype_diagnostics(diagnostics: &mut [Diagnostic], before: &str, after: &str) {
    if before == after
        || !diagnostics.iter().any(|diagnostic| {
            matches!(
                diagnostic.rule_id.as_str(),
                "HEADER_PROTOTYPE_IMPLEMENTATION_MISSING"
                    | "HEADER_PROTOTYPE_IMPLEMENTATION_EMPTY"
                    | "UNSAFE_ORPHAN_PROTOTYPE_PROOF_BLOCKED"
            )
        })
    {
        return;
    }
    let parsed = CParser::new().and_then(|mut parser| {
        Ok((
            parser.parse(before)?.facts().clone(),
            parser.parse(after)?.facts().clone(),
        ))
    });
    let Ok((before_facts, after_facts)) = parsed else {
        for diagnostic in diagnostics.iter_mut().filter(|diagnostic| {
            matches!(
                diagnostic.rule_id.as_str(),
                "HEADER_PROTOTYPE_IMPLEMENTATION_MISSING"
                    | "HEADER_PROTOTYPE_IMPLEMENTATION_EMPTY"
                    | "UNSAFE_ORPHAN_PROTOTYPE_PROOF_BLOCKED"
            )
        }) {
            diagnostic.range = TextRange::empty(TextSize::new(0));
        }
        return;
    };
    for diagnostic in diagnostics.iter_mut().filter(|diagnostic| {
        matches!(
            diagnostic.rule_id.as_str(),
            "HEADER_PROTOTYPE_IMPLEMENTATION_MISSING"
                | "HEADER_PROTOTYPE_IMPLEMENTATION_EMPTY"
                | "UNSAFE_ORPHAN_PROTOTYPE_PROOF_BLOCKED"
        )
    }) {
        let before_prototypes = before_facts
            .functions
            .iter()
            .filter(|function| function.kind == CFunctionKind::Prototype)
            .collect::<Vec<_>>();
        let Some((name, ordinal)) = before_prototypes
            .iter()
            .find(|function| function.name_range == diagnostic.range)
            .map(|target| {
                let ordinal = before_prototypes
                    .iter()
                    .filter(|function| function.name == target.name)
                    .position(|function| function.name_range == target.name_range)
                    .unwrap_or(0);
                (target.name.as_str(), ordinal)
            })
        else {
            diagnostic.range = TextRange::empty(TextSize::new(0));
            continue;
        };
        diagnostic.range = after_facts
            .functions
            .iter()
            .filter(|function| function.kind == CFunctionKind::Prototype && function.name == name)
            .nth(ordinal)
            .map_or_else(
                || TextRange::empty(TextSize::new(0)),
                |function| function.name_range,
            );
    }
}

pub(super) fn failed_source(
    file: &DiscoveredFile,
    path: Utf8PathBuf,
    original_bytes: Vec<u8>,
    original: String,
    failure: String,
) -> FileWork {
    failed_source_with_report(
        file,
        path,
        original_bytes,
        original.clone(),
        original,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        failure,
    )
}

#[allow(clippy::too_many_arguments)]
fn failed_source_with_report(
    file: &DiscoveredFile,
    path: Utf8PathBuf,
    _original_bytes: Vec<u8>,
    original: String,
    proposed: String,
    fix_records: Vec<FixRecord>,
    before: Vec<Diagnostic>,
    after: Vec<Diagnostic>,
    failure: String,
) -> FileWork {
    let changed = original != proposed;
    FileWork {
        absolute_path: file.path.clone(),
        report: FileReport {
            budget: Vec::new(),
            path,
            changed,
            written: false,
            backup: None,
            failure: Some(failure),
            fixes: fix_records,
            before,
            after,
            original: Some(Arc::from(original)),
            fixed: Some(Arc::from(proposed)),
        },
        // Operational validation failures never authorize a partial write.
        plan: None,
        read_preconditions: Vec::new(),
    }
}

/// The same numbers the budget sentence carries, as fields.
///
/// A caller reading JSON should not have to take apart "lines 4/25 (21 left)"
/// to learn that a function has 21 lines of room. The sentence stays for a
/// person; this is the same run's answer for everything else.
fn function_budgets(path: &Utf8PathBuf, source: &str) -> Vec<normfix_report::FunctionBudget> {
    analyze_budget(path.as_path(), source).map_or_else(
        |_| Vec::new(),
        |budgets| {
            budgets
                .into_iter()
                .map(|budget| normfix_report::FunctionBudget {
                    function: budget.function,
                    line: budget.line,
                    lines: budget.lines,
                    line_limit: budget.line_limit,
                    variables: budget.variables,
                    variable_limit: budget.variable_limit,
                    parameters: budget.parameters,
                    parameter_limit: budget.parameter_limit,
                })
                .collect()
        },
    )
}

fn budget_diagnostics(path: &Utf8PathBuf, source: &str) -> Vec<Diagnostic> {
    analyze_budget(path.as_path(), source).map_or_else(
        |_| Vec::new(),
        |budgets| {
            budgets
                .into_iter()
                .map(|budget| Diagnostic {
                    rule_id: "NORM_BUDGET".to_owned(),
                    path: path.clone(),
                    range: line_point_range(source, budget.line),
                    severity: Severity::Info,
                    message: format!(
                        "{}(): lines {}/{} ({} left), variables {}/{} ({} left), parameters {}/{} ({} left).",
                        budget.function,
                        budget.lines,
                        budget.line_limit,
                        budget.line_limit.saturating_sub(budget.lines),
                        budget.variables,
                        budget.variable_limit,
                        budget.variable_limit.saturating_sub(budget.variables),
                        budget.parameters,
                        budget.parameter_limit,
                        budget.parameter_limit.saturating_sub(budget.parameters),
                    ),
                    source: DiagnosticSource::NativeNorm41,
                    notes: Vec::new(),
                    help: Some(
                        "Keep headroom for defense-day changes; limits already exceeded are also reported as warnings."
                            .to_owned(),
                    ),
                    localized: None,
                })
                .collect()
        },
    )
}

fn apply_guard_approval(source: &str, approval: &GuardApproval) -> Option<String> {
    if !guard_approval_is_current(approval)
        || *blake3::hash(source.as_bytes()).as_bytes() != approval.rename.header_digest
    {
        return None;
    }
    let mut output = source.to_owned();
    for (range, current) in [
        (
            approval.rename.define_range.clone(),
            approval.rename.define_current.as_str(),
        ),
        (
            approval.rename.ifndef_range.clone(),
            approval.rename.current.as_str(),
        ),
    ] {
        if output.get(range.clone())? != current {
            return None;
        }
        output.replace_range(range, &approval.rename.expected);
    }
    Some(output)
}

fn apply_guard_insertion(source: &str, approval: &GuardInsertionApproval) -> Option<String> {
    if !guard_insertion_approval_is_current(approval)
        || *blake3::hash(source.as_bytes()).as_bytes() != approval.insertion.header_digest
    {
        return None;
    }
    let guard = &approval.insertion.expected;
    let mut body_start = normfix_header::c_header_span(source).map_or(0, |range| range.end);
    while source
        .as_bytes()
        .get(body_start)
        .is_some_and(|byte| matches!(byte, b'\r' | b'\n'))
    {
        body_start += 1;
    }
    let body = source.get(body_start..)?;
    let mut output = String::with_capacity(source.len() + guard.len() * 2 + 40);
    output.push_str(source.get(..body_start)?);
    output.push_str("#ifndef ");
    output.push_str(guard);
    output.push_str("\n# define ");
    output.push_str(guard);
    output.push_str("\n\n");
    output.push_str(body);
    if !body.is_empty() && !body.ends_with('\n') {
        output.push('\n');
    }
    if !body.is_empty() && !output.ends_with("\n\n") {
        output.push('\n');
    }
    output.push_str("#endif\n");
    Some(output)
}
