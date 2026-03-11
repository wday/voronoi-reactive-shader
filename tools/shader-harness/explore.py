#!/usr/bin/env python3
"""
Random parameter space explorer for shader feedback.

Generates batches of images by sampling parameter space with taste constraints.
Each render is saved with a descriptive filename encoding the params, so
interesting results are reproducible.

Output is organized by generation:
    shader-explore/
        generations.json          # index of all generations
        gen_000/manifest.json     # seed generation
        gen_001/manifest.json     # evolved from gen_000 keepers
        gen_001/*.png

Usage:
    uv run python explore.py                    # generate 40 images (next gen)
    uv run python explore.py --count 100        # more images
    uv run python explore.py --seed 42          # reproducible run
    uv run python explore.py --evolve fav1.png fav2.png  # mutate favorites
    uv run python explore.py --keepers keepers.json      # evolve from gallery keepers
"""

import argparse
import json
import os
import random
import struct
import sys
import time
from pathlib import Path

import moderngl
import pygame
from PIL import Image, ImageDraw, ImageFont

# ── Config ──────────────────────────────────────────────────────────────────

WIDTH, HEIGHT = 800, 600
OUTPUT_DIR = Path("/mnt/c/Users/alien/Desktop/shader-explore")

RENDER_PRESETS = {
    "hi":    (2560, 1920),   # 2k — ~5 megapixels
    "max":   (3840, 2880),   # 4k — ~11 megapixels
    "ultra": (7680, 5760),   # 8k — ~44 megapixels
}

# Parameter ranges — "taste constraints" to avoid boring/degenerate output
PARAM_SPACE = {
    "u_scale": (0.3, 4.0),       # zoom in ↔ zoom out
    "u_rotation": (-1.5, 1.5),   # radians
    "u_swirl": (-3.0, 3.0),      # swirl intensity
    "u_mirror": (0.0, 1.0),      # will be snapped to 0 or 1
    "u_translate_x": (-0.4, 0.4),
    "u_translate_y": (-0.4, 0.4),
}

ITERATION_RANGE = (30, 250)
SOURCE_MIX_RANGE = (0.02, 0.2)

# Bias toward more interesting regions
SCALE_WEIGHTS = [
    (0.3, 0.8, 0.15),   # zoom in — tight, detail-oriented
    (0.8, 1.3, 0.25),   # near unity — subtle drift territory
    (1.3, 2.5, 0.35),   # zoom out — kaleidoscope territory
    (2.5, 4.0, 0.25),   # extreme zoom out — fractal tiling
]

# ── Source image generators ─────────────────────────────────────────────────

def make_shapes(w, h):
    """Geometric shapes on black — good for seeing mirror structure."""
    img = Image.new("RGBA", (w, h), (0, 0, 0, 255))
    draw = ImageDraw.Draw(img)
    draw.polygon([(w//2, h//8), (w//4, h*3//4), (w*3//4, h*3//4)],
                 fill=(255, 50, 50, 255))
    draw.ellipse((w*3//8, h//4, w*5//8, h*2//3), fill=(50, 50, 255, 255))
    draw.rectangle((w//8, h//8, w//4, h//4), fill=(50, 255, 50, 255))
    return img


def make_grid(w, h):
    """Grid pattern — shows transform warping clearly."""
    img = Image.new("RGBA", (w, h), (10, 10, 20, 255))
    draw = ImageDraw.Draw(img)
    spacing = 40
    for x in range(0, w, spacing):
        c = 180 if x % (spacing * 4) == 0 else 60
        draw.line([(x, 0), (x, h)], fill=(c, c, c + 40, 255), width=1)
    for y in range(0, h, spacing):
        c = 180 if y % (spacing * 4) == 0 else 60
        draw.line([(0, y), (w, y)], fill=(c, c + 40, c, 255), width=1)
    # Center marker
    draw.ellipse((w//2-10, h//2-10, w//2+10, h//2+10), fill=(255, 200, 50, 255))
    return img


def make_gradient_rings(w, h):
    """Concentric colored rings — beautiful with swirl."""
    img = Image.new("RGBA", (w, h), (0, 0, 0, 255))
    pixels = img.load()
    cx, cy = w / 2, h / 2
    max_r = (cx**2 + cy**2) ** 0.5
    for y in range(h):
        for x in range(w):
            r = ((x - cx)**2 + (y - cy)**2) ** 0.5
            t = r / max_r
            import math
            hue = (t * 6 + 0.5) % 1.0
            # HSV to RGB (saturation=1, value=1)
            hi = int(hue * 6) % 6
            f = hue * 6 - int(hue * 6)
            rgb = [
                (255, int(f*255), 0),
                (int((1-f)*255), 255, 0),
                (0, 255, int(f*255)),
                (0, int((1-f)*255), 255),
                (int(f*255), 0, 255),
                (255, 0, int((1-f)*255)),
            ][hi]
            # Fade rings
            ring = 0.5 + 0.5 * math.sin(r * 0.15)
            pixels[x, y] = (int(rgb[0]*ring), int(rgb[1]*ring), int(rgb[2]*ring), 255)
    return img


def make_noise_blocks(w, h):
    """Random colored blocks — chaotic seed for chaotic output."""
    img = Image.new("RGBA", (w, h), (0, 0, 0, 255))
    draw = ImageDraw.Draw(img)
    block = 60
    for y in range(0, h, block):
        for x in range(0, w, block):
            if random.random() > 0.4:
                r = random.randint(30, 255)
                g = random.randint(30, 255)
                b = random.randint(30, 255)
                draw.rectangle([x, y, x+block-2, y+block-2], fill=(r, g, b, 255))
    return img


def make_single_line(w, h):
    """Single diagonal line — minimal seed, maximum surprise."""
    img = Image.new("RGBA", (w, h), (0, 0, 0, 255))
    draw = ImageDraw.Draw(img)
    draw.line([(0, 0), (w, h)], fill=(255, 255, 255, 255), width=3)
    return img


SOURCE_GENERATORS = {
    "shapes": make_shapes,
    "grid": make_grid,
    "rings": make_gradient_rings,
    "blocks": make_noise_blocks,
    "line": make_single_line,
}

# ── Shader harness (minimal inline version) ─────────────────────────────────

VERT = """
#version 330
in vec2 position;
in vec2 texcoord;
out vec2 v_uv;
void main() { v_uv = texcoord; gl_Position = vec4(position, 0, 1); }
"""

MIX_FRAG = """
#version 330
in vec2 v_uv;
out vec4 out_color;
uniform sampler2D u_feedback;
uniform sampler2D u_source;
uniform float u_source_mix;
void main() {
    out_color = mix(texture(u_feedback, v_uv), texture(u_source, v_uv), u_source_mix);
}
"""


def load_transform_shader():
    """Load the transform fragment shader."""
    root = Path(__file__).resolve().parent.parent.parent
    shader_path = root / "plugins/mirror-transform/src/shaders/transform.frag.glsl"
    src = shader_path.read_text().replace("#version 150", "#version 330")
    return src


def render_one(ctx, transform_prog, mix_prog, vao, mix_vao,
               source_tex, fbo_a, fbo_b, tex_a, tex_b, feedback_fbo, feedback_tex,
               params, iterations, source_mix, width, height):
    """Render a single parameter set and return the final image."""

    # Clear feedback
    feedback_fbo.use()
    ctx.clear(0, 0, 0, 1)

    for i in range(iterations):
        # Mix source into feedback (or use source on frame 0)
        if i > 0:
            fbo_a.use()
            ctx.clear(0, 0, 0, 1)
            feedback_tex.use(location=0)
            source_tex.use(location=1)
            mix_prog["u_feedback"].value = 0
            mix_prog["u_source"].value = 1
            mix_prog["u_source_mix"].value = source_mix
            mix_vao.render()
            input_tex = tex_a
        else:
            input_tex = source_tex

        # Run transform
        fbo_b.use()
        ctx.clear(0, 0, 0, 1)
        input_tex.use(location=0)
        transform_prog["u_input"].value = 0
        for k, v in params.items():
            if k in transform_prog:
                transform_prog[k].value = v
        vao.render()

        # Copy to feedback
        ctx.copy_framebuffer(feedback_fbo, fbo_b)

    # Read result
    data = fbo_b.read(components=4)
    img = Image.frombytes("RGBA", (width, height), data)
    return img.transpose(Image.FLIP_TOP_BOTTOM)


# ── Parameter sampling ──────────────────────────────────────────────────────

def sample_scale():
    """Weighted random scale from interesting regions."""
    r = random.random()
    cumulative = 0
    for lo, hi, weight in SCALE_WEIGHTS:
        cumulative += weight
        if r < cumulative:
            return random.uniform(lo, hi)
    return random.uniform(1.0, 2.0)


def sample_params():
    """Sample a random parameter set."""
    params = {
        "u_scale": sample_scale(),
        "u_rotation": random.uniform(*PARAM_SPACE["u_rotation"]),
        "u_swirl": random.uniform(*PARAM_SPACE["u_swirl"]),
        "u_mirror": 1.0 if random.random() > 0.25 else 0.0,  # 75% mirror on
        "u_translate_x": random.uniform(*PARAM_SPACE["u_translate_x"]),
        "u_translate_y": random.uniform(*PARAM_SPACE["u_translate_y"]),
    }
    # Sometimes zero out translate for cleaner results
    if random.random() < 0.4:
        params["u_translate_x"] = 0.0
        params["u_translate_y"] = 0.0
    # Sometimes zero out swirl for pure geometric fractals
    if random.random() < 0.3:
        params["u_swirl"] = 0.0

    iterations = random.randint(*ITERATION_RANGE)
    source_mix = random.uniform(*SOURCE_MIX_RANGE)
    source_name = random.choice(list(SOURCE_GENERATORS.keys()))

    return params, iterations, source_mix, source_name


STRATEGIES = {
    "deep": {
        "strength": 0.12,
        "iter_sigma": 15,
        "mix_sigma": 0.015,
        "mirror_flip_chance": 0.03,
        "source_crossover": False,
        "keeper_ratio": 0.9,      # 90% mutations, 10% random
    },
    "neutral": {
        "strength": 0.3,
        "iter_sigma": 30,
        "mix_sigma": 0.03,
        "mirror_flip_chance": 0.1,
        "source_crossover": False,
        "keeper_ratio": 0.7,
    },
    "bold": {
        "strength": 0.6,
        "iter_sigma": 60,
        "mix_sigma": 0.06,
        "mirror_flip_chance": 0.3,
        "source_crossover": True,
        "keeper_ratio": 0.45,     # 45% mutations, 55% random
    },
}


def mutate_params(params, iterations, source_mix, source_name, strategy="neutral"):
    """Mutate a parameter set according to strategy."""
    s = STRATEGIES[strategy]
    new_params = {}
    for k, v in params.items():
        if k == "u_mirror":
            new_params[k] = 1.0 - v if random.random() < s["mirror_flip_chance"] else v
        else:
            lo, hi = PARAM_SPACE[k]
            perturbation = random.gauss(0, (hi - lo) * s["strength"])
            new_params[k] = max(lo, min(hi, v + perturbation))

    new_iterations = max(20, int(iterations + random.gauss(0, s["iter_sigma"])))
    new_mix = max(0.01, min(0.3, source_mix + random.gauss(0, s["mix_sigma"])))

    new_source = source_name
    if s["source_crossover"] and random.random() < 0.3:
        new_source = random.choice(list(SOURCE_GENERATORS.keys()))

    return new_params, new_iterations, new_mix, new_source


def params_to_filename(params, iterations, source_mix, source_name, idx):
    """Encode params into a readable filename."""
    s = params["u_scale"]
    r = params["u_rotation"]
    w = params["u_swirl"]
    m = "M" if params["u_mirror"] > 0.5 else "C"  # Mirror vs Clip
    tx = params["u_translate_x"]
    ty = params["u_translate_y"]
    return (f"{idx:03d}_{source_name}_s{s:.2f}_r{r:.2f}_w{w:.2f}_{m}"
            f"_tx{tx:.2f}_ty{ty:.2f}_n{iterations}_mix{source_mix:.2f}.png")


# ── Generation management ────────────────────────────────────────────────────

def next_gen_number(output_dir):
    """Find the next generation number from existing gen_NNN/ folders."""
    existing = sorted(output_dir.glob("gen_[0-9][0-9][0-9]"))
    if not existing:
        return 0
    return int(existing[-1].name[4:]) + 1


def load_generations_index(output_dir):
    """Load or create the generations index."""
    path = output_dir / "generations.json"
    if path.exists():
        return json.loads(path.read_text())
    return {"generations": []}


def save_generations_index(output_dir, index):
    """Save the generations index."""
    path = output_dir / "generations.json"
    with open(path, "w") as f:
        json.dump(index, f, indent=2)


# ── High-res render ──────────────────────────────────────────────────────────

def render_keepers(args):
    """Re-render keepers at high resolution using offscreen FBOs."""
    preset = args.render
    w, h = RENDER_PRESETS[preset]

    output_dir = Path(args.output)
    render_dir = output_dir / f"renders_{preset}"
    render_dir.mkdir(parents=True, exist_ok=True)

    # Load keepers
    keepers_path = Path(args.keepers) if args.keepers else output_dir / "keepers.json"
    if not keepers_path.exists():
        print(f"No keepers file found: {keepers_path}")
        sys.exit(1)
    keeper_data = json.loads(keepers_path.read_text())
    if keeper_data and isinstance(keeper_data[0], dict):
        filenames = [k["file"] for k in keeper_data]
    else:
        filenames = keeper_data

    print(f"Rendering {len(filenames)} keepers at {w}x{h} ({preset})")
    print(f"Output: {render_dir}/\n")

    # Init pygame with small window, render offscreen
    pygame.init()
    pygame.display.set_mode((320, 240), pygame.OPENGL | pygame.DOUBLEBUF)
    pygame.display.set_caption(f"Render {preset}")
    ctx = moderngl.create_context()

    # Compile shaders
    transform_frag = load_transform_shader()
    transform_prog = ctx.program(vertex_shader=VERT, fragment_shader=transform_frag)
    mix_prog = ctx.program(vertex_shader=VERT, fragment_shader=MIX_FRAG)

    # Shared quad
    QUAD = [-1,-1,0,0, 1,-1,1,0, -1,1,0,1, 1,-1,1,0, 1,1,1,1, -1,1,0,1]
    vbo = ctx.buffer(struct.pack(f"{len(QUAD)}f", *QUAD))
    vao = ctx.vertex_array(transform_prog, [(vbo, "2f 2f", "position", "texcoord")])
    mix_vao = ctx.vertex_array(mix_prog, [(vbo, "2f 2f", "position", "texcoord")])

    # FBOs at render resolution
    tex_a = ctx.texture((w, h), 4); tex_a.filter = (moderngl.LINEAR, moderngl.LINEAR)
    fbo_a = ctx.framebuffer([tex_a])
    tex_b = ctx.texture((w, h), 4); tex_b.filter = (moderngl.LINEAR, moderngl.LINEAR)
    fbo_b = ctx.framebuffer([tex_b])
    feedback_tex = ctx.texture((w, h), 4); feedback_tex.filter = (moderngl.LINEAR, moderngl.LINEAR)
    feedback_fbo = ctx.framebuffer([feedback_tex])

    # Pre-generate source images at render resolution
    source_textures = {}
    for name, gen_fn in SOURCE_GENERATORS.items():
        img = gen_fn(w, h)
        tex = ctx.texture((w, h), 4, img.tobytes())
        tex.filter = (moderngl.LINEAR, moderngl.LINEAR)
        source_textures[name] = tex

    for i, fname in enumerate(filenames):
        parsed = parse_filename(fname)
        if not parsed:
            continue
        params, iterations, source_mix, source_name = parsed
        source_tex = source_textures[source_name]

        t0 = time.time()
        result = render_one(
            ctx, transform_prog, mix_prog, vao, mix_vao,
            source_tex, fbo_a, fbo_b, tex_a, tex_b, feedback_fbo, feedback_tex,
            params, iterations, source_mix, w, h
        )
        dt = time.time() - t0

        out_name = Path(fname).stem + f"_{preset}.png"
        result.save(str(render_dir / out_name))
        print(f"  [{i+1}/{len(filenames)}] {out_name}  ({dt:.1f}s)")

    print(f"\nDone! {len(filenames)} renders → {render_dir}/")
    pygame.quit()


# ── Main ────────────────────────────────────────────────────────────────────

def main():
    parser = argparse.ArgumentParser(description="Random shader feedback explorer")
    parser.add_argument("--count", type=int, default=40, help="Number of images to generate")
    parser.add_argument("--seed", type=int, default=None, help="Random seed for reproducibility")
    parser.add_argument("--width", type=int, default=WIDTH)
    parser.add_argument("--height", type=int, default=HEIGHT)
    parser.add_argument("--output", type=str, default=str(OUTPUT_DIR))
    parser.add_argument(
        "--evolve", nargs="+",
        help="Mutate from these filenames (parse params from filename)"
    )
    parser.add_argument(
        "--keepers", type=str,
        help="Path to keepers.json from gallery (alternative to --evolve)"
    )
    parser.add_argument(
        "--strategy", type=str, default="neutral",
        choices=["deep", "neutral", "bold"],
        help="Evolution strategy: deep (refine), neutral, bold (explore)"
    )
    parser.add_argument(
        "--render", type=str, default=None,
        choices=["hi", "max", "ultra"],
        help="Re-render keepers at high res: hi (2k), max (4k), ultra (8k)"
    )
    args = parser.parse_args()

    if args.render:
        render_keepers(args)
        return

    output_dir = Path(args.output)
    output_dir.mkdir(parents=True, exist_ok=True)

    # Determine parent generation from keepers
    parent_gen = None
    keeper_files = []

    if args.keepers:
        keepers_path = Path(args.keepers)
        if not keepers_path.exists():
            keepers_path = output_dir / "keepers.json"
        if keepers_path.exists():
            keeper_data = json.loads(keepers_path.read_text())
            # Keepers can be plain filenames or {gen, file} objects
            if keeper_data and isinstance(keeper_data[0], dict):
                parent_gen = keeper_data[0].get("gen")
                args.evolve = [k["file"] for k in keeper_data]
                keeper_files = [k["file"] for k in keeper_data]
            else:
                args.evolve = keeper_data
                keeper_files = keeper_data
            print(f"Loaded {len(args.evolve)} keepers from {keepers_path}")
        else:
            print(f"Keepers file not found: {args.keepers}")
            sys.exit(1)

    if args.seed is not None:
        random.seed(args.seed)
        seed = args.seed
    else:
        seed = int(time.time()) % 100000
        random.seed(seed)
        print(f"Seed: {seed} (use --seed {seed} to reproduce)")

    # Determine generation number and create output subfolder
    gen_num = next_gen_number(output_dir)
    gen_dir = output_dir / f"gen_{gen_num:03d}"
    gen_dir.mkdir(parents=True, exist_ok=True)

    w, h = args.width, args.height

    # Init pygame + GL
    pygame.init()
    pygame.display.set_mode((w, h), pygame.OPENGL | pygame.DOUBLEBUF)
    pygame.display.set_caption("Explorer")
    ctx = moderngl.create_context()

    # Compile shaders
    transform_frag = load_transform_shader()
    transform_prog = ctx.program(vertex_shader=VERT, fragment_shader=transform_frag)
    mix_prog = ctx.program(vertex_shader=VERT, fragment_shader=MIX_FRAG)

    # Shared quad
    QUAD = [-1,-1,0,0, 1,-1,1,0, -1,1,0,1, 1,-1,1,0, 1,1,1,1, -1,1,0,1]
    vbo = ctx.buffer(struct.pack(f"{len(QUAD)}f", *QUAD))
    vao = ctx.vertex_array(transform_prog, [(vbo, "2f 2f", "position", "texcoord")])
    mix_vao = ctx.vertex_array(mix_prog, [(vbo, "2f 2f", "position", "texcoord")])

    # FBOs
    tex_a = ctx.texture((w, h), 4); tex_a.filter = (moderngl.LINEAR, moderngl.LINEAR)
    fbo_a = ctx.framebuffer([tex_a])
    tex_b = ctx.texture((w, h), 4); tex_b.filter = (moderngl.LINEAR, moderngl.LINEAR)
    fbo_b = ctx.framebuffer([tex_b])
    feedback_tex = ctx.texture((w, h), 4); feedback_tex.filter = (moderngl.LINEAR, moderngl.LINEAR)
    feedback_fbo = ctx.framebuffer([feedback_tex])

    # Pre-generate source images
    source_textures = {}
    for name, gen_fn in SOURCE_GENERATORS.items():
        img = gen_fn(w, h)
        tex = ctx.texture((w, h), 4, img.tobytes())
        tex.filter = (moderngl.LINEAR, moderngl.LINEAR)
        source_textures[name] = tex

    # Generate parameter sets (with parent tracking)
    strategy = args.strategy
    strat = STRATEGIES[strategy]
    param_sets = []  # list of (params, iterations, source_mix, source_name, parent_file)
    if args.evolve:
        mutation_budget = int(args.count * strat["keeper_ratio"])
        mutations_per = max(1, mutation_budget // len(args.evolve))
        for fname in args.evolve:
            base_params = parse_filename(fname)
            if base_params:
                for _ in range(mutations_per):
                    mutated = mutate_params(*base_params, strategy=strategy)
                    param_sets.append((*mutated, fname))
                print(f"Evolving from {fname}: {mutations_per} mutations [{strategy}]")

    # Fill remaining with random samples
    remaining = args.count - len(param_sets)
    for _ in range(remaining):
        param_sets.append((*sample_params(), None))
    if args.evolve:
        print(f"Strategy: {strategy} ({len(param_sets) - remaining} mutations + {remaining} random)")

    # Render batch
    print(f"\nGeneration {gen_num}: {len(param_sets)} images → {gen_dir}/\n")
    manifest = []

    for i, (params, iterations, source_mix, source_name, parent) in enumerate(param_sets):
        filename = params_to_filename(params, iterations, source_mix, source_name, i)
        filepath = gen_dir / filename

        source_tex = source_textures[source_name]

        t0 = time.time()
        result = render_one(
            ctx, transform_prog, mix_prog, vao, mix_vao,
            source_tex, fbo_a, fbo_b, tex_a, tex_b, feedback_fbo, feedback_tex,
            params, iterations, source_mix, w, h
        )
        dt = time.time() - t0

        result.save(str(filepath))
        entry = {"file": filename, "params": params,
                 "iterations": iterations, "source_mix": source_mix,
                 "source": source_name}
        if parent:
            entry["parent"] = parent
        manifest.append(entry)

        print(f"  [{i+1:3d}/{len(param_sets)}] {filename}  ({dt:.1f}s)")

    # Save per-generation manifest
    manifest_path = gen_dir / "manifest.json"
    with open(manifest_path, "w") as f:
        json.dump(manifest, f, indent=2)

    # Update generations index
    gen_index = load_generations_index(output_dir)
    gen_entry = {
        "gen": gen_num,
        "dir": f"gen_{gen_num:03d}",
        "count": len(param_sets),
        "seed": seed,
        "strategy": strategy if args.evolve else "random",
        "timestamp": time.strftime("%Y-%m-%dT%H:%M:%S"),
    }
    if parent_gen is not None:
        gen_entry["parent_gen"] = parent_gen
    if keeper_files:
        gen_entry["keepers_from"] = keeper_files
    gen_index["generations"].append(gen_entry)
    save_generations_index(output_dir, gen_index)

    print(f"\nDone! Generation {gen_num} → {gen_dir}/")
    print(f"Index: {output_dir / 'generations.json'}")

    pygame.quit()


def parse_filename(fname):
    """Parse params back from a filename generated by params_to_filename."""
    try:
        name = Path(fname).stem
        parts = name.split("_")
        # idx_source_sX.XX_rX.XX_wX.XX_M/C_txX.XX_tyX.XX_nXXX_mixX.XX
        source_name = parts[1]
        params = {
            "u_scale": float(parts[2][1:]),
            "u_rotation": float(parts[3][1:]),
            "u_swirl": float(parts[4][1:]),
            "u_mirror": 1.0 if parts[5] == "M" else 0.0,
            "u_translate_x": float(parts[6][2:]),
            "u_translate_y": float(parts[7][2:]),
        }
        iterations = int(parts[8][1:])
        source_mix = float(parts[9][3:])
        return params, iterations, source_mix, source_name
    except (IndexError, ValueError) as e:
        print(f"Warning: couldn't parse '{fname}': {e}")
        return None


if __name__ == "__main__":
    main()
