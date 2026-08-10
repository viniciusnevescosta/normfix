//! The human-message catalogue.
//!
//! Completeness is a compile error, not a test: every locale is one struct
//! literal, so adding a field without translating it does not build. A
//! partially translated locale is worse than an English one, because the reader
//! cannot tell which half they are looking at.
//!
//! Entries hold explanations only. Command names, flag spellings, rule IDs,
//! and product names appear inside a sentence but are never translated.

use crate::Locale;

/// Every human string the native command line can print.
///
/// `{name}` placeholders are substituted with [`crate::fill`]. A translation
/// must carry the same placeholder set as the English entry.
#[allow(clippy::struct_field_names)]
#[derive(Clone, Copy, Debug)]
pub struct Messages {
    /// Header printed above the start-of-run configuration block.
    pub starting_banner: &'static str,
    /// Label: selected workflow.
    pub label_action: &'static str,
    /// Label: whether the run writes, checks, or previews.
    pub label_mode: &'static str,
    /// Label: effective scope.
    pub label_scope: &'static str,
    /// Label: working directory.
    pub label_working_directory: &'static str,
    /// Label: identity used for official headers.
    pub label_identity: &'static str,
    /// Label: worker configuration.
    pub label_workers: &'static str,
    /// Label: which checkers run.
    pub label_checks: &'static str,
    /// Label: Norminette executable selection.
    pub label_norminette: &'static str,
    /// Label: untested-release policy.
    pub label_version_rule: &'static str,
    /// Label: per-file timeout.
    pub label_timeout: &'static str,
    /// Label: analysis cache.
    pub label_cache: &'static str,
    /// Label: ignore-file handling.
    pub label_gitignore: &'static str,
    /// Label: backup policy.
    pub label_backups: &'static str,
    /// Label: enabled destructive capabilities.
    pub label_destructive: &'static str,
    /// Label: whether `--force` was supplied.
    pub label_force: &'static str,
    /// Label: advisory produced while preparing the run.
    pub label_advisory: &'static str,

    /// Mode: writes accepted fixes in place.
    pub mode_write: &'static str,
    /// Mode: analyses without writing.
    pub mode_check: &'static str,
    /// Mode: prints a diff without writing.
    pub mode_diff: &'static str,

    /// Checks: official checker plus the strict compiler pass.
    pub checks_norminette_and_compiler: &'static str,
    /// Checks: official checker only.
    pub checks_norminette: &'static str,

    /// A capability or feature that is on.
    pub state_enabled: &'static str,
    /// A capability or feature that is off.
    pub state_disabled: &'static str,
    /// Ignore files are honored during discovery.
    pub gitignore_respected: &'static str,
    /// Ignore files are not consulted.
    pub gitignore_not_applied: &'static str,
    /// `--force` was supplied.
    pub force_acknowledged: &'static str,
    /// `--force` was not supplied.
    pub force_absent: &'static str,
    /// Worker count follows the hardware default.
    pub workers_automatic: &'static str,
    /// The checker is located on `PATH`.
    pub norminette_path_discovery: &'static str,
    /// Only the tested checker release is accepted.
    pub version_policy_strict: &'static str,
    /// Another parseable release continues with a warning.
    pub version_policy_advisory: &'static str,
    /// Backups go to the managed external location.
    pub backups_automatic: &'static str,
    /// Backups go to a caller-selected directory. Placeholder: `{path}`.
    pub backups_directory: &'static str,
    /// Ordinary writes are not backed up.
    pub backups_disabled: &'static str,
    /// No verified identity is available for official headers.
    pub identity_unavailable: &'static str,
    /// Per-file checker timeout. Placeholder: `{seconds}`.
    pub timeout_per_file: &'static str,
    /// Whole-directory scope. Placeholder: `{directory}`.
    pub scope_recursive: &'static str,
    /// Git-derived scope. Placeholders: `{kind}`, `{directory}`, `{count}`.
    pub scope_git: &'static str,
    /// Git scope restricted to the index.
    pub scope_git_staged: &'static str,
    /// Git scope covering the working tree.
    pub scope_git_changed: &'static str,
    /// Truncation marker in an explicit path list. Placeholder: `{count}`.
    pub scope_more_paths: &'static str,

    /// No destructive capability is enabled.
    pub destructive_none: &'static str,
    /// Removal of comments rejected at exact official locations.
    pub destructive_invalid_comments: &'static str,
    /// Compaction of simple standard NULL comparisons.
    pub destructive_null_checks: &'static str,
    /// Removal of proven-missing or trivia-only Makefile source tokens.
    pub destructive_makefile_entries: &'static str,
    /// Removal of header prototypes with no implementation and no use.
    pub destructive_orphan_prototypes: &'static str,
    /// Removal of `static` functions proven unreachable.
    pub destructive_unused_statics: &'static str,
    /// Quarantine of unexpected project files.
    pub destructive_quarantine: &'static str,
}

/// Returns the catalogue for `locale`.
#[must_use]
pub const fn messages(locale: Locale) -> &'static Messages {
    match locale {
        Locale::English => &ENGLISH,
        Locale::Portuguese => &PORTUGUESE,
        Locale::Spanish => &SPANISH,
        Locale::French => &FRENCH,
    }
}

const ENGLISH: Messages = Messages {
    starting_banner: "normfix · starting",
    label_action: "action",
    label_mode: "mode",
    label_scope: "scope",
    label_working_directory: "working dir",
    label_identity: "identity",
    label_workers: "workers",
    label_checks: "checks",
    label_norminette: "norminette",
    label_version_rule: "version rule",
    label_timeout: "timeout",
    label_cache: "cache",
    label_gitignore: "gitignore",
    label_backups: "backups",
    label_destructive: "destructive",
    label_force: "force",
    label_advisory: "advisory",

    mode_write: "write",
    mode_check: "read-only check",
    mode_diff: "read-only diff",

    checks_norminette_and_compiler: "Norminette + strict compiler",
    checks_norminette: "Norminette",

    state_enabled: "enabled",
    state_disabled: "disabled",
    gitignore_respected: "respected",
    gitignore_not_applied: "not applied",
    force_acknowledged: "acknowledged",
    force_absent: "no",
    workers_automatic: "auto",
    norminette_path_discovery: "automatic PATH discovery",
    version_policy_strict: "strict (tested release required)",
    version_policy_advisory: "advisory (other releases continue)",
    backups_automatic: "automatic external backup",
    backups_directory: "external directory {path}",
    backups_disabled: "disabled for ordinary writes",
    identity_unavailable: "unavailable (headers will be reported)",
    timeout_per_file: "{seconds}s per file",
    scope_recursive: "{directory} (recursive)",
    scope_git: "Git {kind} in {directory} ({count} selected file(s))",
    scope_git_staged: "staged",
    scope_git_changed: "changed",
    scope_more_paths: "+{count} more",

    destructive_none: "none",
    destructive_invalid_comments: "invalid comments",
    destructive_null_checks: "NULL-check compaction",
    destructive_makefile_entries: "missing or trivia-only Makefile entries",
    destructive_orphan_prototypes: "orphan header prototypes",
    destructive_unused_statics: "unreachable static functions",
    destructive_quarantine: "unexpected-file quarantine",
};

const PORTUGUESE: Messages = Messages {
    starting_banner: "normfix · iniciando",
    label_action: "ação",
    label_mode: "modo",
    label_scope: "escopo",
    label_working_directory: "diretório",
    label_identity: "identidade",
    label_workers: "processos",
    label_checks: "verificações",
    label_norminette: "norminette",
    label_version_rule: "regra de versão",
    label_timeout: "tempo limite",
    label_cache: "cache",
    label_gitignore: "gitignore",
    label_backups: "backups",
    label_destructive: "destrutivo",
    label_force: "force",
    label_advisory: "aviso",

    mode_write: "grava as correções",
    mode_check: "somente leitura",
    mode_diff: "diff, somente leitura",

    checks_norminette_and_compiler: "Norminette + compilador estrito",
    checks_norminette: "Norminette",

    state_enabled: "ativado",
    state_disabled: "desativado",
    gitignore_respected: "respeitado",
    gitignore_not_applied: "não aplicado",
    force_acknowledged: "reconhecido",
    force_absent: "não",
    workers_automatic: "automático",
    norminette_path_discovery: "descoberta automática no PATH",
    version_policy_strict: "estrita (exige a versão testada)",
    version_policy_advisory: "informativa (outras versões continuam)",
    backups_automatic: "backup externo automático",
    backups_directory: "diretório externo {path}",
    backups_disabled: "desativado para gravações comuns",
    identity_unavailable: "indisponível (os cabeçalhos serão apenas reportados)",
    timeout_per_file: "{seconds}s por arquivo",
    scope_recursive: "{directory} (recursivo)",
    scope_git: "Git {kind} em {directory} ({count} arquivo(s) selecionado(s))",
    scope_git_staged: "no índice",
    scope_git_changed: "modificados",
    scope_more_paths: "+{count} outros",

    destructive_none: "nenhum",
    destructive_invalid_comments: "comentários inválidos",
    destructive_null_checks: "compactação de comparações com NULL",
    destructive_makefile_entries: "entradas do Makefile ausentes ou só com trivialidades",
    destructive_orphan_prototypes: "protótipos órfãos de cabeçalho",
    destructive_unused_statics: "funções static inalcançáveis",
    destructive_quarantine: "quarentena de arquivos inesperados",
};

const SPANISH: Messages = Messages {
    starting_banner: "normfix · iniciando",
    label_action: "acción",
    label_mode: "modo",
    label_scope: "alcance",
    label_working_directory: "directorio",
    label_identity: "identidad",
    label_workers: "procesos",
    label_checks: "comprobaciones",
    label_norminette: "norminette",
    label_version_rule: "regla de versión",
    label_timeout: "tiempo límite",
    label_cache: "caché",
    label_gitignore: "gitignore",
    label_backups: "copias",
    label_destructive: "destructivo",
    label_force: "force",
    label_advisory: "aviso",

    mode_write: "escribe las correcciones",
    mode_check: "solo lectura",
    mode_diff: "diff, solo lectura",

    checks_norminette_and_compiler: "Norminette + compilador estricto",
    checks_norminette: "Norminette",

    state_enabled: "activado",
    state_disabled: "desactivado",
    gitignore_respected: "respetado",
    gitignore_not_applied: "no aplicado",
    force_acknowledged: "reconocido",
    force_absent: "no",
    workers_automatic: "automático",
    norminette_path_discovery: "descubrimiento automático en PATH",
    version_policy_strict: "estricta (exige la versión probada)",
    version_policy_advisory: "informativa (otras versiones continúan)",
    backups_automatic: "copia de seguridad externa automática",
    backups_directory: "directorio externo {path}",
    backups_disabled: "desactivadas para escrituras normales",
    identity_unavailable: "no disponible (las cabeceras solo se informarán)",
    timeout_per_file: "{seconds}s por archivo",
    scope_recursive: "{directory} (recursivo)",
    scope_git: "Git {kind} en {directory} ({count} archivo(s) seleccionado(s))",
    scope_git_staged: "en el índice",
    scope_git_changed: "modificados",
    scope_more_paths: "+{count} más",

    destructive_none: "ninguno",
    destructive_invalid_comments: "comentarios inválidos",
    destructive_null_checks: "compactación de comparaciones con NULL",
    destructive_makefile_entries: "entradas del Makefile ausentes o solo con trivialidades",
    destructive_orphan_prototypes: "prototipos huérfanos de cabecera",
    destructive_unused_statics: "funciones static inalcanzables",
    destructive_quarantine: "cuarentena de archivos inesperados",
};

const FRENCH: Messages = Messages {
    starting_banner: "normfix · démarrage",
    label_action: "action",
    label_mode: "mode",
    label_scope: "portée",
    label_working_directory: "répertoire",
    label_identity: "identité",
    label_workers: "processus",
    label_checks: "vérifications",
    label_norminette: "norminette",
    label_version_rule: "règle de version",
    label_timeout: "délai",
    label_cache: "cache",
    label_gitignore: "gitignore",
    label_backups: "sauvegardes",
    label_destructive: "destructif",
    label_force: "force",
    label_advisory: "avis",

    mode_write: "écrit les corrections",
    mode_check: "lecture seule",
    mode_diff: "diff, lecture seule",

    checks_norminette_and_compiler: "Norminette + compilateur strict",
    checks_norminette: "Norminette",

    state_enabled: "activé",
    state_disabled: "désactivé",
    gitignore_respected: "respecté",
    gitignore_not_applied: "non appliqué",
    force_acknowledged: "reconnu",
    force_absent: "non",
    workers_automatic: "automatique",
    norminette_path_discovery: "découverte automatique dans le PATH",
    version_policy_strict: "stricte (version testée exigée)",
    version_policy_advisory: "indicative (les autres versions continuent)",
    backups_automatic: "sauvegarde externe automatique",
    backups_directory: "répertoire externe {path}",
    backups_disabled: "désactivées pour les écritures ordinaires",
    identity_unavailable: "indisponible (les en-têtes seront seulement signalés)",
    timeout_per_file: "{seconds}s par fichier",
    scope_recursive: "{directory} (récursif)",
    scope_git: "Git {kind} dans {directory} ({count} fichier(s) sélectionné(s))",
    scope_git_staged: "indexés",
    scope_git_changed: "modifiés",
    scope_more_paths: "+{count} de plus",

    destructive_none: "aucun",
    destructive_invalid_comments: "commentaires invalides",
    destructive_null_checks: "compactage des comparaisons avec NULL",
    destructive_makefile_entries: "entrées du Makefile absentes ou sans code",
    destructive_orphan_prototypes: "prototypes orphelins d'en-tête",
    destructive_unused_statics: "fonctions static inatteignables",
    destructive_quarantine: "mise en quarantaine des fichiers inattendus",
};

#[cfg(test)]
mod tests {
    use crate::{Locale, PUBLISHED, messages};

    fn entries(messages: &'static super::Messages) -> Vec<(&'static str, &'static str)> {
        vec![
            ("starting_banner", messages.starting_banner),
            ("label_action", messages.label_action),
            ("label_mode", messages.label_mode),
            ("label_scope", messages.label_scope),
            ("label_working_directory", messages.label_working_directory),
            ("label_identity", messages.label_identity),
            ("label_workers", messages.label_workers),
            ("label_checks", messages.label_checks),
            ("label_norminette", messages.label_norminette),
            ("label_version_rule", messages.label_version_rule),
            ("label_timeout", messages.label_timeout),
            ("label_cache", messages.label_cache),
            ("label_gitignore", messages.label_gitignore),
            ("label_backups", messages.label_backups),
            ("label_destructive", messages.label_destructive),
            ("label_force", messages.label_force),
            ("label_advisory", messages.label_advisory),
            ("mode_write", messages.mode_write),
            ("mode_check", messages.mode_check),
            ("mode_diff", messages.mode_diff),
            (
                "checks_norminette_and_compiler",
                messages.checks_norminette_and_compiler,
            ),
            ("checks_norminette", messages.checks_norminette),
            ("state_enabled", messages.state_enabled),
            ("state_disabled", messages.state_disabled),
            ("gitignore_respected", messages.gitignore_respected),
            ("gitignore_not_applied", messages.gitignore_not_applied),
            ("force_acknowledged", messages.force_acknowledged),
            ("force_absent", messages.force_absent),
            ("workers_automatic", messages.workers_automatic),
            (
                "norminette_path_discovery",
                messages.norminette_path_discovery,
            ),
            ("version_policy_strict", messages.version_policy_strict),
            ("version_policy_advisory", messages.version_policy_advisory),
            ("backups_automatic", messages.backups_automatic),
            ("backups_directory", messages.backups_directory),
            ("backups_disabled", messages.backups_disabled),
            ("identity_unavailable", messages.identity_unavailable),
            ("timeout_per_file", messages.timeout_per_file),
            ("scope_recursive", messages.scope_recursive),
            ("scope_git", messages.scope_git),
            ("scope_git_staged", messages.scope_git_staged),
            ("scope_git_changed", messages.scope_git_changed),
            ("scope_more_paths", messages.scope_more_paths),
            ("destructive_none", messages.destructive_none),
            (
                "destructive_invalid_comments",
                messages.destructive_invalid_comments,
            ),
            ("destructive_null_checks", messages.destructive_null_checks),
            (
                "destructive_makefile_entries",
                messages.destructive_makefile_entries,
            ),
            (
                "destructive_orphan_prototypes",
                messages.destructive_orphan_prototypes,
            ),
            (
                "destructive_unused_statics",
                messages.destructive_unused_statics,
            ),
            ("destructive_quarantine", messages.destructive_quarantine),
        ]
    }

    fn placeholders(template: &str) -> Vec<String> {
        let mut names = Vec::new();
        let mut rest = template;
        while let Some(start) = rest.find('{') {
            let Some(length) = rest[start..].find('}') else {
                break;
            };
            let end = start + length;
            names.push(rest[start + 1..end].to_owned());
            rest = &rest[end + 1..];
        }
        names.sort();
        names
    }

    #[test]
    fn no_locale_leaves_an_entry_empty() {
        for locale in PUBLISHED {
            for (key, value) in entries(messages(*locale)) {
                assert!(
                    !value.trim().is_empty(),
                    "{}: `{key}` is empty",
                    locale.code()
                );
            }
        }
    }

    #[test]
    fn every_translation_carries_the_english_placeholder_set() {
        let english = entries(messages(Locale::English));
        for locale in PUBLISHED {
            for ((key, source), (_, translated)) in english.iter().zip(entries(messages(*locale))) {
                assert_eq!(
                    placeholders(source),
                    placeholders(translated),
                    "{}: `{key}` changed its placeholders",
                    locale.code()
                );
            }
        }
    }

    #[test]
    fn a_translation_is_actually_translated() {
        // Product names and a few identifiers legitimately match English.
        let shared = [
            "label_norminette",
            "label_force",
            "label_mode",
            "label_action",
            "label_cache",
            "label_gitignore",
            "label_backups",
            "checks_norminette",
        ];
        for locale in PUBLISHED.iter().filter(|l| **l != Locale::English) {
            let english = entries(messages(Locale::English));
            let translated = entries(messages(*locale));
            let identical = english
                .iter()
                .zip(&translated)
                .filter(|((key, source), (_, target))| source == target && !shared.contains(key))
                .count();
            assert!(
                identical <= 6,
                "{}: {identical} entries are still English",
                locale.code()
            );
        }
    }
}
