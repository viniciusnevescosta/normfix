# normfix

[![CI](https://github.com/viniciusnevescosta/normfix/actions/workflows/ci.yml/badge.svg)](https://github.com/viniciusnevescosta/normfix/actions/workflows/ci.yml)
[![Release](https://github.com/viniciusnevescosta/normfix/actions/workflows/release.yml/badge.svg)](https://github.com/viniciusnevescosta/normfix/releases)

Fix a whole 42 project's Norm errors in one command, without sudo.

```sh
curl -fsSL https://normfix.vercel.app/install.sh | sh   # no toolchain, no root
cd path/to/a/42-project
normfix --diff                                          # see it first
normfix                                                 # apply it
```

```diff
-int add(int a,int b){
-return a+b;
+int	add(int a, int b)
+{
+	return (a + b);
 }
```

Measured on four real 42 projects: **5 300 official Norminette errors fixed**,
zero regressions. What it cannot prove, it explains in English instead of a
rule name, and refuses to touch.

**[Try it in your browser](https://normfix.vercel.app)**, no install, nothing
uploaded.

## Requirements

The official Norminette `3.3.59` on `PATH`, or supplied with
`--norminette PATH`. Install it by following the instructions in its own
repository: **[42School/norminette](https://github.com/42School/norminette)**.
That project owns how it is installed, and its README is the only source that
stays correct when that changes.

`normfix` does not reimplement it. The official checker decides what counts as
a Norm error, and every run compares its answer before and after the edits; that
comparison is the reason a run cannot leave a file worse than it found it.
Another parseable release continues with a prominent compatibility advisory,
because its rule names and locations have not yet been verified. Pinned CI can
require exactly `3.3.59` with `--strict-norminette-version`.

Nothing else is required. The release archives contain a native binary and no
toolchain.

## Install

### Universal command

```sh
curl -fsSL https://normfix.vercel.app/install.sh | sh
```

Downloads the archive for your machine, verifies it against the published
`SHA256SUMS`, and installs into `~/.local/bin`. No sudo, no system path, no
toolchain, which is what makes it work on a locked-down 42 workstation. The
default channel is the latest stable GitHub release. Before the first stable
release exists, it falls back to the newest release candidate; set
`NORMFIX_VERSION=vX.Y.Z` to request one exact tag.

### Homebrew (MacOS / Linux distributions):

```sh
brew install viniciusnevescosta/normfix/normfix
```

### Scoop (Windows):

```powershell
scoop bucket add normfix https://github.com/viniciusnevescosta/scoop-normfix
scoop install normfix
```

Once installed, keep it current with:

```sh
normfix upgrade
```

You can also download the archive directly from the
[releases page](https://github.com/viniciusnevescosta/normfix/releases), or
build it from a checkout with
`cargo install --path crates/normfix-cli --locked`.

### Verify what you installed

Every archive is published with build provenance and a checksum manifest, so
you never have to trust the download channel:

```sh
gh attestation verify normfix-aarch64-macos.tar.gz --repo viniciusnevescosta/normfix
grep " normfix-aarch64-macos.tar.gz$" SHA256SUMS | shasum -a 256 -c -
```

The installer performs the checksum step for you and refuses to install on a
mismatch. The attestation check is the stronger one, because it ties the binary
to the workflow run that built it: use it if you are installing on a machine you
care about.

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
| [`upgrade`](https://normfix.vercel.app/docs/commands/upgrade) | yes | Replace this binary with the newest release in its channel |
| [`uninstall`](https://normfix.vercel.app/docs/commands/uninstall) | yes | Remove this binary, and with `--purge` the data it created |

Exit codes: `0` clean or fixed, `1` work remains, `2` the run itself failed,
`130` an interactive review was cancelled.

`preflight` ends with a non-conclusive score and grade, plus exact hard-fail
locations for unexpected files, installed-Norminette findings, and Makefile
problems in the current on-disk snapshot—even when the read-only shadow proposes
a fix. It runs a bounded compiler analyzer, but never an untrusted Makefile or
project binary; README absence is not a failure.

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
| [Performance](https://normfix.vercel.app/docs/reference/performance) | Measured numbers, and what dominates a run |
| [Architecture](https://normfix.vercel.app/docs/ARCHITECTURE) | What each crate owns and why the boundaries exist |
| [Compatibility](https://normfix.vercel.app/docs/COMPATIBILITY) | Supported Norminette, MSRV, and what versioning covers |

## Contributing

Building from source, the complete gate list every change must pass, and the
npm workspace that produces the playground and the documentation are in
[CONTRIBUTING.md](CONTRIBUTING.md).

The gate that matters most is the differential one: it asserts the claim the
whole project rests on, that a run never leaves a file with more official
diagnostics than it started with.

## Project

- [Changelog](CHANGELOG.md)
- [Contributing](CONTRIBUTING.md), including what a new transformation needs
- [Security policy](SECURITY.md), including how to verify a release

Created for the 42 curriculum by vneves-c.
Released under the [MIT License](LICENSE).
