# Delay Lens — Dev Log

## 2026-08-29 — Design session (requirements + plan)

Came out of live-use feedback on the delay: the half-res tape was mushing tight
fractal feedback into blobs. That got diagnosed and fixed on the delay-line-v3 side
(full-res tape, `BUFFER_DEPTH` 240 → 120 — see that devlog), but the fix removed a
character worth keeping: the per-lap blur was *good* for goopy fluid feedback.

### The reframe

Don't put the blur back in the tape. Make softening one of several swept degrades on
the read path — and note that blur is the least interesting of them. User's own
prompt: chromatic aberration and "natural effects camera feedback would introduce".

The load-bearing distinction: **blur destroys information, the others displace or add
it.** Round-trip loop gain < 1 at high frequencies vs. ≈ 1. That's why a camera
feedback loop builds structure where a blur-in-the-loop dissolves it, and it's the
reason these knobs belong specifically in a feedback context.

### Insert FX vs. in-suite — settled

The user raised both and was open to either. Decided in-suite, on a signal-path
argument rather than a convenience one: anything between Tap and Write is *already*
inside the loop (that's the point of the split), but every insert is another pass
through Resolume's 8-bit inter-effect FBO, and that quantization compounds every
lap. Four inserts = four quantizations per lap; four fused knobs = zero extra passes.
Convenience happened to agree.

Corollary that made the suite easier to explain: **Write is the tape, Tap is the
lens.** Medium vs. optics.

### Locked

Five knobs on the Tap, appended after the existing four: Focus/Bloom, Aberration,
Vignette, Grain, Shear. Full spec in `requirements.md`; build order in `plan.md`
(Vignette + Grain first — they're free and they test the model; Focus/Bloom last
because it's the only one that needs filter design).

Explicitly out: lens distortion (overlaps `mirror-transform`), bloom/halation (wants
its own multi-pass plugin), exposure/AGC (needs a luminance readback).

### Open

Q1 bipolar-vs-two-knobs for Focus/Bloom, Q2 aberration angle/mode, Q3 grain
advection. All deferred to feel-in-Resolume rather than decided on paper.

### Next

Nothing until the full-res tape has been played live. The knobs are meant to be
tuned against a clean loop.
