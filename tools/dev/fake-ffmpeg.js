// A stand-in for ffmpeg, only for tests: understands -version and "-i <in> ... <out>" and writes a small file.
const fs = require('fs'); const a = process.argv.slice(2);
if (a.includes('-version')) { console.log('ffmpeg version fake'); process.exit(0); }
const input = a[a.indexOf('-i') + 1], out = a[a.length - 1];
const kb = Number((a[a.indexOf('-b:a') + 1] || '96k').replace('k', ''));
const src = fs.readFileSync(input);
fs.writeFileSync(out, src.subarray(0, Math.max(2000, Math.floor(src.length * kb / 1411))));
fs.appendFileSync(process.env.FAKE_FFMPEG_LOG || require('os').tmpdir() + '/fake-ffmpeg.log', a.join(' ') + '\n');
