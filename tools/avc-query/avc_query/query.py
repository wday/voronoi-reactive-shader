"""Query parser and execution engine."""

from __future__ import annotations

import fnmatch
from collections import OrderedDict
from dataclasses import dataclass, field
from pathlib import Path

from .parse import Block, Param, Source, expand_params, parse_avc


# -- query model --------------------------------------------------------------


@dataclass
class WhereClause:
    subject: str
    like: str | None = None
    filters: list = field(default_factory=list)  # [(param, op, value), ...]


@dataclass
class Query:
    verb: str  # list, group, count
    subject: str  # blocks, sources
    like: str | None = None
    group_by: str | None = None
    filters: list = field(default_factory=list)
    order_by: str | None = None
    where: WhereClause | None = None
    file_glob: str | None = None
    json_output: bool = False


# -- query parser -------------------------------------------------------------

VERBS = {"list", "group", "count"}
SUBJECTS = {"blocks", "sources", "params", "block", "source", "param"}
KEYWORDS = {"like", "by", "with", "order", "where", "in", "--json"}
OPS = {"<", ">", "=", "!=", "<=", ">=", "like"}

FIELD_ALIASES = {
    "blend_mode": "blend",
    "blendmode": "blend",
    "source_type": "source_type",
    "sourcetype": "source_type",
    "source_name": "source_name",
    "sourcename": "source_name",
}


def parse_query(args: list[str]) -> Query:
    json_output = "--json" in args
    args = [a for a in args if a != "--json"]

    if len(args) < 2:
        raise ValueError("Usage: avc <verb> <subject> [clauses...]")

    verb = args[0].lower()
    if verb not in VERBS:
        raise ValueError(f"Unknown verb: {verb} (expected: list, group, count)")

    subject = _norm_subject(args[1])
    q = Query(verb=verb, subject=subject, json_output=json_output)

    i = 2
    while i < len(args):
        tok = args[i].lower()

        if tok == "like":
            q.like = args[i + 1]
            i += 2

        elif tok == "by":
            field, i = _read_field(args, i + 1)
            q.group_by = field

        elif tok == "with":
            field, i = _read_field(args, i + 1)
            op = args[i].lower()
            val = args[i + 1]
            q.filters.append((field, op, val))
            i += 2

        elif tok == "order":
            if i + 1 < len(args) and args[i + 1].lower() == "by":
                field, i = _read_field(args, i + 2)
                q.order_by = field
            else:
                raise ValueError("Expected 'order by <field>'")

        elif tok == "where":
            w_subject = _norm_subject(args[i + 1])
            w = WhereClause(subject=w_subject)
            i += 2
            # Parse clauses for where until we hit 'in' or end
            while i < len(args):
                wt = args[i].lower()
                if wt == "like":
                    w.like = args[i + 1]
                    i += 2
                elif wt == "with":
                    field, i = _read_field(args, i + 1)
                    op = args[i].lower()
                    val = args[i + 1]
                    w.filters.append((field, op, val))
                    i += 2
                else:
                    break  # hand back to outer parser
            q.where = w

        elif tok == "in":
            q.file_glob = args[i + 1]
            i += 2

        else:
            raise ValueError(f"Unexpected token: {args[i]}")

    return q


def _read_field(args: list[str], i: int) -> tuple[str, int]:
    """Read a field name, possibly multi-word (e.g. 'blend mode')."""
    stop = KEYWORDS | OPS
    parts = [args[i]]
    j = i + 1
    while j < len(args) and args[j].lower() not in stop:
        parts.append(args[j])
        j += 1
    raw = "_".join(p.lower() for p in parts)
    return FIELD_ALIASES.get(raw, raw), j


def _norm_subject(s: str) -> str:
    s = s.lower()
    if s in ("block", "blocks"):
        return "blocks"
    if s in ("source", "sources"):
        return "sources"
    if s in ("param", "params"):
        return "params"
    raise ValueError(f"Unknown subject: {s} (expected: blocks, sources, params)")


# -- execution ----------------------------------------------------------------


def execute(query: Query, compositions_dir: str) -> list | dict | int:
    files = resolve_files(compositions_dir, query.file_glob)

    all_blocks: list[Block] = []
    all_sources: list[Source] = []
    for f in files:
        blocks, sources = parse_avc(f)
        all_blocks.extend(blocks)
        all_sources.extend(sources)

    # Select primary records
    if query.subject == "blocks":
        records = list(all_blocks)
    elif query.subject == "params":
        records = expand_params(all_blocks)
    else:
        records = list(all_sources)

    # Cross-cut: where clause
    if query.where:
        w = query.where
        pool = list(all_blocks) if w.subject == "blocks" else list(all_sources)

        if w.like:
            pool = [r for r in pool if _like_match(r, w.like)]
        for param, op, val in w.filters:
            pool = [r for r in pool if _with_filter(r, param, op, val)]

        match_locs = {_loc_key(r) for r in pool}
        records = [r for r in records if _loc_matches(r, match_locs)]

    # Filter: like
    if query.like:
        records = [r for r in records if _like_match(r, query.like)]

    # Filter: with
    for param, op, val in query.filters:
        records = [r for r in records if _with_filter(r, param, op, val)]

    # Convert to dicts
    dicts = [_to_dict(r) for r in records]

    # Sort
    if query.order_by:
        dicts.sort(key=lambda d: _sort_key(d, query.order_by))

    # Verb
    if query.verb == "group":
        return _group(dicts, query.group_by)
    elif query.verb == "count":
        return _count(dicts, query.group_by)
    return dicts


# -- file resolution ----------------------------------------------------------


def resolve_files(compositions_dir: str, file_glob: str | None = None) -> list[Path]:
    comp_dir = Path(compositions_dir)
    if not comp_dir.exists():
        raise FileNotFoundError(f"Compositions directory not found: {comp_dir}")

    all_avcs = sorted(comp_dir.glob("*.avc"), key=lambda p: p.stat().st_mtime, reverse=True)
    if not all_avcs:
        raise FileNotFoundError(f"No .avc files in {comp_dir}")

    if file_glob is None:
        return [all_avcs[0]]

    matched = [
        p for p in all_avcs
        if fnmatch.fnmatch(p.stem, file_glob) or fnmatch.fnmatch(p.name, file_glob)
    ]
    if not matched:
        raise FileNotFoundError(f"No .avc files matching '{file_glob}'")
    return matched


# -- matching helpers ---------------------------------------------------------


def _like_match(record, pattern: str) -> bool:
    pat = pattern.lower()
    if isinstance(record, Block):
        fields = [record.type, record.name, record.blend or ""]
    elif isinstance(record, Param):
        fields = [record.name, record.effect_type, record.effect_name, record.osc]
    elif isinstance(record, Source):
        fields = [record.source_type, record.source_name, record.clip or ""]
    else:
        return False
    return any(pat in f.lower() for f in fields)


def _with_filter(record, param: str, op: str, val: str) -> bool:
    actual = _get_record_field(record, param)
    if actual is None:
        return False

    if op == "like":
        return val.lower() in str(actual).lower()

    try:
        val_f = float(val)
        actual_f = float(actual)
    except (ValueError, TypeError):
        # String comparison for = and !=
        if op == "=":
            return str(actual).lower() == val.lower()
        if op == "!=":
            return str(actual).lower() != val.lower()
        return False

    ops = {
        "<": actual_f < val_f,
        ">": actual_f > val_f,
        "=": actual_f == val_f,
        "!=": actual_f != val_f,
        "<=": actual_f <= val_f,
        ">=": actual_f >= val_f,
    }
    return ops.get(op, False)


def _get_record_field(record, field_name: str):
    fn = field_name.lower()
    # Check dataclass fields
    for attr in ("type", "name", "blend", "opacity", "scope", "layer", "clip", "col",
                 "file", "source_type", "source_name", "osc", "effect_index",
                 "effect_type", "effect_name", "value"):
        if attr.lower() == fn and hasattr(record, attr):
            return getattr(record, attr)
    # Check params dict
    if hasattr(record, "params"):
        for k, v in record.params.items():
            if k.lower() == fn:
                return v
    return None


# -- location cross-cut -------------------------------------------------------


def _loc_key(record) -> tuple:
    return (record.file, record.layer, record.col, record.scope)


def _loc_matches(record, match_locs: set) -> bool:
    for mf, ml, mc, ms in match_locs:
        if record.file != mf:
            continue
        if ms == "comp":
            return True
        if ms == "layer" and record.layer == ml:
            return True
        if ms == "clip" and record.layer == ml and record.col == mc:
            return True
    return False


# -- dict conversion ----------------------------------------------------------


def _to_dict(record) -> dict:
    if isinstance(record, Block):
        return {
            "type": record.type,
            "name": record.name,
            "blend": record.blend or "",
            "opacity": record.opacity,
            "scope": record.scope,
            "layer": record.layer,
            "clip": record.clip or "",
            "col": record.col,
            "effect_index": record.effect_index,
            "osc": record.osc,
            "file": record.file,
            "params": dict(record.params),
        }
    elif isinstance(record, Param):
        return {
            "name": record.name,
            "value": record.value,
            "osc": record.osc,
            "effect_type": record.effect_type,
            "effect_name": record.effect_name,
            "scope": record.scope,
            "layer": record.layer,
            "clip": record.clip or "",
            "file": record.file,
        }
    elif isinstance(record, Source):
        return {
            "source_type": record.source_type,
            "source_name": record.source_name,
            "scope": record.scope,
            "layer": record.layer,
            "clip": record.clip or "",
            "col": record.col,
            "file": record.file,
        }
    return {}


# -- grouping / counting ------------------------------------------------------


def _get_field(d: dict, field_name: str):
    fn = field_name.lower()
    for k, v in d.items():
        if k.lower() == fn and k != "params":
            return v
    for k, v in d.get("params", {}).items():
        if k.lower() == fn:
            return v
    return None


def _sort_key(d: dict, field_name: str):
    v = _get_field(d, field_name)
    if v is None:
        return (1, 0, "")
    try:
        return (0, float(v), "")
    except (ValueError, TypeError):
        return (0, 0, str(v))


def _group(records: list[dict], group_by: str | None) -> OrderedDict:
    if not group_by:
        return OrderedDict([("all", records)])
    groups: OrderedDict = OrderedDict()
    for r in records:
        key = _get_field(r, group_by)
        key = str(key) if key is not None else "-"
        groups.setdefault(key, []).append(r)
    return groups


def _count(records: list[dict], group_by: str | None) -> list | int:
    if not group_by:
        return len(records)
    from collections import Counter
    counts: Counter = Counter()
    for r in records:
        key = _get_field(r, group_by)
        key = str(key) if key is not None else "-"
        counts[key] += 1
    return sorted(counts.items(), key=lambda x: -x[1])
