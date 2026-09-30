# /// script
# requires-python = ">=3.12"
# dependencies = ["moderngl>=5.12", "numpy", "pillow"]
# ///
"""Headless ISF preview: wraps an ISF generator the way hosts do and renders a contact sheet.
usage: isf_preview.py shader.fs out.png '{"mode":0}' '{"mode":1,"pulse":1}' ... [--times 0,1,2] [--fps 60]

A value may be a timeline, [[time, value], ...]: it holds each value from its time on,
e.g. '{"seed":[[0,3],[1.5,7]]}' changes Seed at 1.5 s.
Shaders with PASSES run frame by frame from time 0 at --fps, so PERSISTENT targets
build up state the way they do in a host (TIMEDELTA, FRAMEINDEX, PASSINDEX are set).

Effects and debugging:
  --base='{"dt":1.7}'   params shared by every row (rows override; labels show only the row)
  --size=480x270        render size (default 320x180); aspect matters for flow shaders
  --input=spiral        feed image INPUTS: spiral (built-in, isotropic in pixels) or a .png path
  --seed=velA:1         fill a PERSISTENT target's RG with uniform noise in [-1,1]*amp at start,
                        like a host that hands over uncleared texture memory
  --view=velA:velocity  show a target instead of the output: velocity (hue = direction,
                        brightness = |RG|/3) or rgb (raw)"""
import json, re, sys
import moderngl, numpy as np
from PIL import Image, ImageDraw

path, out, *specs = sys.argv[1:]
times = [0.0, 0.9, 1.7, 2.6]
fps = 60.0
W, H = 320, 180
input_src, seeds, view, base = None, {}, None, {}
for flag in [s for s in specs if s.startswith('--')]:
    specs.remove(flag)
    key, val = flag[2:].split('=', 1)
    if key == 'times': times = [float(x) for x in val.split(',')]
    elif key == 'fps': fps = float(val)
    elif key == 'base': base = json.loads(val)
    elif key == 'size': W, H = (int(x) for x in val.split('x'))
    elif key == 'input': input_src = val
    elif key == 'seed': name, amp = val.split(':'); seeds[name] = float(amp)
    elif key == 'view': view = tuple(val.split(':'))
src = open(path).read()
meta = json.loads(re.search(r'/\*(\{.*?\})\*/', src, re.S).group(1))
body = src[src.index('*/') + 2:]
passes = meta.get('PASSES') or [{}]
decl = []
for i in meta['INPUTS']:
    t = {'float':'float','long':'int','bool':'bool','color':'vec4','point2D':'vec2','image':'sampler2D','event':'bool'}[i['TYPE']]
    decl.append(f"uniform {t} {i['NAME']};")
decl += [f"uniform sampler2D {p['TARGET']};" for p in passes if 'TARGET' in p]
frag = ("#version 410\nuniform float TIME; uniform float TIMEDELTA; uniform int PASSINDEX; uniform int FRAMEINDEX;\n"
        "uniform vec2 RENDERSIZE; in vec2 isf_FragNormCoord;\nout vec4 isf_out;\n#define gl_FragColor isf_out\n"
        "#define IMG_NORM_PIXEL(i, uv) texture(i, uv)\n#define IMG_SIZE(i) vec2(textureSize(i, 0))\n"
        "#define IMG_PIXEL(i, c) texture(i, (c) / IMG_SIZE(i))\n" + "\n".join(decl) + "\n" + body)
vert = "#version 410\nin vec2 pos; out vec2 isf_FragNormCoord; void main(){ isf_FragNormCoord = pos*0.5+0.5; gl_Position = vec4(pos,0,1); }"
ctx = moderngl.create_standalone_context(require=410)
prog = ctx.program(vertex_shader=vert, fragment_shader=frag)
vbo = ctx.buffer(np.array([-1,-1, 1,-1, -1,1, 1,1], 'f4').tobytes())
vao = ctx.vertex_array(prog, [(vbo, '2f', 'pos')])
fbo = ctx.simple_framebuffer((W, H))
defaults = {i['NAME']: i.get('DEFAULT', 0) for i in meta['INPUTS'] if i['TYPE'] != 'image'}
images = [i['NAME'] for i in meta['INPUTS'] if i['TYPE'] == 'image']

def input_texture():
    if input_src is None or input_src == 'spiral':
        y, x = np.mgrid[0:H, 0:W].astype('f4')
        cx, cy = (x - W / 2) / H, (H / 2 - y) / H
        lum = 0.5 + 0.5 * np.sin(np.hypot(cx, cy) * 40 - np.arctan2(cy, cx) * 2)
        rgba = np.stack([lum] * 3 + [np.ones_like(lum)], -1)
    else:
        rgba = np.asarray(Image.open(input_src).convert('RGBA').resize((W, H)), 'f4') / 255
    return ctx.texture((W, H), 4, np.ascontiguousarray(rgba[::-1]).astype('f4').tobytes(), dtype='f4')
input_tex = input_texture() if images else None

view_prog = ctx.program(vertex_shader=vert, fragment_shader="""#version 410
uniform sampler2D tex; uniform int mode; in vec2 isf_FragNormCoord; out vec4 o;
void main() {
    vec4 c = texture(tex, isf_FragNormCoord);
    if (mode == 0) { o = vec4(c.rgb, 1.0); return; }
    float h = fract(atan(c.g, c.r) / 6.28318530718);
    vec3 k = clamp(abs(fract(h + vec3(0.0, 2.0, 1.0) / 3.0) * 6.0 - 3.0) - 1.0, 0.0, 1.0);
    o = vec4(mix(vec3(1.0), k, 0.9) * clamp(length(c.rg) / 3.0, 0.0, 1.0), 1.0);
}""")
view_vao = ctx.vertex_array(view_prog, [(vbo, '2f', 'pos')])

def size_of(p):
    dim = lambda k, full: int(eval(str(p.get(k, full)).replace('$WIDTH', str(W)).replace('$HEIGHT', str(H))))
    return dim('WIDTH', W), dim('HEIGHT', H)

def at(v, t):
    if isinstance(v, list) and v and isinstance(v[0], list):
        return [x for when, x in v if when <= t + 1e-9][-1]
    return v

class Target:   # double-buffered, so a pass can read its own last frame while it writes
    def __init__(self, p):
        self.size = size_of(p)
        dtype = 'f4' if p.get('FLOAT') else 'f1'
        self.tex = [ctx.texture(self.size, 4, dtype=dtype) for _ in range(2)]
        self.fbo = [ctx.framebuffer(color_attachments=[t]) for t in self.tex]
        for f in self.fbo: f.clear()
        self.front = 0
        if p['TARGET'] in seeds:
            w, h = self.size
            noise = np.random.default_rng(1).uniform(-1, 1, (h, w, 4)).astype('f4') * seeds[p['TARGET']]
            noise[..., 2:] = 0
            self.tex[0].write(noise.tobytes() if dtype == 'f4'
                              else (np.clip(noise * 0.5 + 0.5, 0, 1) * 255).astype('u1').tobytes())

def frame(vals, t, dt, index, targets):
    for k, v in vals.items():
        v = at(v, t)
        if k in prog: prog[k].value = int(v) if isinstance(v, bool) else v
    for k, v in [('TIME', t), ('TIMEDELTA', dt), ('FRAMEINDEX', index)]:
        if k in prog: prog[k].value = v
    for n, p in enumerate(passes):
        for unit, (name, tg) in enumerate(targets.items()):
            tg.tex[tg.front].use(unit)
            if name in prog: prog[name].value = unit
        for unit, name in enumerate(images, len(targets)):
            input_tex.use(unit)
            if name in prog: prog[name].value = unit
        if 'PASSINDEX' in prog: prog['PASSINDEX'].value = n
        tg = targets.get(p.get('TARGET'))
        dest, size = (tg.fbo[1 - tg.front], tg.size) if tg else (fbo, (W, H))
        if 'RENDERSIZE' in prog: prog['RENDERSIZE'].value = size
        dest.use(); dest.clear(); vao.render(moderngl.TRIANGLE_STRIP)
        if tg: tg.front = 1 - tg.front

def grab():
    return Image.frombytes('RGB', (W, H), fbo.read(components=3)).transpose(Image.FLIP_TOP_BOTTOM)

rows = specs or ['{}']
sheet = Image.new('RGB', (W * len(times) + 4 * len(times), (H + 18) * len(rows)), (60, 0, 60))
draw = ImageDraw.Draw(sheet)
for r, spec in enumerate(rows):
    vals = {**defaults, **base, **json.loads(spec)}
    shots = []
    if 'PASSES' in meta:
        targets = {p['TARGET']: Target(p) for p in passes if 'TARGET' in p}
        want, index, t = sorted(times), 0, 0.0
        while want:
            frame(vals, t, 0.0 if index == 0 else 1.0 / fps, index, targets)
            while want and want[0] <= t + 0.5 / fps:
                if view:
                    fbo.use(); targets[view[0]].tex[targets[view[0]].front].use(0)
                    view_prog['tex'].value = 0
                    view_prog['mode'].value = 1 if view[1:] == ('velocity',) else 0
                    view_vao.render(moderngl.TRIANGLE_STRIP)
                shots.append((want.pop(0), grab()))
            index += 1; t = index / fps
        shots = [img for _, img in sorted(shots, key=lambda s: times.index(s[0]))]
    else:
        for t in times:
            frame(vals, t, 0.0, 0, {}); shots.append(grab())
    for c, img in enumerate(shots):
        sheet.paste(img, (c * (W + 4), r * (H + 18) + 18))
    draw.text((4, r * (H + 18) + 3), spec + f"   t={times}", fill=(255, 255, 255))
sheet.save(out); print('wrote', out)
