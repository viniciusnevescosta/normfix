# Release notes

Every published version of `normfix`, newest first. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html): a hyphenated
version such as `0.4.0-beta.1` is a pre-release and is never published as the
latest stable download.

Published archives, checksums, and build provenance live on the
[releases page](https://github.com/viniciusnevescosta/normfix/releases).
`docs/RELEASING.md` describes how a release is produced.

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

[1.0.0-rc.1]: https://github.com/viniciusnevescosta/normfix/releases/tag/v1.0.0-rc.1
[0.4.0-beta.5]: https://github.com/viniciusnevescosta/normfix/releases/tag/v0.4.0-beta.5
[0.4.0-beta.4]: https://github.com/viniciusnevescosta/normfix/releases/tag/v0.4.0-beta.4
[0.4.0-beta.3]: https://github.com/viniciusnevescosta/normfix/releases/tag/v0.4.0-beta.3
[0.4.0-beta.2]: https://github.com/viniciusnevescosta/normfix/releases/tag/v0.4.0-beta.2
[0.4.0-beta.1]: https://github.com/viniciusnevescosta/normfix/releases/tag/v0.4.0-beta.1
