# avc-query — Resolume composition query tool

## Goal
Ergonomic CLI for querying Resolume `.avc` (XML) composition files. Answers questions like "what effects use LoRez blending", "which clips have cameras", "what's the opacity on everything" — without opening Resolume or hand-reading XML.

## Design principles
- **Work backwards from ergonomics** — the query language should feel natural, not like XPath or jq
- **Chainable subjects** — `blocks` (effects) and `sources` (clip inputs) can cross-reference via `where`
- **Fuzzy by default** — `like` does case-insensitive substring/fuzzy match so you don't need to remember `LoRez` vs `lores`
- **Sensible defaults** — most recent `.avc` file, table output, no boilerplate

## Query grammar

```
avc <verb> <subject> [clauses...]
```

### Verbs
| Verb | Purpose |
|------|---------|
| `list` | Show matching records |
| `group` | Group output by a field |
| `count` | Count records (optionally grouped) |

### Subjects
| Subject | What it represents | Key fields |
|---------|-------------------|------------|
| `blocks` | Effect instances (RenderPass) | type, name, blend, params (opacity, etc.), location |
| `sources` | Clip video sources | source_type (feedback/generator/capture/file), source_name, file_path, device_id, location |

Both subjects share **location context**: deck, layer (index), clip (name + column), scope (comp/layer/clip level).

### Clauses (composable, order-independent)
| Clause | Example | Purpose |
|--------|---------|---------|
| `like <pattern>` | `like lo` | Fuzzy/case-insensitive match on type/name |
| `by <field>` | `by blend` | Group-by key |
| `with <param> <op> <value>` | `with opacity < 0.5` | Filter on param value |
| `order by <param>` | `order by opacity` | Sort by param value |
| `where <other-subject> <clause>` | `where blocks like lo` | Cross-cut: filter by related subject at clip level |
| `in <glob>` | `in "geiger*"` | Scope to specific files |

### Scope resolution
- **Default**: most recently modified `.avc` in the Resolume compositions directory
- **`in <glob>`**: match against composition filenames (not full paths)
- Compositions directory from rktpnt facts: `/mnt/c/Users/alien/Documents/Resolume Avenue/Compositions/`

### Output
- **Default**: aligned table (human-readable)
- **`--json`**: NDJSON for piping to `jq`

## Example queries

```bash
# Effects
avc list blocks                           # all effects in most recent composition
avc list blocks like mirror               # effects matching "mirror"
avc group blocks by blend like lo         # LoRez-ish blends, grouped by blend mode
avc list blocks with opacity < 0.5        # low-opacity effects
avc list blocks order by opacity          # sorted by opacity
avc count blocks by type                  # effect census

# Sources
avc list sources                          # all sources
avc list sources like camera              # capture devices
avc group sources by type                 # feedback vs generator vs capture vs file

# Cross-cutting
avc list sources where blocks like lo     # sources in clips that use LoRez-ish effects
avc list blocks where source like camera  # effects on camera clips

# Multi-file
avc count blocks by type in "geiger*"     # effect census across geiger compositions
```

## AVC XML structure (reference)

### Effect instances (`blocks`)
- `RenderPass` elements with `type` attribute
- `baseType`: `Effect`, `DryWetEffect` (wrapper), `Mixer` (blend mode)
- DryWet wrapper contains: inner effect RenderPass + ChoosableMixer with blend RenderPass
- Params nested as `ParamRange` (numeric) and `ParamChoice` (enum)
- Location: nested under Composition/Layer/Clip → VideoTrack → RenderPass chain

### Video sources (`sources`)
- `VideoSource` element inside each Clip
- `type` attribute: `VideoSourceFeedback`, `GeneratorVideoSource`, `CaptureDeviceVideoSource`, `VideoFormatReaderSource`
- Generator identity: nested `RenderPass.type` (e.g., `Tunnelines`, `Rings`)
- Capture identity: `CaptureSource.deviceId`
- File identity: nested `VideoFormatReaderSource.fileName`

### Location hierarchy
- Composition → Deck (named) → Layer (indexed) → Clip (named, indexed by layer+column)
- Effects can live at any level: composition VideoTrack, layer VideoTrack, or clip VideoTrack

## Implementation
- Python, uv-managed project in `tools/avc-query/`
- `xml.etree.ElementTree` for parsing
- Entry point: `avc_query.py` (or `avc` alias via uv script)
- No external dependencies beyond stdlib initially
