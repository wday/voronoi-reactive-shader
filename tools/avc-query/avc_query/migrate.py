"""Rewrite a composition's media paths onto a different machine's filesystem.

Distinct from `relink`, which swaps a file for its HAP sibling on the *same* machine.
Here the file is the same file; the machine underneath it changed. That shows up as
four eras of stored path in one set of compositions:

    C:\\Users\\alien\\Documents\\Dropbox\\Satans Goat Sluts\\...   old Windows Dropbox
    D:\\Dropbox\\Music\\...                                       current Windows
    /Volumes/T9/Dropbox/...                                      an external, on macOS
    /Users/williamday/Desktop/...                                the old MacBook

so a mapping keyed on the path as stored would only ever catch one of them.

Matching is on the longest run of trailing path components, not on the filename, because
filenames collide: 71 of the 416 manifest entries share a basename with another entry.
IMG_4307.mov exists three times over — as the camera original, as its HAP encode, and as
a tone-mapped variant — and linking a composition to the wrong one produces a file that
opens and plays and is silently not what the set was built against. Trailing components
disambiguate those (`.../hap/IMG_4307.mov` beats `.../iphone/IMG_4307.MOV` on the second
component) and survive the machine change at the same time, since what stays stable
across all four eras is the tail of the path, never its head.
"""

from __future__ import annotations

import os
from dataclasses import dataclass, field
from pathlib import Path

from .relink import apply as _apply, rewrite as _rewrite


@dataclass
class Entry:
    """One manifest row: where the file is now, and where it is going."""
    source: str          # path on the machine the manifest was built on
    target_rel: str      # path relative to the new media root, POSIX form
    cls: str = "stage"
    parts: tuple = ()    # normalised source components, reversed for suffix matching


# Resolume ships demo media inside its own install. Those paths are not in any manifest
# and must not be: they are supplied by the application, at a location that differs per
# platform. Translating them is a fixed rule, and a macOS-form one is already correct.
_BUNDLED_WIN = "program files/resolume avenue/media/"
_BUNDLED_MAC = "/Applications/Resolume Avenue/media/"


def _bundled(stored: str) -> str | None:
    """New path for Resolume's own media, '' if already correct, None if not bundled."""
    low = stored.replace("\\", "/").lower()
    if low.startswith("/applications/resolume avenue/media/"):
        return ""            # already macOS form — leave the link alone
    i = low.find(_BUNDLED_WIN)
    if i >= 0:
        return _BUNDLED_MAC + stored.replace("\\", "/")[i + len(_BUNDLED_WIN):]
    return None


@dataclass
class MigratePlan:
    source_avc: Path
    target_avc: Path
    mapping: dict = field(default_factory=dict)   # stored path -> new absolute path
    unmatched: list = field(default_factory=list)  # stored paths no entry claimed
    bundled: list = field(default_factory=list)    # Resolume's own media, handled by rule
    ambiguous: list = field(default_factory=list)  # (stored, score, [targets]) — tied
    skipped_class: list = field(default_factory=list)  # matched a row not being staged


def _norm(p: str) -> tuple:
    """Path -> reversed, lowercased components. Drives all matching."""
    s = p.replace("\\", "/").rstrip("/")
    return tuple(reversed([c for c in s.lower().split("/") if c and c != "."]))


def load_manifest(path: str | Path) -> list[Entry]:
    """Read the TSV. A header row is detected and skipped; Class is optional."""
    entries: list[Entry] = []
    with open(path, encoding="utf-8") as fh:
        for n, line in enumerate(fh):
            row = line.rstrip("\n").split("\t")
            if len(row) < 2:
                continue
            if n == 0 and row[0].strip().lower() == "source":
                continue
            src, tgt = row[0], row[1]
            cls = row[3] if len(row) > 3 else "stage"
            entries.append(Entry(source=src, target_rel=tgt.replace("\\", "/"),
                                 cls=cls, parts=_norm(src)))
    if not entries:
        raise ValueError(f"no usable rows in manifest: {path}")
    return entries


def best_match(stored: str, entries: list[Entry]) -> tuple[list[Entry], int]:
    """Entries sharing the longest trailing run with `stored`, and that run's length.

    Returns every entry tied at the top so the caller can report a genuine ambiguity
    rather than silently taking whichever sorted first.
    """
    want = _norm(stored)
    if not want:
        return [], 0
    best: list[Entry] = []
    best_n = 0
    for e in entries:
        n = 0
        for a, b in zip(want, e.parts):
            if a != b:
                break
            n += 1
        if n == 0:
            continue
        if n > best_n:
            best_n, best = n, [e]
        elif n == best_n:
            best.append(e)
    return best, best_n


def plan(source_avc: Path, stored_paths: list[str], entries: list[Entry],
         root: str, out_dir: Path) -> MigratePlan:
    """Map every stored path onto `root`, reporting anything that cannot be placed."""
    p = MigratePlan(source_avc=source_avc, target_avc=out_dir / source_avc.name)
    root = root.rstrip("/")

    for stored in stored_paths:
        if not stored:
            continue
        b = _bundled(stored)
        if b is not None:
            p.bundled.append(stored)
            if b:
                p.mapping[stored] = b
            continue

        hits, score = best_match(stored, entries)
        if not hits:
            p.unmatched.append(stored)
            continue
        # A tie on a single component is just a filename collision — not a match.
        distinct = {h.target_rel for h in hits}
        if len(distinct) > 1:
            p.ambiguous.append((stored, score, sorted(distinct)))
            continue
        hit = hits[0]
        if hit.cls != "stage":
            p.skipped_class.append(stored)
            continue
        p.mapping[stored] = f"{root}/{hit.target_rel}"
    return p


def apply(p: MigratePlan) -> int:
    """Write the rewritten composition. The stem is unchanged: this is the same
    composition on another machine, not a new version of it."""
    p.target_avc.parent.mkdir(parents=True, exist_ok=True)
    raw = p.source_avc.read_bytes()
    text, count = _rewrite(raw.decode("utf-8"), p.mapping,
                           p.source_avc.stem, p.source_avc.stem)
    p.target_avc.write_bytes(text.encode("utf-8"))
    return count
