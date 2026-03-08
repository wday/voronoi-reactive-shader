# Delay Line Module — Implementation Plan

## Architecture

A single FFGL plugin with **Send/Receive mode** that creates feedback delay loops through shared GPU buffer channels. Native Resolume effects placed between Receive and Send compound through the feedback loop automatically.

### Per-frame signal flow

**Send mode**: writes current frame to shared buffer, outputs delayed frame (Approach B)
```
input → [write to buffer[write_pos]] → output buffer[write_pos - D]
```

**Receive mode**: mixes delayed echo into live signal
```
input → output (input + feedback × buffer[write_pos - D])
```

**Combined on one layer**:
```
webcam → Receive(mix echo) → [Resolume FX] → Send(write + output delayed)
```

### Shared buffer registry

Global static keyed by channel (1–4). Each channel holds:
- `GL_TEXTURE_2D_ARRAY` — 900-layer ring buffer
- Write pointer — advanced by Send each frame
- Resolution — set by Send, Receive adapts via GL bilinear scaling

Allocated on first Send write. All instances in the same DLL share the registry.

### Delay calculation

```
fps_estimate = lerp(fps_estimate, 1.0 / frame_delta, 0.05)
D = round((60.0 / bpm) * subdivision_beats * fps_estimate)
D = clamp(D, 1, buffer_size - 1)
```

Read pointer derived fresh each frame from `write_pos - D`. No drift possible.

## Files

```
plugins/delay-line-module/
├── Cargo.toml
└── src/
    ├── lib.rs          # entry point: plugin_main!
    ├── delay.rs        # DelayLine struct + SimpleFFGLInstance (Send/Receive logic)
    ├── params.rs       # 4 params: Mode, Channel, Subdivision, Feedback
    ├── registry.rs     # global shared buffer registry (channels 1-4)
    ├── shader.rs       # quad geometry, shader loading, GL helpers
    └── shaders/
        ├── fullscreen.vert.glsl   # shared vertex shader
        ├── write.frag.glsl        # passthrough: sampler2D → FBO layer
        ├── read.frag.glsl         # sample: sampler2DArray + layer → output
        └── receive.frag.glsl      # mix: input + feedback × buffer[layer]
```

## Parameters

| # | Name | Type | Range | Default | Notes |
|---|------|------|-------|---------|-------|
| 0 | Mode | Option | Receive, Send | Receive | Switches plugin behavior |
| 1 | Channel | Option | 1, 2, 3, 4 | 1 | Pairs Send with Receive |
| 2 | Subdivision | Option | 1/16–4 bars | 1/4 | Delay length (both modes) |
| 3 | Feedback | Standard | 0.0–1.0 | 0.5 | Receive only: echo intensity |

## Key design decisions

- **Send outputs delayed frame (Approach B)**: ensures first visible echo is already transformed — no "dry echo" artifact
- **Single DLL with mode parameter**: all instances share the global buffer registry naturally (same address space)
- **GPU-only texture array**: no PBO transfers, no system RAM, no CPU pixel work
- **FPS via EMA**: measured from frame deltas, converges within ~1 second, no drift by construction
- **Additive receive mix**: `output = input + feedback × delayed`, clamped to [0,1]
