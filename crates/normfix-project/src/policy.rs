//! Strict, dependency-free parsing of the small project policy surface.

use std::collections::BTreeSet;
use std::fs;
use std::io::Read as _;
use std::path::{Path, PathBuf};

use thiserror::Error;

const POLICY_LIMIT: u64 = 64 * 1024;

/// Subject-specific project policy loaded from `normfix.toml`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectPolicy {
    /// Configuration file used for diagnostics.
    pub path: PathBuf,
    /// Optional human-readable 42 project name.
    pub name: Option<String>,
    /// Complete allowlist of external callable identifiers.
    pub allowed_functions: BTreeSet<String>,
}

/// A present policy file could not be interpreted without guessing.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum PolicyError {
    /// Policy metadata or bytes could not be read.
    #[error("could not read `{path}`: {message}")]
    Read {
        /// Policy path.
        path: PathBuf,
        /// Operating-system detail.
        message: String,
    },
    /// The bounded `[project]` syntax is malformed or ambiguous.
    #[error("invalid policy `{path}`: {message}")]
    Invalid {
        /// Policy path.
        path: PathBuf,
        /// Actionable parse detail.
        message: String,
    },
}

/// Loads `normfix.toml` from a project root.
///
/// Only `[project]`, an optional quoted `name`, and a quoted-string `allowed`
/// array are accepted. Other sections and keys are ignored, while a malformed
/// relevant value fails closed. Missing files return `Ok(None)`.
///
/// # Errors
///
/// Returns [`PolicyError`] for an unreadable, oversized, non-UTF-8, duplicate,
/// or malformed relevant policy value.
pub fn load_project_policy(root: &Path) -> Result<Option<ProjectPolicy>, PolicyError> {
    let path = root.join("normfix.toml");
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(PolicyError::Read {
                path,
                message: error.to_string(),
            });
        }
    };
    if !metadata.file_type().is_file() || metadata.len() > POLICY_LIMIT {
        return Err(PolicyError::Invalid {
            path,
            message: format!(
                "policy must be one non-symlink regular file no larger than {POLICY_LIMIT} bytes"
            ),
        });
    }
    let input = fs::File::open(&path).map_err(|error| PolicyError::Read {
        path: path.clone(),
        message: error.to_string(),
    })?;
    let opened_metadata = input.metadata().map_err(|error| PolicyError::Read {
        path: path.clone(),
        message: error.to_string(),
    })?;
    if !same_file_snapshot(&metadata, &opened_metadata) {
        return Err(PolicyError::Invalid {
            path,
            message: "policy changed while its snapshot was being opened".to_owned(),
        });
    }
    let capacity = usize::try_from(metadata.len()).map_err(|error| PolicyError::Invalid {
        path: path.clone(),
        message: format!("policy size cannot be represented on this platform: {error}"),
    })?;
    let mut bytes = Vec::with_capacity(capacity);
    input
        .take(POLICY_LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| PolicyError::Read {
            path: path.clone(),
            message: error.to_string(),
        })?;
    if bytes.len() as u64 > POLICY_LIMIT {
        return Err(PolicyError::Invalid {
            path,
            message: format!("policy grew beyond the {POLICY_LIMIT}-byte limit while reading"),
        });
    }
    let final_metadata = fs::symlink_metadata(&path).map_err(|error| PolicyError::Read {
        path: path.clone(),
        message: error.to_string(),
    })?;
    if !final_metadata.file_type().is_file()
        || !same_file_snapshot(&opened_metadata, &final_metadata)
    {
        return Err(PolicyError::Invalid {
            path,
            message: "policy changed while its snapshot was being read".to_owned(),
        });
    }
    let source = String::from_utf8(bytes).map_err(|error| PolicyError::Invalid {
        path: path.clone(),
        message: format!("policy is not valid UTF-8: {error}"),
    })?;
    parse_policy(path, &source).map(Some)
}

#[cfg(unix)]
fn same_file_snapshot(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;

    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
}

#[cfg(not(unix))]
fn same_file_snapshot(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.len() == right.len()
        && left.modified().ok() == right.modified().ok()
        && left.file_type().is_file() == right.file_type().is_file()
}

fn parse_policy(path: PathBuf, source: &str) -> Result<ProjectPolicy, PolicyError> {
    let logical = logical_lines(source).map_err(|message| PolicyError::Invalid {
        path: path.clone(),
        message,
    })?;
    let mut in_project = false;
    let mut name = None;
    let mut allowed = None;
    for line in logical {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') {
            in_project = line == "[project]";
            continue;
        }
        if !in_project {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            return Err(PolicyError::Invalid {
                path,
                message: format!("expected key = value, found `{line}`"),
            });
        };
        match key.trim() {
            "name" => {
                if name.is_some() {
                    return Err(PolicyError::Invalid {
                        path,
                        message: "project.name is declared more than once".to_owned(),
                    });
                }
                name =
                    Some(
                        parse_string(value.trim()).map_err(|message| PolicyError::Invalid {
                            path: path.clone(),
                            message: format!("project.name {message}"),
                        })?,
                    );
            }
            "allowed" => {
                if allowed.is_some() {
                    return Err(PolicyError::Invalid {
                        path,
                        message: "project.allowed is declared more than once".to_owned(),
                    });
                }
                allowed = Some(parse_string_array(value.trim()).map_err(|message| {
                    PolicyError::Invalid {
                        path: path.clone(),
                        message: format!("project.allowed {message}"),
                    }
                })?);
            }
            _ => {}
        }
    }
    let allowed_functions = allowed.ok_or_else(|| PolicyError::Invalid {
        path: path.clone(),
        message: "[project] must declare allowed = [\"function\", ...]".to_owned(),
    })?;
    Ok(ProjectPolicy {
        path,
        name,
        allowed_functions,
    })
}

fn logical_lines(source: &str) -> Result<Vec<String>, String> {
    let mut lines = Vec::new();
    let mut pending = String::new();
    let mut bracket_depth = 0_i32;
    for physical in source.lines() {
        let clean = strip_comment(physical)?;
        if pending.is_empty() {
            pending.push_str(clean.trim());
        } else {
            pending.push(' ');
            pending.push_str(clean.trim());
        }
        bracket_depth += bracket_delta(&clean)?;
        if bracket_depth < 0 {
            return Err("unexpected closing array bracket".to_owned());
        }
        if bracket_depth == 0 {
            lines.push(std::mem::take(&mut pending));
        }
    }
    if bracket_depth != 0 {
        return Err("unterminated array".to_owned());
    }
    if !pending.trim().is_empty() {
        lines.push(pending);
    }
    Ok(lines)
}

fn strip_comment(line: &str) -> Result<String, String> {
    let mut quoted = false;
    let mut escaped = false;
    for (index, character) in line.char_indices() {
        if escaped {
            escaped = false;
        } else if quoted && character == '\\' {
            escaped = true;
        } else if character == '"' {
            quoted = !quoted;
        } else if character == '#' && !quoted {
            return Ok(line[..index].to_owned());
        }
    }
    if quoted || escaped {
        return Err("unterminated quoted string".to_owned());
    }
    Ok(line.to_owned())
}

fn bracket_delta(line: &str) -> Result<i32, String> {
    let mut quoted = false;
    let mut escaped = false;
    let mut delta = 0_i32;
    for character in line.chars() {
        if escaped {
            escaped = false;
        } else if quoted && character == '\\' {
            escaped = true;
        } else if character == '"' {
            quoted = !quoted;
        } else if !quoted && character == '[' {
            delta += 1;
        } else if !quoted && character == ']' {
            delta -= 1;
        }
    }
    if quoted || escaped {
        return Err("unterminated quoted string".to_owned());
    }
    Ok(delta)
}

fn parse_string(value: &str) -> Result<String, String> {
    let mut values = parse_comma_separated_strings(value, false)?;
    if values.len() != 1 {
        return Err("must be exactly one quoted string".to_owned());
    }
    Ok(values.remove(0))
}

fn parse_string_array(value: &str) -> Result<BTreeSet<String>, String> {
    let trimmed = value.trim();
    let inner = trimmed
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .ok_or_else(|| "must be a bracketed array".to_owned())?;
    if inner.trim().is_empty() {
        return Ok(BTreeSet::new());
    }
    let values = parse_comma_separated_strings(inner, true)?;
    let mut output = BTreeSet::new();
    for value in values {
        if !is_identifier(&value) {
            return Err(format!("contains invalid callable identifier `{value}`"));
        }
        if !output.insert(value.clone()) {
            return Err(format!("contains duplicate `{value}`"));
        }
    }
    Ok(output)
}

fn parse_comma_separated_strings(value: &str, allow_multiple: bool) -> Result<Vec<String>, String> {
    let bytes = value.as_bytes();
    let mut output = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        while bytes.get(index).is_some_and(u8::is_ascii_whitespace) {
            index += 1;
        }
        if index == bytes.len() {
            break;
        }
        if bytes[index] != b'"' {
            return Err("must contain only comma-separated quoted strings".to_owned());
        }
        index += 1;
        let mut item = String::new();
        let mut closed = false;
        while index < bytes.len() {
            match bytes[index] {
                b'"' => {
                    closed = true;
                    index += 1;
                    break;
                }
                b'\\' => {
                    index += 1;
                    let Some(escaped) = bytes.get(index) else {
                        break;
                    };
                    if !matches!(escaped, b'"' | b'\\') {
                        return Err("supports only \\\" and \\\\ escapes".to_owned());
                    }
                    item.push(char::from(*escaped));
                    index += 1;
                }
                byte if byte.is_ascii() => {
                    item.push(char::from(byte));
                    index += 1;
                }
                _ => return Err("must contain ASCII identifiers".to_owned()),
            }
        }
        if !closed {
            return Err("contains an unterminated string".to_owned());
        }
        output.push(item);
        while bytes.get(index).is_some_and(u8::is_ascii_whitespace) {
            index += 1;
        }
        if index == bytes.len() {
            break;
        }
        if !allow_multiple || bytes[index] != b',' {
            return Err("must contain only comma-separated quoted strings".to_owned());
        }
        index += 1;
        while bytes.get(index).is_some_and(u8::is_ascii_whitespace) {
            index += 1;
        }
        if index == bytes.len() {
            break;
        }
    }
    Ok(output)
}

fn is_identifier(value: &str) -> bool {
    let mut bytes = value.bytes();
    bytes
        .next()
        .is_some_and(|byte| byte == b'_' || byte.is_ascii_alphabetic())
        && bytes.all(|byte| byte == b'_' || byte.is_ascii_alphanumeric())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::{PolicyError, load_project_policy};

    #[test]
    fn loads_multiline_allowed_functions_deterministically() {
        let root = TempDir::new().expect("root");
        fs::write(
            root.path().join("normfix.toml"),
            "[project]\nname = \"get_next_line\"\nallowed = [\n  \"read\", # subject\n  \"malloc\",\n  \"free\",\n]\n",
        )
        .expect("policy");

        let policy = load_project_policy(root.path())
            .expect("valid")
            .expect("present");
        assert_eq!(policy.name.as_deref(), Some("get_next_line"));
        assert_eq!(
            policy.allowed_functions.into_iter().collect::<Vec<_>>(),
            ["free", "malloc", "read"]
        );
    }

    #[test]
    fn malformed_or_duplicate_allowlists_fail_closed() {
        let root = TempDir::new().expect("root");
        fs::write(
            root.path().join("normfix.toml"),
            "[project]\nallowed = [\"read\", \"read\"]\n",
        )
        .expect("policy");
        assert!(matches!(
            load_project_policy(root.path()),
            Err(PolicyError::Invalid { .. })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn refuses_a_symlinked_policy_instead_of_reading_outside_the_project() {
        use std::os::unix::fs::symlink;

        let root = TempDir::new().expect("root");
        let outside = TempDir::new().expect("outside");
        let target = outside.path().join("outside.toml");
        fs::write(&target, "[project]\nallowed = [\"read\"]\n").expect("outside policy");
        symlink(&target, root.path().join("normfix.toml")).expect("policy symlink");

        assert!(matches!(
            load_project_policy(root.path()),
            Err(PolicyError::Invalid { .. })
        ));
    }
}
