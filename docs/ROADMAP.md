# Release roadmap

The roadmap separates changes by compatibility promise. Dates are deliberately
absent: a release moves only after its tests, real-project checks, native
archives, and browser build are green.

## 1.0 release candidates

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

### 1.0.0-rc.3 — browser and documentation

- Monaco-based multi-file playground for C, headers, Makefiles, and README
  documents;
- local-only 42 identity support for official headers;
- English, Portuguese, Spanish, and French navigation/content foundations;
- GitHub project metadata, official dependency links, examples, and SEO;
- a dependency tree with no known npm advisory in the shipped lockfile.

### 1.0.0 — stable contract

- no new feature after the final candidate;
- complete native, MSRV, Clippy, rustdoc, WASM, site, security, and
  real-project gates;
- immutable four-platform archives and checksum/provenance publication;
- verified installer and Homebrew formula update.

The first code change after the 1.0.0 tag is splitting
`crates/normfix-engine/src/pipeline.rs` into smaller internal modules. It is
intentionally deferred so the release-candidate behavior is not obscured by a
large structural diff.

## 1.1 — localized terminal experience

Translate human diagnostics and help into Portuguese, Spanish, and French.
Command and flag spellings remain stable English API tokens. JSON keys, rule
IDs, and exit codes stay language-neutral.

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
