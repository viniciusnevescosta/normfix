# Release roadmap

The roadmap separates changes by compatibility promise. Dates are deliberately
absent: a release moves only after its tests, real-project checks, native
archives, and browser build are green.

`1.0.0` is released. The command surface, rule IDs, exit codes, and the
`schema_version: 2` JSON report are stable for all of 1.x.

## Shipped: the 1.0 line

### 1.0.0-rc.2 — evaluator and safe automation

- persist a validated 42 identity in user-owned configuration;
- state the selected action, scope, and effective safety configuration before
  a project run;
- refuse broad or sensitive filesystem roots unless explicitly forced;
- make untested official Norminette releases usable by default with a visible
  compatibility warning;
- strengthen `preflight` with project consistency, Makefile, README, compiler,
  analyzer, sanitizer, and non-conclusive evaluation guidance;
- publish the operational contract for AI agents.

### 1.0.0-rc.3 — terminal language and release ergonomics

The browser and documentation work originally planned here shipped early, in
rc.2: the Monaco multi-file playground, local-only 42 identity, the four-locale
navigation foundations, dependency links and SEO, and an advisory-free
lockfile are all published. This candidate carries what took their place.

- a locale layer whose catalogue cannot be incomplete, because each language is
  one struct literal and a missing translation fails the build;
- language selection from `--lang` or the process locale, falling back to
  English without ever making output language a reason to refuse a run;
- the run announcement, the report's own prose, and every safety-critical
  prompt in English, Portuguese, Spanish, and French;
- JSON that stays language-neutral in every locale;
- release notes that open with the commands that install that exact version.

### 1.0.0 — stable contract

Released with the complete documentation in English, Portuguese, Spanish, and
French, and with `normfix uninstall` so the tool can be removed as cleanly as
it is installed.

The first code change after the 1.0.0 tag is splitting
`crates/normfix-engine/src/pipeline.rs` into smaller internal modules. It is
intentionally deferred so the release-candidate behavior is not obscured by a
large structural diff.

## 1.1 — complete the terminal translation

rc.3 localized the text normfix itself authors. This line finishes the job:
the `explain` catalogue, command-line help, and the rule messages produced by
the analysis backends. Until then a non-English run says plainly that backend
messages are still English, because a localized frame that implies otherwise is
worse than an English one.

Command and flag spellings remain stable English API tokens, as does the `y`
answer to a confirmation prompt. JSON keys, values, rule IDs, and exit codes
stay language-neutral.

## 1.2 — project initialization

Add a guided `normfix init` workflow that can create a project name, allowed
function policy, `main.c`, header, Makefile, README, `src/`, and `tests/` from
explicit user choices and the stored 42 identity.

## 1.3 — platform hardening

Reserve this line for bug fixes, security work, measured performance changes,
and native Windows x86-64/ARM64 support. Windows support includes filesystem,
terminal, compiler, archive, installer, and CI behavior; producing a binary
alone does not count as support.

## 2.0 — Python projects

Add a separate Python policy/formatting pipeline around mypy and flake8, plus a
Python-capable playground. The C/Norminette contract remains available and
versioned instead of being silently generalized.
