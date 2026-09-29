# Varispeed — Plan

Remaining work for the v0.2 contraction. Code is landed, built and deployed to
Resolume's Extra Effects; what is left is playing it. Delete steps as they land.

---

## Step 1 — Live validation in Resolume
Existing patches need re-making regardless (loop max 2 s -> 1 s, Confine encoding
remapped, param indices shifted).
- **VS-ROUNDTRIP:** Free, `Rate = 1x`, `Loop = 1/4`. Content must recirculate on the
  quarter note. Changing Loop Length must audibly/visibly change the tap time —
  this is the knob that did nothing before.
- **VS-MULTITAP:** one Write + three Free Reads on channel 1 at 1/16, 1/8, 1/4.
  Three distinct taps off one recording.
- **VS-CHANNELS:** two independent networks on channels 1 and 2 with different
  in-loop FX stacks; confirm no cross-talk.
- **VS-CONFINED-STABLE:** Confined, `Rate = 1/2x`, recording — accumulates without
  the runaway cascade.
- **VRAM:** confirm ~1.9 GB with both channels live (was ~4 GB for one tape).

If Resolume shows the **old** param list (no Channel, a Reverse option, a 2000 ms
loop max), it has cached the plugin by `unique_id`: bump `VsRd`/`VsWr` ->
`VsR2`/`VsW2` in each crate's `plugin_info()`, rebuild and redeploy. Not done
pre-emptively because it breaks existing patches' param mappings.
