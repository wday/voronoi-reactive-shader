"""Compile + run shaders/wavefolder.fs through an ISF shim, verify fold behavior."""
import re, sys
from pathlib import Path
import numpy as np
import moderngl

FS = Path("/home/alien/dev/voronoi-reactive-shader/shaders/wavefolder.fs").read_text()
# strip the /*{ ... }*/ ISF header
body = re.sub(r"/\*\{.*?\}\*/", "", FS, count=1, flags=re.DOTALL).strip()

SHIM = """#version 330
in vec2 v_uv;
out vec4 out_color;
#define gl_FragColor out_color
uniform sampler2D inputImage;
uniform vec2 RENDERSIZE;
uniform float drive, stages, shape, bias, wet;
#define isf_FragNormCoord v_uv
#define IMG_THIS_PIXEL(img) texture(img, v_uv)
#define IMG_NORM_PIXEL(img, uv) texture(img, uv)
"""
frag = SHIM + "\n" + body

VERT = """#version 330
in vec2 position; in vec2 texcoord; out vec2 v_uv;
void main(){ v_uv = texcoord; gl_Position = vec4(position,0,1);}"""

W, H = 256, 64
ctx = moderngl.create_standalone_context()
try:
    prog = ctx.program(vertex_shader=VERT, fragment_shader=frag)
except Exception as e:
    print("SHADER COMPILE FAILED:\n", e); sys.exit(1)
print("compiled OK")

# bright horizontal ramp 0..1 in all channels (drive pushes it over 1 in-shader)
ramp = np.linspace(0, 1, W, dtype="f4")
src = np.stack([np.tile(ramp, (H, 1))] * 3 + [np.ones((H, W), "f4")], axis=-1)
tex = ctx.texture((W, H), 4, src.tobytes(), dtype="f4")
tex.use(0)

verts = np.array([-1,-1,0,0, 1,-1,1,0, -1,1,0,1, 1,1,1,1], "f4")
vbo = ctx.buffer(verts.tobytes())
vao = ctx.vertex_array(prog, [(vbo, "2f 2f", "position", "texcoord")])
out_tex = ctx.texture((W, H), 4, dtype="f4")
fbo = ctx.framebuffer(color_attachments=[out_tex])

def render(**u):
    for k, v in u.items():
        if k in prog: prog[k].value = v
    if "inputImage" in prog: prog["inputImage"].value = 0
    if "RENDERSIZE" in prog: prog["RENDERSIZE"].value = (W, H)
    fbo.use(); ctx.clear()
    vao.render(moderngl.TRIANGLE_STRIP)
    buf = np.frombuffer(fbo.read(components=4, dtype="f4"), "f4").reshape(H, W, 4)
    return buf[H // 2]  # middle scanline

# --- Test 1: wet=0 is passthrough ---
row = render(drive=1.8, stages=2.0, shape=0.0, bias=0.0, wet=0.0)
passthrough_err = np.abs(row[:, 0] - ramp).max()
print(f"[wet=0 passthrough] max |out-in| = {passthrough_err:.6f}  -> {'PASS' if passthrough_err < 1e-4 else 'FAIL'}")

# --- Test 2: triangle identity below fold point (drive=1, no over-range) ---
row = render(drive=1.0, stages=4.0, shape=0.0, bias=0.0, wet=1.0)
ident_err = np.abs(row[:, 0] - ramp).max()
print(f"[triangle identity, drive=1] max |out-in| = {ident_err:.6f}  -> {'PASS' if ident_err < 1e-3 else 'FAIL'}")

# --- Test 3: blowout control — output stays in [0,1], and folds (non-monotonic) ---
row = render(drive=4.0, stages=3.0, shape=0.0, bias=0.0, wet=1.0)
r = row[:, 0]
in_range = (r.min() >= -1e-4) and (r.max() <= 1.0 + 1e-4)
# count sign changes in the derivative -> folds create non-monotonic banding
d = np.diff(r)
sign_changes = int(np.sum(np.abs(np.diff(np.sign(d[np.abs(d) > 1e-4]))) > 0))
print(f"[drive=4 fold] out range [{r.min():.3f},{r.max():.3f}] in[0,1]={in_range}, direction reversals={sign_changes} -> {'PASS' if in_range and sign_changes >= 2 else 'FAIL'}")

# --- Test 4: wrap mode (shape=1) also bounded ---
row = render(drive=4.0, stages=3.0, shape=1.0, bias=0.0, wet=1.0)
r = row[:, 0]
print(f"[wrap shape=1] out range [{r.min():.3f},{r.max():.3f}] -> {'PASS' if r.max() <= 1.0+1e-4 and r.min() >= -1e-4 else 'FAIL'}")

# --- Test 5: per-channel color from neutral blowout ---
# feed neutral grey 0.9 everywhere, high drive+bias asymmetry shouldn't matter;
# instead feed a neutral value and confirm channels stay equal (grey in -> grey out),
# then a slightly colored input diverges per channel.
def render_const(c, **u):
    s = np.zeros((H, W, 4), "f4"); s[..., :3] = c; s[..., 3] = 1
    t = ctx.texture((W, H), 4, s.tobytes(), dtype="f4"); t.use(0)
    return render(**u)[W // 2]
grey = render_const((0.8, 0.8, 0.8), drive=3.0, stages=2.0, shape=0.0, bias=0.0, wet=1.0)
color = render_const((0.8, 0.6, 0.4), drive=3.0, stages=2.0, shape=0.0, bias=0.0, wet=1.0)
print(f"[grey 0.8 in] out rgb = {grey[:3]}  (channels equal = {np.ptp(grey[:3]) < 1e-4})")
print(f"[color in]    out rgb = {color[:3]}  (channels diverge = {np.ptp(color[:3]) > 1e-3})")
