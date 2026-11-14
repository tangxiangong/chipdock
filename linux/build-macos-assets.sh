#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 1 || $# -gt 2 ]]; then
  echo 'Usage: build-macos-assets.sh <output-directory> [<ChipDock.app>]' >&2
  exit 1
fi
[[ "$(uname -s)" == Darwin ]] || { echo 'macOS icon compilation requires macOS and Xcode 26+.' >&2; exit 1; }
asset_repo="$(cd "$(dirname "$0")/.." && pwd)"
asset_output="$1"
asset_bundle="${2:-}"
asset_work="$(mktemp -d)"
trap 'rm -rf "$asset_work"' EXIT
if ! asset_compiler="$(xcrun --sdk macosx --find actool 2>/dev/null)"; then
  echo 'The full Xcode 26+ installation and its actool are required; Command Line Tools alone are insufficient.' >&2
  exit 1
fi
mkdir -p "$asset_output"
asset_output="$(cd "$asset_output" && pwd)"
if [[ -n "$asset_bundle" ]]; then
  [[ -d "$asset_bundle" ]] || { echo "Not an app bundle: $asset_bundle" >&2; exit 1; }
  asset_bundle="$(cd "$asset_bundle" && pwd)"
fi

# This is the native target's CompileAssetCatalog command, without Swift/ObjC symbol generation.
"$asset_compiler" "$asset_repo/assets/app-icons/ChipDock.icon" \
  --compile "$asset_output" --output-format human-readable-text --notices --warnings \
  --export-dependency-info "$asset_work/dependencies" \
  --output-partial-info-plist "$asset_output/IconInfo.plist" \
  --app-icon ChipDock --enable-on-demand-resources NO --development-region zh-Hans \
  --target-device mac --minimum-deployment-target 14.0 --platform macosx
[[ -s "$asset_output/Assets.car" && -s "$asset_output/ChipDock.icns" ]] || {
  echo 'actool did not generate the canonical macOS asset catalog and icon.' >&2
  exit 1
}
iconutil -c iconset "$asset_output/ChipDock.icns" -o "$asset_work/ChipDock.iconset"
asset_png="$(python3 - "$asset_work/ChipDock.iconset" <<'PY'
import struct
import sys
from pathlib import Path

images = []
for path in Path(sys.argv[1]).glob("*.png"):
    data = path.read_bytes()
    if data[:8] == b"\x89PNG\r\n\x1a\n":
        width, height = struct.unpack(">II", data[16:24])
        if width == height:
            images.append((width, path))
if not images:
    sys.exit("The canonical ICNS contains no square PNG representation")
print(max(images, key=lambda image: image[0])[1])
PY
)"
install -m 644 "$asset_png" "$asset_output/ChipDock.png"

if [[ -n "$asset_bundle" ]]; then
  [[ -f "$asset_bundle/Contents/Info.plist" ]] || { echo "Not an app bundle: $asset_bundle" >&2; exit 1; }
  mkdir -p "$asset_bundle/Contents/Resources"
  install -m 644 "$asset_output/Assets.car" "$asset_output/ChipDock.icns" "$asset_bundle/Contents/Resources/"
  python3 - "$asset_output/IconInfo.plist" "$asset_bundle/Contents/Info.plist" <<'PY'
import plistlib
import sys
from pathlib import Path

metadata = plistlib.loads(Path(sys.argv[1]).read_bytes())
info_path = Path(sys.argv[2])
info = plistlib.loads(info_path.read_bytes())
for key in ("CFBundleIconName", "CFBundleIconFile"):
    if metadata.get(key) != "ChipDock":
        sys.exit(f"Unexpected actool icon identity: {key}={metadata.get(key)!r}")
    info[key] = metadata[key]
info_path.write_bytes(plistlib.dumps(info))
PY
  plutil -lint "$asset_bundle/Contents/Info.plist"
fi
