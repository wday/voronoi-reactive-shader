#!/usr/bin/env bash
# Stage-0 Windows spike: prove two independently-LoadLibrary'd plugin DLLs share
# ONE delay_core.dll registry on the REAL Windows loader. Built with the project's
# Windows MSVC cargo (via cmd.exe over the \\wsl$ UNC path, target on C:\ per the
# dep-info UNC bug) and run as a native .exe through WSL interop.
set -euo pipefail

WSL_USER="${WSL_USER:-alien}"
WSL_DISTRO="${WSL_DISTRO:-Ubuntu}"
WIN_CARGO_BIN="C:\\Users\\${WSL_USER}\\scoop\\apps\\rustup\\current\\.cargo\\bin"
WIN_SHIMS="C:\\Users\\${WSL_USER}\\scoop\\shims"
WIN_SPIKE="\\\\wsl\$\\${WSL_DISTRO}\\home\\${WSL_USER}\\dev\\voronoi-reactive-shader\\features\\delay-line-v3\\spike-win"
WIN_TARGET="C:\\Users\\${WSL_USER}\\.cargo-target\\spike-win"
OUT="/mnt/c/Users/${WSL_USER}/.cargo-target/spike-win/release"
DIST="/mnt/c/Users/${WSL_USER}/.cargo-target/spike-win/dist"
WIN_DIST="C:\\Users\\${WSL_USER}\\.cargo-target\\spike-win\\dist"

echo "=== build (windows msvc cargo via cmd.exe) ==="
cd /mnt/c && cmd.exe /c "pushd ${WIN_SPIKE}&&set PATH=${WIN_CARGO_BIN};${WIN_SHIMS};%PATH%&&set CARGO_TARGET_DIR=${WIN_TARGET}&&cargo build --release"

echo "=== stage dist: co-locate all 3 DLLs (models a Resolume plugin folder) ==="
mkdir -p "$DIST"
cp "$OUT/delay_core.dll" "$OUT/plugin_a.dll" "$OUT/plugin_b.dll" "$DIST/"

echo "=== positive: host loads both plugins from dist; each finds delay_core beside itself ==="
"$OUT/host.exe" "${WIN_DIST}\\plugin_a.dll" "${WIN_DIST}\\plugin_b.dll"

echo
echo "=== negative: plugins WITHOUT a co-located delay_core.dll should fail to bind ==="
DIST2="/mnt/c/Users/${WSL_USER}/.cargo-target/spike-win/dist_nocore"
WIN_DIST2="C:\\Users\\${WSL_USER}\\.cargo-target\\spike-win\\dist_nocore"
rm -rf "$DIST2"; mkdir -p "$DIST2"
cp "$OUT/plugin_a.dll" "$OUT/plugin_b.dll" "$DIST2/"   # deliberately NO delay_core.dll
if "$OUT/host.exe" "${WIN_DIST2}\\plugin_a.dll" "${WIN_DIST2}\\plugin_b.dll" 2>&1; then
  echo "!! UNEXPECTED: bound delay_core without it co-located (ambient search path?)"
else
  echo "OK: failed as expected — proves the plugin loads delay_core from its OWN dir, not an ambient path"
fi
