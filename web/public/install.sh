#!/bin/sh
# normfix installer.
#
#   curl -fsSL https://normfix.vercel.app/install.sh | sh
#
# Downloads the release archive for this machine, verifies it against the
# published SHA256SUMS manifest, and installs the binary into a directory you
# own. It never uses sudo, never touches a system path, and never installs a
# toolchain. On a 42 workstation that means it needs no privileges at all.
#
# Environment:
#   NORMFIX_VERSION   install this exact tag instead of the newest release
#   NORMFIX_BIN_DIR   install here instead of ~/.local/bin

set -eu

REPO="viniciusnevescosta/normfix"
BIN_DIR="${NORMFIX_BIN_DIR:-$HOME/.local/bin}"

die() {
    printf 'normfix install: %s\n' "$1" >&2
    exit 1
}

note() {
    printf '%s\n' "$1"
}

need() {
    command -v "$1" >/dev/null 2>&1 || die "this installer needs $1 on PATH"
}

need uname
need tar
need mkdir
need install

if command -v curl >/dev/null 2>&1; then
    fetch() { curl -fsSL "$1"; }
    fetch_to() { curl -fsSL "$1" -o "$2"; }
elif command -v wget >/dev/null 2>&1; then
    fetch() { wget -qO- "$1"; }
    fetch_to() { wget -qO "$2" "$1"; }
else
    die "this installer needs curl or wget on PATH"
fi

# A checksum that is never verified is decoration, so refuse to continue
# without a tool that can compute one.
if command -v sha256sum >/dev/null 2>&1; then
    checksum() { sha256sum "$1" | cut -d' ' -f1; }
elif command -v shasum >/dev/null 2>&1; then
    checksum() { shasum -a 256 "$1" | cut -d' ' -f1; }
else
    die "this installer needs sha256sum or shasum to verify the download"
fi

os="$(uname -s)"
arch="$(uname -m)"
case "$os:$arch" in
    Linux:x86_64) archive="normfix-x86_64-linux-gnu.tar.gz" ;;
    Linux:aarch64 | Linux:arm64) archive="normfix-aarch64-linux-gnu.tar.gz" ;;
    Darwin:x86_64) archive="normfix-x86_64-macos.tar.gz" ;;
    Darwin:arm64) archive="normfix-aarch64-macos.tar.gz" ;;
    *)
        die "no prebuilt binary for $os $arch. Build from source, or use the browser playground at https://normfix.vercel.app"
        ;;
esac

version="${NORMFIX_VERSION:-}"
if [ -z "$version" ]; then
    # The newest release, whether or not it is a pre-release. Releases are
    # listed newest first.
    version="$(
        fetch "https://api.github.com/repos/$REPO/releases" 2>/dev/null |
            sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' |
            head -n 1
    )"
fi
[ -n "$version" ] || die "could not determine the newest release; set NORMFIX_VERSION=vX.Y.Z"

base="https://github.com/$REPO/releases/download/$version"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT INT TERM

note "normfix $version for $os $arch"

fetch_to "$base/$archive" "$work/$archive" ||
    die "could not download $archive for $version"
fetch_to "$base/SHA256SUMS" "$work/SHA256SUMS" ||
    die "could not download the checksum manifest for $version"

expected="$(sed -n "s/^\([0-9a-f]\{64\}\)[[:space:]]\{1,\}\*\{0,1\}$archive\$/\1/p" "$work/SHA256SUMS")"
[ -n "$expected" ] || die "$archive is not listed in SHA256SUMS"

actual="$(checksum "$work/$archive")"
if [ "$expected" != "$actual" ]; then
    die "checksum mismatch for $archive
  expected $expected
  actual   $actual
Refusing to install. Report this at https://github.com/$REPO/security/advisories/new"
fi
note "checksum verified"

tar -xzf "$work/$archive" -C "$work"
[ -f "$work/normfix" ] || die "the archive did not contain a normfix binary"

mkdir -p "$BIN_DIR"
install -m 0755 "$work/normfix" "$BIN_DIR/normfix"
note "installed $BIN_DIR/normfix"

case ":$PATH:" in
    *":$BIN_DIR:"*) ;;
    *)
        note ""
        note "$BIN_DIR is not on your PATH. Add it:"
        note "  bash/zsh   echo 'export PATH=\"$BIN_DIR:\$PATH\"' >> ~/.profile"
        note "  fish       fish_add_path $BIN_DIR"
        ;;
esac

note ""
note "normfix needs the official Norminette 3.3.59:"
note "  pipx install norminette==3.3.59"
note ""
note "Documentation: https://normfix.vercel.app/docs"
