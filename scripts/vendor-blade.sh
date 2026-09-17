#!/usr/bin/env bash
# Vendor only the Blade packages used by the web client.
# Idempotent: restores blade/ at the exact revision pinned by Cargo.toml while
# keeping the ignored checkout limited to the active graphics packages.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

# Single source of truth: the rev pinned in sow-render/Cargo.toml
# (sow-client and sow-map pin the same rev).
REV="$(sed -nE 's|.*ohsalmeron/blade", rev = "([0-9a-f]{40})".*|\1|p' sow-render/Cargo.toml | head -n 1)"
if [ -z "${REV}" ]; then
  echo "vendor-blade: cannot parse pinned blade rev from sow-render/Cargo.toml" >&2
  exit 1
fi
URL="https://github.com/ohsalmeron/blade"

if [ ! -d blade/.git ] || [ ! -f blade/blade-graphics/Cargo.toml ] || [ ! -f blade/blade-macros/Cargo.toml ]; then
  echo "vendor-blade: cloning ${URL} @ ${REV}"
  rm -rf blade
  git clone --filter=blob:none --no-checkout "${URL}" blade
fi
CURRENT="$(git -C blade rev-parse HEAD 2>/dev/null || echo MISSING)"
if [ "${CURRENT}" != "${REV}" ]; then
  echo "vendor-blade: checkout ${REV} (have ${CURRENT})"
  git -C blade fetch --quiet origin "${REV}" || git -C blade fetch --quiet
fi

# The upstream checkout contains packages outside the web renderer.
# They are not part of this workspace and must not be materialized locally.
SPARSE_ENABLED="$(git -C blade config --get core.sparseCheckout 2>/dev/null || true)"
SPARSE_PATHS="$(git -C blade sparse-checkout list 2>/dev/null || true)"
if [ "${SPARSE_ENABLED}" != "true" ] || [ "${SPARSE_PATHS}" != $'blade-graphics\nblade-macros' ]; then
  git -C blade sparse-checkout init --no-cone
  git -C blade sparse-checkout set --no-cone blade-graphics blade-macros
fi
git -C blade checkout --quiet "${REV}"

cp scripts/blade-web-workspace.toml blade/Cargo.toml
# blade-macros only needs syn/proc-macro2/quote at runtime. Its upstream test
# dependencies reference packages intentionally omitted from the web vendor.
sed -i '/^\[dev-dependencies\]$/,$d' blade/blade-macros/Cargo.toml

if ! cmp -s scripts/blade-web-workspace.toml blade/Cargo.toml; then
  echo "vendor-blade: web workspace manifest mismatch" >&2
  exit 1
fi
if grep -q '^\[dev-dependencies\]' blade/blade-macros/Cargo.toml; then
  echo "vendor-blade: inactive macro test dependencies materialized" >&2
  exit 1
fi

GRAPHICS_VER="$(sed -nE 's/^version = "([^"]+)".*/\1/p' blade/blade-graphics/Cargo.toml | head -n 1)"
if [ "${GRAPHICS_VER}" != "0.8.4" ]; then
  echo "vendor-blade: version mismatch at ${REV} (graphics=${GRAPHICS_VER})" >&2
  exit 1
fi
MACROS_VER="$(sed -nE 's/^version = "([^"]+)".*/\1/p' blade/blade-macros/Cargo.toml | head -n 1)"
if [ "${MACROS_VER}" != "0.3.0" ]; then
  echo "vendor-blade: version mismatch at ${REV} (macros=${MACROS_VER})" >&2
  exit 1
fi
echo "vendor-blade: OK ${REV} (graphics ${GRAPHICS_VER}, macros ${MACROS_VER})"
