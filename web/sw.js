/* Volvix Home — service worker: lets the app open without a connection.
   - the app files: shown from the cache straight away and refreshed in the background
   - your library, profile and playlists: the network first, the last copy when you're offline
   - covers: cached as they're seen
   Songs are NOT handled here: downloaded songs live in the page's own cache (downloads.js). */
const SHELL = 'vh-shell-v1';
const DATA = 'vh-api';          // cleared when you sign out
const FRESH = ['/api/library', '/api/me', '/api/me/data', '/api/people', '/api/shared', '/api/household', '/api/notifications', '/api/status'];

self.addEventListener('install', function () { self.skipWaiting(); });
self.addEventListener('activate', function (e) { e.waitUntil(self.clients.claim()); });

self.addEventListener('fetch', function (e) {
  const req = e.request;
  if (req.method !== 'GET') return;
  const url = new URL(req.url);
  if (url.origin !== location.origin) return;
  const p = url.pathname;
  if (p.indexOf('/api/stream/') === 0 || p.indexOf('/api/devices/') === 0 || p.indexOf('/api/party') === 0) return; // live: never cached
  if (FRESH.indexOf(p) !== -1 || p.indexOf('/api/lyrics/') === 0 || p.indexOf('/api/credits/') === 0) {
    e.respondWith(networkFirst(req, DATA));
  } else if (p.indexOf('/api/cover/') === 0 || p.indexOf('/api/avatar/') === 0) {
    e.respondWith(staleWhileRevalidate(req, DATA));
  } else if (p.indexOf('/api/') !== 0) {
    e.respondWith(staleWhileRevalidate(req, SHELL, req.mode === 'navigate' ? '/' : null));
  }
});

function networkFirst(req, name) {
  return fetch(req).then(function (res) {
    if (res.ok) { const copy = res.clone(); caches.open(name).then(function (c) { c.put(req, copy); }); }
    return res;
  }).catch(function () {
    return caches.match(req).then(function (hit) { return hit || new Response('{"error":"You\'re offline."}', { status: 503, headers: { 'Content-Type': 'application/json' } }); });
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
