# Shared helpers for the scripts in scripts/. Source this file; don't run it.
# shellcheck shell=bash

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
pins="$root/vanilla.toml"

fail() {
    echo "error: $*" >&2
    exit 1
}

# Read `key = value` or `key = "value"` from vanilla.toml.
pin() {
    local value
    value="$(sed -n "s/^$1 = \"\{0,1\}\([^\"]*\)\"\{0,1\}\$/\1/p" "$pins" | head -n 1)"
    [ -n "$value" ] || fail "missing key '$1' in vanilla.toml"
    printf '%s' "$value"
}

# expect NAME ACTUAL EXPECTED
expect() {
    [ "$2" = "$3" ] || fail "$1 mismatch: expected '$3', got '$2'"
    echo "ok: $1 = $2"
}

sha1_of() { sha1sum "$1" | cut -d ' ' -f 1; }
sha256_of() { sha256sum "$1" | cut -d ' ' -f 1; }

# Convert a path for native Windows programs (such as java) under Git Bash.
native_path() {
    if command -v cygpath >/dev/null; then
        cygpath -w "$1"
    else
        printf '%s' "$1"
    fi
}
