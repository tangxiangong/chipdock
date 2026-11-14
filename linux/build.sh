#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
profile="${PROFILE:-release}"
case "$profile" in
  release) build_profile=release ;;
  debug) build_profile=dev ;;
  *) echo "Unsupported PROFILE: $profile" >&2; exit 1 ;;
esac
if [[ "$(uname -s)" == Darwin ]]; then
  export MACOSX_DEPLOYMENT_TARGET=15.0
fi
export SOURCE_DATE_EPOCH="${SOURCE_DATE_EPOCH:-$(git log -1 --format=%ct)}"
build_output="$(mktemp)"
trap 'rm -f "$build_output"' EXIT
# Cargo's artifact path also works with the shared-cache cargo that ignores target-dir.
cargo build --locked --profile "$build_profile" -p chipdock-nsh --bin ChipDock \
  --message-format=json-render-diagnostics > "$build_output"
binary="$(python3 - "$build_output" <<'PY'
import json
import sys
from pathlib import Path

executables = []
for line in Path(sys.argv[1]).read_text().splitlines():
    try:
        artifact = json.loads(line)
    except json.JSONDecodeError:
        continue
    if (artifact.get("reason") == "compiler-artifact"
            and artifact.get("target", {}).get("name") == "ChipDock"
            and artifact.get("executable")):
        executables.append(artifact["executable"])
if not executables:
    sys.exit("Cargo did not report the ChipDock executable")
print(executables[-1])
PY
)"
package_id="$(cargo pkgid --locked -p chipdock-nsh)"
version="${package_id##*@}"
host="${CARGO_BUILD_TARGET:-$(rustc -vV | sed -n 's/^host: //p')}"
bash linux/package.sh "$binary" "$version" "$host" "${OUTPUT_DIR:-dist/gpui}"
