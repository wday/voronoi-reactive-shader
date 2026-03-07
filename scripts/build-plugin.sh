#!/usr/bin/env bash
# Build a single plugin from plugins.json
# Usage: ./scripts/build-plugin.sh <plugin_name>
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
REGISTRY="$PROJECT_DIR/plugins.json"

WSL_USER="${WSL_USER:-alien}"
WSL_DISTRO="${WSL_DISTRO:-Ubuntu}"

WIN_CARGO_BIN="C:\\Users\\${WSL_USER}\\scoop\\apps\\rustup\\current\\.cargo\\bin"
WIN_SCOOP="C:\\Users\\${WSL_USER}\\scoop\\shims"
WIN_TARGET="C:\\Users\\${WSL_USER}\\.cargo-target\\ffgl-rs"
WIN_PROJECT="\\\\wsl\$\\${WSL_DISTRO}\\home\\${WSL_USER}\\dev\\voronoi-reactive-shader"
TARGET_DIR="/mnt/c/Users/${WSL_USER}/.cargo-target/ffgl-rs/release"

NAME="${1:?Usage: build-plugin.sh <plugin_name>}"

# Read plugin config from registry
PLUGIN_JSON=$(python3 -c "
import json, sys
plugins = {p['name']: p for p in json.load(open('$REGISTRY'))['plugins']}
p = plugins.get('$NAME')
if not p:
    print(f'Unknown plugin: $NAME', file=sys.stderr)
    sys.exit(1)
import json as j
print(j.dumps(p))
")

PTYPE=$(echo "$PLUGIN_JSON" | python3 -c "import json,sys; print(json.load(sys.stdin)['type'])")
PDLL=$(echo "$PLUGIN_JSON" | python3 -c "import json,sys; print(json.load(sys.stdin)['dll'])")

if [ "$PTYPE" = "isf" ]; then
    PSHADER=$(echo "$PLUGIN_JSON" | python3 -c "import json,sys; print(json.load(sys.stdin)['shader'])")
    WIN_ISF_SRC="Z:\\home\\${WSL_USER}\\dev\\voronoi-reactive-shader\\$(echo "$PSHADER" | tr '/' '\\')"

    echo "==> Building ISF plugin: $NAME ($PSHADER)"
    cd /mnt/c && cmd.exe /c \
        "pushd ${WIN_PROJECT}\\ffgl-rs&&set PATH=${WIN_CARGO_BIN};${WIN_SCOOP};%PATH%&&set ISF_SOURCE=${WIN_ISF_SRC}&&set ISF_NAME=${NAME}&&set CARGO_TARGET_DIR=${WIN_TARGET}&&cargo build --release -p ffgl-isf"

    cp "${TARGET_DIR}/ffgl_isf.dll" "${TARGET_DIR}/${PDLL}"
    echo "==> Built: ${TARGET_DIR}/${PDLL}"

elif [ "$PTYPE" = "rust" ]; then
    PCRATE=$(echo "$PLUGIN_JSON" | python3 -c "import json,sys; print(json.load(sys.stdin)['crate'])")

    echo "==> Building Rust plugin: $NAME (crate: $PCRATE)"
    cd /mnt/c && cmd.exe /c \
        "pushd ${WIN_PROJECT}\\ffgl-rs&&set PATH=${WIN_CARGO_BIN};${WIN_SCOOP};%PATH%&&set CARGO_TARGET_DIR=${WIN_TARGET}&&cargo build --release -p ${PCRATE}"

    echo "==> Built: ${TARGET_DIR}/${PDLL}"
else
    echo "Unknown plugin type: $PTYPE" >&2
    exit 1
fi
