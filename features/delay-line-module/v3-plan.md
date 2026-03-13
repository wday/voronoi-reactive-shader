# Delay Line v3 — Split Send/Receive with SSBO Side Channel

## Goal

Replace single delay-line-module (3 modes, 7 params, phantom controls) with two
focused plugins that communicate via a GPU-side convention.

## Architecture

```
delay-line-send.dll          delay-line-receive.dll
  ┌──────────────┐             ┌──────────────────┐
  │ Owns buffer  │             │ Reads buffer      │
  │ Owns SSBO    │             │ Discovers SSBO    │
  │ Writes meta  │──SSBO@15──▶│ Reads meta        │
  │ 5 params     │             │ 6 params          │
  └──────────────┘             └──────────────────┘
```

## SSBO Protocol

Binding point: `GL_MAX_SHADER_STORAGE_BUFFER_BINDINGS - 1` (query at init, typically 15 or 95).

Layout (40 bytes):
```
offset  0: u32 magic        = 0xDE1A7000
offset  4: u32 buffer_depth = 240
offset  8: u32 ch0_tex       ← GL texture array handle
offset 12: u32 ch0_write_pos
offset 16: u32 ch0_width
offset 20: u32 ch0_height
offset 24: u32 ch1_tex
offset 28: u32 ch1_write_pos
offset 32: u32 ch1_width
offset 36: u32 ch1_height
```

Rules:
- Send creates the SSBO + texture arrays. Writes metadata each frame.
- Receive discovers SSBO by binding point, validates magic, reads metadata.
- Both bind/unbind within their draw call — don't leave it bound.
- Receive caches the SSBO buffer ID after first discovery.
- Receive re-validates magic each frame. Stale/missing → passthrough.
- FBO is Send-local (only Send writes to the texture array).

## Parameters

**delay-line-send** (5 params):
| # | Name | Type |
|---|------|------|
| 0 | Channel | Option: 1, 2 |
| 1 | Sync Mode | Option: Subdivision, Ms, Frames |
| 2 | Subdivision | Option: 1/16 .. 4 bars |
| 3 | Delay Ms | Standard: 0–4000 |
| 4 | Delay Frames | Standard: 1–239 |

**delay-line-receive** (6 params):
| # | Name | Type |
|---|------|------|
| 0 | Channel | Option: 1, 2 |
| 1 | Sync Mode | Option: Subdivision, Ms, Frames |
| 2 | Subdivision | Option: 1/16 .. 4 bars |
| 3 | Delay Ms | Standard: 0–4000 |
| 4 | Delay Frames | Standard: 1–239 |
| 5 | Feedback | Standard: 0.0–1.0 |

## Plugin IDs

- Send: `DLSn`, name `"DL Send         "` (16 bytes)
- Receive: `DLRx`, name `"DL Receive      "` (16 bytes)

## Crate Structure

```
plugins/
  delay-line-send/
    Cargo.toml
    src/lib.rs         # FFGL entry
    src/send.rs        # draw: create buffer, write, output delayed, update SSBO
    src/params.rs      # 5 params
    src/shader.rs      # QuadGeometry, write_pass, read_pass
    src/protocol.rs    # SSBO create/write
    src/shaders/
      fullscreen.vert.glsl
      write.frag.glsl
      read.frag.glsl
  delay-line-receive/
    Cargo.toml
    src/lib.rs         # FFGL entry
    src/receive.rs     # draw: read SSBO, read buffer, mix with input
    src/params.rs      # 6 params
    src/shader.rs      # QuadGeometry, write_pass (passthrough), receive_pass
    src/protocol.rs    # SSBO discover/read
    src/shaders/
      fullscreen.vert.glsl
      write.frag.glsl
      receive.frag.glsl
```

Code duplication: ~200 lines (shader.rs, delay calc, GL state save/restore).
protocol.rs is small (~60 lines) and mirrored (write side vs read side).
No shared crate — the convention IS the interface.

## Implementation Order

1. Create delay-line-send crate (extract from delay-line-module, add SSBO write)
2. Create delay-line-receive crate (extract from delay-line-module, add SSBO read)
3. Update plugins/Cargo.toml workspace, plugins.json
4. Build + deploy both, test in Resolume
5. Keep delay-line-module in tree until confirmed working, then remove

## Risks

- **SSBO binding collision**: Mitigated by using max-1 binding point + magic validation.
  If collision occurs, Receive falls back to passthrough (safe failure).
- **Resolume touching SSBO state**: Low probability — SSBO is GL 4.3 compute-era feature,
  Resolume's renderer is fragment-shader-based. Validated by magic number each frame.
- **Plugin load order**: Receive before Send → passthrough until Send runs. Clean.
- **Send removed mid-session**: SSBO becomes stale. Magic check catches this if the
  buffer object is deleted. If it persists with old data, Receive reads stale write_pos
  (frozen echo, not a crash). Acceptable degradation.
