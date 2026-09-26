"""Query parser and execution engine."""

from __future__ import annotations

import fnmatch
from collections import OrderedDict
from dataclasses import dataclass, field
from pathlib import Path

from . import media as media_mod
from . import migrate as migrate_mod
from . import relink as relink_mod
from . import resolume
from .media import ConvertOpts, Media
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
    live: bool = True                     # resolve the loaded composition via the API
    api_base: str = resolume.DEFAULT_BASE
    opts: ConvertOpts = field(default_factory=ConvertOpts)


# -- query parser -------------------------------------------------------------

VERBS = {"list", "group", "count", "check", "convert", "replace", "relink", "migrate"}
SUBJECTS = {"blocks", "sources", "params", "media",
            "block", "source", "param"}
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

    if len(args) < 1:
        raise ValueError("Usage: avc <verb> <subject> [clauses...]")
    if len(args) < 2 and args[0].lower() not in ("check", "convert", "replace", "relink", "migrate"):
        raise ValueError("Usage: avc <verb> <subject> [clauses...]")

    verb = args[0].lower()
    if verb == "relink":
        verb = "replace"
    if verb not in VERBS:
        raise ValueError(
            f"Unknown verb: {verb} "
            "(expected: list, group, count, check, convert, replace, migrate)")

    # 'check' and 'convert' only ever act on media, so the subject may be left off.
    if verb in ("check", "convert", "replace", "relink", "migrate") and (
            len(args) < 2 or args[1].lower() not in SUBJECTS):
        args = [args[0], "media"] + args[1:]

    subject = _norm_subject(args[1])
    if verb in ("check", "convert", "replace", "relink", "migrate") and subject != "media":
        raise ValueError(f"'{verb}' only applies to media, not {subject}")
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
    if s in ("media", "medium", "files", "file"):
        return "media"
    raise ValueError(f"Unknown subject: {s} (expected: blocks, sources, params, media)")


# -- execution ----------------------------------------------------------------


def execute(query: Query, compositions_dir: str) -> list | dict | int:
    files, origin = resolve_scope(query, compositions_dir)

    all_blocks: list[Block] = []
    all_sources: list[Source] = []
    for f in files:
        blocks, sources = parse_avc(f)
        all_blocks.extend(blocks)
        all_sources.extend(sources)

    if query.verb == "migrate":
        return _execute_migrate(query, files, origin)

    if query.subject == "media":
        if query.verb == "replace":
            return _execute_replace(query, all_sources, files, origin)
        live_sources, origin = _live_sources(query, origin)
        return _execute_media(query, live_sources or all_sources, files, origin)

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


def _live_sources(query: Query, origin: str) -> tuple[list | None, str]:
    """Prefer the live composition's own file list over the .avc on disk.

    Resolume exposes layers[].clips[].video.fileinfo.path, which is the truth including
    clips added since the last save. Falls back to the parsed .avc when unreachable.
    """
    if query.file_glob or not query.live:
        return None, origin

    srcs = resolume.file_sources(query.api_base)
    if srcs is None:
        return None, origin

    name = resolume.composition_name(query.api_base) or "?"
    base = resolume.discover_base(query.api_base) or "?"
    n_connected = sum(1 for s in srcs if s.connected)
    return srcs, (f"live: '{name}' as loaded in Resolume ({base}) — "
                  f"{len(srcs)} file clip(s), {n_connected} connected")


def _execute_migrate(query: Query, files: list[Path], origin: str):
    """Rewrite each composition's stored media paths onto another machine's filesystem.

    Reads paths straight out of the .avc rather than going through media.collect(), which
    probes every file with ffprobe: the destination media need not exist here, and on a
    machine where it does exist probing 300+ files would cost minutes for data this never
    uses. It also never consults the live API — Resolume is not expected to be running,
    and on the source machine it must not be, or it may relink clips to the staging drive.
    """
    if not query.opts.manifest:
        raise ValueError("migrate needs --manifest <path>")
    if not query.opts.root:
        raise ValueError("migrate needs --root <media root on the destination machine>")

    out_dir = Path(query.opts.out_dir) if query.opts.out_dir else Path.cwd() / "migrated"
    entries = migrate_mod.load_manifest(query.opts.manifest)

    results = []
    for f in files:
        _blocks, srcs = parse_avc(f)
        stored = []
        for s in srcs:
            if getattr(s, "source_type", "") != "file":
                continue
            raw = getattr(s, "file_path", "") or getattr(s, "source_name", "")
            if raw and raw not in stored:
                stored.append(raw)
        p = migrate_mod.plan(f, stored, entries, query.opts.root, out_dir)
        written = 0 if query.opts.dry_run else migrate_mod.apply(p)
        results.append({
            "composition": f.stem,
            "refs": len(stored),
            "mapped": len(p.mapping),
            "unmatched": p.unmatched,
            "bundled": p.bundled,
            "ambiguous": p.ambiguous,
            "skipped_class": p.skipped_class,
            "replacements": written,
            "target": str(p.target_avc),
        })
    return {"kind": "migrate", "origin": origin, "dry_run": query.opts.dry_run,
            "root": query.opts.root, "out_dir": str(out_dir), "results": results}


def _execute_replace(query: Query, sources: list[Source], files: list[Path], origin: str):
    """Write a version-bumped copy of the .avc with its media relinked to HAP.

    This rewrites a file, so it works from the .avc on disk rather than the live
    composition — but it still checks the live one, because unsaved changes would be
    silently dropped from the copy.
    """
    if len(files) != 1:
        raise ValueError(
            f"replace needs exactly one composition, got {len(files)} — narrow it with 'in <glob>'")
    source_avc = files[0]

    records = media_mod.collect(sources, query.opts)
    if query.like:
        records = [r for r in records if _like_match(r, query.like)]

    warnings = _unsaved_warning(query, records, source_avc)
    p = relink_mod.plan(source_avc, records, query.opts)

    if p.needs_conversion and not query.opts.convert:
        return {
            "kind": "replace", "blocked": True, "origin": origin, "warnings": warnings,
            "source": str(source_avc), "target": str(p.target_avc),
            "needs": [m.name for m in p.needs_conversion],
            "unresolvable": [m.name for m in p.unresolvable],
            "relinked": len(p.mapping),
        }

    converted = 0
    if p.needs_conversion and query.opts.convert:
        summary = media_mod.convert_all(p.needs_conversion, query.opts)
        if summary["failed"]:
            raise RuntimeError(
                f"{summary['failed']} conversion(s) failed — nothing was written")
        converted = summary["converted"]
        p = relink_mod.plan(source_avc, records, query.opts)
        if p.needs_conversion:
            raise RuntimeError("conversion reported success but outputs are still missing")

    if query.opts.dry_run:
        return {
            "kind": "replace", "dry_run": True, "origin": origin, "warnings": warnings,
            "source": str(source_avc), "target": str(p.target_avc),
            "mapping": dict(p.mapping), "converted": converted,
            "unresolvable": [m.name for m in p.unresolvable],
        }

    if p.target_avc.exists():   # next_free already avoids this; belt and braces
        raise RuntimeError(f"refusing to overwrite {p.target_avc}")
    replacements = relink_mod.apply(p)

    return {
        "kind": "replace", "origin": origin, "warnings": warnings,
        "source": str(source_avc), "target": str(p.target_avc),
        "mapping": dict(p.mapping), "replacements": replacements,
        "converted": converted,
        "unresolvable": [m.name for m in p.unresolvable],
    }


def _unsaved_warning(query: Query, records: list, source_avc: Path) -> list:
    """Compare the file on disk against what Resolume actually has loaded.

    Only meaningful when the target IS the loaded composition; comparing a different
    one via 'in <glob>' would report every difference as unsaved work.
    """
    if not query.live:
        return []
    name = resolume.composition_name(query.api_base)
    if not name or name != source_avc.stem:
        return []
    live = resolume.file_sources(query.api_base)
    if live is None:
        return []
    live_paths = {s.file_path.lower() for s in live}
    disk_paths = {m.file_path.lower() for m in records}
    only_live = live_paths - disk_paths
    only_disk = disk_paths - live_paths
    if not (only_live or only_disk):
        return []
    msg = ["Resolume's loaded composition does not match the file on disk — save it first."]
    for p in sorted(only_live):
        msg.append(f"  only in Resolume: {p}")
    for p in sorted(only_disk):
        msg.append(f"  only on disk:     {p}")
    return msg


def _execute_media(query: Query, sources: list[Source], files: list[Path], origin: str):
    records = media_mod.collect(sources, query.opts)

    if query.like:
        records = [r for r in records if _like_match(r, query.like)]
    for param, op, val in query.filters:
        records = [r for r in records if _with_filter(r, param, op, val)]

    records.sort(key=lambda m: (_VERDICT_ORDER.get(m.verdict, 9), m.name.lower()))

    if query.verb == "convert":
        summary = media_mod.convert_all(records, query.opts)
        summary["kind"] = "convert"
        summary["origin"] = origin
        return summary

    dicts = [_media_dict(m) for m in records]

    if query.order_by:
        dicts.sort(key=lambda d: _sort_key(d, query.order_by))

    if query.verb == "group":
        return _group(dicts, query.group_by or "verdict")
    if query.verb == "count":
        return _count(dicts, query.group_by)
    if query.verb == "check":
        return {
            "kind": "check",
            "origin": origin,
            "files": [] if origin.startswith("live:") else [f.name for f in files],
            "media": dicts,
            "totals": _media_totals(records, query.opts),
        }
    return dicts


_VERDICT_ORDER = {"convert": 0, "missing": 1, "unreadable": 2, "converted": 3, "optimal": 4}


def _media_totals(records: list[Media], opts: ConvertOpts) -> dict:
    todo = [m for m in records if m.verdict == "convert"]
    return {
        "total": len(records),
        "convert": len(todo),
        "optimal": sum(1 for m in records if m.verdict == "optimal"),
        "converted": sum(1 for m in records if m.verdict == "converted"),
        "missing": sum(1 for m in records if m.verdict == "missing"),
        "unreadable": sum(1 for m in records if m.verdict == "unreadable"),
        "source_bytes": sum(m.size for m in todo),
        "est_output_bytes": sum(m.est_size for m in todo),
    }


def _media_dict(m: Media) -> dict:
    return {
        "name": m.name,
        "verdict": m.verdict,
        "reason": m.reason,
        "codec": m.codec,
        "codec_tag": m.codec_tag,
        "width": m.width,
        "height": m.height,
        "size_wh": f"{m.width}x{m.height}" if m.width else "",
        "fps": round(m.avg_fps or m.fps, 3),
        "vfr": m.vfr,
        "alpha": m.alpha,
        "audio": m.audio,
        "duration": round(m.duration, 2),
        "size": m.size,
        "est_size": m.est_size,
        "uses": m.uses,
        "clips": list(m.clips),
        "file_path": m.file_path,
        "out_path": m.out_path,
        "file": m.file,
    }


# -- file resolution ----------------------------------------------------------


def resolve_scope(query: Query, compositions_dir: str) -> tuple[list[Path], str]:
    """Pick the .avc files to read, preferring the composition Resolume has loaded.

    The API is asked for identity only; paths always come off the .avc on disk, so
    clips added since the last save are invisible. The origin string says which way
    it went, and the caller shows it.
    """
    if query.file_glob:
        return resolve_files(compositions_dir, query.file_glob), f"disk: matching '{query.file_glob}'"
    if not query.live:
        return resolve_files(compositions_dir, None), "disk: most recently modified .avc (--no-live)"

    name = resolume.composition_name(query.api_base)
    if name:
        candidate = Path(compositions_dir) / f"{name}.avc"
        if candidate.is_file():
            return [candidate], f"live: Resolume has '{name}' loaded"
        matched = resolve_files(compositions_dir, name)
        if matched:
            return matched[:1], f"live: Resolume has '{name}' loaded"
        return (resolve_files(compositions_dir, None),
                f"live: Resolume has '{name}' loaded but no such .avc on disk — "
                f"falling back to the most recent")

    return (resolve_files(compositions_dir, None),
            "offline: Resolume API not reachable — using the most recently modified .avc")


# -- legacy file resolution ---------------------------------------------------


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
    elif isinstance(record, Media):
        fields = [record.name, record.codec, record.verdict, record.file_path,
                  " ".join(record.clips)]
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
                 "effect_type", "effect_name", "value",
                 "verdict", "codec", "width", "height", "fps", "duration", "size",
                 "uses", "alpha", "audio", "vfr"):
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
