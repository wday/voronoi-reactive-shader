# Delay Line v3 — Implementation Plan

Sequenced to de-risk: prove cross-DLL buffer sharing and the frame barrier
first, then build the two plugin surfaces, then layer generative depth. Each
stage is independently testable in Resolume (or the harness where possible).

## Stage 0 — Prove cross-DLL buffer sharing (spike, throwaway UI)

The whole split hinges on two DLLs sharing one ring buffer. Build the minimum to
confirm it before investing in params.

- Extract the registry (`registry.rs`) into a shared component with a **stable C
  ABI**: `begin_frame_write`, `read_channel`, `acquire`, `release`,
  `buffer_depth`, plus the new barrier/accumulation entry points.
- **Mechanism decision (make here, with a spike):**
  - **Core lib** (reviewer's pick): `delay-core` as a `cdylib` with C ABI; both
    plugin crates call it via FFI. Single owner of the registry + barrier — one
    static, one lock, deterministic. Cost: ship `delay_core.{dll,dylib}` and make
    the plugin DLLs find it (Windows: `AddDllDirectory`/place next to host;
    macOS: `@loader_path` rpath).
  - **OS shared memory**: named segment holds the tiny registry table; GL handles
    are process-global so they cross the boundary. No extra artifact to locate,
    but needs a process-shared lock/atomics and stale-segment cleanup across
    Resolume restarts.
  - Deciding criterion: deterministic multi-Write-per-frame accumulation (Stage
    3). Lean core-lib unless deployment/load-path proves worse than shm cleanup.
- **Spike test:** trivial two-plugin build — DLL A writes a test pattern, DLL B
  reads it back on another layer. Confirm B sees A's buffer in Resolume. Confirm
  clean teardown (remove effects, no leaked textures / stale segment).
- Exit criterion: sharing works and tears down cleanly. Only then proceed.

## Stage 1 — Frame barrier (replaces 2ms wall-clock)

- Identify the host per-frame signal in `FFGLData` (host time / frame index —
  verify what ffgl-core exposes; extend if needed).
- Registry tracks per channel: current frame id, first-writer-seen flag, the
  frame's source read slot. First writer in a frame advances write_pos + owns the
  fade/clear; subsequent writers in the same frame accumulate against the *same*
  source slot.
- Test: two Write instances on one channel, confirm exactly one advance/frame and
  stable ordering (Resolume bottom-to-top layer order).

## Stage 2 — Two plugin skeletons with behavior-named controls

Split into `plugins/delay-write` and `plugins/delay-tap` (two crates, two
16-byte names, two 4-byte ids; provisional `DlyW` / `DlyT`). Both depend on the
shared registry from Stage 0.

- **Delay Write** params: Time (sync mode + value), Thru↔Playback, Regen, Blend
  mode, Channel. Output = `mix(live_thru, oldest_delayed, thru_playback)`; write
  applies Regen via the selected blend mode. Reuse the read/output pass for the
  playback fetch.
- **Delay Tap** params: Tap Offset, Multi-tap, Buffer Mix, Channel. Output =
  `mix(node_input, buffer_at_offset, buffer_mix)`. Tap must sample its own input
  (today's Read ignores it).
- Defaults: a single Delay Write with sensible Time/Thru↔Playback is a usable
  echo out of the box; Regen 0, Blend = Crossfade.
- Test: single Write = echo; Tap→FX→Write = feedback loop with FX insert.

## Stage 3 — Additive accumulation (absorbs overdub)

- Implement Blend mode = **Additive** on Write: first writer fades/clears
  (`decay`), subsequent writers add (`GL_ONE, GL_ONE`), all reading the frame's
  shared source slot (from Stage 1 barrier).
- Validation composition: the Sierpinski/IFS recipe from `features/overdub/` —
  N Writes (each behind a contractive transform) + one Tap output. Confirm an
  attractor forms and is stable across generations.
- This is the deciding test for the Stage 0 mechanism choice.

## Stage 4 — Sweepable time

- Add a **free/continuous** time mode alongside beat-sync: bypass latching,
  interpolate loop length smoothly on change so sweeps glide (no stepping).
- Keep beat-sync (subdivision/ms/frames) as selectable modes.
- Test: sweep Time live, confirm smooth pitch/smear/zoom-rush with no audible
  stepping or black frames.

## Stage 5 — Variable + multi-tap

- Delay Tap: Tap Offset 0…loop_length; Multi-tap = read K offsets and combine
  (sum/screen, or K instanced reads). Cheap — sampler reads at chosen layers.
- Test: multi-tap echo cloud from one Write loop; several offsets with distinct
  transforms on separate layers.

## Stage 6 — Raw palette + precision/resolution budget

- Expose per-channel: texture **wrap** (clamp/repeat/mirror), **filter**
  (nearest/linear), **no-clear-on-realloc**.
- **Buffer: Crisp↔Deep** macro: precision (RGBA8 / RGB10_A2 / RGBA16F) +
  resolution-scale, auto-balanced to hold VRAM ~constant. Advanced path exposes
  precision, res-scale, filter independently.
- Decouple buffer resolution from input resolution: allocate at scaled dims,
  render Write at buffer-res (viewport = buffer dims), read/output upsamples via
  the sampler.
- **Over-unity:** with RGBA16F, allow Regen > 1.0 to persist in the buffer
  (controlled runaway); tone-map/clamp only at output. 8-bit/RGB10_A2 clamp.
- Test: long-running feedback shows no 8-bit banding at Deep; over-unity bloom
  accumulates without clipping; wrap/filter visibly change the palette.

## Stage 7 — Docs, deploy, migration

- Rewrite `docs/delay-line-manual.md` for the two-plugin model (supersedes the
  v2 Read/Write manual). Reason from principles; no artist names.
- Update `PLUGINS.md`, `plugins.json`, build scripts for the new crates + the
  shared core lib (ensure CI/release build and package `delay-core` and set its
  load path on both platforms).
- Release notes: migration — existing `DLMd` compositions must be rebuilt.
- `make build` / `make deploy` verified for both plugins + core lib.

## Open implementation questions (resolve in devlog as encountered)

- Exact `FFGLData` per-frame signal (Stage 1) — confirm availability.
- Core-lib load-path ergonomics vs. shm cleanup (Stage 0) — pick with the spike.
- Multi-tap combine semantics (sum vs. screen vs. per-tap weight) — pick in
  Stage 5 from feel.
- Whether Regen and Thru↔Playback interact in a way that needs a third control
  for the FX-loop (Write terminal) case — watch during Stage 2/3.
