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
