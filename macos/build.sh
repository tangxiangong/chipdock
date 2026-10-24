#!/usr/bin/env bash
# The CLI and Just use the same Xcode project as the IDE; this never launches it.
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
case "${CONFIGURATION:-Release}" in
    Debug|debug) CONFIGURATION=Debug ;;
    Release|release) CONFIGURATION=Release ;;
    *) echo 'CONFIGURATION must be Debug or Release' >&2; exit 1 ;;
esac
ARCHITECTURE="${ARCH:-$(uname -m)}"
case "$ARCHITECTURE" in
    arm64|x86_64) ARCHITECTURES="$ARCHITECTURE" ;;
    universal) ARCHITECTURES='arm64 x86_64' ;;
    *) echo 'ARCH must be arm64, x86_64 or universal' >&2; exit 1 ;;
esac
DERIVED_DATA="${DERIVED_DATA_PATH:-$PROJECT_ROOT/target/xcode/$ARCHITECTURE}"
xcodebuild -project "$PROJECT_ROOT/macos/ChipDock.xcodeproj" -scheme ChipDock \
    -configuration "$CONFIGURATION" -destination 'generic/platform=macOS' \
    -derivedDataPath "$DERIVED_DATA" "ARCHS=$ARCHITECTURES" ONLY_ACTIVE_ARCH=NO build
APP_BUNDLE="$DERIVED_DATA/Build/Products/$CONFIGURATION/ChipDock.app"
[[ -x "$APP_BUNDLE/Contents/MacOS/ChipDock" ]] || { echo 'error: Xcode did not produce ChipDock.app' >&2; exit 1; }
if [[ "${BUILD_ONLY:-1}" == 0 ]]; then
    DESTINATION="$PROJECT_ROOT/dist/macos/$ARCHITECTURE/ChipDock.app"
    mkdir -p "$(dirname "$DESTINATION")"
    if [[ -d "$DESTINATION" ]]; then rm -rf "$DESTINATION"; fi
    ditto "$APP_BUNDLE" "$DESTINATION"
    APP_BUNDLE="$DESTINATION"
fi
printf '%s\n' "$APP_BUNDLE"
