#!/usr/bin/env bash
# Run the delay-line tests OUTSIDE Resolume, on the native Linux toolchain
# (mesa/llvmpipe software GL). Two tiers:
#   Tier 1 — pure-logic unit tests in delay-dsp (ring off-by-one, param curves,
#            time conversion). No GL.
#   Tier 2 — headless-GL integration harness (delay-gl-tests): the real delay-core
#            ring + the real plugin shaders on a surfaceless EGL context, asserting
#            pixels end-to-end.
#
# These use the Linux target dirs and are independent of the Windows plugin build
# (`make build`, which cross-compiles to a C: target). Requires libEGL + mesa
# (already present under WSLg). Usage: ./scripts/test-delay.sh
set -euo pipefail

PLUGINS_DIR="$(cd "$(dirname "$0")/../plugins" && pwd)"

# Ensure we do NOT inherit the Windows CARGO_TARGET_DIR (C:\...) if it's exported.
unset CARGO_TARGET_DIR

echo "== Tier 1: delay-dsp pure-logic unit tests =="
cargo test --manifest-path "$PLUGINS_DIR/Cargo.toml" -p delay-dsp

echo
echo "== Tier 2: headless-GL integration harness (delay-gl-tests) =="
cargo test --manifest-path "$PLUGINS_DIR/delay-gl-tests/Cargo.toml"

echo
echo "All delay-line tests passed."
