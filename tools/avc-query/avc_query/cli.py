"""avc-query CLI entry point."""

from __future__ import annotations

import os
import sys

from .fmt import format_output
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
        query = parse_query(args)
        result = execute(query, COMPOSITIONS_DIR)
        output = format_output(result, query)
        if output:
            print(output)
    except (ValueError, FileNotFoundError) as e:
        print(f"error: {e}", file=sys.stderr)
        sys.exit(1)
    except IndexError:
        print("error: incomplete query — missing argument", file=sys.stderr)
        sys.exit(1)


def _help():
    print("""\
avc-query — query Resolume .avc composition files

Usage: avc <verb> <subject> [clauses...]

Verbs:    list, group, count
Subjects: blocks (effects), sources (clip inputs), params (OSC-addressable parameters)

Clauses:
  like <pattern>              fuzzy match on type/name
  by <field>                  group by field
  with <param> <op> <value>   filter (op: < > = != <= >= like)
  order by <param>            sort by param value
  where <subject> <clause>    cross-cut filter
  in <glob>                   scope to files (default: most recent)

Flags:
  --json                      NDJSON output

Examples:
  avc list blocks
  avc group blocks by blend like lo
  avc list blocks with opacity < 0.5 order by opacity
  avc list sources where blocks like lo
  avc count blocks by type in "geiger*"
  avc list params                         OSC-addressable parameters
  avc list params like blur               params for blur effects
  avc list params --json                  NDJSON for piping to tools
""")


if __name__ == "__main__":
    main()
