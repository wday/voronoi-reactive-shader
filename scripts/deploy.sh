#!/bin/bash
# Deploy voronoi_reactive.fs as an FFGL plugin for Resolume via ffgl-rs.
# Usage: ./scripts/deploy.sh [--debug]
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
FFGL_RS_DIR="$PROJECT_DIR/vendor/ffgl-rs"
SHADER="$PROJECT_DIR/shaders/voronoi_reactive.fs"

if [ ! -f "$SHADER" ]; then
    echo "ERROR: Shader not found at $SHADER"
    exit 1
fi

if [ ! -d "$FFGL_RS_DIR" ]; then
    echo "ERROR: ffgl-rs not found at $FFGL_RS_DIR"
    echo "       Run: git submodule update --init"
    exit 1
fi

# Ensure bindgen/clang can find macOS SDK headers (handles spaces in Xcode path)
export SDKROOT="$(xcrun --show-sdk-path)"

# Validate
echo "==> Validating ISF shader..."
cd "$FFGL_RS_DIR"
./validate_isf.sh "$SHADER"
echo ""

# Build + deploy via ffgl-rs
echo "==> Building and deploying FFGL plugin..."
if [ "$1" = "--debug" ]; then
    DEBUG=1 ./ffgl-isf/deploy_isf.sh "$SHADER"
else
    ./ffgl-isf/deploy_isf.sh "$SHADER"
fi

echo ""
echo "==> Done. Plugin 'voronoi_reacti' installed."
echo "    Restart Resolume to pick up the new plugin."
