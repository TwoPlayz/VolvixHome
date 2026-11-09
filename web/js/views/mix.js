/* Volvix Home — a "Made for you" mix: its songs, with Play, Shuffle and Save as playlist. */
window.Spin = window.Spin || {};
Spin.views = Spin.views || {};
(function (S) {
  'use strict';

  /** A card for a mix: a 2x2 of its cover art, the name and what it is. */
  S.mixCard = function (m) {
    const albums = [];
    m.ids.forEach(function (id) { const t = S.data.track(id); if (t && albums.indexOf(t.albumId) === -1 && albums.length < 4) albums.push(t.albumId); });
    const art = albums.length >= 4
      ? '<div class="mosaic">' + albums.map(function (id) { return S.cover(S.data.album(id)); }).join('') + '</div>'
      : S.cover(S.data.album(albums[0]));
    return '<a class="card mix-card" href="#/mix/' + m.id + '"><span class="card-art">' + art + '</span>' +
      '<span class="card-title">' + S.esc(m.title) + '</span><span class="card-sub">' + S.esc(m.sub) + '</span></a>';
  };

  S.views.mix = {
    render: function (root, params) {
      const m = S.mixes.get(params.id);
      if (!m) { root.innerHTML = S.emptyState('music', 'That mix isn\'t available', 'Mixes change as you listen. Open Made for you on Home.', '<a class="btn btn-secondary" href="#/home">Home</a>'); return; }
      const ctx = { type: 'mix', name: m.title };
      root.innerHTML =
        '<header class="profile-head"><span class="artist-hero mix-hero">' + S.icon(m.icon, 40) + '</span>' +
          '<div class="profile-info"><div class="eyebrow">Made for you</div><h1 class="album-title">' + S.esc(m.title) + '</h1>' +
          '<div class="album-meta">' + S.esc(m.sub) + ' · ' + m.ids.length + ' songs</div>' +
          '<div class="profile-actions"><button class="btn btn-primary" data-mix-play>' + S.icon('play', 16) + 'Play</button>' +
            '<button class="btn btn-secondary" data-mix-shuffle>' + S.icon('shuffle', 16) + 'Shuffle</button>' +
            '<button class="btn btn-secondary" data-mix-save>' + S.icon('plus', 16) + 'Save as playlist</button></div></div></header>' +
        '<section class="section">' + S.trackList(m.ids, { showCover: true, showAlbum: true, context: ctx }) + '</section>';
      root.onclick = function (e) {
        if (e.target.closest('[data-mix-play]')) S.player.play(m.ids, 0, ctx);
        else if (e.target.closest('[data-mix-shuffle]')) { if (!S.player.state.shuffle) S.player.toggleShuffle(); S.player.play(m.ids, -1, ctx); }
        else if (e.target.closest('[data-mix-save]')) {
          const pl = S.pl.create({ name: m.title + ' · ' + new Date().toLocaleDateString(undefined, { month: 'short', day: 'numeric' }), description: m.sub }, m.ids);
          S.toast('Saved as a playlist', 'check');
          S.router.go('/playlist/' + (pl && pl.id ? pl.id : pl));
        }
      };
    }
  };
})(window.Spin);
