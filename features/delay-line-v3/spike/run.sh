#!/usr/bin/env bash
# Stage-0 spike: prove two independently-loaded plugin cdylibs share ONE
# delay-core registry instance (the premise of the two-plugin split).
set -euo pipefail
cd "$(dirname "$0")"

# Pin the target dir locally so a globally-set CARGO_TARGET_DIR (used for the
# Windows plugin builds) doesn't send artifacts elsewhere and break the
# $ORIGIN/link-search paths.
export CARGO_TARGET_DIR="$(pwd)/target"

cargo build -p delay-core          # must exist before the plugins link it
cargo build -p plugin-a -p plugin-b
cargo build -p host

# Co-locate the three shared objects; the plugins' $ORIGIN rpath finds
# libdelay_core.so right beside them (models the Resolume plugin folder).
mkdir -p dist
cp target/debug/libdelay_core.so target/debug/libplugin_a.so target/debug/libplugin_b.so dist/

echo "=== running host (dlopen both plugins) ==="
exec ./target/debug/host dist/libplugin_a.so dist/libplugin_b.so
