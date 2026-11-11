/* Volvix Home — Playlist page: songs, reorder (drag a row from anywhere), add songs, invite people to edit. */
window.Spin = window.Spin || {};
Spin.views = Spin.views || {};
(function (S) {
  'use strict';

  const addUi = { q: '' };

  /** Owner first, then everyone who can edit. */
  function people(pl) {
    const ids = [pl.owner].concat(pl.collaborators || []).filter(function (id, i, a) { return a.indexOf(id) === i; });
    return '<span class="avatar-stack">' + ids.map(function (id) { return S.avatar(S.data.person(id), 26); }).join('') + '</span>';
  }

  function addSongs(pl) {
    const have = S.pl.trackIds(pl);
    let ids;
    let title;
    if (addUi.q) {
      const q = addUi.q.toLowerCase();
      ids = Object.values(S.data.tracks).filter(function (t) {
        return (t.title.toLowerCase().indexOf(q) !== -1 || t.artist.toLowerCase().indexOf(q) !== -1) && have.indexOf(t.id) === -1;
      }).slice(0, 8).map(function (t) { return t.id; });
      title = ids.length ? 'Results' : 'No songs match "' + addUi.q + '"';
    } else {
      // owned songs not already here, picked deterministically per playlist
      const pool = Object.values(S.data.tracks).filter(function (t) { return S.lib.isTrackOwned(t.id) && have.indexOf(t.id) === -1; });
      let seed = 0;
      for (const ch of pl.id) seed = (seed * 31 + ch.charCodeAt(0)) >>> 0;
      ids = [];
      for (let i = 0; i < pool.length && ids.length < 5; i++) {
        const t = pool[(seed + i * 7) % pool.length];
        if (ids.indexOf(t.id) === -1) ids.push(t.id);
      }
      title = 'Recommended from your library';
    }
    return '<div class="add-results-title">' + S.esc(title) + '</div>' +
      ids.map(function (id) {
        const t = S.data.track(id);
        const al = S.data.album(t.albumId);
        const owned = S.lib.isTrackOwned(id);
        return '<div class="add-row' + (owned ? '' : ' locked') + '">' +
          '<span class="row-cover">' + S.cover(al) + '</span>' +
          '<span class="t-text"><span class="t-name">' + S.esc(t.title) + '</span>' +
          '<span class="t-sub">' + (owned ? '' : S.icon('lock', 11)) + S.artistLink(t.artist) + '</span></span>' +
          '<span class="add-album">' + S.albumLink(al) + '</span>' +
          '<button class="btn btn-secondary btn-sm" data-add-track="' + id + '">Add</button>' +
        '</div>';
      }).join('');
  }

  S.views.playlist = {
    render: function (root, params) {
      // Liked Songs is always there (as in the Windows app), even before the first heart
      const pl = S.pl.get(params.id) || (params.id === 'pl_liked'
        ? { id: 'pl_liked', name: 'Liked Songs', description: 'Songs you\'ve liked.', owner: 'u_me', collaborative: false, collaborators: [], liked: true, tracks: [] }
        : null);
      if (!pl) {
        root.innerHTML = S.emptyState('music', 'Playlist not found', 'It may have been deleted.',
          '<a class="btn btn-secondary" href="#/library">Go to Library</a>');
        return;
      }
      const ids = S.pl.trackIds(pl);
      const locked = S.pl.lockedCount(pl);
      const total = S.lib.totalDuration(ids);
      const owner = S.data.person(pl.owner);
      const isOwner = pl.owner === 'u_me';
      const canEdit = S.pl.canEdit(pl);
      const following = !isOwner && S.pl.isFollowed(pl.id);

      root.innerHTML =
        '<header class="album-head">' +
          '<div class="album-cover">' + S.playlistCover(pl) + '</div>' +
          '<div class="album-info">' +
            '<div class="eyebrow">' + (pl.collaborative ? 'Collaborative playlist' : 'Playlist') + '</div>' +
            '<h1 class="album-title">' + (isOwner && !pl.liked ? '<button class="title-btn" data-edit title="Edit details">' + S.esc(pl.name) + '</button>' : S.esc(pl.name)) + '</h1>' +
            (pl.description ? '<p class="pl-desc">' + S.esc(pl.description) + '</p>' : '') +
            '<div class="album-meta pl-meta">' +
              (pl.collaborative ? people(pl) : S.avatar(owner, 24)) +
              '<span><strong>' + S.esc(owner.name) + '</strong>' +
              ' · ' + ids.length + ' song' + (ids.length === 1 ? '' : 's') + (ids.length ? ', ' + S.fmtLong(total) : '') +
              ((pl.invited || []).length && isOwner ? ' · ' + pl.invited.length + ' invited' : '') + '</span>' +
            '</div>' +
          '</div>' +
        '</header>' +

        '<div class="action-bar">' +
          '<button class="play-lg" data-pl-play="' + pl.id + '"' + (ids.length ? '' : ' disabled') + ' aria-label="Play">' + S.icon('play', 24) + '<span class="pill-label">Play</span></button>' +
          '<button class="icon-btn icon-btn-lg" data-pl-shuffle aria-label="Shuffle" title="Shuffle: play this in a random order"' + (ids.length ? '' : ' disabled') + '>' + S.icon('shuffle', 22) + '<span class="pill-label">Shuffle</span></button>' +
          (isOwner && !pl.liked ? '<button class="icon-btn icon-btn-lg" data-pl-invite aria-label="' + (pl.shared ? 'Who can edit' : 'Invite people to edit') + '" title="' + (pl.shared ? 'Who can edit' : 'Invite people to edit') + '">' + S.icon('users', 22) + '</button>' : '') +
          (!isOwner ? '<button class="btn btn-sm ' + (following ? 'btn-secondary' : 'btn-primary') + '" data-pl-follow>' + (following ? S.icon('check', 15) + 'Following' : S.icon('plus', 15) + 'Follow') + '</button>' : '') +
          (S.dl && S.dl.supported && ids.length ? S.dl.button('data-dl-playlist="' + pl.id + '"', ids) : '') +
          '<button class="icon-btn icon-btn-lg" data-pl-more aria-label="More options">' + S.icon('more', 22) + '</button>' +
        '</div>' +

        (ids.length
          ? S.trackList(ids, {
              showAlbum: true, showCover: true, playlistId: canEdit ? pl.id : null,
              addedBy: pl.collaborative ? pl.tracks.map(function (t) { return t.addedBy; }) : null,
              addedAt: pl.tracks.map(function (t) { return t.at || 0; }),
              context: { type: 'playlist', id: pl.id, name: pl.name }
            })
          : '') +

        (!canEdit ? '<p class="list-note readonly-note">' + S.icon('lock', 12) + ' This is ' + S.esc(owner.name.split(' ')[0]) +
          '\'s playlist. Follow it, or save a copy to make changes.</p>' : '') +

        '<section class="add-songs">' +
          '<h2>' + (ids.length ? 'Add songs' : 'Let\'s find something for your playlist') + '</h2>' +
          '<div class="search-box">' + S.icon('search', 20) +
            '<input class="search-input" type="search" data-add-search placeholder="Search for songs to add" value="' + S.esc(addUi.q) + '"></div>' +
          '<div data-add-results>' + addSongs(pl) + '</div>' +
        '</section>';
      if (!canEdit) S.$('.add-songs', root).remove();

      const ctx = { type: 'playlist', id: pl.id, name: pl.name };
      const results = S.$('[data-add-results]', root);
      const search = S.$('[data-add-search]', root);
      if (search) search.addEventListener('input', function () {
        addUi.q = search.value.trim();
        results.innerHTML = addSongs(pl);
      });

      root.onclick = function (e) {
        const t = e.target;
        if (t.closest('[data-pl-play]')) {
          const c = S.player.state.context;
          if (S.player.current() && c && c.id === pl.id) S.player.toggle();
          else S.player.play(ids, -1, ctx); // from the top, or a random song when shuffle is on
        }
        // shuffle is a switch for how songs from here get queued; it never starts or interrupts playback
        else if (t.closest('[data-pl-shuffle]')) S.player.toggleShuffle();
        else if (t.closest('[data-pl-follow]')) S.pl.toggleFollow(pl.id);
        else if (t.closest('[data-pl-invite]')) S.pl.inviteDialog(pl.id);
        else if (t.closest('[data-edit]')) { if (isOwner && !pl.liked) S.pl.editDialog(pl.id); }
        else if (t.closest('[data-add-track]')) {
          const id = t.closest('[data-add-track]').dataset.addTrack;
          const keepQ = addUi.q;
          S.pl.addTracks(pl.id, [id]);
          addUi.q = keepQ;
          const tr = S.data.track(id);
          S.toast('Added "' + tr.title + '"' + (S.lib.isTrackOwned(id) ? '' : ' (locked until you own it)'), 'check');
        }
        else if (t.closest('[data-pl-more]')) {
          const btn = t.closest('[data-pl-more]');
          const items = [];
          if (isOwner && !pl.liked) {
            items.push({ label: 'Edit details', icon: 'settings', onClick: function () { S.pl.editDialog(pl.id); } });
            items.push({ label: pl.shared ? 'Who can edit' : 'Make collaborative', icon: 'users', onClick: function () { S.pl.inviteDialog(pl.id); } });
            if (pl.shared) items.push({ label: 'Stop sharing', icon: 'x', onClick: function () { S.pl.toggleCollaborative(pl.id); } });
          }
          if (!isOwner && pl.shared) {
            items.push({ label: 'Leave playlist', icon: 'x', onClick: function () { S.pl.leave(pl.id); } });
          }
          if (!isOwner && !pl.shared) {
            items.push({ label: following ? 'Unfollow' : 'Follow', icon: following ? 'x' : 'plus', onClick: function () { S.pl.toggleFollow(pl.id); } });
            items.push({ label: 'Save a copy to my playlists', icon: 'music', onClick: function () { S.pl.duplicate(pl.id); } });
            items.push({ label: owner.name.split(' ')[0] + '\'s page', icon: 'user', onClick: function () { S.router.go('/household'); } });
          }
          if (ids.length) items.push({ label: 'Add to queue', icon: 'queue', onClick: function () { S.player.addToQueue(ids); } });
          if (ids.length && S.data.can('share')) items.push({ label: 'Share link…', icon: 'send', onClick: function () { S.shareDialog('playlist', pl.name, ids); } });
          if (isOwner && !pl.liked) {
            items.push({ sep: true });
            items.push({ label: 'Delete playlist', icon: 'x', danger: true, onClick: function () { S.pl.deleteDialog(pl.id); } });
          }
          S.popover(btn, { items: items });
        }
      };

      bindDrag(root, pl.id);
      S.markPlaying();
    }
  };

  /* ---------- drag to reorder: grab a row anywhere (mouse, touch and pen) ---------- */
  function bindDrag(root, plId) {
    root.onpointerdown = function (e) {
      S.dragRows(e, {
        row: '.track[data-pl-index]',
        scope: root,
        index: function (row) { return parseInt(row.dataset.plIndex, 10); },
        drop: function (from, to) { S.pl.move(plId, from, to); }
      });
    };
  }
})(window.Spin);