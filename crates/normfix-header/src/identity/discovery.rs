//! Passive identity discovery from bounded editor and shell settings.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use regex::Regex;

use super::SETTINGS_SIZE_LIMIT;
use super::file::{SymlinkPolicy, read_bounded_regular_file};
use super::validation::canonical_42_email;

#[derive(Clone, Copy)]
enum EditorPattern {
    Vim,
    Lua,
    Shell,
    Json,
}

struct EditorLocation {
    path: PathBuf,
    pattern: EditorPattern,
    source: &'static str,
}

pub(super) fn saved_editor_emails(home: &Path) -> BTreeMap<String, BTreeSet<String>> {
    let mut candidates = BTreeMap::<String, BTreeSet<String>>::new();
    for location in editor_locations(home) {
        let Some(bytes) =
            read_bounded_regular_file(&location.path, SETTINGS_SIZE_LIMIT, SymlinkPolicy::Follow)
        else {
            continue;
        };
        let content = String::from_utf8_lossy(&bytes);
        for captures in editor_regex(location.pattern).captures_iter(&content) {
            let Some(value) = captures.get(1) else {
                continue;
            };
            let Some(email) = canonical_42_email(value.as_str()) else {
                continue;
            };
            candidates
                .entry(email)
                .or_default()
                .insert(location.source.to_owned());
        }
    }
    candidates
}

fn editor_regex(pattern: EditorPattern) -> &'static Regex {
    static VIM: OnceLock<Regex> = OnceLock::new();
    static LUA: OnceLock<Regex> = OnceLock::new();
    static SHELL: OnceLock<Regex> = OnceLock::new();
    static JSON: OnceLock<Regex> = OnceLock::new();
    let (cell, expression) = match pattern {
        EditorPattern::Vim => (&VIM, r#"\bg:mail42\s*=\s*['"]([^'"]+)['"]"#),
        EditorPattern::Lua => (&LUA, r#"\bvim\.g\.mail42\s*=\s*['"]([^'"]+)['"]"#),
        EditorPattern::Shell => (
            &SHELL,
            r#"(?m)^[ \t]*(?:export[ \t]+)?MAIL\s*=\s*['"]?([^'"\s#]+)"#,
        ),
        EditorPattern::Json => (&JSON, r#""42header\.email"\s*:\s*"([^"]+)""#),
    };
    cell.get_or_init(|| Regex::new(expression).expect("editor email regex is constant"))
}

fn editor_locations(home: &Path) -> Vec<EditorLocation> {
    [
        (".vimrc", EditorPattern::Vim, "Vim settings"),
        (
            ".config/nvim/init.vim",
            EditorPattern::Vim,
            "Neovim settings",
        ),
        (
            ".config/nvim/init.lua",
            EditorPattern::Lua,
            "Neovim settings",
        ),
        (".zshrc", EditorPattern::Shell, "shell settings"),
        (".zprofile", EditorPattern::Shell, "shell settings"),
        (".bashrc", EditorPattern::Shell, "shell settings"),
        (".bash_profile", EditorPattern::Shell, "shell settings"),
        (
            "Library/Application Support/Code/User/settings.json",
            EditorPattern::Json,
            "VS Code settings",
        ),
        (
            "Library/Application Support/Cursor/User/settings.json",
            EditorPattern::Json,
            "Cursor settings",
        ),
        (
            ".config/Code/User/settings.json",
            EditorPattern::Json,
            "VS Code settings",
        ),
        (
            ".config/VSCodium/User/settings.json",
            EditorPattern::Json,
            "VSCodium settings",
        ),
        (
            ".config/Cursor/User/settings.json",
            EditorPattern::Json,
            "Cursor settings",
        ),
    ]
    .into_iter()
    .map(|(relative, pattern, source)| EditorLocation {
        path: home.join(relative),
        pattern,
        source,
    })
    .collect()
}
