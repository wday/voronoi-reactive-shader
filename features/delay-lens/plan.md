# Delay Lens — Implementation Plan

Prerequisite (**done 2026-08-29**): full-res tape. `TAPE_SCALE = 1.0`,
`BUFFER_DEPTH = 120`, built + deployed. The knobs get tuned against a clean loop,
not against the old half-res mush — that sequencing was deliberate.

All work lands in two files plus params:
- `plugins/pluglib/src/shaders/output.frag.glsl` — the shared node output. **Careful:
  Delay Write draws through this too** (as `dry=1, wet=0, gamma=1` passthrough), so
  every new uniform needs a zero default that leaves the Write's path untouched.
- `plugins/delay-tap/src/params.rs` — 5 appended params.
- `plugins/delay-tap/src/lib.rs` — plumb them into `output.draw(...)`.

## Phase 0 — Guard the current behaviour

Before adding anything: a Tier-2 headless assertion that the Tap's output for a
known input is what it is today. This is the C3 regression net every later phase
leans on. Without it, "zero = unchanged" is an assumption rather than a test.

`OutputProgram::draw` signature grows; consider a small `LensParams` struct rather
than nine positional `f32`s (it's already at 8 args with `#[allow(clippy::too_many_arguments)]`
on the caller).

## Phase 1 — The two free ones (Vignette, Grain)

Start here: no extra texture fetches, no filter design, and they are the two most
likely to change how the loop *behaves* rather than how it looks.

1. **L3 Vignette** — `smoothstep` on `length(v_uv - 0.5)`, multiplied into `buf`.
   In linear mode apply after decode (C5).
2. **L4 Grain** — Hoskins hash, keep the credit line. Needs a per-frame seed
   uniform; `frame_count` already exists on `DelayTap`.

Check in Resolume before going further. Vignette in particular should visibly
stabilize a loop that currently saturates, and if it doesn't, the model behind
this whole feature is wrong and that's worth knowing at step one.

## Phase 2 — Aberration (L2)

Radial per-channel scale about frame centre. Three samples of `u_buffer` at
scaled uv, take `.r`, `.g`, `.b`. Bipolar.

Watch: the existing `sampleContent` clamp guards the *input* against NPOT padding;
the tape is our own exactly-sized texture so it needs its own clamp only if the
scaled uv can leave [0,1] — it can, at max aberration. Clamp to texel centres, same
pattern as `reference_npot_edge_seam`.

## Phase 3 — Shear (L5)

Pass `buf_size` as a uniform (Rust already has it). Per-row layer offset, wrapped
in ring space. Verify the wrap against `delay_dsp::Ring` rather than reimplementing
modulo logic in GLSL by eye — an off-by-one here reads as a one-frame tear, which is
exactly the class of bug the last four commits on this repo were about.

## Phase 4 — Focus / Bloom (L1)

Last, because it's the only one that needs filter design and the only one that can
destabilize the loop.

- Bloom: 5-tap tent on the tape read.
- Focus: unsharp against that same blur — reuse the taps, don't compute two kernels.
- Resolve **Q1** (one bipolar knob vs. two) by feel once it's in.

## Phase 5 — Live pass

Tune all five together at a 3-frame loop and at a 1/4 note. The interactions are the
point: Vignette + Focus should hold a fractal loop stable where Focus alone runs
away, and Grain should be what keeps it from settling.

Update `devlog.md` as decisions land, per the repo's feature pattern.

## Deploy

`make build PLUGIN=delay_tap && make deploy PLUGIN=delay_tap` — Resolume must be
fully closed or the DLL copy is refused (it now fails loudly, commit e138c84).

⚠️ Resolume caches param descriptors per `unique_id`. If the five new knobs don't
appear after deploy, the fix is bumping `DlyT` in `delay-tap/src/lib.rs:211` — which
makes it a **new** effect and orphans the old one in existing comps. Check this
before a gig, not during load-in.
