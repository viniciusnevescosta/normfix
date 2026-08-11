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

## 1.1 — explanations in your language, and a readable engine

Shipped. `normfix explain` answers in English, Portuguese, Spanish, or French,
and `crates/normfix-engine/src/pipeline.rs` is split into focused modules — the
change that was deferred through all three release candidates.

## 1.2 — the playground as a workspace

Make the browser playground usable for a whole project rather than one file:

- drag and drop supported files onto the page, and drop a complete project
  folder, with the interface showing that a drop is being received;
- one button to fix every open file, and one to fix only the current file;
- download everything as `.zip`, which is the format every operating system
  opens without a extra tool.

## 1.3 — project initialization

A guided `normfix init` that creates a project from explicit choices: name,
allowed function policy, `main.c`, header, Makefile, `README.md` carrying the
student's name, `src/`, `tests/`, and an initialized Git repository.

## 1.4 — platform hardening

Bug fixes, vulnerability work, measured performance changes, and native support
for Windows and BSD on x86-64 and ARM64. Support means filesystem, terminal,
compiler, archive, installer, and CI behavior; producing a binary alone does not
count.

## 1.5 — leak checking

Add Valgrind support so a run can report leaks in the code, within the same rule
as every other backend: what it cannot prove, it reports rather than asserts.

## 1.6 — Python projects

Add a separate Python policy/formatting pipeline around mypy and flake8, plus a
Python-capable playground. The C/Norminette contract remains available and
versioned instead of being silently generalized.
