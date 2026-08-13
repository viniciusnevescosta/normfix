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
NORMFIX_VERSION=v1.6.0 sh -c "$(curl -fsSL https://normfix.vercel.app/install.sh)"
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
| Windows x86-64 | `normfix-x86_64-windows.zip` |
| Windows ARM64 | `normfix-aarch64-windows.zip` |
| FreeBSD x86-64 | `normfix-x86_64-freebsd.tar.gz` |

For example, on Apple Silicon with release `1.6.0`:

```sh
version=1.6.0
archive="normfix-aarch64-macos.tar.gz"
curl -LO "https://github.com/viniciusnevescosta/normfix/releases/download/v${version}/${archive}"
curl -LO "https://github.com/viniciusnevescosta/normfix/releases/download/v${version}/SHA256SUMS"
grep " ${archive}$" SHA256SUMS | shasum -a 256 -c -
tar -xzf "${archive}"
install -m 0755 normfix "$HOME/.local/bin/normfix"
normfix --version
```

Create `$HOME/.local/bin` first if necessary and ensure it is on `PATH`.

### When the system says the developer cannot be verified

macOS and Windows warn about programs that are not signed with a paid developer
certificate. normfix is not, so you may see one — and which route you took
decides whether you do.

The one-line installer downloads with `curl`, which does not attach the flag
that triggers the warning. Installing that way, you will not see it at all. A
browser does attach it, so an archive downloaded from the releases page is the
case that warns.

On macOS the message is that the developer cannot be verified. Open it once
from the Finder with **right-click → Open**, which offers a button the normal
double-click does not, or clear the flag directly:

```sh
xattr -d com.apple.quarantine ./normfix
```

On Windows, SmartScreen says it protected your PC. Choose **More info**, then
**Run anyway**. From PowerShell:

```powershell
Unblock-File .\normfix.exe
```

Being unsigned is a deliberate position rather than an oversight. A signing
certificate proves that someone paid a certificate authority; it says nothing
about what the binary contains. Every archive here is published with a checksum
manifest and with build provenance that ties it to the workflow run that
produced it, which is a stronger claim — the operating system simply does not
consult it:

```sh
gh attestation verify normfix-aarch64-macos.tar.gz --repo viniciusnevescosta/normfix
```

If that command succeeds, the file you have came out of this project's release
workflow, whatever the operating system says about its signature.

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

The same one-line installer works from any POSIX shell — Git Bash, MSYS2,
Cygwin, or WSL — and installs `normfix.exe`:

```sh
curl -fsSL https://normfix.vercel.app/install.sh | sh
```

With only PowerShell, Scoop is the convenience:

```powershell
scoop bucket add normfix https://github.com/viniciusnevescosta/scoop-normfix
scoop install normfix
```

Or download `normfix-x86_64-windows.zip` or `normfix-aarch64-windows.zip` from
the releases page and put `normfix.exe` on `PATH`.

The official Norminette is a Python program and installs on Windows the same
way it does anywhere else. Running the Linux build inside WSL is still
supported and unchanged; the [compatibility policy](/COMPATIBILITY) names the
two places where native Windows behaves differently from Unix.

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
