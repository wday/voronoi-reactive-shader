# Delay Line Module — User Manual

## What it is

A beat-synced frame delay effect for Resolume that creates feedback loops and
echo trails. One DLL, two modes — **Read** and **Write** — that communicate
through shared GPU buffer channels.

The delay line does one thing Resolume can't: hold frames in a GPU ring buffer
and read them back later. Everything else — spatial transforms, color, blur —
is handled by native Resolume effects or companion plugins placed *between*
Read and Write in the effect chain. The delay line is the clock; the FX chain
is the function.

## Parameters

| # | Name | Type | Range | Default | Applies to |
|---|------|------|-------|---------|------------|
| 0 | Mode | Option | Read / Write | Read | — |
| 1 | Channel | Option | 1 / 2 | 1 | Both |
| 2 | Sync Mode | Option | Subdivision / Ms / Frames | Subdivision | Write |
| 3 | Subdivision | Option | 1/16 – 4 bars | 1/4 | Write (sync = Subdivision) |
| 4 | Delay Ms | Integer | 1 – 4000 | 500 | Write (sync = Ms) |
| 5 | Delay Frames | Integer | 1 – 239 | 30 | Write (sync = Frames) |
| 6 | Decay | Standard | 0.0 – 1.0 | 0.0 | Write |

FFGL doesn't support conditional visibility, so all 7 parameters are always
shown. Parameters that don't apply to the current mode are ignored.

**Read ignores every timing parameter.** Read always outputs the *oldest*
frame in the buffer, which is exactly `loop_length` frames old — and
`loop_length` is set entirely by the Write instance on the same channel. There
is no per-instance tap offset; the loop length is a property of the channel,
owned by Write.

## The two modes

### Write

Writes the incoming frame into the ring buffer, advances the write position by
one slot per frame, and outputs what it just wrote. Write owns all timing.

- **Sync Mode / Subdivision / Delay Ms / Delay Frames** set the **loop length**
  (how many frames the buffer holds before it wraps). See *Loop length* below.
- **Decay** controls how much of the buffer's previous contents survive when a
  new frame is written:

  ```
  frame[write_pos] = decay * old + (1 - decay) * input
  ```

  At `decay = 0.0` each write is a clean overwrite (default). Above 0.0, the
  slot's previous contents — which are one full loop old — are crossfaded with
  the new input, producing dub-echo trails that regenerate every loop period.

  The Decay knob is **curved** (fourth-root): most of its travel maps to the
  useful 0.90–0.99 range instead of cramming it into the last 10% of the knob.
  Knob ≈ 0.65 gives ~0.90 decay; knob ≈ 0.95 gives ~0.99.

### Read

Outputs the oldest frame in the buffer — `loop_length` frames old — and nothing
else. Read has no timing logic and **ignores its own input texture**.

Because Read discards its input and emits buffer content, you control how much
delayed signal mixes with the live source using **Resolume's native effect
controls on the Read instance**:

- The Read effect's **opacity / mix** slider crossfades between the live source
  (the effect's dry input) and the buffer (the effect's wet output). This is the
  feedback level — 0% = pure live source, 100% = pure delayed buffer.
- The Read effect's **blend mode** (Add, Screen, etc.) determines *how* the
  buffer composites over the source.
- A gain/exposure effect *before* Read scales the live source that Resolume
  will blend in — this is the "how much live signal enters the loop" control.

If no Write has written to the selected channel yet, Read outputs black.

## The feedback loop: Read → FX → Write

This is the core pattern, on a single Resolume layer. Effect chain, top to
bottom in Resolume (which is the processing order):

```
  1. [gain / exposure]                        — set live-source strength
  2. Delay Line [Read, ch 1, opacity 70%, Add] — mix buffer over source
  3. Transform (rotate 15°, scale 0.95)        — the feedback FUNCTION
  4. Blur (subtle)
  5. Delay Line [Write, ch 1, 1/4 note, decay] — write result, set loop length
```

Per-frame signal flow:

1. Read replaces the image with the buffer frame from `loop_length` frames ago;
   Resolume's opacity/blend on the Read effect mixes that with the live source.
2. Resolume's native effects transform the mixed signal.
3. Write writes the transformed result back to the buffer and outputs it.

Each pass around the loop accumulates another application of the FX chain. The
spiral / zoom / trail / drift emerges from the native effects, not from the
delay line itself. **The FX chain between Read and Write IS the feedback
function.**

### Who controls what

| Concern | Controlled by |
|---------|--------------|
| Loop length / echo timing | Sync params on **Write** |
| How much past mixes into the loop | **Opacity + blend mode** on the Read effect (Resolume-native) |
| How much live signal enters | Gain/exposure effect before Read |
| Spatial transform per echo | Native Resolume effects between Read and Write |
| Trail persistence within a loop | **Decay** on Write |
| Overall brightness / blowout | Layer opacity, color-correction effects |

## Loop length and the ring buffer

Write sets the **loop length** L (in frames) from its Sync Mode:

- **Subdivision** — beat-synced. L is derived from host BPM and a measured FPS
  estimate (e.g. a 1/4 note at 120 BPM ≈ 30 frames at 60 fps).
- **Ms** — L from a millisecond value and the FPS estimate.
- **Frames** — L set directly (1–239), independent of BPM/FPS. Most predictable.

Internally the buffer allocates `L + 1` slots. Write advances to a new slot each
frame and writes there; Read reads `(write_pos + 1) % (L+1)` — the oldest slot,
which is exactly L frames old. The extra "+1 stitch" slot guarantees Read and
Write never touch the same slot in a frame.

Timing is **latched**: L is only recomputed when BPM or a sync parameter
actually changes (BPM by >0.5), so FPS-estimate jitter doesn't make the loop
length flicker frame to frame.

Notes and edge cases:

- **Read follows Write's loop length.** Put Read and Write on the same channel;
  the delay period is whatever Write is set to. Read has no separate delay.
- **No Write running → Read is stale or black.** If nothing writes the channel,
  Read freezes on the last content, or outputs black if nothing ever wrote it.
- **Max loop length is 239 frames** (buffer depth is 240 slots).

## Channels

Two independent channels (1 and 2), each with its own buffer. A Read and a Write
on the same channel form one loop. Cross-coupling is possible: a Write on
channel 2 fed by a Read on channel 1 (and vice versa) creates two loops that
feed each other — see Pattern 2.

## Layer routing patterns

### Pattern 1: Single-layer feedback trail

```
Layer 1 (bottom): Camera source, opacity 0%
Layer 2 (Add):    VideoRouter→L1,
                    [Exposure] → Read(ch1, opacity 70%, Add)
                      → Rotate(15°) → Scale(0.95)
                      → Write(ch1, 1/4, decay 0.0)
Layer 3 (top):    VideoRouter→L1 (dry output)
```

Layer 1 is invisible but routable (Video Router ignores layer opacity). Layer 2
is the self-contained feedback loop: Read mixes the buffer with the routed
source, the transform is the feedback function, Write stores the result and
sets the loop length. Layer 3 shows the dry camera on top.

### Pattern 2: Dual-channel cross-feedback

```
Layer 1 (bottom): Camera, opacity 0%
Layer 2 (Add):    VideoRouter→L1, Read(ch1, 50%, Add) → BlueShift → Write(ch2, 1/4)
Layer 3 (Add):    VideoRouter→L1, Read(ch2, 50%, Add) → RedShift  → Write(ch1, 1/8)
Layer 4 (top):    Read(ch1, 100%) blended with Read(ch2, 100%)
```

Channel 1's loop feeds channel 2 and vice versa, with different color shifts and
loop lengths on each path. Layer 4 uses two pure Reads (opacity 100%) purely to
display the two buffers.

### Displaying a buffer without writing

A Read at **opacity 100%** on a layer whose source is anything (its input is
ignored) shows the raw buffer content for that channel — useful as a dedicated
output/monitor layer, as in Pattern 2's Layer 4.

## Roadmap: IFS fractal accumulation (not yet supported)

An earlier design aimed to build iterated function systems (e.g. a Sierpinski
triangle) by having **several Write instances on one channel each frame**, each
preceded by a different contractive transform, all *additively* accumulating
into the same buffer slot.

**This does not work in the current build.** Multiple Writes to the same channel
in the same frame share one slot, but each Write re-applies its own decay
crossfade (`decay * (decay * old + …) + …`), so contributions don't cleanly sum
into an attractor. True IFS accumulation needs an additive (`GL_ONE, GL_ONE`)
write path with first-vs-subsequent Write coordination — tracked as the open
"overdub" feature. Until then, treat multi-Write-per-channel-per-frame as
unsupported.

The affine math is kept here for when accumulation lands. Sierpinski = three
maps, each scaling to 0.5 and translating to a triangle vertex:

| Map | Scale | Translate X | Translate Y |
|-----|-------|-------------|-------------|
| T1 (top)          | 0.5 | 0.0   | +0.25 |
| T2 (bottom-left)  | 0.5 | -0.25 | -0.25 |
| T3 (bottom-right) | 0.5 | +0.25 | -0.25 |

Mirror Transform mapping — scale is exponential `actual = 2^(param*2 - 1)` (so
0.5× → param 0.0); translate is linear `actual = param*2 - 1`:

| Map | Scale param | Rotation param | Translate X param | Translate Y param |
|-----|-------------|---------------|-------------------|-------------------|
| T1 (top)          | 0.0 | 0.5 | 0.5   | 0.625 |
| T2 (bottom-left)  | 0.0 | 0.5 | 0.375 | 0.375 |
| T3 (bottom-right) | 0.0 | 0.5 | 0.625 | 0.375 |

## Troubleshooting

**No output from Read**: No Write has written to the channel yet. Check that a
Write instance exists on the same channel and its layer is enabled.

**Read shows only the live source (no delay)**: The Read effect's opacity is too
low — raise it toward 100% to bring in more buffer content. Remember the mix is a
Resolume-native effect control, not a plugin parameter.

**Echo blows out to white**: Read opacity too high combined with an Add blend, or
Decay too high on Write. Lower the Read opacity, switch its blend to Screen, or
add a Levels / Brightness-Contrast effect in the chain to attenuate.

**Ghost images / VRAM artifacts**: Buffer allocation may have failed silently on
a GPU with limited VRAM. Each channel at 1080p uses ~1.9 GB (240 slots). Two
channels ≈ 3.8 GB. Check your GPU has headroom beyond what Resolume itself uses.

**Timing feels wrong at high BPM**: Subdivision and Ms modes derive frame counts
from BPM and an FPS estimate. The FPS estimate is an exponential moving average —
it takes ~2 seconds to stabilize after launch. Use Frames mode to bypass it
entirely.

**Delay period won't change**: Loop length is owned by Write, not Read. Adjust
the sync parameters on the Write instance; Read has no delay control.
