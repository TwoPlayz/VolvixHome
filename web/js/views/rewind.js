/* Volvix Home — Rewind: your year in music. It opens on January 1 for the year that just ended
   (so 2026's Rewind appears on 1 Jan 2027), built from the plays recorded in stats.js. */
window.Spin = window.Spin || {};
Spin.views = Spin.views || {};
(function (S) {
  'use strict';

  const thisYear = function () { return new Date().getFullYear(); };

  /** Years you can open now: every finished year you have plays for. */
  function openYears() { return S.stats.years().filter(function (y) { return y < thisYear(); }); }

  function summary(y) {
    const d = S.stats.year(y) || { plays: {}, sec: 0 };
    const songs = [], artists = {}, albums = {}, genres = {};
    let total = 0;
    Object.keys(d.plays).forEach(function (id) {
      const t = S.data.track(id); if (!t) return;
      const n = d.plays[id], al = S.data.album(t.albumId);
      total += n; songs.push({ t: t, n: n });
      artists[t.artist] = (artists[t.artist] || 0) + n;
      if (al) { albums[al.id] = (albums[al.id] || 0) + n; if (al.genre) genres[al.genre] = (genres[al.genre] || 0) + n; }
    });
    const rank = function (o) { return Object.keys(o).sort(function (a, b) { return o[b] - o[a]; }).map(function (k) { return { k: k, n: o[k] }; }); };
    songs.sort(function (a, b) { return b.n - a.n; });
    return { songs: songs, artists: rank(artists), albums: rank(albums), genres: rank(genres), plays: total, minutes: Math.round(d.sec / 60) };
  }

  /** A card for Home when last year's Rewind is ready. */
  S.rewindBanner = function () {
    const y = thisYear() - 1;
    if (openYears().indexOf(y) === -1) return '';
    return '<a class="rewind-banner" href="#/rewind/' + y + '"><span class="rewind-badge">' + y + '</span><span><strong>Your ' + y + ' Rewind is ready</strong>' +
      '<small>See what you played the most</small></span>' + S.icon('chevronRight', 20) + '</a>';
  };

  function stat(n, label) { return '<div class="rw-stat"><strong>' + n + '</strong><span>' + label + '</span></div>'; }

  S.views.rewind = {
    render: function (root, params) {
      const years = openYears();
      const y = Number(params.year) || years[years.length - 1];
      if (!y || y >= thisYear()) {
        const have = S.stats.year(thisYear());
        root.innerHTML = '<header class="page-head"><h1 class="page-title">Rewind</h1></header>' +
          S.emptyState('history', 'Your ' + thisYear() + ' Rewind opens on January 1, ' + (thisYear() + 1),
            have ? 'It\'s building as you listen: ' + Object.keys(have.plays).length + ' songs so far.' : 'Play some music and it starts building.',
            years.length ? '<a class="btn btn-secondary" href="#/rewind/' + years[years.length - 1] + '">See ' + years[years.length - 1] + '</a>' : '');
        return;
      }
      const r = summary(y);
      if (!r.plays) { root.innerHTML = '<header class="page-head"><h1 class="page-title">Rewind ' + y + '</h1></header>' + S.emptyState('history', 'No plays in ' + y, 'Nothing was recorded that year.'); return; }
      const topAlbum = S.data.album(r.albums[0].k);
      const ids = r.songs.slice(0, 5).map(function (s) { return s.t.id; });
      root.innerHTML =
        '<header class="rewind-hero"><div class="rewind-year">' + y + '</div><h1>Your year in music</h1>' +
          '<div class="rw-stats">' + stat(r.minutes.toLocaleString(), 'minutes listened') + stat(r.plays.toLocaleString(), 'plays') + stat(r.songs.length.toLocaleString(), 'different songs') + '</div></header>' +
        '<section class="section"><div class="section-title"><div><h2>Top songs</h2></div>' +
          '<button class="btn btn-secondary btn-sm" data-rw-play>' + S.icon('play', 14) + 'Play them</button></div>' +
          '<div class="rw-list">' + r.songs.slice(0, 5).map(function (s, i) {
            const al = S.data.album(s.t.albumId);
            return '<div class="rw-row"><span class="rw-rank">' + (i + 1) + '</span><span class="rw-cover">' + S.cover(al) + '</span>' +
              '<span class="rw-text"><strong>' + S.albumLink(al, s.t.title) + '</strong><small>' + S.artistLink(s.t.artist) + '</small></span><span class="rw-n">' + s.n + ' plays</span></div>';
          }).join('') + '</div></section>' +
        '<section class="section"><div class="section-title"><div><h2>Top artists</h2></div></div><div class="rw-chips">' +
          r.artists.slice(0, 5).map(function (a, i) { return '<a class="rw-chip" href="' + S.artistUrl(a.k) + '"><b>' + (i + 1) + '</b>' + S.esc(a.k) + '<small>' + a.n + ' plays</small></a>'; }).join('') + '</div></section>' +
        '<section class="section"><div class="section-title"><div><h2>Top album</h2></div></div>' + (topAlbum ? S.shelf('', [S.albumCard(topAlbum)].join('')) : '') + '</section>' +
        (r.genres.length ? '<p class="set-hint">Most played genre: <strong>' + S.esc(r.genres[0].k) + '</strong></p>' : '') +
        (years.length > 1 ? '<p class="set-hint">Other years: ' + years.filter(function (v) { return v !== y; }).map(function (v) { return '<a href="#/rewind/' + v + '">' + v + '</a>'; }).join(' · ') + '</p>' : '');
      root.onclick = function (e) { if (e.target.closest('[data-rw-play]')) S.player.play(ids, 0, { type: 'rewind', name: 'Your ' + y + ' Rewind' }); };
    }
  };
})(window.Spin);
