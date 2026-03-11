#!/usr/bin/env python3
"""
Shader chain test harness — ModernGL + Pygame

Loads GLSL fragment shaders, chains them through FBOs, displays the result.
Supports temporal feedback (output fed back as input) and PNG export.

Usage:
    python harness.py                          # interactive window
    python harness.py --export out.png -n 60   # run 60 iterations, save PNG
    python harness.py --shaders logistic.frag.glsl channel_displace.frag.glsl

Controls:
    Tab/Left/Right  Select active shader's uniform group
    Up/Down         Adjust selected uniform value
    Space           Toggle feedback loop
    S               Save current frame as PNG
    R               Reset feedback buffer
    Q/Esc           Quit
"""

import argparse
import os
import sys
from pathlib import Path

import moderngl
import pygame
from PIL import Image

# Default resolution
WIDTH, HEIGHT = 800, 600

# Fullscreen quad (two triangles)
QUAD_VERTICES = [
    -1.0, -1.0, 0.0, 0.0,
     1.0, -1.0, 1.0, 0.0,
    -1.0,  1.0, 0.0, 1.0,
     1.0, -1.0, 1.0, 0.0,
     1.0,  1.0, 1.0, 1.0,
    -1.0,  1.0, 0.0, 1.0,
]

VERT_SHADER = """
#version 330
in vec2 position;
in vec2 texcoord;
out vec2 v_uv;

void main() {
    v_uv = texcoord;
    gl_Position = vec4(position, 0.0, 1.0);
}
"""

# Passthrough shader for blitting to screen
BLIT_SHADER = """
#version 330
in vec2 v_uv;
out vec4 out_color;
uniform sampler2D u_input;

void main() {
    out_color = texture(u_input, v_uv);
}
"""

# Blend source into feedback: out = mix(feedback, source, source_mix)
MIX_SHADER = """
#version 330
in vec2 v_uv;
out vec4 out_color;
uniform sampler2D u_feedback;
uniform sampler2D u_source;
uniform float u_source_mix;

void main() {
    vec4 fb = texture(u_feedback, v_uv);
    vec4 src = texture(u_source, v_uv);
    out_color = mix(fb, src, u_source_mix);
}
"""


def find_project_root():
    """Walk up from this script to find the project root."""
    p = Path(__file__).resolve().parent
    while p != p.parent:
        if (p / "plugins.json").exists():
            return p
        p = p.parent
    return Path(__file__).resolve().parent


def discover_shaders(project_root):
    """Find all .frag.glsl files in the plugins directory."""
    shaders = {}
    plugins_dir = project_root / "plugins"
    for glsl in sorted(plugins_dir.rglob("*.frag.glsl")):
        rel = glsl.relative_to(plugins_dir)
        # Key by plugin name / shader name
        key = f"{rel.parts[0]}/{glsl.stem.replace('.frag', '')}"
        shaders[key] = glsl
    return shaders


def load_shader_source(path):
    """Load a fragment shader, upgrading version 150 → 330 for ModernGL."""
    src = Path(path).read_text()
    # ModernGL requires 330 core; our shaders use 150 which is compatible
    src = src.replace("#version 150", "#version 330")
    return src


def load_shader_defaults(shader_path):
    """Load uniform defaults from a companion .json file, if it exists.

    Example: transform.frag.glsl → transform.defaults.json
    Format: {"u_scale": 1.0, "u_rotation": 0.0, ...}
    """
    import json
    defaults_path = Path(shader_path).with_suffix("").with_suffix(".defaults.json")
    if defaults_path.exists():
        return json.loads(defaults_path.read_text())
    return {}


class ShaderPass:
    """A single shader in the chain with its own FBO and uniforms."""

    def __init__(self, ctx, name, frag_src, width, height, vbo, defaults=None):
        self.ctx = ctx
        self.name = name
        self.width = width
        self.height = height

        self.prog = ctx.program(vertex_shader=VERT_SHADER, fragment_shader=frag_src)
        self.vao = ctx.vertex_array(
            self.prog,
            [(vbo, "2f 2f", "position", "texcoord")],
        )

        # FBO for this pass
        self.texture = ctx.texture((width, height), 4)
        self.texture.filter = (moderngl.LINEAR, moderngl.LINEAR)
        self.fbo = ctx.framebuffer(color_attachments=[self.texture])

        defaults = defaults or {}

        # Discover uniforms (exclude samplers)
        self.uniforms = {}
        for name in self.prog:
            member = self.prog[name]
            # Skip samplers and built-in
            if hasattr(member, "value") and isinstance(member, moderngl.Uniform):
                if "sampler" not in member.name.lower() and name != "u_input":
                    if name in defaults:
                        val = defaults[name]
                    elif member.dimension == 1:
                        # FFGL convention: 0.5 is midpoint/neutral for Standard params
                        val = 0.5
                    else:
                        val = list(member.value)
                    self.uniforms[name] = {
                        "value": val,
                        "dimension": member.dimension,
                    }

    def render(self, input_tex):
        """Render this shader pass, reading from input_tex, writing to self.fbo."""
        self.fbo.use()
        self.ctx.clear(0.0, 0.0, 0.0, 1.0)

        # Bind input texture
        input_tex.use(location=0)
        if "u_input" in self.prog:
            self.prog["u_input"].value = 0

        # Set uniforms
        for uname, udata in self.uniforms.items():
            if uname in self.prog:
                self.prog[uname].value = udata["value"]

        self.vao.render()

    def set_uniform(self, name, value):
        if name in self.uniforms:
            self.uniforms[name]["value"] = value


class ShaderChain:
    """Chain of shader passes with optional feedback."""

    def __init__(self, ctx, shader_paths, width, height):
        self.ctx = ctx
        self.width = width
        self.height = height

        # Shared VBO
        import struct
        vbo_data = struct.pack(f"{len(QUAD_VERTICES)}f", *QUAD_VERTICES)
        self.vbo = ctx.buffer(vbo_data)

        # Load shader passes
        self.passes = []
        for path in shader_paths:
            name = Path(path).stem.replace(".frag", "")
            frag_src = load_shader_source(path)
            defaults = load_shader_defaults(path)
            sp = ShaderPass(ctx, name, frag_src, width, height, self.vbo, defaults)
            self.passes.append(sp)

        # Feedback buffer (previous frame output)
        self.feedback_tex = ctx.texture((width, height), 4)
        self.feedback_tex.filter = (moderngl.LINEAR, moderngl.LINEAR)
        self.feedback_fbo = ctx.framebuffer(color_attachments=[self.feedback_tex])

        # Source texture (test pattern or loaded image)
        self.source_tex = ctx.texture((width, height), 4)
        self.source_tex.filter = (moderngl.LINEAR, moderngl.LINEAR)

        # Blit program for screen output
        self.blit_prog = ctx.program(
            vertex_shader=VERT_SHADER, fragment_shader=BLIT_SHADER
        )
        self.blit_vao = ctx.vertex_array(
            self.blit_prog,
            [(self.vbo, "2f 2f", "position", "texcoord")],
        )

        # Mix program for blending source into feedback each frame
        self.mix_prog = ctx.program(
            vertex_shader=VERT_SHADER, fragment_shader=MIX_SHADER
        )
        self.mix_vao = ctx.vertex_array(
            self.mix_prog,
            [(self.vbo, "2f 2f", "position", "texcoord")],
        )
        # Mixed input buffer
        self.mixed_tex = ctx.texture((width, height), 4)
        self.mixed_tex.filter = (moderngl.LINEAR, moderngl.LINEAR)
        self.mixed_fbo = ctx.framebuffer(color_attachments=[self.mixed_tex])

        self.feedback_enabled = True
        self.source_mix = 0.1  # 10% source injected per frame
        self.frame_count = 0

    def load_source_image(self, path):
        """Load an image as the source texture."""
        img = Image.open(path).convert("RGBA").resize((self.width, self.height))
        self.source_tex.write(img.tobytes())

    def generate_test_pattern(self):
        """Generate a gradient test pattern."""
        img = Image.new("RGBA", (self.width, self.height))
        pixels = img.load()
        for y in range(self.height):
            for x in range(self.width):
                r = int(255 * x / self.width)
                g = int(255 * y / self.height)
                b = int(255 * (1.0 - x / self.width))
                pixels[x, y] = (r, g, b, 255)
        self.source_tex.write(img.tobytes())

    def render_frame(self):
        """Run the full chain for one frame."""
        # Input: blend source into feedback, or pure source on first frame
        if self.feedback_enabled and self.frame_count > 0:
            # Mix source into feedback: keeps content alive through iterations
            self.mixed_fbo.use()
            self.ctx.clear(0.0, 0.0, 0.0, 1.0)
            self.feedback_tex.use(location=0)
            self.source_tex.use(location=1)
            self.mix_prog["u_feedback"].value = 0
            self.mix_prog["u_source"].value = 1
            self.mix_prog["u_source_mix"].value = self.source_mix
            self.mix_vao.render()
            current_input = self.mixed_tex
        else:
            current_input = self.source_tex

        # Chain passes
        for shader_pass in self.passes:
            # Pass texel size if the shader wants it
            if "u_texel_size" in shader_pass.uniforms:
                shader_pass.set_uniform(
                    "u_texel_size", (1.0 / self.width, 1.0 / self.height)
                )
            shader_pass.render(current_input)
            current_input = shader_pass.texture

        # Copy final output to feedback buffer
        if self.feedback_enabled:
            self.ctx.copy_framebuffer(
                self.feedback_fbo, self.passes[-1].fbo if self.passes else None
            )

        self.frame_count += 1

    def blit_to_screen(self, screen_fbo):
        """Blit the final output to the screen."""
        screen_fbo.use()
        final_tex = self.passes[-1].texture if self.passes else self.source_tex
        final_tex.use(location=0)
        self.blit_prog["u_input"].value = 0
        self.blit_vao.render()

    def get_frame(self):
        """Read the current output as a PIL Image."""
        final_fbo = self.passes[-1].fbo if self.passes else self.feedback_fbo
        data = final_fbo.read(components=4)
        img = Image.frombytes("RGBA", (self.width, self.height), data)
        return img.transpose(Image.FLIP_TOP_BOTTOM)

    def save_frame(self, path):
        """Save the current output as a PNG."""
        self.get_frame().save(path)
        print(f"Saved: {path}")

    def reset_feedback(self):
        """Clear the feedback buffer."""
        self.feedback_fbo.use()
        self.ctx.clear(0.0, 0.0, 0.0, 1.0)
        self.frame_count = 0


def run_interactive(chain, width, height):
    """Run the interactive Pygame window."""
    clock = pygame.time.Clock()
    screen_fbo = chain.ctx.detect_framebuffer()
    running = True

    # Uniform editing state
    active_pass_idx = 0
    active_uniform_idx = 0

    font = pygame.font.SysFont("monospace", 14)

    while running:
        for event in pygame.event.get():
            if event.type == pygame.QUIT:
                running = False
            elif event.type == pygame.KEYDOWN:
                if event.key in (pygame.K_q, pygame.K_ESCAPE):
                    running = False
                elif event.key == pygame.K_SPACE:
                    chain.feedback_enabled = not chain.feedback_enabled
                    status = "ON" if chain.feedback_enabled else "OFF"
                    print(f"Feedback: {status}")
                elif event.key == pygame.K_s:
                    chain.save_frame(f"frame_{chain.frame_count:04d}.png")
                elif event.key == pygame.K_r:
                    chain.reset_feedback()
                    print("Feedback buffer reset")
                elif event.key == pygame.K_TAB:
                    if chain.passes:
                        active_pass_idx = (active_pass_idx + 1) % len(chain.passes)
                        active_uniform_idx = 0
                        print(f"Active pass: {chain.passes[active_pass_idx].name}")
                elif event.key == pygame.K_LEFT:
                    sp = chain.passes[active_pass_idx] if chain.passes else None
                    if sp and sp.uniforms:
                        keys = list(sp.uniforms.keys())
                        active_uniform_idx = (active_uniform_idx - 1) % len(keys)
                        print(f"  Uniform: {keys[active_uniform_idx]} = {sp.uniforms[keys[active_uniform_idx]]['value']}")
                elif event.key == pygame.K_RIGHT:
                    sp = chain.passes[active_pass_idx] if chain.passes else None
                    if sp and sp.uniforms:
                        keys = list(sp.uniforms.keys())
                        active_uniform_idx = (active_uniform_idx + 1) % len(keys)
                        print(f"  Uniform: {keys[active_uniform_idx]} = {sp.uniforms[keys[active_uniform_idx]]['value']}")
                elif event.key in (pygame.K_UP, pygame.K_DOWN):
                    sp = chain.passes[active_pass_idx] if chain.passes else None
                    if sp and sp.uniforms:
                        keys = list(sp.uniforms.keys())
                        uname = keys[active_uniform_idx]
                        udata = sp.uniforms[uname]
                        step = 0.05 if event.key == pygame.K_UP else -0.05
                        if udata["dimension"] == 1:
                            udata["value"] = round(udata["value"] + step, 4)
                        print(f"  {uname} = {udata['value']}")

        chain.render_frame()
        chain.blit_to_screen(screen_fbo)

        # HUD overlay
        surface = pygame.display.get_surface()
        hud_lines = [
            f"Frame: {chain.frame_count}  Feedback: {'ON' if chain.feedback_enabled else 'OFF'}  FPS: {clock.get_fps():.0f}",
            f"[Tab] pass  [←→] uniform  [↑↓] adjust  [Space] feedback  [S] save  [R] reset",
        ]
        if chain.passes:
            sp = chain.passes[active_pass_idx]
            hud_lines.append(f"Pass: {sp.name}")
            if sp.uniforms:
                keys = list(sp.uniforms.keys())
                for i, k in enumerate(keys):
                    marker = ">" if i == active_uniform_idx else " "
                    hud_lines.append(f"  {marker} {k} = {sp.uniforms[k]['value']}")

        for i, line in enumerate(hud_lines):
            text = font.render(line, True, (255, 255, 255), (0, 0, 0))
            surface.blit(text, (4, 4 + i * 16))

        pygame.display.flip()
        clock.tick(60)


def run_export(chain, path, num_frames):
    """Run N frames and save the result."""
    for i in range(num_frames):
        chain.render_frame()
        if (i + 1) % 10 == 0:
            print(f"  Frame {i + 1}/{num_frames}")
    chain.save_frame(path)


def main():
    project_root = find_project_root()

    parser = argparse.ArgumentParser(description="Shader chain test harness")
    parser.add_argument(
        "--shaders", nargs="+", help="Fragment shader paths (in chain order)"
    )
    parser.add_argument("--source", help="Source image path (default: test pattern)")
    parser.add_argument("--export", help="Export final frame to PNG path")
    parser.add_argument(
        "-n", "--frames", type=int, default=60, help="Number of frames for export mode"
    )
    parser.add_argument("--width", type=int, default=WIDTH)
    parser.add_argument("--height", type=int, default=HEIGHT)
    parser.add_argument(
        "--list", action="store_true", help="List available shaders and exit"
    )
    parser.add_argument(
        "--uniform", action="append", nargs=3, metavar=("PASS", "NAME", "VALUE"),
        help="Set initial uniform: --uniform 0 u_r_base 3.2"
    )
    args = parser.parse_args()

    # List mode
    if args.list:
        shaders = discover_shaders(project_root)
        print("Available fragment shaders:")
        for key, path in shaders.items():
            print(f"  {key:40s} {path}")
        return

    # Init Pygame + OpenGL
    pygame.init()
    pygame.font.init()
    pygame.display.set_mode(
        (args.width, args.height), pygame.OPENGL | pygame.DOUBLEBUF
    )
    pygame.display.set_caption("Shader Harness")
    ctx = moderngl.create_context()

    # Resolve shader paths
    shader_paths = []
    if args.shaders:
        available = discover_shaders(project_root)
        for s in args.shaders:
            # Try as direct path first
            if os.path.exists(s):
                shader_paths.append(s)
            # Try as discovered key
            elif s in available:
                shader_paths.append(str(available[s]))
            # Try fuzzy match
            else:
                matches = [k for k in available if s in k]
                if matches:
                    shader_paths.append(str(available[matches[0]]))
                    print(f"Matched '{s}' → {matches[0]}")
                else:
                    print(f"Shader not found: {s}")
                    sys.exit(1)
    else:
        # Default: load all chaos toolkit shaders
        available = discover_shaders(project_root)
        print("No shaders specified. Available:")
        for key in available:
            print(f"  {key}")
        print("\nUsage: python harness.py --shaders transform logistic displace")
        pygame.quit()
        return

    print(f"Chain: {' → '.join(Path(p).stem for p in shader_paths)}")

    chain = ShaderChain(ctx, shader_paths, args.width, args.height)

    # Load source
    if args.source:
        chain.load_source_image(args.source)
        print(f"Source: {args.source}")
    else:
        chain.generate_test_pattern()
        print("Source: test pattern")

    # Set initial uniforms
    if args.uniform:
        for pass_idx, uname, uval in args.uniform:
            idx = int(pass_idx)
            if idx < len(chain.passes):
                chain.passes[idx].set_uniform(uname, float(uval))
                print(f"  Pass {idx} {uname} = {uval}")

    if args.export:
        run_export(chain, args.export, args.frames)
    else:
        run_interactive(chain, args.width, args.height)

    pygame.quit()


if __name__ == "__main__":
    main()
