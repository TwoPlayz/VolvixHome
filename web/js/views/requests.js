/* Volvix Home — getting music with Lidarr: the "Get more music" results in Search, and the Requests page
 * (what's downloading, what's waiting, who asked). Lidarr does the finding and downloading; when it imports
 * an album into the music folder, the server rescans and it appears in your library. */
window.Spin = window.Spin || {};
Spin.views = Spin.views || {};
(function (S) {
  'use strict';

  const L = (window.VH && window.VH.lidarr) || { configured: false, canRequest: false };
  S.lidarr = L;

  /* ---------- result cards (Search) ---------- */
  function art(img, round, fallbackText) {
    const initials = String(fallbackText || '?').split(/\s+/).map(function (w) { return w[0]; }).join('').slice(0, 2).toUpperCase();
    return '<span class="gm-art' + (round ? ' round' : '') + '">' + (img ? '<img src="' + S.esc(img) + '" alt="" loading="lazy" referrerpolicy="no-referrer" onerror="this.remove()">' : '') +
      '<span class="gm-initials">' + S.esc(initials) + '</span></span>';
  }
  function stateButton(item, kind) {
    if (item.onServer) {
      const href = kind === 'album' ? '#/album/' + item.onServer : S.artistUrl(item.onServer);
      return '<a class="btn btn-secondary btn-sm" href="' + href + '">' + S.icon('check', 14) + 'In your library</a>';
    }
    if (item.requested || item.monitored) return '<span class="gm-state">' + S.icon('download', 14) + 'Requested</span>';
    return '<button class="btn btn-primary btn-sm" data-get="' + kind + '" data-id="' + S.esc(item.foreignId) + '">' + S.icon('download', 14) + (kind === 'artist' ? 'Get all' : 'Get') + '</button>';
  }
  function albumCard(a) {
    return '<div class="gm-card">' + art(a.image, false, a.title) +
      '<div class="gm-text"><span class="t-name">' + S.esc(a.title) + '</span>' +
      '<span class="t-sub">' + S.esc([a.artist, a.year, a.type && a.type !== 'Album' ? a.type : ''].filter(Boolean).join(' · ')) + '</span></div>' +
      stateButton(a, 'album') + '</div>';
  }
  function artistRow(r) {
    return '<div class="gm-card gm-artist">' + art(r.image, true, r.name) +
      '<div class="gm-text"><span class="t-name">' + S.esc(r.name) + '</span>' +
      '<span class="t-sub">' + S.esc(['Artist', r.disambiguation].filter(Boolean).join(' · ')) + '</span></div>' +
      stateButton(r, 'artist') + '</div>';
  }

  let seq = 0;
  /** Fill `host` with Lidarr results for `q` (Search calls this, debounced). */
  function searchInto(host, q) {
    if (!L.canRequest || !host) return;
    const mine = ++seq;
    host.innerHTML = '<section class="section"><div class="section-title"><div><h2>Get more music</h2><p>Searching with Lidarr…</p></div></div></section>';
    S.server.json('GET', '/api/lidarr/search?term=' + encodeURIComponent(q)).then(function (r) {
      if (mine !== seq) return; // a newer search took over
      if (!r.artists.length && !r.albums.length) { host.innerHTML = ''; return; }
      host.innerHTML = '<section class="section"><div class="section-title"><div><h2>Get more music</h2>' +
          '<p>Not in your library yet. Lidarr finds it and adds it here when it\'s done.</p></div></div>' +
        (r.artists.length ? '<div class="gm-list">' + r.artists.slice(0, 3).map(artistRow).join('') + '</div>' : '') +
        (r.albums.length ? '<div class="gm-grid">' + r.albums.map(albumCard).join('') + '</div>' : '') +
        '</section>';
    }).catch(function (e) {
      if (mine !== seq) return;
      host.innerHTML = '<section class="section"><div class="section-title"><div><h2>Get more music</h2><p class="gm-error">' + S.esc(e.message) + '</p></div></div></section>';
    });
  }

  /** Get buttons anywhere (Search, Requests). */
  document.addEventListener('click', function (e) {
    const b = e.target.closest('[data-get]');
    if (!b) return;
    e.preventDefault();
    b.disabled = true;
    b.innerHTML = S.icon('download', 14) + 'Asking Lidarr…';
    S.server.json('POST', '/api/lidarr/request', { kind: b.dataset.get, foreignId: b.dataset.id }).then(function (r) {
      const what = r.request.kind === 'artist' ? 'Everything by ' + r.request.artist : r.request.title;
      b.outerHTML = '<span class="gm-state">' + S.icon('download', 14) + 'Requested</span>';
      S.linkToast(what + ' requested. It shows up in your library when Lidarr is done.', 'Requests', '/requests');
    }).catch(function (x) {
      b.disabled = false;
      b.innerHTML = S.icon('download', 14) + 'Try again';
      S.toast(x.message, 'x');
    });
  });

  /* ---------- Requests page ---------- */
  function timeLeft(t) {
    if (!t) return '';
    const p = String(t).split(':').map(Number); // "hh:mm:ss" or "d.hh:mm:ss"
    const secs = p.length === 3 ? p[0] * 3600 + p[1] * 60 + p[2] : 0;
    if (!secs) return '';
    return secs >= 3600 ? Math.round(secs / 3600) + ' h left' : secs >= 60 ? Math.round(secs / 60) + ' min left' : 'Almost done';
  }
  function ago(sec) {
    const d = Math.max(0, Date.now() / 1000 - sec);
    return d < 60 ? 'just now' : d < 3600 ? Math.round(d / 60) + ' min ago' : d < 86400 ? Math.round(d / 3600) + ' h ago' : Math.round(d / 86400) + ' d ago';
  }
  const STATES = { downloading: 'Downloading', importPending: 'Ready to import', importing: 'Importing', imported: 'Imported', failedPending: 'Problem', failed: 'Failed', queued: 'Waiting', paused: 'Paused', delay: 'Waiting', completed: 'Finishing' };

  let poll = null, seenTracks = null;
  function render(root) {
    if (!L.configured) {
      root.innerHTML = '<header class="page-head"><h1 class="page-title">Requests</h1></header>' +
        S.emptyState('download', 'Lidarr isn\'t connected', S.data.me.admin ? 'Connect Lidarr in Settings to search for music you don\'t have and add it automatically.' : 'Ask whoever runs this server to connect Lidarr.',
          S.data.me.admin ? '<a class="btn btn-primary" href="#/account/settings">Connect Lidarr</a>' : '');
      return;
    }
    root.innerHTML = '<header class="page-head"><h1 class="page-title">Requests</h1>' +
      '<p class="page-sub">Music Lidarr is getting for you. It appears in your library as soon as it\'s imported.</p></header>' +
      '<div data-queue></div><div data-requests></div>';
    load(root);
    clearInterval(poll);
    poll = setInterval(function () {
      if (!document.body.contains(root) || !S.$('[data-queue]', root)) { clearInterval(poll); return; }
      load(root);
    }, 5000);
  }

  function load(root) {
    Promise.all([
      S.server.json('GET', '/api/lidarr/queue').catch(function (e) { return { error: e.message, items: [] }; }),
      S.server.json('GET', '/api/lidarr/requests'),
      S.server.json('GET', '/api/status')
    ]).then(function (r) {
      const q = r[0], reqs = r[1].requests, status = r[2];
      const qHost = S.$('[data-queue]', root), rHost = S.$('[data-requests]', root);
      if (!qHost) return;
      qHost.innerHTML = '<section class="section"><div class="section-title"><div><h2>Downloading</h2></div></div>' +
        (q.error ? '<p class="gm-error">' + S.esc(q.error) + '</p>' :
          q.items.length ? '<div class="dl-queue">' + q.items.map(function (d) {
            const pct = Math.round(d.progress * 100);
            const bad = d.status === 'warning' || d.status === 'error' || /fail/i.test(d.state);
            return '<div class="dlq-row' + (bad ? ' bad' : '') + '">' + art(d.image, false, d.title) +
              '<div class="dlq-text"><span class="t-name">' + S.esc(d.title) + '</span><span class="t-sub">' + S.esc(d.artist) + '</span>' +
                '<div class="dlq-bar"><span style="width:' + pct + '%"></span></div>' +
                '<span class="dlq-meta">' + S.esc(STATES[d.state] || d.state) + (d.state === 'downloading' ? ' · ' + pct + '%' + (timeLeft(d.timeLeft) ? ' · ' + timeLeft(d.timeLeft) : '') : '') +
                  (d.message ? ' · ' + S.esc(d.message) : '') + '</span></div></div>';
          }).join('') + '</div>' : '<p class="set-hint">Nothing downloading right now.</p>') +
        '</section>';
      rHost.innerHTML = '<section class="section"><div class="section-title"><div><h2>Requested</h2></div></div>' +
        (reqs.length ? '<div class="rows">' + reqs.map(function (x) {
          const downloading = q.items.some(function (d) { return d.foreignAlbumId === x.foreignId; });
          const where = x.onServer
            ? '<a class="btn btn-secondary btn-sm" href="' + (x.kind === 'album' ? '#/album/' + x.onServer : S.artistUrl(x.onServer)) + '">' + S.icon('check', 14) + 'In your library</a>'
            : '<span class="gm-state">' + (downloading ? 'Downloading' : 'Waiting for Lidarr') + '</span>';
          return '<div class="list-row">' + art(x.image, x.kind === 'artist', x.title) +
            '<span class="list-text"><span class="t-name">' + S.esc(x.kind === 'artist' ? 'Everything by ' + x.artist : x.title) + '</span>' +
            '<span class="t-sub">' + S.esc((x.kind === 'album' ? x.artist + ' · ' : '') + 'Requested by ' + (x.byId === S.data.me.serverId ? 'you' : x.by) + ' · ' + ago(x.at)) + '</span></span>' +
            where + '</div>';
        }).join('') + '</div>' : S.emptyState('search', 'Nothing requested yet', 'Search for an artist or album. Anything not in your library can be requested from there.',
          '<a class="btn btn-primary" href="#/search">Search</a>')) +
        '</section>';
      // new music arrived since this page opened: offer to load it
      if (seenTracks == null) seenTracks = Object.keys(S.data.tracks).length;
      if (status.tracks > seenTracks && !S.$('[data-new-music]', root)) {
        const bar = document.createElement('div');
        bar.className = 'gm-new';
        bar.setAttribute('data-new-music', '');
        bar.innerHTML = S.icon('check', 16) + '<span>New music is in your library.</span><button class="btn btn-primary btn-sm" onclick="location.reload()">Show it</button>';
        root.insertBefore(bar, root.children[1]);
      }
    }).catch(function () { /* signed out etc.: the next call reloads */ });
  }

  S.views.requests = { render: render };
  S.getMusic = { searchInto: searchInto };
})(window.Spin);
