/*{
    "DESCRIPTION": "Diagonal Combs — after Viking Eggeling, Symphonie Diagonale (1924). Comb, harp and fan figures on a diagonal axis whose teeth are revealed and hidden in rhythm: built up tooth by tooth, occluded by a sweeping band, or alternating odd and even. Pulse pushes the reveal forward on a hit; Energy stretches the teeth.",
    "CREDIT": "wday, after Viking Eggeling",
    "ISFVSN": "2",
    "CATEGORIES": ["Generator"],
    "INPUTS": [
        { "NAME": "reveal", "LABEL": "Reveal", "TYPE": "long", "DEFAULT": 0,
          "VALUES": [0, 1, 2], "LABELS": ["Build", "Occlude", "Alternate"] },
        { "NAME": "profile", "LABEL": "Profile", "TYPE": "long", "DEFAULT": 0,
          "VALUES": [0, 1, 2], "LABELS": ["Comb", "Harp", "Fan"] },
        { "NAME": "figures", "LABEL": "Figures", "TYPE": "float", "DEFAULT": 3.0, "MIN": 1.0, "MAX": 6.0 },
        { "NAME": "teeth", "LABEL": "Teeth", "TYPE": "float", "DEFAULT": 14.0, "MIN": 3.0, "MAX": 40.0 },
        { "NAME": "tempo", "LABEL": "Tempo", "TYPE": "float", "DEFAULT": 0.2, "MIN": 0.0, "MAX": 2.0 },
        { "NAME": "phase", "LABEL": "Phase", "TYPE": "float", "DEFAULT": 0.0, "MIN": 0.0, "MAX": 1.0 },
        { "NAME": "pulse", "LABEL": "Pulse", "TYPE": "float", "DEFAULT": 0.0, "MIN": 0.0, "MAX": 1.0 },
        { "NAME": "energy", "LABEL": "Energy", "TYPE": "float", "DEFAULT": 0.4, "MIN": 0.0, "MAX": 1.0 },
        { "NAME": "growth", "LABEL": "Growth", "TYPE": "float", "DEFAULT": 0.0, "MIN": 0.0, "MAX": 1.0 },
        { "NAME": "angle", "LABEL": "Angle", "TYPE": "float", "DEFAULT": 0.125, "MIN": 0.0, "MAX": 1.0 },
        { "NAME": "toothLength", "LABEL": "Length", "TYPE": "float", "DEFAULT": 0.5, "MIN": 0.1, "MAX": 1.5 },
        { "NAME": "toothWidth", "LABEL": "Tooth Width", "TYPE": "float", "DEFAULT": 0.3, "MIN": 0.05, "MAX": 0.9 },
        { "NAME": "mirror", "LABEL": "Mirror", "TYPE": "bool", "DEFAULT": false },
        { "NAME": "invertGround", "LABEL": "White Ground", "TYPE": "bool", "DEFAULT": false },
        { "NAME": "softness", "LABEL": "Softness", "TYPE": "float", "DEFAULT": 0.0015, "MIN": 0.0, "MAX": 0.05 },
        { "NAME": "seed", "LABEL": "Seed", "TYPE": "float", "DEFAULT": 3.0, "MIN": 0.0, "MAX": 100.0 }
    ]
}*/

// Eggeling drew his figures on scrolls and animated them by addition and subtraction:
// a comb gains a tooth, a harp gains a string, then they are taken away again, all
// ranged along a diagonal. Here each figure is a spine with perpendicular teeth. The
// tooth under a pixel is found by division, not by looping over teeth, so a figure
// costs the same with 3 teeth or 40. Growth 0 snaps each tooth in whole (his stepped
// frame-by-frame additions); Growth 1 lets the newest tooth grow out smoothly.

const int MAX_FIGURES = 6;

float hash(float n) { return fract(sin(n * 12.9898 + seed * 78.233) * 43758.5453); }

float sdBox(vec2 p, vec2 b)
{
    vec2 d = abs(p) - b;
    return length(max(d, 0.0)) + min(max(d.x, d.y), 0.0);
}

vec2 rotate(vec2 p, float a)
{
    float c = cos(a), s = sin(a);
    return vec2(c * p.x - s * p.y, s * p.x + c * p.y);
}

// How much of tooth i (of n) shows at this point in the figure's cycle, 0..1.
float visibility(float i, float n, float life)
{
    if (reveal == 0)
    {
        // Build: the first half of the cycle adds teeth along the spine, the second
        // half removes them in the same order. Pulse throws the build forward.
        float grown = life < 0.5 ? life * 2.0 * n + pulse * n * 0.5 : n;
        float gone  = life < 0.5 ? 0.0 : (life - 0.5) * 2.0 * n;
        if (gone - i >= 1.0)
            return 0.0;
        float part = clamp(grown - i, 0.0, 1.0);   // how much of this tooth has grown
        return mix(step(0.001, part), part, growth);
    }
    if (reveal == 1)
    {
        // Occlude: a band sweeps along the spine and fingers pass behind it.
        float band = mix(-0.3, 1.3, life) * n;
        float halfWidth = n * (0.18 + 0.12 * pulse);
        return step(halfWidth, abs(i + 0.5 - band));
    }
    // Alternate: odd and even teeth trade places each step; Pulse flips early.
    float k = floor(life * 8.0) + step(0.5, pulse);
    return step(0.5, mod(i + k, 2.0));
}

void main()
{
    float aspectRatio = RENDERSIZE.x / RENDERSIZE.y;
    vec2 p = (isf_FragNormCoord - 0.5) * vec2(aspectRatio, 1.0);
    float axis = angle * 6.2831853;
    float aa = max(softness, 1.5 / RENDERSIZE.y);

    float nf = floor(figures + 0.5);
    float nt = floor(teeth + 0.5);
    float t  = TIME * tempo + phase;
    float v  = 0.0;

    for (int f = 0; f < MAX_FIGURES; ++f)
    {
        float ff = float(f);
        if (ff >= nf)
            break;

        // Figures are ranged along the diagonal, alternating sides of it.
        float along = (ff + 0.5) / nf - 0.5;
        float side  = (mod(ff, 2.0) < 1.0 ? -1.0 : 1.0) * (0.08 + 0.12 * hash(ff + 5.0));
        float size  = 0.35 + 0.35 * hash(ff + 11.0);
        // Centres run along the frame's own diagonal whatever the figures' angle, so a
        // row of figures stays inside the picture; side pushes each off the line.
        vec2 diag = normalize(vec2(aspectRatio, 1.0));
        vec2 centre = vec2(along * 0.55 * aspectRatio, along * 0.4) + vec2(-diag.y, diag.x) * side;
        // Teeth grow to one side of the spine, so shift the figure back by half its
        // tooth length to keep it centred where it was placed (mirrored figures are).
        if (!mirror)
            centre -= rotate(vec2(0.0, 1.0), axis) * toothLength * size * 0.5;

        // Local frame: spine along x centred on the figure, teeth along +y (and -y mirrored).
        vec2 q = rotate(p - centre, -axis);
        float spineLen = (0.5 + 0.3 * hash(ff + 23.0)) * aspectRatio / (0.9 + 0.45 * nf);
        q.x += spineLen * 0.5;
        float spacing = spineLen / nt;
        float life = fract(t + ff / nf);

        // Which tooth is under this pixel.
        float i = floor(q.x / spacing);
        float cover = 0.0;
        if (i >= 0.0 && i < nt)
        {
            float u = (i + 0.5) / nt;
            float prof = profile == 0 ? 1.0
                       : profile == 1 ? 0.25 + 0.75 * u
                       : sin(3.14159265 * u);
            float len = toothLength * size * prof * (0.6 + 0.8 * energy);
            float vis = visibility(i, nt, life);
            len *= vis;   // a partly grown tooth is a shorter tooth

            float xc = (i + 0.5) * spacing;
            float w  = spacing * toothWidth * 0.5;
            float y  = mirror ? abs(q.y) : q.y;
            float d  = sdBox(vec2(q.x - xc, y - len * 0.5), vec2(w, len * 0.5));
            cover = (1.0 - smoothstep(-aa, aa, d)) * step(0.001, vis);
        }

        // The spine shows as far as the figure is currently revealed.
        float reach = 0.0;
        if (reveal == 0)
            reach = life < 0.5 ? clamp(life * 2.0 + pulse * 0.5, 0.0, 1.0) : 1.0 - (life - 0.5) * 2.0;
        else
            reach = 1.0;
        float x0 = (reveal == 0 && life >= 0.5) ? spineLen * (1.0 - reach) : 0.0;
        float x1 = (reveal == 0 && life < 0.5) ? spineLen * reach : spineLen;
        float sw = spacing * toothWidth * 0.35;
        float ds = sdBox(vec2(q.x - (x0 + x1) * 0.5, q.y), vec2(max(x1 - x0, 0.0) * 0.5, sw));
        cover = max(cover, (1.0 - smoothstep(-aa, aa, ds)) * step(0.0001, x1 - x0));

        v = max(v, cover);
    }

    if (invertGround)
        v = 1.0 - v;

    gl_FragColor = vec4(vec3(v), 1.0);
}
