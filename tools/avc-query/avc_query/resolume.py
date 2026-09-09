"""Talk to a running Resolume, to learn which composition is loaded.

Arena and Avenue serve a REST API on port 8080 (Wire on 8081) once it is switched on in
Preferences -> Web Server. Resolume does not publish a schema for the composition object,
so name extraction is deliberately tolerant: walk the JSON for a name-shaped field rather
than assuming a fixed path. Every failure returns None — a dead API must degrade to
reading the most recent .avc, never crash the query.
"""

from __future__ import annotations

import functools
import json
import os
import pathlib
import urllib.error
import urllib.request
from dataclasses import dataclass

# Avenue/Arena default to 8080 and Wire to 8081, but 8080 is commonly squatted by a
# Windows service that accepts the connection and then resets it, in which case Resolume
# lands on 8088 instead. Under WSL, 'localhost' is the Linux VM, not Windows, so the
# Windows side is only reachable over the default-gateway address.
DEFAULT_BASE = os.environ.get("AVC_RESOLUME_API", "")
CANDIDATE_PORTS = (8080, 8088, 8081)
TIMEOUT = 3.0
PROBE_TIMEOUT = 1.0


def _get(base: str, path: str, timeout: float = TIMEOUT):
    url = f"{base.rstrip('/')}/{path.lstrip('/')}"
    try:
        with urllib.request.urlopen(url, timeout=timeout) as r:
            return json.loads(r.read().decode("utf-8", "replace"))
    except (urllib.error.URLError, OSError, ValueError, json.JSONDecodeError):
        return None


def _gateway() -> str | None:
    """The Windows host address as seen from WSL — 'localhost' is the Linux VM."""
    try:
        with open("/proc/net/route") as f:
            for line in f.readlines()[1:]:
                fields = line.split()
                if len(fields) > 2 and fields[1] == "00000000":
                    hexip = fields[2]
                    octets = [str(int(hexip[i:i + 2], 16)) for i in (6, 4, 2, 0)]
                    return ".".join(octets)
    except OSError:
        pass
    return None


def candidate_bases() -> list:
    """Where Resolume might be listening, best guess first."""
    if DEFAULT_BASE:
        return [DEFAULT_BASE]
    hosts = ["127.0.0.1"]
    gw = _gateway()
    if gw:
        hosts.append(gw)
    return [f"http://{h}:{p}/api/v1" for h in hosts for p in CANDIDATE_PORTS]


CACHE = pathlib.Path(
    os.environ.get("XDG_CACHE_HOME", os.path.expanduser("~/.cache"))
) / "avc-query" / "api-base"


@functools.lru_cache(maxsize=4)
def discover_base(explicit: str = "") -> str | None:
    """First candidate that answers /product, or None if Resolume isn't reachable.

    The winner is cached to disk: probing six candidates costs a couple of seconds,
    which is too much to pay on every invocation. A stale cache costs one failed
    request before the full sweep runs again.
    """
    if explicit:
        return explicit if _get(explicit, "product", timeout=PROBE_TIMEOUT) else None

    try:
        cached = CACHE.read_text().strip()
    except OSError:
        cached = ""
    if cached and _get(cached, "product", timeout=PROBE_TIMEOUT) is not None:
        return cached

    for base in candidate_bases():
        if _get(base, "product", timeout=PROBE_TIMEOUT) is not None:
            try:
                CACHE.parent.mkdir(parents=True, exist_ok=True)
                CACHE.write_text(base)
            except OSError:
                pass  # a read-only cache dir must not break the query
            return base
    return None


def product(base: str = "") -> dict | None:
    """Liveness check. Returns the product blob (name/version) or None."""
    base = discover_base(base) or ""
    p = _get(base, "product") if base else None
    return p if isinstance(p, dict) else None


@functools.lru_cache(maxsize=4)
def composition(base: str = "") -> dict | None:
    base = discover_base(base) or ""
    c = _get(base, "composition") if base else None
    return c if isinstance(c, dict) else None


def composition_name(base: str = "") -> str | None:
    """Name of the loaded composition, or None if Resolume isn't reachable."""
    comp = composition(base)
    if comp is None:
        return None
    return _find_name(comp)


def _find_name(comp: dict) -> str | None:
    """Pull a composition name out of the object without assuming its shape.

    Resolume wraps parameters as {"value": ..., "id": ...}, but the nesting has moved
    between versions, so accept a bare string too. Only the top level is searched —
    descending would pick up layer and clip names.
    """
    for key in ("name", "compositionName", "title"):
        if key not in comp:
            continue
        v = comp[key]
        if isinstance(v, str) and v.strip():
            return v.strip()
        if isinstance(v, dict):
            for vk in ("value", "name", "index"):
                vv = v.get(vk)
                if isinstance(vv, str) and vv.strip():
                    return vv.strip()
    return None


def _param(obj, key):
    """Unwrap Resolume's {"valuetype": ..., "value": ...} parameter envelope."""
    v = (obj or {}).get(key)
    if isinstance(v, dict):
        return v.get("value")
    return v


# 'connected' is a ParamState whose options are
# Empty / Disconnected / Previewing / Connected / Connected & previewing.
_CONNECTED = {"Connected", "Connected & previewing"}


@dataclass
class ApiSource:
    """A file source read from the live composition.

    Shaped to duck-type the parser's Source so media.collect() can consume either.
    """
    source_type: str = "file"
    source_name: str = ""
    file_path: str = ""
    scope: str = "clip"
    layer: int | None = None
    clip: str | None = None
    col: int | None = None
    file: str = ""
    connected: bool = False


def file_sources(base: str = "", comp: dict | None = None) -> list | None:
    """Every clip in the live composition backed by a video file on disk.

    Read from layers[].clips[].video.fileinfo.path. Returns None when Resolume is not
    reachable, which is different from an empty list (reachable, no file clips).

    Layer and column numbering matches the .avc parser's 0-based enumeration so that
    'avc list sources' and 'avc list media' agree with each other.
    """
    if comp is None:
        comp = composition(base)
    if comp is None:
        return None

    name = _find_name(comp) or ""
    out: list[ApiSource] = []
    for li, layer in enumerate(comp.get("layers") or []):
        for ci, clip in enumerate((layer or {}).get("clips") or []):
            # video, fileinfo and path are each explicitly null for non-file clips.
            fileinfo = ((clip or {}).get("video") or {}).get("fileinfo") or {}
            path = fileinfo.get("path")
            if not path:
                continue
            out.append(ApiSource(
                source_name=path.replace("\\", "/").rsplit("/", 1)[-1],
                file_path=path,
                layer=li,
                clip=_param(clip, "name") or "",
                col=ci,
                file=name,
                connected=_param(clip, "connected") in _CONNECTED,
            ))
    return out


def connected_clips(base: str = "") -> set:
    """{(layer_index, clip_index)} for clips currently connected. Empty set if unknown."""
    comp = composition(base)
    if not comp:
        return set()
    out = set()
    for li, layer in enumerate(comp.get("layers") or []):
        for ci, clip in enumerate((layer or {}).get("clips") or []):
            if _param(clip, "connected") in _CONNECTED:
                out.add((li, ci))
    return out
