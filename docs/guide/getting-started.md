# Getting started

By the end of this page you will have `normfix` installed and you will have run
it once on a real project without it changing a single file.

## Install it

One command, on Linux, macOS, and Windows:

```sh
curl -fsSL https://normfix.vercel.app/install.sh | sh
```

It works out which machine you are on, downloads the matching build, checks it
against the published checksums, and puts it in `~/.local/bin`. It never asks
for `sudo`, never writes outside your home directory, and never installs a
compiler — which is what makes it work on a 42 workstation where you are not an
administrator.

On Windows, run it from any POSIX shell: Git Bash, MSYS2, Cygwin, or WSL.

Check that it landed:

```sh
normfix --version
```

If the shell cannot find the command, `~/.local/bin` is not on your `PATH` yet.
Add it to your shell's startup file and open a new terminal.

::: tip Reading before running
Piping a script into a shell means running code you have not read. If you would
rather look first, it is right here:
<https://normfix.vercel.app/install.sh>
:::

## You also need the Norminette

`normfix` does not decide what the Norm says — the official Norminette does,
and `normfix` asks it. So the checker has to be installed and on your `PATH`
before a run means anything.

Install it from its own repository, [42School/norminette][norminette]. That
project decides how it is installed, and its README is the only page that stays
correct when that changes.

[norminette]: https://github.com/42School/norminette

Then check that `normfix` will find it:

```sh
norminette --version
```

A campus-managed install is fine. All that matters is that the command runs and
reports a version.

::: warning If your campus updates the Norminette
`3.3.59` is the version this release was tested against. A different one still
runs, with a warning, so a campus upgrade never leaves you without the tool. In
a pipeline where you want the version pinned, `--strict-norminette-version`
refuses anything else. The [compatibility policy](/COMPATIBILITY) explains what
that warning costs you.
:::

## Your first run changes nothing

Go to a project and ask what `normfix` would do:

```sh
normfix --check
```

That reads your files and writes none of them. It prints what it would fix,
what it cannot fix, and why.

To see the edits themselves rather than a summary:

```sh
normfix --diff
```

When you are ready:

```sh
normfix
```

This one writes. Before it does, it copies every file it is about to touch into
a backup directory outside your project, so `normfix undo` can put them back.

## Other ways to install

The installer above is the one that works everywhere. These are here because
you may already be using one of them.

### Homebrew

```sh
brew tap viniciusnevescosta/normfix https://github.com/viniciusnevescosta/normfix
brew install viniciusnevescosta/normfix/normfix
brew upgrade viniciusnevescosta/normfix/normfix  # later
brew uninstall normfix                            # to remove it
```

Installs the same verified binary rather than building it. macOS and Linuxbrew.

### Scoop, on Windows without a POSIX shell

```powershell
scoop bucket add normfix https://github.com/viniciusnevescosta/normfix
scoop install normfix
scoop update normfix     # later, to upgrade
scoop uninstall normfix  # to remove it
```

Scoop owns that install, so `normfix upgrade` and `normfix uninstall` refuse
and point you back at these — replacing the binary underneath would leave
Scoop's manifest describing something that is no longer there.

### Downloading the archive yourself

Every release publishes a build per platform, with a `SHA256SUMS` file beside
them:

| Platform | Archive |
|---|---|
| Linux x86-64 | `normfix-x86_64-linux-gnu.tar.gz` |
| Linux ARM64 | `normfix-aarch64-linux-gnu.tar.gz` |
| macOS Intel | `normfix-x86_64-macos.tar.gz` |
| macOS Apple Silicon | `normfix-aarch64-macos.tar.gz` |
| Windows x86-64 | `normfix-x86_64-windows.zip` |
| Windows ARM64 | `normfix-aarch64-windows.zip` |
| FreeBSD x86-64 | `normfix-x86_64-freebsd.tar.gz` |

On Apple Silicon, for example:

```sh
version=1.9.0
archive="normfix-aarch64-macos.tar.gz"
curl -LO "https://github.com/viniciusnevescosta/normfix/releases/download/v${version}/${archive}"
curl -LO "https://github.com/viniciusnevescosta/normfix/releases/download/v${version}/SHA256SUMS"
grep " ${archive}$" SHA256SUMS | shasum -a 256 -c -
tar -xzf "${archive}"
install -m 0755 normfix "$HOME/.local/bin/normfix"
normfix --version
```

Create `$HOME/.local/bin` first if it does not exist, and make sure it is on
your `PATH`.

### Pinning a version, or choosing where it goes

```sh
NORMFIX_VERSION=v1.9.0 sh -c "$(curl -fsSL https://normfix.vercel.app/install.sh)"
NORMFIX_BIN_DIR=~/bin sh -c "$(curl -fsSL https://normfix.vercel.app/install.sh)"
```

`NORMFIX_VERSION` is taken literally: that tag is downloaded, with no channel
selection. Without it, you get the newest stable release — or, if none has been
published yet, the newest pre-release, so a release candidate stays installable.

If a checksum does not match, the install stops and prints both values.

### Building from source

```sh
git clone https://github.com/viniciusnevescosta/normfix.git
cd normfix
cargo install --path crates/normfix-cli --locked
```

Or build without installing:

```sh
cargo build --release --locked -p normfix
./target/release/normfix --version
```

This is the one route that needs a Rust toolchain — 1.85 or newer. Cargo
installs into `~/.cargo/bin`, so that directory has to be on your `PATH`.

## If your system says the developer cannot be verified

macOS and Windows warn about programs that are not signed with a paid developer
certificate. `normfix` is not signed, so you may see that warning — and which
route you took decides whether you do at all.

The one-line installer downloads with `curl`, which does not attach the marker
that triggers the warning, so installing that way you will never see it. A
browser does attach it, so downloading an archive from the releases page is the
case that warns.

**On macOS**, the message says the developer cannot be verified. Open it once
from Finder with **right-click → Open**, which offers a button that
double-clicking does not. Or clear the marker yourself:

```sh
xattr -d com.apple.quarantine ./normfix
```

**On Windows**, SmartScreen says it protected your PC. Choose **More info**,
then **Run anyway**. From PowerShell:

```powershell
Unblock-File .\normfix.exe
```

Not signing is a decision, not an oversight. A certificate proves that someone
paid a certificate authority; it says nothing about what is inside the file.
Every archive here is published with its checksum and with build provenance
tying it to the exact workflow run that produced it — a stronger claim, which
the operating system simply does not look at:

```sh
gh attestation verify normfix-aarch64-macos.tar.gz --repo viniciusnevescosta/normfix
```

If that succeeds, the file you have came out of this project's release
pipeline, whatever your system says about its signature.

## Where to go next

- [Command line](/guide/command-line) — the workflows, the flags, and what each
  exit code means.
- [Browser playground](/guide/playground) — try it without installing anything.
- [Architecture](/ARCHITECTURE) — how the pieces fit, and why the boundaries
  are where they are.
