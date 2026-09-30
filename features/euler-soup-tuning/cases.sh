#!/usr/bin/env bash
# Euler Soup experiment cases — reproducible renders behind the findings in devlog.md.
# usage: features/euler-soup-tuning/cases.sh [case ...]    (no args = all cases)
# Each case writes renders/<case>.png: one row per param set, one column per time (s @ 60 fps).
set -euo pipefail
cd "$(dirname "$0")/../.."
OUT=features/euler-soup-tuning/renders
FS=shaders/flow_euler.fs
P="uv run tools/isf_preview.py"
# Resolume live patch, 2026-09-30 (read over the REST API): stir 0, curl 3, viscosity 1.
LIVE='--base={"flowScale":0.018,"curlStrength":3,"stir":0,"churn":1,"viscosity":1,"dt":1.76,"diffusion":0,"decayRate":0.9655,"inputMix":0.169}'
# The host hands over uncleared float textures; seed velA with noise to stand in for that.
HOST="--size=480x270 --input=spiral --seed=velA:1"

aspect() {  # Horizontal bands came from UV-unit velocity: at 16:9 x moved 1.78x further than y.
  local pre; pre=$(mktemp -d)/flow_euler_pre.fs
  git show 96e5fa1:shaders/flow_euler.fs > "$pre"
  $P "$pre" $OUT/aspect_before_wide.png --size=480x270 --input=spiral --times=10 '{}' '{"curlStrength":0}'
  $P "$pre" $OUT/aspect_before_square.png --size=270x270 --input=spiral --times=10 '{}' '{"curlStrength":0}'
  $P $FS $OUT/aspect_after_wide.png --size=480x270 --input=spiral --times=10 '{}' '{"curlStrength":0}'
}

churn() {  # Churn 0→1 substeps the backtrace; 1→2 eases damping and doubles curl.
  $P $FS $OUT/churn.png --size=480x270 --input=spiral --times=10 \
    '{"churn":0}' '{"churn":1}' '{"churn":1.5}' '{"churn":2}'
}

cells() {  # Live patch: velocity is pixel noise, yet dye forms contiguous cells as churn rises.
  $P $FS $OUT/cells_dye.png $HOST "$LIVE" --times=2,10 \
    '{"churn":0}' '{"churn":0.5}' '{"churn":1}' '{"churn":2}'
  $P $FS $OUT/cells_velocity.png $HOST "$LIVE" --view=velA:velocity --times=2,10 \
    '{"churn":0}' '{"churn":1}'
}

flowscale() {  # Live patch: flowScale 0 → max goes smooth → cells → noise.
  $P $FS $OUT/flowscale.png $HOST "$LIVE" --times=10 \
    '{"flowScale":0}' '{"flowScale":0.004}' '{"flowScale":0.018}' \
    '{"flowScale":0.05}' '{"flowScale":0.1}' '{"flowScale":0.2}'
  $P $FS $OUT/flowscale_churn0.png $HOST "$LIVE" --times=10 \
    '{"churn":0,"flowScale":0.004}' '{"churn":0,"flowScale":0.018}' '{"churn":0,"flowScale":0.1}'
}

seed() {  # Live patch from a clean start (no --seed history): does the seed param alone bring cells back?
  $P $FS $OUT/seed.png --size=480x270 --input=spiral "$LIVE" --times=2,10 \
    '{"seed":0}' '{"seed":0.02}' '{"seed":0.1}' '{"seed":0.3}'
  # Reset held for 0.1 s at t=5: the soup should snap back to the input, then regrow.
  $P $FS $OUT/reset.png --size=480x270 --input=spiral "$LIVE" --times=4.9,5.2,10 \
    '{"seed":0.1,"reset":[[0,false],[5,true],[5.1,false]]}'
}

cases=("$@"); [ ${#cases[@]} -eq 0 ] && cases=(aspect churn cells flowscale seed)
for c in "${cases[@]}"; do echo "== $c"; "$c"; done
