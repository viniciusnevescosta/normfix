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

## 1.2 — normfix's own findings in your language

Shipped. A student running with `--lang pt`, or simply with a Portuguese system
locale, reads the findings normfix writes itself in Portuguese. Findings relayed
from the official Norminette or the C compiler stay in the language those tools
produced them, and a non-English run says so as a fact rather than as an
apology.

## 1.3 — the playground as a workspace

Shipped, across three releases:

- 1.3.0 made the playground work with no network and installable as an app,
  added drag and drop for files and whole project folders, gave it one button to
  fix every file and one to fix only the current file, and changed the download
  to `.zip`;
- 1.3.1 added light, dark, and system appearance;
- 1.3.2 took the star count, the stylesheet, and the WebAssembly discovery off
  the critical path, raised every control to a size a finger can hit, and fixed
  two accessibility defects on the documentation site.

## 1.4 — Windows and BSD

Shipped. Native Windows on x86-64 and ARM64, and FreeBSD on x86-64. Support
means what the roadmap always said it meant: every platform runs the complete
suite, drives the real official Norminette, and proves the differential property
on the platform itself, in CI.

Four things had to be fixed before any of that was true, and none of them
announced itself — the protected-scope guard recognized only Unix path shapes,
the transaction flushed a directory in a way Windows refuses, the backup
directory resolved through variables Windows does not set, and the process bound
bounded one process instead of a tree.

FreeBSD on ARM64 is not published: `aarch64-unknown-freebsd` has no prebuilt
standard library on the pinned toolchain, and there is no runner to execute a
suite on. Publishing it would be the binary-without-evidence this roadmap warns
against.

## 1.5 — leak checking

Add Valgrind support so a run can report leaks in the student's code, within the
same rule as every other backend: what it cannot prove, it reports rather than
asserts.

## 1.6 — Python projects

A separate Python pipeline on the same oracle model the Norminette uses, plus a
Python-capable playground. The C/Norminette contract stays available and
versioned instead of being silently generalized.

What matters is the result a student needs: strict type checking and lint
findings they can act on. mypy `--strict` and flake8 are the reference for that
result, and the decision of which tools produce it is open — Astral's `ruff` and
`ty` may reach the same answers faster, and being faster matters for a tool a
student runs before every push. The choice will be made by comparing what each
reports on real 42 Python projects, not by reputation, and whichever is chosen
becomes the versioned oracle the way Norminette 3.3.59 is.

## 1.7 — starting a project

Create a project from explicit choices: its name and allowed function list, then
`main.c`, the header, the Makefile, a `README.md` carrying the student's login,
`src/`, `tests/`, and an initialized Git repository. For C and for Python.

## 1.8 — correctness, security, and speed

A pass over the whole project for bugs, vulnerabilities, and measured
performance changes.

### 1.8.1 — say it in language people actually understand

A translation and documentation pass across the whole project, English
included. Some strings do not read as the language they claim to be, some use
terms nobody outside the project knows, and some contradict each other. The
documentation is rewritten alongside them to be clearer and easier to follow —
the point of translating was accessibility, and a sentence a reader has to
decode is not accessible in any language.
