/* Volvix Home — Artist page: their albums, songs, and albums they appear on. */
window.Spin = window.Spin || {};
Spin.views = Spin.views || {};
(function (S) {
  'use strict';

  S.views.artist = {
    render: function (root, params) {
      const name = S.data.artists.find(function (n) { return S.artistSlug(n) === params.slug; });
      if (!name) { root.innerHTML = S.emptyState('user', 'Artist not found', 'They aren\'t in your library.'); return; }
      const albums = S.data.albumsBy(name).sort(function (a, b) { return (b.year || 0) - (a.year || 0); });
      const appears = S.data.appearsOn(name);
      const songs = [];
      albums.concat(appears).forEach(function (a) {
        a.trackIds.forEach(function (id) { if (S.data.track(id).artist.toLowerCase() === name.toLowerCase()) songs.push(id); });
      });
      const first = albums[0] || appears[0];
      const art = first ? first.art : { c1: '#333', fg: '#fff' };
      const initials = name.split(/\s+/).filter(function (w) { return /\w/.test(w); }).map(function (w) { return w[0]; }).slice(0, 2).join('');
      const ctx = { type: 'artist', name: name };

      root.innerHTML =
        '<header class="profile-head">' +
          (first && first.cover
            ? '<span class="artist-hero artist-hero-img">' + S.cover(first) + '</span>'
            : '<span class="artist-hero" style="background:' + art.c1 + ';color:' + art.fg + '">' + S.esc(initials) + '</span>') +
          '<div class="profile-info"><div class="eyebrow">Artist</div>' +
            '<h1 class="album-title">' + S.esc(name) + '</h1>' +
            '<div class="album-meta">' + [albums.length ? albums.length + ' album' + (albums.length > 1 ? 's' : '') : '', songs.length + ' song' + (songs.length === 1 ? '' : 's')].filter(Boolean).join(' · ') + '</div>' +
            '<div class="profile-actions">' +
              '<button class="btn btn-primary" data-artist-play>' + S.icon('play', 16) + 'Play</button>' +
              '<button class="btn btn-secondary" data-artist-shuffle>' + S.icon('shuffle', 16) + 'Shuffle</button>' +
            '</div>' +
          '</div>' +
        '</header>' +
        '<section class="section"><div class="section-title"><div><h2>Songs</h2></div></div>' +
          S.trackList(songs.slice(0, 10), { showCover: true, showAlbum: true, context: ctx }) + '</section>' +
        (albums.length ? S.shelf('Albums', albums.map(function (a) {
          return S.albumCard(a, { meta: '<span>' + (a.year || '') + '</span>' });
        }).join('')) : '') +
        (appears.length ? S.shelf('Appears on', appears.map(function (a) { return S.albumCard(a); }).join('')) : '');

      root.onclick = function (e) {
        if (e.target.closest('[data-artist-play]')) { S.player.state.shuffle = false; S.player.play(songs, 0, ctx); }
        else if (e.target.closest('[data-artist-shuffle]')) { S.player.state.shuffle = true; S.player.play(songs, 0, ctx); }
      };
    }
  };
})(window.Spin);
