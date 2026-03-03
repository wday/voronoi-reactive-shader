#!/usr/bin/env bash
cd "$(dirname "$0")/../preview"
[ -d node_modules ] || npm install
exec node server.js "$@"
