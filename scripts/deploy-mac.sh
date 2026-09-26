#!/usr/bin/env bash
# Deploy one built plugin (build/macos/<stem>.dylib) into Resolume's Extra Effects on macOS.
# Usage: ./scripts/deploy-mac.sh <plugin_name>
#
# Plugins become <stem>.bundle/Contents/{MacOS/<stem>,Info.plist}, ad-hoc signed.
# Shared cores ("shared_lib": true in plugins.json) are copied loose as lib<stem>.dylib
# beside the bundles: pluglib's loader steps out of the bundle to find them there, so
# every Write/Tap/Read pair dlopens the same file and shares one registry.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
REGISTRY="$PROJECT_DIR/plugins.json"
MAC_OUT="$PROJECT_DIR/build/macos"
RESOLUME_DIR="${RESOLUME_DIR:-$HOME/Documents/Resolume Avenue/Extra Effects}"

NAME="${1:?Usage: deploy-mac.sh <plugin_name>}"

read -r STEM SHARED < <(python3 -c "
import json, sys
p = {p['name']: p for p in json.load(open('$REGISTRY'))['plugins']}.get('$NAME')
if not p:
    sys.exit('Unknown plugin: $NAME')
print(p['dll'].removesuffix('.dll'), int(p.get('shared_lib', False)))
")

# STEM names what gets replaced in Extra Effects below; refuse anything unexpected.
if [[ ! "$STEM" =~ ^[a-z0-9_]+$ ]]; then
    echo "==> ERROR: bad plugin stem '$STEM' for $NAME" >&2
    exit 1
fi

LIB="$MAC_OUT/$STEM.dylib"
if [ ! -f "$LIB" ]; then
    echo "==> ERROR: $LIB not found. Run 'make build PLUGIN=$NAME' first." >&2
    exit 1
fi

mkdir -p "$RESOLUME_DIR"

if [ "$SHARED" = "1" ]; then
    cp "$LIB" "$RESOLUME_DIR/lib$STEM.dylib"
    codesign --force -s - "$RESOLUME_DIR/lib$STEM.dylib" 2>/dev/null
    echo "==> Deployed lib$STEM.dylib (shared core) to $RESOLUME_DIR/"
    exit 0
fi

# A previous deploy is moved aside into the build tree, never deleted in place.
BUNDLE="$RESOLUME_DIR/$STEM.bundle"
if [ -e "$BUNDLE" ]; then
    REPLACED="$MAC_OUT/replaced/$(date +%Y%m%d-%H%M%S)"
    mkdir -p "$REPLACED"
    mv "$BUNDLE" "$REPLACED/"
fi
mkdir -p "$BUNDLE/Contents/MacOS"
cp "$LIB" "$BUNDLE/Contents/MacOS/$STEM"
cat > "$BUNDLE/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>$STEM</string>
    <key>CFBundleIdentifier</key>
    <string>com.github.wday.voronoi-reactive-shader.${STEM//_/-}</string>
    <key>CFBundleName</key>
    <string>$STEM</string>
    <key>CFBundlePackageType</key>
    <string>BNDL</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
</dict>
</plist>
EOF
codesign --force -s - "$BUNDLE" 2>/dev/null
echo "==> Deployed $STEM.bundle to $RESOLUME_DIR/"
