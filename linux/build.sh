#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
profile="${PROFILE:-release}"
if [[ "$profile" == release ]]; then
  cargo build --locked --release -p chipdock-gpui --bin chipdock-gpui
else
  cargo build --locked -p chipdock-gpui --bin chipdock-gpui
fi
