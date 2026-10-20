#!/usr/bin/env bash
set -euo pipefail
set +x

MODE="${1:-package}"
case "$MODE" in package|--publish) ;; *) echo "usage: $0 [package|--publish]" >&2; exit 2 ;; esac
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_BUNDLE="${APP_BUNDLE:-$PROJECT_ROOT/.build/Release/Build/Products/Release/ChipDock.app}"
OUTPUT_DIR="${RELEASE_OUTPUT_DIR:-$PROJECT_ROOT/.build/ReleaseAssets}"
SPARKLE_BIN="${SPARKLE_BIN:-$PROJECT_ROOT/.build/SourcePackages/artifacts/sparkle/Sparkle/bin}"
GITHUB_REPOSITORY="${GITHUB_REPOSITORY:-tangxiangong/flashdesk}"
WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/flashdesk-release.XXXXXX")"
trap 'rm -rf "$WORK_DIR"' EXIT
mkdir -p "$OUTPUT_DIR" "$WORK_DIR/archives"
[[ -x "$APP_BUNDLE/Contents/MacOS/ChipDock" && -x "$SPARKLE_BIN/generate_appcast" ]] || {
    echo 'error: Build the Release app and resolve Sparkle before packaging' >&2; exit 1;
}
APP_VERSION="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$APP_BUNDLE/Contents/Info.plist")"
RELEASE_TAG="${RELEASE_TAG:-v$APP_VERSION}"
[[ "$RELEASE_TAG" == "v$APP_VERSION" && "$APP_VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || {
    echo 'error: Release tag must match the stable application version' >&2; exit 1;
}
python3 - "$APP_BUNDLE/Contents/Info.plist" "$GITHUB_REPOSITORY" <<'PYCODE'
import plistlib, sys
with open(sys.argv[1], 'rb') as source:
    info = plistlib.load(source)
feeds = [info.get('SUFeedURL')] + info.get('SUFallbackFeedURLs', [])
expected = f'https://github.com/{sys.argv[2]}/releases/download/update-feed/appcast.xml'
if expected not in feeds:
    sys.exit('error: Update feeds do not include the release repository')
PYCODE
if [[ "${CI:-false}" == true && -z "${SPARKLE_PRIVATE_ED_KEY:-}" ]]; then
    echo 'error: Configure the SPARKLE_PRIVATE_ED_KEY release secret before publishing' >&2; exit 1
fi
ARCHITECTURES="$(/usr/bin/lipo -archs "$APP_BUNDLE/Contents/MacOS/ChipDock")"
[[ " $ARCHITECTURES " == *" arm64 "* && " $ARCHITECTURES " == *" x86_64 "* ]] || {
    echo 'error: Release app must contain both arm64 and x86_64' >&2; exit 1;
}

ARCHIVE_NAME="ChipDock-$APP_VERSION-macOS-universal.zip"
if [[ -n "${RELEASE_NOTES_FILE:-}" ]]; then
    cargo run --quiet --locked --manifest-path "$PROJECT_ROOT/Cargo.toml" -p flashdesk-release-notes -- \
        macos "$RELEASE_NOTES_FILE" "$WORK_DIR/archives/${ARCHIVE_NAME%.zip}.md"
fi

# Re-sign nested Sparkle components inside-out for certificate-free distribution.
# Preserve the vendor Downloader service's sandbox entitlements.
while IFS= read -r -d '' EXECUTABLE; do
    /usr/bin/codesign --force --sign - "$EXECUTABLE"
done < <(find "$APP_BUNDLE/Contents/Frameworks/Sparkle.framework" -type f -name Autoupdate -print0)
while IFS= read -r -d '' NESTED_BUNDLE; do
    if [[ "$NESTED_BUNDLE" == */Downloader.xpc ]]; then
        /usr/bin/codesign --force --preserve-metadata=entitlements --sign - "$NESTED_BUNDLE"
    else
        /usr/bin/codesign --force --sign - "$NESTED_BUNDLE"
    fi
done < <(find "$APP_BUNDLE/Contents/Frameworks/Sparkle.framework" -type d \( -name '*.xpc' -o -name '*.app' \) -depth -print0)
/usr/bin/codesign --force --sign - "$APP_BUNDLE/Contents/Frameworks/Sparkle.framework"
/usr/bin/codesign --force --sign - "$APP_BUNDLE"
/usr/bin/codesign --verify --deep --strict "$APP_BUNDLE"

ARCHIVE="$WORK_DIR/archives/$ARCHIVE_NAME"
/usr/bin/ditto -c -k --sequesterRsrc --keepParent "$APP_BUNDLE" "$ARCHIVE"
DOWNLOAD_PREFIX="https://github.com/$GITHUB_REPOSITORY/releases/download/$RELEASE_TAG/"
APPCAST_ARGS=(--maximum-deltas 0 --embed-release-notes --download-url-prefix "$DOWNLOAD_PREFIX"
    --link "https://github.com/$GITHUB_REPOSITORY/releases/tag/$RELEASE_TAG"
    -o "$OUTPUT_DIR/appcast.xml" "$WORK_DIR/archives")
if [[ -n "${SPARKLE_PRIVATE_ED_KEY:-}" ]]; then
    printf '%s' "$SPARKLE_PRIVATE_ED_KEY" | "$SPARKLE_BIN/generate_appcast" --ed-key-file - "${APPCAST_ARGS[@]}"
else
    "$SPARKLE_BIN/generate_appcast" --account com.xiaoyu.flashdesk "${APPCAST_ARGS[@]}"
fi
xcrun swiftc -swift-version 6 "$PROJECT_ROOT/script/verify_update.swift" -o "$WORK_DIR/verify_update"
"$WORK_DIR/verify_update" "$OUTPUT_DIR/appcast.xml" "$ARCHIVE" "$APP_BUNDLE/Contents/Info.plist" "$DOWNLOAD_PREFIX"
cp "$ARCHIVE" "$OUTPUT_DIR/$ARCHIVE_NAME"

if [[ "$MODE" == --publish ]]; then
    [[ -n "${GH_TOKEN:-}" ]] || { echo 'error: GH_TOKEN is required for publication' >&2; exit 1; }
    FEED_EXISTS=false
    if gh api "repos/$GITHUB_REPOSITORY/releases/tags/update-feed" > "$WORK_DIR/feed-release.json" 2> "$WORK_DIR/feed-error.log"; then
        FEED_EXISTS=true
        HAS_FEED="$(python3 -c 'import json,sys; print(any(a["name"] == "appcast.xml" for a in json.load(sys.stdin)["assets"]))' < "$WORK_DIR/feed-release.json")"
        if [[ "$HAS_FEED" == True ]]; then
            gh release download update-feed --repo "$GITHUB_REPOSITORY" --pattern appcast.xml --dir "$WORK_DIR"
            "$WORK_DIR/verify_update" "$OUTPUT_DIR/appcast.xml" "$ARCHIVE" "$APP_BUNDLE/Contents/Info.plist" "$DOWNLOAD_PREFIX" "$WORK_DIR/appcast.xml"
        fi
    elif [[ "$(cat "$WORK_DIR/feed-error.log")" != *"(HTTP 404)"* ]]; then
        cat "$WORK_DIR/feed-error.log" >&2
        exit 1
    fi
    gh api "repos/$GITHUB_REPOSITORY/releases/tags/$RELEASE_TAG" > "$WORK_DIR/release.json"
    EXISTING_DIGEST="$(python3 - "$WORK_DIR/release.json" "$ARCHIVE_NAME" <<'PYCODE'
import json, sys
with open(sys.argv[1]) as source:
    asset = next((a for a in json.load(source)['assets'] if a['name'] == sys.argv[2]), None)
print('absent' if asset is None else asset.get('digest') or 'unknown')
PYCODE
)"
    if [[ "$EXISTING_DIGEST" == absent ]]; then
        gh release upload "$RELEASE_TAG" "$OUTPUT_DIR/$ARCHIVE_NAME" --repo "$GITHUB_REPOSITORY"
    else
        LOCAL_DIGEST="sha256:$(shasum -a 256 "$OUTPUT_DIR/$ARCHIVE_NAME" | awk '{print $1}')"
        [[ "$EXISTING_DIGEST" == "$LOCAL_DIGEST" ]] || {
            echo 'error: Refusing to overwrite a published archive with different bytes' >&2; exit 1;
        }
    fi
    if [[ "$FEED_EXISTS" == false ]]; then
        gh release create update-feed --repo "$GITHUB_REPOSITORY" --target "${GITHUB_SHA:?}" \
            --title 'ChipDock update feed' --notes 'Sparkle update metadata.' --prerelease --latest=false
    fi
    # Publish the feed only after the immutable archive is available.
    gh release upload update-feed "$OUTPUT_DIR/appcast.xml" --repo "$GITHUB_REPOSITORY" --clobber
fi
printf 'Packaged and verified: %s\n' "$OUTPUT_DIR/$ARCHIVE_NAME"
