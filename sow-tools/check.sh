#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
python3 sow-tools/check_file_sizes.py
cargo check --workspace
# UI ownership is verified by the WASM preview pipeline; there is no native UI crate.
