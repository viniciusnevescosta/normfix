//! Short, offline explanations for the rules most often requiring review.
//!
//! The articles themselves live in `normfix-i18n`, because a rule identifier is
//! language-neutral while its explanation is not. This module only validates the
//! identifier and renders the answer in the reader's language.

use normfix_i18n::{Locale, Messages};

/// Returns the bundled explanation for `rule`, rendered in `locale`.
///
/// An identifier that is not shaped like a rule ID returns `None` so the caller
/// can report a usage error rather than print an article about nothing.
pub(crate) fn explain(rule: &str, locale: Locale, messages: &Messages) -> Option<String> {
    let canonical = rule.trim().to_ascii_uppercase();
    if canonical.is_empty()
        || !canonical.chars().all(|character| {
            character.is_ascii_uppercase() || character.is_ascii_digit() || character == '_'
        })
    {
        return None;
    }
    let article = normfix_i18n::article(locale, normfix_i18n::article_key(&canonical));
    Some(format!(
        "{canonical}: {}\n\n{}\n  {}\n\n{}\n  {}\n\n{}\n  {}\n",
        article.title,
        messages.explain_why,
        article.why,
        messages.explain_next,
        article.next,
        messages.explain_safety,
        article.safety,
    ))
}

#[cfg(test)]
mod tests {
    use normfix_i18n::{Locale, messages};

    use super::explain;

    #[test]
    fn explanations_are_case_insensitive_and_unknown_rules_stay_safe() {
        let english = messages(Locale::English);
        let text = explain("line_too_long", Locale::English, english).expect("known rule");
        assert!(text.starts_with("LINE_TOO_LONG"));
        assert!(text.contains("80 display columns"));

        let fallback =
            explain("NOT_A_REAL_RULE", Locale::English, english).expect("safe generic explanation");
        assert!(fallback.contains("No dedicated long-form article"));

        assert!(explain("not a rule", Locale::English, english).is_none());
    }

    #[test]
    fn an_explanation_follows_the_reader_language_but_keeps_the_rule_id() {
        let portuguese = explain(
            "TOO_MANY_LINES",
            Locale::Portuguese,
            messages(Locale::Portuguese),
        )
        .expect("known rule");

        // The identifier is an API token and is never translated.
        assert!(portuguese.starts_with("TOO_MANY_LINES:"));
        assert!(portuguese.contains("Por quê"));
        assert!(portuguese.contains("25 linhas"));
    }
}
