//! The authorized-function policy and its proof.
//!
//! A finding here is only meaningful when the whole project could be read and
//! parsed, so an incomplete proof disables every allowlist finding instead of
//! reporting a partial answer.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use camino::Utf8PathBuf;
use normfix_c_actions::analyze_external_calls;
use normfix_c_syntax::{CFunctionKind, CParser};
use normfix_core::{Diagnostic, DiagnosticSource, Severity};
use normfix_project::{
    DiscoveredFile, DiscoveryOptions, ProjectFileKind, ProjectPolicy, discover, load_project_policy,
};

use super::diagnostics::project_diagnostic;
use super::paths::report_path;
use super::{FileWork, FixOptions, FunctionPolicyPlan, FunctionPolicyProof};

pub(super) fn plan_policy_diagnostics(
    selected: &[DiscoveredFile],
    options: &FixOptions,
) -> FunctionPolicyPlan {
    if !selected.iter().any(|file| {
        matches!(
            file.kind,
            ProjectFileKind::CSource | ProjectFileKind::CHeader
        )
    }) {
        return FunctionPolicyPlan::default();
    }
    let policy = match load_project_policy(&options.cwd) {
        Ok(Some(policy)) => policy,
        Ok(None) => {
            if !options.preflight {
                return FunctionPolicyPlan::default();
            }
            let Some(file) = selected.iter().find(|file| {
                matches!(
                    file.kind,
                    ProjectFileKind::CSource | ProjectFileKind::CHeader
                )
            }) else {
                return FunctionPolicyPlan::default();
            };
            let path = report_path(&file.path, &options.cwd)
                .unwrap_or_else(|_| Utf8PathBuf::from(file.path.to_string_lossy().as_ref()));
            return FunctionPolicyPlan {
                proof: None,
                diagnostics: BTreeMap::from([(
                    file.path.clone(),
                    vec![project_diagnostic(
                        path,
                        "FUNCTION_POLICY_NOT_CONFIGURED",
                        "Authorized-function checking is unavailable because this project has no normfix.toml allowlist.",
                        "Create normfix.toml from the subject's exact authorized-function list before relying on preflight.",
                    )],
                )]),
            };
        }
        Err(error) => {
            let Some(file) = selected.first() else {
                return FunctionPolicyPlan::default();
            };
            let path = report_path(&file.path, &options.cwd)
                .unwrap_or_else(|_| Utf8PathBuf::from(file.path.to_string_lossy().as_ref()));
            return FunctionPolicyPlan {
                proof: None,
                diagnostics: BTreeMap::from([(
                    file.path.clone(),
                    vec![project_diagnostic(
                        path,
                        "PROJECT_POLICY_INVALID",
                        &error.to_string(),
                        "Fix normfix.toml; the allowed-function check was skipped for this run.",
                    )],
                )]),
            };
        }
    };
    match build_function_policy_proof(policy, selected, &options.cwd) {
        Ok(proof) => FunctionPolicyPlan {
            proof: Some(proof),
            diagnostics: BTreeMap::new(),
        },
        Err(reason) => function_policy_incomplete_plan(selected, &options.cwd, &reason),
    }
}

pub(super) fn build_function_policy_proof(
    policy: ProjectPolicy,
    selected: &[DiscoveredFile],
    project_root: &Path,
) -> Result<FunctionPolicyProof, String> {
    let discovery = discover(
        &[],
        &DiscoveryOptions::new(project_root)
            .with_respect_gitignore(false)
            .with_respect_normfixignore(false),
    );
    if let Some(error) = discovery.errors.first() {
        return Err(format!(
            "Complete-project C/header discovery failed: {error}"
        ));
    }
    let project_sources = discovery
        .processable_files
        .iter()
        .filter(|file| {
            matches!(
                file.kind,
                ProjectFileKind::CSource | ProjectFileKind::CHeader
            )
        })
        .collect::<Vec<_>>();
    let discovered_paths = project_sources
        .iter()
        .map(|file| file.path.as_path())
        .collect::<BTreeSet<_>>();
    if let Some(file) = selected.iter().find(|file| {
        matches!(
            file.kind,
            ProjectFileKind::CSource | ProjectFileKind::CHeader
        ) && !discovered_paths.contains(file.path.as_path())
    }) {
        return Err(format!(
            "Selected C/header input `{}` is outside the complete project discovery rooted at `{}`.",
            file.path.display(),
            project_root.display()
        ));
    }
    let mut parser = CParser::new()
        .map_err(|error| format!("The native C parser could not be initialized: {error}"))?;
    let mut external_definitions = BTreeSet::new();
    let mut source_digests = BTreeMap::new();
    for file in project_sources {
        let bytes = std::fs::read(&file.path).map_err(|error| {
            format!(
                "Complete-project source `{}` could not be read: {error}",
                file.path.display()
            )
        })?;
        source_digests.insert(file.path.clone(), *blake3::hash(&bytes).as_bytes());
        let source = String::from_utf8(bytes).map_err(|error| {
            format!(
                "Complete-project source `{}` is not valid UTF-8: {error}",
                file.path.display()
            )
        })?;
        let parsed = parser.parse(&source).map_err(|error| {
            format!(
                "Complete-project source `{}` could not be parsed losslessly: {error}",
                file.path.display()
            )
        })?;
        if !parsed.permits_automatic_edits() || !parsed.tape().is_lossless() {
            return Err(format!(
                "Complete-project source `{}` required ambiguous syntax recovery.",
                file.path.display()
            ));
        }
        external_definitions.extend(
            parsed
                .facts()
                .functions
                .iter()
                .filter(|function| {
                    function.kind == CFunctionKind::Definition && !function.is_static
                })
                .map(|function| function.name.clone()),
        );
    }
    Ok(FunctionPolicyProof {
        policy,
        external_definitions,
        source_digests,
    })
}

pub(super) fn function_policy_incomplete_plan(
    selected: &[DiscoveredFile],
    project_root: &Path,
    reason: &str,
) -> FunctionPolicyPlan {
    let Some(file) = selected.iter().find(|file| {
        matches!(
            file.kind,
            ProjectFileKind::CSource | ProjectFileKind::CHeader
        )
    }) else {
        return FunctionPolicyPlan::default();
    };
    let path = report_path(&file.path, project_root)
        .unwrap_or_else(|_| Utf8PathBuf::from(file.path.to_string_lossy().as_ref()));
    FunctionPolicyPlan {
        proof: None,
        diagnostics: BTreeMap::from([(
            file.path.clone(),
            vec![project_diagnostic(
                path,
                "FUNCTION_POLICY_PROOF_INCOMPLETE",
                &format!(
                    "Authorized-function findings were disabled because the complete-project proof is incomplete: {reason}"
                ),
                "Make every project C/header input readable and losslessly parseable, then retry from the project root.",
            )],
        )]),
    }
}

pub(super) fn append_function_policy_diagnostics(
    work: &mut [FileWork],
    proof: Option<&FunctionPolicyProof>,
    project_root: &Path,
) {
    let Some(proof) = proof else {
        return;
    };
    if let Err(reason) = validate_function_policy_snapshot(proof, project_root) {
        append_function_policy_incomplete_diagnostic(work, project_root, &reason);
        return;
    }
    let mut findings = Vec::<(usize, Diagnostic)>::new();
    let mut incomplete = None;
    for (index, item) in work.iter().enumerate().filter(|(_, item)| {
        ProjectFileKind::from_path(&item.absolute_path) == Some(ProjectFileKind::CSource)
    }) {
        let Some(source) = item.report.fixed.as_deref() else {
            incomplete = Some(format!(
                "Final source bytes were unavailable for `{}`.",
                item.absolute_path.display()
            ));
            break;
        };
        let candidates = match analyze_external_calls(item.report.path.as_path(), source) {
            Ok(candidates) => candidates,
            Err(error) => {
                incomplete = Some(format!(
                    "Final source `{}` could not be parsed losslessly: {error}",
                    item.absolute_path.display()
                ));
                break;
            }
        };
        let policy_label = proof.policy.name.as_deref().map_or_else(
            || "this project".to_owned(),
            |name| format!("project `{name}`"),
        );
        for candidate in candidates {
            if proof.external_definitions.contains(&candidate.name)
                || proof.policy.allowed_functions.contains(&candidate.name)
            {
                continue;
            }
            findings.push((
                index,
                Diagnostic {
                    rule_id: "FUNCTION_NOT_ALLOWED".to_owned(),
                    path: candidate.path,
                    range: candidate.name_range,
                    severity: Severity::Warning,
                    message: format!(
                        "External call `{}` is not listed as allowed for {policy_label}.",
                        candidate.name
                    ),
                    source: DiagnosticSource::Project,
                    notes: vec![format!(
                        "Policy source: {}. Same-file definitions, non-static project definitions, and recoverable function-pointer calls were excluded.",
                        proof.policy.path.display()
                    )],
                    help: Some(
                        "Remove the call or add it to [project].allowed only when the 42 subject explicitly permits it."
                            .to_owned(),
                    ),
                    localized: None,
                },
            ));
        }
    }
    if let Some(reason) = incomplete {
        append_function_policy_incomplete_diagnostic(work, project_root, &reason);
        return;
    }
    for (index, diagnostic) in findings {
        work[index].report.after.push(diagnostic);
    }
    for item in work {
        item.report.after.sort();
        item.report.after.dedup();
    }
}

pub(super) fn validate_function_policy_snapshot(
    proof: &FunctionPolicyProof,
    project_root: &Path,
) -> Result<(), String> {
    match load_project_policy(project_root) {
        Ok(Some(current)) if current == proof.policy => {}
        Ok(Some(_)) => return Err("normfix.toml changed after policy planning.".to_owned()),
        Ok(None) => return Err("normfix.toml disappeared after policy planning.".to_owned()),
        Err(error) => {
            return Err(format!(
                "normfix.toml could not be revalidated after policy planning: {error}"
            ));
        }
    }
    let discovery = discover(
        &[],
        &DiscoveryOptions::new(project_root)
            .with_respect_gitignore(false)
            .with_respect_normfixignore(false),
    );
    if let Some(error) = discovery.errors.first() {
        return Err(format!(
            "Complete-project C/header discovery changed or failed during the run: {error}"
        ));
    }
    let current_paths = discovery
        .processable_files
        .into_iter()
        .filter(|file| {
            matches!(
                file.kind,
                ProjectFileKind::CSource | ProjectFileKind::CHeader
            )
        })
        .map(|file| file.path)
        .collect::<BTreeSet<_>>();
    let planned_paths = proof
        .source_digests
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();
    if current_paths != planned_paths {
        return Err("The complete project C/header file set changed during the run.".to_owned());
    }
    for (path, expected) in &proof.source_digests {
        let bytes = std::fs::read(path).map_err(|error| {
            format!(
                "Complete-project source `{}` could not be re-read: {error}",
                path.display()
            )
        })?;
        if blake3::hash(&bytes).as_bytes() != expected {
            return Err(format!(
                "Complete-project source `{}` changed during the run.",
                path.display()
            ));
        }
    }
    Ok(())
}

pub(super) fn append_function_policy_incomplete_diagnostic(
    work: &mut [FileWork],
    project_root: &Path,
    reason: &str,
) {
    let Some(item) = work.iter_mut().find(|item| {
        matches!(
            ProjectFileKind::from_path(&item.absolute_path),
            Some(ProjectFileKind::CSource | ProjectFileKind::CHeader)
        )
    }) else {
        return;
    };
    let path = report_path(&item.absolute_path, project_root)
        .unwrap_or_else(|_| Utf8PathBuf::from(item.absolute_path.to_string_lossy().as_ref()));
    item.report.after.push(project_diagnostic(
        path,
        "FUNCTION_POLICY_PROOF_INCOMPLETE",
        &format!(
            "Authorized-function findings were disabled because the final-source proof is incomplete: {reason}"
        ),
        "Make every selected C source losslessly parseable and retry; no allowlist finding from this run is authoritative.",
    ));
    item.report.after.sort();
    item.report.after.dedup();
}
