# Getting started

## Requirements

- The [official Norminette](https://github.com/42School/norminette) command
  available on `PATH`, or supplied with `--norminette PATH`. Release `3.3.59`
  is the tested compatibility baseline.
- [Rust](https://www.rust-lang.org/tools/install) 1.85 or newer **only** when
  building from source. Release archives contain a native binary and need no
  Rust toolchain.

Install it by following the instructions in its own repository:
**[42School/norminette](https://github.com/42School/norminette)**. That project
owns how it is installed, and its README is the only source that stays correct
when that changes.

Once it is installed, check that `normfix` will find it:

```sh
norminette --version
```

A campus-managed environment works too. Only the command's version and its
availability on `PATH` matter to `normfix`.

::: warning Version compatibility
Another parseable Norminette release runs with a prominent compatibility
advisory so a campus upgrade does not disable the tool. Use
`--strict-norminette-version` to reject anything except `3.3.59` in pinned CI.
See the [compatibility policy](/COMPATIBILITY).
:::

## Install

### The one-line installer

```sh
curl -fsSL https://normfix.vercel.app/install.sh | sh
```

It detects your platform, downloads the matching release archive, verifies it
against the published `SHA256SUMS`, and installs the binary into
`~/.local/bin`. It never uses `sudo`, never writes to a system directory, and
never installs a toolchain, so it works on a 42 workstation where you have no
administrative rights. By default it uses GitHub's latest stable release. If
the project has not published a stable version yet, it safely falls back to
the newest pre-release so the current release candidate remains installable.

Two environment variables change what it does:

```sh
NORMFIX_VERSION=v1.3.1 sh -c "$(curl -fsSL https://normfix.vercel.app/install.sh)"
NORMFIX_BIN_DIR=~/bin sh -c "$(curl -fsSL https://normfix.vercel.app/install.sh)"
```

`NORMFIX_VERSION` is exact: the installer downloads that tag and does not
perform channel selection.

A checksum mismatch aborts the install and prints both digests. Read the script
before piping it to a shell if you would rather see what it does:
<https://normfix.vercel.app/install.sh>

### Homebrew

```sh
brew install viniciusnevescosta/normfix/normfix
```

The formula installs the same verified prebuilt binary; it does not build from
source. Available for macOS and Linuxbrew.

### Prebuilt binaries

Tagged releases provide native archives for Linux x86-64 and ARM64, plus macOS
Intel and Apple Silicon. Download the archive matching your machine from the
[latest release](https://github.com/viniciusnevescosta/normfix/releases/latest),
verify it against `SHA256SUMS`, and place `normfix` on `PATH`.

| Platform | Release archive |
|---|---|
| Linux x86-64 | `normfix-x86_64-linux-gnu.tar.gz` |
| Linux ARM64 | `normfix-aarch64-linux-gnu.tar.gz` |
| macOS Intel | `normfix-x86_64-macos.tar.gz` |
| macOS Apple Silicon | `normfix-aarch64-macos.tar.gz` |

For example, on Apple Silicon with release `1.3.1`:

```sh
version=1.3.1
archive="normfix-aarch64-macos.tar.gz"
curl -LO "https://github.com/viniciusnevescosta/normfix/releases/download/v${version}/${archive}"
curl -LO "https://github.com/viniciusnevescosta/normfix/releases/download/v${version}/SHA256SUMS"
grep " ${archive}$" SHA256SUMS | shasum -a 256 -c -
tar -xzf "${archive}"
install -m 0755 normfix "$HOME/.local/bin/normfix"
normfix --version
```

Create `$HOME/.local/bin` first if necessary and ensure it is on `PATH`.

### Build from source

```sh
git clone https://github.com/viniciusnevescosta/normfix.git
cd normfix
cargo install --path crates/normfix-cli --locked
```

Or build a release binary without installing it:

```sh
cargo build --release --locked -p normfix
./target/release/normfix --version
```

Cargo normally installs the command into `~/.cargo/bin`; ensure that directory
is on `PATH`.

### Windows

There is no native Windows archive. Run the Linux CLI and its Norminette
dependency inside [WSL](https://learn.microsoft.com/windows/wsl/install), or
use the
[browser playground](/guide/playground) for the in-memory formatter preview.
Native PowerShell and Windows process behavior are not part of the supported
CLI contract yet.

## Safe first run

Preview a project before writing anything:

```sh
normfix --check
normfix --diff
```

Then apply the accepted changes:

```sh
normfix
```

Default fix mode writes in place but keeps the original files in an external
backup directory. No project file is written in `--check` or `--diff` mode.

## Next steps

- [Command line](/guide/command-line): workflows, flags, Git scopes, and exit
  codes.
- [Browser playground](/guide/playground): try the formatter with no install.
- [Architecture](/ARCHITECTURE): what each crate owns and why the boundaries
  exist.
