#!/usr/bin/env bash
set -euo pipefail

MODE="${1:-run}"
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DERIVED_DATA="${DERIVED_DATA_PATH:-$PROJECT_ROOT/.build/Xcode}"
APP_BUNDLE="$DERIVED_DATA/Build/Products/Debug/FlashDesk.app"
APP_BINARY="$APP_BUNDLE/Contents/MacOS/FlashDesk"

case "$MODE" in
    run|--debug|debug|--logs|logs|--telemetry|telemetry|--verify|verify) ;;
    *) echo "usage: $0 [run|--debug|--logs|--telemetry|--verify]" >&2; exit 2 ;;
esac

# A standard Quit event lets the app refuse to exit during a hardware operation.
if pgrep -x FlashDesk >/dev/null; then
    /usr/bin/osascript -e 'tell application id "com.xiaoyu.flashdesk" to quit' || true
    for ATTEMPT in {1..20}; do
        if ! pgrep -x FlashDesk >/dev/null; then break; fi
        sleep 0.1
    done
    if pgrep -x FlashDesk >/dev/null; then
        echo 'error: FlashDesk is still running. Finish the active operation and close it before rebuilding.' >&2
        exit 1
    fi
fi

xcodebuild -project "$PROJECT_ROOT/FlashDesk.xcodeproj" -scheme FlashDesk \
    -configuration Debug -destination 'platform=macOS' -derivedDataPath "$DERIVED_DATA" build

case "$MODE" in
    --debug|debug) lldb -- "$APP_BINARY" ;;
    --logs|logs)
        /usr/bin/open -n "$APP_BUNDLE"
        /usr/bin/log stream --info --style compact --predicate 'process == "FlashDesk"'
        ;;
    --telemetry|telemetry)
        /usr/bin/open -n "$APP_BUNDLE"
        /usr/bin/log stream --info --style compact --predicate 'subsystem == "com.xiaoyu.flashdesk"'
        ;;
    --verify|verify)
        /usr/bin/open -n "$APP_BUNDLE"
        sleep 1
        pgrep -x FlashDesk >/dev/null
        echo "Running: $APP_BUNDLE"
        ;;
    *) /usr/bin/open -n "$APP_BUNDLE" ;;
esac
