/*{
    "DESCRIPTION": "Motion Studies — Richter-style rhythm of simple primitives (after Hans Richter, Rhythmus 21/23). N rectangles advance and recede, slide in from the edges, scatter and pulse, or sweep as lines; overlaps paint in grey steps or invert figure and ground. One generator in place of a stack of N+2 layers. Pulse and Energy are for audio: Pulse kicks every box outward on a hit, Energy widens the motion.",
    "CREDIT": "wday, after Hans Richter",
    "ISFVSN": "2",
    "CATEGORIES": ["Generator"],
    "INPUTS": [
        { "NAME": "mode", "LABEL": "Mode", "TYPE": "long", "DEFAULT": 0,
          "VALUES": [0, 1, 2, 3], "LABELS": ["Advance", "Slide", "Scatter", "Lines"] },
        { "NAME": "overlap", "LABEL": "Overlap", "TYPE": "long", "DEFAULT": 1,
          "VALUES": [0, 1], "LABELS": ["Paint", "Invert"] },
        { "NAME": "count", "LABEL": "Count", "TYPE": "float", "DEFAULT": 6.0, "MIN": 1.0, "MAX": 16.0 },
        { "NAME": "tempo", "LABEL": "Tempo", "TYPE": "float", "DEFAULT": 0.25, "MIN": 0.0, "MAX": 2.0 },
        { "NAME": "phase", "LABEL": "Phase", "TYPE": "float", "DEFAULT": 0.0, "MIN": 0.0, "MAX": 1.0 },
        { "NAME": "pulse", "LABEL": "Pulse", "TYPE": "float", "DEFAULT": 0.0, "MIN": 0.0, "MAX": 1.0 },
        { "NAME": "energy", "LABEL": "Energy", "TYPE": "float", "DEFAULT": 0.3, "MIN": 0.0, "MAX": 1.0 },
        { "NAME": "spread", "LABEL": "Spread", "TYPE": "float", "DEFAULT": 0.0, "MIN": 0.0, "MAX": 1.0 },
        { "NAME": "boxAspect", "LABEL": "Aspect", "TYPE": "float", "DEFAULT": 1.0, "MIN": 0.2, "MAX": 5.0 },
        { "NAME": "thickness", "LABEL": "Line Width", "TYPE": "float", "DEFAULT": 0.04, "MIN": 0.002, "MAX": 0.3 },
        { "NAME": "angle", "LABEL": "Angle", "TYPE": "float", "DEFAULT": 0.0, "MIN": 0.0, "MAX": 1.0 },
        { "NAME": "tones", "LABEL": "Grey Steps", "TYPE": "float", "DEFAULT": 3.0, "MIN": 1.0, "MAX": 8.0 },
        { "NAME": "softness", "LABEL": "Softness", "TYPE": "float", "DEFAULT": 0.002, "MIN": 0.0, "MAX": 0.1 },
        { "NAME": "invertGround", "LABEL": "White Ground", "TYPE": "bool", "DEFAULT": false },
        { "NAME": "seed", "LABEL": "Seed", "TYPE": "float", "DEFAULT": 1.0, "MIN": 0.0, "MAX": 100.0 }
    ]
}*/

// Richter's films are orthogonal, monochrome, and about the frame itself: squares
// that advance toward the viewer and recede, bars that wipe across, and overlaps where
// black on white turns into white on black. Each box below is one loop iteration,
// drawn back to front. In Invert mode coverage is counted and its parity decides the
// colour, so any overlap flips figure and ground the way the films do.

const int MAX_BOXES = 16;

float hash(float n) { return fract(sin(n * 12.9898 + seed * 78.233) * 43758.5453); }

// Signed distance to an axis-aligned box of hs-size b centred on the origin.
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

void main()
{
    float aspectRatio = RENDERSIZE.x / RENDERSIZE.y;
    vec2 p = (isf_FragNormCoord - 0.5) * vec2(aspectRatio, 1.0);
    // Angle turns the whole frame: 0.125 of a turn is Rhythmus 23's diagonal.
    p = rotate(p, angle * 6.2831853);

    float n    = floor(count + 0.5);
    float t    = TIME * tempo + phase;
    float kick = 1.0 + pulse * 0.6;
    float aa   = max(softness, 1.5 / RENDERSIZE.y);

    float painted = 0.0;   // Paint: grey of the front-most box
    float front   = 1e9;   // Paint: area of the box currently in front
    float parity  = 0.0;   // Invert: number of boxes covering this pixel

    for (int i = 0; i < MAX_BOXES; ++i)
    {
        float fi = float(i);
        if (fi >= n)
            break;

        float h1 = hash(fi + 1.0);
        float h2 = hash(fi + 17.0);
        float h3 = hash(fi + 43.0);
        float life = fract(t + fi / n);          // each box's place in its cycle

        vec2 centre = (vec2(h1, h2) - 0.5) * spread * vec2(aspectRatio, 1.0);
        vec2 hs;

        if (mode == 0)
        {
            // Advance: nested squares grow from the centre toward the viewer, then
            // recede. Energy deepens the travel; Pulse throws them all forward.
            float s = mix(0.02, 0.35 + 0.6 * energy, life) * kick;
            hs = vec2(s * boxAspect, s);
        }
        else if (mode == 1)
        {
            // Slide: full-length bars wipe in from alternating edges.
            bool vertical = mod(fi, 2.0) < 1.0;
            float travel = (life * 2.0 - 1.0) * (0.6 + 0.6 * energy);
            float w = (0.03 + 0.12 * h3) * kick;
            if (vertical) { centre = vec2(travel * aspectRatio, centre.y); hs = vec2(w, 1.0); }
            else          { centre = vec2(centre.x, travel);               hs = vec2(aspectRatio, w); }
        }
        else if (mode == 2)
        {
            // Scatter: rectangles fixed in place that breathe out of phase.
            centre = (vec2(h1, h2) - 0.5) * (0.4 + 0.6 * spread) * vec2(aspectRatio, 1.0);
            float s = (0.05 + 0.2 * h3) * (0.6 + 0.8 * energy * (0.5 + 0.5 * sin(6.2831853 * life))) * kick;
            hs = vec2(s * boxAspect, s);
        }
        else
        {
            // Lines: thin rules sweeping the frame, horizontal and vertical.
            bool vertical = h3 > 0.5;
            float pos = (fract(life + h1) * 2.0 - 1.0) * 0.5;
            float w = thickness * kick * (0.5 + energy);
            if (vertical) { centre = vec2(pos * aspectRatio, 0.0); hs = vec2(w, 1.0); }
            else          { centre = vec2(0.0, pos);               hs = vec2(aspectRatio, w); }
        }

        float cover = 1.0 - smoothstep(-aa, aa, sdBox(p - centre, hs));

        // Grey steps, lightest in front: quantised so the palette stays Richter's.
        float steps = max(floor(tones + 0.5), 1.0);
        float grey = steps <= 1.0 ? 1.0 : floor(h2 * steps) / (steps - 1.0);
        grey = mix(0.25, 1.0, grey);

        // Paint: smaller boxes sit in front of larger ones, as Richter's advancing
        // squares do, whatever order the loop meets them in.
        float area = hs.x * hs.y;
        if (area < front)
        {
            painted = mix(painted, grey, cover);
            if (cover > 0.5)
                front = area;
        }
        parity += cover;
    }

    float v = (overlap == 0) ? painted : abs(mod(parity + 1.0, 2.0) - 1.0);
    if (invertGround)
        v = 1.0 - v;

    gl_FragColor = vec4(vec3(v), 1.0);
}
