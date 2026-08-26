#!/bin/sh

set -eu

root="$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)"
renderer="$root/packaging/render-manifests.sh"
temporary="$(mktemp -d)"
trap 'rm -rf "$temporary"' EXIT INT TERM
sums="$temporary/SHA256SUMS"
digest='aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'

for archive in \
    normfix-aarch64-macos.tar.gz \
    normfix-x86_64-macos.tar.gz \
    normfix-aarch64-linux-gnu.tar.gz \
    normfix-x86_64-linux-gnu.tar.gz \
    normfix-x86_64-windows.zip \
    normfix-aarch64-windows.zip; do
    printf '%s  %s\n' "$digest" "$archive" >>"$sums"
done

output="$temporary/output"
sh "$renderer" 2.3.4 "$sums" "$output" >"$temporary/render.log"
grep -Fq 'version "2.3.4"' "$output/Formula/normfix.rb"
grep -Fq '"version": "2.3.4"' "$output/bucket/normfix.json"
grep -Fq 'normfix-x86_64-linux-gnu.tar.gz' "$output/Formula/normfix.rb"
grep -Fq 'normfix-aarch64-windows.zip' "$output/bucket/normfix.json"
if grep -Eq 'normfix-[^-]+-unknown-' "$output/Formula/normfix.rb" "$output/bucket/normfix.json"; then
    printf '%s\n' 'a public package name leaked a toolchain vendor placeholder' >&2
    exit 1
fi

duplicate="$temporary/duplicate"
cp "$sums" "$duplicate"
printf '%s  %s\n' "$digest" normfix-aarch64-macos.tar.gz >>"$duplicate"
if sh "$renderer" 2.3.4 "$duplicate" "$temporary/duplicate-output" >/dev/null 2>&1; then
    printf '%s\n' 'a duplicate checksum unexpectedly rendered a manifest' >&2
    exit 1
fi

malformed="$temporary/malformed"
sed '1s/^[0-9a-f]*/xyz/' "$sums" >"$malformed"
if sh "$renderer" 2.3.4 "$malformed" "$temporary/malformed-output" >/dev/null 2>&1; then
    printf '%s\n' 'a malformed checksum unexpectedly rendered a manifest' >&2
    exit 1
fi

if sh "$renderer" '../2.3.4' "$sums" "$temporary/version-output" >/dev/null 2>&1; then
    printf '%s\n' 'an unsafe version unexpectedly rendered a manifest' >&2
    exit 1
fi

printf '%s\n' 'packaging manifest tests passed'
