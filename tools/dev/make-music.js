// Writes a small test library in Lidarr's layout: Artist/Album (Year)/NN - Title.wav, with RIFF INFO tags.
const fs = require('fs'), path = require('path');
const root = process.argv[2];
const lib = [
  ['Harbor Lights', 'Tidewater', 2025, ['Low Tide', 'Lantern Hours', 'Fog Signal', 'Ferry at Dawn'], 220],
  ['Mara Quinn', 'Glass Houses', 2024, ['Glass Houses', 'Paper Crown', 'Weekend Ghost'], 330],
  ['Juniper & Ash', 'Field Notes', 2019, ['Morning Field', 'Creek Song', 'Lanterns'], 262],
];
function chunk(id, data) { const h = Buffer.alloc(8); h.write(id, 0, 'ascii'); h.writeUInt32LE(data.length, 4); const pad = data.length % 2 ? Buffer.alloc(1) : Buffer.alloc(0); return Buffer.concat([h, data, pad]); }
function infoText(id, s) { return chunk(id, Buffer.from(s + '\0', 'utf8')); }
function wav(seconds, freq, tags) {
  const rate = 22050, n = rate * seconds, pcm = Buffer.alloc(n * 2);
  for (let i = 0; i < n; i++) { const env = Math.min(1, i / 2000, (n - i) / 2000); pcm.writeInt16LE(Math.round(Math.sin(2 * Math.PI * freq * i / rate) * 9000 * env), i * 2); }
  const fmt = Buffer.alloc(16); fmt.writeUInt16LE(1, 0); fmt.writeUInt16LE(1, 2); fmt.writeUInt32LE(rate, 4); fmt.writeUInt32LE(rate * 2, 8); fmt.writeUInt16LE(2, 12); fmt.writeUInt16LE(16, 14);
  const parts = [chunk('fmt ', fmt), chunk('data', pcm)];
  if (tags) {
    const info = Buffer.concat([Buffer.from('INFO'), ...Object.entries(tags).map(([k, v]) => infoText(k, String(v)))]);
    parts.push(chunk('LIST', info));
  }
  const body = Buffer.concat([Buffer.from('WAVE'), ...parts]);
  return Buffer.concat([Buffer.from('RIFF'), Buffer.from(Uint32Array.of(body.length).buffer), body]);
}
for (const [artist, album, year, songs, base] of lib) {
  const dir = path.join(root, artist, `${album} (${year})`);
  fs.mkdirSync(dir, { recursive: true });
  songs.forEach((t, i) => {
    const file = path.join(dir, String(i + 1).padStart(2, '0') + ' - ' + t + '.wav');
    fs.writeFileSync(file, wav(12 + i * 3, base + i * 40, { INAM: t, IART: artist, IPRD: album, ITRK: i + 1, ICRD: year, IGNR: 'Indie' }));
  });
}
// an untagged file: names must come from the folders
const dir = path.join(root, 'Low Orchard', 'Orchard Season');
fs.mkdirSync(dir, { recursive: true });
fs.writeFileSync(path.join(dir, '02 - Apple Rain.wav'), wav(10, 300, null));
console.log('written to', root);
