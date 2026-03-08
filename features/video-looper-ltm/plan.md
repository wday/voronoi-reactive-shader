# Implementation Plan — Video Looper LTM (Dream)

## Step 1: Crate scaffold
- Create `plugins/video-looper-ltm-dream/` with FFGL boilerplate
- Register in `plugins.json`, verify build
- Passthrough shader only — confirm plugin loads in Resolume

## Step 2: Texture array allocation
- Allocate 4 `GL_TEXTURE_2D_ARRAY` textures (one per tier)
- Tier depths configurable via constants
- Write pointer per tier
- Verify allocation succeeds, log VRAM usage estimate

## Step 3: Ingest — write live frame to Tier 0
- Render input texture into Tier 0 at current write pointer layer
- Use FBO with `glFramebufferTextureLayer` to target specific array layer
- Advance Tier 0 write pointer

## Step 4: Downsample chain
- 3 FBO passes per frame: T0->T1, T1->T2, T2->T3
- Each pass: bind source tier's newest layer, render into dest tier's next layer
- Downsample shader: simple bilinear (hardware filtering handles it)
- Advance each tier's write pointer

## Step 5: Multi-scale compositor shader
- Sample N frames from each tier (dream taps)
- Weight by tier (T0 high opacity, T3 low opacity)
- Blend into final output
- Render into host FBO

## Step 6: Parameters
- Trail Length: how many tiers contribute / how deep to sample
- Trail Opacity: weight curve shape
- Add to FFGL parameter system

## Step 7: Tuning and testing
- Profile frame times (reuse tracing pattern from video-looper)
- Test at different resolutions
- Tune tier depths for VRAM budget
- A/B comparison with hybrid video-looper in Resolume
