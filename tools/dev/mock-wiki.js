// A stand-in for Wikipedia + Wikidata, only for trying artist bios offline. Usage: node mock-wiki.js <port>
const http = require('http');
const port = Number(process.argv[2] || 3102);
// a 2x2 PNG
const PNG = Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAYAAABytg0kAAAAFklEQVR4nGP8z8Dwn4GBgYEJRIAAAGsKAQjzWeVGAAAAAElFTkSuQmCC', 'base64');
const PAD = Buffer.concat([PNG, Buffer.alloc(200)]);
const bios = {
  'Harbor_Lights_(band)': 'Harbor Lights are an indie band formed on the coast in 2014. Their second album, Tidewater, was recorded over one winter in a converted boathouse and made them a quiet favourite of the radio. The band is known for slow, spacious songs about tides, ferries and long evenings. They have released three albums.',
  'Mara_Quinn': 'Mara Quinn is a songwriter and producer whose work moves between folk and electronic music. Her debut album, Glass Houses, appeared in 2024.',
};
http.createServer((req, res) => {
  const u = new URL(req.url, 'http://x');
  console.log('GET', u.pathname);
  const send = (v, t = 'application/json') => { res.writeHead(200, { 'content-type': t }); res.end(typeof v === 'string' || Buffer.isBuffer(v) ? v : JSON.stringify(v)); };
  const sum = u.pathname.match(/^\/api\/rest_v1\/page\/summary\/(.+)$/);
  if (sum) { const t = decodeURIComponent(sum[1]); return bios[t] ? send({ type: 'standard', title: t, extract: bios[t], thumbnail: { source: 'http://127.0.0.1:' + port + '/img/' + t + '.png' } }) : (res.writeHead(404), res.end()); }
  if (/^\/release-group\/[^/]+\/front-250$/.test(u.pathname)) return u.pathname.includes('nocover') ? (res.writeHead(404), res.end()) : send(PAD, 'image/png');
  if (u.pathname.startsWith('/img/')) return send(PAD, 'image/png');
  if (u.pathname === '/wiki/Special:EntityData/Q4242.json') return send({ entities: { Q4242: { sitelinks: { enwiki: { title: 'Mara Quinn' } } } } });
  res.writeHead(404); res.end();
}).listen(port, () => console.log('mock Wikipedia on', port));
