#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TEST_BUILD_DIR="$PROJECT_ROOT/.build/SerialTests"
mkdir -p "$TEST_BUILD_DIR"
xcrun swiftc -swift-version 6 -parse-as-library \
    "$PROJECT_ROOT/macos/ChipDock/Models/SerialModels.swift" \
    "$PROJECT_ROOT/macos/ChipDock/Stores/SerialAutoSender.swift" \
    "$PROJECT_ROOT/script/tests/SerialSendTests.swift" \
    -o "$TEST_BUILD_DIR/SerialSendTests"
"$TEST_BUILD_DIR/SerialSendTests"
