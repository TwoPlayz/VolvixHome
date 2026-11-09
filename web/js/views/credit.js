/* Volvix Home — a person's credits: every song in your library they worked on, and in what role. */
window.Spin = window.Spin || {};
Spin.views = Spin.views || {};
(function (S) {
  'use strict';

  S.views.credit = {
    render: function (root, params) {
      const name = params.name;
      const found = S.creditedSongs(name);
      if (!found.ids.length) { root.innerHTML = S.emptyState('user', 'No credits found', 'Nobody called "' + name + '" is credited in your library.'); return; }
      const artist = S.data.artists.find(function (n) { return n.toLowerCase() === name.toLowerCase(); });
      const initials = name.split(/\s+/).filter(function (w) { return /\w/.test(w); }).map(function (w) { return w[0]; }).slice(0, 2).join('');
      const roles = Object.keys(found.roles).map(function (r) { return r + ' · ' + found.roles[r]; });
      const ctx = { type: 'credit', name: name };
      root.innerHTML =
        '<header class="profile-head">' +
          '<span class="artist-hero">' + S.esc(initials) + '</span>' +
          '<div class="profile-info"><div class="eyebrow">Credits</div>' +
            '<h1 class="album-title">' + S.esc(name) + '</h1>' +
            '<div class="album-meta">' + found.ids.length + ' song' + (found.ids.length === 1 ? '' : 's') + ' in your library</div>' +
            '<div class="credit-roles">' + roles.map(function (r) { return '<span class="chip-src">' + S.esc(r) + '</span>'; }).join('') + '</div>' +
            '<div class="profile-actions">' +
              '<button class="btn btn-primary" data-credit-play>' + S.icon('play', 16) + 'Play</button>' +
              (artist ? '<a class="btn btn-secondary" href="' + S.artistUrl(artist) + '">' + S.icon('user', 16) + 'Artist page</a>' : '') +
            '</div>' +
          '</div>' +
        '</header>' +
        '<section class="section"><div class="section-title"><div><h2>Songs</h2></div></div>' +
          S.trackList(found.ids, { showCover: true, showAlbum: true, context: ctx }) + '</section>';
      if (S.markPlaying) S.markPlaying();
      root.onclick = function (e) {
        if (e.target.closest('[data-credit-play]')) S.player.play(found.ids, -1, ctx);
      };
    }
  };
})(window.Spin);
