#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
TEST_BUILD_DIR="$PROJECT_ROOT/.build/MacInteractionTests"
BRIDGE_LIBRARY="${CHIPDOCK_BRIDGE_LIBRARY:-$PROJECT_ROOT/target/debug/libchipdock_ffi.a}"
mkdir -p "$TEST_BUILD_DIR"
if [[ ! -f "$BRIDGE_LIBRARY" ]]; then
    echo 'Build the macOS Rust bridge first with macos/build-bindings.sh.' >&2
    exit 1
fi
xcrun swiftc -swift-version 6 -parse-as-library \
    -module-cache-path "$TEST_BUILD_DIR/ModuleCache" \
    -I "$PROJECT_ROOT/bindings/generated/swift" \
    "$PROJECT_ROOT/bindings/generated/swift/ChipDockCore.swift" \
    "$PROJECT_ROOT/macos/ChipDock/Sources/ChipDock/WorkspaceModels.swift" \
    "$PROJECT_ROOT/macos/ChipDock/Sources/ChipDock/TargetConfiguration.swift" \
    "$PROJECT_ROOT/macos/ChipDock/Sources/ChipDock/TerminalTableState.swift" \
    "$PROJECT_ROOT/macos/ChipDock/Sources/ChipDock/RustService.swift" \
    "$PROJECT_ROOT/macos/ChipDock/Sources/ChipDock/WorkbenchStore.swift" \
    "$PROJECT_ROOT/macos/ChipDock/Sources/ChipDock/WorkbenchOperations.swift" \
    "$PROJECT_ROOT/macos/ChipDock/Sources/ChipDock/Formatting.swift" \
    "$PROJECT_ROOT/macos/ChipDock/Sources/ChipDock/ExportDocument.swift" \
    "$PROJECT_ROOT/macos/ChipDock/Tests/MacInteractionTests.swift" \
    "$BRIDGE_LIBRARY" \
    -framework IOKit -framework CoreFoundation -framework Security -framework SystemConfiguration \
    -liconv -lresolv -lc++ -o "$TEST_BUILD_DIR/MacInteractionTests"
"$TEST_BUILD_DIR/MacInteractionTests"
