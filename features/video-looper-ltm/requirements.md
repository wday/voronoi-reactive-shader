# Video Looper LTM — Dream-Tiered Temporal Pyramid (DTTP)

## Project
Logarithmic Video Persistence — plugin name: `video-looper-ltm-dream`

## Aesthetic Goal
Musically-timed visual echoes that dissolve from sharp to dreamy. Recent echoes
are crisp repetitions of the source; distant echoes melt into blurry temporal
texture. Spatial transforms (rotation, swirl, scale, shift) compound through
each echo, creating spirals and kaleidoscopic recursion synced to the beat.

## Core Principle
One tap per pyramid tier. Each tier doubles the delay and halves the resolution.
The resolution loss at longer delays IS the aesthetic — sharp echoes up close,
dissolving into dream at distance. All GPU, zero CPU in the hot path.

## The Pyramid Structure

| Tier | Resolution   | Delay         | Character      | VRAM Cost |
|------|-------------|---------------|----------------|-----------|
| T0   | 100% (full) | 1× subdivision | Sharp echo     | 1.0 unit  |
| T1   | 50%         | 2× subdivision | Soft echo      | 1.0 unit  |
| T2   | 25%         | 4× subdivision | Dreamy         | 1.0 unit  |
| T3   | 12.5%       | 8× subdivision | Deep memory    | 1.0 unit  |

Buffer depths per tier are sized to hold enough frames for the delay at 60fps.
Tier 0 = 288 frames (~4.8s), each subsequent tier 2× as many at half resolution.

**Example at 120 BPM, 1/4 note subdivision (0.5s = 30 frames):**
- Tap 1 (T0): 30 frames back, full resolution — crisp first echo
- Tap 2 (T1): 60 frames back, half resolution — softer second echo
- Tap 3 (T2): 120 frames back, quarter resolution — blurry third echo
- Tap 4 (T3): 240 frames back, eighth resolution — deep temporal smear

## Data Flow

### Ingest (per frame)
Each frame stored in Tier 0 is the live input composited with recursive feedback:
```
stored_frame = screen(live, spatial_transform(prev_frame[delay]) * feedback)
```
Where `delay` = subdivision in frames, and `spatial_transform` applies rotation,
swirl, scale, shift, hue shift, etc. These transforms compound through echoes —
the Nth echo has N× the rotation, N× the swirl, etc.

### Downsample Chain (per frame)
Every frame flows through all tiers via GPU bilinear downsample:
```
T0 (newest) → T1 → T2 → T3
```
No frame skipping — motion is 60fps smooth at every resolution tier.

### Composite (per frame)
One sample from each tier at its musically-timed offset:
```glsl
tap1 = texture(tier0, vec3(uv, write_ptr0 - 1 * delay));   // sharp
tap2 = texture(tier1, vec3(uv, write_ptr1 - 2 * delay));   // soft
tap3 = texture(tier2, vec3(uv, write_ptr2 - 4 * delay));   // dreamy
tap4 = texture(tier3, vec3(uv, write_ptr3 - 8 * delay));   // deep

trail = tap1 * tap1_level + tap2 * tap2_level + tap3 * tap3_level + tap4 * tap4_level;
output = dry * live + wet * trail;
```
Tap levels default to a decay curve but are individually overdrivable (not
group-normalized) for creative effects.

## Parameters

| # | Name         | Range     | Default | Description |
|---|-------------|-----------|---------|-------------|
| 0 | Dry         | 0–1       | 1.0     | Level of unprocessed live signal |
| 1 | Wet         | 0–1       | 0.5     | Level of combined echo taps |
| 2 | Tap 1       | 0–2       | 1.0     | T0 echo level (overdrivable) |
| 3 | Tap 2       | 0–2       | 0.7     | T1 echo level |
| 4 | Tap 3       | 0–2       | 0.4     | T2 echo level |
| 5 | Tap 4       | 0–2       | 0.2     | T3 echo level |
| 6 | Feedback    | 0–1       | 0.85    | Decay multiplier per recursive echo |
| 7 | Shift X     | ±0.5 UV   | 0 (center) | Spatial shift per echo |
| 8 | Shift Y     | ±0.5 UV   | 0 (center) | Spatial shift per echo |
| 9 | Rotation    | ±180°     | 0 (center) | Z rotation per echo |
| 10| Scale       | 0.5×–2.0× | 1.0× (center) | Zoom per echo |
| 11| Swirl       | ±2.0 rad  | 0 (center) | Spiral twist per echo |
| 12| Hue Shift   | ±180°     | 0 (center) | Color rotation per echo |
| 13| Sat Shift   | ±0.5      | 0 (center) | Saturation shift per echo |
| 14| Mirror      | off/on    | off     | Kaleidoscope edge reflection |
| 15| Fold        | 0.1–1.0   | 1.0 (off) | Luminance fold threshold |
| 16| BPM         | 50–200    | 120     | Tempo reference |
| 17| Subdivision | discrete  | 1/4 note | Delay unit: 1/16, 1/8, 1/4, 1/2, 1 bar |

**17 params** (down from 18). Removed: Trail Length, Trail Opacity, Weight T0-T3.
Added: Dry, Wet, Tap 1-4.

## Why One Tap Per Tier Works

- **Musical timing**: delay doubles per tier (1×, 2×, 4×, 8×) — musically natural
- **Natural blur**: resolution halves per tier — distant echoes ARE blurrier
- **Simple mental model**: "4 echo taps, each further away and dreamier"
- **Overdrive**: each tap is independent, can boost distant echoes for effect
- **Dry/Wet**: separate controls, no coupling between live signal and echo mix

## GL Implementation Notes
- Each tier is a `GL_TEXTURE_2D_ARRAY` with `CLAMP_TO_BORDER` wrapping
- Downsample chain: 3 shader passes per frame (T0→T1, T1→T2, T2→T3)
- Write pointer per tier, wrapping at tier depth
- Ingest uses soft-edge bounds checking (smoothstep) to prevent feedback line artifacts
- GL state (scissor/blend/depth) saved and restored around FBO passes
- All rendering stays in VRAM — zero CPU involvement in hot path

## VRAM Budget (1080p reference)
- Per frame at 1080p: 1920 × 1080 × 4 = 8.3MB
- Tier 0 (288 frames, full res): ~2.4GB
- Tier 1 (576 frames, half res): ~1.2GB
- Tier 2 (1152 frames, quarter res): ~0.6GB
- Tier 3 (2304 frames, eighth res): ~0.3GB
- Total: ~4.5GB (tunable by adjusting depths)
