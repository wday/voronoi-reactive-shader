# avc-query — Implementation plan

## Step 1: Project scaffold + AVC parser
- Create `tools/avc-query/` with `pyproject.toml` (uv, no deps)
- Build `avc_parse.py`: parse `.avc` XML into flat records
  - `Block` record: type, name, blend, base_type, params dict, location (deck, layer_idx, clip_name, col_idx, scope)
  - `Source` record: source_type, source_name, file_path, device_id, location
  - Filter out boilerplate (Transform wrappers, DryWet shells) — keep only "visible" effects
  - Resolve DryWet → inner effect + blend mode as a single logical block

## Step 2: Query parser
- Simple token-based parser for the grammar (no external deps)
- Parse: `<verb> <subject> [like <pat>] [by <field>] [with <param> <op> <val>] [order by <param>] [where <subject> <clause>] [in <glob>]`
- Clause order doesn't matter — collect into a query spec

## Step 3: Query engine
- File resolver: find compositions dir (rktpnt fact or fallback), resolve `in <glob>` or default to most recent
- Filter: apply `like` (fuzzy/ci), `with` (param comparison)
- Cross-cut `where`: join blocks↔sources at clip level
- Group/sort/count as post-processing

## Step 4: Output formatting
- Table: aligned columns, truncate long values
- NDJSON: one JSON object per record with `--json`

## Step 5: CLI entry point
- `avc_query.py` as main, args = everything after `avc`
- uv script entry point so `uv run avc <query>` works
- Wire up parser → engine → formatter
