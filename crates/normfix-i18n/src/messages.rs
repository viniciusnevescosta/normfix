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

    /// One-line description printed under the tool name.
    pub report_tagline: &'static str,
    /// Lead-in for the English-source reminder.
    pub report_project_reminder_label: &'static str,
    /// Body of the English-source reminder.
    pub report_project_reminder: &'static str,
    /// What in a non-English report is not this project's own words.
    pub report_translation_scope: &'static str,
    /// Heading above the per-file table.
    pub report_files_heading: &'static str,
    /// Heading above rule-grouped diagnostics.
    pub report_grouped_heading: &'static str,
    /// Heading above ungrouped diagnostics.
    pub report_diagnostics_heading: &'static str,
    /// Explanation of which files a 42 project is expected to contain.
    pub report_expected_files: &'static str,
    /// Note that a preview never moved the listed files.
    pub report_preview_kept_files: &'static str,
    /// Lead-in for the counts line.
    pub report_summary_label: &'static str,
    /// Counts line. Placeholders: `{files}`, `{proposed}`, `{written}`,
    /// `{fixes}`, `{remaining}`, `{info}`, `{failed}`, `{unexpected}`,
    /// `{quarantined}`.
    pub report_summary_counts: &'static str,
    /// Elapsed-time line. Placeholder: `{duration}`.
    pub report_completed_in: &'static str,
    /// Lead-in for the pre-defense estimate.
    pub report_estimate_label: &'static str,
    /// Estimate body. Placeholders: `{verdict}`, `{grade}`, `{score}`.
    pub report_estimate_value: &'static str,
    /// The estimate's standing caveat.
    pub report_estimate_caveat: &'static str,
    /// Heading above located hard failures.
    pub report_hard_fail_heading: &'static str,

    /// Refusal for a protected scope. Placeholders: `{scope}`, `{reason}`.
    pub scope_refusal: &'static str,
    /// Reason: the path is a filesystem root.
    pub scope_reason_filesystem_root: &'static str,
    /// Reason: the path is a complete user home directory.
    pub scope_reason_home_directory: &'static str,
    /// Reason: the path is inside an operating-system-managed tree.
    pub scope_reason_system_tree: &'static str,
    /// Reason: the path is a broad system or multi-project directory.
    pub scope_reason_broad_directory: &'static str,
    /// `--force` was supplied with nothing for it to acknowledge.
    pub force_without_target: &'static str,

    /// Standing warning shown before a destructive run is authorized.
    pub destructive_warning: &'static str,
    /// Destructive confirmation question. The `[y/N]` token stays literal.
    pub destructive_prompt: &'static str,
    /// A destructive run outside an interactive terminal.
    pub destructive_needs_confirmation: &'static str,
    /// The person declined the destructive confirmation.
    pub destructive_cancelled: &'static str,
    /// An undo outside an interactive terminal.
    pub undo_needs_confirmation: &'static str,
    /// Undo question. Placeholders: `{count}`, `{run}`.
    pub undo_question: &'static str,
    /// Undo confirmation prompt. The `[y/N]` token stays literal.
    pub undo_prompt: &'static str,
    /// The person declined the undo confirmation.
    pub undo_cancelled: &'static str,
    /// Reassurance printed under a failed run.
    pub error_nothing_written: &'static str,

    /// Warning shown when an uninstall would delete recovery data.
    pub uninstall_recovery_warning: &'static str,
    /// Uninstall confirmation question. The `[y/N]` token stays literal.
    pub uninstall_prompt: &'static str,
    /// An uninstall outside an interactive terminal.
    pub uninstall_needs_confirmation: &'static str,
    /// The person declined the uninstall confirmation.
    pub uninstall_cancelled: &'static str,
    /// Confirmation that the tool removed itself.
    pub uninstall_done: &'static str,

    /// Leak-check confirmation question. The `{program}` and `[y/N]` tokens stay literal.
    pub leaks_prompt: &'static str,
    /// A leak check requested outside an interactive terminal.
    pub leaks_needs_confirmation: &'static str,
    /// The person declined to run their program.
    pub leaks_cancelled: &'static str,
    /// No leak checker is installed. Followed by the platform hint below.
    pub leaks_unavailable: &'static str,
    /// How to install a leak checker, on a platform Valgrind supports directly.
    pub leaks_install_hint: &'static str,
    /// How to install one on macOS, where upstream Valgrind does not build.
    pub leaks_install_hint_macos: &'static str,
    /// How to reach one on Windows, where Valgrind does not exist.
    pub leaks_install_hint_windows: &'static str,
    /// Nothing was lost on the path the program took.
    pub leaks_none: &'static str,
    /// Bytes lost outright. Placeholders: `{definite}`, `{indirect}`.
    pub leaks_lost: &'static str,
    /// Heading above the list of allocation sites.
    pub leaks_sites: &'static str,
    /// A block lost outright, shown against its source line.
    pub leaks_site_direct: &'static str,
    /// A block reachable only through a lost one, shown against its line.
    pub leaks_site_indirect: &'static str,
    /// What to do about a reported allocation.
    pub leaks_site_help: &'static str,
    /// One invalid access, shown against the line that made it.
    pub leaks_error_at: &'static str,
    /// What to do about an invalid access.
    pub leaks_error_help: &'static str,
    /// Said once when no site could be placed in a file.
    pub leaks_no_debug_info: &'static str,
    /// Memory errors beyond leaks. Placeholder: `{count}`.
    pub leaks_errors: &'static str,
    /// The standing caveat printed with every leak result.
    pub leaks_not_a_proof: &'static str,

    /// Section label: why the rule exists.
    pub explain_why: &'static str,
    /// Section label: the reader's next step.
    pub explain_next: &'static str,
    /// Section label: why the tool did or did not act by itself.
    pub explain_safety: &'static str,
    /// No bundled article exists for a requested identifier. Placeholder: `{rule}`.
    pub explain_unknown_rule: &'static str,
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

    report_tagline: "Safe automatic fixes for the 42 Norm v4.1",
    report_project_reminder_label: "Project reminder:",
    report_project_reminder: "keep submitted code and permitted comments in English (not a Norm rule).",
    report_translation_scope: "Findings from the official Norminette and your C compiler are shown as those tools produced them.",
    report_files_heading: "Files",
    report_grouped_heading: "Diagnostics grouped by rule",
    report_diagnostics_heading: "Diagnostics",
    report_expected_files: "Only .c, .h, Makefile, and README files are expected.",
    report_preview_kept_files: "Preview mode did not move these files.",
    report_summary_label: "Summary:",
    report_summary_counts: "{files} files | {proposed} proposed | {written} written | {fixes} fixes | {remaining} remaining | {info} info | {failed} failed | {unexpected} unexpected | {quarantined} quarantined",
    report_completed_in: "Completed in {duration}.",
    report_estimate_label: "Pre-defense estimate:",
    report_estimate_value: "{verdict} | grade {grade} | {score}/100",
    report_estimate_caveat: "This estimate is heuristic and never replaces the official evaluation.",
    report_hard_fail_heading: "Hard-fail evidence",

    scope_refusal: "refusing to scan or modify protected scope `{scope}` because {reason}; inspect the path and pass --force to acknowledge it explicitly",
    scope_reason_filesystem_root: "it is a filesystem root",
    scope_reason_home_directory: "it is the complete user home directory",
    scope_reason_system_tree: "it is inside an operating-system-managed directory",
    scope_reason_broad_directory: "it is a broad system or multi-project directory",
    force_without_target: "--force requires --unsafe, --remove-unused, --remove-unexpected, or a protected system scope",

    destructive_warning: "WARNING: this run may remove proven-dead static code, proven-missing or trivia-only Makefile entries, unused missing-implementation header prototypes, and/or move unexpected files.",
    destructive_prompt: "Continue with recoverable destructive operations? [y/N] ",
    destructive_needs_confirmation: "destructive operations require an interactive y/N confirmation or --force",
    destructive_cancelled: "destructive operations were cancelled; no files were changed",
    undo_needs_confirmation: "undo requires an interactive y/N confirmation or --force",
    undo_question: "Restore {count} file(s) from {run}? Later edits are protected and will cause refusal.",
    undo_prompt: "Continue? [y/N] ",
    undo_cancelled: "undo was cancelled; no files were changed",
    error_nothing_written: "No unvalidated changes were written.",

    uninstall_recovery_warning: "This also deletes backups and quarantined files, which is the only copy of anything a previous run replaced or moved.",
    uninstall_prompt: "Remove the files listed above? [y/N] ",
    uninstall_needs_confirmation: "uninstall requires an interactive y/N confirmation or --force",
    uninstall_cancelled: "uninstall was cancelled; nothing was removed",
    uninstall_done: "normfix has been removed.",
    leaks_prompt: "normfix is about to run {program} under the leak checker. This executes your program. Continue? [y/N] ",
    leaks_needs_confirmation: "checking for leaks runs your program, which requires an interactive y/N confirmation or --force",
    leaks_cancelled: "the leak check was cancelled; your program was not run",
    leaks_unavailable: "No leak checker is installed.",
    leaks_install_hint: "Install Valgrind from your package manager.",
    leaks_install_hint_macos: "Upstream Valgrind does not build for macOS. The LouisBrunner/valgrind-macos port does: brew install LouisBrunner/valgrind/valgrind. Its Apple Silicon support is limited.",
    leaks_install_hint_windows: "Valgrind does not exist for Windows. Run normfix inside WSL, where the Linux checker works normally.",
    leaks_none: "Nothing was lost on the path this run took.",
    leaks_lost: "Lost {definite} bytes outright, and {indirect} more reachable only through them.",
    leaks_sites: "Allocated at:",
    leaks_site_direct: "{bytes} bytes allocated here were never freed",
    leaks_site_indirect: "{bytes} bytes allocated here were reachable only through a lost block",
    leaks_site_help: "This is where the memory was allocated, not where it should have been freed. Follow it to the path that loses the pointer.",
    leaks_error_at: "{kind}, in {function}",
    leaks_error_help: "The program touched memory it does not own. This is a bug regardless of what the Norm says about the file.",
    leaks_no_debug_info: "No line could be named. Rebuild with -g so the checker can place each allocation in your source.",
    leaks_errors: "The checker also reported {count} memory errors that are not leaks.",
    leaks_not_a_proof: "This is what one run observed with the arguments it was given. It is not a proof that the program never leaks.",

    explain_why: "Why",
    explain_next: "Next",
    explain_safety: "Safety",
    explain_unknown_rule: "No bundled explanation exists for `{rule}`. The rule remains available in the normal diagnostic report.",
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

    report_tagline: "Correções automáticas seguras para a Norm v4.1 da 42",
    report_project_reminder_label: "Lembrete do projeto:",
    report_project_reminder: "mantenha o código entregue e os comentários permitidos em inglês (não é uma regra da Norm).",
    report_translation_scope: "Os achados da Norminette oficial e do seu compilador C aparecem como aquelas ferramentas os produziram.",
    report_files_heading: "Arquivos",
    report_grouped_heading: "Diagnósticos agrupados por regra",
    report_diagnostics_heading: "Diagnósticos",
    report_expected_files: "Só são esperados arquivos .c, .h, Makefile e README.",
    report_preview_kept_files: "O modo de pré-visualização não moveu estes arquivos.",
    report_summary_label: "Resumo:",
    report_summary_counts: "{files} arquivo(s) | {proposed} proposto(s) | {written} gravado(s) | {fixes} correção(ões) | {remaining} pendente(s) | {info} informativo(s) | {failed} com falha | {unexpected} inesperado(s) | {quarantined} em quarentena",
    report_completed_in: "Concluído em {duration}.",
    report_estimate_label: "Estimativa pré-defesa:",
    report_estimate_value: "{verdict} | nota {grade} | {score}/100",
    report_estimate_caveat: "Esta estimativa é heurística e nunca substitui a avaliação oficial.",
    report_hard_fail_heading: "Evidências de reprovação",

    scope_refusal: "recusando ler ou modificar o escopo protegido `{scope}` porque {reason}; inspecione o caminho e passe --force para reconhecê-lo explicitamente",
    scope_reason_filesystem_root: "é a raiz do sistema de arquivos",
    scope_reason_home_directory: "é o diretório pessoal completo do usuário",
    scope_reason_system_tree: "está dentro de um diretório gerenciado pelo sistema operacional",
    scope_reason_broad_directory: "é um diretório amplo do sistema ou com vários projetos",
    force_without_target: "--force exige --unsafe, --remove-unused, --remove-unexpected ou um escopo protegido do sistema",

    destructive_warning: "ATENÇÃO: esta execução pode remover código static comprovadamente morto, entradas do Makefile comprovadamente ausentes ou só com trivialidades, protótipos de cabeçalho sem implementação e sem uso, e/ou mover arquivos inesperados.",
    destructive_prompt: "Continuar com as operações destrutivas recuperáveis? [y/N] ",
    destructive_needs_confirmation: "operações destrutivas exigem uma confirmação interativa y/N ou --force",
    destructive_cancelled: "as operações destrutivas foram canceladas; nenhum arquivo foi alterado",
    undo_needs_confirmation: "desfazer exige uma confirmação interativa y/N ou --force",
    undo_question: "Restaurar {count} arquivo(s) de {run}? Edições posteriores são protegidas e causarão recusa.",
    undo_prompt: "Continuar? [y/N] ",
    undo_cancelled: "o desfazer foi cancelado; nenhum arquivo foi alterado",
    error_nothing_written: "Nenhuma alteração não validada foi gravada.",

    uninstall_recovery_warning: "Isso também apaga os backups e os arquivos em quarentena, que são a única cópia de qualquer coisa que uma execução anterior tenha substituído ou movido.",
    uninstall_prompt: "Remover os arquivos listados acima? [y/N] ",
    uninstall_needs_confirmation: "a desinstalação exige uma confirmação interativa y/N ou --force",
    uninstall_cancelled: "a desinstalação foi cancelada; nada foi removido",
    uninstall_done: "o normfix foi removido.",
    leaks_prompt: "O normfix vai executar {program} sob o verificador de vazamentos. Isso roda o seu programa. Continuar? [y/N] ",
    leaks_needs_confirmation: "verificar vazamentos executa o seu programa, o que exige uma confirmação interativa y/N ou --force",
    leaks_cancelled: "a verificação foi cancelada; seu programa não foi executado",
    leaks_unavailable: "Nenhum verificador de vazamentos está instalado.",
    leaks_install_hint: "Instale o Valgrind pelo gerenciador de pacotes do seu sistema.",
    leaks_install_hint_macos: "O Valgrind oficial não compila no macOS. O port LouisBrunner/valgrind-macos compila: brew install LouisBrunner/valgrind/valgrind. O suporte dele a Apple Silicon é limitado.",
    leaks_install_hint_windows: "O Valgrind não existe para Windows. Rode o normfix dentro do WSL, onde o verificador de Linux funciona normalmente.",
    leaks_none: "Nada foi perdido no caminho que esta execução percorreu.",
    leaks_lost: "Perdidos {definite} bytes de vez, e mais {indirect} alcançáveis só por eles.",
    leaks_sites: "Alocados em:",
    leaks_site_direct: "{bytes} bytes alocados aqui nunca foram liberados",
    leaks_site_indirect: "{bytes} bytes alocados aqui só eram alcançáveis por um bloco perdido",
    leaks_site_help: "Aqui é onde a memória foi alocada, não onde deveria ter sido liberada. Siga daqui até o caminho que perde o ponteiro.",
    leaks_error_at: "{kind}, em {function}",
    leaks_error_help: "O programa mexeu em memória que não é dele. Isso é um bug, independentemente do que a Norm diga sobre o arquivo.",
    leaks_no_debug_info: "Nenhuma linha pôde ser apontada. Recompile com -g para o verificador localizar cada alocação no seu código.",
    leaks_errors: "O verificador também relatou {count} erros de memória que não são vazamentos.",
    leaks_not_a_proof: "Isto é o que uma execução observou com os argumentos que recebeu. Não é prova de que o programa nunca vaza.",

    explain_why: "Por quê",
    explain_next: "A seguir",
    explain_safety: "Segurança",
    explain_unknown_rule: "Nenhuma explicação embutida existe para `{rule}`. A regra continua disponível no relatório normal de diagnósticos.",
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

    report_tagline: "Correcciones automáticas seguras para la Norm v4.1 de 42",
    report_project_reminder_label: "Recordatorio del proyecto:",
    report_project_reminder: "mantén el código entregado y los comentarios permitidos en inglés (no es una regla de la Norm).",
    report_translation_scope: "Los hallazgos de la Norminette oficial y de tu compilador C se muestran tal como los produjeron esas herramientas.",
    report_files_heading: "Archivos",
    report_grouped_heading: "Diagnósticos agrupados por regla",
    report_diagnostics_heading: "Diagnósticos",
    report_expected_files: "Solo se esperan archivos .c, .h, Makefile y README.",
    report_preview_kept_files: "El modo de vista previa no movió estos archivos.",
    report_summary_label: "Resumen:",
    report_summary_counts: "{files} archivo(s) | {proposed} propuesto(s) | {written} escrito(s) | {fixes} corrección(es) | {remaining} pendiente(s) | {info} informativo(s) | {failed} con fallo | {unexpected} inesperado(s) | {quarantined} en cuarentena",
    report_completed_in: "Completado en {duration}.",
    report_estimate_label: "Estimación previa a la defensa:",
    report_estimate_value: "{verdict} | nota {grade} | {score}/100",
    report_estimate_caveat: "Esta estimación es heurística y nunca sustituye a la evaluación oficial.",
    report_hard_fail_heading: "Evidencias de suspenso",

    scope_refusal: "se rechaza leer o modificar el alcance protegido `{scope}` porque {reason}; inspecciona la ruta y pasa --force para reconocerla explícitamente",
    scope_reason_filesystem_root: "es la raíz del sistema de archivos",
    scope_reason_home_directory: "es el directorio personal completo del usuario",
    scope_reason_system_tree: "está dentro de un directorio gestionado por el sistema operativo",
    scope_reason_broad_directory: "es un directorio amplio del sistema o con varios proyectos",
    force_without_target: "--force requiere --unsafe, --remove-unused, --remove-unexpected o un alcance protegido del sistema",

    destructive_warning: "ATENCIÓN: esta ejecución puede eliminar código static probadamente muerto, entradas del Makefile probadamente ausentes o solo con trivialidades, prototipos de cabecera sin implementación y sin uso, y/o mover archivos inesperados.",
    destructive_prompt: "¿Continuar con las operaciones destructivas recuperables? [y/N] ",
    destructive_needs_confirmation: "las operaciones destructivas requieren una confirmación interactiva y/N o --force",
    destructive_cancelled: "las operaciones destructivas se cancelaron; no se modificó ningún archivo",
    undo_needs_confirmation: "deshacer requiere una confirmación interactiva y/N o --force",
    undo_question: "¿Restaurar {count} archivo(s) de {run}? Las ediciones posteriores están protegidas y provocarán un rechazo.",
    undo_prompt: "¿Continuar? [y/N] ",
    undo_cancelled: "el deshacer se canceló; no se modificó ningún archivo",
    error_nothing_written: "No se escribió ningún cambio sin validar.",

    uninstall_recovery_warning: "Esto también borra las copias de seguridad y los archivos en cuarentena, que son la única copia de todo lo que una ejecución anterior sustituyó o movió.",
    uninstall_prompt: "¿Eliminar los archivos listados arriba? [y/N] ",
    uninstall_needs_confirmation: "la desinstalación requiere una confirmación interactiva y/N o --force",
    uninstall_cancelled: "la desinstalación se canceló; no se eliminó nada",
    uninstall_done: "normfix se ha eliminado.",
    leaks_prompt: "normfix va a ejecutar {program} bajo el verificador de fugas. Esto ejecuta tu programa. ¿Continuar? [y/N] ",
    leaks_needs_confirmation: "comprobar fugas ejecuta tu programa, lo que exige una confirmación interactiva y/N o --force",
    leaks_cancelled: "la comprobación se canceló; tu programa no se ejecutó",
    leaks_unavailable: "No hay ningún verificador de fugas instalado.",
    leaks_install_hint: "Instala Valgrind desde el gestor de paquetes de tu sistema.",
    leaks_install_hint_macos: "Valgrind oficial no compila en macOS. El port LouisBrunner/valgrind-macos sí: brew install LouisBrunner/valgrind/valgrind. Su soporte para Apple Silicon es limitado.",
    leaks_install_hint_windows: "Valgrind no existe para Windows. Ejecuta normfix dentro de WSL, donde el verificador de Linux funciona con normalidad.",
    leaks_none: "No se perdió nada en el camino que tomó esta ejecución.",
    leaks_lost: "Se perdieron {definite} bytes del todo, y {indirect} más alcanzables solo a través de ellos.",
    leaks_sites: "Reservados en:",
    leaks_site_direct: "{bytes} bytes reservados aquí nunca se liberaron",
    leaks_site_indirect: "{bytes} bytes reservados aquí solo eran alcanzables por un bloque perdido",
    leaks_site_help: "Aquí es donde se reservó la memoria, no donde debería haberse liberado. Sigue desde aquí hasta el camino que pierde el puntero.",
    leaks_error_at: "{kind}, en {function}",
    leaks_error_help: "El programa tocó memoria que no es suya. Eso es un error, diga lo que diga la Norm sobre el archivo.",
    leaks_no_debug_info: "No se pudo señalar ninguna línea. Recompila con -g para que el verificador ubique cada reserva en tu código.",
    leaks_errors: "El verificador también informó de {count} errores de memoria que no son fugas.",
    leaks_not_a_proof: "Esto es lo que observó una ejecución con los argumentos que recibió. No es una prueba de que el programa nunca tenga fugas.",

    explain_why: "Por qué",
    explain_next: "A continuación",
    explain_safety: "Seguridad",
    explain_unknown_rule: "No hay ninguna explicación incluida para `{rule}`. La regla sigue disponible en el informe normal de diagnósticos.",
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

    report_tagline: "Corrections automatiques sûres pour la Norm v4.1 de 42",
    report_project_reminder_label: "Rappel du projet :",
    report_project_reminder: "gardez le code rendu et les commentaires autorisés en anglais (ce n'est pas une règle de la Norm).",
    report_translation_scope: "Les constats de la Norminette officielle et de votre compilateur C sont affichés tels que ces outils les ont produits.",
    report_files_heading: "Fichiers",
    report_grouped_heading: "Diagnostics groupés par règle",
    report_diagnostics_heading: "Diagnostics",
    report_expected_files: "Seuls les fichiers .c, .h, Makefile et README sont attendus.",
    report_preview_kept_files: "Le mode aperçu n'a déplacé aucun de ces fichiers.",
    report_summary_label: "Résumé :",
    report_summary_counts: "{files} fichier(s) | {proposed} proposé(s) | {written} écrit(s) | {fixes} correction(s) | {remaining} restant(s) | {info} informatif(s) | {failed} en échec | {unexpected} inattendu(s) | {quarantined} en quarantaine",
    report_completed_in: "Terminé en {duration}.",
    report_estimate_label: "Estimation avant soutenance :",
    report_estimate_value: "{verdict} | note {grade} | {score}/100",
    report_estimate_caveat: "Cette estimation est heuristique et ne remplace jamais l'évaluation officielle.",
    report_hard_fail_heading: "Preuves d'échec",

    scope_refusal: "refus de lire ou de modifier la portée protégée `{scope}` car {reason} ; inspectez le chemin et passez --force pour l'accepter explicitement",
    scope_reason_filesystem_root: "c'est une racine du système de fichiers",
    scope_reason_home_directory: "c'est le répertoire personnel complet de l'utilisateur",
    scope_reason_system_tree: "il se trouve dans un répertoire géré par le système d'exploitation",
    scope_reason_broad_directory: "c'est un répertoire système large ou contenant plusieurs projets",
    force_without_target: "--force exige --unsafe, --remove-unused, --remove-unexpected ou une portée système protégée",

    destructive_warning: "ATTENTION : cette exécution peut supprimer du code static prouvé mort, des entrées de Makefile prouvées absentes ou sans code, des prototypes d'en-tête sans implémentation ni usage, et/ou déplacer des fichiers inattendus.",
    destructive_prompt: "Continuer avec les opérations destructives récupérables ? [y/N] ",
    destructive_needs_confirmation: "les opérations destructives exigent une confirmation interactive y/N ou --force",
    destructive_cancelled: "les opérations destructives ont été annulées ; aucun fichier n'a été modifié",
    undo_needs_confirmation: "l'annulation exige une confirmation interactive y/N ou --force",
    undo_question: "Restaurer {count} fichier(s) depuis {run} ? Les modifications ultérieures sont protégées et provoqueront un refus.",
    undo_prompt: "Continuer ? [y/N] ",
    undo_cancelled: "l'annulation a été abandonnée ; aucun fichier n'a été modifié",
    error_nothing_written: "Aucune modification non validée n'a été écrite.",

    uninstall_recovery_warning: "Cela supprime aussi les sauvegardes et les fichiers en quarantaine, qui sont l'unique copie de tout ce qu'une exécution précédente a remplacé ou déplacé.",
    uninstall_prompt: "Supprimer les fichiers listés ci-dessus ? [y/N] ",
    uninstall_needs_confirmation: "la désinstallation exige une confirmation interactive y/N ou --force",
    uninstall_cancelled: "la désinstallation a été annulée ; rien n'a été supprimé",
    uninstall_done: "normfix a été supprimé.",
    leaks_prompt: "normfix va exécuter {program} sous le détecteur de fuites. Cela exécute votre programme. Continuer ? [y/N] ",
    leaks_needs_confirmation: "vérifier les fuites exécute votre programme, ce qui exige une confirmation interactive y/N ou --force",
    leaks_cancelled: "la vérification a été annulée ; votre programme n’a pas été exécuté",
    leaks_unavailable: "Aucun détecteur de fuites n’est installé.",
    leaks_install_hint: "Installez Valgrind depuis le gestionnaire de paquets de votre système.",
    leaks_install_hint_macos: "Valgrind amont ne se compile pas sur macOS. Le port LouisBrunner/valgrind-macos, si : brew install LouisBrunner/valgrind/valgrind. Sa prise en charge d’Apple Silicon est limitée.",
    leaks_install_hint_windows: "Valgrind n’existe pas pour Windows. Exécutez normfix dans WSL, où le détecteur Linux fonctionne normalement.",
    leaks_none: "Rien n’a été perdu sur le chemin emprunté par cette exécution.",
    leaks_lost: "Perdus {definite} octets définitivement, et {indirect} de plus accessibles uniquement par eux.",
    leaks_sites: "Alloués à :",
    leaks_site_direct: "{bytes} octets alloués ici n’ont jamais été libérés",
    leaks_site_indirect: "{bytes} octets alloués ici n’étaient accessibles que par un bloc perdu",
    leaks_site_help: "C’est là que la mémoire a été allouée, pas là où elle aurait dû être libérée. Suivez ce point jusqu’au chemin qui perd le pointeur.",
    leaks_error_at: "{kind}, dans {function}",
    leaks_error_help: "Le programme a touché de la mémoire qui ne lui appartient pas. C’est un bug, quoi que la Norm dise du fichier.",
    leaks_no_debug_info: "Aucune ligne n’a pu être nommée. Recompilez avec -g pour que le vérificateur situe chaque allocation dans votre code.",
    leaks_errors: "Le détecteur a aussi signalé {count} erreurs mémoire qui ne sont pas des fuites.",
    leaks_not_a_proof: "Voici ce qu’une exécution a observé avec les arguments reçus. Ce n’est pas une preuve que le programme ne fuit jamais.",

    explain_why: "Pourquoi",
    explain_next: "Ensuite",
    explain_safety: "Sûreté",
    explain_unknown_rule: "Aucune explication intégrée n'existe pour `{rule}`. La règle reste disponible dans le rapport de diagnostics normal.",
};

#[cfg(test)]
mod tests {
    use crate::{Locale, PUBLISHED, messages};

    // One line per catalogue entry: the list is long by construction, and
    // splitting it would only hide which entries the guards actually cover.
    #[allow(clippy::too_many_lines)]
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
            ("report_tagline", messages.report_tagline),
            (
                "report_project_reminder_label",
                messages.report_project_reminder_label,
            ),
            ("report_project_reminder", messages.report_project_reminder),
            (
                "report_translation_scope",
                messages.report_translation_scope,
            ),
            ("report_files_heading", messages.report_files_heading),
            ("report_grouped_heading", messages.report_grouped_heading),
            (
                "report_diagnostics_heading",
                messages.report_diagnostics_heading,
            ),
            ("report_expected_files", messages.report_expected_files),
            (
                "report_preview_kept_files",
                messages.report_preview_kept_files,
            ),
            ("report_summary_label", messages.report_summary_label),
            ("report_summary_counts", messages.report_summary_counts),
            ("report_completed_in", messages.report_completed_in),
            ("report_estimate_label", messages.report_estimate_label),
            ("report_estimate_value", messages.report_estimate_value),
            ("report_estimate_caveat", messages.report_estimate_caveat),
            (
                "report_hard_fail_heading",
                messages.report_hard_fail_heading,
            ),
            ("scope_refusal", messages.scope_refusal),
            (
                "scope_reason_filesystem_root",
                messages.scope_reason_filesystem_root,
            ),
            (
                "scope_reason_home_directory",
                messages.scope_reason_home_directory,
            ),
            (
                "scope_reason_system_tree",
                messages.scope_reason_system_tree,
            ),
            (
                "scope_reason_broad_directory",
                messages.scope_reason_broad_directory,
            ),
            ("force_without_target", messages.force_without_target),
            ("destructive_warning", messages.destructive_warning),
            ("destructive_prompt", messages.destructive_prompt),
            (
                "destructive_needs_confirmation",
                messages.destructive_needs_confirmation,
            ),
            ("destructive_cancelled", messages.destructive_cancelled),
            ("undo_needs_confirmation", messages.undo_needs_confirmation),
            ("undo_question", messages.undo_question),
            ("undo_prompt", messages.undo_prompt),
            ("undo_cancelled", messages.undo_cancelled),
            ("error_nothing_written", messages.error_nothing_written),
            (
                "uninstall_recovery_warning",
                messages.uninstall_recovery_warning,
            ),
            ("uninstall_prompt", messages.uninstall_prompt),
            (
                "uninstall_needs_confirmation",
                messages.uninstall_needs_confirmation,
            ),
            ("uninstall_cancelled", messages.uninstall_cancelled),
            ("uninstall_done", messages.uninstall_done),
            ("explain_why", messages.explain_why),
            ("explain_next", messages.explain_next),
            ("explain_safety", messages.explain_safety),
            ("explain_unknown_rule", messages.explain_unknown_rule),
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
            "report_diagnostics_heading",
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
