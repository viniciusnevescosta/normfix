# Release notes

Every published version of `normfix`, newest first. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html): a hyphenated
version such as `0.4.0-beta.1` is a pre-release and is never published as the
latest stable download.

Published archives, checksums, and build provenance live on the
[releases page](https://github.com/viniciusnevescosta/normfix/releases).
`docs/RELEASING.md` describes how a release is produced.

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

[0.4.0-beta.1]: https://github.com/viniciusnevescosta/normfix/releases/tag/v0.4.0-beta.1
