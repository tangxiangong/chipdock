#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:$PATH"
export MACOSX_DEPLOYMENT_TARGET=14.0
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$PROJECT_ROOT/target}"
TEST_BUILD_DIR="$PROJECT_ROOT/.build/BackendTests"
BINDINGS_DIR="$PROJECT_ROOT/bindings/generated/swift"
mkdir -p "$BINDINGS_DIR"

# macOS ships the UniFFI bridge; the Interoptopus crates are tested on Windows.
cargo test --manifest-path "$PROJECT_ROOT/Cargo.toml" --package chipdock-core --package chipdock-service --package chipdock-ffi --package chipdock-bindgen --all-targets --locked
cargo build --manifest-path "$PROJECT_ROOT/Cargo.toml" --package chipdock-ffi --lib --locked
bash "$PROJECT_ROOT/bindings/generate-swift.sh" "$CARGO_TARGET_DIR/debug/libchipdock_ffi.dylib" "$BINDINGS_DIR"
cp "$BINDINGS_DIR/ChipDockCoreFFI.modulemap" "$BINDINGS_DIR/module.modulemap"

xcrun swiftc -swift-version 6 -parse-as-library -I "$BINDINGS_DIR" \
    "$BINDINGS_DIR/ChipDockCore.swift" \
    "$PROJECT_ROOT/macos/ChipDock/Tests/RustInteropTests.swift" \
    "$CARGO_TARGET_DIR/debug/libchipdock_ffi.a" \
    -framework IOKit -framework CoreFoundation -framework Security -framework SystemConfiguration \
    -liconv -lresolv -lc++ -o "$TEST_BUILD_DIR/BackendInteropTests"

# A Foundation-only executable: no app/window startup and no hardware enumeration.
"$TEST_BUILD_DIR/BackendInteropTests"
