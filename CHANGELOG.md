# Release notes

Every published version of `normfix`, newest first. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html): a hyphenated
version such as `0.4.0-beta.1` is a pre-release and is never published as the
latest stable download.

Published archives, checksums, and build provenance live on the
[releases page](https://github.com/viniciusnevescosta/normfix/releases).
`docs/RELEASING.md` describes how a release is produced.

## [1.6.0] / 2026-08-13

Bug fixes across the project, found by running the same files through both
runs and comparing the bytes rather than by reading the code.

### Fixed

- **Layout rules only fired where the official checker had already pointed.**
  In the terminal that is invisible, because the checker is right there. In the
  browser playground there is no Norminette, so those rules were inert: it
  returned a file it called formatted with `if(`, a one-line block, space
  indentation and an untabbed signature still in it, and reported no style
  problem over any of them.

  Every one of those answers was already derivable from the syntax tree, so the
  official report was selecting lines rather than deciding anything. One space
  between a control keyword and its parenthesis, over a closed set of reserved
  words no call can enter. A block's statements and its closing brace on their
  own lines. Indentation from the brace depth the parser reports, skipped where
  the leading whitespace is inside a string or a comment and is content. One tab
  between a return type and its declarator, which the identical prototype rule
  already did without asking. One space on each side of a binary or assignment
  operator, read from the operator the grammar names, so unary `-a` and `a++`
  are never reached. No space before a comma and one after. A declarator star
  bound to its name, while multiplication stays untouched. A single-statement
  control body on its own line. No blank line pressed against a brace. No
  padding inside a subscript.

  The two runs now produce identical bytes on the reported files.

- **A brace written as `){` was never moved.** The rule replaced the whitespace
  in front of the brace and, finding none, gave up — so a run reported success
  and left the file with an official error.

- **Nested braceless bodies were indented too shallowly.** The model counted
  braces and treated everything else as a single continuation, so `if` inside
  `if` inside `if` put the body at the header's own depth. A control header and
  an unfinished expression need opposite counting: `if (x)` stacks, `a +` does
  not.

- **A statement that is only a semicolon is deleted.** `return (0);;` and a
  stray `;` after a block survived every phase. The deletion carries its own
  proof: the parent must be a block or the file, since the same shape is how
  `while (x);` spells an empty body, and no preprocessor directive may precede
  it, since the `;` may then belong to one build configuration only.

- **A README lost its checkboxes and footnotes.** The reprinter read plain
  CommonMark, which has no task lists, footnotes, tables, or strikethrough, so
  `- [x] done` and `[^1]` were ordinary text and a faithful reprint escaped
  their brackets. Documents are now read in the dialect they are written in.

- **A byte-order mark ended up in the middle of the file.** The pass that
  removes it looks at the first byte, and the 42 header was written above it
  first, so the mark moved to where the official checker reads it as a stray
  lexeme sharing a line with an instruction. Three official errors became two,
  so the differential guard stayed quiet while the file got worse.

- **The playground could hang waiting for a frame that never came.** Pressing
  Run yielded to let the "formatting" message paint, and yielded by waiting for
  an animation frame; a hidden or backgrounded tab never fires one.

### Added

- **A recipe line indented with spaces is reported.** Make refuses the entire
  file with `missing separator`, so nothing in it runs. It is reported rather
  than corrected, because replacing the spaces with a tab decides that the line
  was meant as a command.

### Performance

A run reads the file once to plan, once per accepted batch to prove the batch,
and no more. It used to read it twice per pass and throw the proving read away,
and each proven layout rule derived the preprocessor line set by scanning the
whole file for itself — once per rule, and once per block for the rule that runs
per block.

```text
clean 50 lines    963 µs -> 525 µs
messy 40 lines   1.93 ms -> 2.11 ms
messy 800 lines    38 ms ->  42 ms
```

A clean file is 45% faster than in 1.5.0. A messy one is within a tenth of it
while applying five times as many fixes, because this release finds far more to
fix. A test holds the scheduler to that budget by counting reads rather than
timing them, so it means the same thing on any machine — the benchmark CI runs
is informational, and nothing would have failed when this regressed.

## [1.5.0] / 2026-08-12

A leak check you can actually run, with the boundary it needed drawn where a
reader can see it.

### Added

- **`normfix leaks` runs a program you already built, under a leak checker.**
  It is the only command that executes your code rather than reading it, so it
  names the program and waits for a `y`; `--force` is how a script says it meant
  it, and outside a terminal that is the only way through.

  normfix never builds the program. Building means running your Makefile's
  recipes, which is a second and much larger category of running code you wrote,
  and "you built it, I ran it" is a far smaller promise than "I built and ran
  it".

  Every result carries the line that says what it is: what one run observed on
  one path with the arguments it was given, never a proof that a program does
  not leak. Memory still reachable at exit is not counted as lost, because 42
  evaluates memory nobody can reach any more. And output the checker produces
  that cannot be read as a leak summary is an error rather than a clean result —
  a checker that was killed and a checker that found nothing produce the same
  silence.

  Which checker answers is not normfix's business: it locates `valgrind` on
  `PATH` and verifies it by its own `--version`. That is why macOS works through
  the community port with no code here, and why Windows is answered with WSL
  rather than a second tool with a different output format, a second version to
  pin, and findings that could not be compared with these.

### Fixed

- **The compatibility policy no longer says normfix never invokes a leak
  checker.** That was true until this release made it false, in all four
  languages.

## [1.4.0] / 2026-08-12

Windows and FreeBSD, on the evidence CI produces for them rather than on the
assumption that portable code ports.

### Added

- **Native Windows on x86-64 and ARM64, and FreeBSD on x86-64.** Every one of
  them runs the complete test suite, drives the real official Norminette, and
  proves on the platform itself that a run never leaves a file with more
  official diagnostics than it started with.

- **The one-line installer installs everywhere.** It downloads the Windows
  archive, verifies the same published checksum, and installs `normfix.exe`;
  Homebrew and Scoop are conveniences on top of it rather than the only route.
  On Windows it needs a POSIX shell — Git Bash, MSYS2, Cygwin, or WSL — which
  the documentation states where a reader meets it.

### Fixed

- **The protected-scope guard recognized nothing on Windows.** The lists of
  filesystem roots, home directories, and operating-system trees were Unix path
  shapes, so a Windows build would have shipped with the refusal to scan them
  silently absent. System trees are now read from the environment, because
  Windows can be installed on any drive, and two spellings of one directory no
  longer disagree.

- **Every commit and every backup failed on Windows.** Durability flushed the
  parent directory with `File::open`, which POSIX requires and Windows refuses.

- **The backup directory did not resolve on Windows**, so the default action
  would have been unavailable behind a message about configuration rather than
  about the platform.

- **The process bound bounded one process.** A checker that spawned helpers
  could leave them running past its own deadline on any non-Unix platform. The
  tool now goes into a job object, and a test proves the tree dies — verified to
  fail when containment is bypassed, so it is not passing by accident.

### Changed

- **The compatibility policy claims Windows and FreeBSD, and names what is
  still different.** Containment has a window on Windows that Unix does not
  have, and a rename is not written through there. FreeBSD on ARM64 is not
  published: it has no prebuilt standard library on the pinned toolchain and no
  runner to execute a suite on.

## [1.3.2] / 2026-08-12

What a first visit costs, and what a finger and a screen reader can reach.
Site only; nothing in the versioned surface moved.

### Fixed

- **The playground stopped making a phone wait for what it does not need.**
  Measured on a throttled mobile connection, the largest paint took 5.8 s. Three
  things caused it, and all three are gone: the GitHub star count — decoration —
  sat on the critical path for 900 ms and now waits for an idle moment after
  load; the stylesheet blocked the first render for its own round trip and is
  now inlined; and the WebAssembly module was three hops down the chain, since
  the browser could not learn it existed until the entry script had loaded the
  wasm-bindgen glue. It is now announced in the HTML and starts downloading
  immediately.

- **Every control can be hit with a finger.** On a phone the remember-identity
  checkbox measured 13×13, Forget 40×19, the dependency links 14 tall, and both
  header selects 23 — under the 24 CSS pixels WCAG asks for. They are raised
  for coarse pointers only, so a mouse keeps the dense layout the workbench was
  designed around.

- **The primary documentation button is legible in the dark theme.** White on
  the light green brand color measured 1.9:1. The label is now dark on that
  background, which measures 8.0:1.

- **The documentation landing pages expose a main landmark.** Every other page
  gets one from the theme, so the landing page was the single page where a
  screen reader had nothing to skip to.

- **`llms.txt` says what it is supposed to say.** It listed its links as plain
  text rather than as Markdown links, so a reader parsing it found none. It now
  follows the published format, with a summary and grouped sections.

## [1.3.1] / 2026-08-11

Site and documentation only. No CLI flag, exit code, JSON field, or on-disk
layout changed, so nothing this release touches is part of the versioned
surface described in `docs/COMPATIBILITY.md`.

### Added

- **Light, dark, or system appearance in the playground**, beside the language
  selector. It follows the operating system unless the reader says otherwise,
  and the choice is remembered on the device. Following the system costs no
  flash of the wrong colors, because the stylesheet — not a script — decides
  the first frame for a reader who never chose anything.

### Fixed

- **Fake tools in the test suite wait until they can actually be run.** The
  same `Text file busy` race resurfaced in the oracle suite, where only two of
  sixteen fixtures were protected. The fixture helper itself now probes the
  script it just wrote, which covers every call site at the source; the retry
  at the point of use stays, because a probe narrows the window rather than
  closing it.

- **The release gate no longer fails at random.** Two test suites spawned a
  shell script they had just written, and Linux refuses to execute a file while
  any process holds it open for writing — which happens whenever a sibling test
  thread forks between its own `fork` and `exec`. `normfix-oracle` already
  waited that window out; the engine and Git-scope suites now do too. The
  Git-scope timeout test also allowed only 20 ms to start a shell, which is
  under the real cost of doing so on a loaded, instrumented runner.

## [1.3.0] / 2026-08-11

The playground stops needing a network, and starts behaving like a workbench
for a real project instead of a text box that formats one file.

### Added

- **The playground works with no network, and can be installed as an app.**
  Open it once and the page, the WebAssembly formatter, and the interface are
  stored on the device. After that it runs on a plane, on school wifi at its
  worst, or while the site itself is down. Nothing was ever uploaded, so this
  changes how a student reaches the tool, not what it does.

  Two boundaries are deliberate. The desktop editor is not part of the install:
  Monaco would roughly double a first visit to buy syntax highlighting, so it
  is fetched only when there is a connection and kept afterwards, and a cold
  offline start falls back to the plain text area, which formats identically.
  And the worker answers only for the playground; the documentation shares the
  origin and is passed through, because a stale cached document is worse than a
  missing one.

  A new release never swaps itself in under an open tab. It downloads in the
  background and the header offers **New version ready** with **Reload**; until
  that is pressed, the reader keeps the version they started with.

- **Each language installs as its own app.** The playground publishes one web
  app manifest per locale, so an installed playground opens under the name and
  in the language its reader chose.

- **Drop files, or a whole project folder.** A dropped folder keeps its
  structure, so `libft/src/ft_strlen.c` arrives under that path rather than
  flattened. Object files, the compiled binary, `.git`, and editor settings are
  skipped rather than treated as errors — refusing an entire drop over one file
  normfix does not format would make the feature useless for the case it exists
  for. Nothing is skipped quietly: the count is always reported, and when
  nothing usable arrived, the first rejected path says exactly why.

- **Fix every file at once, or only the one in front of you.** Applying results
  one at a time was the only option, which does not scale to a real project. A
  run still covers the whole project, because a header and the file that
  includes it are judged correctly only together; the new choice is what to do
  with the answer.

### Changed

- **Downloads are a `.zip` instead of a `.tar`.** Every desktop platform opens
  a zip by double-clicking it; several need a separate tool for a tar. This
  also removes a path rule that only existed to satisfy the tar header format
  and rejected deeply nested but perfectly portable project layouts.

- **Importing skips what normfix does not format instead of failing.** Choosing
  or dropping thirty files no longer fails because one of them is an object
  file. The count of what was skipped is always shown.

- **Counted messages agree with their number in every language.** "1 arquivos
  adicionados" is wrong in Portuguese, Spanish, and French alike. Messages with
  a count now carry one entry per plural category, with a test that fails if a
  locale uses one wording for one and for many.

### Fixed

- **A failure to load the desktop editor no longer takes the playground with
  it.** Monaco is a large dynamic import and therefore the first thing to fail
  on a lost connection; it now falls back to the text area instead of leaving
  the page with no editor at all.

- **The contributor localization guide no longer promises a translation that
  is not coming.** The Portuguese, Spanish, and French guides still said native
  diagnostics would be translated "until CLI 1.1". They now state the shipped
  rule: normfix translates the findings it writes, and relays Norminette and
  compiler output in the language those tools produced it.

- **Five documentation pages were summaries rather than translations.** The
  landing page, getting started, the browser playground, the safety reference,
  and the compatibility policy were between a sixth and a half the length of
  their English originals in Portuguese, Spanish, and French — not shorter
  wording, but missing sections. They are complete, and each landing page now
  opens on "why normfix" rather than on installation.

## [1.2.0] / 2026-08-11

Accessibility for students who do not read English comfortably: the findings
normfix writes itself now arrive in their language.

### Added

- **normfix's own diagnostics in four languages.** A student running with
  `--lang pt` — or simply with a Portuguese system locale — now reads the
  project's own findings in Portuguese, not just the report's headings.

  The scope is deliberate and permanent. A finding relayed from the official
  Norminette or from the C compiler is *not* translated and never will be: that
  text is those tools' own output, and rewriting it would make the report
  disagree with what running `norminette` directly prints. The notice under a
  non-English report now says exactly that, as a fact about where the words come
  from rather than an apology for something missing.

  A diagnostic can now carry a reader-language rendering beside its English
  text. Three properties make that safe on a type that is serialized,
  deduplicated, and sorted: it never reaches JSON, equality ignores it — which
  is why `PartialEq` is hand-written rather than derived, since `dedup` relies
  on it and two diagnostics differing only by language are the same finding —
  and ordering ignores it, so a report's order does not depend on who is
  reading. Verified rather than assumed: `--lang pt --format json` still emits
  the English `message`.

### Fixed

- **Preflight stopped implying that a piscina exercise is missing something.** A
  piscina exercise is expected to contain only `.c` files, so a Makefile and
  project headers are both optional. The score was already correct, but the
  advisory read as a to-do list: "add or select the required Makefile before
  relying on preflight." It now says the check did not run, that this is normal
  for loose `.c` files, and that only the subject can say whether a Makefile is
  required — which normfix cannot read. A regression test locks it: loose `.c`
  files with no Makefile and no header score 100 with no hard failures.

### Changed

- **The Norminette repository owns its own install instructions.** Every page,
  the installer, the Homebrew caveat, and the release notes printed
  `pipx install norminette==3.3.59`. That command is not this project's to give:
  when 42School changes how their checker is installed, every copy here goes
  stale at once and a student follows an instruction that no longer works. All
  of them now point at [42School/norminette](https://github.com/42School/norminette),
  which is the only source that stays correct. The tested compatibility baseline
  is still named, because that is a fact about normfix rather than an
  installation step.

- Dependency updates: blake3 1.8.6, clap 4.6.6, criterion 0.7.0, and
  `actions/setup-python` v7. Criterion 0.8 is deliberately not taken because it
  requires Rust 1.86 and the supported minimum is 1.85.

### Notes

Choosing a language in the browser playground only changes the language: it
rewrites no files, re-runs no analysis, and issues no network request. That was
verified in a browser, and the choice is remembered until it is changed again.

The differential gate was re-run against the installed official Norminette for
this release.

`docs/ROADMAP.md` is rewritten for the next lines: the playground as a
workspace, project initialization, platform hardening including BSD, Valgrind
leak checking, and Python.

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
[1.6.0]: https://github.com/viniciusnevescosta/normfix/releases/tag/v1.6.0
[1.5.0]: https://github.com/viniciusnevescosta/normfix/releases/tag/v1.5.0
[1.4.0]: https://github.com/viniciusnevescosta/normfix/releases/tag/v1.4.0
[1.3.2]: https://github.com/viniciusnevescosta/normfix/releases/tag/v1.3.2
[1.3.1]: https://github.com/viniciusnevescosta/normfix/releases/tag/v1.3.1
[1.3.0]: https://github.com/viniciusnevescosta/normfix/releases/tag/v1.3.0
[1.2.0]: https://github.com/viniciusnevescosta/normfix/releases/tag/v1.2.0
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
