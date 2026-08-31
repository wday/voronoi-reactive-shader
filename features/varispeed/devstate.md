# Varispeed — Development State

What is built, what is verified and how, what must not break, what is unverified.
Spec: [requirements.md](requirements.md). Remaining work: [plan.md](plan.md).

---

## Built

**Crates.** `varispeed-dsp` (pure read-head math), `varispeed-core` (cdylib: the
tapes + write-cursor barrier, loaded across DLLs by `pluglib::vc_api`),
`varispeed-read` (`VsRd`), `varispeed-write` (`VsWr`).

**Topology.** `src -> VsRd -> FX... -> VsWr -> tape`, per channel. The Write is a
metronome (one frame per host frame, `record_index mod N`) and does not tick while
frozen; the Read floats and never advances the cursor.

**Two channels.** `NUM_CHANNELS = 2` in `varispeed-core`; every `vc_*` except
`vc_depth` / `vc_channels` takes a leading `channel`, out-of-range is a no-op
returning a zero value. Both plugins carry Channel as param 0 and rebalance the
`vc_acquire` / `vc_release` refcount in `set_param` (mirrors `DelayTap`).

**Depth 61 / 1 s max loop.** `BUFFER_DEPTH = 61 = MAX_LOOP_FRAMES + 1`. 965 MiB per
channel at 1080p full-res RGBA16F, 1.88 GiB for both — against ~3.98 GB for the old
single 240-layer tape.

**Reverse removed.** `LoopMode` is `Free | Confined`. Gone: `sample_block`,
`reverse_reset_pos`, the `active_block` / `rec_pos` / `play_pos` / `rev_len` state,
the per-mode `max_len` match. Reverse was the only thing forcing `N = 2 * L_max`.

**Age seeding (VS-ANCHOR).** `anchor_age(len) = len - 1` seeds Free's `age` on first
draw, on a latched Loop Length change, and on entry to Free. This is what makes Loop
Length a real knob in Free mode.

## Invariants — do not break

- **Round trip is `age + 1`, not `age`.** The Read draws before the Write: the Read
  sees `record_index = R` and reads layer `R - age`; the Write then advances to
  `R + 1` and records there. An `L`-frame loop is therefore `age = L - 1`, which is
  why `age` wraps mod `L` (giving round-trip `1..L`) and why `N = L_max + 1`.
- **`N = MAX_LOOP_FRAMES + 1`.** The stitch guard: at the deepest tap the read layer
  and the layer the Write is about to fill differ by `L_max < N`, so they never
  alias. Raising `MAX_LOOP_FRAMES` without raising `BUFFER_DEPTH` breaks this.
- **A Free Read writes no tape state.** It reads `record_index` and publishes
  nothing. This is what makes multi-tap work; anything that makes Free publish
  breaks VS-MULTITAP.
- **One `loop_slot` per channel** ⇒ at most one Confined Read per channel, and at
  most one Write per channel (`vc_write_tick` de-dups on `frame_id`, so a second
  Write in a host frame overwrites the first).
- **VS-ISOLATION.** `delay-core` / `DlyT` / `DlyW` are never touched. The shared
  `pluglib` loader is: `VcApi`'s signatures must stay in lockstep with
  `varispeed-core`'s `vc_*`, and `Api` (the `dc_*` side) must stay untouched.
- Save/restore the host FBO and `GL_TEXTURE0` — shared GL context with Resolume.

## Verified

- **`cargo test -p varispeed-dsp`** — 28 pass. Covers the Catmull-Rom tap layout and
  seam wrap (age + confined), `anchor_age` being the top of the window, the seeded
  age being a fixed point at `dr = rate = 1` across `len` 1/2/7/30/60 (VS-ROUNDTRIP),
  the seeded read landing on the oldest frame of the window, and the max-length read
  slot never aliasing the write slot (VS-CAPACITY).
- **`cargo test -p delay-dsp`** — 17 pass, unchanged. VS-ISOLATION at the dsp level.
- **`cargo build`** — `varispeed-core`, `delay-core`, `pluglib` clean on Linux.
- **Windows MSVC release build** — all three DLLs clean, no crate warnings.
  `varispeed_core.dll` exports all nine `vc_*` including the new `vc_channels`.
- **Deployed** — `varispeed_core.dll`, `varispeed_read.dll`, `varispeed_write.dll`
  copied into Resolume's Extra Effects directory. The core DLL must sit beside the
  two plugins: they runtime-load it from their own directory so both bind ONE copy.

## Unverified

Everything below needs Resolume. All three DLLs are **deployed** to
`Documents/Resolume Avenue/Extra Effects/`; nothing has been run yet.

- **Nothing in this change has run in Resolume.**
- Whether Resolume picks up the new param lists or has cached them by `unique_id`
  (fallback: bump `VsRd`/`VsWr` -> `VsR2`/`VsW2`).
- Live behaviour of every VS-* requirement: round trip on the quarter note, N Free
  taps off one channel, two channels not cross-talking, Confined staying stable at
  `Rate != 1`, and the ~1.9 GB VRAM figure.
- The old-encoding migration (`Confine` 0.5 and 1.0 both landing on Confined).
- No Tier-2 headless-GL coverage for varispeed. `delay-gl-tests` covers the delay
  only; the varispeed read shader has never been pixel-asserted.

## Notes

- FFGL crates do not build on WSL: vendored `ffgl-core`'s `build.rs` has cfg
  branches for macOS and Windows only, so `clang_args_ffgl` is undefined on Linux.
  Pure crates test natively; plugins go through `./scripts/build-plugin.sh`.
- `TAPE_SCALE = 1.0` in both Writes. Promoting it to a param is the lever for going
  past 2 channels (requirements §11) — not needed at 2.
