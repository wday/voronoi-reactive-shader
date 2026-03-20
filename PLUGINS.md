# Plugin Reference

Parameter reference for all plugins in this suite. For build/deploy instructions see [README.md](README.md).

---

## Rust Plugins

### Delay Line Module (`DLMd`)

Modular beat-synced frame delay with send/receive routing. Add it twice on one layer — Receive at the top, Send at the bottom — with transforms between them to create per-layer feedback loops through a shared GPU buffer. See [plugins/delay-line-module/README.md](plugins/delay-line-module/README.md) for routing diagrams and Resolume layer setup.

| Parameter | Values | Description |
|-----------|--------|-------------|
| **Mode** | Receive / Send / Tap | Receive: mix delayed frame in. Send: write to buffer. Tap: read-only (no feedback) |
| **Channel** | 1 / 2 | Independent buffer channel — pair a Send with a Receive on the same channel |
| **Sync Mode** | Subdivision / Ms / Frames | Timing source for delay length |
| **Subdivision** | 1/16, 1/8, 1/4, 1/2, 1 bar, 2 bars, 4 bars | Musical delay length at host BPM (when Sync Mode = Subdivision) |
| **Delay Ms** | 1–4000 ms | Millisecond delay (when Sync Mode = Ms) |
| **Delay Frames** | 1–239 | Frame count delay (when Sync Mode = Frames) |
| **Feedback** | 0.0–1.0 | Echo intensity (Receive mode only) |
| **Zero Tap** | 0.0–1.0 | Direct/dry tap (Send mode only) |
| **Decay** | 0.0–1.0 | Previous-iteration survival — overdub/IFS accumulation |

### Mirror Transform (`MrTx`)

Spatial transformation with edge mirroring for IFS/kaleidoscope compositions. Applies rotation, scale, swirl, and translation in a single fragment shader pass.

| Parameter | Values | Description |
|-----------|--------|-------------|
| **Scale** | 0.5×–2.0× (exp) | Zoom around center |
| **Rotation** | −180°–+180° | Rotation around center |
| **Swirl** | −2.0–+2.0 rad | Radial angular displacement |
| **Mirror** | Off / On | Reflect at edges (on) or clip to black (off) |
| **Translate X** | −1.0–+1.0 UV | Horizontal pan |
| **Translate Y** | −1.0–+1.0 UV | Vertical pan |

### Logistic Feedback (`LgFb`)

Per-channel logistic map chaos dynamics applied to image feedback. The logistic map `x → R·x·(1−x)` produces bifurcation, period-doubling, and chaos as R increases from 0 to 4.

| Parameter | Values | Description |
|-----------|--------|-------------|
| **R** | 0.0–4.0 | Logistic map parameter — stable below ~3.0, bifurcates toward chaos at 3.57+ |
| **Sensitivity** | 0.0–1.0 | Spatial modulation strength |
| **Spatial Mode** | Off / Radial / Edge | Off: uniform R. Radial: R varies by distance from center. Edge: R varies at detected edges |
| **Dry/Wet** | 0.0–1.0 | Blend original vs. chaos feedback |

### Channel Displace (`ChDp`)

Cross-channel UV displacement — each color channel samples from a UV offset derived from another channel. Useful for chromatic aberration, strange attractor dynamics, and fractal spiral patterns.

| Parameter | Values | Description |
|-----------|--------|-------------|
| **Amount** | 0.0–0.1 UV | Coupling strength (max 10% of frame) |
| **Pattern** | Cyclic / Mutual | Cyclic: R←G, G←B, B←R. Mutual: each ← average of others |
| **Angle** | 0°–360° | Direction of UV displacement |
| **Dry/Wet** | 0.0–1.0 | Blend original vs. displaced |

### Video Looper (`VdLp`)

Ring buffer looper with decay feedback. Stores frames in a circular GPU buffer and blends them for echo/ghosting effects.

| Parameter | Values | Description |
|-----------|--------|-------------|
| **Loop Beats** | 1, 2, 4, 8, 16, 32 | Number of beats to loop |
| **Decay** | 0.0–1.0 | Feedback decay per loop cycle |
| **Quality** | 0.0–1.0 | Color depth/fidelity (1.0 = pristine) |
| **Dry/Wet** | 0.0–1.0 | Live input vs. loop blend (0 = live, 1 = loop) |

### Dream LTM (`DtLm`)

Multi-tier temporal pyramid — 4 delay taps with spatial transforms (rotation, scale, swirl), color shifts, mirror/fold modes, and beat-synced timing. Generates dreamlike layered echoes with compound feedback.

| Parameter | Values | Description |
|-----------|--------|-------------|
| **Dry** | 0.0–2.0 | Live signal level (overdrivable) |
| **Wet** | 0.0–2.0 | Combined echo tap level (overdrivable) |
| **Tap 1–4** | 0.0–2.0 | Individual delay tier levels (defaults: 1.0, 0.7, 0.4, 0.2) |
| **Feedback** | 0.0–1.0 | Recursive echo decay |
| **Shift X** | −0.5–+0.5 UV | Horizontal offset per iteration |
| **Shift Y** | −0.5–+0.5 UV | Vertical offset per iteration |
| **Rotation** | −180°–+180° | Z rotation per iteration |
| **Scale** | 0.5×–2.0× (exp) | Size multiplier per iteration |
| **Swirl** | −2.0–+2.0 rad | Radial distortion per iteration |
| **Hue Shift** | −180°–+180° | Color rotation per iteration |
| **Sat Shift** | −0.5–+0.5 HSV | Saturation change per iteration |
| **Mirror** | Off / On | Reflect at edges vs. clip |
| **Fold** | 0.1–1.0 | Luminance folding threshold (1.0 = off) |
| **BPM** | 50–200 | Host tempo or manual override |
| **Subdivision** | 1/16, 1/8, 1/4, 1/2, 1 measure | Delay length in beat subdivisions |

---

## ISF Shaders

### Voronoi Reactive

3-layer multi-scale Voronoi with animated seeds, edge glow, spatial warp, and HSV coloring. Designed as a generative control surface for audio reactivity.

| Parameter | Values | Description |
|-----------|--------|-------------|
| **Density** | 2.0–30.0 | Voronoi cell scale (higher = smaller cells) |
| **Layer Spread** | 1.5–4.0 | Scale multiplier between layers |
| **Layer Mix** | 0.0–1.0 | Blend between layers |
| **Drift Speed** | 0.0–3.0 | Seed animation velocity |
| **Drift Chaos** | 0.0–1.0 | Noise complexity in seed drift |
| **Edge Width** | 0.0–0.15 | Voronoi border thickness |
| **Edge Glow** | 0.0–1.0 | Edge luminance boost |
| **Warp** | 0.0–1.0 | Spatial distortion (value noise) |
| **Color Shift** | 0.0–1.0 | HSV hue rotation |
| **Color Sat** | 0.0–1.0 | Saturation |
| **Brightness** | 0.0–2.0 | Overall luminance |
| **Contrast** | 0.0–2.0 | Tonal range |
| **Tint** | RGBA color | Overlay color |
| **Image Blend** | 0.0–1.0 | Mix with input image |
| **Image Influence** | 0.0–1.0 | Input luminance modulates cell size |
| **NC Kernel** | 0.01–0.5 | Normalized convolution kernel size |
| **Cert Contrast** | 0.1–5.0 | Certainty sharpness |
| **Cert Brightness** | 0.0–1.0 | Certainty offset (0 = off) |

### Flow Inject

Writes scene information into a persistent flow field texture. Reads depth/greyscale and outputs velocity (RG), scalar (B), and density (A). Route upstream of Flow Euler for Eulerian fluid effects.

| Parameter | Values | Description |
|-----------|--------|-------------|
| **Velocity Scale** | 0.0–5.0 | Gradient-to-velocity multiplier |
| **Density Scale** | 0.0–5.0 | Edge detection to density multiplier |
| **Edge Threshold** | 0.0–0.2 | Minimum gradient for density injection |
| **Decay Rate** | 0.0–1.0 | Frame-to-frame field persistence |

### Flow Euler

Eulerian fluid simulation with semi-Lagrangian advection and ping-pong persistent textures. Input field convention: RG = velocity, B = curl, A = density.

| Parameter | Values | Description |
|-----------|--------|-------------|
| **Timestep** | 0.01–2.0 | Simulation substep size (smaller = more stable) |
| **Viscosity** | 0.0–1.0 | Velocity diffusion (smoothing) |
| **Decay** | 0.8–1.0 | Field persistence per frame |
| **Curl** | 0.0–2.0 | Vorticity confinement strength |
| **Boundary** | Wrap / Reflect / Absorb | Edge behavior |
| **Input Mix** | 0.0–1.0 | Blend new input with advected field |
