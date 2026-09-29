"""Translate between Windows paths (as stored in .avc) and WSL mount paths."""

from __future__ import annotations

import re

_DRIVE = re.compile(r"^([A-Za-z]):[\\/](.*)$", re.DOTALL)
_MNT = re.compile(r"^/mnt/([a-z])/(.*)$", re.DOTALL)


def to_wsl(win_path: str) -> str | None:
    """'D:\\a\\b.mov' -> '/mnt/d/a/b.mov'. None for UNC or unrecognised forms."""
    if not win_path:
        return None
    if win_path.startswith("\\\\") or win_path.startswith("//"):
        return None  # UNC share — no deterministic mount point
    if win_path.startswith("/"):
        return win_path  # already POSIX
    m = _DRIVE.match(win_path)
    if not m:
        return None
    drive, rest = m.group(1).lower(), m.group(2).replace("\\", "/")
    return f"/mnt/{drive}/{rest}"


def to_win(wsl_path: str) -> str:
    """'/mnt/d/a/b.mov' -> 'D:\\a\\b.mov'. Passes anything else through unchanged."""
    m = _MNT.match(wsl_path)
    if not m:
        return wsl_path
    drive, rest = m.group(1).upper(), m.group(2).replace("/", "\\")
    return f"{drive}:\\{rest}"
