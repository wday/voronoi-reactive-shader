# Delay Line v3 — Implementation Plan

Sequenced to de-risk cross-DLL sharing first, then build the two atoms, then
(only if wanted) layer refinements. Stages 0–3 are built; the model was then
**refined down** (see devlog 2026-07-05) to the two-atom / five-knob engine in
`requirements.md`. Everything past the refinement is fast-follow, not required.

## Stage 0 — Cross-DLL buffer sharing — DONE

Proved two plugin DLLs share one `delay_core` registry + barrier, on the Windows
loader (the primary platform). Core-lib approach (C-ABI cdylib, runtime-loaded by
pluglib from the plugin's own directory). See devlog.

## Stage 1 — Frame barrier — DONE

Per-frame advance keyed off `FFGLData.host_time` (whole ms). Single writer per
channel → the barrier just guarantees one advance per frame. (Multi-writer
determinism is only needed for the deferred additive-IFS refinement.)

## Stage 2 — Two plugin skeletons — DONE

`plugins/delay-write` (`DlyW`) + `plugins/delay-tap` (`DlyT`) on the shared core,
plus `pluglib` (loader + GL helpers). Deploy-ready.

## Stage 3 → Refinement — the two-atom model (this pass) — DONE (build)

Collapsed the over-built control surface to the five-knob engine:

- **Write**: dropped Thru↔Playback + the Blend-mode selector; added **Send**. One
  record path: `tape[slot] = Regen·old + Send·input` (fade pre-pass scales the
  slot by Regen, additive `CONSTANT_COLOR=Send, dst=ONE` write adds Send·input).
  Output is now a pure passthrough of the input.
- **Tap**: dropped Tap Offset + Multi-tap + Buffer Mix; added **Dry** + **Wet**.
  Fixed full-delay read (oldest slot, one lap back); output
  `clamp(Dry·source + Wet·tape)` — two gains, not a crossfade.
- **Shared output shader**: generalized `mix(live,buf,wet)` → `clamp(dry·live +
  wet·buf)`; `OutputProgram::draw` gains a `dry` arg. Write passthrough = dry 1,
  wet 0.
- Deleted the barrier-driven multi-writer additive path from the Write.
- Builds clean on the Windows MSVC toolchain.

**Next (not code): live Resolume validation** of the refined model — see the
smoke-test checklist in the devlog. This is the gate before any fast-follow.

## Fast-follow (deferred — bolt onto the working atoms)

Each is independent; do only if the atoms prove out live and the look calls for
it. Order is by likely value, not commitment.

- **F1 — Variable + multi-tap** (Tap): Tap Offset 0…loop, then K taps combined.
- **F2 — Sweepable/continuous Time** (Write): free mode alongside beat-sync,
  interpolated so time sweeps glide.
- **F3 — Over-unity float + precision budget**: RGBA16F so Send/Regen can persist
  >1.0 (controlled bloom), tone-map at output; Crisp↔Deep macro (precision +
  resolution-scale at ~constant VRAM). RGB10_A2 as free anti-banding.
- **F4 — Raw palette**: wrap (clamp/repeat/mirror), filter (nearest/linear),
  no-clear-on-realloc.
- **F5 — Additive multi-writer IFS**: N Writes summing into one slot via the
  deterministic barrier (contractive transforms → attractors). Absorbs the old
  overdub feature. The heaviest item; needs the multi-writer determinism the core
  already proved but this model doesn't yet exercise.
- **F6 — Channel count 4–8**: bump `NUM_CHANNELS` (+ REGISTRY literal); VRAM-heavy.

## Stage 7 — Docs, deploy, migration (when releasing)

- Rewrite `docs/delay-line-manual.md` for the two-atom model (principles, no
  artist names).
- `PLUGINS.md`, `plugins.json`, CI/release: build + co-locate `delay_core.dll`
  beside both plugin DLLs; verify load path on both platforms.
- Release notes: migration — existing `DLMd` compositions rebuild onto Tap+Write.
