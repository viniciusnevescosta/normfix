# Localization guide

This is the contributor contract for translating every human-facing normfix
surface. The first published locales are English (`en`), Portuguese
(`pt`), Spanish (`es`), and French (`fr`). A locale is complete only when a
reader can install the tool, understand its safety boundary, use the browser
playground, and follow the core documentation without falling back to English
UI text.

Localization must not change the machine interface. Translate explanations,
not identifiers.

## What stays in English

These values are stable API or source-code tokens and must remain unchanged in
every locale:

- the `normfix` command and subcommand names;
- flags such as `--check`, `--changed`, and `--format json`;
- rule IDs such as `TOO_MANY_LINES` and `MAKEFILE_SOURCE_NOT_FOUND`;
- JSON keys, enum values, `schema_version`, and exit codes;
- configuration keys, environment-variable names, and filenames;
- C identifiers, shell commands, paths, archive names, and code examples;
- Git commit messages and Rust/TypeScript source comments.

Keep official product names—Norminette, Rust, WSL, Clang, Vite, Monaco, Git,
GitHub, and Vercel—unchanged. Translate the sentence around them and retain the
official link.

## Current surfaces

| Surface | Source of translated text | Published behavior |
|---|---|---|
| Browser playground | `web/i18n.ts` and `data-i18n*` attributes in `web/index.html` | Complete `en`, `pt`, `es`, and `fr` UI; native rule diagnostics remain English until the CLI catalogue is localized |
| Documentation | Locale trees under `docs/` plus locale navigation in `docs/.vitepress/config.ts` | Localized landing, installation, playground, safety, compatibility, and contributor paths, with English as the explicit fallback for pages not yet published |
| SEO | VitePress head/config, `web/index.html`, sitemaps, and `robots.txt` | Canonical URLs and `hreflang` only for pages that really exist |
| Native CLI | Rust human-message catalogue planned for 1.1 | Commands, flags, JSON, rule IDs, and exit codes remain language-neutral |

## Translating the playground

1. Add the locale code to `SUPPORTED_LOCALES` in `web/i18n.ts`.
2. Supply every `MessageKey`. Do not publish a locale that silently inherits
   an English button, validation error, privacy statement, or accessibility
   label.
3. Put static HTML text behind `data-i18n`, `data-i18n-title`,
   `data-i18n-placeholder`, or `data-i18n-aria`. Put dynamic text behind
   `translate()`; do not leave
   user-facing string literals in `web/app.ts` or `web/editor.ts`.
4. Use named placeholders such as `{path}` and `{count}`. Every translation
   must preserve the same placeholder set as English.
5. Format numbers and dates with the selected locale. Do not localize the
   fixed 42-header timestamp or other protocol text.
6. Set the document `lang` value and provide a visible language selector.
7. Never inject a translation with `innerHTML`. Continue to use `textContent`
   and DOM nodes so translated or source-controlled text cannot become markup.
8. Test the narrow-screen textarea fallback as well as Monaco. Monaco itself
   does not define the product's localization completeness.

Native Rust diagnostics returned by WebAssembly currently stay in English.
The UI must say this plainly instead of presenting a partially translated
diagnostic as complete localization.

## Translating documentation

Use the English page as the source of truth. Preserve headings that are link
targets unless the locale config also provides a tested redirect. Keep command
examples byte-for-byte valid; translate their surrounding prose and expected
human output only.

For a new locale:

1. create its locale directory and translate the landing page;
2. translate getting started, the browser-playground guide, safety/recovery,
   compatibility, and this localization guide before advertising the locale;
3. add localized VitePress labels, navigation, sidebar, search labels, footer,
   and edit-link text;
4. link the official Norminette, Rust, WSL, and Clang pages wherever those
   tools are named as dependencies;
5. add canonical and alternate-language metadata only between equivalent
   translated pages;
6. include every published localized URL in the generated sitemap;
7. verify every internal link and code fence in the production build.

Do not create a thin page whose only content is an automatic redirect to
English and call it a translation. An explicit “This page is available in
English” link is an acceptable temporary fallback when the localized route is
not advertised as complete.

## Translating the native CLI (1.1)

The future CLI catalogue must keep diagnostic data separate from rendering.
Each human message should have a stable internal key, English fallback, typed
placeholders, and translations in the same review. JSON serialization must
continue to emit language-neutral fields and values; scripts should never need
to select English to remain reliable.

Locale selection should follow an explicit CLI/config choice first, then the
standard process locale, then English. Unsupported or incomplete locale data
must fall back to English with one concise advisory, never a panic. Snapshot
tests should cover help, a clean run, a blocking diagnostic, an operational
failure, the pre-defense estimate, and destructive confirmation in every
published locale.

## Terminology and tone

- Use the vocabulary students already see at 42.
- Prefer short, direct sentences in warnings and buttons.
- Keep the distinction between **warning**, **failure**, **unsafe**,
  **recoverable**, **advisory**, and **conclusive** precise.
- Do not translate “safe” as “guaranteed correct.” It means the documented
  proof for that edit passed.
- Do not translate the pre-defense estimate as an official grade.
- Preserve the statement that browser identity is device-local configuration,
  not an encrypted secret.

When a term is disputed, update a small glossary in the locale's contributor
notes and use one spelling consistently across playground and docs.

## Validation

Run the complete site checks after any localization change:

```sh
npm ci
npm run typecheck
npm audit --audit-level=moderate
npm run build
```

Then review each locale at desktop and narrow widths. Check keyboard access,
focus labels, text overflow, plural/count wording, code-copy behavior, broken
links, canonical URLs, `hreflang`, and the sitemap. A reviewer fluent in the
target language should approve meaning and tone; a passing TypeScript build
only proves catalogue shape.

For a CLI catalogue change, also run the Rust workspace tests, Clippy with
warnings denied, rustdoc with warnings denied, and the JSON-schema fixtures.

## Pull-request checklist

- [ ] Every new human-facing string is in the correct catalogue.
- [ ] Commands, flags, rule IDs, JSON keys, and code samples are unchanged.
- [ ] Placeholder names and safety meaning match English.
- [ ] Navigation, accessibility labels, metadata, and error paths are translated.
- [ ] Canonical, `hreflang`, and sitemap entries point only to real pages.
- [ ] Official dependency links are retained.
- [ ] Site and Rust gates relevant to the change pass.
- [ ] A fluent reviewer checked the rendered result, not only the source diff.
