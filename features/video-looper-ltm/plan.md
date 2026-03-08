# Implementation Plan — Video Looper LTM (Dream)

## Completed

### Step 1: Crate scaffold ✓
- Plugin loads in Resolume, passthrough works

### Step 2: Texture array allocation ✓
- 4-tier pyramid with CLAMP_TO_BORDER wrapping
- GL state save/restore around FBO passes

### Step 3: Ingest with recursive feedback ✓
- Screen blend: `live + prev - live * prev`
- Beat-synced delay via BPM + subdivision
- Spatial transforms: rotation, swirl, scale, shift
- Color transforms: hue shift, sat shift
- Soft-edge bounds checking (smoothstep)
- Mirror (kaleidoscope) mode

### Step 4: Downsample chain ✓
- T0→T1→T2→T3 every frame, GPU bilinear

## Remaining

### Step 5: Revise parameters
- Replace Trail Length, Trail Opacity, Weight T0-T3 with:
  - Dry (live signal level)
  - Wet (echo mix level)
  - Tap 1-4 (per-tier echo level, overdrivable 0-2)
- Reorder param indices to match new layout
- Update DreamParams methods:
  - `dry() -> f32`
  - `wet() -> f32`
  - `tap_levels() -> [f32; 4]` (mapped from 0..1 param to 0..2 range)
  - Remove: `active_tiers()`, `trail_length()`, `trail_opacity()`, `tier_weights()`
- Keep all spatial/color/tempo params as-is

### Step 6: Rewrite composite shader
- Remove multi-tap-per-tier loop (`sampleTierMulti`)
- One sample per tier at musically-timed offset:
  - T0: `write_ptr0 - 1 * delay_frames`
  - T1: `write_ptr1 - 2 * delay_frames`
  - T2: `write_ptr2 - 4 * delay_frames`
  - T3: `write_ptr3 - 8 * delay_frames`
- New uniforms: `u_dry`, `u_wet`, `u_tap[0-3]`, `u_delay`
- Remove uniforms: `u_trail_opacity`, `u_trail_length`, `u_weight[0-3]`
- Output: `dry * live + wet * (tap1*level1 + tap2*level2 + tap3*level3 + tap4*level4)`

### Step 7: Update dream.rs composite pass
- Compute delay_frames from BPM + subdivision
- Pass delay, dry, wet, tap levels as uniforms
- Update CompositeUniforms struct
- Remove active_tiers logic (always sample all 4)

### Step 8: Update MIDI output
- Consider sending tap levels as CC if useful for Ableton sync
- Keep subdivision + feedback CC as-is

### Step 9: Test and tune
- Verify tap timing at various BPM/subdivision combos
- Confirm higher tiers sample correctly at 2×/4×/8× delay
- Test tap overdrive (levels > 1.0)
- Test dry=0 (fully wet), wet=0 (bypass)
- A/B with previous version
