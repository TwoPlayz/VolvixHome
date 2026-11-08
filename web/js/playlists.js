/* Volvix — playlist model + dialogs (create, edit, delete, add-to, share, collaborators). */
window.Spin = window.Spin || {};
(function (S) {
  'use strict';

  function list() { return S.store.state.playlists; }
  /** Your playlists first, then other people's (read-only). */
  function get(id) {
    return list().find(function (p) { return p.id === id; }) ||
      S.data.friendPlaylists.find(function (p) { return p.id === id; }) || null;
  }
  function friendsForSharing() {
    return S.social ? S.social.friends() : [];
  }

  function commit(fn) {
    S.store.update(fn);
    S.app.refreshSidebar();
    S.app.refresh();
  }

  const LIKED_ID = 'pl_liked';
  function newId() { return 'pl_' + Date.now().toString(36) + Math.random().toString(36).slice(2, 5); }

  function lockedNote(ids) {
    const locked = ids.filter(function (id) { return !S.lib.isTrackOwned(id); }).length;
    if (!locked) return '';
    return locked === 1 ? ' · 1 song is locked until you own it' : ' · ' + locked + ' songs are locked until you own them';
  }

  S.pl = {
    all: list,
    get: get,
    /** Playlists you can add to: your own + collaborative ones you're on. */
    editable: function () {
      return list().filter(function (p) { return p.owner === 'u_me' || (p.collaborative && p.collaborators.indexOf('u_me') !== -1); });
    },
    canEdit: function (pl) {
      return pl.owner === 'u_me' || (pl.collaborative && pl.collaborators.indexOf('u_me') !== -1);
    },
    isFollowed: function (id) { return S.store.state.followed.indexOf(id) !== -1; },
    followed: function () { return S.store.state.followed.map(get).filter(Boolean); },
    toggleFollow: function (id) {
      const on = !S.pl.isFollowed(id);
      commit(function (s) {
        s.followed = on ? s.followed.concat(id) : s.followed.filter(function (x) { return x !== id; });
      });
      S.toast(on ? 'Following "' + get(id).name + '". It\'s in your sidebar.' : 'Unfollowed "' + get(id).name + '"', on ? 'check' : null);
    },
    /** Copy someone else's playlist into your own. */
    duplicate: function (id) {
      const src = get(id);
      const pl = S.pl.create({ name: src.name + ' (copy)', description: src.description }, S.pl.trackIds(src));
      S.toast('Saved a copy to your playlists', 'check');
      S.router.go('/playlist/' + pl.id);
    },
    trackIds: function (pl) { return pl.tracks.map(function (t) { return t.id; }); },
    lockedCount: function (pl) { return pl.tracks.filter(function (t) { return !S.lib.isTrackOwned(t.id); }).length; },
    shareUrl: function (pl) { return 'https://volvix.app/p/' + S.pl.shareId(pl); },
    /** Short id used in share links: pl_sunday → sunday. */
    shareId: function (pl) { return pl.id.replace(/^pl_/, ''); },
    /** Find a playlist from a share-link id. */
    fromShareId: function (sid) { return get(sid) || get('pl_' + sid) || null; },
    /** Demo: the public page someone sees when they open the link without signing in. */
    previewUrl: function (pl) { return location.pathname + '?preview=1&signedout=1#/p/' + S.pl.shareId(pl); },

    create: function (data, trackIds) {
      const pl = {
        id: newId(), name: data.name || 'New Playlist', description: data.description || '',
        owner: 'u_me', collaborative: !!data.collaborative, collaborators: [],
        tracks: (trackIds || []).map(function (id) { return { id: id, addedBy: 'u_me' }; })
      };
      // new playlists go under Liked Songs, which stays first
      commit(function (s) { s.playlists.splice(s.playlists.some(function (p) { return p.liked; }) ? 1 : 0, 0, pl); });
      return pl;
    },
    update: function (id, patch) {
      commit(function () { Object.assign(get(id), patch); });
    },
    remove: function (id) {
      if (id === LIKED_ID) return; // Liked Songs is built in
      commit(function (s) { s.playlists = s.playlists.filter(function (p) { return p.id !== id; }); });
    },

    /* ----- Liked Songs: one built-in playlist, filled by the heart button ----- */
    liked: function () { return list().find(function (p) { return p.id === LIKED_ID; }) || null; },
    isLiked: function (trackId) {
      const p = S.pl.liked();
      return !!p && p.tracks.some(function (t) { return t.id === trackId; });
    },
    /** Like / unlike a song. The newest like goes on top. */
    toggleLike: function (trackId) {
      if (!trackId || !S.data.track(trackId)) return;
      const on = !S.pl.isLiked(trackId);
      S.store.update(function (s) {
        let p = s.playlists.find(function (x) { return x.id === LIKED_ID; });
        if (!p) {
          p = { id: LIKED_ID, name: 'Liked Songs', description: 'Songs you\'ve liked.', owner: 'u_me', collaborative: false, collaborators: [], liked: true, tracks: [] };
          s.playlists.unshift(p);
        }
        if (on) p.tracks.unshift({ id: trackId, addedBy: 'u_me' });
        else p.tracks = p.tracks.filter(function (t) { return t.id !== trackId; });
      });
      S.app.refreshSidebar();
      S.pl.syncHearts();
      // if the Liked Songs page is open, its list changes too
      if (/^\/playlist\/pl_liked/.test(S.router.current())) S.app.refresh();
      S.toast(on ? 'Added to Liked Songs' : 'Removed from Liked Songs', on ? 'heartFill' : 'heart');
    },
    /** Make every heart button on screen match what's liked. */
    syncHearts: function () {
      S.$$('[data-like]').forEach(function (b) {
        const id = b.dataset.like;
        const liked = !!id && S.pl.isLiked(id);
        b.classList.toggle('on', liked);
        b.setAttribute('aria-pressed', liked ? 'true' : 'false');
        const label = liked ? 'Remove from Liked Songs' : 'Add to Liked Songs';
        b.setAttribute('aria-label', label); b.title = label;
        b.innerHTML = S.icon(liked ? 'heartFill' : 'heart', parseInt(b.dataset.size || '18', 10));
      });
    },
    /** Adds tracks, skipping duplicates. Returns number actually added. */
    addTracks: function (id, ids) {
      const pl = get(id);
      const have = S.pl.trackIds(pl);
      const fresh = ids.filter(function (t, i) { return have.indexOf(t) === -1 && ids.indexOf(t) === i; });
      if (fresh.length) {
        commit(function () {
          fresh.forEach(function (t) { pl.tracks.push({ id: t, addedBy: 'u_me' }); });
        });
      }
      return fresh.length;
    },
    removeAt: function (id, index) {
      const pl = get(id);
      const t = pl.tracks[index];
      commit(function () { pl.tracks.splice(index, 1); });
      return t;
    },
    insertAt: function (id, index, entry) {
      commit(function () { get(id).tracks.splice(index, 0, entry); });
    },
    move: function (id, from, to) {
      if (from === to) return;
      commit(function () {
        const tracks = get(id).tracks;
        const item = tracks.splice(from, 1)[0];
        tracks.splice(to, 0, item);
      });
    }
  };

  /* =========================================================
     Dialogs
     ========================================================= */

  function field(id, label, value, opts) {
    opts = opts || {};
    const input = opts.textarea
      ? '<textarea class="input textarea" id="' + id + '" rows="3" maxlength="200" placeholder="' + (opts.placeholder || '') + '">' + S.esc(value || '') + '</textarea>'
      : '<input class="input" id="' + id + '" maxlength="60" value="' + S.esc(value || '') + '" placeholder="' + (opts.placeholder || '') + '">';
    return '<div class="field"><label for="' + id + '">' + label + '</label>' + input + '</div>';
  }

  function switchRow(id, title, text, on) {
    return '<label class="switch-row" for="' + id + '"><span class="switch-text"><strong>' + title + '</strong><span>' + text + '</span></span>' +
      '<span class="switch"><input type="checkbox" id="' + id + '"' + (on ? ' checked' : '') + '><span class="switch-ui"></span></span></label>';
  }

  /** Create a playlist, optionally seeded with tracks. */
  S.pl.createDialog = function (trackIds) {
    const m = S.modal({
      title: 'New playlist',
      body: '<div class="stack form-modal">' +
        field('pl-name', 'Name', '', { placeholder: 'My playlist' }) +
        field('pl-desc', 'Description', '', { textarea: true, placeholder: 'Add an optional description' }) +
        switchRow('pl-collab', 'Collaborative', 'Friends you invite can add and remove songs.', false) +
        '</div>',
      actions: [
        { label: 'Cancel', kind: 'secondary' },
        { label: 'Create', kind: 'primary', onClick: submit }
      ]
    });
    const name = S.$('#pl-name', m.el);
    name.focus();
    name.addEventListener('keydown', function (e) { if (e.key === 'Enter') submit(m.close); });

    function submit(close) {
      const pl = S.pl.create({
        name: name.value.trim() || 'My Playlist #' + (list().length + 1),
        description: S.$('#pl-desc', m.el).value.trim(),
        collaborative: S.$('#pl-collab', m.el).checked
      }, trackIds);
      close();
      S.toast('Created "' + pl.name + '"' + (trackIds && trackIds.length ? lockedNote(trackIds) : ''), 'check');
      if (!trackIds) S.router.go('/playlist/' + pl.id);
      if (pl.collaborative && !trackIds) setTimeout(function () { S.pl.inviteDialog(pl.id); }, 150);
    }
  };

  S.pl.editDialog = function (id) {
    const pl = get(id);
    const m = S.modal({
      title: 'Edit details',
      body: '<div class="stack form-modal">' +
        field('pl-name', 'Name', pl.name) +
        field('pl-desc', 'Description', pl.description, { textarea: true, placeholder: 'Add an optional description' }) +
        '</div>',
      actions: [
        { label: 'Cancel', kind: 'secondary' },
        { label: 'Save', kind: 'primary', onClick: save }
      ]
    });
    const name = S.$('#pl-name', m.el);
    name.focus(); name.select();
    name.addEventListener('keydown', function (e) { if (e.key === 'Enter') save(m.close); });
    function save(close) {
      S.pl.update(id, { name: name.value.trim() || pl.name, description: S.$('#pl-desc', m.el).value.trim() });
      close();
      S.toast('Playlist updated', 'check');
    }
  };

  S.pl.deleteDialog = function (id) {
    const pl = get(id);
    S.modal({
      title: 'Delete playlist?',
      body: '<p>This will delete <strong>' + S.esc(pl.name) + '</strong>' +
        (pl.collaborative && pl.collaborators.length ? ' for you and ' + pl.collaborators.length + ' collaborator' + (pl.collaborators.length > 1 ? 's' : '') : '') +
        '. Your music stays in your library.</p>',
      actions: [
        { label: 'Cancel', kind: 'secondary' },
        { label: 'Delete', kind: 'danger', onClick: function (close) {
          close();
          const onPage = S.router.current() === '/playlist/' + id;
          S.pl.remove(id);
          S.toast('Deleted "' + pl.name + '"');
          if (onPage) S.router.go('/library');
        } }
      ]
    });
  };

  /** Pick a playlist to add tracks to. */
  S.pl.addToDialog = function (trackIds) {
    const pls = S.pl.editable();
    const rows = pls.map(function (p) {
      const have = S.pl.trackIds(p);
      const all = trackIds.every(function (t) { return have.indexOf(t) !== -1; });
      return '<button class="pick-row" data-pick="' + p.id + '"' + (all ? ' disabled' : '') + '>' +
        '<span class="pick-art">' + S.playlistCover(p) + '</span>' +
        '<span class="pick-text"><span class="t-name">' + S.esc(p.name) + '</span>' +
        '<span class="t-sub">' + (p.collaborative ? 'Collaborative · ' : '') + p.tracks.length + ' songs</span></span>' +
        (all ? '<span class="pick-state">' + S.icon('check', 16) + 'Added</span>' : '') +
      '</button>';
    }).join('');

    const m = S.modal({
      title: trackIds.length === 1 ? 'Add to playlist' : 'Add ' + trackIds.length + ' songs to playlist',
      body: '<div class="pick-list">' +
        '<button class="pick-row" data-pick-new><span class="pick-art pick-new">' + S.icon('plus', 22) + '</span>' +
        '<span class="pick-text"><span class="t-name">New playlist</span></span></button>' + rows + '</div>',
      actions: [{ label: 'Cancel', kind: 'secondary' }]
    });
    m.el.querySelector('.pick-list').addEventListener('click', function (e) {
      if (e.target.closest('[data-pick-new]')) { m.close(); S.pl.createDialog(trackIds); return; }
      const row = e.target.closest('[data-pick]');
      if (!row || row.disabled) return;
      const pl = get(row.dataset.pick);
      const added = S.pl.addTracks(pl.id, trackIds);
      m.close();
      if (!added) S.toast('Already in "' + pl.name + '"');
      else S.toast('Added to "' + pl.name + '"' + lockedNote(trackIds), 'check');
    });
  };

  S.pl.shareDialog = function (id) {
    const pl = get(id);
    const url = S.pl.shareUrl(pl);
    const locked = S.pl.lockedCount(pl);
    const friends = friendsForSharing().filter(function (p) { return p.id !== pl.owner; });
    const m = S.modal({
      title: 'Share "' + pl.name + '"',
      body:
        '<div class="share-link"><span class="share-url">' + S.esc(url) + '</span>' +
          '<button class="btn btn-primary btn-sm" data-copy>' + S.icon('external', 14) + 'Copy link</button></div>' +
        '<div class="share-note">' + S.icon('lock', 15) +
          '<span>Listeners only hear the songs they own. Everything else shows as locked and is skipped' +
          (locked ? ' (you have ' + locked + ' locked here yourself)' : '') + '.</span></div>' +
        '<div class="share-preview">' + S.icon('eye', 15) + '<span>People without Volvix see a preview page with every song and a sign-up button.</span>' +
          '<a class="btn btn-ghost btn-sm" href="' + S.esc(S.pl.previewUrl(pl)) + '" target="_blank" rel="noopener">Open preview</a></div>' +
        '<div class="q-label share-label">Send to a friend</div>' +
        '<div class="share-friends">' + friends.map(function (f) {
          return '<button class="share-friend" data-send="' + f.id + '">' + S.avatar(f, 44) + '<span>' + S.esc(f.name.split(' ')[0]) + '</span></button>';
        }).join('') + '</div>',
      actions: [{ label: 'Done', kind: 'secondary' }]
    });
    m.el.addEventListener('click', function (e) {
      if (e.target.closest('[data-copy]')) {
        try { if (navigator.clipboard) navigator.clipboard.writeText(url).catch(function () {}); } catch (err) { /* ignore */ }
        S.toast('Link copied', 'check');
      }
      const send = e.target.closest('[data-send]');
      if (send && !send.classList.contains('sent')) {
        send.classList.add('sent');
        send.insertAdjacentHTML('beforeend', '<span class="sent-badge">' + S.icon('check', 12) + '</span>');
        S.toast('Sent to ' + S.data.person(send.dataset.send).name, 'check');
      }
    });
  };

  S.pl.inviteDialog = function (id) {
    const pl = get(id);
    const friends = friendsForSharing();
    const m = S.modal({
      title: 'Collaborators',
      body: '<p>Collaborators can add, remove and reorder songs. Each person only hears the songs they own.</p>' +
        '<div class="pick-list">' + friends.map(function (f) {
          const on = pl.collaborators.indexOf(f.id) !== -1;
          return '<label class="pick-row pick-check">' + S.avatar(f, 40) +
            '<span class="pick-text"><span class="t-name">' + S.esc(f.name) + '</span><span class="t-sub">@' + S.esc(f.username) +
            (f.relation === 'family' ? ' · Family' : '') + '</span></span>' +
            '<input type="checkbox" value="' + f.id + '"' + (on ? ' checked' : '') + '></label>';
        }).join('') + '</div>',
      actions: [
        { label: 'Cancel', kind: 'secondary' },
        { label: 'Save', kind: 'primary', onClick: function (close) {
          const chosen = S.$$('input[type=checkbox]:checked', m.el).map(function (c) { return c.value; });
          S.pl.update(id, { collaborators: chosen, collaborative: true });
          close();
          S.toast(chosen.length ? 'Invited ' + chosen.length + ' collaborator' + (chosen.length > 1 ? 's' : '') : 'Collaborative: invite friends anytime', 'users');
        } }
      ]
    });
  };

  S.pl.toggleCollaborative = function (id) {
    const pl = get(id);
    if (pl.collaborative) {
      S.pl.update(id, { collaborative: false, collaborators: [] });
      S.toast('Playlist is no longer collaborative');
    } else {
      S.pl.inviteDialog(id);
    }
  };
})(window.Spin);
