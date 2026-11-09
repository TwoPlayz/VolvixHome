/* Volvix Home — the household: playlists several people edit (S.shared), what everyone's listening to and
 * their public playlists (S.household), and telling the others what you're playing (presence). */
window.Spin = window.Spin || {};
(function (S) {
  'use strict';

  const VH = window.VH || {};
  const meId = function () { return S.data.me.serverId; };
  /** Inside the app you are 'u_me'; everyone else keeps their server id. */
  const toMe = function (id) { return id === meId() ? 'u_me' : id; };

  /* ---------- shared (collaborative) playlists ---------- */
  function fromServer(s) {
    return {
      id: s.id, name: s.name, description: s.description || '', owner: toMe(s.owner),
      collaborative: true, shared: true, public: !!s.public,
      collaborators: (s.collaborators || []).map(toMe), invited: s.invited || [], rev: s.rev || 0,
      tracks: (s.tracks || []).map(function (t) { return { id: t.id, addedBy: toMe(t.addedBy), at: t.at || 0 }; })
    };
  }
  let sharedList = ((VH.shared && VH.shared.playlists) || []).map(fromServer);
  let invites = (VH.shared && VH.shared.invites) || [];

  function applyResult(r) {
    if (r.gone) sharedList = sharedList.filter(function (p) { return p.id !== r.playlist.id; });
    else {
      const pl = fromServer(r.playlist);
      const i = sharedList.findIndex(function (p) { return p.id === pl.id; });
      if (i >= 0) sharedList[i] = pl; else sharedList.push(pl);
    }
  }

  S.shared = {
    all: function () { return sharedList; },
    invites: function () { return invites; },
    get: function (id) { return sharedList.find(function (p) { return p.id === id; }) || null; },
    fromServer: fromServer,
    /** Change one of them on the server (add, remove, move, rename, invite…). Resolves with the server's answer. */
    op: function (id, body) {
      return S.server.json('POST', '/api/shared/' + encodeURIComponent(id) + '/op', body).then(function (r) { applyResult(r); return r; });
    },
    /** Make one of your playlists shared (you stay the owner). */
    create: function (pl) {
      return S.server.json('POST', '/api/shared', {
        name: pl.name, description: pl.description || '', public: !!pl.public, tracks: pl.tracks.map(function (t) { return t.id; })
      }).then(function (r) { applyResult(r); return fromServer(r.playlist); });
    },
    /** Fetch the latest. Resolves true when something changed. */
    refresh: function () {
      return S.server.json('GET', '/api/shared').then(function (r) {
        const next = r.playlists.map(fromServer);
        const changed = JSON.stringify(next.map(function (p) { return [p.id, p.rev, p.collaborators.length, p.invited.length]; })) !==
          JSON.stringify(sharedList.map(function (p) { return [p.id, p.rev, p.collaborators.length, p.invited.length]; })) ||
          r.invites.length !== invites.length;
        sharedList = next; invites = r.invites;
        return changed;
      });
    }
  };

  /* ---------- everyone else ---------- */
  let hh = VH.household || { members: [] };
  S.household = {
    members: function () { return hh.members; },
    load: function () { return S.server.json('GET', '/api/household').then(function (r) { hh = r; return r; }); },
    /** A public playlist of someone else, in the shape the playlist page uses (read-only for you). */
    playlist: function (id) {
      for (const m of hh.members) {
        const p = m.playlists.find(function (x) { return x.id === id; });
        if (p) {
          return { id: p.id, name: p.name, description: p.description || '', owner: m.id, collaborative: (p.collaborators || []).length > 0,
            collaborators: p.collaborators || [], public: true, household: true,
            tracks: (p.tracks || []).map(function (t) { return { id: t.id, addedBy: toMe(t.addedBy), at: t.at || 0 }; }) };
        }
      }
      return null;
    },
    allPlaylists: function () {
      const out = [];
      hh.members.forEach(function (m) { m.playlists.forEach(function (p) { out.push(S.household.playlist(p.id)); }); });
      return out.filter(Boolean);
    }
  };

  /* ---------- presence: tell the household what you're playing ---------- */
  let lastSent = '';
  function body() {
    const s = S.store.state.settings;
    const t = S.player.current();
    const share = s.shareActivity !== false && !s.privateSession && !!t;
    return share ? { trackId: t.id, playing: !!S.player.state.playing } : { trackId: null, playing: false };
  }
  function report(heartbeat) {
    const b = body();
    const key = JSON.stringify(b);
    // a change is sent right away; while something plays we also repeat every 15 s so we stay "live"
    if (key === lastSent && !(heartbeat && b.trackId && b.playing)) return;
    lastSent = key;
    fetch('/api/me/now', { method: 'PUT', credentials: 'same-origin', headers: { 'Content-Type': 'application/json' }, body: key }).catch(function () { /* offline */ });
  }
  S.household.syncPresence = function () { report(false); };
  S.player.hooks.push(function () { report(false); });
  setInterval(function () { report(true); }, 15000);
  window.addEventListener('pagehide', function () {
    try { fetch('/api/me/now', { method: 'PUT', keepalive: true, credentials: 'same-origin', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ trackId: null, playing: false }) }); } catch (e) { /* ignore */ }
  });

  /* ---------- keep shared playlists fresh while the app is open ---------- */
  setInterval(function () {
    if (document.hidden) return;
    S.shared.refresh().then(function (changed) {
      if (!changed) return;
      S.app.refreshSidebar();
      // don't yank the page away from someone mid-edit
      if (document.querySelector('.modal-scrim') || document.body.classList.contains('reordering')) return;
      if (/^\/(playlist|library|home)/.test(S.router.current())) S.app.refresh();
    }).catch(function () { /* next time */ });
  }, 20000);
})(window.Spin);
