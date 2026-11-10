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
target_dir="${CARGO_TARGET_DIR:-target}"
cargo build --locked --profile "$build_profile" --target-dir "$target_dir" -p chipdock-gpui --bin chipdock-gpui
package_id="$(cargo pkgid --locked -p chipdock-gpui)"
version="${package_id##*@}"
host="${CARGO_BUILD_TARGET:-$(rustc -vV | sed -n 's/^host: //p')}"
binary_dir="$target_dir/${CARGO_BUILD_TARGET:+$CARGO_BUILD_TARGET/}$profile"
bash linux/package.sh "$binary_dir/chipdock-gpui" "$version" "$host" "${OUTPUT_DIR:-dist/gpui}"
