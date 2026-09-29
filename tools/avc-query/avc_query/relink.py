"""Write a version-bumped copy of a composition with its media relinked to HAP.

The original .avc is never modified. A file path is stored in FOUR places per clip, not
one, so relinking by rewriting <VideoFormatReaderSource fileName> alone would leave
Resolume preloading the original CPU-decoded file:

    <PreloadData><VideoFile value="D:\\...mov"/></PreloadData>
    <VideoSource ...><VideoFormatReaderSource fileName="D:\\...mov"/></VideoSource>
    <PreloadData><AudioFile value="D:\\...mov"/></PreloadData>
    <AudioFileSource FileName="D:\\...mov"/>

so the rewrite replaces the path string wherever it appears. That is safe because a full
absolute path is unique enough not to collide with anything else in the document, and it
keeps the rest of the file byte-identical — no XML round-trip, no reordered attributes.
"""

from __future__ import annotations

import os
import re
from dataclasses import dataclass
from pathlib import Path
from xml.sax.saxutils import escape

from .media import ConvertOpts, Media, output_path
from .paths import to_win

# vX.Y or vX.Y.Z anywhere in the name: 'glitch dynamics v3.1.0 cv control'.
_VERSION = re.compile(r"v(\d+)\.(\d+)(?:\.(\d+))?", re.IGNORECASE)


@dataclass
class RelinkPlan:
    source_avc: Path
    target_avc: Path
    old_stem: str
    new_stem: str
    mapping: dict                      # {old windows path: new windows path}
    needs_conversion: list             # Media still to encode
    unresolvable: list                 # Media that cannot be converted (missing/unreadable)


def bump_version(stem: str) -> str:
    """Bump the patch field. 'x v3.1.0 y' -> 'x v3.1.1 y'; 'x v1.0' -> 'x v1.0.1'.

    A two-field version is read as though its patch were 0. A name with no version at all
    is treated as an implicit v1.0.0, so it gains ' v1.0.1'.
    """
    matches = list(_VERSION.finditer(stem))
    if not matches:
        return f"{stem} v1.0.1"
    m = matches[-1]
    major, minor = int(m.group(1)), int(m.group(2))
    patch = int(m.group(3) or 0) + 1
    return f"{stem[:m.start()]}v{major}.{minor}.{patch}{stem[m.end():]}"


def next_free(directory: Path, stem: str) -> str:
    """Keep bumping until the name is free, so an existing version is never overwritten."""
    candidate = bump_version(stem)
    while (directory / f"{candidate}.avc").exists():
        candidate = bump_version(candidate)
    return candidate


def plan(source_avc: Path, media: list[Media], opts: ConvertOpts) -> RelinkPlan:
    mapping: dict = {}
    needs: list[Media] = []
    unresolvable: list[Media] = []

    for m in media:
        if m.verdict == "optimal":
            continue  # already a GPU codec — leave the link alone
        if m.verdict in ("missing", "unreadable"):
            unresolvable.append(m)
            continue
        out = output_path(m, opts)
        if os.path.isfile(out):
            mapping[m.file_path] = to_win(out)
        else:
            needs.append(m)

    old_stem = source_avc.stem
    new_stem = next_free(source_avc.parent, old_stem)
    return RelinkPlan(
        source_avc=source_avc,
        target_avc=source_avc.parent / f"{new_stem}.avc",
        old_stem=old_stem,
        new_stem=new_stem,
        mapping=mapping,
        needs_conversion=needs,
        unresolvable=unresolvable,
    )


def rewrite(text: str, mapping: dict, old_stem: str, new_stem: str) -> tuple[str, int]:
    """Swap every stored path and retitle the composition. Returns (text, replacements)."""
    count = 0
    for old, new in mapping.items():
        # Attribute values are XML-escaped; paths are matched in that same form.
        old_esc, new_esc = escape(old), escape(new)
        n = text.count(old_esc)
        if n:
            text = text.replace(old_esc, new_esc)
            count += n

    # Resolume shows the name stored inside the file, not the filename, so retitle both
    # places it lives — otherwise the copy opens still calling itself the old version.
    old_esc, new_esc = escape(old_stem), escape(new_stem)
    text = re.sub(
        r'(<CompositionInfo\b[^>]*?\bname=")' + re.escape(old_esc) + r'(")',
        lambda m: m.group(1) + new_esc + m.group(2), text, count=1)
    text = re.sub(
        r'(<Param\b[^>]*?\bname="Name"[^>]*?\bvalue=")' + re.escape(old_esc) + r'(")',
        lambda m: m.group(1) + new_esc + m.group(2), text, count=1)
    return text, count


def apply(p: RelinkPlan) -> int:
    """Binary I/O throughout.

    Text mode would translate the file's CRLF line endings to LF on read and write them
    back as LF, silently rewriting all ~8.7k lines of a composition that is supposed to
    change only where a path does.
    """
    raw = p.source_avc.read_bytes()
    text, count = rewrite(raw.decode("utf-8"), p.mapping, p.old_stem, p.new_stem)
    p.target_avc.write_bytes(text.encode("utf-8"))
    return count
