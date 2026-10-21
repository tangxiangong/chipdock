#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TEST_BUILD_DIR="$PROJECT_ROOT/.build/MacInteractionTests"
BRIDGE_LIBRARY="$PROJECT_ROOT/.build/Rust/libchipdock_core.a"
mkdir -p "$TEST_BUILD_DIR"
if [[ ! -f "$BRIDGE_LIBRARY" ]]; then
    echo 'Build the macOS Rust bridge first with script/build_rust.sh.' >&2
    exit 1
fi
xcrun swiftc -swift-version 6 -parse-as-library \
    -module-cache-path "$TEST_BUILD_DIR/ModuleCache" \
    -I "$PROJECT_ROOT/macos/ChipDock/Generated" \
    "$PROJECT_ROOT/macos/ChipDock/Generated/ChipDockCore.swift" \
    "$PROJECT_ROOT/macos/ChipDock/Models/WorkspaceModels.swift" \
    "$PROJECT_ROOT/macos/ChipDock/Models/TargetConfiguration.swift" \
    "$PROJECT_ROOT/macos/ChipDock/Models/TerminalTableState.swift" \
    "$PROJECT_ROOT/macos/ChipDock/Services/RustService.swift" \
    "$PROJECT_ROOT/macos/ChipDock/Stores/WorkbenchStore.swift" \
    "$PROJECT_ROOT/macos/ChipDock/Stores/WorkbenchOperations.swift" \
    "$PROJECT_ROOT/macos/ChipDock/Support/Formatting.swift" \
    "$PROJECT_ROOT/macos/ChipDock/Support/ExportDocument.swift" \
    "$PROJECT_ROOT/script/tests/MacInteractionTests.swift" \
    "$BRIDGE_LIBRARY" \
    -framework IOKit -framework CoreFoundation -framework Security -framework SystemConfiguration \
    -liconv -lresolv -lc++ -o "$TEST_BUILD_DIR/MacInteractionTests"
"$TEST_BUILD_DIR/MacInteractionTests"
