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
    const cls = 'tracks' + (opts.showAlbum ? ' with-album' : '') + ((opts.addedBy || opts.addedAt) ? ' with-added' : '') + (opts.playlistId ? ' editable' : '');

    const head = '<div class="track-head" aria-hidden="true">' +
      (opts.playlistId ? '<span class="c-drag"></span>' : '') +
      '<span class="c-num">#</span><span>Title</span>' +
      (opts.showAlbum ? '<span class="c-album">Album</span>' : '') +
      ((opts.addedBy || opts.addedAt) ? '<span class="c-added">' + (opts.addedBy ? 'Added by' : 'Date added') + '</span>' : '') +
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
        (opts.playlistId ? '<span class="drag-sign" title="Drag to reorder: grab the row anywhere" aria-hidden="true">' + S.icon('grip', 18) + '</span>' : '') +
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
        ((opts.addedBy || opts.addedAt) ? '<span class="c-added">' + (adder ? S.avatar(adder, 22) + '<span>' + S.esc(adder.id === 'u_me' ? 'You' : adder.name.split(' ')[0]) + '</span>' : '') +
          (opts.addedAt && opts.addedAt[i] ? '<span class="c-when">' + S.fmtDay(opts.addedAt[i]) + '</span>' : '') + '</span>' : '') +
        '<span class="c-dur">' + S.fmtTime(t.duration) + '</span>' +
        '<span class="c-act">' + S.likeButton(id, 'row-like', 16) + S.addButton(id, 'row-add') +
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
      // hovering the song that's playing offers Pause, not Play
      const rp = row.querySelector('.row-play');
      if (rp) { const want = S.player.isCurrent(row.dataset.trackId) && S.player.state.playing ? 'pause' : 'play'; if (rp.dataset.icon !== want) { rp.dataset.icon = want; rp.innerHTML = S.icon(want, 14); } }
    });
    // big play button on album pages
    const btn = S.$('[data-album-play]');
    if (btn) {
      const cur = S.player.current();
      const playingThis = !!cur && cur.albumId === btn.dataset.albumPlay && S.player.state.playing;
      btn.innerHTML = S.icon(playingThis ? 'pause' : 'play', 24);
      btn.setAttribute('aria-label', playingThis ? 'Pause' : 'Play');
    }
    const st = S.player.state, ctx = st.context || {}, live = !!S.player.current() && st.playing;
    // playlist page: the big play button follows what's playing from this playlist
    const pb = S.$('[data-pl-play]');
    if (pb) {
      const on = live && ctx.id === pb.dataset.plPlay;
      pb.innerHTML = S.icon(on ? 'pause' : 'play', 24);
      pb.setAttribute('aria-label', on ? 'Pause' : 'Play');
    }
    // artist page
    const ab = S.$('[data-artist-play]');
    if (ab) {
      const on = live && ctx.type === 'artist' && ctx.name === ab.dataset.artistPlay;
      ab.innerHTML = S.icon(on ? 'pause' : 'play', 16) + (on ? 'Pause' : 'Play');
    }
    // shuffle is one switch for the whole player: every shuffle button shows whether it's on
    S.$$('[data-album-shuffle], [data-pl-shuffle], [data-artist-shuffle]').forEach(function (b) {
      b.classList.toggle('on', !!st.shuffle);
      b.setAttribute('aria-pressed', st.shuffle ? 'true' : 'false');
    });
  };

  /**
   * Drag-to-reorder from anywhere on a row. Call it from a pointerdown handler:
   *   S.dragRows(event, { row: '.track', scope: element, index: row => number, drop: (from, to) => … })
   * (optional: rows() for the rows that can take a drop, scroller for the element to auto-scroll.)
   * Mouse and pen: moving a few pixels starts the drag. Touch: press and hold briefly, so scrolling still works.
   * Buttons and links inside the row keep working, and a drag never counts as a click.
   */
  S.dragRows = function (e, o) {
    if (e.button > 0 || e.isPrimary === false) return;
    const row = e.target.closest(o.row);
    if (!row) return;
    const control = e.target.closest('a, button, input, select, textarea, [role="button"], [data-go]');
    if (control && control !== row) return;
    const scope = o.scope || document;
    const touch = e.pointerType === 'touch';
    const startX = e.clientX, startY = e.clientY;
    const scrollEl = o.scroller || S.$('#main');
    let dragging = false, hold = null, auto = null, lastY = startY, ghost = null;
    let items = [], fromPos = -1, toPos = -1, grab = 0, step = 0, scroll0 = 0;

    const rowsNow = function () { return o.rows ? o.rows() : S.$$(o.row, scope); };

    /** Pick the row up: a see-through copy follows the pointer, and the others make room as it passes. */
    function begin() {
      dragging = true;
      const rows = rowsNow();
      scroll0 = scrollEl.scrollTop;
      items = rows.map(function (r) { const b = r.getBoundingClientRect(); return { el: r, top: b.top + scroll0, h: b.height, idx: o.index(r) }; });
      fromPos = rows.indexOf(row); toPos = fromPos;
      const rb = row.getBoundingClientRect();
      grab = startY - rb.top;
      // distance from one row to the next (its height plus any gap)
      step = rb.height;
      for (let j = 1; j < items.length; j++) step = j === 1 ? Math.abs(items[1].top - items[0].top) : Math.min(step, Math.abs(items[j].top - items[j - 1].top));
      if (items.length < 2) step = rb.height;
      ghost = row.cloneNode(true);
      ghost.classList.add('drag-ghost');
      ghost.style.width = rb.width + 'px'; ghost.style.height = rb.height + 'px';
      ghost.style.left = rb.left + 'px'; ghost.style.top = rb.top + 'px';
      row.parentNode.appendChild(ghost);
      row.classList.add('dragging');
      rows.forEach(function (r) { r.classList.add('drag-shifting'); });
      document.body.classList.add('reordering');
      try { window.getSelection().removeAllRanges(); if (navigator.vibrate) navigator.vibrate(8); } catch (err) { /* no vibration */ }
      if (touch) document.addEventListener('touchmove', block, { passive: false });
      // scroll while holding a row near the top or bottom edge
      auto = setInterval(function () {
        const b = scrollEl.getBoundingClientRect();
        if (lastY < b.top + 60) scrollEl.scrollTop -= 12; else if (lastY > b.bottom - 60) scrollEl.scrollTop += 12;
        place(lastY);
      }, 30);
    }
    function block(ev) { ev.preventDefault(); }

    /** Move the copy under the pointer and slide the other rows up or down to open a gap where it would land. */
    function place(y) {
      lastY = y;
      if (!ghost) return;
      ghost.style.top = (y - grab) + 'px';
      // where the copy's middle sits, in the same coordinates as the rows' starting positions (client + scroll)
      const mid = (y - grab) + scrollEl.scrollTop + items[fromPos].h / 2;
      let to = 0;
      for (let j = 0; j < items.length; j++) {
        if (j !== fromPos && items[j].top + items[j].h / 2 < mid) to++;
      }      toPos = Math.max(0, Math.min(items.length - 1, to));
      items.forEach(function (it, j) {
        if (j === fromPos) return;
        let dy = 0;
        if (fromPos < toPos && j > fromPos && j <= toPos) dy = -step;
        else if (fromPos > toPos && j >= toPos && j < fromPos) dy = step;
        it.el.style.transform = dy ? 'translateY(' + dy + 'px)' : '';
      });
    }
    function move(ev) {
      if (ev.pointerId !== e.pointerId) return;
      lastY = ev.clientY;
      if (!dragging) {
        const d = Math.hypot(ev.clientX - startX, ev.clientY - startY);
        if (touch) { if (d > 8) stop(false); return; } // moved before the hold finished: they're scrolling
        if (d < 5) return;
        begin();
      }
      place(ev.clientY);
    }
    function stop(drop) {
      clearTimeout(hold); clearInterval(auto);
      window.removeEventListener('pointermove', move);
      window.removeEventListener('pointerup', up);
      window.removeEventListener('pointercancel', cancel);
      document.removeEventListener('touchmove', block);
      if (!dragging) return;
      document.body.classList.remove('reordering');
      if (ghost) ghost.remove();
      items.forEach(function (it) { it.el.classList.remove('dragging', 'drag-shifting'); it.el.style.transform = ''; });
      // the click that follows letting go must not play the row
      const swallow = function (ev) { ev.stopPropagation(); ev.preventDefault(); };
      window.addEventListener('click', swallow, true);
      setTimeout(function () { window.removeEventListener('click', swallow, true); }, 60);
      if (drop && toPos !== fromPos && toPos >= 0) o.drop(items[fromPos].idx, items[toPos].idx);
    }
    function up() { stop(true); }
    function cancel() { stop(false); }
    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', up);
    window.addEventListener('pointercancel', cancel);
    if (touch) hold = setTimeout(begin, 280);
  };
  /** The (i) menu next to what's playing: just things about this song. Lyrics joins it when lyrics exist. */
  S.songInfoMenu = function (anchor) {
    const t = S.player.current();
    if (!t) return;
    S.popover(anchor, {
      items: [
        S.lyrics && S.lyrics.open ? { label: 'Lyrics', icon: 'lyrics', onClick: function () { S.lyrics.open(t.id); } } : null,
        { label: 'Credits', icon: 'book', onClick: function () { S.creditsDialog(t.id); } }
      ]
    });
  };
  document.addEventListener('click', function (e) {
    const b = e.target.closest('[data-player="info"], [data-np-info]');
    if (b) { e.preventDefault(); e.stopPropagation(); S.songInfoMenu(b); }
  });

  /* ---------- credits ---------- */
  // everyone credited on a song in the library (by file tags), lower-cased -> display name
  let creditIdx = null;
  S.creditIndexReset = function () { creditIdx = null; };
  function creditIndex() {
    if (creditIdx) return creditIdx;
    creditIdx = {};
    Object.keys(S.data.tracks).forEach(function (id) {
      (S.data.tracks[id].credits || []).forEach(function (c) {
        if (c.role === 'ISRC') return;
        c.names.split(', ').forEach(function (n) { n = n.trim(); if (n && !creditIdx[n.toLowerCase()]) creditIdx[n.toLowerCase()] = n; });
      });
    });
    return creditIdx;
  }
  /** Every song in the library a person is credited on (as artist or by role). */
  S.creditedSongs = function (name) {
    const low = name.toLowerCase(), out = [], roles = {};
    Object.keys(S.data.tracks).forEach(function (id) {
      const t = S.data.tracks[id];
      const rs = [];
      if (t.artist.toLowerCase() === low) rs.push('Artist');
      (t.credits || []).forEach(function (c) {
        if (c.role !== 'ISRC' && c.names.split(', ').some(function (n) { return n.trim().toLowerCase() === low; })) rs.push(c.role);
      });
      if (rs.length) { out.push(id); rs.forEach(function (r) { roles[r] = (roles[r] || 0) + 1; }); }
    });
    return { ids: out, roles: roles };
  };
  /** A person's name as a link: their artist page, or their credits page, or plain text if we know nothing else. */
  S.personLink = function (name) {
    const low = name.toLowerCase();
    const artist = S.data.artists.find(function (n) { return n.toLowerCase() === low; });
    if (artist) return S.link(S.artistUrl(artist), S.esc(name));
    if (creditIndex()[low]) return S.link('#/credit/' + encodeURIComponent(name), S.esc(name));
    return S.esc(name);
  };
  function nameList(names) { return names.split(', ').map(function (n) { return S.personLink(n.trim()); }).join(', '); }
  function creditRows(list) {
    return list.map(function (c) { return '<div class="credit-row"><dt>' + S.esc(c.role) + '</dt><dd>' + nameList(c.names) + '</dd></div>'; }).join('');
  }
  /** pairs are [label, text] or [label, text, html]; html is trusted markup that replaces the text. */
  function detailRows(pairs) {
    return pairs.filter(function (p) { return p[1]; }).map(function (p) { return '<div class="credit-row"><dt>' + S.esc(p[0]) + '</dt><dd>' + (p[2] || S.esc(p[1])) + '</dd></div>'; }).join('');
  }

  /** Who made a song: what's tagged in the file, plus what MusicBrainz knows. */
  S.creditsDialog = function (trackId) {
    const t = S.data.track(trackId), al = S.data.album(t.albumId);
    const q = S.lib.quality(al);
    const own = t.credits || [];
    const m = S.modal({
      title: 'Credits',
      body: '<div class="credits">' +
        '<div class="credits-head"><a class="credits-art" href="#/album/' + al.id + '" aria-label="Go to the album">' + S.cover(al) + '</a><div>' +
          '<strong>' + S.esc(t.title) + '</strong>' +
          '<span class="credits-artist">' + S.personLink(t.artist) + '</span>' +
          '<span>' + S.albumLink(al) + (al.year ? ' · ' + al.year : '') + '</span></div></div>' +
        '<div class="q-label">Credits</div>' +
        '<dl class="credit-list">' + '<div class="credit-row"><dt>Artist</dt><dd>' + S.personLink(t.artist) + '</dd></div>' + creditRows(own.filter(function (c) { return c.role !== 'ISRC'; })) + '<span data-mb></span></dl>' +
        '<p class="set-hint" data-mb-note><span class="chip-src">MusicBrainz</span> Looking up more…</p>' +
        '<div class="q-label">Details</div>' +
        '<dl class="credit-list">' + detailRows([
          ['Album', al.title, S.albumLink(al)], ['Year', al.year ? String(al.year) : ''], ['Label', al.label], ['Genre', al.genre],
          ['Format', q.detail], ['Length', t.duration ? S.fmtTime(t.duration) : ''], ['Copyright', al.copyright],
          ['ISRC', ((own.find(function (c) { return c.role === 'ISRC'; }) || {}).names) || '']
        ]) + '</dl>' + (function () {
          const mine = S.stats && S.stats.get(trackId);
          return '<div class="q-label">Your listening</div><dl class="credit-list">' + (mine
            ? detailRows([['Plays', String(mine.n)], ['First played', S.fmtDay(mine.first)], ['Last played', S.fmtDay(mine.last)]])
            : detailRows([['Plays', 'Not played yet']])) + '</dl>';
        })() + '</div>',
      actions: [{ label: 'Close', kind: 'secondary' }]
    });
    S.server.json('GET', '/api/credits/' + encodeURIComponent(trackId)).then(function (r) {
      if (!document.body.contains(m.el)) return;
      // MusicBrainz fills in what the file doesn't say
      const have = own.map(function (c) { return c.role; });
      const extra = r.credits.filter(function (c) { return have.indexOf(c.role) === -1; });
      S.$('[data-mb]', m.el).outerHTML = creditRows(extra);
      const chip = '<span class="chip-src">MusicBrainz</span> ';
      S.$('[data-mb-note]', m.el).innerHTML = r.credits.length
        ? chip + (extra.length ? 'Added credits the file doesn\'t have.' : 'Matches what\'s in the file.')
        : (own.length ? '' : 'No credits found for this song in the file or on MusicBrainz.');
      if (!own.length && !r.credits.length) S.$('[data-mb-note]', m.el).classList.add('credits-none');
    }).catch(function (x) {
      if (!document.body.contains(m.el)) return;
      S.$('[data-mb]', m.el).remove();
      S.$('[data-mb-note]', m.el).textContent = own.length ? '' : (x.message || 'Couldn\'t reach MusicBrainz.');
    });
  };

  /** Album info: label, catalog number, copyright, formats, and everyone credited on its songs. */
  S.albumInfoDialog = function (albumId) {
    const al = S.data.album(albumId);
    const q = S.lib.quality(al);
    const total = S.lib.totalDuration(al.trackIds);
    // who's credited anywhere on the album, by role
    const roles = {};
    al.trackIds.forEach(function (id) {
      (S.data.track(id).credits || []).forEach(function (c) {
        if (c.role === 'ISRC') return;
        const set = roles[c.role] = roles[c.role] || [];
        c.names.split(', ').forEach(function (n) { if (!set.some(function (x) { return x.toLowerCase() === n.toLowerCase(); })) set.push(n); });
      });
    });
    const people = Object.keys(roles).map(function (r) { return { role: r, names: roles[r].join(', ') }; });
    S.modal({
      title: 'Album info',
      body: '<div class="credits"><div class="credits-head"><a class="credits-art" href="#/album/' + al.id + '" aria-label="Go to the album">' + S.cover(al) + '</a><div><strong>' + S.albumLink(al) + '</strong>' +
        '<span class="credits-artist">' + S.personLink(al.artist) + '</span></div></div>' +
        '<dl class="credit-list">' + detailRows([
          ['Released', al.year ? String(al.year) : ''], ['Label', al.label], ['Catalog number', al.catalogNumber], ['Genre', al.genre],
          ['Songs', al.trackIds.length + ' · ' + S.fmtLong(total)], ['Quality', q.label + (q.detail ? ' (' + q.detail + ')' : '')], ['Copyright', al.copyright]
        ]) + '</dl>' +
        (people.length ? '<div class="q-label">Credits on this album</div><dl class="credit-list">' + creditRows(people) + '</dl>'
          : '<p class="set-hint">No credits are tagged in these files. Open a song\'s ⋯ menu → Credits to look it up on MusicBrainz.</p>') +
        '</div>',
      actions: [{ label: 'Close', kind: 'secondary' }]
    });
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
    if (S.dl && S.dl.supported) {
      const got = S.dl.has(trackId);
      items.push({ label: got ? 'Remove download' : 'Download', icon: got ? 'check' : 'download', onClick: function () { got ? S.dl.remove([trackId]) : S.dl.add([trackId]); } });
    }
    items.push({ label: 'Go to album', icon: 'disc', onClick: function () { S.router.go('/album/' + t.albumId); } });
    items.push({ label: 'Credits', icon: 'book', onClick: function () { S.creditsDialog(trackId); } });
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
  /** Toast with a button. It goes to "path", or runs "onClick" instead when given (e.g. Join a party). */
  S.linkToast = function (message, label, path, onClick) {
    const host = document.getElementById('toasts');
    const el = document.createElement('div');
    el.className = 'toast toast-action';
    el.innerHTML = '<span>' + S.esc(message) + '</span><button class="toast-btn">' + S.esc(label) + '</button>';
    el.querySelector('button').addEventListener('click', function () { el.remove(); if (onClick) onClick(); else S.router.go(path); });
    host.appendChild(el);
    const stay = onClick ? 12000 : 5000; // an invite needs longer than a confirmation
    setTimeout(function () { el.classList.add('leaving'); }, stay);
    setTimeout(function () { el.remove(); }, stay + 300);
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
      const pl = S.pl.get(playPl.dataset.playPlaylist);
      if (pl) S.player.play(pl.tracks.map(function (x) { return x.id; }), 0, { type: 'playlist', id: pl.id, name: pl.name });
      return;
    }
    const likeBtn = e.target.closest('[data-like]');
    if (likeBtn) { e.preventDefault(); e.stopPropagation(); if (likeBtn.dataset.like) S.pl.toggleLike(likeBtn.dataset.like); return; }
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
    if (row && !e.target.closest('a, button, [data-go]')) activateRow(row);
  });

  document.addEventListener('keydown', function (e) {
    if (e.key !== 'Enter') return;
    const row = e.target.closest && e.target.closest('.track[data-track-id]');
    if (row && e.target === row) activateRow(row); // Enter on a link/button inside the row does its own thing
  });

  /** Heart: like / unlike a song (adds or removes it in Liked Songs). */
  S.likeButton = function (id, cls, size) {
    const liked = S.pl && S.pl.isLiked(id);
    const label = liked ? 'Remove from Liked Songs' : 'Add to Liked Songs';
    return '<button class="like-btn' + (cls ? ' ' + cls : '') + (liked ? ' on' : '') + '" data-like="' + id + '" data-size="' + (size || 16) + '" aria-pressed="' + (liked ? 'true' : 'false') +
      '" aria-label="' + label + '" title="' + label + '">' + S.icon(liked ? 'heartFill' : 'heart', size || 16) + '</button>';
  };

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
