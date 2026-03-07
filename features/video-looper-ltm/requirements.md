# Video Looper LTM — Dream-Tiered Temporal Pyramid (DTTP)

## Project
Logarithmic Video Persistence — plugin name: `video-looper-ltm-dream`

## Aesthetic Goal
Fluid, ethereal, non-stuttering motion trails. High-detail edges of current
movement "melt" into low-resolution temporal memory. Dreamy accumulation cloud
when output feeds back into input.

## Core Principle
Doubling temporal capacity (2x frames) for every halving of spatial resolution
(0.5x). Every tier occupies the same VRAM. No CPU involvement in the hot path.

## The Pyramid Structure

| Tier | Resolution    | Buffer Depth | Total Time (60fps) | VRAM Cost |
|------|---------------|--------------|---------------------|-----------|
| 0    | 100% (1080p)  | 64 frames    | ~1.0s               | 1.0 unit  |
| 1    | 50% (540p)    | 256 frames   | ~4.2s               | 1.0 unit  |
| 2    | 25% (270p)    | 1,024 frames | ~17.0s              | 1.0 unit  |
| 3    | 12.5% (135p)  | 4,096 frames | ~68.0s              | 1.0 unit  |

**Result**: 4.0 units of VRAM (~2.1GB at 1080p) yields >1 minute of fluid
temporal history. Tier depths are tunable — halving to 32/128/512/2048 cuts
to ~1GB.

## Data Flow: Continuous Demotion

Unlike N-th frame skip (which causes stutter), this uses continuous
downsampling. Every single frame contributes to every tier.

1. **Ingest**: Write live frame to the current slot in Tier 0.
2. **Continuous Drip** (every frame):
   - Take newest frame from Tier 0, downsample, write to Tier 1.
   - Take newest frame from Tier 1, downsample, write to Tier 2.
   - Take newest frame from Tier 2, downsample, write to Tier 3.

Because every tier receives a write every frame, motion at 25% resolution is
just as smooth (60fps) as full resolution — just blurrier.

## Shader: Multi-Scale Sampler

The "dreamy" quality comes from sampling across tiers with linear interpolation
to bridge resolution gaps.

### Temporal Blur Logic
Because Tier 3 is 1/8th the size of Tier 0, the GPU's hardware sampler
naturally "smears" pixels. To enhance dreaminess, sample clusters of frames:

```glsl
// Logic for a single "Dream Tap"
vec4 sampleDream(sampler2DArray tier, float offset, int depth) {
    vec2 uv = TexCoord;
    float index = mod(u_WritePtr - offset, float(depth));
    return texture(tier, vec3(uv, index));
}
```

### Output Compositing
Fragment shader mixes tiers using a weight curve:
- T0: high opacity (sharp, recent)
- T3: low opacity (blurred, ancient)
- Gaussian or linear weight distribution (tunable parameter)

## Why This Eliminates Stutter
- **No `if (frameCount % N == 0)` gating** — every frame flows through all tiers
- **No CPU readback** — no PBO, no system RAM ring buffer, no memcpy
- **No CPU pixel math** — downsampling is GPU shader passes
- **Predictable VRAM** — if one tier fits, all tiers fit (same byte size)

## GL Implementation Notes
- Each tier is a `GL_TEXTURE_2D_ARRAY` — hardware-indexed by layer
- Downsample chain: 3 shader passes per frame (T0->T1, T1->T2, T2->T3)
- Write pointer per tier, wrapping at tier depth
- All rendering stays in VRAM — zero CPU involvement in hot path

## Recursive Feedback
If the composited output is fed back into Tier 0 (via Resolume's effect chain
or explicit feedback path), you get an accumulation cloud: sharp current motion
dissolving into deep temporal blur.

## Parameters (Initial)
- **Trail Length**: controls how many tiers / how deep to sample
- **Trail Opacity**: weight curve for tier mixing (sharp vs dreamy)
- **Tier Depths**: global scale factor on buffer depths (VRAM vs history tradeoff)

## VRAM Budget (1080p reference)
- Per frame at 1080p: 1920 x 1080 x 4 = 8.3MB
- Per tier: ~530MB (64 full-res frames equivalent)
- Total 4 tiers: ~2.1GB
- At 720p: ~930MB total
- Halved depths (32/128/512/2048): ~1.05GB at 1080p
