/* Volvix Home — service worker: lets the app open without a connection.
   - the app files: always the newest from the server; the saved copy is only used when you're offline
   - your library, profile and playlists: the network first, the last copy when you're offline
   - covers: cached as they're seen
   Songs are NOT handled here: downloaded songs live in the page's own cache (downloads.js). */
const SHELL = 'vh-shell-v3';
const DATA = 'vh-api';          // cleared when you sign out
const FRESH = ['/api/library', '/api/me', '/api/me/data', '/api/people', '/api/shared', '/api/household', '/api/notifications', '/api/status'];

self.addEventListener('install', function () { self.skipWaiting(); });
self.addEventListener('activate', function (e) {
  // drop the old app-file cache (an older version served stale files first)
  e.waitUntil(caches.keys().then(function (names) {
    return Promise.all(names.filter(function (n) { return n !== SHELL && n !== DATA && n !== 'vh-audio'; }).map(function (n) { return caches.delete(n); }));
  }).then(function () { return self.clients.claim(); }));
});

self.addEventListener('fetch', function (e) {
  const req = e.request;
  if (req.method !== 'GET') return;
  const url = new URL(req.url);
  if (url.origin !== location.origin) return;
  const p = url.pathname;
  if (p.indexOf('/api/stream/') === 0 || p.indexOf('/api/devices/') === 0 || p.indexOf('/api/party') === 0) return; // live: never cached
  if (FRESH.indexOf(p) !== -1 || p.indexOf('/api/lyrics/') === 0 || p.indexOf('/api/credits/') === 0) {
    e.respondWith(networkFirst(req, DATA));
  } else if (p.indexOf('/api/cover/') === 0 || p.indexOf('/api/discover/image/') === 0 || p.indexOf('/api/avatar/') === 0) {
    e.respondWith(staleWhileRevalidate(req, DATA));
  } else if (p.indexOf('/api/') !== 0) {
    e.respondWith(networkFirst(req, SHELL, req.mode === 'navigate' ? '/' : null));
  }
});

function networkFirst(req, name, fallbackUrl) {
  return fetch(req, { cache: 'no-cache' }).then(function (res) {
    if (res.ok) { const copy = res.clone(); caches.open(name).then(function (c) { c.put(req, copy); }); }
    return res;
  }).catch(function () {
    return caches.match(req).then(function (hit) { return hit || (fallbackUrl ? caches.match(fallbackUrl) : null) || new Response('{"error":"You\'re offline."}', { status: 503, headers: { 'Content-Type': 'application/json' } }); });
  });
}

function staleWhileRevalidate(req, name, fallbackUrl) {
  return caches.open(name).then(function (c) {
    return c.match(req, { ignoreSearch: false }).then(function (hit) {
      const net = fetch(req).then(function (res) { if (res.ok) c.put(req, res.clone()); return res; }).catch(function () { return null; });
      if (hit) return hit;
      return net.then(function (res) {
        if (res) return res;
        return (fallbackUrl ? c.match(fallbackUrl) : Promise.resolve(null)).then(function (f) { return f || new Response('', { status: 504 }); });
      });
    });
  });
}
