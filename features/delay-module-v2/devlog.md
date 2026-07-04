# Delay Module v2 — Dev Log

## 2026-03-22 — Requirements and plan

- Wrote requirements based on read-before-write diagram (`/mnt/d/assets/jzp15i/read-before-write-simple-model.png`)
- Core insight: split read and write so Resolume effects can process feedback before it's written back

### Design iteration: mixing belongs to Resolume, not the plugin

Initial draft had Feedback and Send Amount as plugin params. Realized this duplicates what Resolume already provides:
- **Resolume effect opacity + blend mode on Read** = feedback level (how much delayed signal mixes with input)
- **Gain/exposure effect before Read** = send level (how much live source enters the loop)

This means Read is trivially simple — just output `buffer[write_pos]`. No timing, no mixing, no feedback param. Write owns all timing and buffer management. Plugin params drop from 8 to 7 (removed Passthrough).

### Buffer sizing: N+1 stitch

Walking through the frame sequence revealed that reading `buffer[write_pos]` gives 1-frame delay regardless of loop length — Read gets what Write wrote last frame, not loop_length frames ago.

Fix: allocate `loop_length + 1` slots. Read reads `(write_pos + 1) % buf_size` — the oldest slot, which is exactly `loop_length` frames old. The +1 slot prevents read/write from aliasing the same frame.

Registry returns `buf_size` instead of `loop_length` so Read can compute its position without knowing timing details.

### Key decisions
- Read has no timing params — computes read_pos from write_pos + buf_size via registry
- Write outputs buffer content (what it wrote), not passthrough of live input
- "No Write active = Read frozen on a frame" is an acceptable edge case for now
- Linked to rktpnt project `jzp15i` (Delay Module v2), task `j6kwz5`

## 2026-07-04 — Direction resolved: v2 is canonical, overdub absorbed

Two uncommitted delay-line directions had diverged: `overdub` (decay +
additive multi-Send on the old Send/Receive model) and this v2 Read/Write
restructure. Decided **v2 Read/Write is the canonical model** going forward.

Overdub's IFS-accumulation goal folds into v2 rather than living as a
separate feature:
- **Decay** lives on Write (`Write(ch1, decay=0.9)`), scaling the previous
  buffer content before the new write.
- **Additive accumulation** across contractive maps is achieved via
  Resolume blend modes on Read — which v2 already delegates to Resolume —
  instead of the plugin's own `GL_BLEND(ONE, ONE)` multi-Send coordination.

Net effect: `features/overdub/` is superseded as an implementation but
survives as a **validation composition** (the IFS attractor recipe) to
prove v2 out once Read/Write is built. Next step: implement the v2 plan
(registry N+1 stitch, params restructure, Read/Write passes).
