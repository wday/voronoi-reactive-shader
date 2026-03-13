#!/usr/bin/env python3
"""Resolume offset calculator.

Coordinate system: origin at composition center, X+ = right, Y+ = up (standard GL).
Offsets are in pixels.

Usage:
    python tools/resolume-calc.py grid 0.25              # all 16 positions at scale 0.25
    python tools/resolume-calc.py corner 0.25 top-left   # single corner
    python tools/resolume-calc.py offset 0.25 2 3        # grid position (col, row) from top-left
    python tools/resolume-calc.py tile 0.5                # 2x2 grid positions
"""

import argparse
import sys

COMP_W = 1920
COMP_H = 1080


def offset_for_cell(scale: float, col: int, row: int) -> tuple[float, float]:
    """Pixel offset for a grid cell (col, row) where (0,0) is top-left."""
    cell_w = COMP_W * scale
    cell_h = COMP_H * scale
    # Center of the cell in pixel coords (origin top-left)
    px = col * cell_w + cell_w / 2
    py = row * cell_h + cell_h / 2
    # Convert to GL coords (origin center, Y-up)
    ox = px - COMP_W / 2
    oy = -(py - COMP_H / 2)
    return ox, oy


def grid_size(scale: float) -> tuple[int, int]:
    cols = round(1 / scale)
    rows = round(1 / scale * COMP_W / COMP_H)
    # Simpler: just tile evenly
    cols = int(1 / scale)
    rows = int(1 / scale)
    return cols, rows


def cmd_grid(args):
    scale = args.scale
    cols, rows = grid_size(scale)
    print(f"Composition: {COMP_W}x{COMP_H}, scale: {scale}")
    print(f"Grid: {cols}x{rows} ({cols * rows} cells)")
    print(f"Cell size: {COMP_W * scale:.0f}x{COMP_H * scale:.0f}")
    print()
    for row in range(rows):
        for col in range(cols):
            ox, oy = offset_for_cell(scale, col, row)
            label = f"({col},{row})"
            print(f"  {label:>8}  X: {ox:+8.1f}  Y: {oy:+8.1f}")
        print()


CORNERS = {
    "top-left": (0, 0),
    "top-right": (-1, 0),  # sentinel, resolved below
    "bottom-left": (0, -1),
    "bottom-right": (-1, -1),
    "center": None,
}


def cmd_corner(args):
    scale = args.scale
    name = args.corner
    cols, rows = grid_size(scale)

    if name == "center":
        print(f"X: 0.0  Y: 0.0")
        return

    col, row = CORNERS[name]
    if col == -1:
        col = cols - 1
    if row == -1:
        row = rows - 1

    ox, oy = offset_for_cell(scale, col, row)
    print(f"{name} at scale {scale}: X: {ox:+.1f}  Y: {oy:+.1f}")


def cmd_offset(args):
    ox, oy = offset_for_cell(args.scale, args.col, args.row)
    print(f"col={args.col} row={args.row} at scale {args.scale}: X: {ox:+.1f}  Y: {oy:+.1f}")


def cmd_tile(args):
    """Print positions for tiling the full composition at given scale."""
    scale = args.scale
    cols = int(1 / scale)
    rows = int(1 / scale)
    print(f"Tile {cols}x{rows} at scale {scale}:")
    print()
    for row in range(rows):
        for col in range(cols):
            ox, oy = offset_for_cell(scale, col, row)
            print(f"  [{col},{row}]  X: {ox:+8.1f}  Y: {oy:+8.1f}")


def main():
    parser = argparse.ArgumentParser(description="Resolume offset calculator")
    parser.add_argument("--width", type=int, default=COMP_W, help="Composition width")
    parser.add_argument("--height", type=int, default=COMP_H, help="Composition height")
    sub = parser.add_subparsers(dest="cmd")

    p = sub.add_parser("grid", help="Show all grid positions")
    p.add_argument("scale", type=float)

    p = sub.add_parser("corner", help="Offset for a named corner")
    p.add_argument("scale", type=float)
    p.add_argument("corner", choices=list(CORNERS.keys()))

    p = sub.add_parser("offset", help="Offset for a specific grid cell")
    p.add_argument("scale", type=float)
    p.add_argument("col", type=int)
    p.add_argument("row", type=int)

    p = sub.add_parser("tile", help="All tile positions for a scale")
    p.add_argument("scale", type=float)

    args = parser.parse_args()

    global COMP_W, COMP_H
    if args.width != COMP_W or args.height != COMP_H:
        COMP_W = args.width
        COMP_H = args.height

    if args.cmd is None:
        parser.print_help()
        sys.exit(1)

    {"grid": cmd_grid, "corner": cmd_corner, "offset": cmd_offset, "tile": cmd_tile}[args.cmd](args)


if __name__ == "__main__":
    main()
