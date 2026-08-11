# Release notes

Every published version of `normfix`, newest first. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html): a hyphenated
version such as `0.4.0-beta.1` is a pre-release and is never published as the
latest stable download.

Published archives, checksums, and build provenance live on the
[releases page](https://github.com/viniciusnevescosta/normfix/releases).
`docs/RELEASING.md` describes how a release is produced.

## [1.1.1] / 2026-08-11

### Fixed

- **Preflight stopped implying that a piscina exercise is missing something.**
  A piscina exercise is expected to contain only `.c` files, so a Makefile and
  project headers are both optional there. The score was already correct —
  `MAKEFILE_NOT_FOUND` is informational and the deduction only counts
  severities above Info — but the advisory read as a to-do list: "add or select
  the required Makefile before relying on preflight."

  It now says what is true: the check did not run, that is normal for loose
  `.c` files, absence is never a hard fail and costs no score, and only the
  subject can say whether a Makefile is required, which normfix cannot read. A
  regression test locks it: loose `.c` files with no Makefile and no header
  score 100 with no hard failures.

### Changed

- Dependency updates: blake3 1.8.6, clap 4.6.6, criterion 0.7.0, and
  `actions/setup-python` v7. Criterion 0.8 is deliberately not taken because it
  requires Rust 1.86 and the supported minimum is 1.85.

### Notes

The differential gate was re-run against the installed official Norminette for
this release: a run still never leaves a file with more official diagnostics
than it started with, a second run changes nothing, and a file that compiled
still compiles.

## [1.1.0] / 2026-08-11

Two things: `normfix explain` now answers in your language, and the engine's
largest module is finally readable.

### Added

- **The `explain` catalogue in four languages.** This was the largest remaining
  English surface: you could run the entire tool in Portuguese and still get a
  four-paragraph article in English the moment you asked what a rule meant. All
  twenty-five articles now exist in English, Portuguese, Spanish, and French.

  The rule identifier stays language-neutral and its mapping to an article
  happens once, shared by every locale, so a translation can change the words
  but never which rule an explanation belongs to. Each locale matches
  exhaustively on the article key, which makes an untranslated article a build
  failure rather than an English paragraph inside a translated answer. A second
  test catches the copy-paste that compiles perfectly: a translated article that
  still carries the English explanation.

### Changed

- **`pipeline.rs` is split into focused modules.** This is the change deferred
  through all three release candidates, so that release-candidate behavior was
  never obscured by a large structural diff. The file went from 4882 lines to
  2210, and eight modules now carry the rest: the path vocabulary, the
  recoverable quarantine, the compiler and analyzer boundary, Makefile handling,
  README handling, diagnostic construction, the function policy, and the
  preflight advisories.

  Each module took its tests with it, which is most of the point. A test for
  recovery-storage overlap now sits beside the function that creates that
  storage, and the two column-convention tests sit beside the code that
  reconciles display columns with byte columns, instead of four thousand lines
  away.

  Behavior is unchanged: the public API is identical and the differential gate
  still holds.

### Fixed

- **The playground star count was frozen.** It used `cache: "force-cache"`,
  which serves a stored response regardless of its age. GitHub sends
  `max-age=60` with an ETag, and force-cache ignores both, so whatever count a
  browser saw first was the count it kept showing — a star added hours earlier
  never appeared. The default cache mode respects that freshness window and then
  revalidates conditionally, which is both fresher and cheaper than refetching.

- **No documentation link changes your language any more.** Clicking Architecture
  from the Portuguese site loaded the English page and took the whole site with
  it: navigation, language selector, and every subsequent link. Six documents are
  maintained in English — the architecture record, the release process, the
  roadmap, the changelog, and the two files GitHub expects at the repository
  root — and each is now mirrored into every locale at build time, so the reader
  stays on their own route with a banner in their language saying the body below
  is English.

- **The site remembers the language you picked**, sharing the playground's
  storage key so a choice made in one carries into the other. The landing route
  follows the stored choice, then the browser languages, then English. A URL that
  names a locale is never overridden, so a shared link opens the page it names.

- **The Homebrew path is documented in both directions.** The release notes
  offered `normfix upgrade`, which refuses on a Homebrew-managed binary, so the
  single instruction shown was the one that would not work for a `brew` user.
  Both commands now appear, and `brew uninstall` is near the top of the
  uninstall page in all four languages.

### Notes

Command-line help and the rule messages produced by the analysis backends are
still English, and a non-English run keeps saying so in one line. Finishing that
is 1.2: it needs a diagnostic to carry a stable key and typed arguments instead
of a rendered English sentence, because the dynamic parts are already
interpolated by the time a renderer sees them. `docs/ROADMAP.md` records the
plan.

## [1.0.0] / 2026-08-10

The stable release.

`normfix` started from one observation: a 42 student's scarcest resource is
hours, and a meaningful share of those hours goes into whitespace. Fixing
indentation. Moving declarations. Breaking lines at 80 columns. Pasting
headers. Across a cursus that is thousands of files, in project after project,
none of which teaches you anything the second time you do it.

This is the version that gives those hours back and is willing to be held to
it.

### What it does

One command, in a project directory:

```sh
normfix
```

It reads the project, applies every fix it can prove is safe, and explains the
rest in a sentence instead of a rule name. No configuration file is required,
nothing is uploaded, and every file it rewrites is backed up outside the project
first.

The proof is not a figure of speech. The installed official Norminette runs
before and after every batch of edits, and if a batch introduces a rule
violation that was not there before, the whole batch is reverted and your
original bytes stay. Edits touch the byte range they proved something about and
nothing else, so the diff is reviewable and the rest of the file is identical.
That is why you can run this on work in progress.

### What 1.0.0 promises

A stable contract, and the restraint that makes it worth something.

- **The command surface is stable.** Command names, flag spellings, rule IDs,
  exit codes, configuration keys, and `--format json` values do not change
  inside 1.x. A script written today keeps working.
- **`schema_version: 2` is the JSON contract.** Branch on it and the rest is
  yours to depend on.
- **Every write is recoverable.** One transaction, external backups, a journal,
  and a `undo` that refuses to restore over work you did afterwards.
- **Destructive means authorized.** Nothing is removed without a capability
  flag and a confirmation, every removal keeps recovery storage, and an
  unproven case is reported rather than guessed.
- **Refusals are the feature.** Extracting a long function, reordering includes
  across a conditional, and guaranteeing 80 columns with no safe break are all
  left to you, with the reason and the next step.

### What is in it

Everything from the three release candidates, and it is worth reading them
together because they are one argument in three parts.

**rc.1 — see the error.** Every diagnostic is shown against its own source,
with carets under the exact bytes the rule is about, rendered with the library
`rustc` uses for its own errors. Shared context is stated once per rule instead
of repeated under every occurrence.

**rc.2 — know whether you would pass.** `preflight` ends with a transparent
0–100 estimate, a letter band, a verdict, and exactly located hard failures —
evaluated against the bytes on disk, not the bytes normfix could write, because
a repair you have not written is not part of what an evaluator will open. Every
run announces its action, scope, and safety configuration before touching
anything, and filesystem roots, home directories, and operating-system trees
are refused outright. Orphan header prototypes and trivia-only Makefile sources
became visible. Untested Norminette releases became usable, with the
compatibility gap named rather than hidden.

**rc.3 — read it in your language.** The run announcement, the report, and every
safety-critical prompt in English, Portuguese, Spanish, and French, from a
catalogue that cannot be half-translated because a missing entry fails the
build. JSON stays language-neutral in every locale, so automation never has to
pick a language.

**And for 1.0.0:** the complete documentation — 25 pages — in all four
languages, and `normfix uninstall`, which prints exactly what it will remove,
keeps your backups unless you name them, and refuses a Homebrew-managed install
with the command that actually works.

### What it will not do

The honest list, unchanged since the first commit, because it is the point of
the tool rather than a gap in it:

- it will not extract a long function for you;
- it will not redesign control flow, rename across a project, or change a public
  signature;
- it will not prove your program is leak-free — the analyzer can suggest a leak,
  never its absence;
- it will not guarantee 80 columns when no safe break exists;
- and it will never present its own estimate as a 42 grade. `conclusive` is
  `false` in every report this tool can produce.

### The rule it is built on

> Change what can be proven, explain what cannot, and never turn uncertainty
> into permission.

Every decision here follows from that sentence, including the ones that make
the tool do less than it could.

### Thanks

To the official [Norminette](https://github.com/42School/norminette), which
remains the authority this tool never argues with, and to everyone who ran a
release candidate against real work and said what broke.

### Notes

Splitting `crates/normfix-engine/src/pipeline.rs` into smaller internal modules
is the first change after this tag. It has been deferred through three
candidates so that release-candidate behavior was never obscured by a large
structural diff.

Backend rule messages remain English; 1.1 finishes that translation, and until
then a non-English run says so in one line rather than implying a completeness
it does not have.

## [1.0.0-rc.3] / 2026-08-10

The third release candidate. normfix now speaks the reader's language for the
text it writes itself, and every release page tells you how to install the
version you are looking at.

### Added

- **A locale layer that cannot be half-translated.** `crates/normfix-i18n` owns
  language selection and the message catalogue. Each language is one struct
  literal, so an entry that some locale does not translate is a build failure
  rather than a review item. Two tests cover what the type system cannot: no
  entry may be empty, and a translation must carry the same `{placeholder}` set
  as its English original. Placeholders are named, so a translation is free to
  reorder them — Portuguese and Spanish put the grade word before its value
  where English does not.

- **`--lang`, and a process locale that is only a hint.** Selection follows
  `--lang`, then `NORMFIX_LANG`, `LC_ALL`, `LC_MESSAGES`, and `LANG`, then
  English. Only the primary subtag matters, so `pt_BR.UTF-8` is Portuguese. An
  unpublished `--lang` value continues in English with one advisory; an
  unpublished process locale falls back silently, because a hint is not a
  decision. Neither is ever fatal: output language must not be a reason to
  refuse to analyze a project.

- **English, Portuguese, Spanish, and French terminal output** for the run
  announcement, the report's own prose, and — the part that actually matters —
  every safety-critical prompt. The destructive warning and confirmation, the
  undo confirmation, the protected-scope refusal with its reason, and the line
  stating that nothing was written are all translated. A reader being asked in
  a language they do not read to confirm an irreversible operation is a real
  gap, not a cosmetic one.

- **Install commands at the top of every release.** A reader who lands on a
  release page wants to try that version. Stable releases open with the curl
  installer and the Homebrew formula; a pre-release prints the pinned
  `NORMFIX_VERSION=` form and says why, since the unpinned installer and the
  formula both track the latest stable release.

### Changed

- **The scope guard returns a reason, not a sentence.** It decides which scopes
  are protected; the catalogue owns the words. This is what let the refusal be
  translated without moving any part of the decision into the message.

- **`--format json` is never localized.** The `execution_start` event and the
  final report keep English values in every language, so a script never has to
  select a locale to stay reliable. The `y` answer to a confirmation prompt is
  the same kind of token and also stays English in every language: a prompt
  that offered a translated letter and then rejected it would be a trap in
  exactly the place that must not have one.

### Notes

Rule messages from the analysis backends are still English, and a non-English
run prints one line saying so. That line is not an apology; it is the honest
description of a partially translated report, and removing it is part of
translating the backends in 1.1 rather than a cosmetic change of its own.

Status tokens in the file table and severity words stay English beside the rule
IDs they belong to.

The browser and documentation work that this candidate was originally scheduled
to carry shipped early, in rc.2. `docs/ROADMAP.md` now records what actually
landed where.

Splitting `crates/normfix-engine/src/pipeline.rs` remains the first change
after 1.0.0.

## [1.0.0-rc.2] / 2026-08-10

The second release candidate. It is about answering the question a student
actually has before a defense — *will this pass as it stands?* — and about a
run never being a surprise.

### Added

- **A pre-defense evaluation.** `preflight` now ends with a transparent 0–100
  estimate, a letter band, a verdict, and exactly located hard failures. It is
  structurally non-conclusive rather than conclusive-with-a-disclaimer:
  `conclusive` is always `false`, coverage gaps downgrade the verdict to
  `INCOMPLETE` instead of producing a confident grade, and the caveats travel
  inside the JSON `notes` array so an agent relaying the result cannot drop
  them.

  The evaluation judges the bytes on disk, not the bytes normfix could write.
  A project whose Norm and Makefile errors are all auto-fixable used to look
  like it would pass a defense it would actually fail. On a four-file sample it
  reports `31/100` with fourteen hard failures; after `normfix` writes the
  fixes it had already proven, the same project reports `59/100` with one — the
  Makefile still lists a deleted source, which is a decision, not a fix.

- **Every run announces itself.** Before reading a single file, `normfix` states
  the action, the resolved scope, and the effective safety configuration. The
  `scope` line is the one that matters: a command typed in the wrong directory
  looks wrong there, rather than in the summary afterwards. JSON mode emits the
  same information as one `execution_start` event on `stderr`, leaving the
  versioned report as the only document on `stdout`.

- **Protected scopes are refused.** Filesystem roots, complete user home
  directories, operating-system trees, and broad multi-project directories exit
  `2` without reading anything. The decision resolves symbolic links and
  collapses `..` first, so neither `/work/../etc` nor a link into `/etc` slips
  past it, and a Git-scoped run is judged by the repository root rather than by
  the thousands of files beneath it. `--force` acknowledges such a path and
  grants nothing else.

- **Orphan header prototypes.** A prototype with no implementation and no use
  anywhere in the project is reported by default, at the prototype name.
  Removal is available only under explicit authorization, only when the
  selected inputs are the complete project C/header set, and only when that set
  contains no definition, call, function-pointer reference, macro, string,
  conditional, attribute, or token-paste evidence. A definition whose body is
  only braces, whitespace, and comments is warning-only, because an intentional
  no-op can be correct.

- **Trivia-only Makefile sources.** A source token pointing at a file that
  exists but holds nothing beyond whitespace and comments is now distinguished
  from a missing one and reported through its own rule, so the reason for a
  removal is never ambiguous.

- **A verified 42 identity is remembered.** Supplying a valid identity once
  saves it atomically in the platform's private per-user configuration, and
  later runs stop asking.

- **The browser playground.** A Monaco-based multi-file editor for C sources,
  headers, Makefiles, and README documents, localized in English, Portuguese,
  Spanish, and French, with the 42 header available from a locally stored
  identity that is never sent anywhere.

### Changed

- **An untested Norminette release is usable by default.** Refusing to run was
  protecting the wrong thing: the before/after regression proof compares two
  answers from the same executable, so it stays valid whatever version that is.
  What an untested release actually costs is the guarantee that the native
  rules agree with it, so the run continues, says so prominently, and
  attributes every official finding to the version that produced it.
  `--strict-norminette-version` is there for CI that deliberately pins the
  tested checker; `--allow-untested-norminette` remains as a hidden no-op.

- **Coverage is stated instead of assumed.** A missing `normfix.toml`, an
  unidentifiable compiler, an unselected root Makefile, an absent Makefile, and
  a present README each produce their own advisory. A README is never a
  preflight failure — its presence only raises a 42-criteria review the tool
  cannot decide automatically.

- **Preflight runs the bounded analyzer without a second flag**, and carries the
  AddressSanitizer/UndefinedBehaviorSanitizer recipe, the LeakSanitizer caveat,
  and `clang-tidy` availability as guidance for the manual pass it will not run
  for you.

- **The JSON schema version moves to 2**, adding the optional `evaluation`
  object.

- **Releases are append-only.** A tag must be annotated and contained in `main`,
  an existing release is now a hard failure instead of an asset re-upload, and a
  pre-release passes `--latest=false` so a stable install can never resolve to
  it. Every workflow that can publish proves the npm dependency tree carries no
  known advisory first.

### Fixed

- **A function typedef is no longer recorded as a prototype.** Tree-sitter puts
  a `function_declarator` inside both `typedef int t_callback(void);` and a
  function-pointer typedef, so a type alias was collected as if it declared a
  callable symbol. Any proof that a declaration has no implementation would have
  accused the alias. The exclusion now lives at the fact boundary, where no
  consumer can miss it.

- **A closed-world removal is bound to the source set it was proven against.**
  Nothing stopped a file from appearing or disappearing between analysis and
  commit, which would let a removal land against a project that no longer
  matched its own proof. The transaction now revalidates that membership and
  refuses on any mismatch.

- **A guarded multi-file transaction is preserved** rather than partially
  applied, and `upgrade` keeps a stable install off prereleases.

### Notes

Documentation now shows what commands emit instead of only describing it. The
announcement banner, both protected-scope refusals with their exit status, the
estimate before and after a fix, and the `evaluation` JSON object are captured
from real runs.

Splitting `crates/normfix-engine/src/pipeline.rs` remains the first change
after 1.0.0.

## [1.0.0-rc.1] / 2026-08-06

The first release candidate for 1.0.0. It is about one thing: seeing where an
error is without leaving the terminal.

### Added

- **Every diagnostic is shown against its own source.** Snippets used to appear
  only under `--verbose`; the default view was a list of `path:line:column`
  coordinates you had to go and look up one at a time. The default now shows
  the code, with carets under the exact bytes the rule is about.

  ```text
  error[CC_IMPLICIT_FUNCTION_DECLARATION]: 2 occurrences in 2 files
    --> srcs/sort/sort.c:30:3
     |
  30 |         sort_medium(ctx);
     |         ^^^^^^^^^^^ call to undeclared function 'sort_medium'
  ```

  Occurrences of one rule in one file share a snippet, so the pattern is
  visible in its own context, and each keeps its own message as the label on
  its own carets. The rendering uses
  [`annotate-snippets`](https://crates.io/crates/annotate-snippets), the
  library `rustc` renders its own diagnostics with. Its only dependencies are
  `anstyle` and `unicode-width`, both already in this workspace, so the whole
  feature adds one crate to the dependency graph and no new transitive one.

### Changed

- **Shared context is stated once per rule.** Help, notes, origin, and the
  `explain` hint were repeated under every occurrence, so four hits of one rule
  meant reading the same compiler note four times. That repetition was most of
  what made the grouped output hard to scan.
- **The default view stops after three occurrences of a rule** and reports how
  many it held back, naming `--verbose` for the rest. A project can carry
  thousands of one diagnostic, and printing every snippet would make the report
  unreadable in exactly the way snippets are meant to prevent.

### Fixed

- **A compiler column is read as a byte offset, not a display column.** The two
  authorities disagree about what a column is, and the disagreement is
  invisible until a line is indented with tabs: the official Norminette counts
  display columns, expanding a tab to the next four-column stop, while a C
  compiler counts bytes. Both were being read as display columns, so on a line
  indented with two tabs a compiler diagnostic at column 3 resolved to the
  first tab instead of the identifier. Every caret on an indented line pointed
  into the whitespace, which is most lines of a 42 project.
- **A caret spans the whole construct** rather than a single character. The
  previous renderer measured its underline from the wrong end of the range and
  usually produced one `^`.
- **A compiler diagnostic with no position inside the file it belongs to**,
  normally because the real location is in an included header, now names the
  file and the header instead of drawing a caret on line 1. Line 1 of a 42 file
  is the header block, so the old behavior accused unrelated code.

### Notes

The column in the `-->` line is now counted in characters, the convention a C
compiler uses, where it was previously a display column. The official
Norminette still reports display columns, so its own output can name a larger
column for the same character on a tab-indented line. The caret is the
authoritative answer to *where*, and `docs/reference/reporting.md` says so.

Terminal safety is unchanged: messages, notes, and paths are still escaped
before rendering, control characters in source are shown as visible pictures,
and spans are clamped to the source before reaching the renderer, so a stale
cache entry degrades to a caret in roughly the right place rather than a panic.

Splitting `crates/normfix-engine/src/pipeline.rs` remains the first change
after 1.0.0.

## [0.4.0-beta.5] / 2026-08-06

Written against an external code review. The review's own top priority was
benchmarks, on the grounds that a stated performance goal without instruments
is a guess. That turned out to be exactly right: the first benchmark found a
3-to-4x win that hand timing had hidden for weeks.

### Added

- **Benchmarks** (`cargo bench -p normfix-c-actions`) covering an
  already-correct file, a messy file, and a large one, plus an informational CI
  job. They measure the code this project owns; the official checker and the
  compiler dominate a cold run, but their cost is a process launch and says
  nothing about a change made here.
- **Two generated properties**: formatting any generated function twice equals
  formatting it once, and a function name survives layout verbatim.
- **Two fuzz targets** for the invariants generation cannot reach: the tape
  reconstructs any input byte for byte, and the action pipeline returns a value
  or a typed error rather than panicking. `cargo-fuzz` needs nightly, so the
  crate sits outside the pinned workspace and is documented as a manual step.
- **`--allow-untested-norminette`.** Refusing every release but 3.3.59 means
  the tool stops working for everyone the day 42 upgrades. The flag continues
  and reports `NORMINETTE_VERSION_UNTESTED`. This does not weaken the safety
  argument: the before/after proof compares two answers from the same
  executable, so a run still cannot make its own official result worse.

### Changed

- **The source is parsed once per pass instead of once per phase.** It cannot
  change while the phase loop runs, because accepting a batch is the only thing
  that rewrites it and that breaks out immediately. An already-correct 50-line
  file went from 4.49 ms to 0.98 ms and a messy 800-line file from 108 ms to
  37.9 ms. End to end on a real project a warm run improved 29 percent and a
  cold run 5 percent, because a cold run is dominated by the checker
  subprocess.
- **The README opens with something executable**: the install line, a
  before-and-after diff, and the field-test number. Provenance verification
  moved next to the install instructions, because an attestation nobody is told
  about protects nobody.

### Known gaps

- `pipeline.rs` is 3 800 lines and remains the largest module. Splitting it is
  a maintenance improvement with no behavioral effect, and a refactor of that
  size immediately before a stable release trades a real risk for a future
  benefit. It is the first change after 1.0.0.
- Benchmarks measure this project's own code, not the end-to-end run. The
  dominant cold-run cost is one Python process per file, which would need
  batched invocation to address, and that restructures the compatibility path.

## [0.4.0-beta.4] / 2026-08-06

The last beta. A performance and hardening pass, plus self-update.

### Added

- **`normfix upgrade`** replaces the running binary with the newest published
  release. The archive is verified against `SHA256SUMS` before anything is
  written, staging happens inside the destination directory so the final step
  is a rename, and a Homebrew-managed install is refused with the command that
  does the right thing instead.
- **A release notice.** A run prints one line when a newer version exists. This
  is the only network access outside `upgrade`, and it is narrow on purpose: at
  most once a day, only for interactive human output, never for `--format json`
  or a non-terminal, silent on failure, and disabled by
  `NORMFIX_NO_UPDATE_CHECK`. It sends no path, no source, and no identifier.

### Fixed

- The upgrade download used a predictable path under the shared temporary
  directory. On a multi-user machine another account could create that path
  first as a symbolic link and redirect the write. Downloads now go through a
  private directory that cannot already exist.

### Changed

- The physical line index is built once per parse instead of once per query.
  Every formatting phase asks for it and a file is re-examined once per
  fixed-point pass, so a full scan and allocation ran dozens of times per file.
  An 800-line file went from 0.67 s to 0.25 s; files the size of real 42
  sources were already fast enough that the difference is within noise.

### Verified, not changed

A pass over the crates looking for optimization and security work produced more
confirmation than repair, which is worth recording:

- **Hostile input fails closed.** Invalid UTF-8, embedded NUL bytes, a 1.6 MB
  file, a 200 000-term expression on one line, nesting deep enough to crash the
  official checker itself, a symbolic link to `/etc/passwd`, a symbolic link
  loop, and a filename containing `$(id)` were all refused or reported without
  modifying a single file and without executing anything.
- **The analysis cache earns its place**: a repeat run over a real project is
  about ten times faster than a cold one.
- **The official checker is invoked once per distinct file content**, not once
  per stage, because the in-memory run cache already deduplicates it.
- **Shell independence**: the binary and the installer behave identically under
  bash, zsh, and fish. The binary is compiled and the installer is POSIX `sh`,
  so the only shell-specific concern is `PATH` guidance, which the installer
  prints for both styles.

## [0.4.0-beta.3] / 2026-08-05

The analyzer release. `--analyzer` now uses whatever deep analyzer the
installed compiler ships, which on macOS means it runs at all for the first
time.

### Added

- **Clang analyzer support.** `--analyzer` previously meant GCC `-fanalyzer`
  only, so on macOS, where `cc` is Clang, the deep pass was always skipped.
  The compiler is now classified by its own version banner and gets the flags it
  understands: `-fanalyzer` on GCC, `--analyze` on Clang. This matters because
  `/usr/bin/gcc` on macOS answers `Apple clang version ...`, so the command name
  cannot be trusted.

### Fixed

- `normfix explain CC_ANALYZER_UNAVAILABLE` described the rule as a finding the
  analyzer had produced, when it means the analyzer never ran.
- A Clang analyzer finding was reported twice, once as the tagged warning and
  once as the first note of its own path trace.

## [0.4.0-beta.2] / 2026-08-05

The release that prepares 1.0.0. No new formatting behavior beyond the
convergence fix: this version exists to close the quality gaps that stood
between the project and a stable promise, and to exercise the release pipeline
itself before it carries a 1.0 tag.

### Fixed

- **A single run now converges.** Several actions are driven by diagnostics the
  official checker reports, and the checker only sees the bytes in front of it,
  so correcting indentation exposed an alignment rule that had been masked. One
  pass converged the native actions but not the file. The pipeline now
  re-consults the checker within one invocation, bounded at three rounds. Found
  by running the tool over four real 42 projects, where a second run was still
  applying between 2 and 24 fixes; it now applies none.
- The Vercel install command no longer relies on `npm ci --prefix`, which does
  not reliably honor the flag and looks for a lockfile in the current directory.

### Added

- **A verified one-line installer.**
  `curl -fsSL https://normfix.vercel.app/install.sh | sh` detects the platform,
  verifies the download against the published `SHA256SUMS`, and installs into
  `~/.local/bin`. No `sudo`, no system path, no toolchain, so it works where a
  student has no administrative rights. A checksum mismatch aborts and prints
  both digests.
- **A Homebrew tap.** `brew install viniciusnevescosta/normfix/normfix`
  installs the same verified prebuilt binary.
- **A copy button** in the playground's review output, falling back to
  selecting the text when a browser refuses clipboard access.
- **Generated property tests**: the token tape reconstructs any input exactly
  and its pieces tile the source without a gap or overlap; include reordering
  preserves the multiset of directives, sorts it, and settles.
- **Differential tests against the real Norminette 3.3.59**, asserting the three
  claims the product is stated in terms of: official diagnostics never rise, a
  second run changes nothing, and a file that compiled still compiles.
- **Unit tests for the pipeline orchestration**, which previously had none,
  covering the transaction root, Makefile source resolution, and every refusal
  path of the quarantine recovery root.
- **Supply-chain gates**: `cargo-deny` for advisories, licences, and sources,
  plus Dependabot for cargo, npm, and the actions.
- **A documentation site** with a page per command, a reference for every flag,
  a purpose page, and the project policies published from their repository-root
  files.

### Changed

- The README is 143 lines instead of 843. Nothing was removed: the reference
  material became site pages.
- The playground and the documentation are one npm workspace with one lockfile.
- A documentation page loads about 155 KB of JavaScript instead of about 4 MB;
  the diagram renderer is fetched only by a page that renders one.
- Dependency updates reviewed individually: `similar` 3.1, `redb` 2.6.3, `nix`
  0.31.3, `comrak` 0.54, `clap` 4.6.5, and four GitHub Actions majors. A bump
  that rewrote the MSRV guard on `ignore` was rejected, and that crate is now
  excluded from automated updates.

### Known gaps

- The Homebrew formula is updated by hand after a release rather than by the
  release workflow.
- Field testing covers four 42 projects on macOS. A wider corpus, and Linux
  field data, would strengthen the case for 1.0.0.

## [0.4.0-beta.1] / 2026-08-05

First published release, and the first release of the native Rust
implementation. The Python package that preceded it is gone; the CLI is now a
single native binary and the command is `normfix`, not `norminette-fix`.

### Added

- **Native Rust workspace.** Sixteen crates with explicit ownership: syntax,
  semantic facts, actions, official-tool compatibility, reporting, and
  filesystem writes are separated so uncertainty cannot leak into
  authorization.
- **Include ordering.** Contiguous `#include` blocks are reordered so system
  headers precede project headers, alphabetically inside each category. A block
  is rewritten only while every one of its lines is exactly one include
  directive; `--no-reorder-includes` disables it.
- **Focused workflows.** `format`, `lint`, `check`, `budget`, `preflight`,
  `explain`, and `undo` subcommands beside the commandless interface.
- **Interactive review.** `--interactive` previews each changed-file diff and
  writes only the files whose second-pass plan still matches the approved
  snapshot bytes.
- **Recoverable transactions.** External backups, a journal with an explicit
  state machine, commits in canonical path order, rollback from exact bytes,
  and an `undo` that fails closed on a changed target, a corrupt backup, a
  symbolic link, or a path escape.
- **Git scopes.** `--changed` and `--staged` resolve through bounded,
  NUL-delimited Git subprocesses with path-confinement checks.
- **Allowed-function policy.** `normfix.toml` declares the external functions a
  subject permits; findings are disabled entirely when the project snapshot is
  incomplete, biasing toward a missed warning over a false accusation.
- **Compiler preflight.** `cc -fsyntax-only -Wall -Wextra -Werror` runs by
  default as a diagnostics-only pass, with optional `-fanalyzer` advisories.
- **Per-function budgets.** `budget` reports body lines, locals, and parameters
  against the 25/5/4 limits.
- **Makefile and Markdown support.** A dedicated conservative Make formatter and
  literal source-reference reconciliation; CommonMark reprinting for README
  documents.
- **Recoverable destructive capabilities.** Comment removal, unused-`static`
  removal, missing Makefile source removal, and unexpected-file quarantine, each
  behind an explicit capability grant with external recovery storage.
- **External content-addressed cache.** A redb database outside the project,
  keyed by every deterministic input, failing open as a miss.
- **Browser playground.** A WebAssembly build of the parser and C actions,
  served as a static Vite site under a strict content security policy. Source
  never leaves the tab.
- **Documentation site.** VitePress documentation published at `/docs/` beside
  the playground, with rendered architecture diagrams.
- **Release pipeline.** Four prebuilt archives (Linux x86-64/ARM64, macOS
  Intel/Apple Silicon) with build provenance attestation and a `SHA256SUMS`
  manifest.
- **One-line installer.** `curl -fsSL https://normfix.vercel.app/install.sh | sh`
  detects the platform, verifies the download against the published
  `SHA256SUMS`, and installs into `~/.local/bin`. No `sudo`, no system path, no
  toolchain, so it works where a student has no administrative rights.
- **Homebrew tap.** `brew install viniciusnevescosta/normfix/normfix` installs
  the same verified prebuilt binary.
- **Offline rule explanations.** `normfix explain RULE_ID` prints a bundled
  article without scanning a project or using the network.

### Fixed

- A single run now converges. Several actions are driven by official
  diagnostics, and the oracle only sees the bytes in front of it: correcting
  indentation exposes an alignment rule that was masked before. The pipeline
  re-consults the official checker within one invocation instead of leaving
  those fixes for a second run. Found by running the tool over four real 42
  projects, where the second run was still applying between 2 and 24 fixes.
- Makefile source preconditions are expressed under the caller's project root.
  Canonicalizing them collapsed the transaction root to `/` on macOS, where
  `/var` is a symbolic link, and the write was then refused.
- Quarantine recovery storage rejects any location that overlaps the project,
  both before and after canonicalization, refuses symbolic-link ancestors, and
  revalidates after each directory it creates.
- The analysis cache refuses a database whose parent overlaps the analyzed
  project.
- The compiler preflight refuses to read a project source reached through a
  symbolic link, matching the boundary already used for writes.

### Compatibility

- Requires the official Norminette **3.3.59** exactly; other releases are
  rejected rather than accepted with a warning.
- Minimum supported Rust version: **1.85**, verified independently in CI. The
  `ignore` dependency is held below 0.4.30, which needs a newer compiler and
  publishes no `rust-version` for the resolver to honor.
- No native Windows archive. Use WSL for the full CLI, or the browser
  playground for an in-memory preview.

### Known gaps

- The pipeline orchestration layer has no unit tests; it is covered indirectly
  through end-to-end tests.
- There are no generated property tests and no differential corpus run against
  real 42 projects.
- The documentation prefix relaxes `script-src` and `style-src` to
  `'unsafe-inline'`, because VitePress emits an inline script whose content
  changes with every build. The playground keeps the strict policy.

## [0.4.0-alpha.1] / unreleased

The native Rust rewrite as first committed. Never published; superseded by
`0.4.0-beta.1` before any archive existed.

## 0.3.0, 0.2.0, 0.1.0 / unreleased

The original Python `norminette-fix` package: a Norminette adapter, header
handling, declaration alignment, line compaction, Makefile support, and a
pytest suite. These versions were developed in the repository but never
published as GitHub releases, and the implementation was removed in
`0.4.0-beta.1`.

[1.1.1]: https://github.com/viniciusnevescosta/normfix/releases/tag/v1.1.1
[1.1.0]: https://github.com/viniciusnevescosta/normfix/releases/tag/v1.1.0
[1.0.0]: https://github.com/viniciusnevescosta/normfix/releases/tag/v1.0.0
[1.0.0-rc.3]: https://github.com/viniciusnevescosta/normfix/releases/tag/v1.0.0-rc.3
[1.0.0-rc.2]: https://github.com/viniciusnevescosta/normfix/releases/tag/v1.0.0-rc.2
[1.0.0-rc.1]: https://github.com/viniciusnevescosta/normfix/releases/tag/v1.0.0-rc.1
[0.4.0-beta.5]: https://github.com/viniciusnevescosta/normfix/releases/tag/v0.4.0-beta.5
[0.4.0-beta.4]: https://github.com/viniciusnevescosta/normfix/releases/tag/v0.4.0-beta.4
[0.4.0-beta.3]: https://github.com/viniciusnevescosta/normfix/releases/tag/v0.4.0-beta.3
[0.4.0-beta.2]: https://github.com/viniciusnevescosta/normfix/releases/tag/v0.4.0-beta.2
[0.4.0-beta.1]: https://github.com/viniciusnevescosta/normfix/releases/tag/v0.4.0-beta.1
