# Generation Tracking — Plan

## Step 1: explore.py — generation-aware output
- Auto-detect next gen number from existing `gen_NNN/` folders
- Write images + manifest into `gen_NNN/` subfolder
- Add `generation` and `parent` fields to manifest entries
- Add top-level `generations.json` that indexes all gens with metadata (seed, timestamp, keeper source)
- `--keepers` reads from the specified gen's keepers.json

## Step 2: gallery.py — generation timeline UI
- Scan all `gen_NNN/` folders on load
- Build combined manifest with generation info
- Add horizontal timeline slider bar at top of gallery
- Slider positions = generation numbers; clicking/sliding filters the grid
- "All" option to see everything
- Show generation info in header (gen number, count, parent gen)

## Step 3: gallery.py — per-gen keepers
- Keepers saved to `gen_NNN/keepers.json` (current gen)
- Load keepers per-gen from localStorage keyed by gen number
- "Save & Evolve" writes to current gen's folder

## Step 4: lineage display
- Show parent keeper filename on card hover (in info overlay)
- Modal view shows full lineage chain

## Step 5: migrate existing data
- Move current flat images into `gen_000/` as the seed generation
