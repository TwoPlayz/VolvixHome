// A stand-in for Lidarr's v1 API, for testing Volvix Home without a real Lidarr.
// node mock-lidarr.js <musicDir> <volvixDataDir> [port]
// "Downloads" take ~8 s, then real tagged WAV files are written to <musicDir> and Volvix Home's webhook is called.
const http = require('http'), fs = require('fs'), path = require('path'), url = require('url');
const [, , MUSIC, VHDATA, PORTARG] = process.argv;
const PORT = Number(PORTARG || 8686), KEY = 'testkey';
const img = id => `http://127.0.0.1:${PORT}/img/${id}.svg`;

const ARTISTS = [
  { foreignArtistId: 'mb-odessa', artistName: 'Odessa Rowe', artistType: 'Person', disambiguation: 'singer-songwriter' },
  { foreignArtistId: 'mb-harbor', artistName: 'Harbor Lights', artistType: 'Group' },
  { foreignArtistId: 'mb-velvet', artistName: 'The Velvet Static', artistType: 'Group' },
];
const ALBUMS = [
  { foreignAlbumId: 'mb-atlas', title: 'Midnight Atlas', artist: 'mb-odessa', releaseDate: '2026-02-06T00:00:00Z', albumType: 'Album', songs: ['North Star', 'Paper Maps', 'Night Ferry'] },
  { foreignAlbumId: 'mb-firstlight', title: 'First Light', artist: 'mb-harbor', releaseDate: '2017-05-12T00:00:00Z', albumType: 'Album', songs: ['Dawn Chorus', 'Harbor Wall'] },
  { foreignAlbumId: 'mb-tidewater', title: 'Tidewater', artist: 'mb-harbor', releaseDate: '2025-03-01T00:00:00Z', albumType: 'Album', songs: ['Low Tide'] },
  { foreignAlbumId: 'mb-static', title: 'Static Bloom', artist: 'mb-velvet', releaseDate: '2023-09-15T00:00:00Z', albumType: 'Album', songs: ['Polaroid', 'Echo Park Drive'] },
  { foreignAlbumId: 'mb-northstar-single', title: 'North Star', artist: 'mb-odessa', releaseDate: '2025-11-01T00:00:00Z', albumType: 'Single', songs: ['North Star'] },
  { foreignAlbumId: 'mb-atlas-ep', title: 'Atlas Sessions', artist: 'mb-odessa', releaseDate: '2026-06-01T00:00:00Z', albumType: 'EP', songs: ['North Star (Live)'] },
];
const db = { artists: {}, albums: {}, nextId: 1 };   // what "Lidarr" has added: foreignId -> id
const queue = [];                                     // { id, album, started, size }
const log = [];

const artistOf = a => ARTISTS.find(x => x.foreignArtistId === a.artist);
function artistJson(a) {
  return Object.assign({}, a, { id: db.artists[a.foreignArtistId] || 0, monitored: !!db.artists[a.foreignArtistId], images: [{ coverType: 'poster', url: '/MediaCover/x.jpg', remoteUrl: img(a.foreignArtistId) }] });
}
function albumJson(al) {
  const { songs, artist, ...rest } = al;
  return Object.assign({}, rest, { id: db.albums[al.foreignAlbumId] || 0, monitored: !!db.albums[al.foreignAlbumId],
    artist: artistJson(artistOf(al)), images: [{ coverType: 'cover', url: '/MediaCover/y.jpg', remoteUrl: img(al.foreignAlbumId) }] });
}

// ---- WAV writer (same as the test library) ----
function chunk(id, data) { const h = Buffer.alloc(8); h.write(id, 0, 'ascii'); h.writeUInt32LE(data.length, 4); return Buffer.concat([h, data, data.length % 2 ? Buffer.alloc(1) : Buffer.alloc(0)]); }
function wav(seconds, freq, tags) {
  const rate = 22050, n = rate * seconds, pcm = Buffer.alloc(n * 2);
  for (let i = 0; i < n; i++) pcm.writeInt16LE(Math.round(Math.sin(2 * Math.PI * freq * i / rate) * 8000 * Math.min(1, i / 2000, (n - i) / 2000)), i * 2);
  const fmt = Buffer.alloc(16); fmt.writeUInt16LE(1, 0); fmt.writeUInt16LE(1, 2); fmt.writeUInt32LE(rate, 4); fmt.writeUInt32LE(rate * 2, 8); fmt.writeUInt16LE(2, 12); fmt.writeUInt16LE(16, 14);
  const info = Buffer.concat([Buffer.from('INFO'), ...Object.entries(tags).map(([k, v]) => chunk(k, Buffer.from(String(v) + '\0')))]);
  const body = Buffer.concat([Buffer.from('WAVE'), chunk('fmt ', fmt), chunk('data', pcm), chunk('LIST', info)]);
  return Buffer.concat([Buffer.from('RIFF'), Buffer.from(Uint32Array.of(body.length).buffer), body]);
}

function startDownload(al) {
  if (queue.some(q => q.album === al) || fs.existsSync(albumDir(al))) return;
  const q = { id: db.nextId++, album: al, started: Date.now(), size: 50e6 };
  queue.push(q);
  log.push('download ' + al.title);
  setTimeout(() => importAlbum(q), 8000);
}
function albumDir(al) { return path.join(MUSIC, artistOf(al).artistName, `${al.title} (${al.releaseDate.slice(0, 4)})`); }
function importAlbum(q) {
  const al = q.album, dir = albumDir(al), artist = artistOf(al).artistName;
  fs.mkdirSync(dir, { recursive: true });
  al.songs.forEach((t, i) => fs.writeFileSync(path.join(dir, String(i + 1).padStart(2, '0') + ' - ' + t + '.wav'),
    wav(8 + i * 2, 300 + i * 50, { INAM: t, IART: artist, IPRD: al.title, ITRK: i + 1, ICRD: al.releaseDate.slice(0, 4), IGNR: 'Indie' })));
  queue.splice(queue.indexOf(q), 1);
  log.push('imported ' + al.title);
  // Lidarr → Connect → Webhook ("On Release Import")
  let token = '';
  try { token = JSON.parse(fs.readFileSync(path.join(VHDATA, 'lidarr.json'), 'utf8')).hook_token; } catch (e) {}
  const req = http.request({ host: '127.0.0.1', port: Number(process.env.VH_PORT || 4747), path: '/api/hooks/lidarr?token=' + token, method: 'POST', headers: { 'Content-Type': 'application/json' } },
    r => log.push('webhook -> ' + r.statusCode));
  req.on('error', e => log.push('webhook error ' + e.message));
  req.end(JSON.stringify({ eventType: 'AlbumDownload', album: { title: al.title } }));
}

function send(res, code, body) { res.writeHead(code, { 'Content-Type': 'application/json' }); res.end(body === undefined ? '' : JSON.stringify(body)); }
function readBody(req) { return new Promise(r => { let b = ''; req.on('data', c => b += c); req.on('end', () => { try { r(JSON.parse(b || 'null')); } catch (e) { r(null); } }); }); }

http.createServer(async (req, res) => {
  const u = url.parse(req.url, true), p = u.pathname;
  if (p.startsWith('/img/')) { const id = p.slice(5, -4); const hue = [...id].reduce((h, c) => h + c.charCodeAt(0), 0) % 360;
    res.writeHead(200, { 'Content-Type': 'image/svg+xml' }); return res.end(`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"><rect width="100" height="100" fill="hsl(${hue},45%,35%)"/><circle cx="70" cy="30" r="22" fill="hsl(${hue + 40},70%,70%)"/></svg>`); }
  if (p === '/log') return send(res, 200, log);
  if (req.headers['x-api-key'] !== KEY) return send(res, 401, { message: 'Unauthorized' });
  const term = String(u.query.term || '').toLowerCase();
  const match = (text) => term.split(/\s+/).every(w => text.toLowerCase().includes(w));
  switch (req.method + ' ' + p) {
    case 'GET /api/v1/system/status': return send(res, 200, { version: '2.9.6.4552', appName: 'Lidarr' });
    case 'GET /api/v1/rootfolder': return send(res, 200, [{ id: 1, path: MUSIC, defaultQualityProfileId: 1, defaultMetadataProfileId: 1 }]);
    case 'GET /api/v1/qualityprofile': return send(res, 200, [{ id: 1, name: 'Lossless' }, { id: 2, name: 'Any' }]);
    case 'GET /api/v1/metadataprofile': return send(res, 200, [{ id: 1, name: 'Standard' }]);
    case 'GET /api/v1/artist/lookup':
      if (term.startsWith('lidarr:')) return send(res, 200, ARTISTS.filter(a => a.foreignArtistId === term.slice(7)).map(artistJson));
      return send(res, 200, ARTISTS.filter(a => match(a.artistName)).map(artistJson));
    case 'GET /api/v1/album/lookup':
      if (term.startsWith('lidarr:')) return send(res, 200, ALBUMS.filter(a => a.foreignAlbumId === term.slice(7)).map(albumJson));
      return send(res, 200, ALBUMS.filter(a => match(a.title + ' ' + artistOf(a).artistName)).map(albumJson));
    case 'POST /api/v1/album': {
      const b = await readBody(req);
      const al = ALBUMS.find(a => a.foreignAlbumId === (b && b.foreignAlbumId));
      if (!al) return send(res, 400, [{ errorMessage: 'Album not found' }]);
      if (!b.artist || !b.artist.rootFolderPath || !b.artist.qualityProfileId) return send(res, 400, [{ errorMessage: "'Root Folder Path' must not be empty." }]);
      db.albums[al.foreignAlbumId] = db.nextId++;
      db.artists[al.artist] = db.artists[al.artist] || db.nextId++;
      log.push('add album ' + al.title + ' root=' + b.artist.rootFolderPath + ' q=' + b.artist.qualityProfileId + ' search=' + (b.addOptions && b.addOptions.searchForNewAlbum));
      if (b.addOptions && b.addOptions.searchForNewAlbum) startDownload(al);
      return send(res, 201, albumJson(al));
    }
    case 'POST /api/v1/artist': {
      const b = await readBody(req);
      const ar = ARTISTS.find(a => a.foreignArtistId === (b && b.foreignArtistId));
      if (!ar) return send(res, 400, [{ errorMessage: 'Artist not found' }]);
      db.artists[ar.foreignArtistId] = db.nextId++;
      log.push('add artist ' + ar.artistName + ' monitor=' + (b.addOptions && b.addOptions.monitor));
      if (b.addOptions && b.addOptions.searchForMissingAlbums) ALBUMS.filter(a => a.artist === ar.foreignArtistId).forEach(a => { db.albums[a.foreignAlbumId] = db.albums[a.foreignAlbumId] || db.nextId++; startDownload(a); });
      return send(res, 201, artistJson(ar));
    }
    case 'PUT /api/v1/album/monitor': { const b = await readBody(req); log.push('monitor ' + JSON.stringify(b.albumIds)); return send(res, 202, {}); }
    case 'POST /api/v1/command': {
      const b = await readBody(req);
      log.push('command ' + b.name);
      if (b.name === 'AlbumSearch') (b.albumIds || []).forEach(id => { const f = Object.keys(db.albums).find(k => db.albums[k] === id); const al = ALBUMS.find(a => a.foreignAlbumId === f); if (al) startDownload(al); });
      return send(res, 201, { id: db.nextId++, name: b.name });
    }
    case 'GET /api/v1/queue': {
      const records = queue.map(q => {
        const done = Math.min(0.95, (Date.now() - q.started) / 8000);
        return { id: q.id, title: q.album.title + ' [FLAC]', size: q.size, sizeleft: q.size * (1 - done), timeleft: '00:00:0' + Math.max(1, Math.round(8 - (Date.now() - q.started) / 1000)),
          status: 'downloading', trackedDownloadStatus: 'ok', trackedDownloadState: 'downloading', album: albumJson(q.album), artist: artistJson(artistOf(q.album)) };
      });
      return send(res, 200, { page: 1, pageSize: 100, totalRecords: records.length, records });
    }
  }
  send(res, 404, { message: 'not found: ' + req.method + ' ' + p });
}).listen(PORT, () => console.log('mock Lidarr on ' + PORT));
