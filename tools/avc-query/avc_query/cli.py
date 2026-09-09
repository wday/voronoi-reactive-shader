"""avc-query CLI entry point."""

from __future__ import annotations

import os
import sys

from . import resolume
from .fmt import format_output
from .media import VARIANTS, ConvertOpts
from .query import execute, parse_query

COMPOSITIONS_DIR = os.environ.get(
    "AVC_COMPOSITIONS_DIR",
    "/mnt/c/Users/alien/Documents/Resolume Avenue/Compositions",
)


def main():
    args = sys.argv[1:]

    if not args or args[0] in ("--help", "-h", "help"):
        _help()
        return

    try:
        args, opts, live, api_base = _split_flags(args)
        query = parse_query(args)
        query.opts = opts
        query.live = live
        query.api_base = api_base
        result = execute(query, COMPOSITIONS_DIR)
        output = format_output(result, query)
        if output:
            print(output)
    except (ValueError, FileNotFoundError, RuntimeError) as e:
        print(f"error: {e}", file=sys.stderr)
        sys.exit(1)
    except IndexError:
        print("error: incomplete query — missing argument", file=sys.stderr)
        sys.exit(1)


def _split_flags(args: list[str]) -> tuple[list[str], ConvertOpts, bool, str]:
    """Pull media flags out of the arg list; the rest is the query."""
    opts = ConvertOpts()
    live = True
    api_base = resolume.DEFAULT_BASE
    rest: list[str] = []

    i = 0
    while i < len(args):
        a = args[i]
        if a == "--dry-run":
            opts.dry_run = True
        elif a == "--convert":
            opts.convert = True
        elif a == "--force":
            opts.force = True
        elif a == "--no-live":
            live = False
        elif a == "--live":
            live = True
        elif a == "--variant":
            opts.variant = _need(args, i + 1, "--variant")
            if opts.variant not in VARIANTS:
                raise ValueError(f"--variant must be one of: {', '.join(sorted(VARIANTS))}")
            i += 1
        elif a == "--fit":
            opts.fit = _need(args, i + 1, "--fit")
            if opts.fit not in ("crop", "pad"):
                raise ValueError("--fit must be crop or pad")
            i += 1
        elif a == "--out-dir":
            opts.out_dir = _need(args, i + 1, "--out-dir")
            i += 1
        elif a == "--jobs":
            opts.jobs = _int(_need(args, i + 1, "--jobs"), "--jobs")
            i += 1
        elif a == "--chunks":
            opts.chunks = _int(_need(args, i + 1, "--chunks"), "--chunks")
            if not 1 <= opts.chunks <= 64:
                raise ValueError("--chunks must be between 1 and 64")
            i += 1
        elif a == "--api":
            api_base = _need(args, i + 1, "--api")
            i += 1
        else:
            rest.append(a)
        i += 1

    return rest, opts, live, api_base


def _need(args: list[str], i: int, flag: str) -> str:
    if i >= len(args):
        raise ValueError(f"{flag} needs a value")
    return args[i]


def _int(v: str, flag: str) -> int:
    try:
        return int(v)
    except ValueError:
        raise ValueError(f"{flag} needs a number, got '{v}'") from None


def _help():
    print("""\
avc-query — query Resolume .avc composition files

Usage: avc <verb> <subject> [clauses...]

Verbs:    list, group, count, check, convert, replace
Subjects: blocks (effects), sources (clip inputs), params (OSC-addressable parameters),
          media (linked video files)

Clauses:
  like <pattern>              fuzzy match on type/name
  by <field>                  group by field
  with <param> <op> <value>   filter (op: < > = != <= >= like)
  order by <param>            sort by param value
  where <subject> <clause>    cross-cut filter
  in <glob>                   scope to files (default: the composition Resolume has loaded,
                              else the most recently modified .avc)

Flags:
  --json                      NDJSON output
  --no-live                   don't ask Resolume which composition is loaded
  --api <url>                 REST API base (default http://localhost:8080/api/v1)

Media flags (check / convert):
  --dry-run                   print the ffmpeg commands, run nothing
  --variant hap_q|hap|hap_alpha   HAP flavour (default hap_q; alpha sources force hap_alpha)
  --chunks N                  HAP chunk count 1-64 (default: by resolution)
  --fit crop|pad              reach a multiple of 4 (default crop — pad leaves a black seam)
  --out-dir DIR               all output in one directory (default '<source dir>/hap')
  --jobs N                    parallel encodes (default 2)
  --force                     re-encode even if an up-to-date output exists

Replace flags:
  --convert                   encode any missing HAP siblings before relinking
  --dry-run                   show what would be relinked, write nothing

Examples:
  avc list blocks
  avc group blocks by blend like lo
  avc list blocks with opacity < 0.5 order by opacity
  avc list sources where blocks like lo
  avc count blocks by type in "geiger*"
  avc list params like blur               params for blur effects

  avc check                               audit the loaded set for stutter-free playback
  avc list media                          linked files, codec and verdict
  avc convert media --dry-run             show the ffmpeg commands
  avc convert media --jobs 4              encode to HAP Q beside each source
  avc replace                             write a version-bumped .avc linked to the HAP files
  avc replace --convert                   ...encoding anything still missing first
  avc replace --dry-run                   show the relink plan, write nothing

Playback note: Resolume decodes DXV3 and HAP on the GPU; everything else (H.264, HEVC,
ProRes, DNxHD) is decoded on the CPU and is the usual cause of stutter. DXV3 has no
third-party encoder, so this tool targets HAP. Expect HAP files to be LARGER than the
sources they replace — that is the trade for GPU decoding.
""")


if __name__ == "__main__":
    main()
