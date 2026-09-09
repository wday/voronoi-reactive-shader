# avc-media — audit and convert linked media for stutter-free playback

## Goal
Extend the `avc` tool so it can answer "will this set play smoothly?" and fix the answer.
Read the composition **currently loaded in Resolume**, resolve every linked video file,
judge whether it is in a GPU-decodable format, batch-convert the ones that aren't, and write
a version-bumped copy of the composition that points at the converted files.

## Why these formats
Resolume plays back smoothly when frames are **intra-coded and GPU-decoded**: every frame
decodes independently (cheap scrubbing, reverse, random access) and the compressed blocks
upload straight to the GPU as DXT/BC textures. Two codecs qualify:

| Codec | Who writes it | Notes |
|-------|---------------|-------|
| **DXV3** | Resolume Alley only | Resolume's own recommendation. No third-party encoder exists. |
| **HAP / HAP Alpha / HAP Q** | ffmpeg (`-c:v hap`) | Open equivalent, played natively by Resolume. |

**This tool targets HAP**, because it is the only one of the two that can be driven
headlessly from a script. DXV3 is out of scope.

Everything else is a conversion candidate, including formats that *look* professional:
- **H.264 / HEVC** — long-GOP, CPU-decoded. Worst case; a scrub can force decoding a whole GOP.
- **ProRes** — intra-frame but CPU-decoded and bandwidth-hungry (ProRes 422 HQ 1080p ≈ 168 Mbps).
- **MPEG-4, DNxHD, MJPEG, animation/RLE, image sequences** — same reasoning.

## Non-goals
- No DXV3 encoding.
- No modification of any existing file. `replace` writes a **new** `.avc`; the original
  composition and all original media are untouched.
- No live relinking through the API. The new composition is opened by the operator.
- No transcoding decisions about resolution or frame rate. Source geometry and timing are preserved.

## Expect files to get BIGGER
HAP trades size for decode cost. HAP Q is DXT5-YCoCg — 8 bits/pixel before snappy, so
1080p24 has an upper bound near 400 Mbps, above the ProRes source it replaces. Plain HAP
(DXT1) is 4 bits/pixel. This is normal and is the point of the format; the audit must
report an estimated output size up front so a batch cannot silently fill a disk.

## Source of truth for "currently loaded"
The REST API is the primary source, because the live composition object carries the file
paths directly at `layers[].clips[].video.fileinfo.path`. That is the true current state,
**including clips added since the last save**.

1. Discover the API and read the live composition; build the media set from its file paths.
2. If Resolume is unreachable, fall back to parsing the most recently modified `.avc` from
   disk and **say so in the report header**.

`--no-live` forces the disk path; `in <glob>` implies it. The header always states which
way it went, the composition name, and the API base it found.

### API details
Base path `/api/v1`; liveness is `GET /api/v1/product`. Enabled in Preferences → Web Server.

**Port discovery is required, not optional.** Arena/Avenue default to 8080 and Wire to 8081,
but 8080 is commonly held by a Windows `svchost` that accepts the TCP connection and then
resets it, in which case Resolume lands on **8088**. Under WSL, `localhost` is the Linux VM,
so the Windows side is only reachable over the default-gateway address from `/proc/net/route`.
The tool sweeps {127.0.0.1, gateway} × {8080, 8088, 8081} for one that answers `/product`,
and caches the winner to `~/.cache/avc-query/api-base` because a full sweep costs ~2s and
that is too much to pay per invocation. `--api <url>` overrides, `AVC_RESOLUME_API` presets it.

### Response shape
Parameters are wrapped as `{"valuetype": ..., "value": ...}` and must be unwrapped.
`clip.video`, `video.fileinfo` and `fileinfo.path` are each explicitly `null` for non-file
clips. `connected` is a `ParamState` over
Empty / Disconnected / Previewing / Connected / Connected & previewing.
`fileinfo` also carries `exists`, `duration_ms`, `framerate` and geometry, and
`video.description` carries Resolume's own decoder note (e.g. `MF h264, 480x360, 23.98 Fps`) —
ffprobe stays authoritative for verdicts.

Layer and column numbering follows the `.avc` parser's 0-based enumeration so that
`avc list sources` and `avc list media` agree with each other.

## Verdicts
Each linked file resolves to exactly one verdict:

| Verdict | Meaning |
|---------|---------|
| `optimal` | HAP family already, correct container, nothing to do |
| `convert` | A real file in a CPU-decoded format |
| `missing` | Path in the `.avc` does not resolve on disk |
| `unreadable` | Resolves but ffprobe cannot parse it |
| `converted` | An up-to-date HAP output already exists for it |

Non-file sources (capture devices, routers, generators, feedback) are not media and are excluded.

## Conversion rules
- **Variant**: `hap_q` by default. Sources carrying an alpha channel get `hap_alpha`
  (ffmpeg has no `hap_q_alpha`); this is a quality drop and is reported.
- **Dimensions**: HAP requires multiples of 4 and ffmpeg errors out otherwise. Non-conforming
  sources are **cropped down** to the nearest multiple of 4 (at most 3px per axis), not padded —
  padding puts a black seam at the frame edge, which is destructive in feedback chains.
  `--fit pad` overrides.
- **Chunks**: `-chunks` defaults to 1, which serialises decode. Set by resolution:
  ≤720p → 1, ≤1080p → 4, above → 8. `--chunks N` overrides.
- **Audio**: preserved as `pcm_s16le`. Files without audio stay without.
- **Frame rate**: preserved. Variable-frame-rate sources are flagged and forced to CFR at
  their average rate, because VFR is itself a stutter cause.
- **Container**: `.mov`.

## Output layout
Converted files land in a **sibling folder** next to each source: `<source dir>/hap/<stem>.mov`.
`--out-dir DIR` sends everything to one directory instead. Originals are never touched.
An existing output that is newer than its source is left alone unless `--force` is given.

## CLI surface
Extends the existing grammar with the subject `media` and the verbs `check` and `convert`.

```
avc list media                     linked files with codec, geometry, verdict
avc check media                    audit + per-file reason + total estimated output size
avc convert media                  convert everything with verdict=convert
avc convert media --dry-run        print the exact ffmpeg command per file, run nothing
avc convert media --jobs 4         parallel encodes (default 2)
avc convert media --out-dir DIR    single output directory
avc convert media --variant hap    force plain HAP (half the size of HAP Q)
avc convert media --force          re-encode even if an up-to-date output exists
avc check media --no-live          skip the API, use the most recent .avc
avc check media in "glitch*"       audit named compositions instead of the loaded one
avc list media --json              NDJSON
```

## Relinking — the `replace` verb
`avc replace` writes a **version-bumped copy** of a composition whose media points at the
HAP files instead of the originals.

### Version bump
The patch field of a `vX.Y.Z` in the filename is incremented, wherever it sits in the name:
`glitch dynamics v3.1.0 cv control` → `glitch dynamics v3.1.1 cv control`.
A two-field `vX.Y` is read as though its patch were 0 (`v1.0` → `v1.0.1`). A name with no
version is treated as an implicit v1.0.0 and gains ` v1.0.1`. If the target name already
exists the patch keeps incrementing until it is free — **an existing version is never
overwritten**.

### A path lives in four places, not one
Per clip, Resolume stores the same path in four attributes:

```
<PreloadData><VideoFile value="..."/></PreloadData>
<VideoSource ...><VideoFormatReaderSource fileName="..."/></VideoSource>
<PreloadData><AudioFile value="..."/></PreloadData>
<AudioFileSource FileName="..."/>
```

Rewriting only `fileName` would leave Resolume preloading the original CPU-decoded file, so
the rewrite replaces the path string wherever it occurs. An absolute path is unique enough
in the document not to collide with anything else.

The composition also stores its **own name** internally, in `<CompositionInfo name="...">`
and `<Param name="Name" value="...">`. Both are retitled, or the copy opens still calling
itself the old version.

### The rewrite must be byte-exact everywhere else
Read and write as **bytes**. Text mode translates the file's CRLF endings to LF, silently
rewriting all ~8.7k lines of a file that should change only where a path does. No XML
round-trip either — that would reorder attributes and drop formatting.

### Missing siblings
If any linked file has no HAP output yet, `replace` **writes nothing** and lists them.
`--convert` encodes them first and then relinks; if any encode fails, nothing is written.
Files that are `missing` or `unreadable` on disk cannot be converted — they are reported and
left pointing at their original path.

### Unsaved changes
`replace` rewrites a file, so it reads the `.avc` from disk, not the live composition. When
the target *is* the loaded composition, its file list is compared against the API's and any
divergence is reported as a warning to save first. Comparing against a different
composition would be meaningless, so it is skipped.

## Path translation
`.avc` stores Windows paths (`D:\Dropbox\...`). The tool maps drive letters to `/mnt/<letter>/`
for reading, and reports paths back in **Windows form**, since that is what the operator
types into Resolume. UNC paths are reported as `missing` with a distinct reason.

## Preconditions
- `ffmpeg` and `ffprobe` on `PATH` (WSL build is fine; the hap encoder is present in 6.1.1).
- The tool refuses to run `convert` if the hap encoder is absent, naming the missing piece.
