/* Volvix Home — your app data: playlists, recently played, settings.
 * Kept per person on the server (/api/me/data), so every device you sign in on has the same playlists
 * and picks up where you left off. A local copy makes start-up instant and covers brief disconnects. */
window.Spin = window.Spin || {};
(function (S) {
  'use strict';

  const me = (window.VH && window.VH.user) || { id: 'local' };
  const STORE_KEY = 'volvixhome.' + me.id;   // one local copy per person on a shared computer

  function defaults() {
    return {
      v: 1,
      theme: 'system',                 // 'system' | 'dark' | 'light'
      playlists: [],
      followed: [],                    // other people's playlists you follow
      followedArtists: [],             // artist names you follow (Library → Artists)
      recent: [],                      // album ids, most recent first
      settings: {
        gapless: true,
        normalize: false,
        privateSession: false,
        shareActivity: true,             // let the household see what you're listening to
        allowExplicit: true
      }
    };
  }

  function merge(saved) {
    const base = defaults();
    if (!saved || typeof saved !== 'object') return base;
    const out = Object.assign(base, saved);
    out.settings = Object.assign(defaults().settings, saved.settings || {});
    // drop anything that points at music no longer on the server
    out.recent = (out.recent || []).filter(function (id) { return S.data.album(id); });
    return out;
  }

  function load() {
    let local = null;
    try { local = JSON.parse(localStorage.getItem(STORE_KEY) || 'null'); } catch (e) { /* storage blocked */ }
    const server = window.VH && window.VH.userdata;
    // newest copy wins (a device that was offline may have changes the server hasn't seen)
    const pick = server && (!local || (server.savedAt || 0) >= (local.savedAt || 0)) ? server : local;
    return merge(pick);
  }

  let state = load();
  const subscribers = new Set();

  /* ---------- saving: locally right away, to the server a moment later ---------- */
  let pushTimer = null, pushing = false, again = false;
  function push() {
    if (pushing) { again = true; return; }
    pushing = true;
    fetch('/api/me/data', { method: 'PUT', credentials: 'same-origin', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(state) })
      .catch(function () { /* offline: the local copy is newer and will win next time */ })
      .finally(function () { pushing = false; if (again) { again = false; push(); } });
  }
  function save() {
    state.savedAt = Date.now();
    try { localStorage.setItem(STORE_KEY, JSON.stringify(state)); } catch (e) { /* ignore */ }
    clearTimeout(pushTimer);
    pushTimer = setTimeout(push, 800);
  }
  window.addEventListener('pagehide', function () {
    if (!pushTimer) return;
    clearTimeout(pushTimer);
    try { fetch('/api/me/data', { method: 'PUT', keepalive: true, credentials: 'same-origin', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(state) }); } catch (e) { /* ignore */ }
  });

  S.store = {
    get state() { return state; },
    isPreview: false,
    update: function (fn) {
      fn(state);
      save();
      subscribers.forEach(function (sub) { sub(state); });
    },
    subscribe: function (fn) { subscribers.add(fn); return function () { subscribers.delete(fn); }; },
    /** Small side records kept on this device only (e.g. the exact playback position, saved every few seconds). */
    aux: {
      get: function (name) { try { return JSON.parse(localStorage.getItem(STORE_KEY + ':' + name) || 'null'); } catch (e) { return null; } },
      set: function (name, value) {
        try { if (value == null) localStorage.removeItem(STORE_KEY + ':' + name); else localStorage.setItem(STORE_KEY + ':' + name, JSON.stringify(value)); } catch (e) { /* full */ }
      }
    },
    // signed in by definition: boot.js only starts the app with a valid session
    session: function () { return { method: 'server' }; },
    setSession: function () {}
  };

  /* ---------- theme ---------- */
  const mq = window.matchMedia ? window.matchMedia('(prefers-color-scheme: light)') : null;
  S.applyTheme = function () {
    const t = state.theme;
    const resolved = t === 'system' ? (mq && mq.matches ? 'light' : 'dark') : t;
    document.documentElement.setAttribute('data-theme', resolved);
    let meta = document.querySelector('meta[name="theme-color"]');
    if (!meta) { meta = document.createElement('meta'); meta.name = 'theme-color'; document.head.appendChild(meta); }
    meta.content = resolved === 'light' ? '#E9E9EC' : '#000000';
    document.documentElement.style.colorScheme = resolved;
  };
  S.setTheme = function (theme) {
    S.store.update(function (s) { s.theme = theme; });
    S.applyTheme();
  };
  if (mq && mq.addEventListener) mq.addEventListener('change', function () { if (state.theme === 'system') S.applyTheme(); });
  S.applyTheme();
})(window.Spin);
