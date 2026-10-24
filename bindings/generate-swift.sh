#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LIBRARY="${1:-$PROJECT_ROOT/target/release/libchipdock_ffi.dylib}"
OUTPUT="${2:-$PROJECT_ROOT/bindings/generated/swift}"
if [[ ! -f "$LIBRARY" ]]; then
    echo "error: Missing Rust library: $LIBRARY. Build chipdock-ffi first." >&2
    exit 1
fi

cargo run --locked --manifest-path "$PROJECT_ROOT/Cargo.toml" -p chipdock-bindgen -- \
    generate --library "$LIBRARY" --language swift --no-format \
    --config "$PROJECT_ROOT/crates/chipdock-ffi/uniffi.toml" --out-dir "$OUTPUT"
