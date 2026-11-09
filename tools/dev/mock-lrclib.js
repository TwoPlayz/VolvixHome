// A tiny stand-in for LRCLIB (GET /api/get) so lyrics can be tried without the internet. Usage: node mock-lrclib.js <port>
const http = require('http');
const port = Number(process.argv[2] || 4751);
const songs = {
  'glass houses': { syncedLyrics: '[00:01.00]Glass houses by the sea\n[00:04.00]Nobody throws a stone\n[00:07.00]We keep the lights on low', plainLyrics: 'Glass houses by the sea\nNobody throws a stone\nWe keep the lights on low' },
  'paper crown': { plainLyrics: 'Paper crown, paper crown\nTry it on for size' },
  'creek song': { instrumental: true },
};
http.createServer((req, res) => {
  const u = new URL(req.url, 'http://x');
  if (u.pathname !== '/api/get') { res.writeHead(404); return res.end(); }
  const hit = songs[(u.searchParams.get('track_name') || '').toLowerCase()];
  console.log('GET', u.searchParams.get('track_name'), '->', hit ? 'found' : '404', 'UA:', req.headers['user-agent']);
  if (!hit) { res.writeHead(404, { 'content-type': 'application/json' }); return res.end('{"code":404,"name":"NotFound"}'); }
  res.writeHead(200, { 'content-type': 'application/json' }); res.end(JSON.stringify(Object.assign({ id: 1, trackName: u.searchParams.get('track_name') }, hit)));
}).listen(port, () => console.log('mock LRCLIB on', port));
