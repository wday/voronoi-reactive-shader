# Flow Plugin Family — Requirements

## Goal

A composable pipeline of FFGL plugins for real-time GPU fluid/particle simulation in Resolume. Depth feeds velocity, velocity drives particles — each stage a separate plugin in the effects stack.

## Field Texture Convention

All flow plugins communicate via RGBA float texture:

| Channel | Contents |
|---------|----------|
| R | Velocity X |
| G | Velocity Y |
| B | Scalar field (pressure/curl/divergence/temperature — context-dependent) |
| A | Density / weight |

Plugins must be adjacent in the Resolume stack — any non-flow effect between them corrupts the field silently. Document channel usage in each plugin's ISF `DESCRIPTION`.

## Plugins

### 1. `stereo-depth` (ISF) — DONE

Dual-camera block-matching disparity → greyscale depth map.

- 3-pass ISF: disparity search (1/4 res) → temporal EMA (persistent FBO) → upscale
- 9-point ring SAD at configurable block radius
- Confidence-weighted temporal blend
- Output: monochrome depth, 0 = far, 1 = near (invertible)
- Location: `stereo_depth.fs` (project root)

### 2. `flow-inject` (ISF)

Bridge between depth/audio/video inputs and the flow simulation.

**Writes to field texture:**
- Depth gradient → RG velocity (particles flow along depth surfaces)
- Depth edges (dFdx/dFdy) → A density spike (spawn at object boundaries)
- Depth value → B scalar (pressure/temperature from proximity)

**Parameters:**
- `injectVelocity` — gradient → velocity scale
- `injectDensity` — edge → density strength
- `edgeThreshold` — minimum gradient magnitude for edge detection
- `decayRate` — injected density fade per frame

### 3. `flow-euler` (ISF)

Eulerian fluid simulation on persistent dual-texture state.

**Persistent textures:** `velocityTex`, `scratchTex` (ping-pong)

**Pass structure:**
- Pass 0: Inject — blend external input into field
- Pass 1: Velocity update — pressure gradient, curl, divergence, external forces
- Pass 2: Advect (semi-Lagrangian) — `sourceUV = uv - velocity * dt`, bilinear sample
- Pass 3: Decay + output

**Key property:** Semi-Lagrangian advection traces backwards. Bilinear interpolation is free. Each fragment independent — no iteration, no search. Stable by construction.

**Parameters:**
- `dt` — timestep
- `viscosity` — velocity diffusion rate
- `decayRate` — density decay per frame
- `curlStrength` — rotational force injection
- `boundaryMode` — wrap / reflect / absorb

**Output:** Field texture. Can route directly to Channel Displace as displacement map.

### 4. `flow-lagrange` (Rust FFGL)

Lagrangian particle system advected through Eulerian velocity field. Requires Rust/FFGL for vertex pipeline (GL_POINTS).

**State:** Persistent RGBA float texture, 512x512 = 262K particles. `.rg` = position, `.ba` = velocity.

**Render architecture:**
1. Vertex shader reads particle position texture
2. Emit GL_POINTS at screen position from `.rg`
3. GPU rasterizer places point sprites — no fragment search
4. Small points + additive blending (overdraw = density, a feature)

**Physics:** Particles don't interact with each other. Each samples the velocity field at its position — single texture lookup. Embarrassingly parallel.

**Parameters:**
- `particleCount`, `pointSize`, `speedScale`, `drag`
- `spawnMode` — random / density-weighted / depth-edge
- `colorMode` — speed / age / depth / curl

**Pass structure:**
- Pass 0 (fragment): Update position texture — integrate per particle
- Pass 1 (vertex+fragment): Render GL_POINTS from position texture

## Composition

Canonical Resolume stack:
```
stereo-depth    → depth map (greyscale)
flow-inject     → field texture (depth-sourced)
flow-euler      → evolved field texture
flow-lagrange   → particle render
[Channel Displace] → optional: velocity field as warp
[Dream LTM]       → optional: depth-weighted feedback
```

Each plugin independently useful (euler→displace without particles, inject→lagrange without fluid sim, etc.)

## Open Questions

- **Particle birth/death:** Age in `.b`, CPU free list, or infinite-lifetime wrap? Decision deferred.
- **Velocity texture resolution:** 1/2 vs 1/4 output res — test for perf.
- **Field texture format:** `GL_RGBA16F` vs `GL_RGBA32F` — 16F likely sufficient.
- **Audio input to flow-inject:** Depends on whether Resolume exposes audio FFT as texture to FFGL. Investigate.
