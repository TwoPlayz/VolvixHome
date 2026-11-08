/* Volvix Home — shared view components: album cards, shelves, track lists, track menus. */
window.Spin = window.Spin || {};
(function (S) {
  'use strict';

  /* ---------- badges ---------- */
  S.formatBadge = function (album) {
    return '<span class="fmt">' + S.esc(S.data.FORMATS[album.format] || '') + '</span>';
  };

  S.explicitBadge = function () {
    return '<span class="explicit" title="Explicit" aria-label="Explicit">E</span>';
  };

  /* ---------- cards ---------- */
  S.albumCard = function (album, opts) {
    opts = opts || {};
    const status = S.lib.status(album.id);
    let meta;
    if (opts.meta) meta = opts.meta;
    else if (!status) meta = S.icon('lock', 12) + '<span>' + S.artistLink(album.artist, { inert: true }) + '</span>';
    else meta = '<span>' + S.artistLink(album.artist, { inert: true }) + '</span>';

    let tag = '';
    if (status === 'family') tag = '<span class="card-tag">Family</span>';
    else if (status === 'partial') tag = '<span class="card-tag">' + S.lib.ownedTrackCount(album) + '/' + album.trackIds.length + '</span>';
    else if (!status && album.isNew) tag = '<span class="card-tag card-tag-accent">New</span>';

    return '<a class="card' + (status ? '' : ' card-locked') + '" href="#/album/' + album.id + '">' +
      '<div class="card-art">' + S.cover(album) + tag +
        (status ? '<button class="card-play" data-play-album="' + album.id + '" aria-label="Play ' + S.esc(album.title) + '">' + S.icon('play', 18) + '</button>' : '') +
      '</div>' +
      '<div class="card-title">' + S.esc(album.title) + '</div>' +
      '<div class="card-meta">' + meta + '</div>' +
      (opts.showFormat ? '<div class="card-foot">' + S.formatBadge(album) + (album.preSpin ? '<span class="fmt fmt-muted">Pre-Volvix</span>' : '') + '</div>' : '') +
    '</a>';
  };

  S.playlistCard = function (pl) {
    return '<a class="card" href="#/playlist/' + pl.id + '">' +
      '<div class="card-art">' + S.playlistCover(pl) + '</div>' +
      '<div class="card-title">' + S.esc(pl.name) + '</div>' +
      '<div class="card-meta"><span>' +
        (pl.owner !== 'u_me' ? 'By ' + S.esc(S.data.person(pl.owner).name.split(' ')[0]) + ' · ' : (pl.collaborative ? 'Collaborative · ' : '')) +
        pl.tracks.length + ' songs</span></div>' +
    '</a>';
  };

  /** Horizontal shelf with a heading. */
  S.shelf = function (title, cardsHtml, opts) {
    opts = opts || {};
    return '<section class="section">' +
      '<div class="section-title"><div><h2>' + S.esc(title) + '</h2>' +
        (opts.sub ? '<p>' + S.esc(opts.sub) + '</p>' : '') + '</div>' +
        (opts.href ? '<a class="see-all" href="' + opts.href + '">Show all</a>' : '') +
      '</div>' +
      '<div class="shelf">' + cardsHtml + '</div>' +
    '</section>';
  };

  S.emptyState = function (icon, title, text, actionHtml) {
    return '<div class="empty">' +
      '<div class="empty-icon">' + S.icon(icon, 28) + '</div>' +
      '<h3>' + S.esc(title) + '</h3>' +
      '<p>' + S.esc(text) + '</p>' + (actionHtml || '') +
    '</div>';
  };

  /* ---------- track list ---------- */
  /**
   * S.trackList(trackIds, { showAlbum, showCover, numbered, context })
   * Rows carry data-track-id; clicking a row plays the list from that row.
   */
  S.trackList = function (trackIds, opts) {
    opts = opts || {};
    const listId = 'tl' + Math.random().toString(36).slice(2, 8);
    S.trackList.lists[listId] = { ids: trackIds.slice(), context: opts.context || null };
    const cls = 'tracks' + (opts.showAlbum ? ' with-album' : '') + (opts.addedBy ? ' with-added' : '') + (opts.playlistId ? ' editable' : '');

    const head = '<div class="track-head" aria-hidden="true">' +
      '<span class="c-num">#</span><span>Title</span>' +
      (opts.showAlbum ? '<span class="c-album">Album</span>' : '') +
      (opts.addedBy ? '<span class="c-added">Added by</span>' : '') +
      '<span class="c-dur">' + S.icon('history', 15) + '</span><span></span></div>';

    const dlSet = S.dl ? S.dl.trackSet() : {};
    const offline = !!(S.dl && S.dl.isOffline && S.dl.isOffline());
    const rows = trackIds.map(function (id, i) {
      const t = S.data.track(id);
      if (!t) return '';
      const album = S.data.album(t.albumId);
      const owned = S.lib.isTrackOwned(id);
      const downloaded = !!dlSet[id];
      const explicitOff = t.explicit && S.store.state.settings && S.store.state.settings.allowExplicit === false;
      const unavailable = owned && ((offline && !downloaded) || explicitOff);
      const current = S.player && S.player.isCurrent(id);
      const plAttr = opts.playlistId ? ' data-pl="' + opts.playlistId + '" data-pl-index="' + i + '"' : '';
      const adder = opts.addedBy ? S.data.person(opts.addedBy[i]) : null;
      return '<div class="track' + (owned ? '' : ' locked') + (unavailable ? ' unavailable' : '') + (current ? ' playing' : '') + '" role="button" tabindex="0" data-track-id="' + id + '" data-list="' + listId + '" data-index="' + i + '"' + plAttr + '>' +
        '<span class="c-num"><span class="num">' + (opts.numbered === false ? '' : (opts.useTrackNo ? t.no : i + 1)) + '</span>' +
          '<span class="eq" aria-hidden="true"><i></i><i></i><i></i></span>' +
          '<span class="row-play">' + S.icon('play', 14) + '</span></span>' +
        '<span class="c-title">' +
          (opts.showCover ? '<span class="row-cover">' + S.cover(album) + '</span>' : '') +
          (function () {
            // on an album page every song is by the album artist, so don't repeat it; then the
            // little lock / E / downloaded marks sit next to the title instead of on a line of their own
            const hide = opts.hideArtist === t.artist;
            const marks = (downloaded ? '<span class="dl-dot" title="Downloaded">' + S.icon('download', 10) + '</span>' : '') +
              (t.explicit ? S.explicitBadge() : '') + (owned ? '' : S.icon('lock', 11));
            return hide
              ? '<span class="t-text"><span class="t-name t-name-marks">' + S.esc(t.title) + (marks ? '<span class="t-marks">' + marks + '</span>' : '') + '</span></span>'
              : '<span class="t-text"><span class="t-name">' + S.esc(t.title) + '</span><span class="t-sub">' + marks + S.artistLink(t.artist) + '</span></span>';
          })() +
        '</span>' +
        (opts.showAlbum ? '<a class="c-album" href="#/album/' + album.id + '">' + S.esc(album.title) + '</a>' : '') +
        (opts.addedBy ? '<span class="c-added">' + (adder ? S.avatar(adder, 22) + '<span>' + S.esc(adder.id === 'u_me' ? 'You' : adder.name.split(' ')[0]) + '</span>' : '') + '</span>' : '') +
        '<span class="c-dur">' + S.fmtTime(t.duration) + '</span>' +
        '<span class="c-act">' + S.addButton(id, 'row-add') +
          (opts.playlistId ? '<span class="drag-handle" data-drag title="Drag to reorder" aria-hidden="true">' + S.icon('grip', 18) + '</span>' : '') +
          '<button class="row-more" data-track-menu="' + id + '" aria-label="More options for ' + S.esc(t.title) + '">' + S.icon('more', 18) + '</button></span>' +
      '</div>';
    }).join('');

    return '<div class="' + cls + '">' + head + rows + '</div>';
  };
  S.trackList.lists = {};

  /** Highlight whichever row is currently playing (called by the player). */
  S.markPlaying = function () {
    S.$$('[data-track-id]').forEach(function (row) {
      row.classList.toggle('playing', S.player.isCurrent(row.dataset.trackId));
      row.classList.toggle('paused', S.player.isCurrent(row.dataset.trackId) && !S.player.state.playing);
    });
    // big play button on album pages
    const btn = S.$('[data-album-play]');
    if (btn) {
      const cur = S.player.current();
      const playingThis = !!cur && cur.albumId === btn.dataset.albumPlay && S.player.state.playing;
      btn.innerHTML = S.icon(playingThis ? 'pause' : 'play', 24);
      btn.setAttribute('aria-label', playingThis ? 'Pause' : 'Play');
    }
  };

  function trackMenu(anchor, trackId) {
    const t = S.data.track(trackId);
    const owned = S.lib.isTrackOwned(trackId);
    const row = anchor.closest('.track');
    const plId = row && row.dataset.pl;
    const items = [];
    if (owned) {
      items.push({ label: 'Play next', icon: 'queue', onClick: function () { S.player.addNext([trackId]); } });
      items.push({ label: 'Add to queue', icon: 'plus', onClick: function () { S.player.addToQueue([trackId]); } });
    }
    items.push({ label: 'Add to playlist', icon: 'music', onClick: function () { S.pl.addToDialog([trackId]); } });
    if (plId) {
      const index = parseInt(row.dataset.plIndex, 10);
      const count = S.pl.get(plId).tracks.length;
      // keyboard / screen reader friendly alternative to dragging
      if (index > 0) items.push({ label: 'Move up', icon: 'arrowUp', onClick: function () { S.pl.move(plId, index, index - 1); refocusMenu(index - 1); } });
      if (index < count - 1) items.push({ label: 'Move down', icon: 'arrowDown', onClick: function () { S.pl.move(plId, index, index + 1); refocusMenu(index + 1); } });
      items.push({ label: 'Remove from this playlist', icon: 'x', onClick: function () {
        const entry = S.pl.removeAt(plId, index);
        undoToast('Removed "' + t.title + '"', function () { S.pl.insertAt(plId, index, entry); });
      } });
    }
    items.push({ label: 'Go to album', icon: 'disc', onClick: function () { S.router.go('/album/' + t.albumId); } });
    items.push({ label: 'Go to artist', icon: 'user', onClick: function () { location.hash = S.artistUrl(t.artist); } });
    S.popover(anchor, { items: items });
  }

  /** After moving a song, put focus back on its ⋯ button at the new position. */
  function refocusMenu(index) {
    const btn = S.$('.track[data-pl-index="' + index + '"] [data-track-menu]');
    if (btn) btn.focus();
  }

  /** Small "Undo" pill that appears for a few seconds after a destructive action. */
  function undoToast(message, undo) {
    const host = document.getElementById('toasts');
    const el = document.createElement('div');
    el.className = 'toast toast-action';
    el.innerHTML = '<span>' + S.esc(message) + '</span><button class="toast-btn">Undo</button>';
    el.querySelector('button').addEventListener('click', function () { undo(); el.remove(); S.toast('Restored', 'check'); });
    host.appendChild(el);
    setTimeout(function () { el.classList.add('leaving'); }, 4500);
    setTimeout(function () { el.remove(); }, 4800);
  }
  S.undoToast = undoToast;

  /** Toast with a button that takes you somewhere (e.g. the exact settings section). */
  S.linkToast = function (message, label, path) {
    const host = document.getElementById('toasts');
    const el = document.createElement('div');
    el.className = 'toast toast-action';
    el.innerHTML = '<span>' + S.esc(message) + '</span><button class="toast-btn">' + S.esc(label) + '</button>';
    el.querySelector('button').addEventListener('click', function () { el.remove(); S.router.go(path); });
    host.appendChild(el);
    setTimeout(function () { el.classList.add('leaving'); }, 5000);
    setTimeout(function () { el.remove(); }, 5300);
  };


  /* ---------- global delegated interactions ---------- */
  document.addEventListener('click', function (e) {
    const playAlbum = e.target.closest('[data-play-album]');
    if (playAlbum) {
      e.preventDefault(); e.stopPropagation();
      const album = S.data.album(playAlbum.dataset.playAlbum);
      S.player.play(album.trackIds, 0, { type: 'album', id: album.id, name: album.title });
      return;
    }
    const playPl = e.target.closest('[data-play-playlist]');
    if (playPl) {
      e.preventDefault(); e.stopPropagation();
      const pl = S.store.state.playlists.find(function (p) { return p.id === playPl.dataset.playPlaylist; });
      if (pl) S.player.play(pl.tracks.map(function (x) { return x.id; }), 0, { type: 'playlist', id: pl.id, name: pl.name });
      return;
    }
    const menuBtn = e.target.closest('[data-track-menu]');
    if (menuBtn) { e.stopPropagation(); trackMenu(menuBtn, menuBtn.dataset.trackMenu); return; }

    // one-tap "Add to playlist" on any song, and on whatever is playing
    const addBtn = e.target.closest('[data-add-pl]');
    if (addBtn) { e.preventDefault(); e.stopPropagation(); S.pl.addToDialog(addBtn.dataset.addPl.split(',')); return; }
    if (e.target.closest('[data-add-current]')) {
      const cur = S.player.current();
      if (cur) S.pl.addToDialog([cur.id]);
      return;
    }

    const row = e.target.closest('.track[data-track-id]');
    if (row && !e.target.closest('a, button, [data-drag], [data-go]')) activateRow(row);
  });

  document.addEventListener('keydown', function (e) {
    if (e.key !== 'Enter') return;
    const row = e.target.closest && e.target.closest('.track[data-track-id]');
    if (row && e.target === row) activateRow(row); // Enter on a link/button inside the row does its own thing
  });

  /** Small round "+" that opens Add to playlist for one or more songs. */
  S.addButton = function (ids, cls) {
    ids = [].concat(ids);
    return '<button class="add-pl' + (cls ? ' ' + cls : '') + '" data-add-pl="' + ids.join(',') + '" aria-label="Add to playlist" title="Add to playlist">' + S.icon('plus', 16) + '</button>';
  };

  function activateRow(row) {
    const id = row.dataset.trackId;
    if (!S.lib.isTrackOwned(id)) {
      S.toast('Locked: buy the physical release to unlock this song', 'lock');
      return;
    }
    if (S.lib.isPlayable && !S.lib.isPlayable(id)) {
      const t = S.data.track(id);
      if (t.explicit && S.store.state.settings.allowExplicit === false) S.linkToast('Explicit songs are off.', 'Playback settings', '/account/settings/playback');
      else S.toast('No internet connection, and this song isn\'t downloaded.', 'wifiOff');
      return;
    }
    if (S.player.isCurrent(id)) { S.player.toggle(); return; }
    const list = S.trackList.lists[row.dataset.list];
    if (list) S.player.play(list.ids, parseInt(row.dataset.index, 10), list.context);
  }
})(window.Spin);
