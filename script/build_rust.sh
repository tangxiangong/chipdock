#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:$PATH"
export MACOSX_DEPLOYMENT_TARGET="${MACOSX_DEPLOYMENT_TARGET:-14.0}"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$PROJECT_ROOT/target}"
PRODUCTS_DIR="${BUILT_PRODUCTS_DIR:-$PROJECT_ROOT/.build/Rust}"
GENERATED_DIR="$PROJECT_ROOT/macos/FlashDesk/Generated"
TEMP_GENERATED_DIR="${DERIVED_FILE_DIR:-$PROJECT_ROOT/.build}/UniFFI"
RUST_PROFILE=debug
CARGO_PROFILE=dev
if [[ "${CONFIGURATION:-Debug}" == Release ]]; then
    RUST_PROFILE=release
    CARGO_PROFILE=release
fi

if ! command -v cargo >/dev/null 2>&1; then
    echo 'error: Rust is required. Install it from https://rustup.rs, then build again.' >&2
    exit 1
fi

mkdir -p "$PRODUCTS_DIR" "$GENERATED_DIR" "$TEMP_GENERATED_DIR"
LIBRARIES=()
FIRST_LIBRARY=""
for ARCHITECTURE in ${ARCHS:-$(uname -m)}; do
    case "$ARCHITECTURE" in
        arm64|aarch64) RUST_TARGET=aarch64-apple-darwin ;;
        x86_64) RUST_TARGET=x86_64-apple-darwin ;;
        *) echo "error: Unsupported macOS architecture: $ARCHITECTURE" >&2; exit 1 ;;
    esac
    if ! rustup target list --installed | /usr/bin/grep -qx "$RUST_TARGET"; then
        rustup target add "$RUST_TARGET"
    fi
    cargo build --manifest-path "$PROJECT_ROOT/Cargo.toml" --locked \
        --package flashdesk-core --lib --target "$RUST_TARGET" --profile "$CARGO_PROFILE"
    LIBRARIES+=("$CARGO_TARGET_DIR/$RUST_TARGET/$RUST_PROFILE/libflashdesk_core.a")
    if [[ -z "$FIRST_LIBRARY" ]]; then
        FIRST_LIBRARY="$CARGO_TARGET_DIR/$RUST_TARGET/$RUST_PROFILE/libflashdesk_core.dylib"
    fi
done

/usr/bin/lipo -create "${LIBRARIES[@]}" -output "$PRODUCTS_DIR/libflashdesk_core.a"
cargo run --manifest-path "$PROJECT_ROOT/Cargo.toml" --locked --package flashdesk-bindgen -- \
    generate --library "$FIRST_LIBRARY" --language swift \
    --out-dir "$TEMP_GENERATED_DIR"

for GENERATED_FILE in "$TEMP_GENERATED_DIR"/*; do
    GENERATED_NAME="$(basename "$GENERATED_FILE")"
    if [[ "$GENERATED_NAME" == *.modulemap ]]; then GENERATED_NAME=module.modulemap; fi
    DESTINATION="$GENERATED_DIR/$GENERATED_NAME"
    if ! cmp -s "$GENERATED_FILE" "$DESTINATION"; then
        cp "$GENERATED_FILE" "$DESTINATION"
    fi
done
