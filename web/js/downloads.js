/* Volvix Home — downloads: keep songs on this device so they play with no connection.
   Songs go in the browser's own storage (Cache Storage) on THIS device only. Offline, only downloaded songs play:
   the rest are greyed out and skipped. Settings > Downloads has the "only play downloads" switch and a clean-up. */
window.Spin = window.Spin || {};
(function (S) {
  'use strict';

  const CACHE = 'vh-audio';
  const key = function () { return 'vh_dl_' + S.data.me.serverId; };
  let manifest = {};   // trackId -> bytes
  try { manifest = JSON.parse(localStorage.getItem(key()) || '{}') || {}; } catch (e) { manifest = {}; }
  const saveManifest = function () { try { localStorage.setItem(key(), JSON.stringify(manifest)); } catch (e) { /* full / private */ } };
  const urlFor = function (id) { return S.server.streamUrl(id); };
  const supported = typeof caches !== 'undefined';
  let busy = 0, blobUrl = null;
  const progress = { done: 0, total: 0 };

  // forget manifest entries whose file the browser threw away
  if (supported) caches.open(CACHE).then(function (c) { return c.keys(); }).then(function (reqs) {
    const have = {}; reqs.forEach(function (r) { have[new URL(r.url).pathname] = true; });
    let changed = false;
    Object.keys(manifest).forEach(function (id) { if (!have[urlFor(id)]) { delete manifest[id]; changed = true; } });
    if (changed) { saveManifest(); redraw(); }
  }).catch(function () {});

  function redraw() { try { S.app.refresh(); } catch (e) { /* app not up yet */ } }
  const settings = function () { return S.store.state.settings || {}; };

  const D = S.dl = {
    supported: supported,
    trackSet: function () { return manifest; },
    has: function (id) { return !!manifest[id]; },
    isOffline: function () { return !!settings().offlineOnly || navigator.onLine === false; },
    count: function () { return Object.keys(manifest).length; },
    bytes: function () { return Object.keys(manifest).reduce(function (s, id) { return s + (manifest[id] || 0); }, 0); },
    allDownloaded: function (ids) { return !!ids.length && ids.every(function (id) { return manifest[id]; }); },
    progress: progress,

    /** Download songs one at a time. Skips what's already here. */
    add: function (ids) {
      if (!supported) { S.toast('This browser can\'t keep downloads', 'x'); return Promise.resolve(); }
      const todo = ids.filter(function (id) { return !manifest[id] && S.data.track(id); });
      if (!todo.length) { S.toast('Already on this device', 'check'); return Promise.resolve(); }
      if (navigator.onLine === false) { S.toast('Connect to download', 'wifiOff'); return Promise.resolve(); }
      if (navigator.storage && navigator.storage.persist) navigator.storage.persist().catch(function () {});
      progress.done = 0; progress.total += todo.length; busy++;
      S.toast('Downloading ' + todo.length + ' song' + (todo.length === 1 ? '' : 's') + '…', 'download');
      return caches.open(CACHE).then(function (c) {
        let failed = 0;
        return todo.reduce(function (p, id) {
          return p.then(function () {
            return fetch(urlFor(id), { credentials: 'same-origin' }).then(function (r) {
              if (!r.ok) throw new Error('bad');
              const len = Number(r.headers.get('content-length')) || 0;
              return c.put(urlFor(id), r).then(function () { manifest[id] = len || 1; saveManifest(); });
            }).catch(function () { failed++; }).then(function () { progress.done++; redrawSoon(); });
          });
        }, Promise.resolve()).then(function () {
          busy--; if (!busy) { progress.total = 0; progress.done = 0; }
          S.toast(failed ? 'Downloaded ' + (todo.length - failed) + ', ' + failed + ' failed (storage full?)' : 'Downloaded ' + todo.length + ' song' + (todo.length === 1 ? '' : 's'), failed ? 'x' : 'check');
          redraw();
        });
      });
    },
    remove: function (ids) {
      if (!supported) return Promise.resolve();
      return caches.open(CACHE).then(function (c) {
        return Promise.all(ids.map(function (id) { delete manifest[id]; return c.delete(urlFor(id)); }));
      }).then(function () { saveManifest(); redraw(); });
    },
    removeAll: function () { return D.remove(Object.keys(manifest)); },

    /** An object URL for a downloaded song (the player plays this instead of the network). */
    blobUrl: function (id) {
      return caches.open(CACHE).then(function (c) { return c.match(urlFor(id)); }).then(function (r) { return r ? r.blob() : null; })
        .then(function (b) {
          if (!b) { delete manifest[id]; saveManifest(); return null; }
          if (blobUrl) URL.revokeObjectURL(blobUrl);
          blobUrl = URL.createObjectURL(b);
          return blobUrl;
        }).catch(function () { return null; });
    },

    /** A download button for a group of songs (album, playlist). */
    button: function (attrs, ids) {
      const done = D.allDownloaded(ids);
      return '<button class="icon-btn icon-btn-lg dl-btn' + (done ? ' on' : '') + '" ' + attrs + ' aria-label="' + (done ? 'Remove download' : 'Download') + '" title="' + (done ? 'Downloaded · tap to remove' : 'Download to this device') + '">' +
        S.icon(done ? 'check' : 'download', 22) + '</button>';
    }
  };

  let soon = null;
  function redrawSoon() { if (soon) return; soon = setTimeout(function () { soon = null; redraw(); }, 800); }

  /* ---------- offline / online ---------- */
  function pill() {
    let el = document.getElementById('offline-pill');
    const off = D.isOffline();
    if (!off) { if (el) el.remove(); return; }
    if (!el) { el = document.createElement('div'); el.id = 'offline-pill'; el.className = 'offline-pill'; document.body.appendChild(el); }
    el.innerHTML = S.icon('wifiOff', 14) + '<span>' + (navigator.onLine === false ? 'You\'re offline' : 'Offline mode') + ' · only downloaded songs play</span>';
  }
  function changed() { pill(); try { S.player.render(); } catch (e) { /* ignore */ } redraw(); }
  window.addEventListener('online', changed);
  window.addEventListener('offline', changed);
  S.dl.refreshOffline = changed;
  setTimeout(pill, 300);

  /* ---------- buttons ---------- */
  document.addEventListener('click', function (e) {
    const b = e.target.closest('[data-dl-album], [data-dl-playlist], [data-dl-track]');
    if (!b) return;
    e.preventDefault(); e.stopPropagation();
    let ids = [];
    if (b.dataset.dlAlbum) { const al = S.data.album(b.dataset.dlAlbum); ids = al ? al.trackIds : []; }
    else if (b.dataset.dlPlaylist) { const pl = S.pl.get ? S.pl.get(b.dataset.dlPlaylist) : null; ids = pl ? S.pl.trackIds(pl) : []; }
    else ids = [b.dataset.dlTrack];
    if (D.allDownloaded(ids)) D.remove(ids).then(function () { S.toast('Removed from this device', 'check'); });
    else D.add(ids);
  });
})(window.Spin);
