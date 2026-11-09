// Stand-in for MusicBrainz, paired with mock-lidarr.js. node mock-mb.js [port]
// Release-group ids match the Lidarr mock's foreignAlbumId (mb-atlas, ...) so a request connects end to end.
const http = require('http'), url = require('url');
const PORT = Number(process.argv[2] || 3100);

// mirrors the album set in mock-lidarr.js
const ALBUMS = [
  { id: 'mb-atlas', title: 'Midnight Atlas', artist: 'Odessa Rowe', type: 'Album', date: '2026-02-06', songs: ['North Star', 'Paper Maps', 'Night Ferry'] },
  { id: 'mb-firstlight', title: 'First Light', artist: 'Harbor Lights', type: 'Album', date: '2017-05-12', songs: ['Dawn Chorus', 'Harbor Wall'] },
  { id: 'mb-tidewater', title: 'Tidewater', artist: 'Harbor Lights', type: 'Album', date: '2025-03-01', songs: ['Low Tide'] },
  { id: 'mb-static', title: 'Static Bloom', artist: 'The Velvet Static', type: 'Album', date: '2023-09-15', songs: ['Polaroid', 'Echo Park Drive'] },
];
const SINGLES = [
  { id: 'mb-northstar-single', title: 'North Star', artist: 'Odessa Rowe', type: 'Single', date: '2025-11-01', song: 'North Star' },
];
const ARTISTS = [
  { id: 'mb-odessa', name: 'Odessa Rowe', type: 'Person' },
  { id: 'mb-harbor', name: 'Harbor Lights', type: 'Group' },
  { id: 'mb-velvet', name: 'The Velvet Static', type: 'Group' },
];
const norm = s => s.toLowerCase().replace(/[^a-z0-9 ]/g, '');
const matchAll = (text, q) => norm(q).split(/\s+/).filter(Boolean).every(w => norm(text).includes(w));

http.createServer((req, res) => {
  const u = url.parse(req.url, true), p = u.pathname;
  const q = decodeURIComponent(u.query.query || '').replace(/%20/g, ' ').replace(/(recording|artist|release):/gi, '').replace(/ and .*/i, '');
  const send = v => { res.writeHead(200, { 'Content-Type': 'application/json' }); res.end(JSON.stringify(v)); };
  if (p === '/ws/2/artist') return send({ artists: ARTISTS.filter(a => matchAll(a.name, q)).map(a => ({ id: a.id, name: a.name, type: a.type, country: 'US' })) });
  if (p === '/ws/2/release-group') return send({ 'release-groups': ALBUMS.filter(a => matchAll(a.title + ' ' + a.artist, q)).map(a => ({ id: a.id, title: a.title, 'primary-type': a.type, 'first-release-date': a.date, 'artist-credit': [{ name: a.artist }] })) });
  if (p === '/ws/2/recording') {
    const recs = [];
    ALBUMS.forEach(al => al.songs.forEach(song => {
      if (!matchAll(song + ' ' + al.artist, q)) return;
      const single = SINGLES.find(s => s.song === song && s.artist === al.artist);
      const releases = [{ date: al.date, 'release-group': { id: al.id, 'primary-type': al.type, 'first-release-date': al.date } }];
      if (single) releases.push({ date: single.date, 'release-group': { id: single.id, 'primary-type': 'Single', 'first-release-date': single.date } });
      recs.push({ id: 'rec-' + norm(song).replace(/ /g, ''), score: 100, title: song, length: 210000, 'artist-credit': [{ name: al.artist }], releases, _artist: al.artist });
    }));
    return send({ recordings: recs });
  }
  // a recording's relationships (who produced, mixed, composed, wrote it)
  const lookup = p.match(/^\/ws\/2\/recording\/(rec-.+)$/);
  if (lookup) {
    const id = lookup[1];
    let artist = 'Unknown';
    ALBUMS.forEach(al => al.songs.forEach(s => { if ('rec-' + norm(s).replace(/ /g, '') === id) artist = al.artist; }));
    return send({ id, relations: [
      { type: 'producer', artist: { name: 'Sam Lee' } },
      { type: 'engineer', attributes: ['mix'], artist: { name: 'Ana Ruiz' } },
      { type: 'vocal', attributes: ['lead vocals'], artist: { name: artist } },
      { type: 'performance', work: { relations: [{ type: 'composer', artist: { name: artist } }, { type: 'lyricist', artist: { name: 'Jane Doe' } }] } }
    ] });
  }
  res.writeHead(404); res.end('{}');
}).listen(PORT, () => console.log('mock MusicBrainz on ' + PORT));
