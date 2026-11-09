// Adds a release to the iPhone app's update feed (apps/ios/source.json), the "source" AltStore and SideStore follow.
//   node tools/make-source.js <tag> <ipaBytes> <owner/repo> [releaseNotes]
// Run by .github/workflows/release.yml after the IPA is attached to the release.
const fs = require('fs'), path = require('path');
const [, , tag, size, repo, notes] = process.argv;
if (!tag || !size || !repo) { console.error('usage: make-source.js <tag> <ipaBytes> <owner/repo> [notes]'); process.exit(1); }
const file = path.join(__dirname, '..', 'apps', 'ios', 'source.json');
const [owner] = repo.split('/');
const raw = (p) => `https://raw.githubusercontent.com/${repo}/main/${p}`;
const version = tag.replace(/^v/, '');
const feed = fs.existsSync(file) ? JSON.parse(fs.readFileSync(file, 'utf8').replace(/^\uFEFF/, '')) : {};
const app = (feed.apps && feed.apps[0]) || {};
const entry = {
  version, buildVersion: process.env.GITHUB_RUN_NUMBER || '1', date: new Date().toISOString().slice(0, 10),
  downloadURL: `https://github.com/${repo}/releases/download/${tag}/VolvixHome.ipa`, size: Number(size),
  localizedDescription: notes || `Volvix Home ${version}`, minOSVersion: '15.0'
};
const versions = [entry].concat((app.versions || []).filter(v => v.version !== version)).slice(0, 20);
const out = {
  name: 'Volvix Home', identifier: 'app.volvix.home.source', subtitle: 'Your household\'s own music server',
  description: 'The iPhone app for Volvix Home. It connects to your own server; updates arrive here.',
  iconURL: raw('apps/ios/VolvixHome/Assets.xcassets/AppIcon.appiconset/icon-1024.png'), tintColor: 'E5484D',
  sourceURL: raw('apps/ios/source.json'),
  apps: [{
    name: 'Volvix Home', bundleIdentifier: 'app.volvix.home', developerName: owner, subtitle: 'Your music server, on your iPhone',
    localizedDescription: 'A window onto your own Volvix Home server: your library, playlists, parties and downloads. Enter your server\'s address once.',
    iconURL: raw('apps/ios/VolvixHome/Assets.xcassets/AppIcon.appiconset/icon-1024.png'), tintColor: 'E5484D', screenshotURLs: [],
    versions,
    // the older single-version fields, for clients that don't read "versions" yet
    version: entry.version, versionDate: entry.date, downloadURL: entry.downloadURL, size: entry.size, versionDescription: entry.localizedDescription
  }],
  news: []
};
fs.writeFileSync(file, JSON.stringify(out, null, 2) + '\n');
console.log(`feed now lists ${versions.length} version(s), newest ${version}`);
