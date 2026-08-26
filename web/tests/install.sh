#!/bin/sh

set -eu

root="$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)"
installer="$root/web/public/install.sh"
temporary="$(mktemp -d)"
trap 'rm -rf "$temporary"' EXIT INT TERM
tools="$temporary/tools"
mkdir -p "$tools"

cat >"$tools/uname" <<'EOF'
#!/bin/sh
case "$1" in
    -s) printf '%s\n' Linux ;;
    -m) printf '%s\n' x86_64 ;;
    *) exit 2 ;;
esac
EOF

cat >"$tools/sha256sum" <<'EOF'
#!/bin/sh
printf '%064d  %s\n' 0 "$1"
EOF

cat >"$tools/tar" <<'EOF'
#!/bin/sh
destination=""
while [ "$#" -gt 0 ]; do
    if [ "$1" = "-C" ]; then
        destination="$2"
        shift 2
    else
        shift
    fi
done
[ -n "$destination" ] || exit 2
printf '#!/bin/sh\nexit 0\n' >"$destination/normfix"
EOF

cat >"$tools/curl" <<'EOF'
#!/bin/sh
target=""
url=""
while [ "$#" -gt 0 ]; do
    case "$1" in
        -o)
            target="$2"
            shift 2
            ;;
        *)
            url="$1"
            shift
            ;;
    esac
done
printf '%s\n' "$url" >>"$TEST_REQUEST_LOG"
case "$url" in
    *"/releases/latest")
        [ "$TEST_LATEST_STATUS" = "ok" ] || exit 22
        printf '%s\n' "$TEST_LATEST_JSON" >"$target"
        ;;
    *"/releases?per_page=100")
        printf '%s\n' "$TEST_RELEASES_JSON" >"$target"
        ;;
    *"/SHA256SUMS")
        printf '%064d  %s\n' 0 normfix-x86_64-linux-gnu.tar.gz >"$target"
        ;;
    *"/normfix-x86_64-linux-gnu.tar.gz")
        printf 'archive\n' >"$target"
        ;;
    *)
        exit 22
        ;;
esac
EOF

chmod +x "$tools/uname" "$tools/sha256sum" "$tools/tar" "$tools/curl"

assert_contains() {
    expected="$1"
    file="$2"
    if ! grep -Fq "$expected" "$file"; then
        printf 'expected "%s" in %s:\n' "$expected" "$file" >&2
        sed 's/^/  /' "$file" >&2
        exit 1
    fi
}

run_install() (
    case_name="$1"
    latest_status="$2"
    latest_json="$3"
    releases_json="$4"
    exact_version="$5"
    case_dir="$temporary/$case_name"
    mkdir -p "$case_dir/bin"
    : >"$case_dir/requests"
    env \
        HOME="$case_dir/home" \
        PATH="$tools:$PATH" \
        NORMFIX_BIN_DIR="$case_dir/bin" \
        NORMFIX_VERSION="$exact_version" \
        TEST_REQUEST_LOG="$case_dir/requests" \
        TEST_LATEST_STATUS="$latest_status" \
        TEST_LATEST_JSON="$latest_json" \
        TEST_RELEASES_JSON="$releases_json" \
        sh "$installer" >"$case_dir/output"
    test -x "$case_dir/bin/normfix"
)

stable_json='{"tag_name":"v1.0.0","draft":false,"prerelease":false}'
preview_json='[{"tag_name":"v1.1.0-rc.1","draft":false,"prerelease":true}]'

run_install stable ok "$stable_json" "$preview_json" ""
assert_contains 'normfix v1.0.0 for Linux x86_64' "$temporary/stable/output"
assert_contains '/releases/latest' "$temporary/stable/requests"
if grep -Fq '/releases?per_page=100' "$temporary/stable/requests"; then
    printf '%s\n' 'stable install unexpectedly consulted the preview feed' >&2
    exit 1
fi

run_install preview_fallback fail '' "$preview_json" ""
assert_contains 'normfix v1.1.0-rc.1 for Linux x86_64' "$temporary/preview_fallback/output"
assert_contains '/releases?per_page=100' "$temporary/preview_fallback/requests"

mixed_json='[{"tag_name":"v1.1.0-rc.1","draft":false,"prerelease":true},{"tag_name":"v1.0.0","draft":false,"prerelease":false}]'
run_install stable_fallback fail '' "$mixed_json" ""
assert_contains 'normfix v1.0.0 for Linux x86_64' "$temporary/stable_fallback/output"

unexpected_preview='{"tag_name":"v1.1.0-rc.1","draft":false,"prerelease":true}'
run_install reject_preview ok "$unexpected_preview" "$mixed_json" ""
assert_contains 'normfix v1.0.0 for Linux x86_64' "$temporary/reject_preview/output"

run_install exact fail '' '[]' 'v9.9.9-rc.9'
assert_contains 'normfix v9.9.9-rc.9 for Linux x86_64' "$temporary/exact/output"
if grep -Fq '/api.github.com/' "$temporary/exact/requests"; then
    printf '%s\n' 'exact install unexpectedly consulted an update channel' >&2
    exit 1
fi

draft_then_preview='[{"tag_name":"v2.0.0","draft":true,"prerelease":false},{"tag_name":"v1.2.0-rc.1","draft":false,"prerelease":true}]'
run_install skip_draft fail '' "$draft_then_preview" ""
assert_contains 'normfix v1.2.0-rc.1 for Linux x86_64' "$temporary/skip_draft/output"

invalid_dir="$temporary/invalid_version"
mkdir -p "$invalid_dir/bin"
if env \
    HOME="$invalid_dir/home" \
    PATH="$tools:$PATH" \
    NORMFIX_BIN_DIR="$invalid_dir/bin" \
    NORMFIX_VERSION='../main' \
    TEST_REQUEST_LOG="$invalid_dir/requests" \
    TEST_LATEST_STATUS=fail \
    TEST_LATEST_JSON='' \
    TEST_RELEASES_JSON='[]' \
    sh "$installer" >"$invalid_dir/output" 2>"$invalid_dir/error"; then
    printf '%s\n' 'an invalid exact version unexpectedly installed' >&2
    exit 1
fi
assert_contains "invalid release tag '../main'" "$invalid_dir/error"
if [ -e "$invalid_dir/bin/normfix" ]; then
    printf '%s\n' 'an invalid exact version wrote a binary' >&2
    exit 1
fi

printf '%s\n' 'installer channel tests passed'
