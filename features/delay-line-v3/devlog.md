# Delay Line v3 — Dev Log

## 2026-07-04 — Design session (requirements + plan)

Started from a live-usability complaint: the shipped unified Read/Write plugin
is functional but confusing session-to-session and near-impossible to describe
to someone outside development. Two threads: (1) split the unified library into
two clearly-labeled plugins; (2) rethink wet/dry, which currently leans on
Resolume effect opacity (behavior we're not even sure applies to FFGL).

### Key finding: the split isn't free
Read/Write share the ring buffer via a process-global Rust `static` — works only
because they're one DLL. Two plugins = two DLLs = two copies of the static. The
GPU textures are already process-global (shared GL context); only the small CPU
registry needs cross-DLL sharing. Options: single-owner core lib (C ABI cdylib)
vs. OS shared memory. Deferred the mechanism to the plan; deciding criterion is
deterministic multi-Write accumulation.

### Wet/dry: unified mechanic, behavior-named controls
Landed on `out = mix(node_input, buffer_frame, wet)` on both nodes — so one Write
node is a complete delay ("play back oldest while writing newest"), and Tap is
the feedback-loop tap. But **named per behavior** (Write: Thru↔Playback + Regen;
Tap: Buffer Mix), because a single "Dry/Wet" label meaning different things per
node would recreate the exact confusion we're removing.

### Critical review reshaped the scope
Ran an adversarial design review through a feedback-performance / generative-
fractal lens (principles only — no artist names, per project constraint).
Verdict: proceed-with-changes. It argued the redesign as first specced optimizes
the describable shallow case and amputates the generative depth. Adopted:
- First-class additive accumulation (absorbs `features/overdub/`) via a Write
  blend mode + a real per-frame barrier (not wall-clock).
- Keep Tap expressive: variable offset + multi-tap (don't collapse to oldest).
- Curve Regen (fourth-root), allow past-unity runaway; keep mixing a shortcut so
  FX-in-loop stays the deep path.
- Sweepable continuous time (latching is hostile to time-sweep gestures).
- Expose the raw palette (wrap/filter/no-clear) and buffer precision.

### Precision via spatial tradeoff (decided in session)
Q: can we afford float buffers by trading spatial resolution for bit depth?
A: yes, favorably. 720p RGBA16F ≈ 1080p RGBA8 VRAM; lower-res float = fewer
fragments (compute-neutral-to-cheaper). Two non-obvious points:
- **RGBA16F is the only format that stores over-unity (>1.0)** — so float and the
  past-unity Regen runaway are the same decision. RGB10_A2 is a free anti-banding
  middle option (4× levels at today's VRAM) but clamps at 1.0.
- Trap: low-res buffer in a Tap→FX→Write loop = generational down/up-sample
  lowpass → deep smooth tones but soft fine detail. Coherent, and suits IFS
  (contractive maps shrink detail sub-pixel anyway).
Result: float folded into v1 as a "Crisp↔Deep" precision/resolution budget macro;
**channel-count increase (4–8) deferred to fast-follow** (genuinely VRAM-heavy,
independent of precision).

### Scope decision
"First-class everything now" — additive accumulation, multi-tap, sweepable time,
raw palette, and float-via-tradeoff all in v1. Channel count is the one deferred
item. Plan sequences the cross-DLL sharing spike (Stage 0) first as the gate.

### Next
Stage 0 spike: prove two-DLL buffer sharing + clean teardown before building any
params.

## 2026-07-04 — Stage 0 spike: PASS (core-lib approach)

Built `spike/` — `delay-core` (C-ABI cdylib holding the registry) + two plugin
cdylibs (`plugin-a` writes, `plugin-b` reads) that dynamically link it via an
`$ORIGIN` rpath, + a host that `dlopen`s both. All 10 checks pass:
- Both plugins share one registry: same refcount, same buffer handle, **B reads
  the value A wrote across the DLL boundary**.
- Shared per-frame barrier: first-vs-subsequent-writer detection and
  single-advance-per-frame work across the two DLLs (this is what additive
  accumulation needs).
- Clean refcount teardown frees the channel.

Verified the *mechanism*, not just the result: `DT_NEEDED` on libdelay_core.so,
no absolute build path baked in, and a negative test (remove the co-located core
lib → `dlopen` fails; restore → works) proves the plugin resolves it via
`$ORIGIN` — the Linux analog of macOS `@loader_path`.

**Decision: go with the single-owner core lib** (reviewer's pick). It reuses the
existing `Mutex<[ChannelBuffer]>` registry as-is and gives deterministic barrier
semantics — confirmed. Shared-memory fallback not needed.

**CPU-registry-sharing risk: RETIRED.** Two Stage-0 exit items remain, both
Resolume-side (user's Windows machine), neither an architectural blocker:
1. **GL texture handle sharing across the two DLLs** in Resolume's shared GL
   context. Very low risk — the current single-DLL delay line already shares GL
   handles between separate Read/Write *instances* via the same registry, and
   splitting the DLL doesn't change the GL context. Needs a smoke test to
   confirm, not a redesign.
2. **Windows load path for the core lib.** `$ORIGIN`/`@loader_path` cover
   Linux/macOS, but Windows' loader does NOT search a DLL's own directory for its
   static imports. So a static import of `delay_core.dll` from the plugin folder
   won't resolve. Fix: have each plugin **load `delay_core` at runtime from its
   own directory** (GetModuleFileName → LoadLibraryEx), i.e. an explicit
   `dlopen`-style load rather than a static import. Bounded, known pattern — carry
   into Stage 2 as the packaging detail. (This is the one thing that would make
   OS shared memory tempting instead; noted, not chosen.)

## 2026-07-04 — Stage 0 on Windows (the PRIMARY platform): PASS

Correction: the previous entry framed the Windows load path as a "carry-forward
detail." Wrong priority — Windows is the deployment target (Resolume runs there;
no macOS Resolume license), so cross-DLL sharing *on the Windows loader* is the
gate, not a footnote. Linux `$ORIGIN` does not transfer: the Windows loader won't
search a plugin's own dir for a static import.

Built `spike-win/` with the Windows-correct approach — plugins do NOT statically
import delay_core; a shared `pluglib` (rlib) finds the plugin's own module dir
(GetModuleHandleExW FROM_ADDRESS → GetModuleFileNameW) and LoadLibraryExW's
`<dir>\delay_core.dll` by absolute path, then GetProcAddress-binds the C ABI.
Built with the project's Windows MSVC cargo (cmd.exe over \\wsl$, target on C:),
ran the .exe via WSL interop.

Result on the real Windows loader:
- All 10 positive checks pass — two independently LoadLibrary'd plugin DLLs share
  ONE delay_core.dll registry (B reads A's write, shared barrier, single advance,
  clean teardown).
- Negative test: with delay_core.dll NOT co-located, LoadLibraryExW fails with
  err 126 (ERROR_MOD_NOT_FOUND) and aborts — proving each plugin loads delay_core
  from its OWN directory by absolute path, not an ambient search path.

**Stage 0 fully retired on the primary platform.** Deployment story: ship
delay_core.dll beside the two plugin DLLs in Resolume's plugin folder; pluglib
locates it. Architecture (core-lib + runtime bind) confirmed on Windows.

Remaining unknown, now the only Stage-0 item left, and it needs Resolume: the GL
texture handle being shared across the two DLLs in Resolume's shared GL context.
Low risk (the shipped plugin already shares GL handles between Read/Write
instances via the same registry), and the first real two-plugin build IS that
test.

### Next
Build the real Stages 1–2 (delay-core + pluglib crates, delay-write / delay-tap
skeletons on the core lib) to a deployable state, then a real Resolume test run
of the architecture before adding depth (Stages 3–7) or releasing.

## 2026-07-05 — Stages 1–2 built: four crates compile + deploy-ready

Built the real skeleton. Four new workspace crates under `plugins/`:

- **delay-core** (cdylib → `delay_core.dll`): the shipped `registry.rs` ported to
  a **scalar C ABI** (`dc_acquire/release/begin_frame_write/tex/write_pos/
  buf_size/buffer_depth`). Single shared owner across the two plugin DLLs. Runs
  its own `gl::load_with` (GL fn pointers are per-DLL) and owns only the ring
  **texture array** — FBOs are now plugin-local (see below). Verified the DLL
  exports all 7 `dc_*` symbols (`objdump -p`).
- **pluglib** (rlib): the proven Windows runtime loader (LoadLibraryExW of the
  sibling `delay_core.dll` by absolute path, from the plugin's own module dir) +
  a `#[cfg(unix)]` dladdr/dlopen fallback (macOS **untested**), + shared GL
  helpers (`QuadGeometry`, `ShaderProgram`, `OutputProgram`) and shared shaders.
- **delay-write** (cdylib → `DlyW`, `Delay Write   `): 8 behavior-named params
  (Channel, Sync Mode, Subdivision, Delay Ms, Delay Frames, Thru↔Playback, Regen,
  Blend). One node = complete delay: writes input to the channel, outputs
  `mix(live, oldest, thru_playback)`.
- **delay-tap** (cdylib → `DlyT`, `Delay Tap     `): 4 params (Channel, Tap
  Offset, Multi-tap [reserved, Stage 5], Buffer Mix). Reads channel at offset,
  `mix(own_input, buffer[offset], buffer_mix)`.

### Design decisions made while building
- **FBOs stay plugin-local; only the texture crosses the boundary.** Each Write
  makes its own FBO and attaches the shared texture layer. This sidesteps the
  FBO-context-sharing question entirely — only texture *names* (genuinely shared
  in Resolume's one GL context) cross DLLs. This IS the remaining Stage-0 unknown
  (GL-handle sharing across the two DLLs), now the first thing the Resolume test
  proves.
- **Frame barrier key = `FFGLData.host_time` as whole ms** (resolves the Stage-1
  open question). host_time is host-set and shared by all instances in a frame →
  deterministic first-writer detection. **Unverified: does Resolume actually call
  SetTime?** If not, it falls back to per-instance `now()` — fine for a single
  Write (still advances once/frame), but multi-writer accumulation (Stage 3)
  needs the real shared value. **First Resolume test must confirm host_time is
  populated** (watch the write_pos advance with two Writes on one channel).
- **`dc_release` does NO GL** (leaks the texture name, like shipped v2) — it can
  run off the GL thread (Drop / param change). The only GL delete is in
  `begin_frame_write`'s realloc path, which always runs inside `draw()`.
- **Blend = Additive currently aliases Crossfade** — true barrier-driven additive
  accumulation is Stage 3.
- Registered all three DLLs in `plugins.json` + workspace members.

### Build / deploy
- Compiles clean on the Windows MSVC toolchain (`cargo build -p delay-core
  -p delay-write -p delay-tap`, ~1.3s incremental). DLLs: delay_core 258 KB,
  delay_write 949 KB, delay_tap 940 KB.
- **Deploy all three together** — the plugins load `delay_core.dll` from their own
  directory, so it must sit beside them in Resolume's Extra Effects folder:
  `make build && make deploy PLUGIN=delay_core && make deploy PLUGIN=delay_write
  && make deploy PLUGIN=delay_tap` (or `make deploy` for all).
- Not yet done (Stage 7): CI/release packaging of `delay_core.dll` co-location;
  macOS unix-loader path is untested; migration notes for `DLMd` compositions.

### Resolume smoke test — what to check (before Stages 3–7)
1. **GL-handle sharing across DLLs**: one `Delay Write` on a layer records; a
   `Delay Tap` on another layer, same Channel, reads it back. Tap shows Write's
   buffer → cross-DLL texture sharing confirmed.
2. **host_time barrier**: two Writes on one Channel advance write_pos exactly once
   per frame (not twice). If they double-advance, Resolume isn't setting host_time
   — fall back plan noted above.
3. **One-node echo**: a single `Delay Write`, Thru↔Playback ~0.5, sensible Time =
   a usable echo out of the box.
4. **Feedback loop**: `Delay Tap → [FX] → Delay Write` on the same Channel.
5. **Teardown**: add/remove effects repeatedly — no crash (textures leak until
   Resolume exits, by design). Confirm Resolume ignores `delay_core.dll` (no FFGL
   entry point) gracefully during its plugin scan.

## 2026-07-05 — Refinement: collapse to two atoms + five knobs

Clean-session requirements refinement (the Stage-3 build worked mechanically but
was undescribable). Worked the model down conversationally to the **minimum
*playable* controls** — the atoms of a modular feedback engine — and refactored
the code to match. `requirements.md` + `plan.md` rewritten.

### The insight that unlocked it
**Every single blend knob is a crossfade, so it always couples "how much source
enters" with "how much loop sustains."** Thru↔Playback, Buffer Mix, Regen-as-mix
— the same crossfade wearing different hats, which is why they all felt
equivalent-or-still-broken. The only fix is *two independent gains*. So: expose
two gains, name each by behavior, one per node; defer everything else.

### The tape-loop mental model (user's framing, adopted verbatim)
- **Write = record head.** Pushes the current frame onto the tape, advances one
  slot, controls tape length (Time). Output is a **passthrough** of its input —
  you see the write-head input at all times; it's a visually transparent recorder.
  A Write with no Tap records into the aether.
- **Tap = read head**, placed just before the write head. Reads the frame one full
  lap back (the slot about to be overwritten) and carries it forward with the live
  source.

### The opacity crux — resolved, not worked around
Long thread on where the dry source reaches the FX stack if the Tap is first at
100% opacity. Resolution: **keep every effect at 100% opacity and do all mixing
in-shader.** At 100% the host crossfade (uncertain for FFGL anyway) is a no-op:
Write passes its input through, and the **Tap samples its own input** so the
source is already in its output (Dry is its gain). We never depend on host
opacity — which was exactly the old Read plugin's fragility (it ignored its input
and leaned on opacity to reintroduce the source). This also kills the seeding
chicken-and-egg.

### Final control surface
- **Write** `{Channel, Time (Sync Mode + value), Regen, Send}` —
  `tape[slot] = Regen·old + Send·input`, output = passthrough.
  Regen = decay/ring-out tail; Send = dub throw (pulse it).
- **Tap** `{Channel, Dry, Wet}` — `out = clamp(Dry·source + Wet·tape[full-delay])`,
  two gains not a crossfade. Dry = source into FX; Wet = feedback re-processed by FX.
- **Dub gesture**: hold Wet high, set Regen for the tail, pulse Send. Send→0 rings
  out at the Regen rate instead of cutting to black — why both Wet (morphing
  feedback) and Regen (raw persistence) both earn a place.

### Refactor (built clean, Windows MSVC)
- `pluglib` output shader: `mix(live,buf,wet)` → `clamp(dry·live + wet·buf)`;
  `OutputProgram::draw` gains a `dry` arg. Write passthrough = dry 1, wet 0.
- `delay-write`: dropped Thru↔Playback + Blend selector, added **Send**; single
  record path (fade pre-pass scales slot by Regen, additive `CONSTANT_COLOR=Send,
  dst=ONE` write adds Send·input); output now passthrough. **Deleted** the
  barrier-driven multi-writer additive path. 8→7 params.
- `delay-tap`: dropped Tap Offset + Multi-tap + Buffer Mix, added **Dry** + **Wet**;
  fixed full-delay read (oldest slot). 4→3 params.
- `delay-core`: **unchanged** (C ABI untouched); cargo correctly skipped it.
- Timing note: single writer per channel makes the additive-with-decay a trivial
  read-modify-write of `tape[wp]` (fade in-place then blend), no multi-writer
  barrier. The Tap, being upstream of the Write in the stack, reads slot `(wp+1)` =
  the pre-overwrite content one lap back; the Write then overwrites that slot.

### Deferred to fast-follow (not atoms)
Variable/multi-tap, sweepable time, over-unity float + Crisp↔Deep precision,
raw palette, additive multi-writer IFS (absorbs overdub — the heaviest, needs the
determinism this model doesn't exercise), channel count 4–8.

### Next
Live Resolume validation of the refined model (smoke-test checklist above, now:
Write output should be transparent passthrough; the Tap is what shows the loop).
This is the gate before any fast-follow. Deploy all three DLLs together.
