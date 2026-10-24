#!/usr/bin/env bash
set -euo pipefail
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
TEST_BUILD_DIR="$PROJECT_ROOT/.build/UpdateMirrorTests"
SPARKLE_FRAMEWORK_DIR="$PROJECT_ROOT/.build/SourcePackages/artifacts/sparkle/Sparkle/Sparkle.xcframework/macos-arm64_x86_64"
mkdir -p "$TEST_BUILD_DIR"
xcrun swiftc -swift-version 6 -warnings-as-errors -parse-as-library \
    -target "$(uname -m)-apple-macosx14.0" -module-cache-path "$TEST_BUILD_DIR/ModuleCache" \
    -F "$SPARKLE_FRAMEWORK_DIR" -framework Sparkle \
    -Xlinker -rpath -Xlinker "$SPARKLE_FRAMEWORK_DIR" \
    "$PROJECT_ROOT/macos/ChipDock/Sources/ChipDock/UpdateService.swift" \
    "$PROJECT_ROOT/macos/ChipDock/Tests/UpdateMirrorPolicyTests.swift" \
    -o "$TEST_BUILD_DIR/UpdateMirrorPolicyTests"
"$TEST_BUILD_DIR/UpdateMirrorPolicyTests"
