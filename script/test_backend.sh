#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:$PATH"
export MACOSX_DEPLOYMENT_TARGET=14.0
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$PROJECT_ROOT/target}"
TEST_BUILD_DIR="$PROJECT_ROOT/.build/BackendTests"
BINDINGS_DIR="$TEST_BUILD_DIR/Bindings"
mkdir -p "$BINDINGS_DIR"

# macOS ships the UniFFI bridge; the Interoptopus crates are tested on Windows.
cargo test --manifest-path "$PROJECT_ROOT/Cargo.toml" --package chipdock-core --package chipdock-bindgen --all-targets --locked
cargo build --manifest-path "$PROJECT_ROOT/Cargo.toml" --package chipdock-core --lib --locked
cargo run --manifest-path "$PROJECT_ROOT/Cargo.toml" --package chipdock-bindgen --locked -- \
    generate --library "$CARGO_TARGET_DIR/debug/libchipdock_core.dylib" --language swift --out-dir "$BINDINGS_DIR"
cp "$BINDINGS_DIR/ChipDockCoreFFI.modulemap" "$BINDINGS_DIR/module.modulemap"

xcrun swiftc -swift-version 6 -parse-as-library -I "$BINDINGS_DIR" \
    "$BINDINGS_DIR/ChipDockCore.swift" \
    "$PROJECT_ROOT/crates/chipdock-core/tests/swift/CoreSmoke.swift" \
    "$CARGO_TARGET_DIR/debug/libchipdock_core.a" \
    -framework IOKit -framework CoreFoundation -framework Security -framework SystemConfiguration \
    -liconv -lresolv -lc++ -o "$TEST_BUILD_DIR/BackendInteropTests"

# A Foundation-only executable: no app/window startup and no hardware enumeration.
"$TEST_BUILD_DIR/BackendInteropTests"
