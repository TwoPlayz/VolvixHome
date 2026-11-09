/* Volvix Home — Notifications page: playlist invites (accept / decline), party invites, requests that arrived. */
window.Spin = window.Spin || {};
Spin.views = Spin.views || {};
(function (S) {
  'use strict';

  const ICONS = { playlist_invite: 'users', playlist_joined: 'check', party_invite: 'party', request_ready: 'download' };
  let host = null;
  let shownUnread = {}; // ids that were unread when the page opened (keep their dot while you read)

  function actions(n) {
    if (n.kind === 'playlist_invite') {
      if (n.done) return '<span class="note-state">' + (n.done === 'accepted' ? S.icon('check', 14) + 'Joined' : 'Declined') + '</span>';
      return '<button class="btn btn-primary btn-sm" data-note-accept="' + S.esc(n.data.playlistId) + '" data-note="' + n.id + '">Accept</button>' +
        '<button class="btn btn-secondary btn-sm" data-note-decline="' + S.esc(n.data.playlistId) + '" data-note="' + n.id + '">Decline</button>';
    }
    if (n.kind === 'playlist_joined') return '<a class="btn btn-secondary btn-sm" href="#/playlist/' + S.esc(n.data.playlistId) + '">Open</a>';
    if (n.kind === 'party_invite') {
      if (n.done) return '<span class="note-state">' + (n.done === 'joined' ? S.icon('check', 14) + 'Joined' : 'Over') + '</span>';
      return '<button class="btn btn-primary btn-sm" data-note-join="' + n.id + '">Join</button>';
    }
    if (n.kind === 'request_ready') {
      const artist = n.data.kind === 'artist';
      const here = artist ? S.data.artists.indexOf(n.data.albumId) !== -1 : !!S.data.album(n.data.albumId);
      const href = artist ? S.artistUrl(n.data.albumId) : '#/album/' + n.data.albumId;
      return here ? '<a class="btn btn-secondary btn-sm" href="' + href + '">Open</a>'
        : '<button class="btn btn-secondary btn-sm" data-note-reload="' + S.esc(href) + '">Show it</button>';
    }
    return '';
  }

  function row(n) {
    return '<div class="note-row' + (n.read && !shownUnread[n.id] ? '' : ' unread') + '">' +
      '<span class="note-icon">' + S.icon(ICONS[n.kind] || 'bell', 18) + '</span>' +
      '<div class="note-text"><span>' + S.esc(n.text) + '</span><em>' + S.ago(n.at) + '</em></div>' +
      '<div class="note-actions">' + actions(n) + '</div></div>';
  }

  function draw() {
    if (!host || !document.body.contains(host)) return;
    const items = S.notify.items();
    host.innerHTML = items.length ? items.map(row).join('') :
      S.emptyState('bell', 'Nothing new', 'Playlist invites, party invites and music you requested show up here.');
  }

  S.views.notifications = {
    redraw: draw,
    render: function (root) {
      root.innerHTML = '<header class="page-head page-head-row"><div><h1 class="page-title">Notifications</h1></div>' +
        '<div class="head-actions"><button class="btn btn-secondary btn-sm" data-read-all>Mark all as read</button></div></header>' +
        '<div class="note-list" data-notes></div>';
      host = S.$('[data-notes]', root);
      shownUnread = {};
      S.notify.items().forEach(function (n) { if (!n.read) shownUnread[n.id] = true; });
      draw();
      // opening the page counts as reading
      if (S.notify.unread()) setTimeout(function () { S.notify.markRead(); }, 1200);

      root.onclick = function (e) {
        const t = e.target;
        if (t.closest('[data-read-all]')) { shownUnread = {}; S.notify.markRead(); draw(); return; }
        const acc = t.closest('[data-note-accept]'), dec = t.closest('[data-note-decline]');
        if (acc || dec) {
          const b = acc || dec, id = b.dataset.noteAccept || b.dataset.noteDecline;
          b.disabled = true;
          S.shared.op(id, { op: acc ? 'accept' : 'decline' }).then(function () {
            S.notify.setDone(b.dataset.note, acc ? 'accepted' : 'declined');
            S.app.refreshSidebar(); draw();
            S.toast(acc ? 'You can now edit this playlist' : 'Declined', acc ? 'check' : null);
            return S.notify.load();
          }).catch(function (x) { b.disabled = false; S.toast(x.message, 'x'); });
          return;
        }
        const join = t.closest('[data-note-join]');
        if (join) { const n = S.notify.items().find(function (x) { return x.id === join.dataset.noteJoin; }); if (n && S.party) S.party.joinFromNote(n); return; }
        const rel = t.closest('[data-note-reload]');
        if (rel) { S.data.sync().then(function () { location.hash = rel.dataset.noteReload.replace(/^#/, ''); S.app.refresh(); }); }
      };
    }
  };
})(window.Spin);
