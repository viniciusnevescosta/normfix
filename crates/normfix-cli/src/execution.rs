//! Deterministic start-of-run output for humans and automation.

use std::fmt::Write as _;

use normfix_i18n::Messages;
use serde::Serialize;

/// Effective, already-validated configuration announced before project work.
// These independent booleans are part of a stable, explicit machine event;
// replacing them with state enums would make unrelated settings exclusive.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Debug, Serialize)]
pub struct ExecutionStart {
    /// Stable event discriminator for non-interactive consumers.
    pub event: &'static str,
    /// Selected CLI workflow.
    pub action: String,
    /// Whether the workflow writes, checks, or prints diffs.
    pub mode: String,
    /// Working directory used to resolve relative paths.
    pub current_directory: String,
    /// Compact effective scope description.
    pub scope: String,
    /// Identity selected for official headers, or an unavailable marker.
    pub identity: String,
    /// Origin of the selected identity or refusal explanation.
    pub identity_source: String,
    /// Worker configuration (`auto` or an exact count).
    pub workers: String,
    /// Per-file Norminette timeout in seconds.
    pub timeout_seconds: f64,
    /// Norminette executable selection.
    pub norminette: String,
    /// Whether an untested checker warns or stops the run.
    pub norminette_version_policy: String,
    /// Whether compiler diagnostics are active.
    pub compiler_preflight: bool,
    /// Whether persistent analysis caching is active.
    pub cache: bool,
    /// Whether ignored files are omitted from recursive discovery.
    pub respect_gitignore: bool,
    /// Backup policy in effect for ordinary writes.
    pub backups: String,
    /// Enabled destructive capabilities, or `none`.
    pub destructive: String,
    /// Whether `--force` acknowledged a protected scope or destructive run.
    pub forced: bool,
    /// Optional advisory produced while preparing the run.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub advisory: Option<String>,
}

impl ExecutionStart {
    /// Renders a compact, color-free block suitable for a human terminal.
    ///
    /// The prose fields are supplied already translated by the caller, because
    /// only the human path is localized: the JSON event keeps English values so
    /// automation never has to select a language to stay reliable.
    #[must_use]
    pub fn to_human(&self, messages: &Messages) -> String {
        let rows = [
            (messages.label_action, self.action.clone()),
            (messages.label_mode, self.mode.clone()),
            (messages.label_scope, self.scope.clone()),
            (
                messages.label_working_directory,
                self.current_directory.clone(),
            ),
            (
                messages.label_identity,
                format!("{} ({})", self.identity, self.identity_source),
            ),
            (messages.label_workers, self.workers.clone()),
            (
                messages.label_checks,
                if self.compiler_preflight {
                    messages.checks_norminette_and_compiler
                } else {
                    messages.checks_norminette
                }
                .to_owned(),
            ),
            (messages.label_norminette, self.norminette.clone()),
            (
                messages.label_version_rule,
                self.norminette_version_policy.clone(),
            ),
            (
                messages.label_timeout,
                normfix_i18n::fill(
                    messages.timeout_per_file,
                    &[("seconds", &self.timeout_seconds.to_string())],
                ),
            ),
            (
                messages.label_cache,
                if self.cache {
                    messages.state_enabled
                } else {
                    messages.state_disabled
                }
                .to_owned(),
            ),
            (
                messages.label_gitignore,
                if self.respect_gitignore {
                    messages.gitignore_respected
                } else {
                    messages.gitignore_not_applied
                }
                .to_owned(),
            ),
            (messages.label_backups, self.backups.clone()),
            (messages.label_destructive, self.destructive.clone()),
            (
                messages.label_force,
                if self.forced {
                    messages.force_acknowledged
                } else {
                    messages.force_absent
                }
                .to_owned(),
            ),
        ];

        // A translated label can be longer than its English original, so the
        // value column is measured rather than assumed.
        let width = rows
            .iter()
            .map(|(label, _)| label.chars().count())
            .chain(
                self.advisory
                    .iter()
                    .map(|_| messages.label_advisory.chars().count()),
            )
            .max()
            .unwrap_or(0);

        let mut output = format!("{}\n", messages.starting_banner);
        for (label, value) in &rows {
            field(&mut output, label, value, width);
        }
        if let Some(advisory) = &self.advisory {
            field(&mut output, messages.label_advisory, advisory, width);
        }
        output.push('\n');
        output
    }

    /// Renders one JSON event line without affecting the final JSON report on stdout.
    pub fn to_json_line(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

fn field(output: &mut String, label: &str, value: &str, width: usize) {
    // Padding is counted in characters so an accented label still lines up.
    let padding = " ".repeat(width.saturating_sub(label.chars().count()));
    let _ = writeln!(output, "  {label}{padding} {}", terminal_safe_inline(value));
}

pub(crate) fn terminal_safe_inline(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character if character.is_control() => {
                let _ = write!(output, "\\u{{{:x}}}", u32::from(character));
            }
            character => output.push(character),
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::ExecutionStart;

    fn event() -> ExecutionStart {
        ExecutionStart {
            event: "execution_start",
            action: "format".to_owned(),
            mode: "write".to_owned(),
            current_directory: "/tmp/project".to_owned(),
            scope: "current directory (recursive)".to_owned(),
            identity: "student@student.42.fr".to_owned(),
            identity_source: "user config".to_owned(),
            workers: "auto".to_owned(),
            timeout_seconds: 5.0,
            norminette: "PATH lookup".to_owned(),
            norminette_version_policy: "advisory".to_owned(),
            compiler_preflight: true,
            cache: true,
            respect_gitignore: false,
            backups: "automatic external backup".to_owned(),
            destructive: "none".to_owned(),
            forced: false,
            advisory: None,
        }
    }

    fn english() -> &'static normfix_i18n::Messages {
        normfix_i18n::messages(normfix_i18n::Locale::English)
    }

    #[test]
    fn human_start_block_names_action_scope_and_effective_configuration() {
        let output = event().to_human(english());
        assert!(output.starts_with("normfix · starting\n"));
        assert!(output.contains("action       format"));
        assert!(output.contains("scope        current directory (recursive)"));
        assert!(output.contains("checks       Norminette + strict compiler"));
    }

    #[test]
    fn human_start_block_escapes_terminal_controls() {
        let mut unsafe_event = event();
        unsafe_event.scope = "project\u{1b}]8;;bad\u{7}\nnext".to_owned();
        let output = unsafe_event.to_human(english());
        assert!(!output.contains('\u{1b}'));
        assert!(!output.contains('\u{7}'));
        assert!(output.contains("\\u{1b}"));
        assert!(output.contains("\\nnext"));
    }

    #[test]
    fn a_localized_block_translates_labels_and_derived_values() {
        let messages = normfix_i18n::messages(normfix_i18n::Locale::Portuguese);
        let output = event().to_human(messages);

        assert!(output.starts_with("normfix · iniciando\n"));
        assert!(output.contains("verificações"));
        assert!(output.contains("Norminette + compilador estrito"));
        assert!(output.contains("5s por arquivo"));
        // A command name is an API token and stays English in every locale.
        assert!(output.contains("format"));
    }

    #[test]
    fn an_accented_label_still_aligns_its_value_column() {
        // `vérifications` is longer than any English label, so a fixed width
        // would push its value out of the column the other rows use.
        let messages = normfix_i18n::messages(normfix_i18n::Locale::French);
        let output = event().to_human(messages);

        // `règle de version` is the longest French label, so every value must
        // begin two spaces of indent plus that width plus one separator in.
        let column = 2 + messages.label_version_rule.chars().count() + 1;
        for line in output.lines().skip(1).filter(|line| !line.is_empty()) {
            let characters = line.chars().collect::<Vec<_>>();
            assert!(
                characters.len() > column,
                "row is shorter than the value column: {line:?}"
            );
            assert_eq!(
                characters[column - 1],
                ' ',
                "value column is not preceded by the separator: {line:?}"
            );
            assert_ne!(
                characters[column], ' ',
                "value does not start at the shared column: {line:?}"
            );
        }
    }

    #[test]
    fn json_start_event_is_a_single_machine_readable_line() {
        let output = event().to_json_line().expect("serialize event");
        let value: serde_json::Value = serde_json::from_str(&output).expect("JSON event");
        assert_eq!(value["event"], "execution_start");
        assert_eq!(value["action"], "format");
        assert!(!output.contains('\n'));
    }
}
