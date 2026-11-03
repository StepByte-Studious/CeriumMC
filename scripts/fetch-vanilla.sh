#!/usr/bin/env bash
# Download and verify the pinned vanilla server jar described in vanilla.toml.
#
# Usage:
#   scripts/fetch-vanilla.sh [--dest DIR]   Download (or reuse) and verify the
#                                            jar in DIR (default: .vanilla).
#                                            Requires curl, jq, unzip and
#                                            sha1sum/sha256sum.
#   scripts/fetch-vanilla.sh --check-cached  Offline check that the local
#                                            .cached/ research copy matches
#                                            the pinned version.
#
# The jar is Mojang's property. It is downloaded for local use and CI only and
# must never be committed (see CONTRIBUTING.md).

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
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

version="$(pin version)"
inner_jar_sha256="$(pin inner_jar_sha256)"

check_cached() {
    local cached="$root/.cached"
    [ -d "$cached" ] || fail "no .cached/ directory found"
    local inner="$cached/META-INF/versions/$version/server-$version.jar"
    [ -f "$inner" ] || fail "missing $inner"
    expect "cached inner jar sha256" "$(sha256_of "$inner")" "$inner_jar_sha256"
    grep -Eq "\"id\": *\"$version\"" "$cached/version.json" \
        || fail ".cached/version.json is not version $version"
    grep -Eq "\"protocol_version\": *$(pin protocol)[,[:space:]]" "$cached/version.json" \
        || fail ".cached/version.json protocol does not match"
    grep -Eq "\"world_version\": *$(pin world_version)[,[:space:]]" "$cached/version.json" \
        || fail ".cached/version.json world version does not match"
    echo "ok: .cached/ is vanilla $version"
}

# Confirm the pins still agree with Mojang's live launcher metadata.
check_metadata() {
    local tmp="$1"
    local manifest_url="https://piston-meta.mojang.com/mc/game/version_manifest_v2.json"
    curl -sSfL --retry 3 "$manifest_url" -o "$tmp/manifest.json"
    local entry
    entry="$(jq -c --arg v "$version" '.versions[] | select(.id == $v)' "$tmp/manifest.json")"
    [ -n "$entry" ] || fail "version $version not found in Mojang's manifest"
    expect "manifest metadata url" "$(jq -r .url <<<"$entry")" "$(pin metadata_url)"
    expect "manifest metadata sha1" "$(jq -r .sha1 <<<"$entry")" "$(pin metadata_sha1)"

    curl -sSfL --retry 3 "$(pin metadata_url)" -o "$tmp/version.json"
    expect "metadata file sha1" "$(sha1_of "$tmp/version.json")" "$(pin metadata_sha1)"
    expect "metadata server url" "$(jq -r .downloads.server.url "$tmp/version.json")" "$(pin url)"
    expect "metadata server sha1" "$(jq -r .downloads.server.sha1 "$tmp/version.json")" "$(pin sha1)"
    expect "metadata server size" "$(jq -r .downloads.server.size "$tmp/version.json")" "$(pin size)"
    expect "metadata java version" "$(jq -r .javaVersion.majorVersion "$tmp/version.json")" "$(pin java)"
}

# Check the downloaded jar and the version facts embedded in it.
check_jar() {
    local jar="$1"
    expect "server.jar size" "$(wc -c <"$jar" | tr -d ' ')" "$(pin size)"
    expect "server.jar sha1" "$(sha1_of "$jar")" "$(pin sha1)"
    expect "server.jar sha256" "$(sha256_of "$jar")" "$(pin sha256)"

    local info
    info="$(unzip -p "$jar" version.json)"
    expect "embedded id" "$(jq -r .id <<<"$info")" "$version"
    expect "embedded protocol" "$(jq -r .protocol_version <<<"$info")" "$(pin protocol)"
    expect "embedded world version" "$(jq -r .world_version <<<"$info")" "$(pin world_version)"
    expect "embedded data pack" \
        "$(jq -r '"\(.pack_version.data_major).\(.pack_version.data_minor)"' <<<"$info")" "$(pin data_pack)"
    expect "embedded resource pack" \
        "$(jq -r '"\(.pack_version.resource_major).\(.pack_version.resource_minor)"' <<<"$info")" \
        "$(pin resource_pack)"
    expect "embedded java version" "$(jq -r .java_version <<<"$info")" "$(pin java)"

    expect "versions.list inner sha256" \
        "$(unzip -p "$jar" META-INF/versions.list | cut -f 1)" "$inner_jar_sha256"
    expect "inner jar sha256" \
        "$(unzip -p "$jar" "META-INF/versions/$version/server-$version.jar" | sha256sum | cut -d ' ' -f 1)" \
        "$inner_jar_sha256"
}

fetch() {
    local dest="$1"
    for tool in curl jq unzip sha1sum sha256sum; do
        command -v "$tool" >/dev/null || fail "required tool not found: $tool"
    done
    mkdir -p "$dest"
    # Global so the EXIT trap can still see it after this function returns.
    tmp="$(mktemp -d)"
    trap 'rm -rf "$tmp"' EXIT

    check_metadata "$tmp"

    local jar="$dest/server-$version.jar"
    if [ -f "$jar" ] && [ "$(sha256_of "$jar")" = "$(pin sha256)" ]; then
        echo "ok: reusing cached $jar"
        check_jar "$jar"
    else
        curl -sSfL --retry 3 "$(pin url)" -o "$tmp/server.jar"
        # Verify before moving, so a bad download never lands in the cache.
        check_jar "$tmp/server.jar"
        mv "$tmp/server.jar" "$jar"
    fi
    echo "verified: $jar"
}

case "${1:-}" in
    --check-cached)
        check_cached
        ;;
    --dest)
        [ -n "${2:-}" ] || fail "--dest needs a directory"
        fetch "$2"
        ;;
    "")
        fetch "$root/.vanilla"
        ;;
    *)
        fail "unknown argument: $1"
        ;;
esac
