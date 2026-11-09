/* Volvix Home — start-up: sign in (or set up a fresh server), load your library, then start the app.
 * Runs before the rest of the app so every module can use the real catalog synchronously. */
(function () {
  'use strict';

  // the app, in load order (boot.js appends these once the library is loaded)
  const SCRIPTS = [
    'js/ui.js', 'js/data.js', 'js/store.js', 'js/downloads.js', 'js/router.js', 'js/components.js', 'js/player.js', 'js/stats.js', 'js/household.js',
    'js/playlists.js', 'js/nowplaying.js', 'js/lyrics.js', 'js/notifications.js', 'js/party.js', 'js/devices.js', 'js/cast.js', 'js/output.js', 'js/views/home.js', 'js/views/search.js',
    'js/views/library.js', 'js/views/album.js', 'js/views/artist.js', 'js/views/credit.js','js/views/playlist.js', 'js/views/household.js',
    'js/views/notifications.js', 'js/views/rewind.js', 'js/views/account.js', 'js/views/requests.js', 'js/intro.js', 'js/app.js'
  ];

  const app = document.getElementById('app');

  function get(path) {
    return fetch(path, { credentials: 'same-origin' }).then(function (r) { return r.ok ? r.json() : Promise.reject(r.status); });
  }
  function post(path, body) {
    return fetch(path, { method: 'POST', credentials: 'same-origin', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body) })
      .then(function (r) { return r.json().catch(function () { return {}; }).then(function (j) { if (!r.ok) throw new Error(j.error || 'Something went wrong'); return j; }); });
  }
  function esc(s) { return String(s).replace(/[&<>"']/g, function (c) { return { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]; }); }
  function hideSplash() {
    const el = document.getElementById('splash');
    if (el) { el.classList.add('splash-out'); setTimeout(function () { el.remove(); }, 450); }
  }

  const LOGO = '<svg viewBox="0 0 32 32" width="28" height="28" aria-hidden="true"><circle cx="16" cy="16" r="15" fill="#E5484D"/><circle cx="16" cy="16" r="9.5" fill="none" stroke="#000" stroke-opacity=".22"/><circle cx="16" cy="16" r="3.5" fill="#111"/></svg>';

  /** Sign-in, or first-run setup (creates the admin account). */
  function authScreen(setup, serverName) {
    hideSplash();
    document.title = (setup ? 'Set up' : 'Sign in') + ' · Volvix';
    app.innerHTML =
      '<div class="vh-auth"><form class="vh-auth-card" novalidate>' +
        '<div class="vh-auth-brand">' + LOGO + '<span>Volvix</span></div>' +
        '<h1>' + (setup ? 'Set up your server' : 'Welcome back') + '</h1>' +
        '<p class="vh-auth-sub">' + (setup
          ? 'Create the admin account. You can add everyone else in your household afterwards.'
          : 'Sign in to ' + esc(serverName || 'your music') + '.') + '</p>' +
        '<label class="field"><span class="label">' + (setup ? 'Your name' : 'Name') + '</span><input class="input" name="name" autocomplete="' + (setup ? 'name' : 'username') + '" autocapitalize="words" spellcheck="false" required></label>' +
        '<label class="field"><span class="label">Password</span><input class="input" name="password" type="password" autocomplete="' + (setup ? 'new-password' : 'current-password') + '" required>' +
          (setup ? '<span class="hint">At least 8 characters.</span>' : '') + '</label>' +
        '<p class="auth-error" hidden></p>' +
        '<button class="btn btn-primary btn-block" type="submit">' + (setup ? 'Create account' : 'Sign in') + '</button>' +
        (setup ? '' : '<p class="vh-auth-foot">No account? Ask whoever runs this server to add you.</p>') +
      '</form></div>';
    const form = app.querySelector('form');
    const err = app.querySelector('.auth-error');
    form.querySelector('input').focus();
    form.addEventListener('submit', function (e) {
      e.preventDefault();
      const f = new FormData(form);
      const btn = form.querySelector('button[type=submit]');
      btn.disabled = true; err.hidden = true;
      post(setup ? '/api/setup' : '/api/login', { name: f.get('name') || '', password: f.get('password') })
        .then(function () { start(); })
        .catch(function (x) { err.textContent = x.message; err.hidden = false; btn.disabled = false; });
    });
  }

  function failScreen(message) {
    hideSplash();
    app.innerHTML = '<div class="vh-auth"><div class="vh-auth-card"><div class="vh-auth-brand">' + LOGO + '<span>Volvix</span></div>' +
      '<h1>Can\'t reach your server</h1><p class="vh-auth-sub">' + esc(message) + '</p>' +
      '<button class="btn btn-primary btn-block" onclick="location.reload()">Try again</button></div></div>';
  }

  function loadScripts(list, done) {
    if (!list.length) return done();
    const s = document.createElement('script');
    s.src = list[0] + '?v=' + (window.VH_BUILD || (window.VOLVIX_CONFIG && window.VOLVIX_CONFIG.built) || '');
    s.onload = function () { loadScripts(list.slice(1), done); };
    s.onerror = function () { failScreen('Part of the app (' + list[0] + ') didn\'t load.'); };
    document.body.appendChild(s);
  }

  function start() {
    get('/api/me').then(function (me) {
      const soft = function (p, fallback) { return p.catch(function () { return fallback; }); };
      return Promise.all([
        get('/api/library'), soft(get('/api/me/data'), null),
        soft(get('/api/people'), { people: [] }), soft(get('/api/shared'), { playlists: [], invites: [] }),
        soft(get('/api/household'), { members: [] }), soft(get('/api/notifications'), { items: [], unread: 0 })
      ]).then(function (r) {
        window.VH = { user: me.user, lidarr: me.lidarr || {}, catalog: r[0], userdata: r[1],
          people: r[2].people, shared: r[3], household: r[4], notifications: r[5] };
        app.innerHTML = '';
        loadScripts(SCRIPTS, function () { /* app.js starts itself */ });
      });
    }).catch(function (status) {
      if (status !== 401) return failScreen('Check that Volvix Home is running, then try again.');
      get('/api/status').then(function (s) { authScreen(s.setupNeeded, s.name); })
        .catch(function () { failScreen('Check that Volvix Home is running, then try again.'); });
    });
  }

  // lets the app open without a connection (see sw.js)
  if ('serviceWorker' in navigator) {
    // when a newer service worker takes over (an app update), reload once so the page runs the new files
    const had = !!navigator.serviceWorker.controller;
    navigator.serviceWorker.addEventListener('controllerchange', function () { if (had && !window.__vhReloaded) { window.__vhReloaded = true; location.reload(); } });
    navigator.serviceWorker.register('/sw.js').catch(function () { /* not allowed here: fine */ });
  }

  start();
})();
