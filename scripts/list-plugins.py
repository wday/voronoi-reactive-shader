#!/usr/bin/env python3
"""List plugins from plugins.json in a format consumable by shell scripts.

Usage:
  list-plugins.py [--type TYPE] [--format FORMAT]

Options:
  --type isf|rust    Filter by plugin type
  --format build     Output: name|crate_or_shader|dll
  --format package   Output: name|dll|source_path (Windows paths)
  --platform macos   Use macOS paths (lib prefix, .dylib extension)

Output is pipe-delimited, one plugin per line, for use with:
  script | while IFS='|' read -r NAME ...; do ... done
"""

import json
import sys
import os
import platform

def main():
    args = sys.argv[1:]
    plugin_type = None
    fmt = "build"
    plat = "windows"

    i = 0
    while i < len(args):
        if args[i] == "--type" and i + 1 < len(args):
            plugin_type = args[i + 1]
            i += 2
        elif args[i] == "--format" and i + 1 < len(args):
            fmt = args[i + 1]
            i += 2
        elif args[i] == "--platform" and i + 1 < len(args):
            plat = args[i + 1]
            i += 2
        else:
            i += 1

    # Find plugins.json relative to this script
    script_dir = os.path.dirname(os.path.abspath(__file__))
    registry = os.path.join(script_dir, "..", "plugins.json")

    with open(registry) as f:
        plugins = json.load(f)["plugins"]

    if plugin_type:
        plugins = [p for p in plugins if p["type"] == plugin_type]

    for p in plugins:
        if fmt == "build":
            if p["type"] == "isf":
                print(f"{p['name']}|{p['shader']}|{p['dll']}")
            elif p["type"] == "rust":
                print(f"{p['name']}|{p['crate']}|{p['dll']}")

        elif fmt == "package":
            dll = p["dll"]
            bundle = dll.replace(".dll", "")
            if p["type"] == "isf":
                if plat == "macos":
                    src = "ffgl-rs/target/release/libffgl_isf.dylib"
                else:
                    src = "ffgl-rs/target/release/ffgl_isf.dll"
            elif p["type"] == "rust":
                if plat == "macos":
                    src = f"plugins/target/release/lib{bundle}.dylib"
                else:
                    src = f"plugins/target/release/{dll}"
            print(f"{p['name']}|{bundle}|{dll}|{src}")

if __name__ == "__main__":
    main()
