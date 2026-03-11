# Generation Tracking — Requirements

## Problem
Each evolve run overwrites the previous batch. Lineage is lost, and the explore directory becomes overwhelming with no way to compare across generations.

## Requirements
1. **Generation subfolders** — each explore run writes to `gen_NNN/` under the output dir, auto-incrementing
2. **Generation manifest** — each gen folder gets its own `manifest.json` with parent lineage metadata (which keepers seeded it, from which generation)
3. **Gallery timeline slider** — horizontal slider (top or bottom of gallery) to navigate between generations
4. **Keeper lineage** — track which keeper(s) a mutation descended from; show parent info on hover/detail
5. **Cross-gen keepers** — keepers are stored per-generation; "Save & Evolve" writes keepers into the current gen's folder
6. **Gallery serves all generations** — reads all `gen_NNN/` folders, builds a combined view filterable by generation
