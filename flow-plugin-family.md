# Flow Plugin Family Spec

> Design notes and implementation plan for real-time GPU fluid/particle simulation as FFGL plugins for Resolume.

---

## Background

This family grew out of two threads: dual-camera stereo depth mapping (producing a per-pixel depth field), and the question of what to do with that field beyond direct visualisation. The answer is a composable pipeline — depth feeds a velocity field, a velocity field drives particles — each stage a separate plugin in the Resolume effects stack.

---

## Mental Model

### Eulerian vs Lagrangian

Two distinct physical models, two distinct plugins:

**Eulerian (flow-euler)** — tracks what is happening *at each location in space*. Every cell of a grid always exists; it holds a density and a velocity. No particle identity. Updates are local: each cell reads its neighbours, no global search. Maps cleanly to fragment shaders.

**Lagrangian (flow-lagrange)** — tracks individual entities with position, velocity, age. Visually distinct: discrete sprites, streaks, firefly character. Requires vertex pipeline access (GL_POINTS). Implemented as a Rust FFGL plugin, not ISF.

These are not competing — they compose. Euler produces a velocity field; Lagrange consumes it. Lagrangian particles become probes of the Eulerian field rather than interacting agents.

---

## The Field Texture Convention

All flow plugins communicate via a shared **RGBA float texture**:

| Channel | Contents |
|---------|----------|
| R | Velocity X |
| G | Velocity Y |
| B | Scalar field (pressure / curl magnitude / divergence / temperature — context-dependent) |
| A | Density / weight |

Resolume treats this as a colour texture and passes it between effects without interpretation. Plugins immediately adjacent in the stack share the convention silently. Document which channel carries what in each plugin's ISF JSON `DESCRIPTION`.

> **Note:** If any non-flow effect is inserted between flow plugins in the stack, the field data will be corrupted silently. Keep flow plugins adjacent.

---

## Plugin Family

### `stereo-depth` (ISF — done)

Dual-camera block-matching disparity. Produces a greyscale 0→1 depth map.

- 3-pass ISF: disparity search (1/4 res) → temporal EMA (persistent FBO) → upscale
- 9-point ring SAD at configurable block radius
- Confidence-weighted temporal blend for stability
- Output: monochrome depth, 0 = far, 1 = near (invertible)

Primary input to `flow-inject`.

---

### `flow-inject` (ISF)

Writes scene information into the field texture. Bridge between depth/audio/video inputs and the flow simulation.

**Inputs:**
- Depth map (from `stereo-depth` or any greyscale source)
- Optional: audio texture, video frame

**Writes:**
- Depth gradient → RG velocity channels (particles flow along depth surfaces)
- Depth edges (`dFdx`/`dFdy` of depth) → A density spike (spawn regions at object boundaries)
- Depth value → B scalar (pressure/temperature driven by proximity)

**Parameters:**
- `injectVelocity` — scale of gradient→velocity mapping
- `injectDensity` — strength of edge→density injection
- `edgeThreshold` — minimum gradient magnitude to count as an edge
- `decayRate` — how fast injected density fades each frame

---

### `flow-euler` (ISF)

Eulerian fluid simulation on a persistent dual-texture state.

**Persistent textures:**
- `velocityTex` — RGBA, RG=velocity, B=pressure/curl, A=density
- `scratchTex` — intermediate advection buffer (ping-pong)

**Pass structure:**

```
PASS 0 — Inject
  Blend external input (from flow-inject) into velocity/density field

PASS 1 — Velocity update
  Each cell reads 4 neighbours
  Compute pressure gradient, curl, divergence
  Apply external forces (depth map, audio)
  Write updated velocity

PASS 2 — Advect (semi-Lagrangian)
  sourceUV = uv - velocity * dt
  newDensity = sample(densityTex, sourceUV)  // bilinear, free
  Stable by construction — cannot blow up

PASS 3 — Decay + output
  density *= decayRate
  Output field texture (RG=velocity, B=curl magnitude, A=density)
```

**Key property:** Semi-Lagrangian advection traces backwards through the velocity field. Bilinear interpolation during texture sampling does the interpolation work for free. Each fragment is fully independent — no iteration, no search.

**Parameters:**
- `dt` — timestep (tweak for faster/slower dynamics)
- `viscosity` — velocity diffusion rate
- `decayRate` — density decay per frame
- `curlStrength` — rotational force injection
- `boundaryMode` — wrap / reflect / absorb

**Output:** Field texture conforming to the convention above. Can be routed directly to Channel Displace as a displacement map — the velocity channels are a natural displace source.

---

### `flow-lagrange` (Rust FFGL)

Lagrangian particle system advected through an Eulerian velocity field. Requires Rust/FFGL for vertex pipeline access — ISF cannot emit GL_POINTS.

**State:** One persistent RGBA float texture, NxM where NxM = particle count.
- Recommended: 512×512 = 262,144 particles (well within vertex throughput budget)
- `.rg` = position, `.ba` = velocity

**Render architecture (the GPU hack):**

The "which pixel is near a particle?" problem is dissolved by giving it to the rasterizer:

1. Read particle position texture in the *vertex shader* — position in texture = particle ID
2. Emit `GL_POINTS` at the screen position encoded in `.rg`
3. GPU rasterizer places point sprites — no fragment-side search needed
4. Vertex throughput: ~10B vertices/sec on mid-range GPU → 262K particles at 60fps uses <0.002% of budget
5. Fill rate / overdraw is the real constraint — use small points + additive blending (overdraw accumulates as density, which is a feature)

**Physics — why this is fast:**

Particles do not interact with each other. Each particle interacts only with the *velocity field texture* at its position — a single texture lookup. This restores full embarrassing parallelism:

```glsl
// Vertex shader — each invocation is one particle
vec2 pos = texture(positionTex, particleUV).rg;
vec2 vel = texture(positionTex, particleUV).ba;

// Sample the Eulerian field at this particle's position
vec2 fieldVel = texture(velocityFieldTex, pos).rg;
float fieldDensity = texture(velocityFieldTex, pos).a;

// Euler integration
vel += fieldVel * dt;
pos += vel * dt;
pos = fract(pos);  // wrap
```

No particle needs to know where any other particle is. The inter-particle physics is approximated by the Eulerian field, which was computed separately. The Lagrangian particles are probes of the field state.

**Parameters:**
- `particleCount` — active particles (up to NxM max)
- `pointSize` — sprite radius in pixels
- `speedScale` — velocity field influence strength
- `drag` — velocity damping per frame
- `spawnMode` — random / density-weighted / depth-edge seeding
- `colorMode` — speed / age / depth / curl

**Pass structure (Rust FFGL):**
```
PASS 0 (fragment): Update position texture
  Each fragment reads one particle state, integrates, writes back

PASS 1 (vertex+fragment): Render
  GL_POINTS draw call reading from position texture
  Point sprite fragment shader: age/speed → color/alpha
```

---

## Composition

Canonical stack in Resolume:

```
stereo-depth        → depth map (greyscale)
flow-inject         → field texture (RGBA, depth-sourced)
flow-euler          → evolved field texture
flow-lagrange       → particle render (composited over source or black)
[Channel Displace]  → optional: use velocity field directly as warp
[Dream LTM]         → optional: depth-weighted feedback persistence
```

Each plugin is independently useful:
- `flow-euler` output → Channel Displace input: fluid warp with no particles
- `flow-inject` output → flow-lagrange input: particles driven directly by depth gradient, no fluid sim
- `stereo-depth` output → any existing plugin that takes a greyscale mask

---

## Build Order

1. `stereo-depth` — **done**, in `shaders/`
2. `flow-inject` — ISF, single persistent pass, straightforward
3. `flow-euler` — ISF, 4-pass, core simulation
4. `flow-lagrange` — Rust FFGL, vertex pipeline, build after Euler is working so you have a field to test against

---

## Open Questions

- **Particle birth/death in flow-lagrange:** Dead particles need recycling. Options: age in `.b` of position texture, CPU-side free list written as texture update, or just wrap all particles with infinite lifetime (VJ-appropriate — identity doesn't matter). Decision deferred.
- **Resolution of velocity texture:** flow-euler runs at what fraction of output res? 1/2 is probably sufficient — test against 1/4 for perf.
- **Field texture format:** `GL_RGBA16F` vs `GL_RGBA32F` — 16F is sufficient for velocity/density, saves bandwidth.
- **flow-inject audio input:** Requires audio FFT texture. Depends on whether Resolume exposes audio as texture to FFGL. Investigate before designing audio injection path.
