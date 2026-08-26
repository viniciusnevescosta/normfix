//! Closed-world planning for explicitly destructive C edits.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use camino::Utf8PathBuf;
use normfix_actions::ReadPrecondition;
use normfix_core::{Diagnostic, FileId, FixRecord, SourceSnapshot, apply_source_edits};
use normfix_destructive::{
    ClosedCSourceSet, OrphanPrototypePlan, StaticRemovalPlan, plan_orphan_prototypes,
    plan_unused_static_functions,
};
use normfix_project::{
    DiscoveredFile, DiscoveryOptions, ProjectFileKind, ProjectSnapshot, discover,
};

use super::FixOptions;
use super::diagnostics::project_diagnostic;
use super::paths::{absolute_lexical, report_path};
use super::source_io::read_project_file;

#[derive(Clone, Debug)]
pub(super) struct DestructivePrelude {
    pub(super) source: Option<String>,
    pub(super) original_blake3: Option<[u8; 32]>,
    pub(super) fixes: Vec<FixRecord>,
    pub(super) diagnostics: Vec<Diagnostic>,
    pub(super) read_preconditions: Vec<ReadPrecondition>,
}

impl DestructivePrelude {
    pub(super) fn confirmed_source(&self, original: &[u8]) -> Option<&str> {
        if self.original_blake3 == Some(*blake3::hash(original).as_bytes()) {
            self.source.as_deref()
        } else {
            None
        }
    }
}

struct ClosedSourcePreparationError {
    path: PathBuf,
    message: String,
    help: &'static str,
}

// This is the single audit boundary that sequences complete discovery, scoped
// authorization, closed snapshots, and both destructive C planners.
#[allow(clippy::too_many_lines)]
pub(super) fn plan_destructive_preludes(
    _inputs: &[PathBuf],
    selected: &[DiscoveredFile],
    options: &FixOptions,
) -> BTreeMap<PathBuf, DestructivePrelude> {
    let selected_c = selected
        .iter()
        .filter(|file| {
            matches!(
                file.kind,
                ProjectFileKind::CSource | ProjectFileKind::CHeader
            )
        })
        .map(|file| file.path.clone())
        .collect::<BTreeSet<_>>();
    if selected_c.is_empty() {
        return BTreeMap::new();
    }
    let complete = discover(
        &[],
        &DiscoveryOptions::new(&options.cwd)
            .with_respect_gitignore(false)
            .with_respect_normfixignore(false),
    );
    let complete_c = complete
        .processable_files
        .iter()
        .filter(|file| {
            matches!(
                file.kind,
                ProjectFileKind::CSource | ProjectFileKind::CHeader
            )
        })
        .map(|file| file.path.clone())
        .collect::<BTreeSet<_>>();
    let mut preludes = BTreeMap::new();
    let destructive_requested = options.remove_unused_static || options.remove_orphan_prototypes;
    if !complete.errors.is_empty() {
        if !destructive_requested {
            return preludes;
        }
        attach_destructive_diagnostic(
            &mut preludes,
            selected_c.iter().next().expect("non-empty selected set"),
            &options.cwd,
            "PROJECT_CLOSED_SET_INCOMPLETE",
            "Project-wide implementation analysis was skipped because complete C/header discovery failed.",
            "Resolve every discovery error and rerun from the project root.",
        );
        return preludes;
    }
    let closed = match prepare_closed_source_set(&complete_c, &options.cwd) {
        Ok(closed) => closed,
        Err(error) => {
            if !destructive_requested {
                return preludes;
            }
            attach_destructive_diagnostic(
                &mut preludes,
                &error.path,
                &options.cwd,
                "PROJECT_CLOSED_SET_INVALID",
                &error.message,
                error.help,
            );
            return preludes;
        }
    };
    let mut read_preconditions = vec![ReadPrecondition::project_sources(
        absolute_lexical(&options.cwd),
        complete_c.iter().cloned(),
    )];
    read_preconditions.extend(closed.snapshots().iter().map(|snapshot| {
        ReadPrecondition::Matches {
            path: options.cwd.join(snapshot.relative_path().as_std_path()),
            blake3: *snapshot.content_hash().as_bytes(),
        }
    }));

    if options.remove_unused_static {
        if selected_c != complete_c {
            attach_destructive_diagnostic(
                &mut preludes,
                selected_c.iter().next().expect("non-empty selected set"),
                &options.cwd,
                "UNSAFE_STATIC_CLOSED_SET_INCOMPLETE",
                "Unused static functions were not removed because the selected inputs are not the complete project C/header set.",
                "Run from the project root without partial paths or ignored C files.",
            );
        } else if let Some(authorization) = options.destructive_authorization.as_ref() {
            match plan_unused_static_functions(&closed, authorization) {
                Ok(plan) => apply_static_removal_plan(
                    &mut preludes,
                    plan,
                    &options.cwd,
                    &read_preconditions,
                ),
                Err(error) => attach_destructive_diagnostic(
                    &mut preludes,
                    selected_c.iter().next().expect("non-empty selected set"),
                    &options.cwd,
                    "UNSAFE_STATIC_PLAN_FAILED",
                    &format!("Unused static function planning failed: {error}"),
                    "Review the closed project inputs and destructive authorization.",
                ),
            }
        } else {
            attach_destructive_diagnostic(
                &mut preludes,
                selected_c.iter().next().expect("non-empty selected set"),
                &options.cwd,
                "UNSAFE_AUTHORIZATION_REQUIRED",
                "Unused static functions were not removed because destructive authorization is missing.",
                "Confirm the terminal warning or pass --unsafe --force.",
            );
        }
    }

    let can_remove = options.remove_orphan_prototypes
        && selected_c == complete_c
        && options.destructive_authorization.is_some();
    if options.remove_orphan_prototypes && selected_c != complete_c {
        attach_destructive_diagnostic(
            &mut preludes,
            selected_c.iter().next().expect("non-empty selected set"),
            &options.cwd,
            "UNSAFE_ORPHAN_PROTOTYPE_CLOSED_SET_INCOMPLETE",
            "Orphan header prototypes were not removed because the selected inputs are not the complete project C/header set.",
            "Run from the project root so every declaration and use participates in the proof.",
        );
    } else if options.remove_orphan_prototypes && options.destructive_authorization.is_none() {
        attach_destructive_diagnostic(
            &mut preludes,
            selected_c.iter().next().expect("non-empty selected set"),
            &options.cwd,
            "UNSAFE_AUTHORIZATION_REQUIRED",
            "Orphan header prototypes were not removed because destructive authorization is missing.",
            "Confirm the terminal warning or pass --unsafe --force.",
        );
    }
    let transformed =
        prepare_closed_source_set_with_overrides(&complete_c, &options.cwd, &preludes);
    match transformed {
        Ok(transformed) => {
            let authorization = can_remove
                .then_some(options.destructive_authorization.as_ref())
                .flatten();
            match plan_orphan_prototypes(&transformed, authorization) {
                Ok(plan) => apply_orphan_prototype_plan(
                    &mut preludes,
                    plan,
                    &options.cwd,
                    &selected_c,
                    can_remove,
                    &read_preconditions,
                ),
                Err(error) => attach_destructive_diagnostic(
                    &mut preludes,
                    selected_c.iter().next().expect("non-empty selected set"),
                    &options.cwd,
                    "ORPHAN_PROTOTYPE_PLAN_FAILED",
                    &format!("Prototype implementation analysis failed: {error}"),
                    "Review the complete project inputs and destructive authorization.",
                ),
            }
        }
        Err(error) => attach_destructive_diagnostic(
            &mut preludes,
            &error.path,
            &options.cwd,
            "ORPHAN_PROTOTYPE_CLOSED_SET_INVALID",
            &error.message,
            error.help,
        ),
    }
    preludes
}

fn prepare_closed_source_set(
    selected: &BTreeSet<PathBuf>,
    cwd: &Path,
) -> Result<ClosedCSourceSet, ClosedSourcePreparationError> {
    let mut snapshots = Vec::with_capacity(selected.len());
    for (index, absolute) in selected.iter().enumerate() {
        let bytes = read_project_file(absolute).map_err(|error| ClosedSourcePreparationError {
            path: absolute.clone(),
            message: format!("A C source could not be read for the closed-world proof: {error}"),
            help: "Make every selected C/header file readable and retry.",
        })?;
        let source = String::from_utf8(bytes).map_err(|error| ClosedSourcePreparationError {
            path: absolute.clone(),
            message: format!("A C source is not valid UTF-8: {error}"),
            help: "Convert the source to UTF-8 before destructive analysis.",
        })?;
        let relative =
            absolute
                .strip_prefix(cwd)
                .map_err(|error| ClosedSourcePreparationError {
                    path: absolute.clone(),
                    message: format!("A selected C source is outside the project root: {error}"),
                    help: "Run destructive analysis from one complete project root.",
                })?;
        let relative = Utf8PathBuf::from_path_buf(relative.to_path_buf()).map_err(|path| {
            ClosedSourcePreparationError {
                path: absolute.clone(),
                message: format!("A selected C path is not valid UTF-8: {}", path.display()),
                help: "Rename the path before destructive analysis.",
            }
        })?;
        let file_id = u32::try_from(index).map_err(|error| ClosedSourcePreparationError {
            path: absolute.clone(),
            message: format!("The closed source set is too large to index safely: {error}"),
            help: "Reduce the project to fewer than 2^32 C/header files.",
        })?;
        let snapshot =
            SourceSnapshot::new(FileId::new(file_id), relative, Arc::<str>::from(source)).map_err(
                |error| ClosedSourcePreparationError {
                    path: absolute.clone(),
                    message: format!("A C source snapshot was rejected: {error}"),
                    help: "Use project-relative UTF-8 paths and sources smaller than 4 GiB.",
                },
            )?;
        snapshots.push(snapshot);
    }
    ClosedCSourceSet::from_complete_discovery(snapshots).map_err(|error| {
        ClosedSourcePreparationError {
            path: selected
                .iter()
                .next()
                .cloned()
                .unwrap_or_else(|| cwd.to_path_buf()),
            message: format!("The closed C source set was rejected: {error}"),
            help: "Run from one complete project root with unique C/header paths.",
        }
    })
}

fn prepare_closed_source_set_with_overrides(
    selected: &BTreeSet<PathBuf>,
    cwd: &Path,
    overrides: &BTreeMap<PathBuf, DestructivePrelude>,
) -> Result<ClosedCSourceSet, ClosedSourcePreparationError> {
    let mut snapshots = Vec::with_capacity(selected.len());
    for (index, absolute) in selected.iter().enumerate() {
        let source = if let Some(source) = overrides
            .get(absolute)
            .and_then(|prelude| prelude.source.clone())
        {
            source
        } else {
            let bytes =
                read_project_file(absolute).map_err(|error| ClosedSourcePreparationError {
                    path: absolute.clone(),
                    message: format!(
                        "A C source could not be read for the closed-world proof: {error}"
                    ),
                    help: "Make every selected C/header file readable and retry.",
                })?;
            String::from_utf8(bytes).map_err(|error| ClosedSourcePreparationError {
                path: absolute.clone(),
                message: format!("A C source is not valid UTF-8: {error}"),
                help: "Convert the source to UTF-8 before project-wide analysis.",
            })?
        };
        let relative =
            absolute
                .strip_prefix(cwd)
                .map_err(|error| ClosedSourcePreparationError {
                    path: absolute.clone(),
                    message: format!("A selected C source is outside the project root: {error}"),
                    help: "Run project-wide analysis from one complete project root.",
                })?;
        let relative = Utf8PathBuf::from_path_buf(relative.to_path_buf()).map_err(|path| {
            ClosedSourcePreparationError {
                path: absolute.clone(),
                message: format!("A selected C path is not valid UTF-8: {}", path.display()),
                help: "Rename the path before project-wide analysis.",
            }
        })?;
        let file_id = u32::try_from(index).map_err(|error| ClosedSourcePreparationError {
            path: absolute.clone(),
            message: format!("The closed source set is too large to index safely: {error}"),
            help: "Reduce the project to fewer than 2^32 C/header files.",
        })?;
        snapshots.push(
            SourceSnapshot::new(FileId::new(file_id), relative, Arc::<str>::from(source)).map_err(
                |error| ClosedSourcePreparationError {
                    path: absolute.clone(),
                    message: format!("A C source snapshot was rejected: {error}"),
                    help: "Use project-relative UTF-8 paths and sources smaller than 4 GiB.",
                },
            )?,
        );
    }
    ClosedCSourceSet::from_complete_discovery(snapshots).map_err(|error| {
        ClosedSourcePreparationError {
            path: selected
                .iter()
                .next()
                .cloned()
                .unwrap_or_else(|| cwd.to_path_buf()),
            message: format!("The closed C source set was rejected: {error}"),
            help: "Run from one complete project root with unique C/header paths.",
        }
    })
}

fn apply_static_removal_plan(
    preludes: &mut BTreeMap<PathBuf, DestructivePrelude>,
    plan: StaticRemovalPlan,
    cwd: &Path,
    read_preconditions: &[ReadPrecondition],
) {
    for diagnostic in plan.diagnostics {
        let absolute = cwd.join(diagnostic.path.as_std_path());
        let prelude = prelude_entry(preludes, &absolute);
        prelude.diagnostics.push(diagnostic);
    }
    for file_plan in plan.files {
        let absolute = cwd.join(file_plan.path.as_std_path());
        let prelude = prelude_entry(preludes, &absolute);
        if let Err(error) = load_destructive_source(prelude, &absolute) {
            prelude.diagnostics.push(project_diagnostic(
                file_plan.path,
                "UNSAFE_STATIC_SOURCE_UNREADABLE",
                &format!("The source could not be read for destructive planning: {error}"),
                "Make the file readable and retry; no function was removed.",
            ));
            continue;
        }
        let source = prelude
            .source
            .as_deref()
            .expect("loaded destructive source");
        if blake3::hash(source.as_bytes()).to_hex().to_string() != file_plan.original_hash {
            prelude.diagnostics.push(project_diagnostic(
                file_plan.path,
                "UNSAFE_STATIC_SNAPSHOT_CHANGED",
                "The source changed after destructive planning; no function was removed.",
                "Retry the command against an unchanged project.",
            ));
            continue;
        }
        match apply_source_edits(source, &file_plan.edits) {
            Ok(source) => {
                prelude.source = Some(source);
                prelude.fixes.extend(file_plan.fixes);
                prelude
                    .read_preconditions
                    .extend_from_slice(read_preconditions);
            }
            Err(error) => prelude.diagnostics.push(project_diagnostic(
                file_plan.path,
                "UNSAFE_STATIC_EDIT_REJECTED",
                &format!("The destructive edit set was rejected: {error}"),
                "Review the candidate function manually; no deletion was applied.",
            )),
        }
    }
}

fn apply_orphan_prototype_plan(
    preludes: &mut BTreeMap<PathBuf, DestructivePrelude>,
    plan: OrphanPrototypePlan,
    cwd: &Path,
    selected: &BTreeSet<PathBuf>,
    apply_removals: bool,
    read_preconditions: &[ReadPrecondition],
) {
    let mut attached_diagnostic = false;
    let proof_incomplete = apply_removals
        && plan
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.rule_id == "ORPHAN_PROTOTYPE_CLOSED_SET_INVALID");
    for diagnostic in plan.diagnostics {
        if !apply_removals && diagnostic.rule_id == "ORPHAN_PROTOTYPE_CLOSED_SET_INVALID" {
            continue;
        }
        let absolute = cwd.join(diagnostic.path.as_std_path());
        if selected.contains(&absolute) {
            prelude_entry(preludes, &absolute)
                .diagnostics
                .push(diagnostic);
            attached_diagnostic = true;
        }
    }
    if proof_incomplete && !attached_diagnostic {
        if let Some(first) = selected.iter().next() {
            attach_destructive_diagnostic(
                preludes,
                first,
                cwd,
                "ORPHAN_PROTOTYPE_CLOSED_SET_INVALID",
                "Prototype implementation analysis was inconclusive because another project C/header file could not be parsed losslessly.",
                "Fix every project syntax/recovery issue and rerun preflight.",
            );
        }
    }
    if !apply_removals {
        return;
    }
    for file_plan in plan.files {
        let absolute = cwd.join(file_plan.path.as_std_path());
        if !selected.contains(&absolute) {
            continue;
        }
        let prelude = prelude_entry(preludes, &absolute);
        if let Err(error) = load_destructive_source(prelude, &absolute) {
            prelude.diagnostics.push(project_diagnostic(
                file_plan.path,
                "UNSAFE_ORPHAN_PROTOTYPE_SOURCE_UNREADABLE",
                &format!("The header could not be read for destructive planning: {error}"),
                "Make the file readable and retry; no prototype was removed.",
            ));
            continue;
        }
        let source = prelude
            .source
            .as_deref()
            .expect("loaded destructive source");
        if blake3::hash(source.as_bytes()).to_hex().to_string() != file_plan.original_hash {
            prelude.diagnostics.push(project_diagnostic(
                file_plan.path,
                "UNSAFE_ORPHAN_PROTOTYPE_SNAPSHOT_CHANGED",
                "The header changed after orphan-prototype planning; no prototype was removed.",
                "Retry the command against an unchanged project.",
            ));
            continue;
        }
        match apply_source_edits(source, &file_plan.edits) {
            Ok(source) => {
                prelude.source = Some(source);
                prelude.fixes.extend(file_plan.fixes);
                prelude
                    .read_preconditions
                    .extend_from_slice(read_preconditions);
            }
            Err(error) => prelude.diagnostics.push(project_diagnostic(
                file_plan.path,
                "UNSAFE_ORPHAN_PROTOTYPE_EDIT_REJECTED",
                &format!("The prototype deletion set was rejected: {error}"),
                "Review the prototype manually; no deletion was applied.",
            )),
        }
    }
}

pub(super) fn prelude_entry<'a>(
    preludes: &'a mut BTreeMap<PathBuf, DestructivePrelude>,
    absolute: &Path,
) -> &'a mut DestructivePrelude {
    preludes
        .entry(absolute.to_path_buf())
        .or_insert_with(|| DestructivePrelude {
            source: None,
            original_blake3: None,
            fixes: Vec::new(),
            diagnostics: Vec::new(),
            read_preconditions: Vec::new(),
        })
}

pub(super) fn load_destructive_source(
    prelude: &mut DestructivePrelude,
    absolute: &Path,
) -> Result<(), String> {
    if prelude.source.is_some() {
        return Ok(());
    }
    let bytes = read_project_file(absolute).map_err(|error| error.to_string())?;
    let source = String::from_utf8(bytes.clone()).map_err(|error| error.to_string())?;
    prelude.original_blake3 = Some(*blake3::hash(&bytes).as_bytes());
    prelude.source = Some(source);
    Ok(())
}

pub(super) fn snapshot_preconditions(snapshot: &ProjectSnapshot) -> Vec<ReadPrecondition> {
    snapshot
        .files
        .iter()
        .map(|(path, digest)| ReadPrecondition::Matches {
            path: path.clone(),
            blake3: *digest,
        })
        .collect()
}

fn attach_destructive_diagnostic(
    preludes: &mut BTreeMap<PathBuf, DestructivePrelude>,
    absolute: &Path,
    cwd: &Path,
    rule_id: &str,
    message: &str,
    help: &str,
) {
    let path = report_path(absolute, cwd)
        .unwrap_or_else(|_| Utf8PathBuf::from(absolute.to_string_lossy().as_ref()));
    prelude_entry(preludes, absolute)
        .diagnostics
        .push(project_diagnostic(path, rule_id, message, help));
}
