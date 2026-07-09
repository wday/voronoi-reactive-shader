#!/usr/bin/env python3
"""Golden-rectangle (whirling-squares) spiral generator.

Emits the quarter-circle arc *anchors* (pivot/center of each arc) and endpoints
of a golden spiral, fitted to an arbitrary frame size, in a configurable
coordinate/origin convention. Use it to place a golden-spiral overlay or seed
points (e.g. into Resolume or an ISF shader).

Workflow for "which origin does the host use?":
  1. Render the SVG preview (--svg) — that is ground truth, drawn in plain
     top-left pixels and always correct.
  2. Export coordinates in some ORIGIN (--origin ...), paste into Resolume/shader.
  3. If they don't land where the SVG shows, switch --origin and repeat. The
     preset list (--list-origins) covers every space x y-direction x center combo.

Pure stdlib, no deps. Examples:
  python golden_spiral.py --list-origins
  python golden_spiral.py --origin gl-bottom-left --svg spiral.svg --json spiral.json
  python golden_spiral.py --width 1920 --height 1080 --origin norm11-yup --arcs 8
"""

from __future__ import annotations

import argparse
import json
import math
from dataclasses import dataclass, field

PHI = (1 + 5 ** 0.5) / 2  # 1.6180339887498949


# --------------------------------------------------------------------------- #
# Canonical spiral: rectangle [0, PHI] x [0, 1], origin bottom-left, y up,
# short side = 1. Everything downstream is a transform of this.
# --------------------------------------------------------------------------- #

@dataclass(frozen=True)
class Arc:
    cx: float          # anchor / pivot x  (center of the arc's circle)
    cy: float          # anchor / pivot y
    r: float           # radius (== the square's side)
    a0: float          # start angle (rad, math CCW, y-up)
    a1: float          # end angle   (a1 = a0 - 90 deg; the spiral winds CW inward)
    sx: float          # start point
    sy: float
    ex: float          # end point (== next arc's start point: continuous & tangent)
    ey: float


def canonical_arcs(n: int) -> list[Arc]:
    """The golden spiral as `n` quarter-circle arcs.

    Arc k pivots on the square corner nearest the eye; radius = that square's
    side = PHI**-k. Consecutive arcs share an endpoint, so the curve is C1
    (continuous + tangent). Self-generating from the start point (0, 0):
    each arc's center is `start - r*(cos a0, sin a0)`, its end is 90 deg along.
    """
    arcs: list[Arc] = []
    px, py = 0.0, 0.0  # start point of arc 0
    for k in range(n):
        a0 = math.radians(180 - 90 * k)
        r = PHI ** (-k)
        cx = px - r * math.cos(a0)
        cy = py - r * math.sin(a0)
        a1 = a0 - math.pi / 2
        ex = cx + r * math.cos(a1)
        ey = cy + r * math.sin(a1)
        arcs.append(Arc(cx, cy, r, a0, a1, px, py, ex, ey))
        px, py = ex, ey
    return arcs


def pole(iters: int = 64) -> tuple[float, float]:
    """The eye of the spiral (arc centers converge to it)."""
    a = canonical_arcs(iters)[-1]
    return (a.cx, a.cy)


def sample_arc(arc: Arc, steps: int) -> list[tuple[float, float]]:
    """Polyline approximation of an arc (bulletproof for rendering / point sets)."""
    return [
        (
            arc.cx + arc.r * math.cos(arc.a0 + (arc.a1 - arc.a0) * t / steps),
            arc.cy + arc.r * math.sin(arc.a0 + (arc.a1 - arc.a0) * t / steps),
        )
        for t in range(steps + 1)
    ]


CANON_RECT = [(0.0, 0.0), (PHI, 0.0), (PHI, 1.0), (0.0, 1.0)]  # golden rect corners


# --------------------------------------------------------------------------- #
# Origin config — the part you experiment with. Converts a point given in
# frame pixels (origin bottom-left, y up) into the host's convention.
# --------------------------------------------------------------------------- #

@dataclass(frozen=True)
class Origin:
    space: str            # "px" | "norm01" | "norm11"
    y_down: bool          # False: y up (GL/ISF). True: y down (image/AE/CSS/DOM).
    centered: bool        # origin at frame center (auto-True for norm11)
    name: str = ""


# Every meaningful combination, ready to try one by one.
ORIGINS: dict[str, Origin] = {
    # pixels
    "gl-bottom-left":  Origin("px",     False, False, "gl-bottom-left"),   # OpenGL/ISF pixel
    "image-top-left":  Origin("px",     True,  False, "image-top-left"),   # image / most 2D APIs
    "center-px-yup":   Origin("px",     False, True,  "center-px-yup"),
    "center-px-ydown": Origin("px",     True,  True,  "center-px-ydown"),
    # normalized 0..1
    "norm01-bl":       Origin("norm01", False, False, "norm01-bl"),        # ISF isf_FragNormCoord
    "norm01-tl":       Origin("norm01", True,  False, "norm01-tl"),        # UV / texcoord (DirectX-ish)
    # normalized -1..1 (center origin)
    "norm11-yup":      Origin("norm11", False, True,  "norm11-yup"),       # GL clip space
    "norm11-ydown":    Origin("norm11", True,  True,  "norm11-ydown"),
}
# Best guesses to try first for the two hosts you mentioned:
#   ISF shader sampling:  norm01-bl  (or gl-bottom-left for pixel coords)
#   Resolume position params are usually CENTER-origin normalized -> norm11-yup
#                                                        (try norm11-ydown too)


def apply_origin(x: float, y: float, W: int, H: int, o: Origin) -> tuple[float, float]:
    """frame px (bottom-left, y up)  ->  host convention."""
    yv = (H - y) if o.y_down else y
    centered = o.centered or o.space == "norm11"
    if centered:
        x -= W / 2.0
        yv -= H / 2.0
    if o.space == "px":
        return (x, yv)
    if o.space == "norm01":
        return (x / W, yv / H)
    if o.space == "norm11":
        return (2.0 * x / W, 2.0 * yv / H)
    raise ValueError(f"unknown space {o.space!r}")


def origin_scale(W: int, H: int, o: Origin) -> tuple[float, float]:
    """Per-axis px->space factor (for converting a radius). Note: when W != H a
    circle becomes an ellipse in a normalized space, hence separate rx, ry."""
    if o.space == "px":
        return (1.0, 1.0)
    if o.space == "norm01":
        return (1.0 / W, 1.0 / H)
    if o.space == "norm11":
        return (2.0 / W, 2.0 / H)
    raise ValueError(o.space)


# --------------------------------------------------------------------------- #
# Placement — orient (mirror/rotate to move the eye) then fit into the frame.
# --------------------------------------------------------------------------- #

@dataclass
class Config:
    frame_w: int = 1920
    frame_h: int = 1080
    fit: str = "contain"        # "contain" (whole rect fits) | "width" | "height"
    margin: float = 0.0         # shrink fraction 0..1 (breathing room)
    align_x: float = 0.5        # 0 left .. 1 right, within leftover space
    align_y: float = 0.5        # 0 bottom .. 1 top (y-up frame space)
    mirror_x: bool = False      # flip to move the eye horizontally
    mirror_y: bool = False      # flip to move the eye vertically
    quarter_turns: int = 0      # 0..3, rotate whole spiral 90 deg CCW each
    n_arcs: int = 8
    arc_steps: int = 48         # polyline samples per arc
    origin: Origin = field(default_factory=lambda: ORIGINS["gl-bottom-left"])


def orient(x: float, y: float, cfg: Config) -> tuple[float, float]:
    """Mirror / rotate in canonical space. Absolute position is irrelevant —
    the fit step re-normalizes against the transformed bounding box."""
    if cfg.mirror_x:
        x = -x
    if cfg.mirror_y:
        y = -y
    for _ in range(cfg.quarter_turns % 4):
        x, y = -y, x  # 90 deg CCW
    return x, y


def make_placement(cfg: Config):
    """Returns to_frame_px(x, y) mapping canonical -> frame px (bottom-left, y up),
    plus the isotropic px-per-unit scale (for radii)."""
    pts = [orient(x, y, cfg) for x, y in CANON_RECT]
    xs = [p[0] for p in pts]
    ys = [p[1] for p in pts]
    bx0, bx1 = min(xs), max(xs)
    by0, by1 = min(ys), max(ys)
    bw, bh = bx1 - bx0, by1 - by0
    W, H = cfg.frame_w, cfg.frame_h

    if cfg.fit == "width":
        s = W / bw
    elif cfg.fit == "height":
        s = H / bh
    else:  # contain
        s = min(W / bw, H / bh)
    s *= (1.0 - cfg.margin)

    pw, ph = bw * s, bh * s
    ox = cfg.align_x * (W - pw)
    oy = cfg.align_y * (H - ph)

    def to_frame_px(x: float, y: float) -> tuple[float, float]:
        x, y = orient(x, y, cfg)
        return (ox + (x - bx0) * s, oy + (y - by0) * s)

    return to_frame_px, s


# --------------------------------------------------------------------------- #
# Build result in the target convention.
# --------------------------------------------------------------------------- #

def build(cfg: Config) -> dict:
    to_px, s = make_placement(cfg)
    W, H, o = cfg.frame_w, cfg.frame_h, cfg.origin
    sx, sy = origin_scale(W, H, o)

    def conv(x, y):
        fx, fy = to_px(x, y)
        return apply_origin(fx, fy, W, H, o)

    out_arcs = []
    for k, arc in enumerate(canonical_arcs(cfg.n_arcs)):
        acx, acy = conv(arc.cx, arc.cy)          # anchor (pivot)
        astart = conv(arc.sx, arc.sy)
        aend = conv(arc.ex, arc.ey)
        r_px = arc.r * s
        out_arcs.append({
            "k": k,
            "anchor": [round(acx, 5), round(acy, 5)],
            "radius_px": round(r_px, 4),
            "radius": [round(r_px * sx, 6), round(r_px * sy, 6)],  # in target space (rx, ry)
            "start": [round(astart[0], 5), round(astart[1], 5)],
            "end": [round(aend[0], 5), round(aend[1], 5)],
            "samples": [[round(x, 4), round(y, 4)] for x, y in
                        (conv(px, py) for px, py in sample_arc(arc, cfg.arc_steps))],
        })

    pcx, pcy = conv(*pole())
    return {
        "phi": PHI,
        "frame": [W, H],
        "origin": o.name or o.space,
        "origin_detail": {"space": o.space, "y_down": o.y_down,
                          "centered": o.centered or o.space == "norm11"},
        "fit": cfg.fit, "margin": cfg.margin,
        "align": [cfg.align_x, cfg.align_y],
        "mirror": [cfg.mirror_x, cfg.mirror_y], "quarter_turns": cfg.quarter_turns,
        "pole": [round(pcx, 5), round(pcy, 5)],
        "arcs": out_arcs,
    }


# --------------------------------------------------------------------------- #
# Outputs.
# --------------------------------------------------------------------------- #

def print_table(result: dict) -> None:
    o = result["origin_detail"]
    dp = 2 if o["space"] == "px" else 4
    print(f"# golden spiral  frame={result['frame'][0]}x{result['frame'][1]}  "
          f"origin={result['origin']} (space={o['space']}, "
          f"y_down={o['y_down']}, centered={o['centered']})")
    print(f"#{'k':>2} {'anchor_x':>11} {'anchor_y':>11} {'r_px':>9}   "
          f"{'start(x,y)':>22}   {'end(x,y)':>22}")
    for a in result["arcs"]:
        ax, ay = a["anchor"]
        sx_, sy_ = a["start"]
        ex_, ey_ = a["end"]
        print(f" {a['k']:>2} {ax:>11.{dp}f} {ay:>11.{dp}f} {a['radius_px']:>9.2f}   "
              f"({sx_:>9.{dp}f},{sy_:>9.{dp}f})   ({ex_:>9.{dp}f},{ey_:>9.{dp}f})")
    px, py = result["pole"]
    print(f"# eye/pole: ({px:.{dp}f}, {py:.{dp}f})")


def write_svg(cfg: Config, path: str) -> None:
    """Preview in plain top-left pixel space (independent of --origin): this is
    the geometric ground truth to compare the host against."""
    to_px, s = make_placement(cfg)
    W, H = cfg.frame_w, cfg.frame_h

    def svg(x, y):  # frame px (bl, y-up) -> svg px (tl, y-down)
        fx, fy = to_px(x, y)
        return fx, H - fy

    parts = [
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H}" '
        f'viewBox="0 0 {W} {H}">',
        f'<rect x="0" y="0" width="{W}" height="{H}" fill="#111"/>',
    ]
    # golden rectangle outline
    rect = " ".join(f"{x:.2f},{y:.2f}" for x, y in (svg(*c) for c in CANON_RECT))
    parts.append(f'<polygon points="{rect}" fill="none" stroke="#444" '
                 f'stroke-width="1.5"/>')
    # arcs
    for arc in canonical_arcs(cfg.n_arcs):
        pts = " ".join(f"{x:.2f},{y:.2f}"
                       for x, y in (svg(*p) for p in sample_arc(arc, cfg.arc_steps)))
        parts.append(f'<polyline points="{pts}" fill="none" stroke="#ff3b00" '
                     f'stroke-width="2.5"/>')
        ax, ay = svg(arc.cx, arc.cy)
        parts.append(f'<circle cx="{ax:.2f}" cy="{ay:.2f}" r="4" fill="#00d0ff"/>')
    # eye
    ex, ey = svg(*pole())
    parts.append(f'<circle cx="{ex:.2f}" cy="{ey:.2f}" r="5" fill="none" '
                 f'stroke="#fff" stroke-width="2"/>')
    parts.append('</svg>')
    with open(path, "w") as f:
        f.write("\n".join(parts))


# --------------------------------------------------------------------------- #
# CLI.
# --------------------------------------------------------------------------- #

def parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser(description=__doc__,
                                formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--width", type=int, default=1920)
    p.add_argument("--height", type=int, default=1080)
    p.add_argument("--origin", default="gl-bottom-left", choices=list(ORIGINS))
    p.add_argument("--fit", default="contain", choices=["contain", "width", "height"])
    p.add_argument("--margin", type=float, default=0.0)
    p.add_argument("--align-x", type=float, default=0.5)
    p.add_argument("--align-y", type=float, default=0.5)
    p.add_argument("--mirror-x", action="store_true")
    p.add_argument("--mirror-y", action="store_true")
    p.add_argument("--turns", type=int, default=0, help="quarter turns CCW (0..3)")
    p.add_argument("--arcs", type=int, default=8)
    p.add_argument("--steps", type=int, default=48, help="polyline samples/arc")
    p.add_argument("--svg", metavar="PATH", help="write preview SVG")
    p.add_argument("--json", metavar="PATH", help="write result JSON")
    p.add_argument("--list-origins", action="store_true")
    return p.parse_args()


def main() -> None:
    args = parse_args()
    if args.list_origins:
        print("available origins (pass with --origin):")
        for name, o in ORIGINS.items():
            print(f"  {name:<16} space={o.space:<7} "
                  f"y_down={str(o.y_down):<5} centered={o.centered or o.space=='norm11'}")
        return

    cfg = Config(
        frame_w=args.width, frame_h=args.height, fit=args.fit, margin=args.margin,
        align_x=args.align_x, align_y=args.align_y,
        mirror_x=args.mirror_x, mirror_y=args.mirror_y, quarter_turns=args.turns,
        n_arcs=args.arcs, arc_steps=args.steps, origin=ORIGINS[args.origin],
    )
    result = build(cfg)
    print_table(result)
    if args.json:
        with open(args.json, "w") as f:
            json.dump(result, f, indent=2)
        print(f"# wrote {args.json}")
    if args.svg:
        write_svg(cfg, args.svg)
        print(f"# wrote {args.svg}")


if __name__ == "__main__":
    main()
