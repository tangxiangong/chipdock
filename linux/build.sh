#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
output="${OUTPUT_DIR:-dist/gpui}"
mkdir -p "$output"
version="$(python3 -c 'import json; print(json.load(open("linux/nsh.json"))["version"])')"
package_id="$(cargo pkgid --locked -p chipdock-nsh)"
[[ "$version" == "${package_id##*@}" ]] || { echo 'NSH and Cargo versions differ' >&2; exit 1; }
desktop_entry=.build/NSHDesktopEntry
case "$(uname -s)" in
  Darwin)
    if [[ -e "$output/ChipDock.app" ]]; then
      existing_id="$(plutil -extract CFBundleIdentifier raw -o - "$output/ChipDock.app/Contents/Info.plist")"
      expected_id="$(python3 -c 'import json; print(json.load(open("linux/nsh.json"))["id"])')"
      [[ "$existing_id" == "$expected_id" ]] || {
        echo "Output already contains a different application: $output/ChipDock.app" >&2; exit 1
      }
    fi
    assets="$(mktemp -d)"
    trap 'rm -rf "$assets"' EXIT
    bash linux/build-macos-assets.sh "$assets"
    python3 linux/ota/entry.py desktop "$desktop_entry"
    nsh package "$desktop_entry" --target macos --icon "$assets/ChipDock.png" -o "$output"
    bash linux/build-macos-assets.sh "$assets" "$output/ChipDock.app"
    codesign --force --sign - "$output/ChipDock.app"
    codesign --verify --strict "$output/ChipDock.app"
    arch="$(uname -m)"
    if [[ "$arch" == arm64 ]]; then arch=aarch64; fi
    archive="$output/ChipDock-$version-macos-$arch.tar.gz"
    COPYFILE_DISABLE=1 tar -czf "$archive" -C "$output" ChipDock.app
    ;;
  Linux)
    stage="$(mktemp -d)"
    trap 'rm -rf "$stage"' EXIT
    nsh package linux --target linux -o "$stage"
    bash linux/package.sh "$stage/ChipDock" "$version" "$(uname -m)" "$output"
    ;;
  CYGWIN*|MINGW*|MSYS*)
    python3 linux/ota/entry.py desktop "$desktop_entry"
    nsh package "$desktop_entry" --target windows -o "$output" ;;
  *) echo "Unsupported NSH build host: $(uname -s)" >&2; exit 1 ;;
esac
