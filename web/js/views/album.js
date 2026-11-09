/* Volvix Home — Album page. */
window.Spin = window.Spin || {};
Spin.views = Spin.views || {};
(function (S) {
  'use strict';

  S.views.album = {
    render: function (root, params) {
      const album = S.data.album(params.id);
      if (!album) { S.views.notFound.render(root); return; }
      const q = S.lib.quality(album);
      const total = S.lib.totalDuration(album.trackIds);
      const ctx = { type: 'album', id: album.id, name: album.title };
      const discs = album.trackIds.reduce(function (n, id) { return Math.max(n, S.data.track(id).disc || 1); }, 1);
      const others = S.data.albumsBy(album.artist).filter(function (a) { return a.id !== album.id; });

      // multi-disc albums get a small "Disc 2" heading before each disc's songs
      let list;
      if (discs > 1) {
        list = '';
        for (let d = 1; d <= discs; d++) {
          const ids = album.trackIds.filter(function (id) { return (S.data.track(id).disc || 1) === d; });
          if (ids.length) list += '<div class="disc-head">' + S.icon('disc', 16) + 'Disc ' + d + '</div>' + S.trackList(ids, { useTrackNo: true, context: ctx, hideArtist: album.artist });
        }
      } else {
        list = S.trackList(album.trackIds, { useTrackNo: true, context: ctx, hideArtist: album.artist });
      }

      root.innerHTML =
        '<header class="album-head">' +
          '<div class="album-cover">' + S.cover(album) + '</div>' +
          '<div class="album-info">' +
            '<div class="eyebrow">Album</div>' +
            '<h1 class="album-title">' + S.esc(album.title) + '</h1>' +
            '<a class="album-artist" href="' + S.artistUrl(album.artist) + '">' + S.esc(album.artist) + '</a>' +
            '<div class="album-meta">' + [album.year, album.trackIds.length + ' song' + (album.trackIds.length === 1 ? '' : 's'), S.fmtLong(total)].filter(Boolean).join(' · ') + '</div>' +
            '<div class="album-badges">' +
              '<span class="fmt' + (q.hires ? ' fmt-hires' : '') + '" title="' + S.esc(q.detail) + '">' + S.esc(q.label) + '</span>' +
              (album.genre ? '<span class="fmt fmt-muted">' + S.esc(album.genre) + '</span>' : '') +
            '</div>' +
          '</div>' +
        '</header>' +
        '<div class="action-bar">' +
          '<button class="play-lg" data-album-play="' + album.id + '" aria-label="Play">' + S.icon('play', 24) + '</button>' +
          '<button class="icon-btn icon-btn-lg" data-album-shuffle aria-label="Shuffle" aria-pressed="false" title="Shuffle: play this in a random order">' + S.icon('shuffle', 22) + '</button>' +
          '<button class="icon-btn icon-btn-lg" data-album-add aria-label="Add album to playlist" title="Add to playlist">' + S.icon('plus', 22) + '</button>' +
          '<button class="icon-btn icon-btn-lg" data-album-more aria-label="More options">' + S.icon('more', 22) + '</button>' +
        '</div>' +
        list +
        '<div class="album-foot"><p>' + S.esc(q.detail) + (album.label ? ' · ℗ ' + S.esc(album.label) : '') + '</p></div>' +
        (others.length ? S.shelf('More by ' + album.artist, others.map(function (a) {
          return S.albumCard(a, { meta: '<span>' + (a.year || '') + '</span>' });
        }).join('')) : '');

      if (S.markPlaying) S.markPlaying();

      root.onclick = function (e) {
        if (e.target.closest('[data-album-play]')) {
          const cur = S.player.current();
          if (cur && cur.albumId === album.id) S.player.toggle();
          else S.player.play(album.trackIds, -1, ctx); // from the top, or a random song when shuffle is on
        }
        // shuffle is a switch: it never starts or interrupts what's playing
        else if (e.target.closest('[data-album-shuffle]')) S.player.toggleShuffle();
        else if (e.target.closest('[data-album-add]')) S.pl.addToDialog(album.trackIds);
        else if (e.target.closest('[data-album-more]')) {
          S.popover(e.target.closest('[data-album-more]'), { items: [
            { label: 'Add to queue', icon: 'queue', onClick: function () { S.player.addToQueue(album.trackIds); } },
            { label: 'Play next', icon: 'next', onClick: function () { S.player.addNext(album.trackIds); } },
            { label: 'Credits & info', icon: 'book', onClick: function () { S.albumInfoDialog(album.id); } },
            { label: 'Go to artist', icon: 'user', onClick: function () { location.hash = S.artistUrl(album.artist); } }
          ] });
        }
      };
    }
  };
})(window.Spin);
