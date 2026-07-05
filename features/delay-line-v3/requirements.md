# Delay Line v3 — Two-Plugin Split + Generative Depth

## Problem

The shipped delay line is one plugin (`DLMd`) with a Mode: Read/Write toggle.
It works live but is confusing and hard to describe:

1. **One plugin, two behaviors.** FFGL can't hide inactive-mode params, so all 7
   params always show; most are irrelevant to the current mode.
2. **Wet/dry is hidden in Resolume opacity.** Read ignores its own input texture
   and relies on the host effect's opacity/blend to mix delayed vs. live — a
   mechanism we're not even certain Resolume applies to FFGL effects.
3. **The generative core is amputated.** Additive multi-map accumulation (IFS
   attractors) is unbuilt; the Tap reads only the oldest frame (no offset, no
   multi-tap); timing is latched/frame-quantized (hostile to time sweeps); the
   buffer is 8-bit and the palette (wrap/filter/precision) is hardcoded.

A critical design review (feedback-performance + generative-fractal lens)
concluded: the split is right, but do the architecture properly now and design
the depth in — don't buy describability by amputation.

## Goals

- Two separate, single-purpose plugins with clearly-labeled, self-documenting
  interfaces. Each plugin shows only its own controls.
- A real in-plugin dry/wet, named by behavior — no reliance on host opacity.
- First-class the generative behaviors: additive accumulation, expressive
  multi-tap, sweepable time, and an exposed raw palette including float buffers.
- Layered complexity: defaults give the trivial one-node echo; advanced params
  unlock sweep / multi-tap / additive / raw palette. Simple case easy, deep case
  reachable.

This redesign **absorbs `features/overdub/`** — additive accumulation becomes a
first-class Write blend mode here rather than a separate feature.

## Architecture

Two plugins over one shared GPU ring buffer:

- **Delay Write** (provisional `DlyW`) — the recorder. Writes input to a channel,
  owns loop length + feedback (Regen) + write blend mode. Its output is a
  dry/wet blend, so **one Write node alone is a complete delay**.
- **Delay Tap** (provisional `DlyT`) — the reader. Taps the buffer at a variable
  offset (with multi-tap), blends against its own input. Read-only; used to build
  `Tap → FX → Write` feedback loops with an FX insert.

### Shared buffer across DLLs

The ring buffer is currently shared via a process-global Rust `static` — this
only works because Read and Write are the same DLL. Two DLLs each get their own
copy of a static, so the buffer registry must be shared across DLLs. The GPU
textures themselves are already process-global (Resolume's shared GL context);
only the small CPU registry table (channel → texture handle, write_pos, dims,
frame-barrier state) needs sharing. Mechanism decided in the plan
(single-owner core lib vs. OS shared memory); the **deciding criterion is which
makes multi-Write-per-frame accumulation deterministic**.

### Frame barrier

Replace the 2ms wall-clock new-frame gate with a **host-provided per-frame
identifier** (host time / frame index from FFGLData) as the barrier key. This
must deterministically decide, per channel per frame: who is the first writer
(fades/clears), who are subsequent writers (accumulate), and that all writers in
a frame read the *same* source slot. Wall-clock mis-fires under variable render
load and cannot order accumulation reliably.

## Control model (behavior-named)

### Delay Write
| Control | Role |
|---|---|
| Time | Loop length. Beat-sync (subdivision/ms/frames) **or** free continuous (see Sweepable time). |
| Thru↔Playback | Output blend: dry = live thru, wet = delayed playback of the oldest frame. One node = a delay. |
| Regen | Feedback written back into the buffer. Fourth-root curve; with float buffers may exceed unity for controlled runaway. |
| Blend mode | Replace / Crossfade(decay) / **Additive** — the accumulation selector. Additive + the frame barrier = IFS attractors. |
| Channel | Target buffer channel. |

### Delay Tap
| Control | Role |
|---|---|
| Tap Offset | 0…loop_length — which point in the history to read. |
| Multi-tap | Read several offsets simultaneously (spatial echo clouds / self-similar layering). |
| Buffer Mix | Blend the tapped buffer against the Tap's own input. |
| Channel | Source buffer channel. |

The dry/wet mechanic is `out = mix(node_input, buffer_frame, wet)` on both, but
**named per behavior** so the same label never means two different things
(on Write it has a record side-effect; on Tap it doesn't).

## Sweepable time

A **free/continuous** delay-time mode that bypasses latching and interpolates
smoothly, so sweeping delay time works as an instrument gesture
(pitch/smear/zoom-rush). Beat-sync becomes an option, not the only mode.

## Raw palette + precision/resolution budget

Expose the aesthetic controls currently hardcoded:

- **Texture wrap** (clamp / repeat / mirror) — changes zoom-streak behavior.
- **Filter** (nearest / linear) — crisp/crunchy vs. soft.
- **No-clear-on-realloc** option — allow usable VRAM-garbage instead of a black
  flash.

**Buffer: Crisp ↔ Deep** macro — one describable knob that trades spatial
resolution for bit depth at ~constant VRAM, with precision (8-bit / RGB10_A2 /
RGBA16F), resolution-scale, and filter exposed underneath. Rationale:

- 720p RGBA16F ≈ 1080p RGBA8 on VRAM; lower-res float does *fewer* fragment
  invocations, so the trade is compute-neutral-to-favorable.
- **RGBA16F is the only format that can store over-unity (>1.0).** Float buffers
  and the past-unity Regen runaway are the *same* decision — 8-bit / RGB10_A2
  clamp at 1.0.
- **RGB10_A2** is a free anti-banding middle option (4× tonal levels at today's
  VRAM) when over-unity headroom isn't needed.
- Trap to document: in a Tap→FX→Write loop a low-res buffer means each generation
  down/up-samples (a generational lowpass). Float-low-res buys *deep smooth tones
  that survive many generations* but *soft fine detail* — coherent, suits IFS
  (contractive maps shrink detail sub-pixel anyway), but "deep, not crisp."

## Non-goals / fast-follow

- **Channel count increase (4–8)** for coupled loops — genuinely VRAM-expensive,
  independent of float precision. Deferred to fast-follow; architecture should
  not preclude it (`NUM_CHANNELS` a constant, not baked into assumptions).

## Constraints

- Plugin names exactly 16 bytes; each plugin needs a unique 4-byte id.
- Shared Resolume GL context: save/restore host FBO + GL_TEXTURE0.
- Cross-DLL registry sharing is a new failure surface (lifetime, load path, or
  shm cleanup across Resolume restarts) — prove it works before building UI.
- **Migration:** existing compositions using the unified `DLMd` plugin will need
  rebuilding onto the new two-plugin layout. Flag in release notes.
- VRAM: RGBA8 1080p ≈ 1.9 GB/channel today; precision/res budget must keep the
  default footprint at or below that.
