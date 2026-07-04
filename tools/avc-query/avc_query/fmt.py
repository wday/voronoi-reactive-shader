"""Output formatting — table and NDJSON."""

from __future__ import annotations

import json
from collections import OrderedDict


def format_output(result, query) -> str:
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

    if query.subject == "sources":
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
