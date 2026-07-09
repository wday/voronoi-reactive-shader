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

## 2026-07-06 — Formal requirements + automated tests (outside Resolume)

Two threads landed while the requirements were being whiteboarded.

### Formal requirements — `requirements.sdoc` (StrictDoc)
Ported the prose reqs into a formal StrictDoc spec (40 requirements, custom
grammar with a per-requirement VERIFICATION field grounded in the actual code).
Hardened over 3 rounds of fresh clean-context adversarial review → 0 high-severity
findings. Two open questions were surfaced for the user to rule on (both documented
in the spec's RATIONALEs, not silently resolved):
1. **Realized delay off-by-one** — the Tap reads content `loop_length + 1` frames
   old, not `loop_length`. The read slot `(wp+1)` is identically the slot this
   frame's Write overwrites; draw order (Tap-before-Write) is what keeps the read
   clean, NOT the spare slot (the spare only adds the extra frame). Fix in code
   (`buf_size = loop_length`) or relabel the UI.
2. **GL-state restore gap** — the Write mutates BlendFunc/BlendColor + GL_TEXTURE0
   binding but only restores FBO/viewport/enables. Latent risk; widen or accept.
Also catalogued stale in-code comments (`lib.rs:6`, `fade.frag.glsl:3-4` show the
old ungated record eq; the running gated form is at `lib.rs:165`).

### Automated tests without Resolume — two tiers
New pure crate **`plugins/delay-dsp`** (no ffgl-core, no gl) is now the single
source of truth for the host/GPU-independent logic — the ring index arithmetic,
the Regen curve, and the delay-time conversion — so it `cargo test`s natively.

- **Tier 1 (pure unit tests, `cargo test -p delay-dsp`, 14 tests):** ring
  advance/read-slot incl. an explicit `realized_delay = loop_length + 1` tripwire
  (independent frame-loop simulation), the `read_slot == this-frame's overwrite
  slot` identity, `regen^0.25` curve, and every branch of the time conversion
  (Ms/Subdivision/Frames, BPM≤0 fallback, rounding, clamp).
- **Tier 2 (headless-GL integration, `plugins/delay-gl-tests`, 1 test / 5
  scenarios):** a surfaceless EGL/llvmpipe context runs the REAL `delay-core` ring
  + the REAL plugin shaders (`fade`/`write`/`output`, via `include_str!`) and
  asserts pixels end-to-end — the off-by-one proven in actual pixels, Send=1 color
  fidelity, Tap Wet gain, Dry passthrough + empty-buffer wet-forcing, and Send=0
  Regen decay + the multi-writer-same-frame barrier.

**Wiring:** `delay-core` now uses `delay_dsp::Ring` (added `rlib` crate-type so the
harness can link it in-process); `delay-write` delegates `regen()` and
`compute_delay_frames()` to delay-dsp, so the tests guard the SHIPPED plugin code,
not a copy. `delay-gl-tests` is excluded from the workspace (own EGL/gl stack).
All three plugin DLLs still build clean on the Windows toolchain.

**Run it:** `./scripts/test-delay.sh` (Linux/llvmpipe, independent of `make build`).

### Still next
Live Resolume validation of the refined model remains the gate before fast-follow
— the headless tests cover the delay/feedback math but not the FFGL host glue
(host FBO/opacity, cross-DLL LoadLibraryEx sharing, real BPM/host_time feed).

### v0.2 spec — order-independent absolute addressing (2026-07-06)
Reviewing the requirements diagram surfaced that a `Tap→Write→Tap` sequence in one
frame makes the two Taps read **one-frame-different** content: the read position was
derived from the live `wp`, which the Write advances mid-frame (Tap-before sees
pre-advance `wp` → delay `loop_length+1`; Tap-after sees post-advance → `loop_length`).
Root cause = deriving the read from mutable, mid-frame state.

**Decision (spec v0.2, `requirements.sdoc` bumped 0.1→0.2):** address the tape
**absolutely** off a per-channel monotonic frame counter `frame_index`, advanced
once per frame at a barrier that **both** plugins hit:
- write slot = `frame_index mod buf_size`
- read slot  = `(frame_index − loop_length) mod buf_size`

Both are pure functions of one per-frame value ⇒ every Tap reads the same slot
regardless of draw order or how many Tap/Write instances interleave. Since
`1 ≤ loop_length ≤ buf_size−1`, read slot ≠ write slot always, so the read never
aliases the current write — **draw order stops mattering for timing** (it still
governs FX routing / whether the tail is re-processed, COMPOSE-ORDER).

Bonus: this **resolves open-question #1** (the CORE-DEPTH off-by-one). Realized
delay is now `loop_length` exactly, and the `+1` buffer layer earns its keep as the
**guard slot** that keeps read off write. Also removes the v0.1 seed-lap `N+1`
boundary (both slots derive from the same counter).

**Requirements touched (all → STATUS: Draft, code delta from `dcb44c6` noted):**
Terminology, CORE-DEPTH, CORE-ADVANCE, CORE-BARRIER, CORE-ABI, TAP-READ-SLOT,
TAP-READ-ONLY, COMPOSE-ORDER, COMPOSE-SINGLE-WRITER (+ stale-ref fixups in
CORE-REFCOUNT / WRITE-RECORD-MATH / WRITE-PARAM-TIME). Doc re-validated:
`strictdoc export` clean.

**Code delta still TODO (not yet implemented):**
- `delay-core`: add monotonic `frame_index`; rename `dc_begin_frame_write` →
  `dc_frame_tick` (idempotent per `frame_id`, called by BOTH plugins); replace
  `dc_write_pos` export with `dc_frame_index`.
- `delay-tap`: call `dc_frame_tick` at draw top; `read_pos =
  (frame_index + buf_size − loop_length) % buf_size`.
- `delay-write`: `write_slot = frame_index % buf_size`.
- **Tests to flip:** the `delay-dsp` Tier-1 `realized_delay == loop_length + 1`
  tripwire and the Tier-2 GL off-by-one scenario both assert the OLD +1 — they must
  become `== loop_length`, plus add a `Tap→Write→Tap` order-independence case.
- One accepted degenerate (documented, not guarded): a Tap-only frame (no Write)
  still advances the counter → rotates the tape rather than freezing.

## 2026-07-06 — v0.2 rationale hardening (spec review of the absolute-addressing rework)

Reviewed the v0.2 `frame_index` rework and confirmed the model is sound (delay =
loop_length exactly; read slot `(t−L) mod buf_size` always ≠ write slot `t mod
buf_size` since `1 ≤ L ≤ buf_size−1`; order-independent). Applied four spec fixes
the review surfaced:
1. **WRITE-RECORD-MATH gate rationale** — the `(1−Send)` justification was stale.
   v0.2 decoupled the Tap read from the write slot, so `old` (write slot's prior
   content, age `L+1`) and the Tap read (age `L`) are now ADJACENT frames, not the
   same. The gate is still correct/needed (ungated → coefficient `Regen+Wet > 1`
   at Send=1 → runaway) but the reason is "two adjacent-frame taps," not
   "same-frame double-count." Rewrote accordingly.
2. **LOOP-FEEDBACK-COEFF** — now `≈`, not `=`: sum of two one-frame-apart tap
   gains; exact only for a constant image, accurate guide otherwise. Stability
   conclusions unchanged.
3. **CORE-DEPTH guard-slot wording** — the `+1` is a size margin (keeps read
   trailing write by a nonzero offset), NOT a reserved/unused layer (monotonic
   addressing rotates through all `buf_size` layers).
4. **CORE-DEPTH warm-up** — first `L` frames read a cleared-BLACK slot (handled by
   the alloc-clear, CORE-FORMAT), not by TAP-EMPTY-BUFFER's Wet-mute (which fires
   only when unallocated). Reworded.

Also settled the earlier simplification thread against v0.2: proposal #2 (drop the
barrier) is now impossible — both plugins tick, so frame-identity is required to
advance once/frame; proposal #3 (drop the +1) is now wrong — the +1 is the guard
slot. Proposal #1 (single-pass record) still applies (orthogonal; still closes the
GL-state gap). Spec re-exports clean. **v0.2 CODE implementation still pending**
(ABI rename, frame_index, both plugins tick, flip the delay-dsp/Tier-2 tripwires
loop_length+1 → loop_length, add an order-independence test, Windows verify).

## 2026-07-08 — v0.2 CODE implemented (absolute addressing landed)

Implemented the v0.2 spec across all six crates. Tier-1 + Tier-2 green, all three
DLLs build clean on Windows MSVC and are deployed. **Awaiting live Resolume
validation** (the one thing headless can't cover).

### ABI (delay-core)
- `Channel` gained a monotonic `frame_index: u64`; `Buffer` lost `write_pos`
  (slots are now derived, not stored).
- `dc_begin_frame_write` → **`dc_frame_tick`**, restructured into two decoupled
  responsibilities so draw order can't break either:
  1. buffer sizing — runs only when width/height are non-zero (the Write path),
     NOT gated on the barrier, so a Tap that ticks first can't stop the Write from
     (re)allocating;
  2. the barrier — the first caller of a given `frame_id` advances `frame_index`
     once and returns 1; later callers return 0.
- `dc_write_pos` → **`dc_frame_index`** (returns the counter; plugins map it onto
  slots). `dc_release` now zeroes `frame_index` on refcount→0.

### Slot math (delay-dsp `Ring`, single source of truth)
`Ring` is now just `{ loop_length }` (no stored position). `write_slot(fi) = fi %
buf_size`; `read_slot(fi) = (write_slot(fi)+1) % buf_size` (the oldest slot =
spec's `(fi − loop_length) mod buf_size`, always ≠ write slot ⇒ order-independent).
`realized_delay_frames()` now returns `loop_length` (was `loop_length + 1`).
Added `for_buffer(buf_size)` so the Tap can recover geometry without a loop knob.

### Plugins
- **delay-write**: ticks with real loop_length/dims, then `wp =
  Ring::for_buffer(buf_size).write_slot(frame_index)`.
- **delay-tap**: now computes a `frame_id` from `host_time` and **ticks the
  barrier** (zero dims: read-only) at draw top, so `frame_index` advances even on
  a Tap-only frame (accepted degenerate: rotates, doesn't freeze). Read slot via
  `Ring::for_buffer`. Added `delay-dsp` dep.

### GL-state restore gap (open question #2 — resolved as WIDEN)
Widened both plugins' host-state restore. **Write** now saves/restores blend
func (separate RGB/alpha), blend color, the active texture unit, and the unit-0
2D binding — so it no longer leaves the host with `CONSTANT_COLOR,ONE` + an
unbound unit 0. **Tap** saves/restores the active unit + unit-0 binding (it
touches no blend). Chosen over "accept narrowed scope" per the project note that
GL_TEXTURE0 must be restored.

### Tests flipped/added
- delay-dsp: `realized_delay_is_loop_length_plus_one_offbyone` →
  **`realized_delay_equals_loop_length`**; added `read_slot_never_aliases_write_slot`
  (guard-slot proof), `read_slot_is_frame_index_minus_loop_length_mod_bufsize`
  (spec-form equivalence), and **`read_is_order_independent_within_a_frame`**
  (Tap→Write→Tap reads the same slot). 17 Tier-1 tests pass.
- delay-gl-tests: harness now ticks via `dc_frame_tick`/`dc_frame_index`; both
  `record` and `tap_read` tick the same `frame_id` per frame. Scenario 1 asserts
  realized delay `== loop_length` (was `+1`); scenario 2 first reproduces at
  frame 4 (was 5); scenario 5 asserts `frame_index` (not the raw slot) is
  unchanged by a 2nd same-frame writer. Headless pixel test passes.

### Build/deploy
`make build` for delay_core/delay_write/delay_tap all clean (MSVC). Verified
`delay_core.dll` exports exactly `dc_acquire, dc_buf_size, dc_buffer_depth,
dc_frame_index, dc_frame_tick, dc_release, dc_tex` (old `dc_begin_frame_write` /
`dc_write_pos` gone). Deployed all three to the Resolume Extra Effects folder.

### Still next
- **Live Resolume validation** — the only remaining gate. Confirm: Tap shows the
  loop, realized delay matches the knob (no off-by-one), a `Tap→FX→Write` rotation
  iterates, and no GL-state corruption of downstream effects.
- Flip the 12 v0.2 `requirements.sdoc` reqs from STATUS: Draft → their verified
  state once Resolume-confirmed (doc still reflects "code TODO").

## 2026-07-08 — Write simplified to a pure write head (Regen removed)

Design review (with the user) found the Write's `Regen*old` term was a **lineage
artifact** from the shipped unified single-plugin delay, where one node did
in-place feedback. In the two-plugin split, feedback already lives in the Tap
(`Wet*tape[s_r]`), so the Write reading the buffer again expressed the same loop
twice. Collapsed to a **pure write head**:

```
Write:  out       = input                         (passthrough, unchanged)
        tape[s_w] = Send * input                  (overwrite; NO buffer read)
Tap:    out       = clamp(Dry*source + Wet*tape[s_r])   (unchanged)
```

One loop path now, one gain: **`Send*Wet`** around `source → Tap → FX → Write`.
- **Send** = record/dub level (pulse to throw; Send=0 stops recording, tape
  clears over one lap).
- **Wet** = feedback + monitor (what you see AND what re-enters the FX chain).
- decay = `Send*Wet` (sit below 1 to ring out, ≈1 to hold, >1 to bloom/saturate).

**Regen deleted.** Everything it did is recoverable: clean decaying echo = FX
bypassed in the loop with `Send*Wet<1`; generative iteration = FX in the loop.
The one behavior change: Send=0 now clears over a lap instead of ringing out at an
independent rate (correct write-head behavior). Kept Send (over "fully minimal
Time-only Write") so the picture can look hot while the tape starves, and for the
pulse-to-dub gesture.

### What this removed
- `fade.frag.glsl` (the decay pre-pass) — **deleted**.
- The `Regen` param — Write is now **6 params** (was 7): Channel, Sync Mode,
  Subdivision, Delay Ms, Delay Frames, Send. Playable knobs 5 → **4** (Time, Send,
  Dry, Wet).
- The Write's blend-state mutation — `tape = Send*input` is a single straight
  overwrite (Send as a `u_send` uniform, blend OFF). So the Write no longer touches
  host blend func/color, and the widened blend save/restore added earlier was
  **reverted** — only the texture-unit restore remains. Net: the GL-state gap
  (open Q#2) shrank on its own.
- `delay_dsp::regen_curve` is now unused by shipped code (left in delay-dsp with
  its tests; it's a generic knob-shaping util that may return for a future knob).

### Verification
Tier-1 (delay-dsp) + Tier-2 (headless GL) both pass. Scenario 5 rewritten: a 2nd
same-frame Write with Send=0.5 now *overwrites* the slot to 0.5*white (was the
`(1-Send)*Regen*old` decay), and still asserts frame_index is unchanged. Scenarios
1–2 timing identical (at Send=1 the new and old record equations coincide).
`delay_write.dll` rebuilt clean on Windows MSVC and deployed (core/tap unchanged).

### Still next
- Live Resolume validation (unchanged gate).
- **`requirements.sdoc` needs a pass**: WRITE-RECORD-MATH (`tape = Send*input`),
  drop the Regen param req, LOOP-FEEDBACK-COEFF (now `Send*Wet`), terminology, and
  the GL-state-gap req (narrowed since Write no longer touches blend).

## 2026-07-08 — Blend Space (perceptual vs linear light) on the Tap

Live testing of the simplified loop showed the echo tail tapering "toward recent
frames" — expected geometric feedback (gain `Send*Wet`, a frame k laps back
contributes `(Send*Wet)^k`), independent of colour space. But it re-surfaced the
linear-vs-perceptual blend question: the whole loop does its math on the raw
**sRGB-encoded** bytes (tape is `GL_RGBA8`, no decode anywhere), so every `×Wet`
and `+` happens in gamma space. With Regen gone there's a single, isolated blend
point, so this became a clean knob to expose.

### What shipped
- **New Tap param 3 "Blend Space"** {Perceptual (default), Linear}. Tap is now
  **4 params** (was 3). Perceptual = exact legacy; Linear decodes to ~linear light,
  blends + clamps as real light, re-encodes.
- All of it lives in the shared `output.frag.glsl` behind a `u_gamma` uniform:
  `u_gamma == 1.0` takes the exact legacy path; otherwise `pow(·, g)` /
  `pow(·, 1/g)` around the sum, `g = 2.2` (cheap ~sRGB stand-in, matches the
  "simple math" preference). Only `.rgb` is mapped; alpha stays linear.
- `OutputProgram::draw` gained a `gamma` arg. The Write passes `1.0` (its
  passthrough is `pow(pow(x,g),1/g)`-safe but stays exact at g=1); the Tap passes
  `params.gamma()`.

### Why only the Tap changed (convention)
The tape keeps storing sRGB (`Send*input`, Write untouched). The Tap decodes both
operands on read and re-encodes on output; the loop's per-lap re-encode makes the
linear value round-trip through the sRGB tape, which actually *keeps perceptual
storage precision*. Side effect: in Linear mode the loop-feedback coefficient is
`Wet*Send^g` (Send reshaped, still <1) — noted in LOOP-FEEDBACK-COEFF /
TAP-BLEND-SPACE. No Core/ABI/Write change, no cross-plugin param to keep in sync.

### Verification
- Tier-2 headless GL **scenario 6** added: tape=200, Dry=0, Wet=0.5 →
  ≈146 in Linear (`255·(0.5·(200/255)^2.2)^(1/2.2)`) vs 100 Perceptual, through the
  real shader. Existing scenarios pass unchanged (perceptual path bit-exact).
- All three DLLs rebuilt clean on Windows MSVC.
- **Deploy blocked**: Resolume had the DLLs locked (`cp: Permission denied` — the
  Makefile still prints "Deployed", the known lie). Close Resolume + re-`make
  deploy` before live testing.
- sdoc updated: TAP-BLEND-SPACE added, SYS-PARAM-COUNTS (Tap 3→4), TAP-READ-MATH +
  LOOP-FEEDBACK-COEFF notes; exports clean.

### Still next
- Deploy (once Resolume is closed) + live A/B: expect Linear to bloom/mix cleaner,
  Perceptual to fade more evenly. Pick the better default from what reads best.
- Watch for 8-bit sRGB round-trip banding in deep feedback (Linear) — that's the
  RGBA16F/DEFER-FLOAT axis, not fixed here.

## 2026-07-08 — Linear default + loop-length reseed (and DEFER-FLOAT scoping)

Two follow-ups from the blend-space work, plus a design finding on DEFER-FLOAT.

### Linear is now the default Blend Space
Tap param 3 default flipped Perceptual → **Linear** (`params.rs`: default 1.0).
Existing saved comps keep their stored value; only new instances get Linear.

### Loop-length change reseeds the tape (CORE-LENGTH-RESEED)
Edge case: record a 60-frame loop, shrink to 10, later expand back — the read
sweeps ~50 layers the short loop never rewrote and resurfaces stale pre-shrink
frames for up to a lap. Root cause: `loop_length` IS the ring modulus, so changing
it re-maps every slot's frame_index — the *whole* tape goes incoherent, not just a
tail. Fix: on any `loop_length` change (no resolution change), clear the whole tape
to black in `dc_frame_tick`'s retune branch; the read warms up from black over one
lap like a fresh alloc. Fires only on a real (latched) change; continuously
dragging the time value re-clears each step (stays black while dragging — smooth
time is DEFER-SWEEP).
- New `clear_texture_array(tex)` helper: FBO + `glClear` per layer, GPU-side —
  replaced `alloc_buffer`'s old 240×`TexSubImage3D` CPU clear (~2 GB of uploads at
  1080p) too, so allocation/resolution-change got much cheaper as a bonus. Saves/
  restores host FBO binding, clear colour, scissor enable.
- Tier-2 scenario 7 added: fill a loop_length-8 ring white, change length, assert a
  previously-white layer reads black. All prior scenarios still pass (they now also
  cover the new alloc-time clear path). Built clean on MSVC.

### DEFER-FLOAT — scoped, NOT yet implemented (design finding)
Digging in before switching the tape to RGBA16F surfaced two things worth a
decision before spending VRAM:
1. **Over-unity is architecturally blocked here.** The loop passes through
   Resolume's inter-effect FBO (8-bit unless Resolume composits in float, which we
   don't control) and the Write *overwrites* (no accumulation), so a float tape
   can't retain >1.0 across laps. Real over-unity needs Resolume float mode (loop
   path) and/or additive tape accumulation (DEFER-IFS). So RGBA16F's *realizable*
   win right now is precision/**banding only**, not the runaway bloom the memo
   imagined.
2. **VRAM.** 240 layers @1080p: RGBA8 ≈ 1.94 GB/ch; RGBA16F ≈ 3.98 GB/ch (×2
   channels if both used) — real OOM risk. **RGB10_A2** is the same 4 bytes as
   RGBA8 (no VRAM increase) but 1024 levels/channel → big banding reduction, free.
So for the actual goal (kill linear-mode banding) RGB10_A2 looks strictly better
than RGBA16F until over-unity is unblocked. Put the format choice to the user
(RGB10_A2 vs RGBA16F-keep-depth vs RGBA16F-half-depth/Crisp↔Deep) before coding.

### Deploy status
Linear-default (tap) + reseed (core) built clean on MSVC but **NOT deployed** —
Resolume was reopened and re-locked the DLLs (`cp: Permission denied`; the deployed
copies are the prior build). Close Resolume + re-`make deploy PLUGIN=delay_core`
and `delay_tap`.

## 2026-07-08 — DEFER-FLOAT (precision half): half-res RGBA16F tape

Decision (with the user): spend the resolution budget on **float precision at
~same VRAM** — a half-res (0.5×) RGBA16F tape. Banding gone, ~0.97 GB/ch @1080p
(less than the old full-res RGBA8 ~1.94 GB), max delay unchanged (240 frames / 4s).

### Implemented
- **Core**: `alloc_buffer` now `GL_RGBA16F` + `GL_HALF_FLOAT` (was RGBA8/UNSIGNED_
  BYTE); VRAM log switched to 8 bytes/texel. `clear_texture_array` (FBO+glClear)
  clears float layers fine.
- **Write**: new `TAPE_SCALE = 0.5`. `draw_write` sizes the tape at
  `round(0.5*w) × round(0.5*h)`, passes those to `dc_frame_tick`, and sets the
  write-pass viewport to the tape dims. `uv_scale` still targets the full-res
  input → a full-frame downsample into the smaller layer. Passthrough output stays
  full-res.
- **Tap**: ZERO changes — it already reads the tape with normalised uv + LINEAR
  filter, so it upscales on read regardless of tape resolution.

### Why this shape
The read side being resolution-agnostic (normalised uv) is what made this a
Write-only change. Visual bonus: only the Wet path re-downsamples each lap, so the
feedback tail softens progressively while Dry injection stays crisp — feedback blur
as a (currently fixed) characteristic.

### Not done (still DEFER-FLOAT)
- Over-unity *persistence*: blocked by Resolume's 8-bit inter-effect FBO in the
  loop + overwrite-Write. Needs host float compositing and/or DEFER-IFS. RGBA16F
  gives headroom but the loop still clips each lap.
- Resolution→depth (temporal) trade: `BUFFER_DEPTH` still a fixed 240. Lowering res
  to buy a deeper/longer buffer (dynamic depth + bigger Time max) is the next step
  if longer delays are wanted.
- Live Full/Half/Quarter + Format knobs (user picked the fixed point, not knobs).

### Verification
- Tier-2 scenario 8 added: record a solid colour into a HALF-res tape, read back at
  FULL res → exact (validates resolution-independent read + RGBA16F round-trip).
  All prior scenarios pass unchanged.
- `delay_core` + `delay_write` built clean on MSVC.
- NOTE: the Write's TAPE_SCALE downsample is only exercised in the real plugin
  (the headless harness has its own write path); headless covers the core format +
  the read-side upscale, not delay-write's viewport scaling — that's Resolume-only.

### Deploy
Built clean; **deploy still blocked** (Resolume holding the DLLs). Pending core +
tap (reseed/linear-default) AND now core + write (float/scale) all need Resolume
closed. Close it, then `make deploy PLUGIN=delay_core delay_write delay_tap`.
