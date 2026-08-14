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

Shipped. `normfix leaks` reports leaks in a program you already built, within the
same rule as every other backend: what it cannot prove, it reports rather than
asserts. It is the only command that executes your code, so it asks first, and
normfix never builds the program — you build it, normfix runs it. `preflight`
says whether a checker is installed and what to type, without running anything. A clean report is not a proof that a program never leaks — it is what
one run observed on one path.

The checker is located on `PATH` and verified by its own `--version`, so which
build answers is not normfix's business. Linux and FreeBSD have it. macOS works
through [`LouisBrunner/valgrind-macos`](https://github.com/LouisBrunner/valgrind-macos),
whose Apple Silicon support is limited — worth saying, since most 42 Macs are
now ARM. Windows is answered with WSL rather than with a second tool: Dr. Memory
would mean a different output format, a second version to pin, a second proof in
CI, and findings that cannot be compared with Valgrind's, on a platform whose
documented path for the full CLI already runs through WSL.

## 1.6 — correctness, security, and speed

Shipped. A pass over the whole project for bugs, vulnerabilities, and measured
performance changes.

Most of it came from running the same files through the terminal and through the
checkerless browser path and comparing the bytes, which found what reading the
code had not: layout rules that only fired where the official checker had
already pointed, and so did nothing at all in the playground; a brace written as
`){` that was never moved; nested braceless bodies indented too shallowly; a
README whose checkboxes and footnotes were escaped away; a byte-order mark that
ended up in the middle of the file. A run also reads a file once to plan and
once per accepted batch to prove, where it used to read it twice per pass and
discard the proving read — a clean file is 45% faster than in 1.5.0, and a test
holds that budget by counting reads rather than timing them.

### 1.6.1 — the edits and the surfaces around them

Splitting a declaration from its assignment: `int teste = 10;` becomes
`int teste;` and an assignment after the declaration block. The official checker
already calls this `DECL_ASSIGN_LINE`, and normfix has only ever reported it.
Four shapes are excluded because the split would change the program: `const`
and aggregate initialisers cannot be assigned later, `static` initialises once
where an assignment would rerun, and a file-scope declaration has nowhere to put
one.

`normfix leaks` names the line an allocation came from. Valgrind already emits a
stack trace per loss record; normfix reads only the totals and discards the
rest. A binary built without debug information has no line to name, and that is
said rather than left silent.

The playground gains folders that nest, a `.zip` download that keeps their
structure, a warning when an uploaded folder carries files it does not handle,
and file creation that treats `.md`, `.h`, and `Makefile` as the first-class
choices they are. It links the latest release so a reader can get the full tool,
and says that the site installs and runs offline where the browser supports it.
Diagnostics are shown inline, the way an editor underlines them.

None of it at the cost of what 1.6 measured: the parse budget, the differential
proof, and the parity between the two runs all still hold.

### 1.6.2 — the JSON output is an API

Every command that emits JSON is read primarily by something that is not a
person: a script in CI, or an agent deciding what to do next. That makes the
JSON an interface with the obligations of one. It has to describe every
scenario, not only the ones a human would have read the prose for — an
operational failure, a refused capability, a file that could not be parsed, a
check that did not run — and it has to say which of those happened rather than
leaving a consumer to infer it from an absent field.

`--format json` also has to mean the same thing everywhere: every command, and
every flag, rather than the subset that happens to have been written for a
person watching a terminal. Running each command and reading what came back
finds three gaps.

`upgrade --check` answers in English prose whatever the format, so nothing can
read whether an update exists. `uninstall --dry-run` writes nothing at all to
standard output and describes what it would remove on standard error, which is
the stream a caller is least likely to be parsing. `undo --list` answers with a
bare `[]`, carrying no envelope and no `schema_version`, so "no recovery points"
cannot be told apart from "this build did not understand the request".

The commands that already answer properly — `format`, `lint`, `check`,
`budget`, `preflight`, `explain`, and `leaks` — are the shape the rest have to
reach: one object on standard output, naming the schema, the command, and
whether it succeeded, before anything of its own.

Because this release is about being read by something that is not a person, the
documentation also ships a plain-text copy of every page, so an agent can fetch
one page's instructions without parsing the site around them.

`schema_version` already exists and is honoured. What follows it is coverage:
each command's payload documented as the contract it is, so nothing has to be
learned by running the tool and reading what came back.

### 1.6.3 — say it in language people actually understand

A translation and documentation pass across the whole project, English
included. The root problem is that the translations are literal: each sentence
was carried across word by word, so it is grammatical and says nothing. A reader
learning from it has to translate it back before it means anything, which is the
opposite of why it was translated. On top of that, some strings use terms nobody
outside the project knows, and some contradict each other — three were found and
fixed during 1.4 and 1.5 alone, each one a sentence that had been true one
release earlier.

Calling Homebrew and Scoop "conveniences" goes too: it reads as dismissing the
way many people will actually install the tool. They are supported ways to
install it, and the one-line installer is the one that works everywhere.

So the rewrite is not a correction pass over the existing sentences. Each page is
rewritten to teach: what the reader is trying to do, what the tool does about
it, and what they should do next — then written again in each language as
someone would actually say it there, rather than translated. The point of
translating was accessibility, and a sentence a reader has to decode is not
accessible in any language.

The plain-text copy of every page ships in 1.6.2, alongside the JSON work,
since both exist for the same reader.

## 1.7 — Python projects

A separate Python pipeline on the same oracle model the Norminette uses, plus a
Python-capable playground. The C/Norminette contract stays available and
versioned instead of being silently generalized.

What matters is the result a student needs: strict type checking and lint
findings they can act on. mypy `--strict` and flake8 are the reference for that
result, and the decision of which tools produce it is open — Astral's `ruff` and
`ty` may reach the same answers faster, and being faster matters for a tool a
student runs before every push. Whether either can be embedded, so that nothing
has to be installed, is part of that comparison. The choice will be made by what
each reports on real 42 Python projects, and whichever is chosen becomes the
versioned oracle the way Norminette 3.3.59 is.

## 1.8 — starting a project

Create a project from explicit choices: its name and allowed function list, then
`main.c`, the header, the Makefile, a `README.md` carrying the student's login,
`src/`, `tests/`, and an initialized Git repository. For C and for Python.
