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

curl -skL "https://citeseerx.ist.psu.edu/document?repid=rep1&type=pdf&doi=c57dfd79e1a887408d56d60a7a89055b367bcab6" \
  -o "$DIR/knutsson-westin-normalized-convolution-1993.pdf"
echo "  knutsson-westin-normalized-convolution-1993.pdf"

echo ""
echo "Manual downloads (ACM blocks automated requests, but open access in browser):"
echo "  Perlin, 'An Image Synthesizer' (SIGGRAPH 1985)"
echo "    https://dl.acm.org/doi/epdf/10.1145/325165.325247"
echo "    → save as: $DIR/perlin-image-synthesizer-siggraph-1985.pdf"
echo ""
echo "Done."
