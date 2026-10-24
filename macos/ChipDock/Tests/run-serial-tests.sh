#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
TEST_BUILD_DIR="$PROJECT_ROOT/.build/SerialTests"
mkdir -p "$TEST_BUILD_DIR"
xcrun swiftc -swift-version 6 -parse-as-library \
    -module-cache-path "$TEST_BUILD_DIR/ModuleCache" \
    "$PROJECT_ROOT/macos/ChipDock/Sources/ChipDock/SerialModels.swift" \
    "$PROJECT_ROOT/macos/ChipDock/Sources/ChipDock/SerialAutoSender.swift" \
    "$PROJECT_ROOT/macos/ChipDock/Tests/SerialSendTests.swift" \
    -o "$TEST_BUILD_DIR/SerialSendTests"
"$TEST_BUILD_DIR/SerialSendTests"
