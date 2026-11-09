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
  let canGet = L.canRequest, lidarrOn = L.configured; // set from each search: can this person download?
  S.getMusic = S.getMusic || {};
  /** Shown instead of a Get button when downloading isn't set up for them. */
  function noGet() {
    return S.data.can('settings') && !lidarrOn
      ? '<a class="btn btn-secondary btn-sm" href="#/account/settings">Connect Lidarr</a>'
      : '<span class="gm-state">' + (lidarrOn ? 'Admins only' : 'Ask an admin') + '</span>';
  }
  /** The state of an album / artist: here already (opens it), asked for, or a Get button. */
  function stateButton(item, kind) {
    if (item.onServer) {
      const href = kind === 'album' ? '#/album/' + item.onServer : S.artistUrl(item.onServer);
      return '<a class="btn btn-secondary btn-sm" href="' + href + '">' + S.icon('check', 14) + 'In your library</a>';
    }
    if (item.requested || item.monitored) return '<span class="gm-state">' + S.icon('download', 14) + 'Requested</span>';
    if (!canGet) return noGet();
    return '<button class="btn btn-primary btn-sm" data-get="' + kind + '" data-id="' + S.esc(item.foreignId) + '">' + S.icon('download', 14) + (kind === 'artist' ? 'Get all' : 'Get') + '</button>';
  }
  /** Where clicking a result goes: your own page if it's already here, otherwise a page about it with everything it holds. */
  const albumHref = function (a) { return a.onServer ? '#/album/' + a.onServer : '#/get/album/' + a.foreignId; };
  const artistHref = function (a) { return a.onServer ? S.artistUrl(a.onServer) : '#/get/artist/' + a.foreignId; };
  const artistOfAlbumHref = function (a) { return a.artistId ? '#/get/artist/' + a.artistId : ''; };

  function albumCard(a) {
    return '<div class="gm-card"><a class="gm-link" href="' + albumHref(a) + '">' + art(a.image, false, a.title) +
      '<span class="gm-text"><span class="t-name">' + S.esc(a.title) + '</span>' +
      '<span class="t-sub">' + S.esc([a.artist, a.year, a.type && a.type !== 'Album' ? a.type : ''].filter(Boolean).join(' · ')) + '</span></span></a>' +
      stateButton(a, 'album') + '</div>';
  }
  /** A song: Lidarr fetches releases, not songs, so offer its single (if any) and its album. Click opens the album with all its songs. */
  function songCard(s) {
    let action;
    if (s.onServer) action = '<a class="btn btn-secondary btn-sm" href="#/album/' + s.onServer + '">' + S.icon('check', 14) + 'In your library</a>';
    else if (s.requested) action = '<span class="gm-state">' + S.icon('download', 14) + 'Requested</span>';
    else if (!canGet) action = noGet();
    else {
      const btns = [];
      if (s.singleId) btns.push('<button class="btn btn-primary btn-sm" data-get="album" data-id="' + S.esc(s.singleId) + '" data-label="' + S.esc(s.title) + ' (single)">' + S.icon('download', 14) + 'Get the single</button>');
      if (s.albumId) btns.push('<button class="btn ' + (s.singleId ? 'btn-secondary' : 'btn-primary') + ' btn-sm" data-get="album" data-id="' + S.esc(s.albumId) + '" data-label="the album with ' + S.esc(s.title) + '">' + S.icon('download', 14) + 'Get the album</button>');
      action = '<div class="gm-actions">' + btns.join('') + '</div>';
    }
    const target = s.onServer ? '#/album/' + s.onServer : (s.albumId || s.singleId) ? '#/get/album/' + (s.albumId || s.singleId) : '';
    return '<div class="gm-card gm-song"><' + (target ? 'a class="gm-link" href="' + target + '"' : 'div class="gm-link"') + '>' + art(s.image, false, s.title) +
      '<span class="gm-text"><span class="t-name">' + S.esc(s.title) + '</span>' +
      '<span class="t-sub">' + S.esc([s.artist, s.year].filter(Boolean).join(' · ')) + '</span></span></' + (target ? 'a' : 'div') + '>' +
      action + '</div>';
  }
  function artistRow(r) {
    return '<div class="gm-card gm-artist"><a class="gm-link" href="' + artistHref(r) + '">' + art(r.image, true, r.name) +
      '<span class="gm-text"><span class="t-name">' + S.esc(r.name) + '</span>' +
      '<span class="t-sub">' + S.esc(['Artist', r.disambiguation].filter(Boolean).join(' · ')) + '</span></span></a>' +
      stateButton(r, 'artist') + '</div>';
  }

  let seq = 0;
  const SHOW = { artists: 4, songs: 6, albums: 8 };
  /** Fill `host` with results from MusicBrainz for `q`, only the kinds the filter allows (Search calls this, debounced). */
  function searchInto(host, q, type) {
    if (!host) return;
    type = type || 'all';
    const mine = ++seq;
    host.innerHTML = '<section class="section"><div class="section-title"><div><h2>Get more music</h2><p>Searching…</p></div></div></section>';
    S.server.json('GET', '/api/lidarr/search?term=' + encodeURIComponent(q)).then(function (r) {
      if (mine !== seq) return; // a newer search took over
      canGet = !!r.canRequest; lidarrOn = !!r.configured;
      const open = {}; // which sections the person has expanded
      function draw() {
        const want = function (k) { return type === 'all' || type === k; };
        const part = function (key, title, list, card, wrap) {
          if (!want(key) || !list.length) return '';
          const limit = type === 'all' && !open[key] ? SHOW[key] : list.length;
          return '<h3 class="gm-sub">' + title + '</h3><div class="' + wrap + '">' + list.slice(0, limit).map(card).join('') + '</div>' +
            (list.length > limit ? '<button class="btn btn-ghost btn-sm gm-more" data-gm-more="' + key + '">Show ' + (list.length - limit) + ' more</button>' : '');
        };
        const html = part('artists', 'Artists', r.artists || [], artistRow, 'gm-list') + part('songs', 'Songs', r.songs || [], songCard, 'gm-list') + part('albums', 'Albums', r.albums || [], albumCard, 'gm-grid');
        if (!html) { host.innerHTML = ''; return; }
        host.innerHTML = '<section class="section"><div class="section-title"><div><h2>Get more music</h2>' +
          '<p>' + (canGet ? 'Not in your library yet? Open an artist or album to see everything, then get it. Lidarr downloads it and adds it here when it\'s done.' : lidarrOn ? 'Only admins can ask for new music.' : S.data.can('settings') ? 'Connect Lidarr in Settings to download these.' : 'Ask an admin to connect Lidarr so these can be downloaded.') + '</p></div></div>' +
          html + '<p class="gm-credit">Results from MusicBrainz.</p></section>';
      }
      draw();
      host.onclick = function (e) { const b = e.target.closest('[data-gm-more]'); if (b) { open[b.dataset.gmMore] = true; draw(); } };
    }).catch(function (e) {
      if (mine !== seq) return;
      host.innerHTML = '<section class="section"><div class="section-title"><div><h2>Get more music</h2><p class="gm-error">' + S.esc(e.message) + '</p></div></div></section>';
    });
  }

  // the artist and album pages (discover.js) reuse these
  S.getMusic.searchInto = searchInto;
  S.getMusic.helpers = { art: art, stateButton: stateButton, noGet: noGet, albumHref: albumHref, artistHref: artistHref, artistOfAlbumHref: artistOfAlbumHref, setCan: function (c, l) { canGet = !!c; lidarrOn = !!l; } };
  /** Cancel a request that hasn't been downloaded yet, and show new music without reloading. */
  document.addEventListener('click', function (e) {
    const show = e.target.closest('[data-new-music-show]');
    if (show) { S.data.sync().then(function () { seenTracks = Object.keys(S.data.tracks).length; S.app.refresh(); S.toast('Library updated', 'check'); }); return; }
    const c = e.target.closest('[data-cancel-req]');
    if (!c) return;
    const id = c.dataset.cancelReq, name = c.dataset.name;
    S.modal({
      title: 'Cancel this request?',
      body: '<p class="modal-text">Lidarr will stop looking for <strong>' + S.esc(name) + '</strong> and anything it started downloading is removed. Music that has already been downloaded stays.</p>',
      actions: [{ label: 'Keep it' }, { label: 'Cancel the request', kind: 'danger', onClick: function (close) {
        S.server.json('DELETE', '/api/lidarr/request/' + encodeURIComponent(id)).then(function () { close(); S.toast('Request cancelled', 'check'); S.app.refresh(); })
          .catch(function (x) { close(); S.toast(x.message, 'x'); });
      } }]
    });
  });

  /** Get buttons anywhere (Search, Requests). */
  document.addEventListener('click', function (e) {
    const b = e.target.closest('[data-get]');
    if (!b) return;
    e.preventDefault();
    b.disabled = true;
    b.innerHTML = S.icon('download', 14) + 'Asking Lidarr…';
    S.server.json('POST', '/api/lidarr/request', { kind: b.dataset.get, foreignId: b.dataset.id }).then(function (r) {
      const what = b.dataset.label || (r.request.kind === 'artist' ? 'Everything by ' + r.request.artist : r.request.title);
      const box = b.closest('.gm-actions') || b;
      box.outerHTML = '<span class="gm-state">' + S.icon('download', 14) + 'Requested</span>';
      S.linkToast(what.charAt(0).toUpperCase() + what.slice(1) + ' requested. It shows up in your library when Lidarr is done.', 'Requests', '/requests');
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
        S.emptyState('download', 'Lidarr isn\'t connected', S.data.can('settings') ? 'Connect Lidarr in Settings to search for music you don\'t have and add it automatically.' : 'Ask whoever runs this server to connect Lidarr.',
          S.data.can('settings') ? '<a class="btn btn-primary" href="#/account/settings">Connect Lidarr</a>' : '');
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
            : '<span class="gm-state">' + (downloading ? 'Downloading' : 'Waiting for Lidarr') + '</span>' +
              (x.byId === S.data.me.serverId || S.data.can('members') ? '<button class="btn btn-ghost btn-sm" data-cancel-req="' + S.esc(x.foreignId) + '" data-name="' + S.esc(x.kind === 'artist' ? 'everything by ' + x.artist : x.title) + '">Cancel</button>' : '');
          return '<div class="list-row">' + art(x.image, x.kind === 'artist', x.title) +
            '<span class="list-text"><span class="t-name">' + S.esc(x.kind === 'artist' ? 'Everything by ' + x.artist : x.title) + '</span>' +
            '<span class="t-sub">' + S.esc((x.kind === 'album' ? x.artist + ' · ' : '') + 'Requested by ' + (x.byId === S.data.me.serverId ? 'you' : x.by) + ' · ' + ago(x.at)) + '</span></span>' +
            where + (x.problem ? '<span class="gm-error gm-problem">' + S.esc(x.problem) + '</span>' : '') + '</div>';
        }).join('') + '</div>' : S.emptyState('search', 'Nothing requested yet', 'Search for an artist or album. Anything not in your library can be requested from there.',
          '<a class="btn btn-primary" href="#/search">Search</a>')) +
        '</section>';
      // new music arrived since this page opened: offer to load it
      if (seenTracks == null) seenTracks = Object.keys(S.data.tracks).length;
      if (status.tracks > seenTracks && !S.$('[data-new-music]', root)) {
        const bar = document.createElement('div');
        bar.className = 'gm-new';
        bar.setAttribute('data-new-music', '');
        bar.innerHTML = S.icon('check', 16) + '<span>New music is in your library.</span><button class="btn btn-primary btn-sm" data-new-music-show>Show it</button>';
        root.insertBefore(bar, root.children[1]);
      }
    }).catch(function () { /* signed out etc.: the next call reloads */ });
  }

  S.views.requests = { render: render };
})(window.Spin);
