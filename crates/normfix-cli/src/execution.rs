//! Deterministic start-of-run output for humans and automation.

use std::fmt::Write as _;

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
    #[must_use]
    pub fn to_human(&self) -> String {
        let mut output = String::from("normfix · starting\n");
        field(&mut output, "action", &self.action);
        field(&mut output, "mode", &self.mode);
        field(&mut output, "scope", &self.scope);
        field(&mut output, "working dir", &self.current_directory);
        field(
            &mut output,
            "identity",
            &format!("{} ({})", self.identity, self.identity_source),
        );
        field(&mut output, "workers", &self.workers);
        field(
            &mut output,
            "checks",
            if self.compiler_preflight {
                "Norminette + strict compiler"
            } else {
                "Norminette"
            },
        );
        field(&mut output, "norminette", &self.norminette);
        field(&mut output, "version rule", &self.norminette_version_policy);
        field(
            &mut output,
            "timeout",
            &format!("{}s per file", self.timeout_seconds),
        );
        field(
            &mut output,
            "cache",
            if self.cache { "enabled" } else { "disabled" },
        );
        field(
            &mut output,
            "gitignore",
            if self.respect_gitignore {
                "respected"
            } else {
                "not applied"
            },
        );
        field(&mut output, "backups", &self.backups);
        field(&mut output, "destructive", &self.destructive);
        field(
            &mut output,
            "force",
            if self.forced { "acknowledged" } else { "no" },
        );
        if let Some(advisory) = &self.advisory {
            field(&mut output, "advisory", advisory);
        }
        output.push('\n');
        output
    }

    /// Renders one JSON event line without affecting the final JSON report on stdout.
    pub fn to_json_line(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

fn field(output: &mut String, label: &str, value: &str) {
    let _ = writeln!(output, "  {label:<12} {}", terminal_safe_inline(value));
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

    #[test]
    fn human_start_block_names_action_scope_and_effective_configuration() {
        let output = event().to_human();
        assert!(output.starts_with("normfix · starting\n"));
        assert!(output.contains("action       format"));
        assert!(output.contains("scope        current directory (recursive)"));
        assert!(output.contains("checks       Norminette + strict compiler"));
    }

    #[test]
    fn human_start_block_escapes_terminal_controls() {
        let mut unsafe_event = event();
        unsafe_event.scope = "project\u{1b}]8;;bad\u{7}\nnext".to_owned();
        let output = unsafe_event.to_human();
        assert!(!output.contains('\u{1b}'));
        assert!(!output.contains('\u{7}'));
        assert!(output.contains("\\u{1b}"));
        assert!(output.contains("\\nnext"));
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
