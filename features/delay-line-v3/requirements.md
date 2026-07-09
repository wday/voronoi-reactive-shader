# Delay Line v3 — Two Atoms of a Modular Feedback Engine

## Problem

The shipped delay line is one plugin (`DLMd`) with a Read/Write mode toggle. It
works live but is confusing and hard to describe. The first v3 redesign split it
into two plugins but kept piling on: Thru↔Playback, Buffer Mix, Regen, Blend
mode, variable multi-tap, additive multi-writer IFS accumulation, float buffers,
a Crisp↔Deep precision budget. Live testing of that build surfaced the real
issue — **feedback lived in too many places with mode-dependent meanings**, and
the same label (a shared "Dry/Wet") meant different things per node. It was
close to working but nobody could describe or reliably re-patch it.

Root-cause insight from the refinement session: **every single blend knob is a
crossfade, so it always couples "how much source enters" with "how much loop
sustains."** Thru↔Playback, Buffer Mix, Regen-as-mix — all the same crossfade in
different hats. The only thing that actually decouples them is two independent
gains. So the simplest atom is: expose two gains, name each by behavior, put each
on exactly one node. Everything else is a later refinement, not an atom.

## Goal

The **smallest composable building blocks of a modular feedback engine**: a read
head and a record head over a shared tape, that stack as

```
source → [Tap] → fx1 → fx2 → [Write] → output
```

Minimum *playable* controls (not minimum *possible*): five knobs total, each a
plain gain or a sync value, each doing one thing, none coupled.

This redesign **absorbs `features/overdub/`** (additive accumulation is now the
Write's inherent record math) and **supersedes** the shipped unified `DLMd`.

## The two atoms

### Delay Write (`DlyW`) — the record head
Pushes the current frame onto the tape and advances one slot per frame. It is
**visually transparent**: its output is a passthrough of its input ("you see the
write-head input at all times"). A Write with no Tap records into the aether.

Record math (single writer per channel → a trivial read-modify-write, no
multi-writer barrier):

```
tape[slot] = (1 − Send) · Regen · old  +  Send · input
```

Because the Tap reads the very slot the Write overwrites, `old` is the *same*
delayed frame the Tap already fed back through Wet. Gating Regen by `(1 − Send)`
stops the two feedback paths double-counting: the **single loop-feedback
coefficient is `(1−Send)·Regen + Send·Wet`** — at Send = 1 it is **Wet alone**,
and as Send → 0 it becomes Regen (the ring-out). Feedback ≥ 1 sustains/blooms;
< 1 fades. `Dry` is *injection only* (`Send·Dry`) — it never sets the tail.

| Control | Role |
|---|---|
| **Channel** | The patch point. Tap + Write on the same Channel form a loop. |
| **Time** | Tape length / delay. Beat-sync: Sync Mode ∈ {Subdivision, Ms, Frames} + the value. |
| **Regen** | Ring-out rate: how much of the slot survives when you STOP sending (Send→0). Fourth-root curve. Inactive at Send = 1. |
| **Send** | Dub throw — how hard the current frame commits. At 1 the loop feedback is Wet; pulse it down (Tap Wet high) and echoes ring out at the Regen rate. |

### Delay Tap (`DlyT`) — the read head
Sits first in the stack. Reads the tape at a **fixed full delay** (one lap back —
the slot the Write is about to overwrite) and carries it forward with the source:

```
out = clamp(Dry · source  +  Wet · tape[full-delay])
```

| Control | Role |
|---|---|
| **Channel** | Source tape (the patch point). |
| **Dry** | Gain on the live source into the FX chain — the source's *injection* path. Does NOT set the tail; Dry=0 → no new material, loop rings out per the feedback. |
| **Wet** | Gain on the delayed tape → the loop's **feedback** (at full Send). Wet=1 = infinite echo, Wet<1 = decays. The downstream FX re-process it each lap. Hold high, pulse Write Send, for dub echoes. |

Two independent gains, **not** a crossfade — Dry and Wet are decoupled, so you can
hold a long tail *and* pulse new material in.

### Delay Core (`delay_core.dll`) — the shared tape
Unchanged from Stages 0–2: a C-ABI cdylib owning the GPU ring buffer + a
per-frame advance barrier, shared across the two plugin DLLs (each runtime-loads
it from its own directory via pluglib). Single writer per channel means the
barrier just guarantees one advance per frame; the hard multi-writer determinism
is not exercised in this model.

## The opacity rule (the crux that unblocked this)

Resolume feeds each stacked effect the previous one's output as a single texture,
and applies its own per-effect opacity crossfade (behavior on FFGL effects was
never certain). **Rule: keep every effect at 100% opacity and do all mixing
in-shader.** At 100% the host crossfade is a no-op regardless of whether it
applies to FFGL, so:

- **Write is passthrough** → at 100% it shows its input (transparent recorder).
- **Tap samples its own input** → its output already contains the source, so the
  source survives at 100% (Dry is its gain). We never depend on host opacity.

This is why the old model felt fragile: the Read plugin ignored its input and
leaned on host opacity to reintroduce the source. The Tap carrying the source
itself removes that dependency and the seeding chicken-and-egg with it.

## How the friction is designed out

- **Seeding chicken-and-egg** → gone. Write always records its input; Tap always
  carries the source (Dry). The loop fills on lap one.
- **Overloaded Buffer Mix** → split into Dry (source in) and Wet (feedback),
  different gains on different nodes.
- **Read-timing trick** → gone. Tap is upstream of Write, so it reads the slot
  pre-overwrite (one full lap old); no manual offset workaround.
- **Three mode-dependent mix knobs** → five single-purpose knobs, none coupled.
- **Blend-mode selector** → gone. Additive-with-decay *is* the one record mode.

## The dub gesture

**Wet** sets the tail (the loop's feedback), and the FX re-process that feedback
each lap. **Pulse Send** to throw new video into the ringing loop; drop Send to 0
and the echoes ring out at the **Regen** rate instead of cutting to black. So the
two knobs have non-overlapping jobs: Wet = how much it feeds back while you're
sending, Regen = how it decays once you stop. `Dry` only injects fresh source and
never touches the tail — muting Dry does not stop a loop whose feedback (Wet) is
at unity, which is expected: Wet = 100% is an infinite echo by definition.

## Non-goals / fast-follow (bolt onto working atoms, not atoms)

- **Variable + multi-tap** (Tap Offset, K taps). Fixed full-delay for now.
- **Over-unity float buffers** (RGBA16F) + the Crisp↔Deep precision/resolution
  budget. Send + Regen can exceed 1.0 → for now clamp at 8-bit; let bloom clip.
- **Sweepable/continuous Time.** Beat-sync (latched) for now.
- **Raw palette** (wrap / filter / no-clear-on-realloc).
- **Additive multi-writer IFS accumulation** (N Writes summing into one slot with
  a deterministic barrier). Single-writer only; this is the biggest deferred item
  and the one that forced the hardest cross-DLL determinism.
- **Channel count 4–8.** `NUM_CHANNELS` stays a constant, not baked in.

## Constraints

- Plugin names exactly 16 bytes; unique 4-byte ids (`DlyW`, `DlyT`).
- Shared Resolume GL context: save/restore host FBO + GL_TEXTURE0.
- Deploy `delay_core.dll` beside both plugin DLLs (they load it from their own
  directory).
- **Usage rule:** keep all three effects at 100% opacity; place the Tap first.
- **Migration:** existing `DLMd` compositions rebuild onto the two plugins.
