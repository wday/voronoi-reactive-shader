// demos.js — shared WebGL boilerplate + per-demo initialization

(function () {
'use strict';

// ---- Utilities ----

function fract(x) { return x - Math.floor(x); }

function hash2JS(px, py) {
    // Dave Hoskins no-sine hash (vec2 output)
    var p3x = fract(px * 0.1031);
    var p3y = fract(py * 0.1030);
    var p3z = fract(px * 0.0973);
    var d = p3x * (p3y + 33.33) + p3y * (p3z + 33.33) + p3z * (p3x + 33.33);
    p3x += d; p3y += d; p3z += d;
    return [fract((p3x + p3y) * p3z), fract((p3x + p3z) * p3y)];
}

function sineHash2JS(px, py) {
    return [
        fract(Math.sin(px * 12.9898 + py * 78.233) * 43758.5453),
        fract(Math.sin(px * 78.233 + py * 12.9898) * 43758.5453)
    ];
}

// ---- WebGL boilerplate ----

var VERT_SRC = [
    'attribute vec2 a_position;',
    'varying vec2 v_uv;',
    'void main() {',
    '    v_uv = a_position * 0.5 + 0.5;',
    '    gl_Position = vec4(a_position, 0.0, 1.0);',
    '}'
].join('\n');

function createGLDemo(canvas, fragSrc, uniformDefs) {
    var gl = canvas.getContext('webgl', { antialias: false, alpha: false });
    if (!gl) return null;

    function compile(type, src) {
        var s = gl.createShader(type);
        gl.shaderSource(s, src);
        gl.compileShader(s);
        if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) {
            console.error(gl.getShaderInfoLog(s));
            return null;
        }
        return s;
    }

    var vs = compile(gl.VERTEX_SHADER, VERT_SRC);
    var fs = compile(gl.FRAGMENT_SHADER, 'precision highp float;\n' + fragSrc);
    if (!vs || !fs) return null;

    var prog = gl.createProgram();
    gl.attachShader(prog, vs);
    gl.attachShader(prog, fs);
    gl.linkProgram(prog);
    if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) {
        console.error(gl.getProgramInfoLog(prog));
        return null;
    }
    gl.useProgram(prog);

    // Fullscreen quad
    var buf = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, buf);
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([
        -1, -1,  1, -1,  -1, 1,
        -1,  1,  1, -1,   1, 1
    ]), gl.STATIC_DRAW);
    var posLoc = gl.getAttribLocation(prog, 'a_position');
    gl.enableVertexAttribArray(posLoc);
    gl.vertexAttribPointer(posLoc, 2, gl.FLOAT, false, 0, 0);

    // Uniforms
    var uTime = gl.getUniformLocation(prog, 'u_time');
    var uRes  = gl.getUniformLocation(prog, 'u_resolution');
    var uLocs = {}, uValues = {};
    for (var name in (uniformDefs || {})) {
        uLocs[name] = gl.getUniformLocation(prog, name);
        uValues[name] = uniformDefs[name];
    }

    var elapsed = 0, lastFrame = null, animId = null;

    function frame(now) {
        if (lastFrame !== null) elapsed += (now - lastFrame) / 1000;
        lastFrame = now;

        var dpr = window.devicePixelRatio || 1;
        var w = Math.round(canvas.clientWidth * dpr);
        var h = Math.round(canvas.clientHeight * dpr);
        if (canvas.width !== w || canvas.height !== h) {
            canvas.width = w;
            canvas.height = h;
        }

        gl.viewport(0, 0, w, h);
        if (uTime) gl.uniform1f(uTime, elapsed);
        if (uRes)  gl.uniform2f(uRes, w, h);

        for (var n in uValues) {
            var loc = uLocs[n];
            if (loc) gl.uniform1f(loc, uValues[n]);
        }

        gl.drawArrays(gl.TRIANGLES, 0, 6);
        animId = requestAnimationFrame(frame);
    }

    // Only animate when visible
    var obs = new IntersectionObserver(function (entries) {
        if (entries[0].isIntersecting) {
            if (!animId) { lastFrame = null; animId = requestAnimationFrame(frame); }
        } else {
            if (animId) { cancelAnimationFrame(animId); animId = null; }
        }
    }, { threshold: 0.05 });
    obs.observe(canvas);

    return {
        set: function (name, val) { uValues[name] = val; },
        get: function (name) { return uValues[name]; }
    };
}

// ---- Control wiring ----

function wireSlider(container, uniformName, demo) {
    var slider = container.querySelector('[data-uniform="' + uniformName + '"]');
    if (!slider || !demo) return;
    var label = slider.closest('label');
    var span = label ? label.querySelector('.value') : null;
    slider.addEventListener('input', function () {
        var val = parseFloat(slider.value);
        demo.set(uniformName, val);
        if (span) span.textContent = val.toFixed(2);
    });
}

function wireButtons(container, uniformName, demo) {
    var buttons = container.querySelectorAll('[data-uniform="' + uniformName + '"]');
    buttons.forEach(function (btn) {
        btn.addEventListener('click', function () {
            buttons.forEach(function (b) { b.classList.remove('active'); });
            btn.classList.add('active');
            demo.set(uniformName, parseFloat(btn.dataset.value));
        });
    });
}

// ---- GLSL shared snippets ----

var GLSL_HASH = [
    'float hash1(vec2 p) {',
    '    vec3 p3 = fract(vec3(p.xyx) * 0.1031);',
    '    p3 += dot(p3, p3.yzx + 33.33);',
    '    return fract((p3.x + p3.y) * p3.z);',
    '}',
    '',
    'vec2 hash2(vec2 p) {',
    '    vec3 p3 = fract(vec3(p.xyx) * vec3(0.1031, 0.1030, 0.0973));',
    '    p3 += dot(p3, p3.yzx + 33.33);',
    '    return fract((p3.xx + p3.yz) * p3.zy);',
    '}'
].join('\n');

var GLSL_HSV = [
    'vec3 hsv2rgb(vec3 c) {',
    '    vec4 K = vec4(1.0, 2.0/3.0, 1.0/3.0, 3.0);',
    '    vec3 p = abs(fract(c.xxx + K.xyz) * 6.0 - K.www);',
    '    return c.z * mix(K.xxx, clamp(p - K.xxx, 0.0, 1.0), c.y);',
    '}'
].join('\n');

var GLSL_VALUE_NOISE = [
    'vec2 valueNoise2(vec2 p) {',
    '    vec2 i = floor(p);',
    '    vec2 f = fract(p);',
    '    f = f * f * (3.0 - 2.0 * f);',
    '    vec2 a = hash2(i) - 0.5;',
    '    vec2 b = hash2(i + vec2(1.0, 0.0)) - 0.5;',
    '    vec2 c = hash2(i + vec2(0.0, 1.0)) - 0.5;',
    '    vec2 d = hash2(i + vec2(1.0, 1.0)) - 0.5;',
    '    return mix(mix(a, b, f.x), mix(c, d, f.x), f.y);',
    '}'
].join('\n');

// Voronoi core used by several demos (no warp, simple circular drift)
var GLSL_VORONOI_CORE = [
    'vec4 voronoi(vec2 uv, float scale, float animTime) {',
    '    vec2 p = uv * scale;',
    '    vec2 cell = floor(p);',
    '    vec2 localP = fract(p);',
    '    float f1 = 10.0, f2 = 10.0;',
    '    vec2 nearestCell = vec2(0.0);',
    '    for (int j = -1; j <= 1; j++) {',
    '        for (int i = -1; i <= 1; i++) {',
    '            vec2 neighbor = vec2(float(i), float(j));',
    '            vec2 cellPos = cell + neighbor;',
    '            vec2 seedHash = hash2(cellPos);',
    '            vec2 seedBase = seedHash * 0.8 + 0.1;',
    '            float angle = 6.2832 * seedHash.x + animTime * (0.3 + seedHash.y * 0.7);',
    '            vec2 drift = 0.35 * vec2(cos(angle), sin(angle));',
    '            vec2 point = neighbor + seedBase + drift;',
    '            float dist = length(point - localP);',
    '            if (dist < f1) { f2 = f1; f1 = dist; nearestCell = cellPos; }',
    '            else if (dist < f2) { f2 = dist; }',
    '        }',
    '    }',
    '    return vec4(f1, f2, nearestCell);',
    '}'
].join('\n');

// ---- Demo: Hash distribution (Canvas 2D) ----

function initHashDemo(container) {
    var canvas = container.querySelector('canvas');
    var ctx = canvas.getContext('2d');
    if (!ctx) return;

    var mode = 'nosine';

    function draw() {
        var dpr = window.devicePixelRatio || 1;
        var w = canvas.clientWidth, h = canvas.clientHeight;
        canvas.width = w * dpr;
        canvas.height = h * dpr;
        ctx.scale(dpr, dpr);

        ctx.fillStyle = '#0d1117';
        ctx.fillRect(0, 0, w, h);

        var hashFn = mode === 'nosine' ? hash2JS : sineHash2JS;
        var grid = 64;
        var r = 1.8;

        ctx.fillStyle = 'rgba(88, 166, 255, 0.55)';
        for (var i = 0; i < grid; i++) {
            for (var j = 0; j < grid; j++) {
                var h2 = hashFn(i, j);
                ctx.beginPath();
                ctx.arc(h2[0] * w, h2[1] * h, r, 0, 6.2832);
                ctx.fill();
            }
        }

        ctx.fillStyle = '#8b949e';
        ctx.font = '12px -apple-system, BlinkMacSystemFont, sans-serif';
        ctx.fillText(mode === 'nosine' ? 'Dave Hoskins (no sine)' : 'Classic sine hash', 8, h - 8);
        ctx.fillText(grid + '\u00d7' + grid + ' = ' + (grid * grid) + ' points', 8, 16);
    }

    draw();

    container.querySelectorAll('button[data-mode]').forEach(function (btn) {
        btn.addEventListener('click', function () {
            container.querySelectorAll('button[data-mode]').forEach(function (b) {
                b.classList.remove('active');
            });
            btn.classList.add('active');
            mode = btn.dataset.mode;
            draw();
        });
    });
}

// ---- Demo: Voronoi F1/F2 (WebGL) ----

function initVoronoiF1F2(container) {
    var canvas = container.querySelector('canvas');
    var frag = GLSL_HASH + '\n' + GLSL_HSV + '\n' + [
        'varying vec2 v_uv;',
        'uniform vec2 u_resolution;',
        'uniform float u_time;',
        'uniform float u_mode;',
        '',
        'void main() {',
        '    vec2 uv = v_uv;',
        '    float aspect = u_resolution.x / u_resolution.y;',
        '    uv.x *= aspect;',
        '    float scale = 6.0;',
        '    vec2 p = uv * scale;',
        '    vec2 cell = floor(p);',
        '    vec2 localP = fract(p);',
        '    float f1 = 10.0, f2 = 10.0;',
        '    vec2 nearestCell = vec2(0.0);',
        '    for (int j = -1; j <= 1; j++) {',
        '        for (int i = -1; i <= 1; i++) {',
        '            vec2 neighbor = vec2(float(i), float(j));',
        '            vec2 cellPos = cell + neighbor;',
        '            vec2 seedPos = hash2(cellPos) * 0.8 + 0.1;',
        '            vec2 point = neighbor + seedPos;',
        '            float dist = length(point - localP);',
        '            if (dist < f1) { f2 = f1; f1 = dist; nearestCell = cellPos; }',
        '            else if (dist < f2) { f2 = dist; }',
        '        }',
        '    }',
        '    float hue = fract(hash1(nearestCell));',
        '    vec3 col;',
        '    if (u_mode < 0.5) {',
        '        col = hsv2rgb(vec3(hue, 0.6, 1.0 - f1 * 1.0));',
        '    } else if (u_mode < 1.5) {',
        '        col = hsv2rgb(vec3(hue, 0.6, 1.0 - f2 * 0.6));',
        '    } else {',
        '        float edge = 1.0 - smoothstep(0.0, 0.12, f2 - f1);',
        '        vec3 cellCol = hsv2rgb(vec3(hue, 0.7, 0.4));',
        '        vec3 edgeCol = hsv2rgb(vec3(hue, 0.2, 1.0));',
        '        col = mix(cellCol, edgeCol, edge);',
        '    }',
        '    // Seed dots',
        '    for (int j = -1; j <= 1; j++) {',
        '        for (int i = -1; i <= 1; i++) {',
        '            vec2 nb = vec2(float(i), float(j));',
        '            vec2 cp = cell + nb;',
        '            vec2 sp = nb + hash2(cp) * 0.8 + 0.1;',
        '            float sd = length(sp - localP) * scale;',
        '            col = mix(col, vec3(1.0), smoothstep(3.5, 1.5, sd));',
        '        }',
        '    }',
        '    gl_FragColor = vec4(col, 1.0);',
        '}'
    ].join('\n');

    var demo = createGLDemo(canvas, frag, { u_mode: 2.0 });
    if (demo) wireButtons(container, 'u_mode', demo);
}

// ---- Demo: Seed animation (WebGL) ----

function initSeedAnimation(container) {
    var canvas = container.querySelector('canvas');
    var frag = GLSL_HASH + '\n' + GLSL_HSV + '\n' + [
        'varying vec2 v_uv;',
        'uniform vec2 u_resolution;',
        'uniform float u_time;',
        'uniform float u_driftChaos;',
        '',
        'void main() {',
        '    vec2 uv = v_uv;',
        '    float aspect = u_resolution.x / u_resolution.y;',
        '    uv.x *= aspect;',
        '    float scale = 5.0;',
        '    float animTime = u_time * 0.5;',
        '    vec2 p = uv * scale;',
        '    vec2 cell = floor(p);',
        '    vec2 localP = fract(p);',
        '    float f1 = 10.0, f2 = 10.0;',
        '    vec2 nearestCell = vec2(0.0);',
        '',
        '    for (int j = -1; j <= 1; j++) {',
        '        for (int i = -1; i <= 1; i++) {',
        '            vec2 neighbor = vec2(float(i), float(j));',
        '            vec2 cellPos = cell + neighbor;',
        '            vec2 seedHash = hash2(cellPos);',
        '            vec2 seedBase = seedHash * 0.8 + 0.1;',
        '            float angle = 6.2832 * seedHash.x + animTime * (0.3 + seedHash.y * 0.7);',
        '            vec2 circDrift = 0.35 * vec2(cos(angle), sin(angle));',
        '            float phase = floor(animTime * 0.3);',
        '            float blend = fract(animTime * 0.3);',
        '            blend = blend * blend * (3.0 - 2.0 * blend);',
        '            vec2 rA = hash2(cellPos + vec2(phase * 17.3, phase * 7.1)) - 0.5;',
        '            vec2 rB = hash2(cellPos + vec2((phase+1.0)*17.3, (phase+1.0)*7.1)) - 0.5;',
        '            vec2 chaoDrift = mix(rA, rB, blend) * 0.7;',
        '            vec2 drift = mix(circDrift, chaoDrift, u_driftChaos);',
        '            vec2 point = neighbor + seedBase + drift;',
        '            float dist = length(point - localP);',
        '            if (dist < f1) { f2 = f1; f1 = dist; nearestCell = cellPos; }',
        '            else if (dist < f2) { f2 = dist; }',
        '        }',
        '    }',
        '',
        '    float edgeDist = f2 - f1;',
        '    float hue = fract(hash1(nearestCell));',
        '    vec3 cellCol = hsv2rgb(vec3(hue, 0.7, 0.5));',
        '    float edge = 1.0 - smoothstep(0.0, 0.04, edgeDist);',
        '    vec3 edgeCol = hsv2rgb(vec3(hue, 0.2, 1.0));',
        '    vec3 col = mix(cellCol, edgeCol, edge);',
        '',
        '    // Seed dots',
        '    for (int j = -1; j <= 1; j++) {',
        '        for (int i = -1; i <= 1; i++) {',
        '            vec2 nb = vec2(float(i), float(j));',
        '            vec2 cp = cell + nb;',
        '            vec2 sh = hash2(cp);',
        '            vec2 sb = sh * 0.8 + 0.1;',
        '            float ang = 6.2832 * sh.x + animTime * (0.3 + sh.y * 0.7);',
        '            vec2 cd = 0.35 * vec2(cos(ang), sin(ang));',
        '            float ph = floor(animTime * 0.3);',
        '            float bl = fract(animTime * 0.3);',
        '            bl = bl * bl * (3.0 - 2.0 * bl);',
        '            vec2 rA2 = hash2(cp + vec2(ph*17.3, ph*7.1)) - 0.5;',
        '            vec2 rB2 = hash2(cp + vec2((ph+1.0)*17.3, (ph+1.0)*7.1)) - 0.5;',
        '            vec2 chd = mix(rA2, rB2, bl) * 0.7;',
        '            vec2 dr = mix(cd, chd, u_driftChaos);',
        '            vec2 pt = nb + sb + dr;',
        '            float d = length(pt - localP);',
        '            col = mix(col, vec3(1.0), smoothstep(0.07, 0.02, d));',
        '        }',
        '    }',
        '',
        '    gl_FragColor = vec4(col, 1.0);',
        '}'
    ].join('\n');

    var demo = createGLDemo(canvas, frag, { u_driftChaos: 0.0 });
    if (demo) wireSlider(container, 'u_driftChaos', demo);
}

// ---- Demo: Smoothstep curves (Canvas 2D) ----

function initSmoothstepDemo(container) {
    var canvas = container.querySelector('canvas');
    var ctx = canvas.getContext('2d');
    if (!ctx) return;

    var curves = [
        { fn: function (t) { return t; },
          color: '#8b949e', label: 'Linear' },
        { fn: function (t) { return t * t * (3 - 2 * t); },
          color: '#58a6ff', label: 'Hermite (3t\u00b2 \u2212 2t\u00b3)' },
        { fn: function (t) { return t * t * t * (t * (t * 6 - 15) + 10); },
          color: '#f0883e', label: 'Quintic (6t\u2075 \u2212 15t\u2074 + 10t\u00b3)' }
    ];

    function draw(hoverX) {
        var dpr = window.devicePixelRatio || 1;
        var w = canvas.clientWidth, h = canvas.clientHeight;
        canvas.width = w * dpr;
        canvas.height = h * dpr;
        ctx.scale(dpr, dpr);

        var pad = 40;
        var pw = w - 2 * pad, ph = h - 2 * pad;

        ctx.fillStyle = '#0d1117';
        ctx.fillRect(0, 0, w, h);

        // Grid lines
        ctx.strokeStyle = '#21262d';
        ctx.lineWidth = 1;
        for (var g = 0.25; g < 1; g += 0.25) {
            ctx.beginPath();
            ctx.moveTo(pad + g * pw, pad);
            ctx.lineTo(pad + g * pw, pad + ph);
            ctx.stroke();
            ctx.beginPath();
            ctx.moveTo(pad, pad + ph - g * ph);
            ctx.lineTo(pad + pw, pad + ph - g * ph);
            ctx.stroke();
        }

        // Axes
        ctx.strokeStyle = '#30363d';
        ctx.lineWidth = 1;
        ctx.beginPath();
        ctx.moveTo(pad, pad);
        ctx.lineTo(pad, pad + ph);
        ctx.lineTo(pad + pw, pad + ph);
        ctx.stroke();

        // Labels
        ctx.fillStyle = '#8b949e';
        ctx.font = '11px -apple-system, sans-serif';
        ctx.textAlign = 'center';
        ctx.fillText('0', pad, pad + ph + 14);
        ctx.fillText('1', pad + pw, pad + ph + 14);
        ctx.fillText('t', pad + pw / 2, pad + ph + 14);
        ctx.textAlign = 'right';
        ctx.fillText('0', pad - 6, pad + ph + 4);
        ctx.fillText('1', pad - 6, pad + 4);

        // Curves
        curves.forEach(function (c) {
            ctx.strokeStyle = c.color;
            ctx.lineWidth = 2;
            ctx.beginPath();
            for (var px = 0; px <= pw; px++) {
                var t = px / pw;
                var y = c.fn(t);
                var cx = pad + px;
                var cy = pad + ph - y * ph;
                if (px === 0) ctx.moveTo(cx, cy);
                else ctx.lineTo(cx, cy);
            }
            ctx.stroke();
        });

        // Hover crosshair
        if (hoverX !== null && hoverX >= pad && hoverX <= pad + pw) {
            var t = (hoverX - pad) / pw;
            ctx.strokeStyle = '#30363d';
            ctx.lineWidth = 1;
            ctx.setLineDash([4, 4]);
            ctx.beginPath();
            ctx.moveTo(pad + t * pw, pad);
            ctx.lineTo(pad + t * pw, pad + ph);
            ctx.stroke();
            ctx.setLineDash([]);

            curves.forEach(function (c) {
                var y = c.fn(t);
                ctx.fillStyle = c.color;
                ctx.beginPath();
                ctx.arc(pad + t * pw, pad + ph - y * ph, 5, 0, 6.2832);
                ctx.fill();
            });

            // Value readout
            ctx.textAlign = 'left';
            ctx.font = '11px SFMono-Regular, Consolas, monospace';
            var ry = pad + 12;
            ctx.fillStyle = '#8b949e';
            ctx.fillText('t = ' + t.toFixed(2), pad + pw - 140, ry);
            ry += 14;
            curves.forEach(function (c) {
                ctx.fillStyle = c.color;
                ctx.fillText(c.fn(t).toFixed(3), pad + pw - 140, ry);
                ry += 14;
            });
        }

        // Legend
        ctx.textAlign = 'left';
        var ly = pad + 8;
        curves.forEach(function (c) {
            ctx.fillStyle = c.color;
            ctx.fillRect(pad + 8, ly - 3, 14, 3);
            ctx.fillStyle = '#c9d1d9';
            ctx.font = '11px -apple-system, sans-serif';
            ctx.fillText(c.label, pad + 28, ly);
            ly += 16;
        });
    }

    draw(null);

    canvas.addEventListener('mousemove', function (e) {
        var rect = canvas.getBoundingClientRect();
        draw(e.clientX - rect.left);
    });
    canvas.addEventListener('mouseleave', function () { draw(null); });

    // Redraw on resize
    window.addEventListener('resize', function () { draw(null); });
}

// ---- Demo: Warp (WebGL) ----

function initWarpDemo(container) {
    var canvas = container.querySelector('canvas');
    var frag = GLSL_HASH + '\n' + GLSL_HSV + '\n' + GLSL_VALUE_NOISE + '\n' +
        GLSL_VORONOI_CORE + '\n' + [
        'varying vec2 v_uv;',
        'uniform vec2 u_resolution;',
        'uniform float u_time;',
        'uniform float u_warp;',
        '',
        'void main() {',
        '    vec2 uv = v_uv;',
        '    float aspect = u_resolution.x / u_resolution.y;',
        '    uv.x *= aspect;',
        '    if (u_warp > 0.001) {',
        '        vec2 wo = valueNoise2(uv * 3.0 + u_time * 0.08);',
        '        wo += valueNoise2(uv * 7.0 - u_time * 0.05) * 0.5;',
        '        uv += wo * u_warp * 0.25;',
        '    }',
        '    float animTime = u_time * 0.3;',
        '    vec4 vor = voronoi(uv, 6.0, animTime);',
        '    float edgeDist = vor.y - vor.x;',
        '    float hue = fract(hash1(vor.zw));',
        '    vec3 cellCol = hsv2rgb(vec3(hue, 0.7, 0.5));',
        '    float edge = 1.0 - smoothstep(0.0, 0.04, edgeDist);',
        '    vec3 edgeCol = hsv2rgb(vec3(hue, 0.2, 1.0));',
        '    vec3 col = mix(cellCol, edgeCol, edge);',
        '    gl_FragColor = vec4(col, 1.0);',
        '}'
    ].join('\n');

    var demo = createGLDemo(canvas, frag, { u_warp: 0.0 });
    if (demo) wireSlider(container, 'u_warp', demo);
}

// ---- Demo: Edge glow (WebGL) ----

function initEdgeGlowDemo(container) {
    var canvas = container.querySelector('canvas');
    var frag = GLSL_HASH + '\n' + GLSL_HSV + '\n' + GLSL_VORONOI_CORE + '\n' + [
        'varying vec2 v_uv;',
        'uniform vec2 u_resolution;',
        'uniform float u_time;',
        'uniform float u_edgeWidth;',
        'uniform float u_edgeGlow;',
        '',
        'void main() {',
        '    vec2 uv = v_uv;',
        '    float aspect = u_resolution.x / u_resolution.y;',
        '    uv.x *= aspect;',
        '    float animTime = u_time * 0.3;',
        '    vec4 vor = voronoi(uv, 6.0, animTime);',
        '    float edgeDist = vor.y - vor.x;',
        '    float hue = fract(hash1(vor.zw));',
        '    vec3 cellCol = hsv2rgb(vec3(hue, 0.7, 0.5));',
        '    float ef = 1.0 - smoothstep(0.0, max(u_edgeWidth, 0.001), edgeDist);',
        '    float gr = u_edgeWidth * (1.0 + u_edgeGlow * 4.0);',
        '    float gf = (1.0 - smoothstep(0.0, max(gr, 0.001), edgeDist)) * u_edgeGlow;',
        '    float te = clamp(max(ef, gf), 0.0, 1.0);',
        '    vec3 edgeCol = hsv2rgb(vec3(hue, 0.7 * 0.2, 1.0));',
        '    vec3 col = mix(cellCol, edgeCol, te);',
        '    gl_FragColor = vec4(col, 1.0);',
        '}'
    ].join('\n');

    var demo = createGLDemo(canvas, frag, { u_edgeWidth: 0.04, u_edgeGlow: 0.4 });
    if (demo) {
        wireSlider(container, 'u_edgeWidth', demo);
        wireSlider(container, 'u_edgeGlow', demo);
    }
}

// ---- Auto-init ----

var DEMOS = {
    'hash': initHashDemo,
    'voronoi-f1f2': initVoronoiF1F2,
    'seed-animation': initSeedAnimation,
    'smoothstep': initSmoothstepDemo,
    'warp': initWarpDemo,
    'edge-glow': initEdgeGlowDemo
};

document.addEventListener('DOMContentLoaded', function () {
    document.querySelectorAll('[data-demo]').forEach(function (el) {
        var name = el.dataset.demo;
        if (DEMOS[name]) DEMOS[name](el);
    });
});

})();
