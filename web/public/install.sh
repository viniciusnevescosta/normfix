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
#   NORMFIX_VERSION   install this exact tag instead of the newest stable release
#   NORMFIX_BIN_DIR   install here instead of ~/.local/bin

set -eu

REPO="viniciusnevescosta/normfix"

die() {
    printf 'normfix install: %s\n' "$1" >&2
    exit 1
}

note() {
    printf '%s\n' "$1"
}

if [ -n "${NORMFIX_BIN_DIR:-}" ]; then
    BIN_DIR="$NORMFIX_BIN_DIR"
elif [ -n "${HOME:-}" ]; then
    BIN_DIR="$HOME/.local/bin"
else
    die "HOME is not set; set NORMFIX_BIN_DIR to the directory that should receive normfix"
fi

need() {
    command -v "$1" >/dev/null 2>&1 || die "this installer needs $1 on PATH"
}

need uname
need tar
need mkdir
need install
need mktemp
need mv
need awk
need sed

if command -v curl >/dev/null 2>&1; then
    fetch_to() {
        curl -fsSL --proto '=https' --tlsv1.2 --connect-timeout 10 \
            --max-time 120 --max-filesize 134217728 "$1" -o "$2"
    }
elif command -v wget >/dev/null 2>&1; then
    fetch_to() { wget -q --timeout=120 --tries=1 -O "$2" "$1"; }
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
    FreeBSD:amd64 | FreeBSD:x86_64) archive="normfix-x86_64-freebsd.tar.gz" ;;
    # Git Bash, MSYS2, and Cygwin all report a Windows kernel and all run this
    # script. Windows ships `.zip` because that is what the platform opens on
    # its own; unpacking one from here is a question of which tool is present,
    # handled below.
    MINGW*:x86_64 | MSYS*:x86_64 | CYGWIN*:x86_64 | Windows_NT*:x86_64)
        archive="normfix-x86_64-windows.zip"
        ;;
    MINGW*:aarch64 | MSYS*:aarch64 | CYGWIN*:aarch64 | Windows_NT*:aarch64 | MINGW*:arm64 | MSYS*:arm64 | CYGWIN*:arm64 | Windows_NT*:arm64)
        archive="normfix-aarch64-windows.zip"
        ;;
    *)
        die "no prebuilt binary for $os $arch. Build from source, or use the browser playground at https://normfix.vercel.app"
        ;;
esac

work="$(mktemp -d)"
install_stage=""
trap 'rm -rf "$work"; [ -z "$install_stage" ] || rm -f "$install_stage"' EXIT INT TERM

valid_version() {
    awk -v version="$1" 'BEGIN {
        identifier = "[0-9A-Za-z-]+"
        core = "(0|[1-9][0-9]*)\\.(0|[1-9][0-9]*)\\.(0|[1-9][0-9]*)"
        prerelease = "(-" identifier "(\\." identifier ")*)?"
        build = "(\\+" identifier "(\\." identifier ")*)?"
        if (version !~ ("^v" core prerelease build "$")) {
            exit 1
        }
        candidate = substr(version, 2)
        plus = index(candidate, "+")
        if (plus > 0) {
            candidate = substr(candidate, 1, plus - 1)
        }
        dash = index(candidate, "-")
        if (dash == 0) {
            exit 0
        }
        count = split(substr(candidate, dash + 1), identifiers, ".")
        for (position = 1; position <= count; position++) {
            if (identifiers[position] ~ /^0[0-9]+$/) {
                exit 1
            }
        }
        exit 0
    }'
}

# Emit `tag_name|prerelease` for complete published release records. GitHub's
# JSON is trusted metadata, but malformed, draft, and partial records must not
# become executable download paths. Token scanning works for both its pretty
# response and the compact fixtures below without requiring jq or Python.
published_releases() {
    awk '
        function value(token) {
            sub(/^[^:]*:[[:space:]]*/, "", token)
            gsub(/^"|"$/, "", token)
            return token
        }
        {
            rest = $0
            while (match(rest, /"(tag_name|draft|prerelease)"[[:space:]]*:[[:space:]]*("[^"]*"|true|false)/)) {
                token = substr(rest, RSTART, RLENGTH)
                if (token ~ /^"tag_name"/) {
                    tag = value(token)
                    draft = ""
                    preview = ""
                } else if (token ~ /^"draft"/) {
                    draft = value(token)
                } else {
                    preview = value(token)
                    if (tag != "" && draft == "false" && (preview == "true" || preview == "false")) {
                        print tag "|" preview
                    }
                    tag = ""
                    draft = ""
                    preview = ""
                }
                rest = substr(rest, RSTART + RLENGTH)
            }
        }
    ' "$1"
}

version="${NORMFIX_VERSION:-}"
if [ -z "$version" ]; then
    # GitHub's latest endpoint deliberately excludes pre-releases. Prefer it
    # so a normal installation stays on the stable channel.
    latest="$work/latest.json"
    if fetch_to "https://api.github.com/repos/$REPO/releases/latest" "$latest" 2>/dev/null; then
        version="$(
            sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$latest" |
                head -n 1
        )"
        prerelease="$(
            sed -n 's/.*"prerelease"[[:space:]]*:[[:space:]]*\([[:alpha:]]*\).*/\1/p' "$latest" |
                head -n 1
        )"
        # Fail closed if an unexpected response claims that /latest is a
        # pre-release; the release feed fallback below will reselect safely.
        if [ "$prerelease" != "false" ]; then
            version=""
        fi
    fi

    if [ -z "$version" ]; then
        # `/releases/latest` returns 404 before the first stable release. Scan
        # the public feed and prefer any stable tag; only when none exists do
        # we fall back to its newest pre-release. This also preserves the
        # stable-channel guarantee if the latest endpoint transiently fails.
        releases="$work/releases.json"
        fetch_to "https://api.github.com/repos/$REPO/releases?per_page=100" "$releases" 2>/dev/null ||
            die "could not determine the newest release; set NORMFIX_VERSION=vX.Y.Z"
        first_published=""
        while IFS='|' read -r candidate candidate_preview; do
            [ -n "$candidate" ] || continue
            if [ -z "$first_published" ]; then
                first_published="$candidate"
            fi
            if [ "$candidate_preview" = "false" ]; then
                version="$candidate"
                break
            fi
        done <<EOF
$(published_releases "$releases")
EOF
        if [ -z "$version" ]; then
            version="$first_published"
        fi
    fi
fi
[ -n "$version" ] || die "could not determine the newest release; set NORMFIX_VERSION=vX.Y.Z"
valid_version "$version" ||
    die "invalid release tag '$version'; expected vMAJOR.MINOR.PATCH with optional SemVer prerelease/build identifiers"

base="https://github.com/$REPO/releases/download/$version"

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

case "$archive" in
    *.zip)
        # No single unzip tool is guaranteed on Windows. `unzip` is common in
        # MSYS2 but absent from a stock Git for Windows; the system `tar` is
        # bsdtar, which reads zip; PowerShell is always there. Try each, and
        # say which ones were missing rather than failing on one name.
        if command -v unzip >/dev/null 2>&1; then
            unzip -q "$work/$archive" -d "$work"
        elif tar -xf "$work/$archive" -C "$work" 2>/dev/null; then
            :
        elif command -v powershell >/dev/null 2>&1; then
            powershell -NoProfile -NonInteractive -Command \
                "Expand-Archive -Path '$(cygpath -w "$work/$archive" 2>/dev/null || printf '%s' "$work/$archive")' -DestinationPath '$(cygpath -w "$work" 2>/dev/null || printf '%s' "$work")' -Force" ||
                die "could not unpack $archive"
        else
            die "this installer needs unzip, a tar that reads zip, or powershell to unpack $archive"
        fi
        ;;
    *)
        tar -xzf "$work/$archive" -C "$work"
        ;;
esac

binary=normfix
[ -f "$work/normfix.exe" ] && binary=normfix.exe
[ -f "$work/$binary" ] || die "the archive did not contain a normfix binary"
[ ! -L "$work/$binary" ] || die "the archive contained a symbolic link instead of a normfix binary"

mkdir -p "$BIN_DIR"
install_stage="$(mktemp "$BIN_DIR/.normfix-install.XXXXXX")"
install -m 0755 "$work/$binary" "$install_stage"
mv -f "$install_stage" "$BIN_DIR/$binary"
install_stage=""
note "installed $BIN_DIR/$binary"

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
note "normfix needs the official Norminette on PATH. Install it from its own"
note "repository, which is the only source that stays correct:"
note "  https://github.com/42School/norminette"
note "The tested baseline is 3.3.59; other parseable releases continue with a"
note "compatibility advisory."
note ""
note "Documentation: https://normfix.vercel.app/docs"
