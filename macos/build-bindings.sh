#!/usr/bin/env bash
# Xcode's RustBindings target generates the Swift module before app compilation.
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:$PATH"
export MACOSX_DEPLOYMENT_TARGET="${MACOSX_DEPLOYMENT_TARGET:-14.0}"
GENERATED_DIR="${CHIPDOCK_BINDINGS_DIR:-$PROJECT_ROOT/bindings/generated/swift}"
RUST_BUILD_DIR="${CHIPDOCK_RUST_TARGET_DIR:-${CARGO_TARGET_DIR:-$PROJECT_ROOT/target}}"
export CARGO_TARGET_DIR="$RUST_BUILD_DIR"
case "${CONFIGURATION:-Debug}" in
    Debug|debug) RUST_PROFILE=debug; CARGO_PROFILE=dev ;;
    Release|release) RUST_PROFILE=release; CARGO_PROFILE=release ;;
    *) echo "error: Unsupported configuration: $CONFIGURATION" >&2; exit 1 ;;
esac
if ! command -v cargo >/dev/null 2>&1; then
    echo 'error: Rust is required. Install it from https://rustup.rs, then build again.' >&2
    exit 1
fi
mkdir -p "$GENERATED_DIR/include" "$GENERATED_DIR/raw"

escape_dependency() {
    local value="$1"
    value="${value//\\/\\\\}"
    value="${value// /\\ }"
    value="${value//#/\\#}"
    value="${value//\$/\$\$}"
    printf '%s' "$value"
}
{
    escape_dependency "$GENERATED_DIR/ChipDockCore.swift"
    printf ':'
    while IFS= read -r -d '' INPUT; do
        printf ' '
        escape_dependency "$INPUT"
    done < <(find "$PROJECT_ROOT/crates/chipdock-core" "$PROJECT_ROOT/crates/chipdock-service" \
        "$PROJECT_ROOT/crates/chipdock-ffi" "$PROJECT_ROOT/bindings/generator" \
        -type f \( -name '*.rs' -o -name '*.toml' \) -print0)
    printf '\n'
} > "$GENERATED_DIR/rust-bindings.d"

LIBRARIES=()
FIRST_LIBRARY=""
for ARCHITECTURE in ${ARCHS:-$(uname -m)}; do
    case "$ARCHITECTURE" in
        arm64|aarch64) RUST_TARGET=aarch64-apple-darwin ;;
        x86_64) RUST_TARGET=x86_64-apple-darwin ;;
        *) echo "error: Unsupported macOS architecture: $ARCHITECTURE" >&2; exit 1 ;;
    esac
    if command -v rustup >/dev/null 2>&1 && ! rustup target list --installed | /usr/bin/grep -qx "$RUST_TARGET"; then
        rustup target add "$RUST_TARGET"
    fi
    cargo build --manifest-path "$PROJECT_ROOT/Cargo.toml" --locked \
        --package chipdock-ffi --lib --target "$RUST_TARGET" --profile "$CARGO_PROFILE"
    LIBRARIES+=("$RUST_BUILD_DIR/$RUST_TARGET/$RUST_PROFILE/libchipdock_ffi.a")
    if [[ -z "$FIRST_LIBRARY" ]]; then
        FIRST_LIBRARY="$RUST_BUILD_DIR/$RUST_TARGET/$RUST_PROFILE/libchipdock_ffi.dylib"
    fi
done

bash "$PROJECT_ROOT/bindings/generate-swift.sh" "$FIRST_LIBRARY" "$GENERATED_DIR/raw"
copy_changed() {
    if ! cmp -s "$1" "$2"; then cp "$1" "$2"; fi
}
copy_changed "$GENERATED_DIR/raw/ChipDockCore.swift" "$GENERATED_DIR/ChipDockCore.swift"
copy_changed "$GENERATED_DIR/raw/ChipDockCoreFFI.h" "$GENERATED_DIR/include/ChipDockCoreFFI.h"
copy_changed "$GENERATED_DIR/raw/ChipDockCoreFFI.modulemap" "$GENERATED_DIR/include/module.modulemap"
if [[ ${#LIBRARIES[@]} -eq 1 ]]; then
    copy_changed "${LIBRARIES[0]}" "$GENERATED_DIR/libchipdock_ffi.a"
else
    xcrun lipo -create "${LIBRARIES[@]}" -output "$GENERATED_DIR/libchipdock_ffi.universal.a"
    copy_changed "$GENERATED_DIR/libchipdock_ffi.universal.a" "$GENERATED_DIR/libchipdock_ffi.a"
    rm "$GENERATED_DIR/libchipdock_ffi.universal.a"
fi
