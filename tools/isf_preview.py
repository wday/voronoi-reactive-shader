# /// script
# requires-python = ">=3.12"
# dependencies = ["moderngl>=5.12", "numpy", "pillow"]
# ///
"""Headless ISF preview: wraps an ISF generator the way hosts do and renders a contact sheet.
usage: isf_preview.py shader.fs out.png '{"mode":0}' '{"mode":1,"pulse":1}' ... [--times 0,1,2]"""
import json, re, sys
import moderngl, numpy as np
from PIL import Image, ImageDraw

path, out, *specs = sys.argv[1:]
times = [0.0, 0.9, 1.7, 2.6]
if specs and specs[-1].startswith('--times'):
    times = [float(x) for x in specs.pop().split('=',1)[1].split(',')]
src = open(path).read()
meta = json.loads(re.search(r'/\*(\{.*?\})\*/', src, re.S).group(1))
body = src[src.index('*/') + 2:]
decl = []
for i in meta['INPUTS']:
    t = {'float':'float','long':'int','bool':'bool','color':'vec4','point2D':'vec2'}[i['TYPE']]
    decl.append(f"uniform {t} {i['NAME']};")
frag = "#version 410\nuniform float TIME; uniform vec2 RENDERSIZE; in vec2 isf_FragNormCoord;\nout vec4 isf_out;\n#define gl_FragColor isf_out\n" + "\n".join(decl) + "\n" + body
vert = "#version 410\nin vec2 pos; out vec2 isf_FragNormCoord; void main(){ isf_FragNormCoord = pos*0.5+0.5; gl_Position = vec4(pos,0,1); }"
W, H = 320, 180
ctx = moderngl.create_standalone_context(require=410)
prog = ctx.program(vertex_shader=vert, fragment_shader=frag)
vbo = ctx.buffer(np.array([-1,-1, 1,-1, -1,1, 1,1], 'f4').tobytes())
vao = ctx.vertex_array(prog, [(vbo, '2f', 'pos')])
fbo = ctx.simple_framebuffer((W, H)); fbo.use()
defaults = {i['NAME']: i.get('DEFAULT', 0) for i in meta['INPUTS']}
rows = specs or ['{}']
sheet = Image.new('RGB', (W * len(times) + 4 * len(times), (H + 18) * len(rows)), (60, 0, 60))
draw = ImageDraw.Draw(sheet)
for r, spec in enumerate(rows):
    vals = {**defaults, **json.loads(spec)}
    for c, t in enumerate(times):
        for k, v in vals.items():
            if k in prog: prog[k].value = int(v) if isinstance(v, bool) else v
        prog['TIME'].value = t; prog['RENDERSIZE'].value = (W, H)
        fbo.clear(); vao.render(moderngl.TRIANGLE_STRIP)
        img = Image.frombytes('RGB', (W, H), fbo.read(components=3)).transpose(Image.FLIP_TOP_BOTTOM)
        sheet.paste(img, (c * (W + 4), r * (H + 18) + 18))
    draw.text((4, r * (H + 18) + 3), spec + f"   t={times}", fill=(255, 255, 255))
sheet.save(out); print('wrote', out)
