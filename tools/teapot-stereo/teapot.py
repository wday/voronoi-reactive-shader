#!/usr/bin/env python3
"""
Synthetic stereo test source — rotating Utah teapot.

Renders a teapot from two camera offsets (left/right eye), hstacks the
frames, and writes raw RGBA to stdout. Designed to feed spout-publish
for end-to-end flow-plugin pipeline testing.

Usage:
    uv run python teapot.py --preview              # pygame window
    uv run python teapot.py | spout-publish ...     # pipe to Spout
    uv run python teapot.py --frames 10 > /dev/null # headless test
"""

import argparse
import sys
import time
from pathlib import Path

import moderngl
import numpy as np

# ---------------------------------------------------------------------------
# Mesh generation (surface of revolution teapot)
# ---------------------------------------------------------------------------

def _revolve_profile(profile, n_seg=24):
    """Revolve a 2D profile curve (r, y) around the Y axis.

    Returns (positions, normals, indices) as numpy arrays.
    Profile is list of (radius, height) tuples.
    """
    n_prof = len(profile)
    verts = []
    norms = []

    for i, (r, y) in enumerate(profile):
        for j in range(n_seg):
            theta = 2.0 * np.pi * j / n_seg
            c, s = np.cos(theta), np.sin(theta)
            verts.append([r * c, y, r * s])

            # Approximate normal from profile tangent
            if i == 0:
                dr = profile[1][0] - profile[0][0]
                dy = profile[1][1] - profile[0][1]
            elif i == n_prof - 1:
                dr = profile[-1][0] - profile[-2][0]
                dy = profile[-1][1] - profile[-2][1]
            else:
                dr = profile[i + 1][0] - profile[i - 1][0]
                dy = profile[i + 1][1] - profile[i - 1][1]

            # Normal is perpendicular to tangent in the profile plane
            # tangent = (dr, dy), normal = (dy, -dr), then revolve
            nx = dy * c
            ny = -dr
            nz = dy * s
            ln = np.sqrt(nx * nx + ny * ny + nz * nz)
            if ln > 1e-8:
                nx /= ln; ny /= ln; nz /= ln
            norms.append([nx, ny, nz])

    indices = []
    for i in range(n_prof - 1):
        for j in range(n_seg):
            j1 = (j + 1) % n_seg
            a = i * n_seg + j
            b = i * n_seg + j1
            c = (i + 1) * n_seg + j
            d = (i + 1) * n_seg + j1
            indices.extend([a, b, d, a, d, c])

    return np.array(verts, dtype="f4"), np.array(norms, dtype="f4"), np.array(indices, dtype="i4")


def _swept_tube(path, radius=0.08, n_seg=8):
    """Sweep a circular cross-section along a 3D path.

    Returns (positions, normals, indices).
    """
    path = np.array(path, dtype="f4")
    n_path = len(path)
    verts = []
    norms = []

    for i in range(n_path):
        # Tangent
        if i == 0:
            t = path[1] - path[0]
        elif i == n_path - 1:
            t = path[-1] - path[-2]
        else:
            t = path[i + 1] - path[i - 1]
        t = t / (np.linalg.norm(t) + 1e-8)

        # Find a perpendicular vector
        if abs(t[1]) < 0.9:
            up = np.array([0, 1, 0], dtype="f4")
        else:
            up = np.array([1, 0, 0], dtype="f4")
        n1 = np.cross(t, up)
        n1 = n1 / (np.linalg.norm(n1) + 1e-8)
        n2 = np.cross(t, n1)

        for j in range(n_seg):
            theta = 2.0 * np.pi * j / n_seg
            normal = np.cos(theta) * n1 + np.sin(theta) * n2
            pos = path[i] + radius * normal
            verts.append(pos)
            norms.append(normal)

    indices = []
    for i in range(n_path - 1):
        for j in range(n_seg):
            j1 = (j + 1) % n_seg
            a = i * n_seg + j
            b = i * n_seg + j1
            c = (i + 1) * n_seg + j
            d = (i + 1) * n_seg + j1
            indices.extend([a, b, d, a, d, c])

    return np.array(verts, dtype="f4"), np.array(norms, dtype="f4"), np.array(indices, dtype="i4")


def _cap_disc(center, radius, normal, n_seg=24):
    """Generate a filled disc (triangle fan)."""
    normal = np.array(normal, dtype="f4")
    normal = normal / np.linalg.norm(normal)
    center = np.array(center, dtype="f4")

    if abs(normal[1]) < 0.9:
        up = np.array([0, 1, 0], dtype="f4")
    else:
        up = np.array([1, 0, 0], dtype="f4")
    u = np.cross(normal, up)
    u = u / np.linalg.norm(u)
    v = np.cross(normal, u)

    verts = [center]
    norms = [normal.copy()]
    for j in range(n_seg):
        theta = 2.0 * np.pi * j / n_seg
        pos = center + radius * (np.cos(theta) * u + np.sin(theta) * v)
        verts.append(pos)
        norms.append(normal.copy())

    indices = []
    for j in range(n_seg):
        j1 = (j + 1) % n_seg
        indices.extend([0, j + 1, j1 + 1])

    return np.array(verts, dtype="f4"), np.array(norms, dtype="f4"), np.array(indices, dtype="i4")


def _merge_meshes(meshes):
    """Merge list of (positions, normals, indices) tuples."""
    all_pos = []
    all_norm = []
    all_idx = []
    offset = 0
    for pos, norm, idx in meshes:
        all_pos.append(pos)
        all_norm.append(norm)
        all_idx.append(idx + offset)
        offset += len(pos)
    return (
        np.concatenate(all_pos),
        np.concatenate(all_norm),
        np.concatenate(all_idx),
    )


def generate_teapot_mesh():
    """Generate a Utah-teapot-like mesh procedurally.

    Returns (positions, normals, indices) numpy arrays.
    """
    # Scale factor — teapot centered near origin, ~2 units tall
    S = 0.6

    # Body profile: (radius, height)
    body_profile = [
        (0.0, 0.0),
        (0.6, 0.0),
        (1.2, 0.1),
        (1.6, 0.3),
        (1.9, 0.6),
        (2.1, 1.0),
        (2.15, 1.4),
        (2.0, 1.8),
        (1.8, 2.1),
        (1.5, 2.4),
        (1.45, 2.55),
        (1.5, 2.6),
    ]
    body_profile = [(r * S, y * S) for r, y in body_profile]

    # Lid profile
    lid_profile = [
        (1.5, 2.6),
        (1.3, 2.7),
        (1.0, 2.8),
        (0.6, 2.95),
        (0.3, 3.05),
        (0.15, 3.1),
        (0.25, 3.15),
        (0.25, 3.25),
        (0.15, 3.35),
        (0.0, 3.4),
    ]
    lid_profile = [(r * S, y * S) for r, y in lid_profile]

    meshes = []
    meshes.append(_revolve_profile(body_profile))
    meshes.append(_revolve_profile(lid_profile))

    # Bottom cap
    meshes.append(_cap_disc([0, 0, 0], 0.6 * S, [0, -1, 0]))

    # Spout — Bezier-ish path curving outward and up
    spout_pts = []
    for t in np.linspace(0, 1, 10):
        # Quadratic bezier: base at body, tip sticking out
        p0 = np.array([1.9 * S, 1.5 * S, 0])
        p1 = np.array([2.8 * S, 1.6 * S, 0])
        p2 = np.array([3.0 * S, 2.2 * S, 0])
        pt = (1 - t) ** 2 * p0 + 2 * (1 - t) * t * p1 + t * t * p2
        spout_pts.append(pt)
    # Taper the spout
    spout_radii = np.linspace(0.18 * S, 0.10 * S, 10)
    spout_meshes = []
    for i in range(len(spout_pts) - 1):
        seg_path = [spout_pts[i], spout_pts[i + 1]]
        r = (spout_radii[i] + spout_radii[i + 1]) / 2
        spout_meshes.append(_swept_tube(seg_path, radius=r, n_seg=8))
    if spout_meshes:
        meshes.append(_merge_meshes(spout_meshes))

    # Handle — arc on the opposite side
    handle_pts = []
    for t in np.linspace(0, 1, 12):
        angle = np.pi * 0.3 + t * np.pi * 0.6  # arc from lower to upper body
        r_handle = 0.55 * S  # distance from body surface
        cx = -(1.9 * S + r_handle)  # center of handle arc (negative x = opposite spout)
        x = cx + r_handle * np.cos(angle)
        y = 0.5 * S + t * 1.6 * S  # height range
        handle_pts.append([x, y, 0])
    meshes.append(_swept_tube(handle_pts, radius=0.1 * S, n_seg=8))

    positions, normals, indices = _merge_meshes(meshes)

    # Center vertically
    y_min = positions[:, 1].min()
    y_max = positions[:, 1].max()
    y_center = (y_min + y_max) / 2
    positions[:, 1] -= y_center

    return positions, normals, indices


def save_obj(path, positions, normals, indices):
    """Write mesh to OBJ file."""
    with open(path, "w") as f:
        f.write("# Utah teapot (procedurally generated)\n")
        for p in positions:
            f.write(f"v {p[0]:.6f} {p[1]:.6f} {p[2]:.6f}\n")
        for n in normals:
            f.write(f"vn {n[0]:.6f} {n[1]:.6f} {n[2]:.6f}\n")
        for i in range(0, len(indices), 3):
            a, b, c = indices[i] + 1, indices[i + 1] + 1, indices[i + 2] + 1
            f.write(f"f {a}//{a} {b}//{b} {c}//{c}\n")


def load_obj(path):
    """Minimal OBJ loader — positions and normals only."""
    positions = []
    normals = []
    faces = []

    with open(path) as f:
        for line in f:
            parts = line.strip().split()
            if not parts:
                continue
            if parts[0] == "v":
                positions.append([float(x) for x in parts[1:4]])
            elif parts[0] == "vn":
                normals.append([float(x) for x in parts[1:4]])
            elif parts[0] == "f":
                for vert in parts[1:]:
                    # Handle v, v//vn, v/vt/vn formats
                    idx = vert.split("/")
                    vi = int(idx[0]) - 1
                    ni = int(idx[-1]) - 1 if idx[-1] else vi
                    faces.append((vi, ni))

    return (
        np.array(positions, dtype="f4"),
        np.array(normals, dtype="f4"),
        faces,
    )


def load_or_generate_mesh():
    """Load teapot.obj if it exists, otherwise generate and cache it."""
    obj_path = Path(__file__).parent / "teapot.obj"
    if obj_path.exists():
        positions, normals, faces = load_obj(obj_path)
        # Rebuild as interleaved vertex buffer
        verts = []
        for vi, ni in faces:
            verts.append(np.concatenate([positions[vi], normals[ni]]))
        return np.array(verts, dtype="f4")

    # Generate procedurally
    positions, normals, indices = generate_teapot_mesh()
    save_obj(obj_path, positions, normals, indices)
    print(f"Generated {obj_path}", file=sys.stderr)

    # Build interleaved buffer from indexed mesh
    verts = []
    for i in indices:
        verts.append(np.concatenate([positions[i], normals[i]]))
    return np.array(verts, dtype="f4")


# ---------------------------------------------------------------------------
# Matrix math (numpy, no pyrr)
# ---------------------------------------------------------------------------

def perspective(fov_rad, aspect, near, far):
    f = 1.0 / np.tan(fov_rad / 2.0)
    m = np.zeros((4, 4), dtype="f4")
    m[0, 0] = f / aspect
    m[1, 1] = f
    m[2, 2] = (far + near) / (near - far)
    m[2, 3] = 2.0 * far * near / (near - far)
    m[3, 2] = -1.0
    return m


def look_at(eye, target, up):
    eye = np.array(eye, dtype="f4")
    target = np.array(target, dtype="f4")
    up = np.array(up, dtype="f4")

    f = target - eye
    f = f / np.linalg.norm(f)
    s = np.cross(f, up)
    s = s / np.linalg.norm(s)
    u = np.cross(s, f)

    m = np.eye(4, dtype="f4")
    m[0, :3] = s
    m[1, :3] = u
    m[2, :3] = -f
    m[0, 3] = -np.dot(s, eye)
    m[1, 3] = -np.dot(u, eye)
    m[2, 3] = np.dot(f, eye)
    return m


def rotate_y(angle_rad):
    c, s = np.cos(angle_rad), np.sin(angle_rad)
    m = np.eye(4, dtype="f4")
    m[0, 0] = c;  m[0, 2] = s
    m[2, 0] = -s; m[2, 2] = c
    return m


# ---------------------------------------------------------------------------
# Shaders
# ---------------------------------------------------------------------------

VERT_SRC = """
#version 330
in vec3 in_position;
in vec3 in_normal;

uniform mat4 u_mvp;
uniform mat4 u_model;
uniform mat3 u_normal_mat;

out vec3 v_normal;
out vec3 v_pos;

void main() {
    v_normal = u_normal_mat * in_normal;
    v_pos = (u_model * vec4(in_position, 1.0)).xyz;
    gl_Position = u_mvp * vec4(in_position, 1.0);
}
"""

FRAG_SRC = """
#version 330
in vec3 v_normal;
in vec3 v_pos;
out vec4 out_color;

uniform vec3 u_light_dir;

void main() {
    vec3 n = normalize(v_normal);
    float diff = max(dot(n, normalize(u_light_dir)), 0.0);
    float amb = 0.15;
    vec3 color = vec3(0.8, 0.5, 0.2) * (amb + diff * 0.85);
    out_color = vec4(color, 1.0);
}
"""

# ---------------------------------------------------------------------------
# Renderer
# ---------------------------------------------------------------------------

class StereoTeapotRenderer:
    def __init__(self, ctx, width, height, ipd=0.065):
        self.ctx = ctx
        self.width = width
        self.height = height
        self.ipd = ipd
        self.angle = 0.0

        # Load mesh
        vertex_data = load_or_generate_mesh()
        self.num_verts = len(vertex_data)
        vbo = ctx.buffer(vertex_data.tobytes())

        # Shader program
        self.prog = ctx.program(vertex_shader=VERT_SRC, fragment_shader=FRAG_SRC)
        self.vao = ctx.vertex_array(
            self.prog,
            [(vbo, "3f 3f", "in_position", "in_normal")],
        )

        # Two FBOs for left/right eye
        self.left_tex = ctx.texture((width, height), 4)
        self.left_fbo = ctx.framebuffer(
            color_attachments=[self.left_tex],
            depth_attachment=ctx.depth_renderbuffer((width, height)),
        )
        self.right_tex = ctx.texture((width, height), 4)
        self.right_fbo = ctx.framebuffer(
            color_attachments=[self.right_tex],
            depth_attachment=ctx.depth_renderbuffer((width, height)),
        )

        # Projection matrix
        aspect = width / height
        self.proj = perspective(np.radians(45.0), aspect, 0.1, 100.0)

        # Light direction
        self.prog["u_light_dir"].value = (0.5, 0.8, 0.6)

    def render_eye(self, fbo, eye_offset):
        """Render the teapot from one eye position."""
        fbo.use()
        self.ctx.clear(0.1, 0.1, 0.12, 1.0)
        self.ctx.enable(moderngl.DEPTH_TEST)

        model = rotate_y(self.angle)
        eye = np.array([eye_offset, 0.5, 4.0], dtype="f4")
        view = look_at(eye, [0, 0, 0], [0, 1, 0])

        mvp = self.proj @ view @ model
        normal_mat = np.linalg.inv(model[:3, :3]).T

        self.prog["u_mvp"].write(mvp.T.astype("f4").tobytes())
        if "u_model" in self.prog:
            self.prog["u_model"].write(model.T.astype("f4").tobytes())
        if "u_normal_mat" in self.prog:
            self.prog["u_normal_mat"].write(normal_mat.T.astype("f4").tobytes())

        self.vao.render()
        self.ctx.disable(moderngl.DEPTH_TEST)

    def render_frame(self):
        """Render both eyes. Returns hstacked raw RGBA bytes (top-left origin)."""
        half_ipd = self.ipd / 2.0
        self.render_eye(self.left_fbo, -half_ipd)
        self.render_eye(self.right_fbo, half_ipd)

        # Read FBOs
        left_data = np.frombuffer(self.left_fbo.read(components=4), dtype=np.uint8)
        right_data = np.frombuffer(self.right_fbo.read(components=4), dtype=np.uint8)

        left_img = left_data.reshape(self.height, self.width, 4)
        right_img = right_data.reshape(self.height, self.width, 4)

        # Flip vertically (OpenGL bottom-left → top-left)
        left_img = left_img[::-1]
        right_img = right_img[::-1]

        # Hstack: side-by-side stereo
        stereo = np.concatenate([left_img, right_img], axis=1)
        return stereo.tobytes()

    def advance(self, dt):
        """Advance animation (slow Y rotation)."""
        self.angle += 0.3 * dt  # ~0.3 rad/s ≈ 17 deg/s


# ---------------------------------------------------------------------------
# Modes
# ---------------------------------------------------------------------------

def run_preview(renderer, width, height, fps):
    """Interactive pygame preview window."""
    import pygame

    out_w = width * 2  # side-by-side
    pygame.init()
    screen = pygame.display.set_mode((out_w, height))
    pygame.display.set_caption("Stereo Teapot Preview")
    clock = pygame.time.Clock()

    running = True
    while running:
        for ev in pygame.event.get():
            if ev.type == pygame.QUIT:
                running = False
            elif ev.type == pygame.KEYDOWN and ev.key in (pygame.K_q, pygame.K_ESCAPE):
                running = False

        frame_bytes = renderer.render_frame()
        renderer.advance(1.0 / fps)

        # Blit to pygame surface
        surf = pygame.image.frombuffer(frame_bytes, (out_w, height), "RGBA")
        screen.blit(surf, (0, 0))
        pygame.display.flip()
        clock.tick(fps)

    pygame.quit()


def run_pipe(renderer, fps, max_frames):
    """Write raw RGBA frames to stdout."""
    stdout = sys.stdout.buffer
    frame_time = 1.0 / fps
    frame = 0
    try:
        while max_frames is None or frame < max_frames:
            t0 = time.monotonic()
            frame_bytes = renderer.render_frame()
            renderer.advance(1.0 / fps)
            stdout.write(frame_bytes)
            stdout.flush()
            frame += 1
            elapsed = time.monotonic() - t0
            if elapsed < frame_time:
                time.sleep(frame_time - elapsed)
    except BrokenPipeError:
        pass
    print(f"Wrote {frame} frames ({renderer.width * 2}x{renderer.height} RGBA)", file=sys.stderr)


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------

def main():
    parser = argparse.ArgumentParser(description="Stereo teapot test source")
    parser.add_argument("--preview", action="store_true", help="Show pygame preview window")
    parser.add_argument("--width", type=int, default=640, help="Per-eye width (default 640)")
    parser.add_argument("--height", type=int, default=480, help="Per-eye height (default 480)")
    parser.add_argument("--ipd", type=float, default=0.065, help="Inter-pupillary distance (default 0.065)")
    parser.add_argument("--fps", type=int, default=30, help="Target framerate (default 30)")
    parser.add_argument("--frames", type=int, default=None, help="Max frames (default: unlimited)")
    parser.add_argument("--generate-obj", action="store_true", help="Generate teapot.obj and exit")
    args = parser.parse_args()

    if args.generate_obj:
        positions, normals, indices = generate_teapot_mesh()
        obj_path = Path(__file__).parent / "teapot.obj"
        save_obj(obj_path, positions, normals, indices)
        n_faces = len(indices) // 3
        print(f"Generated {obj_path} ({len(positions)} verts, {n_faces} faces)")
        return

    # Create GL context
    if args.preview:
        # Preview needs a standalone context — pygame window is just for display
        ctx = moderngl.create_context(standalone=True)
    else:
        ctx = moderngl.create_context(standalone=True)

    renderer = StereoTeapotRenderer(ctx, args.width, args.height, args.ipd)

    if args.preview:
        run_preview(renderer, args.width, args.height, args.fps)
    else:
        run_pipe(renderer, args.fps, args.frames)


if __name__ == "__main__":
    main()
