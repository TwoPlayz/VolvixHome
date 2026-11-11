/* Volvix Home — playlists.
 *   - your own playlists live in your app data (state.playlists)
 *   - playlists you share with other people live on the server (S.shared) so everyone edits the same copy
 *   - other people's public playlists come from the Household page data (read-only)
 * Everything here works on all three the same way; shared ones send each change to the server. */
window.Spin = window.Spin || {};
(function (S) {
  'use strict';

  const LIKED_ID = 'pl_liked';

  function local() { return S.store.state.playlists; }
  /** Everything of yours: your own playlists, plus the ones you share. */
  function list() { return local().concat(S.shared.all()); }
  function get(id) {
    return list().find(function (p) { return p.id === id; }) || S.household.playlist(id) || null;
  }

  function commit(fn) {
    S.store.update(fn);
    S.app.refreshSidebar();
    S.app.refresh();
    if (S.pl && S.pl.syncAdds) S.pl.syncAdds();
  }
  /** After a change to a shared playlist: redraw now, and re-sync if the server disagreed. */
  function redraw() { S.app.refreshSidebar(); S.app.refresh(); if (S.pl && S.pl.syncAdds) S.pl.syncAdds(); }
  function sharedFailed(e) {
    S.toast(e && e.message ? e.message : 'That didn\'t go through', 'x');
    S.shared.refresh().then(redraw).catch(function () {});
  }
  function sharedOp(id, body) {
    return S.shared.op(id, body).then(function (r) { redraw(); return r; }).catch(function (e) { sharedFailed(e); throw e; });
  }

  function newId() { return 'pl_' + Date.now().toString(36) + Math.random().toString(36).slice(2, 5); }

  function lockedNote() { return ''; } // everything here is yours: nothing is locked

  S.pl = {
    all: list,
    get: get,
    /** Playlists you can add to: your own + shared ones you're on. */
    editable: function () { return list().filter(S.pl.canEdit); },
    canEdit: function (pl) {
      return pl.owner === 'u_me' || (pl.collaborative && (pl.collaborators || []).indexOf('u_me') !== -1);
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
    lockedCount: function () { return 0; },

    create: function (data, trackIds) {
      const pl = {
        id: newId(), name: data.name || 'New Playlist', description: data.description || '', public: !!data.public,
        owner: 'u_me', collaborative: false, collaborators: [],
        tracks: (trackIds || []).map(function (id) { return { id: id, addedBy: 'u_me', at: Math.floor(Date.now() / 1000) }; })
      };
      // new playlists go under Liked Songs, which stays first
      commit(function (s) { s.playlists.splice(s.playlists.some(function (p) { return p.liked; }) ? 1 : 0, 0, pl); });
      return pl;
    },
    update: function (id, patch) {
      const pl = get(id);
      if (pl.shared) {
        const body = { op: 'update' };
        ['name', 'description', 'public'].forEach(function (k) { if (k in patch) body[k] = patch[k]; });
        return sharedOp(id, body);
      }
      commit(function () { Object.assign(pl, patch); });
    },
    /** Choose a picture for one of your own playlists: the middle square of the file, 512 px, kept on the server. */
    chooseCover: function (id) {
      const inp = document.createElement('input'); inp.type = 'file'; inp.accept = 'image/*';
      inp.onchange = function () {
        const f = inp.files && inp.files[0]; if (!f) return;
        const img = new Image(), url = URL.createObjectURL(f);
        img.onerror = function () { URL.revokeObjectURL(url); S.toast('Couldn\'t read that picture.', 'x'); };
        img.onload = function () {
          const N = 512, c = document.createElement('canvas'); c.width = c.height = N;
          const s = Math.min(img.width, img.height);
          c.getContext('2d').drawImage(img, (img.width - s) / 2, (img.height - s) / 2, s, s, 0, 0, N, N);
          URL.revokeObjectURL(url);
          c.toBlob(function (blob) {
            fetch('/api/playlists/' + encodeURIComponent(id) + '/cover', { method: 'PUT', credentials: 'same-origin', headers: { 'Content-Type': 'image/jpeg' }, body: blob })
              .then(function (r) { return r.json().then(function (j) { if (!r.ok) throw new Error(j.error || 'Couldn\'t save the picture'); return j; }); })
              .then(function (r) { S.pl.update(id, { cover: r.cover }); S.toast('Picture updated', 'check'); })
              .catch(function (x) { S.toast(x.message, 'x'); });
          }, 'image/jpeg', 0.9);
        };
        img.src = url;
      };
      inp.click();
    },
    clearCover: function (id) {
      fetch('/api/playlists/' + encodeURIComponent(id) + '/cover', { method: 'DELETE', credentials: 'same-origin' })
        .then(function () { S.pl.update(id, { cover: 0 }); S.toast('Picture removed', 'check'); });
    },
    remove: function (id) {
      if (id === LIKED_ID) return; // Liked Songs is built in
      const pl = get(id);
      if (pl && pl.shared) { sharedOp(id, { op: 'delete' }); return; }
      commit(function (s) { s.playlists = s.playlists.filter(function (p) { return p.id !== id; }); });
    },
    /** Adds tracks, skipping duplicates. Returns number actually added. */
    addTracks: function (id, ids) {
      const pl = get(id);
      const have = S.pl.trackIds(pl);
      const fresh = ids.filter(function (t, i) { return have.indexOf(t) === -1 && ids.indexOf(t) === i; });
      if (!fresh.length) return 0;
      if (pl.shared) { sharedOp(id, { op: 'add', ids: fresh }); return fresh.length; }
      commit(function () { fresh.forEach(function (t) { pl.tracks.push({ id: t, addedBy: 'u_me', at: Math.floor(Date.now() / 1000) }); }); });
      return fresh.length;
    },
    removeAt: function (id, index) {
      const pl = get(id);
      const t = pl.tracks[index];
      if (pl.shared) {
        pl.tracks.splice(index, 1); redraw();                 // show it gone right away
        sharedOp(id, { op: 'remove', index: index, trackId: t.id }).catch(function () {});
        return t;
      }
      commit(function () { pl.tracks.splice(index, 1); });
      return t;
    },
    insertAt: function (id, index, entry) {
      const pl = get(id);
      if (pl.shared) {
        pl.tracks.splice(index, 0, entry); redraw();
        sharedOp(id, { op: 'insert', index: index, entry: { id: entry.id, addedBy: entry.addedBy === 'u_me' ? S.data.me.serverId : entry.addedBy, at: entry.at || undefined } }).catch(function () {});
        return;
      }
      commit(function () { pl.tracks.splice(index, 0, entry); });
    },
    move: function (id, from, to) {
      if (from === to) return;
      const pl = get(id);
      const apply = function () { const item = pl.tracks.splice(from, 1)[0]; pl.tracks.splice(to, 0, item); };
      if (pl.shared) { apply(); redraw(); sharedOp(id, { op: 'move', from: from, to: to }).catch(function () {}); return; }
      commit(apply);
    },

    /* ----- sharing with the household ----- */
    /** Make one of your playlists shared (you stay the owner). Resolves with the shared playlist. */
    makeShared: function (id) {
      const pl = get(id);
      if (pl.shared) return Promise.resolve(pl);
      return S.shared.create(pl).then(function (sh) {
        commit(function (s) { s.playlists = s.playlists.filter(function (p) { return p.id !== id; }); });
        return sh;
      });
    },
    /** Hand a shared playlist back to just you. Everyone else loses access. */
    unshare: function (id) {
      return S.shared.op(id, { op: 'unshare' }).then(function (r) {
        const sh = S.shared.fromServer(r.playlist);
        commit(function (s) {
          s.playlists.push({ id: sh.id, name: sh.name, description: sh.description, public: sh.public, owner: 'u_me', collaborative: false, collaborators: [], tracks: sh.tracks });
        });
      }).catch(function (e) { sharedFailed(e); });
    },
    leave: function (id) {
      return S.shared.op(id, { op: 'leave' }).then(function () { redraw(); S.toast('You left the playlist'); if (S.router.current() === '/playlist/' + id) S.router.go('/library'); })
        .catch(function (e) { sharedFailed(e); });
    },

    /* ----- Liked Songs: one built-in playlist, filled by the heart button ----- */
    liked: function () { return local().find(function (p) { return p.id === LIKED_ID; }) || null; },
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
        if (on) p.tracks.unshift({ id: trackId, addedBy: 'u_me', at: Math.floor(Date.now() / 1000) });
        else p.tracks = p.tracks.filter(function (t) { return t.id !== trackId; });
      });
      S.app.refreshSidebar();
      S.pl.syncHearts();
      S.pl.syncAdds();
      // if the Liked Songs page is open, its list changes too
      if (/^\/playlist\/pl_liked/.test(S.router.current())) S.app.refresh();
      S.toast(on ? 'Added to Liked Songs' : 'Removed from Liked Songs', on ? 'heartFill' : 'heart');
    },
    /** Make the player's + button (and the phone one) show a check when the playing song is in a playlist. */
    syncAdds: function () {
      const t = S.player && S.player.current && S.player.current();
      const inOne = !!t && S.pl.editable().some(function (p) { return S.pl.trackIds(p).indexOf(t.id) !== -1; });
      const title = inOne ? 'In your playlists · add to another' : 'Add to playlist';
      S.$$('[data-p="add"], .np-add[data-add-pl]').forEach(function (b) {
        const small = b.classList.contains('np-add') ? 22 : 18;
        b.innerHTML = S.icon(inOne ? 'check' : 'plus', small);
        b.classList.toggle('on', inOne);
        b.title = title; b.setAttribute('aria-label', title);
      });
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
        switchRow('pl-collab', 'Collaborative', 'People you invite can add, remove and reorder songs.', false) +
        switchRow('pl-public', 'Visible to the household', 'Everyone here can see it on the Household page.', false) +
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
      const collab = S.$('#pl-collab', m.el).checked;
      const pl = S.pl.create({
        name: name.value.trim() || 'My Playlist #' + (list().length + 1),
        description: S.$('#pl-desc', m.el).value.trim(),
        public: S.$('#pl-public', m.el).checked
      }, trackIds);
      close();
      S.toast('Created "' + pl.name + '"', 'check');
      if (!collab) { if (!trackIds) S.router.go('/playlist/' + pl.id); return; }
      // a collaborative playlist lives on the server so everyone edits the same copy
      S.pl.makeShared(pl.id).then(function (sh) {
        if (!trackIds) S.router.go('/playlist/' + sh.id);
        setTimeout(function () { S.pl.inviteDialog(sh.id); }, 150);
      }).catch(function (e) { S.toast(e.message, 'x'); });
    }
  };

  S.pl.editDialog = function (id) {
    const pl = get(id);
    const m = S.modal({
      title: 'Edit details',
      body: '<div class="stack form-modal">' +
        field('pl-name', 'Name', pl.name) +
        field('pl-desc', 'Description', pl.description, { textarea: true, placeholder: 'Add an optional description' }) +
        switchRow('pl-public', 'Visible to the household', 'Everyone here can see it on the Household page.', !!pl.public) +
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
      S.pl.update(id, { name: name.value.trim() || pl.name, description: S.$('#pl-desc', m.el).value.trim(), public: S.$('#pl-public', m.el).checked });
      close();
      S.toast('Playlist updated', 'check');
    }
  };

  S.pl.deleteDialog = function (id) {
    const pl = get(id);
    const others = (pl.collaborators || []).length;
    S.modal({
      title: 'Delete playlist?',
      body: '<p>This will delete <strong>' + S.esc(pl.name) + '</strong>' +
        (pl.shared && others ? ' for you and ' + others + ' collaborator' + (others > 1 ? 's' : '') : '') +
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
        '<span class="t-sub">' + (p.collaborative ? 'Collaborative · ' : '') + p.tracks.length + (p.tracks.length === 1 ? ' song' : ' songs') + '</span></span>' +
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

  /** Choose who can edit a playlist. People you pick get an invite and join when they accept. */
  S.pl.inviteDialog = function (id) {
    let pl = get(id);
    if (!pl.shared) {
      // it only becomes shared (and moves to the server) once you invite someone
      S.pl.makeShared(id).then(function (sh) { S.pl.inviteDialog(sh.id); }).catch(function (e) { S.toast(e.message, 'x'); });
      return;
    }
    const people = S.data.others();
    const collab = pl.collaborators || [], invited = pl.invited || [];
    const state = function (p) { return collab.indexOf(p.id) !== -1 ? 'collab' : invited.indexOf(p.id) !== -1 ? 'invited' : 'none'; };
    const m = S.modal({
      title: 'Who can edit "' + pl.name + '"?',
      body: people.length
        ? '<p class="modal-text">People you pick get an invite. Once they accept they can add, remove and reorder songs.</p>' +
          '<div class="pick-list">' + people.map(function (p) {
            const st = state(p);
            return '<label class="pick-row pick-check">' + S.avatar(p, 40) +
              '<span class="pick-text"><span class="t-name">' + S.esc(p.name) + '</span>' +
              '<span class="t-sub">' + (st === 'collab' ? 'Can edit' : st === 'invited' ? 'Invited, waiting to accept' : 'Not invited') + '</span></span>' +
              '<input type="checkbox" value="' + p.id + '"' + (st !== 'none' ? ' checked' : '') + '></label>';
          }).join('') + '</div>'
        : '<p class="modal-text">No one else is in your household yet. An admin can add people under Account → People.</p>',
      actions: [
        { label: 'Cancel', kind: 'secondary' },
        people.length ? { label: 'Save', kind: 'primary', onClick: function (close) {
          const chosen = S.$$('input[type=checkbox]:checked', m.el).map(function (c) { return c.value; });
          const current = collab.concat(invited);
          const add = chosen.filter(function (x) { return current.indexOf(x) === -1; });
          const drop = current.filter(function (x) { return chosen.indexOf(x) === -1; });
          close();
          const chain = add.length ? S.shared.op(id, { op: 'invite', userIds: add }) : Promise.resolve();
          drop.reduce(function (p, u) { return p.then(function () { return S.shared.op(id, { op: 'uninvite', userId: u }); }); }, chain)
            .then(function () {
              redraw();
              if (add.length) S.toast('Invited ' + add.map(function (u) { return S.data.person(u).name.split(' ')[0]; }).join(', '), 'users');
              else if (drop.length) S.toast('Updated who can edit');
            }).catch(sharedFailed);
        } } : { label: 'Done', kind: 'primary' }
      ]
    });
  };

  S.pl.toggleCollaborative = function (id) {
    const pl = get(id);
    if (!pl.shared) { S.pl.inviteDialog(id); return; }
    S.modal({
      title: 'Stop sharing this playlist?',
      body: '<p class="modal-text">It becomes just yours again. ' + ((pl.collaborators || []).length ? 'Everyone you invited loses access.' : '') + '</p>',
      actions: [
        { label: 'Cancel', kind: 'secondary' },
        { label: 'Stop sharing', kind: 'danger', onClick: function (close) { close(); S.pl.unshare(id).then(function () { S.toast('Playlist is no longer collaborative'); }); } }
      ]
    });
  };
})(window.Spin);
