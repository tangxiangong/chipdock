#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:$PATH"
export MACOSX_DEPLOYMENT_TARGET=14.0
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$PROJECT_ROOT/target}"
TEST_BUILD_DIR="$PROJECT_ROOT/.build/BackendTests"
BINDINGS_DIR="$TEST_BUILD_DIR/Bindings"
mkdir -p "$BINDINGS_DIR"

cargo test --manifest-path "$PROJECT_ROOT/Cargo.toml" --workspace --all-targets --all-features --locked
cargo build --manifest-path "$PROJECT_ROOT/Cargo.toml" --package flashdesk-core --lib --locked
cargo run --manifest-path "$PROJECT_ROOT/Cargo.toml" --package flashdesk-bindgen --locked -- \
    generate --library "$CARGO_TARGET_DIR/debug/libflashdesk_core.dylib" --language swift --out-dir "$BINDINGS_DIR"
cp "$BINDINGS_DIR/FlashDeskCoreFFI.modulemap" "$BINDINGS_DIR/module.modulemap"

xcrun swiftc -swift-version 6 -parse-as-library -I "$BINDINGS_DIR" \
    "$BINDINGS_DIR/FlashDeskCore.swift" \
    "$PROJECT_ROOT/crates/flashdesk-core/tests/swift/CoreSmoke.swift" \
    "$CARGO_TARGET_DIR/debug/libflashdesk_core.a" \
    -framework IOKit -framework CoreFoundation -framework Security -framework SystemConfiguration \
    -liconv -lresolv -lc++ -o "$TEST_BUILD_DIR/BackendInteropTests"

# A Foundation-only executable: no app/window startup and no hardware enumeration.
"$TEST_BUILD_DIR/BackendInteropTests"
