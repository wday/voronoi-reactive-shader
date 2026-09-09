# avc-media — development state

## Built
`tools/avc-query/` gains the subject `media` and the verbs `check` and `convert`.

| File | Role |
|------|------|
| `avc_query/paths.py` | Windows ↔ WSL path translation; UNC returns None |
| `avc_query/media.py` | ffprobe, verdicts, ffmpeg command construction, parallel conversion |
| `avc_query/resolume.py` | REST API client — port discovery, composition, live file sources, connected clips |
| `avc_query/parse.py` | `Source.file_path` now keeps the full path (it used to keep only the basename) |
| `avc_query/query.py` | `resolve_scope` / `_live_sources` (API-first), `_execute_media`, media field/like support |
| `avc_query/relink.py` | version bumping, relink planning, the byte-exact .avc rewrite |
| `avc_query/fmt.py` | media table columns, the `check` / `convert` / `replace` reports |
| `avc_query/cli.py` | media flags, help text |
| `avc_query/query.py` | `_execute_replace`, `_unsaved_warning` |

`~/.claude/skills/avc/SKILL.md` documents the new verbs, flags and the playback background.

Installed on PATH as an **editable** uv tool:
`uv tool install --editable ./tools/avc-query` → `~/.local/bin/avc`. It loads from the repo,
so source edits take effect with no reinstall; only a `pyproject.toml` change needs
`uv tool install --editable --force ./tools/avc-query`.

## Verified — the `replace` verb
Ran against the loaded composition, producing
`glitch dynamics v3.1.0 cv control.avc` → `glitch dynamics v3.1.1 cv control.avc`,
then diffed the two files line by line:

- **78 of 8694 lines differ, and every one is accounted for**: 76 path references
  (20 `VideoFile`, 20 `VideoFormatReaderSource`, 18 `AudioFile`, 18 `AudioFileSource`) plus
  the 2 internal name attributes. **No line changed for any other reason.**
- Each changed line differs *only* by the path swap — verified by substituting the old path
  into the old line and requiring an exact match with the new one.
- **CRLF preserved** (8693 endings, 0 bare LF), file still parses as XML.
- All 15 new paths exist on disk, are `.mov`, and sit under a `hap/` directory.
- **No old path survives** anywhere in the file.
- Version bumping: `v3.1.0 cv control` → `v3.1.1 cv control` (mid-name), `v1.0` → `v1.0.1`,
  `v3.1.9` → `v3.1.10`, no version → ` v1.0.1`, and with v3.1.1 present the next run
  targets v3.1.2 rather than overwriting.
- Guards: more than one matched composition is refused; a composition with a missing HAP
  sibling writes nothing and names it; the unsaved-changes warning only fires when the
  target actually is the loaded composition.

## Verified live against Resolume
Avenue 7.24.3, webserver on, API found at `http://172.26.224.1:8088/api/v1` (the WSL
default gateway — `localhost` is the Linux VM, and 8080 was held by a Windows `svchost`
that accepts the connection then resets it).

- **Port discovery** picks the right base in ~1s cold and ~0.02s from the cache.
- **Live file sources** — the API reports 20 file clips over 15 distinct paths, which is
  **exactly** what parsing the `.avc` produced. Layer/column numbering agrees between the
  two paths.
- **Name extraction** handles the real envelope, `{"valuetype": "ParamString", "value": ...}`.
- **Null handling** — `clip.video` / `video.fileinfo` are explicitly `null` for non-file
  clips (feedback, capture, router) and no longer raise.
- **`connected`** reads as a `ParamState`; 1 clip was connected and was reported as such.
- **Header** states the live composition, the API base, clip counts and connected count;
  `--no-live` and `in <glob>` correctly report the disk path instead.
- An unreachable base returns None and the audit degrades to disk rather than failing.

## Verified
- **Audit of the loaded composition** — `glitch dynamics v3.1.0 cv control.avc`, 15 distinct
  linked files (8 h264, 7 prores), all judged `convert`. 1.5 GB of source, ≤29.1 GB of HAP.
- **Real conversions, output probed against the source** — frame count, duration, geometry and
  fps identical in both cases:
  - h264 640x480 25fps → `HapY`, 1725 frames / 69.000 s both sides.
  - ProRes 422 HQ 1920x1080 23.976 → `HapY`, 16 frames / 0.667333 s both sides, `pcm_s16le`
    audio preserved.
- **Non-multiple-of-4 crop** — synthetic 641x361 encodes to 640x360 (ffmpeg hard-errors
  without this). `--fit pad` produces 644x364.
- **Alpha routing** — a qtrle/argb source is detected as alpha and encoded `hap_alpha`
  (`Hap5` tag), not `hap_q`.
- **Verdict round-trip** — a converted source re-reads as `converted`; feeding the *output*
  back through the judge reads `optimal`.
- **Alpha detection** — built from `ffmpeg -pix_fmts` component counts. `rgba`/`yuva420p`/
  `ya8`/`gbrap` true; `yuv420p`/`rgb0`/`0rgb`/`gray`/`yuv422p10le` false.
- **Existing verbs unaffected** — list/count/group over blocks, sources, params, and the
  `where` cross-cut all still return.

## Unverified
- Whether Resolume actually plays these HAP files back without stutter — verified as correct
  files, not yet as smooth playback.
- That the live path really does catch a clip added since the last save. The mechanism is
  direct (paths come from the live composition object, not the file), but proving it needs
  a clip dragged in without saving, or a write to the API, which has not been done.
- `connected_clips()` is exposed but only used for the count in the header; nothing filters
  on it yet.
- Discovery has only been exercised against Avenue on 8088. Wire's 8081 and a plain 8080
  install are untested.

## Invariants
- **No existing file is ever modified.** Converted media goes to a sibling `hap/` folder or
  `--out-dir`; `replace` writes a new `.avc` and never overwrites an existing version.
- **The .avc rewrite is binary, not text.** Text mode would translate CRLF to LF across the
  whole file. No XML round-trip either — it would reorder attributes and drop formatting.
- **A path is stored in four attributes per clip** (`VideoFile`, `VideoFormatReaderSource`,
  `AudioFile`, `AudioFileSource`). Rewriting only `fileName` leaves Resolume preloading the
  original file.
- **The composition name is stored inside the file** as well as in the filename, in
  `CompositionInfo/@name` and `Param[@name="Name"]/@value`. Both must be retitled.
- **A failed encode deletes its partial output.** A truncated .mov would otherwise probe as a
  valid clip and read back as `optimal`.
- **HAP requires multiples of 4** — ffmpeg errors out otherwise, so the crop/pad filter is not
  optional for non-conforming sources.
- **Crop, not pad, is the default.** Padding puts a black seam at the frame edge, which is
  destructive in feedback chains.
- **`hap_q` has no alpha in ffmpeg.** Alpha sources must fall to `hap_alpha` (DXT5).
- **The API is the source of truth when reachable**, since `fileinfo.path` reflects clips
  added since the last save. Disk parsing is the fallback, and the header always says which
  one produced the result.
- A dead API must degrade to the most recent `.avc`, never raise.
- **`localhost` does not reach Resolume from WSL** — the Windows host is the default gateway.
- **8080 cannot be assumed.** A Windows service commonly holds it and resets HTTP requests,
  which looks like an open port but a dead API; Resolume then runs on 8088.

## Note on files written
`D:\incoming\hap\Placopus_rubicundus_Vampyrellidae-15826238691.mov` (339.6 MB) was written to
the media drive as the end-to-end test of the default sibling-folder path. Nothing else on
`D:` was touched; the other test outputs are in the session scratchpad.
