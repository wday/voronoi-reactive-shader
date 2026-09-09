"""Output formatting — table and NDJSON."""

from __future__ import annotations

import json
from collections import OrderedDict


def format_output(result, query) -> str:
    if isinstance(result, dict) and result.get("kind") == "check":
        return _format_json(result, query) if query.json_output else _format_check(result)
    if isinstance(result, dict) and result.get("kind") == "replace":
        return _format_json(result, query) if query.json_output else _format_replace(result)
    if isinstance(result, dict) and result.get("kind") == "convert":
        return _format_json(result, query) if query.json_output else _format_convert(result)
    if query.json_output:
        return _format_json(result, query)
    if query.verb == "count":
        return _format_count(result)
    if query.verb == "group":
        return _format_grouped(result, query)
    return _format_list(result, query)


# -- table output -------------------------------------------------------------


def _format_list(records: list[dict], query) -> str:
    if not records:
        return "(no results)"
    cols = _pick_columns(records, query)
    return _render_table(records, cols)


def _format_grouped(groups: OrderedDict, query) -> str:
    if not groups:
        return "(no results)"
    parts = []
    cols = None
    for key, records in groups.items():
        parts.append(f"\n--- {key} ---")
        if cols is None:
            cols = _pick_columns(records, query, exclude={query.group_by})
        parts.append(_render_table(records, cols))
    return "\n".join(parts)


def _format_count(result) -> str:
    if isinstance(result, int):
        return str(result)
    if not result:
        return "(no results)"
    max_key = max(len(str(k)) for k, _ in result)
    return "\n".join(f"{str(k).ljust(max_key)}  {c}" for k, c in result)


def _render_table(records: list[dict], cols: list[str]) -> str:
    if not records:
        return "(no results)"

    # Format cell values
    rows = []
    for r in records:
        row = {}
        for c in cols:
            v = r.get(c)
            if v is None:
                row[c] = "-"
            elif c in ("size", "est_size") and isinstance(v, int):
                row[c] = human(v)
            elif isinstance(v, float):
                row[c] = f"{v:.2f}"
            else:
                row[c] = str(v)
        rows.append(row)

    # Column widths
    widths = {}
    for c in cols:
        widths[c] = max(len(c), max(len(rows[i][c]) for i in range(len(rows))))

    # Render
    header = "  ".join(c.upper().ljust(widths[c]) for c in cols)
    lines = [header]
    for row in rows:
        lines.append("  ".join(row[c].ljust(widths[c]) for c in cols))
    return "\n".join(lines)


def _pick_columns(records: list[dict], query, exclude: set | None = None) -> list[str]:
    exclude = exclude or set()

    if query.subject == "media":
        cols = ["name", "codec", "size_wh", "fps", "size", "uses", "verdict"]
    elif query.subject == "sources":
        cols = ["source_type", "source_name", "layer", "clip"]
    elif query.subject == "params":
        cols = ["effect_type", "name", "value", "osc", "layer", "clip"]
    else:
        cols = ["type", "blend", "opacity", "osc", "layer", "clip"]
        # Show name column only when it differs from type
        if any(r.get("name", "") != r.get("type", "") for r in records):
            cols.insert(1, "name")

    # Prepend file column when multiple files
    files = {r.get("file", "") for r in records}
    if len(files) > 1:
        cols.insert(0, "file")

    return [c for c in cols if c not in exclude]


# -- JSON output --------------------------------------------------------------


def _format_json(result, query) -> str:
    if isinstance(result, int):
        return json.dumps({"count": result})

    if isinstance(result, list):
        if result and isinstance(result[0], tuple):
            # count with group_by
            return "\n".join(json.dumps({"key": k, "count": c}) for k, c in result)
        return "\n".join(json.dumps(r) for r in result)

    if isinstance(result, OrderedDict):
        # grouped — flatten to NDJSON with group key
        lines = []
        for key, records in result.items():
            for r in records:
                lines.append(json.dumps({"_group": key, **r}))
        return "\n".join(lines)

    return json.dumps(result)


# -- media reports ------------------------------------------------------------


def human(n: float) -> str:
    for unit in ("B", "KB", "MB", "GB", "TB"):
        if abs(n) < 1024 or unit == "TB":
            return f"{n:.0f}{unit}" if unit == "B" else f"{n:.1f}{unit}"
        n /= 1024
    return f"{n:.1f}TB"


_HEADINGS = {
    "convert": "NEEDS CONVERSION",
    "converted": "ALREADY CONVERTED",
    "missing": "MISSING",
    "unreadable": "UNREADABLE",
    "optimal": "OPTIMAL",
}
_ORDER = ["convert", "missing", "unreadable", "converted", "optimal"]


def _format_check(result: dict) -> str:
    media = result["media"]
    t = result["totals"]
    lines = [result["origin"]]
    if result["files"]:
        lines.append(f"composition: {', '.join(result['files'])}")
    lines.append("")

    if not media:
        lines.append("no linked video files in this composition")
        return "\n".join(lines)

    for verdict in _ORDER:
        group = [m for m in media if m["verdict"] == verdict]
        if not group:
            continue
        lines.append(f"--- {_HEADINGS[verdict]} ({len(group)}) ---")
        for m in group:
            head = m["name"]
            if m["codec"]:
                head += f"  [{m['codec']} {m['size_wh']} @{m['fps']}fps, {human(m['size'])}]"
            lines.append(f"  {head}")
            lines.append(f"    used by {m['uses']} clip(s): {', '.join(m['clips'])}")
            if m["reason"]:
                lines.append(f"    {m['reason']}")
            if verdict == "convert":
                lines.append(f"    -> {m['out_path']}  (est. <= {human(m['est_size'])})")
        lines.append("")

    lines.append(
        f"{t['convert']} of {t['total']} file(s) need conversion: "
        f"{human(t['source_bytes'])} of source -> up to {human(t['est_output_bytes'])} of HAP"
    )
    if t["convert"]:
        lines.append("HAP is bigger than what it replaces; that is the trade for GPU decoding.")
        lines.append("run 'avc convert media' to encode, or add --dry-run to see the commands")
    return "\n".join(lines)


def _format_convert(result: dict) -> str:
    if result.get("skipped") and not result.get("results"):
        return ""  # dry-run already printed the commands
    if not result.get("results"):
        return "nothing to convert"
    parts = [f"converted {result['converted']}, failed {result['failed']}"]
    if result["failed"]:
        parts.append("re-run with --dry-run to inspect the failing commands")
    return "\n".join(parts)


def _format_replace(r: dict) -> str:
    from pathlib import Path

    lines = [r["origin"]]
    if r.get("warnings"):
        lines += ["", *r["warnings"]]
    lines += ["", f"source: {Path(r['source']).name}"]

    if r.get("blocked"):
        lines.append("")
        lines.append(f"{len(r['needs'])} file(s) have no HAP sibling yet:")
        for n in r["needs"]:
            lines.append(f"  {n}")
        lines.append("")
        lines.append("nothing was written. re-run with --convert to encode them first,")
        lines.append("or run 'avc convert media' and then 'avc replace'.")
        return "\n".join(lines)

    lines.append(f"target: {Path(r['target']).name}")
    if r.get("converted"):
        lines.append(f"converted {r['converted']} file(s) first")

    mapping = r.get("mapping") or {}
    if mapping:
        lines.append("")
        lines.append(f"relinking {len(mapping)} file(s):")
        for old, new in mapping.items():
            lines.append(f"  {Path(old.replace(chr(92), '/')).name}")
            lines.append(f"      -> {new}")

    if r.get("unresolvable"):
        lines.append("")
        lines.append("left pointing at the original (missing or unreadable):")
        for n in r["unresolvable"]:
            lines.append(f"  {n}")

    lines.append("")
    if r.get("dry_run"):
        lines.append("--dry-run: nothing written")
    else:
        lines.append(f"wrote {r['target']}")
        lines.append(f"{r['replacements']} path reference(s) rewritten; the original is untouched")
    return "\n".join(lines)
