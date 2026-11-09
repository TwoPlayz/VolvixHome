/* Volvix Home — Home: jump back in, what's new on the server, your playlists, artists. */
window.Spin = window.Spin || {};
Spin.views = Spin.views || {};
(function (S) {
  'use strict';

  function greeting() {
    const h = new Date().getHours();
    return h < 12 ? 'Good morning' : h < 18 ? 'Good afternoon' : 'Good evening';
  }

  function quickTile(href, art, title, playAttr) {
    return '<a class="quick" href="' + href + '">' +
      '<span class="quick-art">' + art + '</span>' +
      '<span class="quick-title">' + S.esc(title) + '</span>' +
      (playAttr ? '<button class="quick-play" ' + playAttr + ' aria-label="Play ' + S.esc(title) + '">' + S.icon('play', 16) + '</button>' : '') +
    '</a>';
  }

  function artistCard(name) {
    const first = S.data.albumsBy(name)[0];
    return '<a class="card artist-card" href="' + S.artistUrl(name) + '">' +
      '<div class="card-art round">' + S.cover(first) + '</div>' +
      '<div class="card-title">' + S.esc(name) + '</div><div class="card-meta"><span>Artist</span></div></a>';
  }

  S.views.home = {
    render: function (root) {
      const state = S.store.state;
      const albums = S.data.albums;
      if (!albums.length) {
        root.innerHTML = '<header class="page-head"><h1 class="page-title">' + greeting() + '</h1></header>' +
          S.emptyState('music', 'No music yet', S.data.me.admin
            ? 'Point Volvix Home at your music folder (VH_MUSIC_DIR), or let Lidarr fill it. New music shows up here on its own.'
            : 'Your library is empty for now. Music added to the server shows up here on its own.',
            S.data.me.admin ? '<a class="btn btn-secondary" href="#/account/settings">Library settings</a>' : '');
        return;
      }
      const recent = (state.recent || []).map(S.data.album).filter(Boolean);
      const added = albums.slice().sort(function (a, b) { return b.added - a.added; });
      const quick = (recent.length ? recent : added).slice(0, 4).map(function (a) {
        return quickTile('#/album/' + a.id, S.cover(a), a.title, 'data-play-album="' + a.id + '"');
      }).concat(S.pl.all().slice(0, 2).map(function (pl) {
        return quickTile('#/playlist/' + pl.id, S.playlistCover(pl), pl.name, 'data-play-playlist="' + pl.id + '"');
      }));
      // artists with the most albums first, then alphabetical
      const artists = {};
      albums.forEach(function (a) { artists[a.artist] = (artists[a.artist] || 0) + 1; });
      const topArtists = Object.keys(artists).sort(function (a, b) { return artists[b] - artists[a] || a.localeCompare(b); }).slice(0, 12);

      root.innerHTML =
        '<header class="page-head"><h1 class="page-title">' + greeting() + '</h1></header>' +
        S.rewindBanner() +
        '<div class="quick-grid">' + quick.join('') + '</div>' +
        (recent.length ? S.shelf('Recently played', recent.slice(0, 12).map(function (a) { return S.albumCard(a); }).join(''), { href: '#/library' }) : '') +
        S.shelf('Recently added', added.slice(0, 12).map(function (a) {
          return S.albumCard(a, { meta: '<span>' + S.artistLink(a.artist, { inert: true }) + '</span>' });
        }).join(''), { href: '#/library' }) +
        (S.pl.all().length ? S.shelf('Your playlists', S.pl.all().map(S.playlistCard).join(''), { href: '#/library/playlists' }) : '') +
        S.shelf('Artists', topArtists.map(artistCard).join(''), { href: '#/library/artists' });
    }
  };
})(window.Spin);
