# Getting started

## Requirements

- The official Norminette version `3.3.59` available on `PATH`, or supplied
  with `--norminette PATH`.
- Rust 1.85 or newer **only** when building from source. Release archives
  contain a native binary and need no Rust toolchain.

Norminette uses its own Python runtime, as provided by the official package.
Install the exact checker in an isolated environment when it is not already
available, then verify it:

```sh
pipx install norminette==3.3.59
norminette --version
```

A campus-managed Python environment works too. Only the command version and
its availability on `PATH` matter to `normfix`.

::: warning Exactly 3.3.59
Any other Norminette release is rejected rather than accepted with a warning.
The official diagnostic names, locations, and accepted layouts are inputs to
the before/after regression proof, so silently accepting a newer checker could
authorize an edit under rules the formatter has never tested. See the
[compatibility policy](/COMPATIBILITY).
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
administrative rights.

Two environment variables change what it does:

```sh
NORMFIX_VERSION=v0.4.0-beta.1 sh -c "$(curl -fsSL https://normfix.vercel.app/install.sh)"
NORMFIX_BIN_DIR=~/bin sh -c "$(curl -fsSL https://normfix.vercel.app/install.sh)"
```

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

For example, on Apple Silicon with release `0.4.0-beta.1`:

```sh
version=0.4.0-beta.1
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
dependency inside WSL, or use the
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
