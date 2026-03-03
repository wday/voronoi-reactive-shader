const http = require('http');
const fs = require('fs');
const path = require('path');
const { WebSocketServer } = require('ws');

const shaderPath = path.resolve(process.argv[2] || path.join(__dirname, '..', 'shaders', 'voronoi_reactive.fs'));
const port = parseInt(process.env.PORT, 10) || 9000;

const ROUTES = {
  '/': { file: path.join(__dirname, 'index.html'), type: 'text/html' },
  '/lib/isf.js': { file: path.join(__dirname, 'node_modules', 'interactive-shader-format', 'dist', 'build.js'), type: 'application/javascript' },
  '/lib/lil-gui.js': { file: path.join(__dirname, 'node_modules', 'lil-gui', 'dist', 'lil-gui.umd.js'), type: 'application/javascript' },
  '/lib/lil-gui.css': { file: path.join(__dirname, 'node_modules', 'lil-gui', 'dist', 'lil-gui.css'), type: 'text/css' },
};

const server = http.createServer((req, res) => {
  if (req.url === '/shader') {
    fs.readFile(shaderPath, 'utf8', (err, data) => {
      if (err) {
        res.writeHead(500, { 'Content-Type': 'text/plain' });
        res.end(`Error reading shader: ${err.message}`);
        return;
      }
      res.writeHead(200, { 'Content-Type': 'text/plain' });
      res.end(data);
    });
    return;
  }

  const route = ROUTES[req.url];
  if (route) {
    fs.readFile(route.file, (err, data) => {
      if (err) {
        res.writeHead(500, { 'Content-Type': 'text/plain' });
        res.end(`Error: ${err.message}`);
        return;
      }
      res.writeHead(200, { 'Content-Type': route.type });
      res.end(data);
    });
    return;
  }

  res.writeHead(404, { 'Content-Type': 'text/plain' });
  res.end('Not found');
});

const wss = new WebSocketServer({ server });

// File watcher with debounce
let debounceTimer = null;
fs.watch(shaderPath, () => {
  clearTimeout(debounceTimer);
  debounceTimer = setTimeout(() => {
    console.log(`[${new Date().toLocaleTimeString()}] Shader changed, broadcasting reload`);
    for (const client of wss.clients) {
      if (client.readyState === 1) client.send('reload');
    }
  }, 100);
});

server.listen(port, () => {
  console.log(`ISF Preview → http://localhost:${port}`);
  console.log(`Watching: ${shaderPath}`);
});
