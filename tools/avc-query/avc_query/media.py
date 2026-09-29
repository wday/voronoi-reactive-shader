"""Probe linked video files, judge them for stutter-free playback, convert to HAP.

Resolume plays back smoothly from intra-coded, GPU-decoded codecs: DXV3 (its own, no
third-party encoder exists) and the HAP family. Everything else — H.264, HEVC, ProRes,
DNxHD, MJPEG — is decoded on the CPU and is a stutter candidate. We target HAP because
it is the only one of the two that ffmpeg can write.
"""

from __future__ import annotations

import concurrent.futures
import functools
import json
import os
import shutil
import subprocess
import sys
from dataclasses import dataclass, field
from pathlib import Path

from .paths import to_win, to_wsl

# Codecs that decode on the GPU. ffmpeg names them 'hap' and 'dxv'.
OPTIMAL_CODECS = {"hap", "dxv"}

# HAP variant -> bytes per pixel of the DXT payload, before the snappy stage.
BYTES_PER_PIXEL = {"hap": 0.5, "hap_alpha": 1.0, "hap_q": 1.0}

VARIANTS = set(BYTES_PER_PIXEL)


@dataclass
class ConvertOpts:
    variant: str = "hap_q"
    chunks: int | None = None      # None = pick from resolution
    fit: str = "crop"              # crop | pad — reaching a multiple of 4
    out_dir: str | None = None     # None = '<source dir>/hap'
    jobs: int = 2
    force: bool = False
    dry_run: bool = False
    convert: bool = False          # 'replace' may encode missing HAP siblings first
    manifest: str | None = None    # 'migrate': TSV of source -> target-relative paths
    root: str | None = None        # 'migrate': media root on the destination machine


@dataclass
class Media:
    """One distinct video file linked by the composition, with every clip that uses it."""
    file_path: str = ""            # as stored in the .avc (Windows form)
    wsl_path: str | None = None
    name: str = ""
    verdict: str = ""              # optimal | convert | converted | missing | unreadable
    reason: str = ""
    codec: str = ""
    codec_tag: str = ""
    width: int = 0
    height: int = 0
    fps: float = 0.0
    avg_fps: float = 0.0
    vfr: bool = False
    pix_fmt: str = ""
    alpha: bool = False
    audio: bool = False
    duration: float = 0.0
    size: int = 0
    out_path: str = ""             # planned output (Windows form)
    est_size: int = 0
    clips: list = field(default_factory=list)   # ["1/flute", "2/zenface", ...]
    file: str = ""                 # composition stem

    @property
    def uses(self) -> int:
        return len(self.clips)


# -- ffmpeg capability --------------------------------------------------------


def have(tool: str) -> bool:
    return shutil.which(tool) is not None


@functools.lru_cache(maxsize=1)
def has_hap_encoder() -> bool:
    if not have("ffmpeg"):
        return False
    out = _run(["ffmpeg", "-hide_banner", "-encoders"])
    return any(line.split()[1:2] == ["hap"] for line in out.splitlines() if line.strip())


@functools.lru_cache(maxsize=1)
def _alpha_pix_fmts() -> frozenset:
    """Pixel formats carrying an alpha plane, read off `ffmpeg -pix_fmts`.

    Alpha formats have 4 components (rgba, yuva420p, gbrap) or 2 for grey+alpha (ya8).
    The padded formats rgb0/0rgb report 3 components, so they correctly read as opaque.
    """
    fmts = set()
    for line in _run(["ffmpeg", "-hide_banner", "-pix_fmts"]).splitlines():
        parts = line.split()
        if len(parts) < 3 or not parts[0].endswith(("...", "..B", ".P.")) and "." not in parts[0]:
            continue
        try:
            comps = int(parts[2])
        except ValueError:
            continue
        name = parts[1]
        if comps == 4 or (comps == 2 and name.startswith("ya")):
            fmts.add(name)
    return frozenset(fmts)


def _run(cmd: list[str]) -> str:
    try:
        r = subprocess.run(cmd, capture_output=True, text=True, timeout=60)
        return r.stdout + r.stderr
    except (OSError, subprocess.SubprocessError):
        return ""


# -- probing ------------------------------------------------------------------


def probe(wsl_path: str) -> dict | None:
    """ffprobe one file down to the fields the verdict needs. None if unreadable."""
    cmd = [
        "ffprobe", "-v", "error", "-print_format", "json",
        "-show_entries",
        "stream=codec_type,codec_name,codec_tag_string,width,height,"
        "r_frame_rate,avg_frame_rate,pix_fmt:format=duration,size",
        wsl_path,
    ]
    try:
        r = subprocess.run(cmd, capture_output=True, text=True, timeout=120)
    except (OSError, subprocess.SubprocessError):
        return None
    if r.returncode != 0 or not r.stdout.strip():
        return None
    try:
        data = json.loads(r.stdout)
    except json.JSONDecodeError:
        return None

    streams = data.get("streams", [])
    video = next((s for s in streams if s.get("codec_type") == "video"), None)
    if video is None:
        return None

    return {
        "codec": video.get("codec_name", ""),
        "codec_tag": video.get("codec_tag_string", ""),
        "width": int(video.get("width") or 0),
        "height": int(video.get("height") or 0),
        "fps": _ratio(video.get("r_frame_rate")),
        "avg_fps": _ratio(video.get("avg_frame_rate")),
        "pix_fmt": video.get("pix_fmt", ""),
        "audio": any(s.get("codec_type") == "audio" for s in streams),
        "duration": float(data.get("format", {}).get("duration") or 0.0),
        "size": int(data.get("format", {}).get("size") or 0),
    }


def _ratio(v) -> float:
    if not v or "/" not in str(v):
        try:
            return float(v or 0)
        except (TypeError, ValueError):
            return 0.0
    num, den = str(v).split("/", 1)
    try:
        num, den = float(num), float(den)
    except ValueError:
        return 0.0
    return num / den if den else 0.0


# -- building the media set ---------------------------------------------------


def collect(sources: list, opts: ConvertOpts) -> list[Media]:
    """Fold file Sources into one Media per distinct path, probe each, assign a verdict."""
    by_path: dict[str, Media] = {}
    for s in sources:
        if getattr(s, "source_type", "") != "file":
            continue
        raw = getattr(s, "file_path", "") or getattr(s, "source_name", "")
        if not raw:
            continue
        key = raw.lower()
        m = by_path.get(key)
        if m is None:
            m = Media(file_path=raw, name=raw.replace("\\", "/").rsplit("/", 1)[-1],
                      file=getattr(s, "file", ""))
            by_path[key] = m
        loc = f"{s.layer}/{s.clip}" if s.clip else str(s.layer)
        if loc not in m.clips:
            m.clips.append(loc)

    media = list(by_path.values())
    for m in media:
        _judge(m, opts)
    return media


def _judge(m: Media, opts: ConvertOpts):
    m.wsl_path = to_wsl(m.file_path)
    if m.wsl_path is None:
        m.verdict, m.reason = "missing", "path is a UNC share or an unrecognised form"
        return
    if not os.path.isfile(m.wsl_path):
        m.verdict, m.reason = "missing", "no file at this path"
        return

    info = probe(m.wsl_path)
    if info is None:
        m.verdict, m.reason = "unreadable", "ffprobe could not read a video stream"
        return

    for k, v in info.items():
        setattr(m, k, v)
    m.alpha = m.pix_fmt in _alpha_pix_fmts()
    m.vfr = bool(m.avg_fps and m.fps and abs(m.fps - m.avg_fps) / m.fps > 0.01)

    if m.codec in OPTIMAL_CODECS:
        m.verdict = "optimal"
        m.reason = f"already {m.codec} ({m.codec_tag})"
        return

    variant = target_variant(m, opts)
    out = output_path(m, opts)
    m.out_path = to_win(out)
    m.est_size = est_output_bytes(m, variant)

    if os.path.isfile(out) and not opts.force:
        if os.path.getmtime(out) >= os.path.getmtime(m.wsl_path):
            m.verdict = "converted"
            m.reason = f"up-to-date output already at {m.out_path}"
            return

    m.verdict = "convert"
    bits = [f"{m.codec} is decoded on the CPU"]
    if m.codec in ("h264", "hevc", "vp9", "av1", "mpeg4"):
        bits.append("long-GOP, so scrubbing decodes a whole group of pictures")
    if m.vfr:
        bits.append(f"variable frame rate ({m.fps:.3f} nominal vs {m.avg_fps:.3f} average)")
    if crop_dims(m, opts) != (m.width, m.height):
        w, h = crop_dims(m, opts)
        bits.append(f"{m.width}x{m.height} is not a multiple of 4, will {opts.fit} to {w}x{h}")
    if m.alpha:
        bits.append("has alpha, so hap_alpha (DXT5) is used instead of hap_q")
    m.reason = "; ".join(bits)


# -- conversion planning ------------------------------------------------------


def target_variant(m: Media, opts: ConvertOpts) -> str:
    """hap_q has no alpha in ffmpeg, so alpha sources fall back to hap_alpha."""
    if m.alpha and opts.variant == "hap_q":
        return "hap_alpha"
    return opts.variant


def chunk_count(m: Media, opts: ConvertOpts) -> int:
    if opts.chunks:
        return opts.chunks
    px = m.width * m.height
    if px <= 1280 * 720:
        return 1
    if px <= 1920 * 1080:
        return 4
    return 8


def crop_dims(m: Media, opts: ConvertOpts) -> tuple[int, int]:
    """HAP needs multiples of 4; ffmpeg errors out otherwise."""
    if opts.fit == "pad":
        return ((m.width + 3) // 4 * 4, (m.height + 3) // 4 * 4)
    return (max(4, m.width - m.width % 4), max(4, m.height - m.height % 4))


def output_path(m: Media, opts: ConvertOpts) -> str:
    stem = Path(m.name).stem
    if opts.out_dir:
        return str(Path(opts.out_dir) / f"{stem}.mov")
    return str(Path(m.wsl_path).parent / "hap" / f"{stem}.mov")


def est_output_bytes(m: Media, variant: str) -> int:
    frames = m.duration * (m.avg_fps or m.fps)
    return int(m.width * m.height * BYTES_PER_PIXEL[variant] * frames)


def ffmpeg_cmd(m: Media, opts: ConvertOpts) -> list[str]:
    variant = target_variant(m, opts)
    w, h = crop_dims(m, opts)
    out = output_path(m, opts)

    cmd = ["ffmpeg", "-hide_banner", "-loglevel", "error", "-y", "-i", m.wsl_path]

    if (w, h) != (m.width, m.height):
        vf = f"crop={w}:{h}" if opts.fit == "crop" else f"pad={w}:{h}:(ow-iw)/2:(oh-ih)/2"
        cmd += ["-vf", vf]

    cmd += ["-map", "0:v:0", "-c:v", "hap",
            "-format", variant,
            "-chunks", str(chunk_count(m, opts)),
            "-compressor", "snappy"]

    # VFR is itself a stutter cause — pin it to the measured average rate.
    if m.vfr and m.avg_fps:
        cmd += ["-fps_mode", "cfr", "-r", f"{m.avg_fps:.6f}"]

    if m.audio:
        cmd += ["-map", "0:a:0", "-c:a", "pcm_s16le"]

    cmd += ["-f", "mov", out]
    return cmd


# -- running ------------------------------------------------------------------


def convert_all(media: list[Media], opts: ConvertOpts) -> dict:
    """Convert everything with verdict 'convert'. Returns a summary dict."""
    todo = [m for m in media if m.verdict == "convert"]
    if not todo:
        return {"converted": 0, "failed": 0, "skipped": 0, "results": []}

    if opts.dry_run:
        for m in todo:
            print(_shell(ffmpeg_cmd(m, opts)))
        return {"converted": 0, "failed": 0, "skipped": len(todo), "results": []}

    if not has_hap_encoder():
        raise RuntimeError(
            "this ffmpeg has no 'hap' encoder — install a build with libsnappy "
            "(Ubuntu/Debian: apt install ffmpeg)"
        )

    for m in todo:
        Path(output_path(m, opts)).parent.mkdir(parents=True, exist_ok=True)

    results = []
    done = 0
    total = len(todo)

    def work(m: Media):
        cmd = ffmpeg_cmd(m, opts)
        try:
            r = subprocess.run(cmd, capture_output=True, text=True)
        except (OSError, subprocess.SubprocessError) as e:
            return m, 1, str(e)
        return m, r.returncode, (r.stderr or "").strip()

    with concurrent.futures.ThreadPoolExecutor(max_workers=max(1, opts.jobs)) as ex:
        for m, code, err in ex.map(work, todo):
            done += 1
            out = output_path(m, opts)
            if code == 0 and os.path.isfile(out) and os.path.getsize(out) > 0:
                size = os.path.getsize(out)
                print(f"[{done}/{total}] ok    {m.name} -> {to_win(out)} "
                      f"({_human(m.size)} -> {_human(size)})", file=sys.stderr)
                results.append({"name": m.name, "ok": True, "out": to_win(out), "size": size})
            else:
                # A partial file is worse than none — it would probe as a valid clip.
                if os.path.isfile(out):
                    os.unlink(out)
                print(f"[{done}/{total}] FAIL  {m.name}: {err.splitlines()[-1] if err else 'exit %d' % code}",
                      file=sys.stderr)
                results.append({"name": m.name, "ok": False, "error": err})

    ok = sum(1 for r in results if r["ok"])
    return {"converted": ok, "failed": len(results) - ok, "skipped": 0, "results": results}


def _shell(cmd: list[str]) -> str:
    import shlex
    return " ".join(shlex.quote(c) for c in cmd)


def _human(n: float) -> str:
    for unit in ("B", "KB", "MB", "GB", "TB"):
        if abs(n) < 1024 or unit == "TB":
            return f"{n:.0f}{unit}" if unit == "B" else f"{n:.1f}{unit}"
        n /= 1024
    return f"{n:.1f}TB"
