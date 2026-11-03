#!/usr/bin/env bash
# Run vanilla's built-in data generator and store its reports in
# .vanilla/reports/, where the build scripts read them.
#
# Usage:
#   scripts/gen-reports.sh
#
# Needs the pinned jar in .vanilla/ (run scripts/fetch-vanilla.sh first) and
# the Java major version pinned in vanilla.toml. The reports are derived from
# Mojang's game and must never be committed or published as CI artifacts.

set -euo pipefail

# shellcheck source=scripts/lib/common.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib/common.sh"

version="$(pin version)"
jar="$root/.vanilla/server-$version.jar"
reports="$root/.vanilla/reports"

[ -f "$jar" ] || fail "missing $jar (run scripts/fetch-vanilla.sh first)"
expect "server.jar sha256" "$(sha256_of "$jar")" "$(pin sha256)"

command -v java >/dev/null || fail "java not found (Java $(pin java) is required)"
java_major="$(java -version 2>&1 | sed -n 's/.*version "\([0-9]*\).*/\1/p' | head -n 1)"
expect "java major version" "$java_major" "$(pin java)"

# The bundler unpacks its libraries into the working directory, so run it in a
# throwaway directory.
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

echo "running the vanilla data generator..."
if ! (cd "$work" && java -DbundlerMainClass=net.minecraft.data.Main \
    -jar "$(native_path "$jar")" --reports --output "$(native_path "$work/out")" \
    >"$work/generator.log" 2>&1); then
    tail -n 50 "$work/generator.log" >&2
    fail "the data generator failed"
fi
[ -d "$work/out/reports" ] || fail "the data generator produced no reports"

rm -rf "$reports"
mkdir -p "$root/.vanilla"
mv "$work/out/reports" "$reports"

# Fingerprint: SHA-256 over the sorted list of "<sha256>  <path>" lines.
# Pinning it proves the generator output is identical on every machine.
fingerprint="$(cd "$reports" && find . -type f -print0 | LC_ALL=C sort -z \
    | xargs -0 sha256sum | sed 's/ \*/  /' | sha256sum | cut -d ' ' -f 1)"
expect "reports fingerprint" "$fingerprint" "$(pin reports_sha256)"
echo "generated: $reports ($(find "$reports" -type f | wc -l | tr -d ' ') files)"
