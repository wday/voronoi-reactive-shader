#!/usr/bin/env python3
"""
Browser gallery for reviewing shader explore output.
Serves images and a single-page app for flagging keepers.
Supports generation-based navigation with a timeline slider.

Usage:
    uv run python gallery.py                              # default dir
    uv run python gallery.py --dir /path/to/shader-explore
    uv run python gallery.py --port 8080
"""

import argparse
import json
import http.server
import os
import urllib.parse
from pathlib import Path

DEFAULT_DIR = Path.cwd()

HTML_TEMPLATE = r"""<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<title>SHADER EXPLORE</title>
<style>
@import url('https://fonts.googleapis.com/css2?family=Space+Mono:ital,wght@0,400;0,700;1,400&display=swap');

* { margin: 0; padding: 0; box-sizing: border-box; }

:root {
    --bg: #0a0a0a;
    --fg: #c8c8c8;
    --dim: #555;
    --accent: #ff3b00;
    --kept: #fff;
    --border: #333;
    --mono: 'Space Mono', 'Courier New', monospace;
}

::selection { background: var(--accent); color: #000; }

body {
    background: var(--bg); color: var(--fg);
    font-family: var(--mono); font-size: 11px;
    letter-spacing: 0.02em;
    cursor: crosshair;
}

/* ── HEADER ──────────────────────────────────────────────── */
header {
    position: sticky; top: 0; z-index: 100;
    background: var(--bg);
    border-bottom: 2px solid var(--fg);
    padding: 16px 20px 12px;
    display: flex; align-items: baseline; gap: 20px;
    flex-wrap: wrap;
}
header h1 {
    font-size: 11px; font-weight: 700;
    text-transform: uppercase; letter-spacing: 0.3em;
}
header .stats {
    color: var(--dim); font-size: 10px;
    text-transform: uppercase; letter-spacing: 0.15em;
}
header .actions {
    margin-left: auto; display: flex; gap: 4px;
}
header button {
    background: none; color: var(--fg);
    border: 1px solid var(--border);
    padding: 4px 10px; cursor: pointer;
    font-family: var(--mono); font-size: 10px;
    text-transform: uppercase; letter-spacing: 0.1em;
}
header button:hover {
    border-color: var(--fg); color: #fff;
}
header button.hot {
    border-color: var(--accent); color: var(--accent);
}
header button.hot:hover {
    background: var(--accent); color: #000;
}

/* ── TIMELINE ────────────────────────────────────────────── */
.timeline {
    position: sticky; top: 44px; z-index: 99;
    background: var(--bg);
    border-bottom: 1px solid var(--border);
    padding: 8px 20px; display: flex; align-items: center; gap: 0;
}
.timeline .gen-bar {
    flex: 1; display: flex; align-items: stretch;
    height: 24px;
}
.timeline .gen-btn {
    flex: 1; border: none; cursor: pointer;
    font-family: var(--mono); font-size: 10px;
    text-transform: uppercase; letter-spacing: 0.1em;
    color: var(--dim); background: transparent;
    border-bottom: 2px solid transparent;
    position: relative;
}
.timeline .gen-btn:hover { color: var(--fg); }
.timeline .gen-btn.active {
    color: #fff;
    border-bottom-color: var(--accent);
}
.timeline .gen-btn .keeper-dot {
    position: absolute; top: 2px; right: 4px;
    width: 5px; height: 5px;
    background: var(--accent);
}
.timeline .gen-info {
    color: var(--dim); font-size: 9px;
    text-transform: uppercase; letter-spacing: 0.15em;
    white-space: nowrap; padding-left: 16px;
}

/* ── FILTERS ─────────────────────────────────────────────── */
.filters {
    padding: 6px 20px; background: var(--bg);
    border-bottom: 1px solid var(--border);
    display: flex; gap: 2px; flex-wrap: wrap;
}
.filters .tag {
    padding: 2px 8px; cursor: pointer;
    border: 1px solid transparent;
    font-size: 10px; text-transform: uppercase;
    letter-spacing: 0.1em; color: var(--dim);
}
.filters .tag:hover { color: var(--fg); }
.filters .tag.active {
    color: #fff; border-color: var(--fg);
}

/* ── GRID ────────────────────────────────────────────────── */
.grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
    gap: 2px; padding: 2px;
}
.card {
    position: relative; cursor: crosshair; overflow: hidden;
    border: none; outline: 2px solid transparent;
    outline-offset: -2px;
}
.card.kept {
    outline-color: var(--kept);
}
.card.kept::after {
    content: ''; position: absolute; top: 0; right: 0;
    width: 0; height: 0;
    border-top: 20px solid var(--accent);
    border-left: 20px solid transparent;
}
.card img {
    width: 100%; display: block; aspect-ratio: 4/3; object-fit: cover;
    filter: grayscale(0);
    transition: filter 0.3s;
}
.card:hover img {
    filter: contrast(1.1);
}
.card .info {
    position: absolute; bottom: 0; left: 0; right: 0;
    background: rgba(0,0,0,0.88);
    padding: 6px 8px; font-size: 9px; color: var(--dim);
    opacity: 0; transition: opacity 0.1s;
    letter-spacing: 0.05em;
}
.card:hover .info { opacity: 1; }
.card .info .params {
    display: flex; flex-wrap: wrap; gap: 2px 6px;
}
.card .info .param { white-space: nowrap; }
.card .info .parent-tag {
    color: var(--accent);
}

/* ── MODAL ───────────────────────────────────────────────── */
.modal {
    display: none; position: fixed; inset: 0; z-index: 200;
    background: var(--bg);
    justify-content: center; align-items: center;
    cursor: crosshair;
}
.modal.open { display: flex; flex-direction: column; }
.modal img {
    max-width: 95vw; max-height: 82vh; object-fit: contain;
}
.modal .modal-info {
    padding: 20px; text-align: left; max-width: 900px; width: 100%;
}
.modal .modal-info code {
    display: block; margin-top: 8px;
    font-size: 10px; color: var(--dim);
    letter-spacing: 0.05em; line-height: 1.6;
    word-break: break-all;
    border-left: 2px solid var(--accent);
    padding-left: 12px;
}

.hidden { display: none !important; }

/* ── EMPTY STATE ─────────────────────────────────────────── */
.empty {
    padding: 80px 20px; text-align: center;
    color: var(--dim); font-size: 10px;
    text-transform: uppercase; letter-spacing: 0.3em;
}
</style>
</head>
<body>

<header>
    <h1>Shader Explore</h1>
    <span class="stats" id="stats"></span>
    <div class="actions">
        <button onclick="showKeepers()">Kept</button>
        <button onclick="showAll()">All</button>
        <button onclick="clearKeepers()">Clear</button>
        <button class="hot" onclick="saveToFile()">Evolve</button>
    </div>
</header>

<div class="timeline" id="timeline"></div>

<div class="filters" id="filters"></div>

<div class="grid" id="grid"></div>

<div class="modal" id="modal" onclick="closeModal()">
    <img id="modal-img">
    <div class="modal-info">
        <div id="modal-params"></div>
    </div>
</div>

<script>
const allData = ALL_DATA_JSON;
// allData = { generations: [{gen, dir, count, seed, timestamp, ...}], items: [{file, gen, dir, params, ...}] }

let keepers = {};  // keyed by gen: { "0": Set([file, ...]), "1": Set([...]) }
let filterSource = null;
let showKeptOnly = false;
let activeGen = null;  // null = "all"

// Load keepers from localStorage
try {
    const saved = JSON.parse(localStorage.getItem('shader-keepers-v2') || '{}');
    for (const [gen, files] of Object.entries(saved)) {
        keepers[gen] = new Set(files);
    }
} catch(e) {}

function saveKeepersLocal() {
    const obj = {};
    for (const [gen, s] of Object.entries(keepers)) {
        if (s.size > 0) obj[gen] = [...s];
    }
    localStorage.setItem('shader-keepers-v2', JSON.stringify(obj));
    updateStats();
}

function totalKept() {
    let n = 0;
    for (const s of Object.values(keepers)) n += s.size;
    return n;
}

function isKept(gen, file) {
    return keepers[gen] && keepers[gen].has(file);
}

function toggleKeeper(gen, file, card) {
    if (!keepers[gen]) keepers[gen] = new Set();
    if (keepers[gen].has(file)) keepers[gen].delete(file);
    else keepers[gen].add(file);
    card.classList.toggle('kept', keepers[gen].has(file));
    saveKeepersLocal();
    renderTimeline();  // update keeper dots
}

function updateStats() {
    const visible = activeGen !== null
        ? allData.items.filter(i => i.gen === activeGen).length
        : allData.items.length;
    document.getElementById('stats').textContent =
        `${visible} / ${totalKept()} kept`;
}

function formatParam(key, val) {
    const short = key.replace('u_', '');
    if (typeof val === 'number') return `${short}=${val.toFixed(2)}`;
    return `${short}=${val}`;
}

function renderTimeline() {
    const tl = document.getElementById('timeline');
    if (allData.generations.length === 0) {
        tl.innerHTML = '';
        return;
    }

    let html = '<div class="gen-bar">';
    html += `<button class="gen-btn${activeGen === null ? ' active' : ''}" onclick="setGen(null)">*</button>`;
    for (const g of allData.generations) {
        const genKeepers = keepers[g.gen] ? keepers[g.gen].size : 0;
        const dot = genKeepers > 0 ? '<span class="keeper-dot"></span>' : '';
        html += `<button class="gen-btn${activeGen === g.gen ? ' active' : ''}" onclick="setGen(${g.gen})">${g.gen}${dot}</button>`;
    }
    html += '</div>';

    if (activeGen !== null) {
        const g = allData.generations.find(x => x.gen === activeGen);
        if (g) {
            const parts = [`${g.count}`];
            if (g.parent_gen !== undefined) parts.push(`< gen ${g.parent_gen}`);
            html += `<span class="gen-info">${parts.join(' ')}</span>`;
        }
    } else {
        html += `<span class="gen-info">${allData.generations.length} gen / ${allData.items.length}</span>`;
    }

    tl.innerHTML = html;
}

function setGen(gen) {
    activeGen = gen;
    renderTimeline();
    renderGrid();
}

function renderGrid() {
    const grid = document.getElementById('grid');
    grid.innerHTML = '';

    const items = activeGen !== null
        ? allData.items.filter(i => i.gen === activeGen)
        : allData.items;

    const sources = new Set();
    items.forEach(item => sources.add(item.source));

    // Build filter tags
    const filters = document.getElementById('filters');
    filters.innerHTML = '';
    for (const src of [...sources].sort()) {
        const tag = document.createElement('span');
        tag.className = 'tag' + (filterSource === src ? ' active' : '');
        tag.textContent = src;
        tag.onclick = () => { filterSource = filterSource === src ? null : src; renderGrid(); };
        filters.appendChild(tag);
    }

    for (const item of items) {
        if (filterSource && item.source !== filterSource) continue;
        if (showKeptOnly && !isKept(item.gen, item.file)) continue;

        const card = document.createElement('div');
        card.className = 'card' + (isKept(item.gen, item.file) ? ' kept' : '');

        const img = document.createElement('img');
        img.src = '/images/' + item.dir + '/' + encodeURIComponent(item.file);
        img.loading = 'lazy';

        const info = document.createElement('div');
        info.className = 'info';
        const ps = Object.entries(item.params)
            .map(([k,v]) => formatParam(k,v)).join(' ');
        let line = `${ps} n=${item.iterations} mix=${item.source_mix.toFixed(2)} ${item.source} g${item.gen}`;
        let paramHtml = `<span class="param">${line}</span>`;
        if (item.parent) {
            const short = item.parent.replace(/^\d+_/, '').slice(0, 30);
            paramHtml += `<br><span class="parent-tag">&lt; ${short}</span>`;
        }
        info.innerHTML = `<div class="params">${paramHtml}</div>`;

        card.appendChild(img);
        card.appendChild(info);

        card.onclick = (e) => {
            if (e.shiftKey) {
                openModal(item);
            } else {
                toggleKeeper(item.gen, item.file, card);
            }
        };

        grid.appendChild(card);
    }
    updateStats();
}

function openModal(item) {
    const modal = document.getElementById('modal');
    document.getElementById('modal-img').src = '/images/' + item.dir + '/' + encodeURIComponent(item.file);
    const params = Object.entries(item.params)
        .map(([k,v]) => `${k}=${typeof v === 'number' ? v.toFixed(3) : v}`)
        .join('\\n');
    let lines = [item.file, '', params, '',
        `iterations  ${item.iterations}`,
        `source_mix  ${item.source_mix.toFixed(3)}`,
        `source      ${item.source}`,
        `gen         ${item.gen}`];
    if (item.parent) lines.push(`parent      ${item.parent}`);
    document.getElementById('modal-params').innerHTML =
        `<code>${lines.join('<br>')}</code>`;
    modal.classList.add('open');
}

function closeModal() {
    document.getElementById('modal').classList.remove('open');
}

function showKeepers() { showKeptOnly = true; renderGrid(); }
function showAll() { showKeptOnly = false; renderGrid(); }

function clearKeepers() {
    if (confirm('Clear all selections?')) {
        keepers = {};
        saveKeepersLocal();
        renderTimeline();
        renderGrid();
    }
}

function saveToFile() {
    if (totalKept() === 0) { alert('No keepers selected! Click images to flag them.'); return; }
    // Collect all keepers with gen info
    const allKeepers = [];
    for (const [gen, files] of Object.entries(keepers)) {
        const genInfo = allData.generations.find(g => g.gen === parseInt(gen));
        const dir = genInfo ? genInfo.dir : `gen_${String(gen).padStart(3, '0')}`;
        for (const file of files) {
            allKeepers.push({ gen: parseInt(gen), dir, file });
        }
    }
    fetch('/save-keepers', {
        method: 'POST',
        headers: {'Content-Type': 'application/json'},
        body: JSON.stringify(allKeepers)
    }).then(r => r.json()).then(data => {
        document.getElementById('stats').textContent = `${data.count} keepers saved. tell claude: evolve`;
        document.getElementById('stats').style.color = 'var(--accent)';
        setTimeout(() => { document.getElementById('stats').style.color = ''; updateStats(); }, 4000);
    });
}

document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') closeModal();
    // Left/right arrow to navigate generations
    if (e.key === 'ArrowLeft' || e.key === 'ArrowRight') {
        if (allData.generations.length < 2) return;
        const gens = allData.generations.map(g => g.gen);
        if (activeGen === null) {
            activeGen = e.key === 'ArrowLeft' ? gens[0] : gens[gens.length - 1];
        } else {
            const idx = gens.indexOf(activeGen);
            if (e.key === 'ArrowLeft' && idx > 0) activeGen = gens[idx - 1];
            else if (e.key === 'ArrowRight' && idx < gens.length - 1) activeGen = gens[idx + 1];
            else if (e.key === 'ArrowRight' && idx === gens.length - 1) activeGen = null;
        }
        renderTimeline();
        renderGrid();
    }
});

renderTimeline();
renderGrid();
</script>
</body>
</html>"""


class GalleryHandler(http.server.BaseHTTPRequestHandler):
    def __init__(self, *args, image_dir=None, **kwargs):
        self.image_dir = image_dir
        super().__init__(*args, **kwargs)

    def do_GET(self):
        if self.path == "/" or self.path == "/index.html":
            self.serve_gallery()
        elif self.path.startswith("/images/"):
            self.serve_image()
        else:
            self.send_error(404)

    def do_POST(self):
        if self.path == "/save-keepers":
            self.save_keepers()
        else:
            self.send_error(404)

    def save_keepers(self):
        length = int(self.headers.get("Content-Length", 0))
        body = self.rfile.read(length)
        keeper_list = json.loads(body)

        keepers_path = self.image_dir / "keepers.json"
        with open(keepers_path, "w") as f:
            json.dump(keeper_list, f, indent=2)

        print(f"\n★ Saved {len(keeper_list)} keepers → {keepers_path}")

        resp = json.dumps({"count": len(keeper_list), "path": str(keepers_path)})
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.end_headers()
        self.wfile.write(resp.encode())

    def serve_gallery(self):
        all_data = build_all_data(self.image_dir)
        html = HTML_TEMPLATE.replace("ALL_DATA_JSON", json.dumps(all_data))

        self.send_response(200)
        self.send_header("Content-Type", "text/html; charset=utf-8")
        self.end_headers()
        self.wfile.write(html.encode())

    def serve_image(self):
        # Path: /images/gen_000/filename.png or /images/filename.png (legacy)
        rel_path = urllib.parse.unquote(self.path[len("/images/"):])
        filepath = self.image_dir / rel_path

        if not filepath.exists() or not filepath.is_file():
            self.send_error(404)
            return

        try:
            filepath.resolve().relative_to(self.image_dir.resolve())
        except ValueError:
            self.send_error(403)
            return

        self.send_response(200)
        self.send_header("Content-Type", "image/png")
        self.send_header("Cache-Control", "public, max-age=3600")
        self.end_headers()
        self.wfile.write(filepath.read_bytes())

    def log_message(self, format, *args):
        if args and "404" in str(args[0]):
            super().log_message(format, *args)


def build_all_data(image_dir):
    """Build combined data from all generations, or fall back to flat layout."""
    generations = []
    items = []

    gen_dirs = sorted(image_dir.glob("gen_[0-9][0-9][0-9]"))

    if gen_dirs:
        # Generation-based layout
        gen_index_path = image_dir / "generations.json"
        if gen_index_path.exists():
            gen_index = json.loads(gen_index_path.read_text())
            generations = gen_index.get("generations", [])
        else:
            # Build index from directories
            for gd in gen_dirs:
                gen_num = int(gd.name[4:])
                manifest_path = gd / "manifest.json"
                count = len(list(gd.glob("*.png")))
                generations.append({"gen": gen_num, "dir": gd.name, "count": count, "seed": 0, "timestamp": ""})

        for g in generations:
            gen_dir = image_dir / g["dir"]
            manifest_path = gen_dir / "manifest.json"
            if manifest_path.exists():
                manifest = json.loads(manifest_path.read_text())
                for entry in manifest:
                    entry["gen"] = g["gen"]
                    entry["dir"] = g["dir"]
                    items.append(entry)
    else:
        # Legacy flat layout — treat as gen 0
        manifest_path = image_dir / "manifest.json"
        if manifest_path.exists():
            manifest = json.loads(manifest_path.read_text())
            generations = [{"gen": 0, "dir": ".", "count": len(manifest), "seed": 0, "timestamp": ""}]
            for entry in manifest:
                entry["gen"] = 0
                entry["dir"] = "."
                items.append(entry)

    return {"generations": generations, "items": items}


def main():
    parser = argparse.ArgumentParser(description="Shader explore gallery viewer")
    parser.add_argument("--dir", type=str, default=str(DEFAULT_DIR))
    parser.add_argument("--port", type=int, default=8111)
    args = parser.parse_args()

    image_dir = Path(args.dir).resolve()
    if not image_dir.exists():
        print(f"Directory not found: {image_dir}")
        return

    all_data = build_all_data(image_dir)
    total = len(all_data["items"])
    gens = len(all_data["generations"])
    print(f"Serving {total} images across {gens} generation(s) from {image_dir}")
    print(f"Open: http://localhost:{args.port}")
    print()
    print("Controls:")
    print("  Click        → toggle keeper")
    print("  Shift+Click  → full size preview")
    print("  Left/Right   → navigate generations")
    print("  Filter by source type with tags")

    handler = lambda *a, **kw: GalleryHandler(*a, image_dir=image_dir, **kw)
    server = http.server.HTTPServer(("0.0.0.0", args.port), handler)

    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("\nDone.")


if __name__ == "__main__":
    main()
