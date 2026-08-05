# normfix

[![CI](https://github.com/viniciusnevescosta/normfix/actions/workflows/ci.yml/badge.svg)](https://github.com/viniciusnevescosta/normfix/actions/workflows/ci.yml)
[![Release](https://github.com/viniciusnevescosta/normfix/actions/workflows/release.yml/badge.svg)](https://github.com/viniciusnevescosta/normfix/releases)

Safe automatic fixes and clear diagnostics for the 42 Norm.

A 42 student's scarcest resource is hours, and a large share of them goes into
whitespace: indentation, declaration blocks, 80 columns, official headers.
Across a cursus that is thousands of files, none of which teaches you anything
the second time. `normfix` fixes what it can prove is safe to fix, across a
whole project in one command, and explains the rest in English instead of a
rule name.

**[normfix.vercel.app](https://normfix.vercel.app)** hosts the browser
playground and the full documentation.

```sh
cd path/to/a/42-project
normfix
```

## Requirements

- The official Norminette `3.3.59` on `PATH`, or supplied with
  `--norminette PATH`. Any other release is rejected rather than accepted with
  a warning.
- Rust 1.85 or newer only when building from source. Release archives contain a
  native binary.

```sh
pipx install norminette==3.3.59
```

## Install

```sh
curl -fsSL https://normfix.vercel.app/install.sh | sh
```

Downloads the archive for your machine, verifies it against the published
`SHA256SUMS`, and installs into `~/.local/bin`. No sudo, no system path, no
toolchain, which is what makes it work on a locked-down 42 workstation.

With Homebrew:

```sh
brew install viniciusnevescosta/normfix/normfix
```

Or download the archive for your platform from the
[releases page](https://github.com/viniciusnevescosta/normfix/releases) and
verify it yourself, or build from a checkout with
`cargo install --path crates/normfix-cli --locked`.

There is no native Windows archive: use WSL, or the
[browser playground](https://normfix.vercel.app). Full instructions, including
provenance verification, are in
[Getting started](https://normfix.vercel.app/docs/guide/getting-started).

## Use it

Preview before writing anything:

```sh
normfix --check     # what would change
normfix --diff      # the exact diff
```

Then apply it:

```sh
normfix                       # the whole project
normfix src includes          # only these paths
normfix --changed             # only what you just touched
normfix format --interactive  # approve file by file
```

Writes go through one recoverable transaction with external backups.
`normfix undo` restores a run, and refuses to overwrite anything you edited
since.

| Command | Writes | What it is for |
|---|---|---|
| [`format`](https://normfix.vercel.app/docs/commands/format) | yes | Apply the accepted edits |
| [`lint`](https://normfix.vercel.app/docs/commands/lint) | no | Diagnose the bytes on disk, propose nothing |
| [`check`](https://normfix.vercel.app/docs/commands/check) | no | See what a fixing run would do |
| [`budget`](https://normfix.vercel.app/docs/commands/budget) | no | Line, variable, and parameter headroom per function |
| [`preflight`](https://normfix.vercel.app/docs/commands/preflight) | no | The read-only checks before a defense |
| [`explain`](https://normfix.vercel.app/docs/commands/explain) | no | Explain one rule offline |
| [`undo`](https://normfix.vercel.app/docs/commands/undo) | yes | Restore a previous run |

Exit codes: `0` clean or fixed, `1` work remains, `2` the run itself failed,
`130` an interactive review was cancelled.

## Read more

The documentation lives at
**[normfix.vercel.app/docs](https://normfix.vercel.app/docs)**.

| Page | What is in it |
|---|---|
| [Why normfix](https://normfix.vercel.app/docs/why) | The problem it solves, and what it refuses to do |
| [Every flag](https://normfix.vercel.app/docs/reference/flags) | What each one does, with an example |
| [What is fixed](https://normfix.vercel.app/docs/reference/fixes) | The automatic edits and the proof gates behind them |
| [Safety and recovery](https://normfix.vercel.app/docs/reference/safety) | Backups, transactions, and destructive capabilities |
| [Headers and identity](https://normfix.vercel.app/docs/reference/headers) | The official header block and inclusion guards |
| [Makefiles and project files](https://normfix.vercel.app/docs/reference/projects) | Make, README documents, and the compiler preflight |
| [Reporting](https://normfix.vercel.app/docs/reference/reporting) | Terminal output, the JSON schema, and the cache |
| [Known boundaries](https://normfix.vercel.app/docs/reference/boundaries) | Every limit, and why it is deliberate |
| [Architecture](https://normfix.vercel.app/docs/ARCHITECTURE) | What each crate owns and why the boundaries exist |
| [Compatibility](https://normfix.vercel.app/docs/COMPATIBILITY) | Supported Norminette, MSRV, and what versioning covers |

## Development

```sh
cargo fmt --all --check
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps --locked
cargo +1.85.0 test --workspace --all-targets --locked
cargo deny --all-features check
cargo test --locked -p normfix-engine --test differential -- --ignored
```

The last one needs the official Norminette on `PATH`. It asserts the claim the
whole project rests on: a run never leaves a file with more official
diagnostics than it started with.

The site is one npm workspace:

```sh
npm ci && npm run build   # playground into web/dist, documentation into web/dist/docs
npm run dev               # the playground
npm run docs:dev          # the documentation
```

## Project

- [Changelog](CHANGELOG.md)
- [Contributing](CONTRIBUTING.md), including what a new transformation needs
- [Security policy](SECURITY.md), including how to verify a release

Created for the 42 curriculum by vneves-c.
Released under the [MIT License](LICENSE).
