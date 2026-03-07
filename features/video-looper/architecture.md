# Video Looper — Architecture & Data Flow

## Per-Frame Pipeline

The plugin runs as a Resolume effect. Each frame, Resolume calls `draw()` with
the input texture (whatever is upstream in the effect chain). The plugin acts as
a video delay line with feedback.

```mermaid
flowchart TD
    subgraph "Frame N draw() call"

        subgraph "1. Finish previous frame's download"
            PBO_prev["PBO (mapped)"] -->|"memcpy ~8MB"| tmp["tmp_pixels buffer"]
            tmp -->|"write_frame()"| RB_write["Ring Buffer\n[prev_write_pos]"]
        end

        subgraph "2. Upload delayed frame to GPU"
            RB_read["Ring Buffer\n[write_pos]"] -->|"memcpy into PBO"| PBO_up["Upload PBO"]
            PBO_up -->|"glTexSubImage2D\n(DMA, no CPU)"| loop_tex["loop_texture\n(GPU)"]
        end

        subgraph "3. GPU decay blend"
            input_tex["Input Texture\n(from Resolume)"] --> mix1["Mix Shader\nmix(input, delayed, decay)"]
            loop_tex --> mix1
            mix1 -->|"renders to"| blend_fbo["Blend FBO\n→ blend_texture"]
        end

        subgraph "4. Async download blend result"
            blend_fbo -->|"glReadPixels into PBO\n(async, no stall)"| PBO_down["Download PBO\n(ready next frame)"]
        end

        subgraph "5. GPU output"
            input_tex2["Input Texture"] --> mix2["Mix Shader\nmix(input, delayed, dry_wet)"]
            loop_tex2["loop_texture"] --> mix2
            mix2 -->|"renders to"| screen["Screen\n(Resolume output)"]
        end

    end

    style blend_fbo fill:#f96,stroke:#333
    style screen fill:#6f9,stroke:#333
    style RB_write fill:#69f,stroke:#333
    style RB_read fill:#69f,stroke:#333
```

## Memory Layout

```
Ring Buffer (system RAM)
┌────────┬────────┬────────┬────────┬─── ─── ───┬────────┐
│frame 0 │frame 1 │frame 2 │  ...   │           │frame N │  N = 30fps × 30sec = 900
└────────┴────────┴────────┴────────┴─── ─── ───┴────────┘
    ▲
    │ write_pos wraps here
    │ (loop_len frames, set by BPM × loopBeats)
    │
    ├─── write_pos advances by 1 each frame
    │    loop_len might be 60 frames (4 beats @ 120 BPM)
    │    so only frames 0..59 are "active"
    │    frames 60..899 sit unused until loopBeats changes
```

## PBO Double-Buffering

```
Frame N:   begin_download → PBO[0]     (GPU starts async DMA)
Frame N+1: begin_download → PBO[1]     (GPU starts async DMA)
           finish_download ← PBO[0]    (map + memcpy, DMA already done)
Frame N+2: begin_download → PBO[0]     (reuse, orphan old data)
           finish_download ← PBO[1]    (map + memcpy)
```

The key insight: we always read from the PBO we filled **two frames ago**,
giving the GPU time to complete the async transfer. Same pattern for uploads.

## GL Resources (created once per resolution)

| Resource        | Type       | Purpose                                      |
|-----------------|------------|----------------------------------------------|
| `loop_texture`  | GL texture | Holds the delayed frame uploaded from buffer  |
| `blend_texture` | GL texture | Render target for GPU decay blend             |
| `blend_fbo`     | GL FBO     | Framebuffer attached to blend_texture         |
| `read_fbo`      | GL FBO     | Used by PBO download (glReadPixels source)    |
| `download_pbos` | GL PBO ×2  | Double-buffered GPU→RAM transfer              |
| `upload_pbos`   | GL PBO ×2  | Double-buffered RAM→GPU transfer              |

## Parameter Effect on Data Flow

| Param      | Where it acts                     | What it does                        |
|------------|-----------------------------------|-------------------------------------|
| loopBeats  | write_pos wrap point              | Sets active region of ring buffer   |
| decay      | GPU blend shader (step 3)         | 0=fresh input, 1=keep old frame     |
| quality    | CPU blur on wrap (once per cycle) | Tape wear: blur all active frames   |
| dry/wet    | GPU output shader (step 5)        | 0=live input, 1=loop only           |

## Source File Map

| File             | Responsibility                              | Lines |
|------------------|---------------------------------------------|-------|
| `lib.rs`         | Entry point, wires plugin to ffgl-core      | ~10   |
| `looper.rs`      | Main struct, draw loop, GL resource mgmt    | ~200  |
| `ring_buffer.rs` | RAM frame storage, degradation blur         | ~100  |
| `pbo.rs`         | PBO lifecycle, async GPU↔RAM transfers      | ~170  |
| `params.rs`      | FFGL parameter definitions and mapping      | ~85   |
| `shader.rs`      | Vertex/fragment shader, quad rendering      | ~200  |
