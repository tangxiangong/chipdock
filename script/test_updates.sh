#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TEST_BUILD_DIR="$PROJECT_ROOT/.build/UpdateTests"
mkdir -p "$TEST_BUILD_DIR"
xcrun swiftc -swift-version 6 "$PROJECT_ROOT/script/verify_update.swift" -o "$TEST_BUILD_DIR/verify_update"
xcrun swiftc -swift-version 6 -parse-as-library "$PROJECT_ROOT/script/tests/UpdateFeedTests.swift" -o "$TEST_BUILD_DIR/UpdateFeedTests"
"$TEST_BUILD_DIR/UpdateFeedTests" "$TEST_BUILD_DIR/verify_update"
