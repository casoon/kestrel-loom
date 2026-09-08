#!/usr/bin/env bash
# Baut das WASM-Paket nach ./pkg
set -euo pipefail
cd "$(dirname "$0")/crates/wasm"
wasm-pack build --target web --out-dir ../../pkg "$@"
