//! Locale selection and the human-message catalogue for the native command line.
//!
//! Translation covers explanations, never identifiers. Command and flag
//! spellings, rule IDs, JSON keys and values, exit codes, configuration keys,
//! and filenames stay language-neutral in every locale, so a script never has
//! to select English to remain reliable.

mod articles;
mod diagnostics;
mod messages;

pub use articles::{ARTICLE_KEYS, Article, ArticleKey, article, article_key};
pub use diagnostics::{DIAGNOSTIC_KEYS, DiagnosticKey, DiagnosticText, diagnostic_text};
pub use messages::{Messages, messages};

/// A published human-output language.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Locale {
    /// English, and the fallback for anything unpublished.
    #[default]
    English,
    /// Portuguese.
    Portuguese,
    /// Spanish.
    Spanish,
    /// French.
    French,
}

/// Every locale this build publishes, in stable order.
pub const PUBLISHED: &[Locale] = &[
    Locale::English,
    Locale::Portuguese,
    Locale::Spanish,
    Locale::French,
];

impl Locale {
    /// Returns the stable lowercase language code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::English => "en",
            Self::Portuguese => "pt",
            Self::Spanish => "es",
            Self::French => "fr",
        }
    }

    /// Parses a language tag such as `pt`, `pt-BR`, or `pt_BR.UTF-8`.
    ///
    /// Only the primary subtag selects a locale: this project publishes one
    /// translation per language, so a region or encoding suffix must not turn
    /// a supported language into an unsupported one.
    #[must_use]
    pub fn from_tag(tag: &str) -> Option<Self> {
        let primary = tag
            .trim()
            .split(['-', '_', '.', '@', ':'])
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        PUBLISHED
            .iter()
            .copied()
            .find(|locale| locale.code() == primary)
    }

    /// Returns the catalogue for this locale.
    #[must_use]
    pub fn messages(self) -> &'static Messages {
        messages(self)
    }
}

/// The selected locale plus anything the user should be told about the choice.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Resolution {
    /// Locale used to render human output.
    pub locale: Locale,
    /// One concise advisory when a requested language could not be honored.
    pub advisory: Option<String>,
}

/// Selects a locale from an explicit request, then the process environment.
///
/// `environment` is consulted in the documented order and must behave like
/// [`std::env::var`] for a single variable name. An unpublished or malformed
/// request falls back to English with one advisory rather than failing: output
/// language is never a reason to refuse to analyze a project.
pub fn resolve<F>(explicit: Option<&str>, environment: F) -> Resolution
where
    F: Fn(&str) -> Option<String>,
{
    if let Some(requested) = explicit.map(str::trim).filter(|value| !value.is_empty()) {
        return match Locale::from_tag(requested) {
            Some(locale) => Resolution {
                locale,
                advisory: None,
            },
            None => Resolution {
                locale: Locale::English,
                advisory: Some(unpublished_advisory(requested)),
            },
        };
    }

    // An explicit request is a decision; the environment is only a hint, so an
    // unpublished process locale falls back silently.
    for variable in ["NORMFIX_LANG", "LC_ALL", "LC_MESSAGES", "LANG"] {
        let Some(value) = environment(variable) else {
            continue;
        };
        let value = value.trim();
        if value.is_empty() {
            continue;
        }
        if let Some(locale) = Locale::from_tag(value) {
            return Resolution {
                locale,
                advisory: None,
            };
        }
        return Resolution {
            locale: Locale::English,
            advisory: None,
        };
    }

    Resolution {
        locale: Locale::English,
        advisory: None,
    }
}

fn unpublished_advisory(requested: &str) -> String {
    let published = PUBLISHED
        .iter()
        .map(|locale| locale.code())
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "language `{}` is not published; continuing in English (available: {published})",
        requested.escape_debug()
    )
}

/// Picks the wording a count needs, then fills it.
///
/// Writing `file(s)` avoids the question rather than answering it, and a
/// sentence that reads "reported 1 memory errors" is simply wrong in every
/// language here. Which form a number takes is a property of the language, so
/// it belongs beside the language rather than in each call site.
///
/// French counts zero as singular; English, Portuguese, and Spanish do not.
/// That is the whole difference between these four, and it is the reason this
/// takes a locale rather than testing `count == 1` at the call site.
#[must_use]
pub fn fill_plural(
    locale: Locale,
    count: u64,
    one: &str,
    other: &str,
    arguments: &[(&str, &str)],
) -> String {
    let singular = match locale {
        Locale::French => count <= 1,
        Locale::English | Locale::Portuguese | Locale::Spanish => count == 1,
    };
    fill(if singular { one } else { other }, arguments)
}

/// Replaces `{name}` placeholders in a catalogue entry.
///
/// Translations are reviewed against the English entry's placeholder set, so a
/// name missing from `arguments` is a catalogue defect rather than user input.
/// It is left visible instead of silently producing a sentence with a hole.
#[must_use]
pub fn fill(template: &str, arguments: &[(&str, &str)]) -> String {
    let mut output = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        let Some(length) = rest[start..].find('}') else {
            break;
        };
        let end = start + length;
        let name = &rest[start + 1..end];
        output.push_str(&rest[..start]);
        match arguments
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| *value)
        {
            Some(value) => output.push_str(value),
            None => output.push_str(&rest[start..=end]),
        }
        rest = &rest[end + 1..];
    }
    output.push_str(rest);
    output
}

#[cfg(test)]
mod tests {
    use super::{Locale, PUBLISHED, Resolution, fill, resolve};

    fn none(_: &str) -> Option<String> {
        None
    }

    #[test]
    fn a_region_or_encoding_suffix_still_selects_the_language() {
        assert_eq!(Locale::from_tag("pt_BR.UTF-8"), Some(Locale::Portuguese));
        assert_eq!(Locale::from_tag("pt-BR"), Some(Locale::Portuguese));
        assert_eq!(Locale::from_tag("FR"), Some(Locale::French));
        assert_eq!(Locale::from_tag("es@valencia"), Some(Locale::Spanish));
        assert_eq!(Locale::from_tag("C"), None);
        assert_eq!(Locale::from_tag(""), None);
    }

    #[test]
    fn an_explicit_request_wins_over_the_environment() {
        let resolution = resolve(Some("fr"), |name| {
            (name == "LANG").then(|| "pt_BR.UTF-8".to_owned())
        });
        assert_eq!(
            resolution,
            Resolution {
                locale: Locale::French,
                advisory: None
            }
        );
    }

    #[test]
    fn an_unpublished_explicit_request_falls_back_with_one_advisory() {
        let resolution = resolve(Some("de"), none);
        assert_eq!(resolution.locale, Locale::English);
        let advisory = resolution.advisory.expect("advisory for an unknown locale");
        assert!(advisory.contains("`de`"));
        assert!(advisory.contains("en, pt, es, fr"));
    }

    #[test]
    fn an_unpublished_process_locale_is_english_without_nagging() {
        let resolution = resolve(None, |name| {
            (name == "LANG").then(|| "de_DE.UTF-8".to_owned())
        });
        assert_eq!(
            resolution,
            Resolution {
                locale: Locale::English,
                advisory: None
            }
        );
    }

    #[test]
    fn environment_variables_are_consulted_in_the_documented_order() {
        let resolution = resolve(None, |name| match name {
            "LC_ALL" => Some("es_ES.UTF-8".to_owned()),
            "LANG" => Some("pt_BR.UTF-8".to_owned()),
            _ => None,
        });
        assert_eq!(resolution.locale, Locale::Spanish);

        let overridden = resolve(None, |name| match name {
            "NORMFIX_LANG" => Some("fr".to_owned()),
            "LC_ALL" => Some("es_ES.UTF-8".to_owned()),
            _ => None,
        });
        assert_eq!(overridden.locale, Locale::French);
    }

    #[test]
    fn an_empty_variable_is_skipped_rather_than_treated_as_a_choice() {
        let resolution = resolve(None, |name| match name {
            "LC_ALL" => Some("   ".to_owned()),
            "LANG" => Some("pt".to_owned()),
            _ => None,
        });
        assert_eq!(resolution.locale, Locale::Portuguese);
    }

    #[test]
    fn placeholders_are_replaced_and_unknown_names_stay_visible() {
        assert_eq!(fill("{a} and {b}", &[("a", "1"), ("b", "2")]), "1 and 2");
        assert_eq!(fill("{a} and {b}", &[("a", "1")]), "1 and {b}");
        assert_eq!(fill("no placeholder", &[("a", "1")]), "no placeholder");
        assert_eq!(fill("unclosed {a", &[("a", "1")]), "unclosed {a");
    }

    #[test]
    fn every_published_locale_has_a_distinct_code() {
        let mut codes = PUBLISHED
            .iter()
            .map(|locale| locale.code())
            .collect::<Vec<_>>();
        codes.sort_unstable();
        let total = codes.len();
        codes.dedup();
        assert_eq!(codes.len(), total);
    }
}
