#!/usr/bin/env bash
# Download reference PDFs cited in CREDITS.md
# These are not committed to the repo to avoid licensing issues.

set -euo pipefail

DIR="$(cd "$(dirname "$0")/../docs/references" 2>/dev/null || mkdir -p "$(dirname "$0")/../docs/references" && cd "$(dirname "$0")/../docs/references" && pwd)"

echo "Downloading references to $DIR"

curl -sL "https://jcgt.org/published/0009/03/02/paper.pdf" \
  -o "$DIR/hash-functions-gpu-rendering-jcgt-2020.pdf"
echo "  hash-functions-gpu-rendering-jcgt-2020.pdf"

curl -sL "https://cs.nyu.edu/~perlin/paper445.pdf" \
  -o "$DIR/perlin-improving-noise-2002.pdf"
echo "  perlin-improving-noise-2002.pdf"

echo "Done."
