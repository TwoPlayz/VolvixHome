/* Volvix Home — Search your library: artists, albums, songs and playlists. */
window.Spin = window.Spin || {};
Spin.views = Spin.views || {};
(function (S) {
  'use strict';

  const ui = { q: '', type: 'all' };
  const TYPES = [['all', 'All'], ['artists', 'Artists'], ['albums', 'Albums'], ['songs', 'Songs'], ['playlists', 'Playlists']];

  function norm(s) { return String(s || '').toLowerCase().normalize('NFD').replace(/[̀-ͯ]/g, ''); }

  function find(q) {
    const n = norm(q).trim();
    const words = n.split(/\s+/).filter(Boolean);
    const hit = function (text) { const t = norm(text); return words.every(function (w) { return t.indexOf(w) !== -1; }); };
    const albums = S.data.albums.filter(function (a) { return hit(a.title + ' ' + a.artist + ' ' + (a.year || '') + ' ' + (a.genre || '')); });
    const artists = S.data.artists.filter(function (name) { return hit(name); });
    const songs = Object.keys(S.data.tracks).map(S.data.track).filter(function (t) { return hit(t.title + ' ' + t.artist); });
    const playlists = S.pl.all().concat(S.household.allPlaylists()).filter(function (p) { return hit(p.name + ' ' + (p.description || '')); });
    // exact-ish matches first
    const starts = function (text) { return norm(text).indexOf(n) === 0 ? 0 : 1; };
    albums.sort(function (a, b) { return starts(a.title) - starts(b.title); });
    artists.sort(function (a, b) { return starts(a) - starts(b); });
    songs.sort(function (a, b) { return starts(a.title) - starts(b.title); });
    return { albums: albums, artists: artists, songs: songs, playlists: playlists };
  }

  function section(title, html, more) {
    return '<section class="section"><div class="section-title"><div><h2>' + title + '</h2></div>' +
      (more ? '<button class="see-all" data-type="' + more + '">Show all</button>' : '') + '</div>' + html + '</section>';
  }
  function artistChip(name) {
    const first = S.data.albumsBy(name)[0] || S.data.appearsOn(name)[0];
    return '<a class="artist" href="' + S.artistUrl(name) + '"><span class="artist-art">' + S.cover(first) + '</span>' +
      '<span class="artist-name">' + S.esc(name) + '</span><span class="artist-sub">Artist</span></a>';
  }

  function results() {
    if (!ui.q.trim()) {
      // nothing typed: everything, newest first
      const all = S.data.albums.slice().sort(function (a, b) { return b.added - a.added; });
      return all.length ? section('Everything on your server', '<div class="grid">' + all.map(function (a) { return S.albumCard(a); }).join('') + '</div>') :
        S.emptyState('music', 'No music yet', 'Music added to the server shows up here.');
    }
    const r = find(ui.q);
    const show = function (t) { return ui.type === 'all' || ui.type === t; };
    const all = ui.type === 'all';
    let html = '';
    if (show('artists') && r.artists.length) {
      const list = all ? r.artists.slice(0, 6) : r.artists;
      html += section('Artists', '<div class="artists">' + list.map(artistChip).join('') + '</div>', all && r.artists.length > 6 ? 'artists' : null);
    }
    if (show('songs') && r.songs.length) {
      const list = all ? r.songs.slice(0, 5) : r.songs.slice(0, 200);
      html += section('Songs', S.trackList(list.map(function (t) { return t.id; }), { showCover: true, showAlbum: true, numbered: false, context: { type: 'search', name: 'Search' } }),
        all && r.songs.length > 5 ? 'songs' : null);
    }
    if (show('albums') && r.albums.length) {
      const list = all ? r.albums.slice(0, 12) : r.albums;
      html += section('Albums', '<div class="grid">' + list.map(function (a) { return S.albumCard(a); }).join('') + '</div>', all && r.albums.length > 12 ? 'albums' : null);
    }
    if (show('playlists') && r.playlists.length) html += section('Playlists', '<div class="grid">' + r.playlists.map(S.playlistCard).join('') + '</div>');
    if (html) return html;
    // with Lidarr, music you don't have yet is listed right below, so a quiet line is enough
    if (ui.q.trim().length >= 2 && !(S.dl && S.dl.isOffline())) return '<p class="search-none">Nothing in your library for "' + S.esc(ui.q) + '".</p>';
    return S.emptyState('search', 'No results for "' + ui.q + '"', 'Check the spelling, or try fewer words.');
  }

  S.views.search = {
    render: function (root, params) {
      if (params && params.q) { ui.q = params.q; ui.type = 'all'; }
      root.innerHTML =
        '<header class="page-head"><h1 class="page-title">Search</h1></header>' +
        '<div class="search-box">' + S.icon('search', 20) +
          '<input class="search-input" type="search" placeholder="Artists, albums, songs, playlists" aria-label="Search" value="' + S.esc(ui.q) + '">' +
          '<button class="input-action search-clear" aria-label="Clear search"' + (ui.q ? '' : ' hidden') + '>' + S.icon('x', 16) + '</button>' +
        '</div>' +
        '<div class="search-filters"><div class="chips" role="radiogroup" aria-label="Result type">' + TYPES.map(function (t) {
          return '<button class="chip' + (ui.type === t[0] ? ' active' : '') + '" data-type="' + t[0] + '" role="radio" aria-checked="' + (ui.type === t[0]) + '">' + t[1] + '</button>';
        }).join('') + '</div></div>' +
        '<div data-results>' + results() + '</div>' +
        // music you don't have yet, found through Lidarr
        '<div data-get-music></div>';

      const input = S.$('.search-input', root);
      const clear = S.$('.search-clear', root);
      let lidarrTimer = null;
      function lookFurther() {
        clearTimeout(lidarrTimer);
        const host = S.$('[data-get-music]', root);
        if (ui.q.trim().length < 2 || (S.dl && S.dl.isOffline()) || (ui.type !== 'all' && ui.type !== 'albums' && ui.type !== 'artists' && ui.type !== 'songs')) { if (host) host.innerHTML = ''; return; }
        lidarrTimer = setTimeout(function () { S.getMusic.searchInto(host, ui.q.trim()); }, 600);
      }
      function update() {
        lookFurther();
        S.$('[data-results]', root).innerHTML = results();
        clear.hidden = !ui.q;
        S.$$('[data-type]', root).forEach(function (c) { if (c.classList.contains('chip')) { c.classList.toggle('active', c.dataset.type === ui.type); c.setAttribute('aria-checked', c.dataset.type === ui.type); } });
      }
      let timer = null;
      input.addEventListener('input', function () { ui.q = input.value; clearTimeout(timer); timer = setTimeout(update, 120); });
      clear.addEventListener('click', function () { ui.q = ''; input.value = ''; input.focus(); update(); });
      root.onclick = function (e) {
        const ty = e.target.closest('[data-type]');
        if (ty) { ui.type = ty.dataset.type; update(); if (ty.classList.contains('see-all')) S.$('#main').scrollTop = 0; }
      };
      if (!('ontouchstart' in window)) input.focus();
      lookFurther();
    }
  };
})(window.Spin);
