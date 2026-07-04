# avc-query — Development log

## 2026-03-19 — Design session

Explored AVC XML structure from `AT - 003 - Live Wire.avc`:

- Effects are `RenderPass` elements. Key distinction: `DryWetEffect` wraps an inner effect + a blend mode (`Mixer` baseType). LoRez appears as a Mixer (blend mode on BrightnessContrast), not a standalone effect.
- Layers have indices but no user-visible names. Clips have names via nested `Param[@name="Name"]`. Decks have names in `DeckInfo`.
- Four source types discovered: `VideoSourceFeedback`, `GeneratorVideoSource` (identity from nested RenderPass type), `CaptureDeviceVideoSource` (identity from CaptureSource.deviceId), `VideoFormatReaderSource` (identity from fileName attr).

Designed query grammar around two subjects (`blocks`, `sources`) with composable clauses and `where` for cross-cutting joins at clip level. See requirements.md for full spec.

Note: existing rktpnt task `zfv4g7` ("AVC diff tool / rktpnt skill") is related — query tool is the read side, diff would build on same parser.

## 2026-03-19 — v0.1 implementation

Built `tools/avc-query/` — uv-managed Python project, zero external deps.

Files: `parse.py` (XML→records), `query.py` (parser+engine), `fmt.py` (table/NDJSON), `cli.py` (entry point).

Invocation: `uv run --project tools/avc-query avc <verb> <subject> [clauses...]`

Fixes during testing:
- VideoSource lives under `VideoTrack → PrimarySource → VideoSource`, not as a sibling — had to extract sources from within VideoTrack handler
- DryWet-wrapped TransformEffect wasn't being filtered — added skip in `_extract_drywet`
- Windows backslash paths in `VideoFormatReaderSource.fileName` — `Path.name` doesn't work cross-platform, switched to string split

Tested against: `AT - 003 - Live Wire.avc`, `webcam feedback test - 008*.avc`, multi-file queries.

All query shapes working: list, group, count, like, with, order by, where (cross-cut), in (file glob), --json.
