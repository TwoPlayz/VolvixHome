/* Volvix Home — app shell (sidebar, top bar, player bar, phone tab bar) and route table. */
window.Spin = window.Spin || {};
window.Spin.views = window.Spin.views || {};
(function (S) {
  'use strict';

  const NAV = [
    { route: '/home', label: 'Home', icon: 'home' },
    { route: '/search', label: 'Search', icon: 'search' },
    { route: '/library', label: 'Your Library', short: 'Library', icon: 'library' },
    { route: '/household', label: 'Household', icon: 'family' }
  ].concat(window.VH && window.VH.lidarr && window.VH.lidarr.configured ? [{ route: '/requests', label: 'Requests', icon: 'download' }] : []);

  let shellMounted = false;
  const appEl = document.getElementById('app');

  /* ---------- shell ---------- */
  function mountShell() {
    if (shellMounted) return;
    shellMounted = true;
    const me = S.data.me;

    appEl.innerHTML =
      '<div class="app">' +
        '<button class="skip-link" data-action="skip">Skip to content</button>' +
        '<aside class="sidebar">' +
          '<div class="sidebar-head"><a href="#/home" aria-label="Volvix Home">' + S.brand(28) + '</a></div>' +
          '<nav class="nav" aria-label="Main">' + NAV.map(function (n) {
            return '<a class="nav-item" href="#' + n.route + '" data-route="' + n.route + '">' + S.icon(n.icon, 22) + '<span>' + n.label + '</span></a>';
          }).join('') + '</nav>' +
          '<div class="sidebar-section">' +
            '<div class="section-head"><span>Playlists</span>' +
              '<button class="icon-btn" data-action="new-playlist" aria-label="Create playlist" title="Create playlist">' + S.icon('plus', 18) + '</button></div>' +
            '<div class="playlist-list" data-playlists></div>' +
          '</div>' +
          '<div class="sidebar-foot">' +
            '<button class="user-chip" data-action="account-menu" aria-label="Account menu" aria-haspopup="menu">' + S.avatar(me, 32) +
              '<span class="user-chip-text"><span class="user-chip-name">' + S.esc(me.name) + '</span>' +
              '<span class="user-chip-plan">' + (me.admin ? 'Admin' : 'Member') + '</span></span>' + S.icon('more', 18) + '</button>' +
          '</div>' +
        '</aside>' +

        '<main class="main" id="main">' +
          '<div class="topbar">' +
            '<button class="nav-arrow" data-action="back" aria-label="Go back">' + S.icon('chevronLeft', 18) + '</button>' +
            '<button class="nav-arrow" data-action="forward" aria-label="Go forward">' + S.icon('chevronRight', 18) + '</button>' +
            '<button class="mobile-back" data-action="back" aria-label="Back">' + S.icon('chevronLeft', 24) + '</button>' +
            '<a class="mobile-brand" href="#/home">' + S.brand(24) + '</a>' +
            '<div class="topbar-spacer"></div>' +
            S.notify.bellButton() +
            '<button class="avatar-btn" data-action="account-menu" aria-label="Account menu">' + S.avatar(me, 32) + '</button>' +
          '</div>' +
          '<div class="view" id="view" tabindex="-1"></div>' +
        '</main>' +
        '<aside class="queue-panel" data-p="queue-panel" aria-label="Queue" hidden></aside>' +

        playerBar() +

        '<nav class="tabbar" aria-label="Main">' + NAV.map(function (n) {
          return '<a class="tab" href="#' + n.route + '" data-route="' + n.route + '">' + S.icon(n.icon, 22) + '<span>' + (n.short || n.label) + '</span></a>';
        }).join('') + '</nav>' +
      '</div>';

    renderSidebarPlaylists();
  }

  function playerBar() {
    return '<footer class="player" aria-label="Player">' +
      '<div class="player-track">' +
        '<div class="player-art" data-p="art">' + S.icon('disc', 22) + '</div>' +
        '<div class="player-meta"><div class="player-title" data-p="title">Not playing</div>' +
        '<div class="player-sub" data-p="sub">Pick something from your library</div></div>' +
        '<button class="ctrl like-btn player-like" data-p="like" data-like="" data-size="18" aria-label="Add to Liked Songs" title="Add to Liked Songs" hidden>' + S.icon('heart', 18) + '</button>' +
        '<button class="ctrl player-add" data-p="add" data-add-current aria-label="Add to playlist" title="Add to playlist" hidden>' + S.icon('plus', 18) + '</button>' +
      '</div>' +
      '<div class="player-center">' +
        '<div class="player-controls">' +
          ctrl('shuffle', 'Shuffle', 'shuffle') + ctrl('prev', 'Previous', 'prev') +
          '<button class="ctrl-play" data-player="toggle" aria-label="Play">' + S.icon('play', 18) + '</button>' +
          ctrl('next', 'Next', 'next') + ctrl('repeat', 'Repeat', 'repeat') +
        '</div>' +
        '<div class="player-progress"><span class="time" data-p="pos">0:00</span>' +
          '<div class="bar" data-seek role="slider" aria-label="Seek"><div class="bar-fill" data-p="progress-fill" style="width:0%"></div></div>' +
          '<span class="time" data-p="dur">0:00</span></div>' +
      '</div>' +
      '<div class="player-right">' +
        '<button class="ctrl q-btn" data-player="quality" data-p="quality" aria-label="Audio quality" title="Audio quality">' + S.qualityIcon({ id: 'lossless', lossless: true }, 18) + '</button>' +
        ctrl('queue', 'Queue', 'queue') +
        '<div class="volume"><button class="ctrl" data-player="mute" aria-label="Mute">' + S.icon('volume', 18) + '</button>' +
          '<div class="bar" data-volume role="slider" aria-label="Volume"><div class="bar-fill" data-p="volume-fill" style="width:70%"></div></div></div>' +
      '</div>' +
      '<div class="mini-progress"><div data-p="mini-progress"></div></div>' +
    '</footer>';

    function ctrl(icon, label, action) {
      return '<button class="ctrl" data-player="' + action + '" aria-label="' + label + '" title="' + label + '">' + S.icon(icon, 18) + '</button>';
    }
  }

  function renderSidebarPlaylists() {
    const host = S.$('[data-playlists]');
    if (!host) return;
    host.innerHTML = S.pl.all().concat(S.pl.followed()).map(function (pl) {
      const n = pl.tracks.length + (pl.tracks.length === 1 ? ' song' : ' songs');
      const sub = pl.owner !== 'u_me' ? 'By ' + S.esc(S.data.person(pl.owner).name.split(' ')[0]) + ' · ' + n : (pl.collaborative ? 'Collaborative · ' : 'Playlist · ') + n;
      return '<a class="playlist-link" href="#/playlist/' + pl.id + '" data-route="/playlist/' + pl.id + '">' +
        S.playlistCover(pl, 'thumb') +
        '<span class="pl-text"><span class="pl-name">' + S.esc(pl.name) + '</span><span class="pl-sub">' + sub + '</span></span></a>';
    }).join('') || '<p class="pl-empty">Create your first playlist with +</p>';
    setActive(S.router.current());
  }

  function bindShell() {
    appEl.addEventListener('click', function (e) {
      const el = e.target.closest('[data-action]');
      if (!el) return;
      const action = el.dataset.action;
      if (action === 'back') goBack();
      else if (action === 'forward') history.forward();
      else if (action === 'account-menu') accountMenu(el);
      else if (action === 'new-playlist') S.pl.createDialog();
      else if (action === 'skip') { const v = S.$('#view'); v.focus(); }
    });
  }

  function accountMenu(anchor) {
    const me = S.data.me;
    S.popover(anchor, {
      header: '<div class="menu-user">' + S.avatar(me, 40) + '<div><div class="menu-name">' + S.esc(me.name) + '</div><div class="menu-sub">' + (me.admin ? 'Admin' : 'Member') + '</div></div></div>',
      items: [
        { label: 'Account', icon: 'user', onClick: function () { S.router.go('/account/profile'); } },
        me.admin ? { label: 'People', icon: 'family', onClick: function () { S.router.go('/account/people'); } } : null,
        { label: 'Settings', icon: 'settings', onClick: function () { S.router.go('/account/settings'); } },
        { sep: true },
        { label: 'Sign out', icon: 'logout', onClick: function () { S.server.logout(); } }
      ]
    });
  }

  /* ---------- back: in-app history, so Back never leaves the app ---------- */
  const ROOTS = ['/home', '/search', '/library', '/household', '/requests'];
  const trail = [];
  function track(path) {
    if (trail[trail.length - 1] === path) return;
    if (trail[trail.length - 2] === path) trail.pop(); else trail.push(path);
    if (trail.length > 50) trail.shift();
  }
  function goBack() {
    if (trail.length > 1) history.back();
    else { trail.length = 0; S.router.go('/home'); }
  }
  function isRoot(path) { return ROOTS.some(function (r) { return path === r || (r !== '/home' && path.indexOf(r + '/') === 0); }); }

  function setActive(path) {
    document.documentElement.classList.toggle('at-root', isRoot(path));
    S.$$('[data-route]').forEach(function (el) {
      const r = el.dataset.route;
      const active = path === r || (r !== '/home' && path.indexOf(r + '/') === 0);
      el.classList.toggle('active', active);
      if (active) el.setAttribute('aria-current', 'page'); else el.removeAttribute('aria-current');
    });
  }

  /* ---------- rendering ---------- */
  const SIDE_SCROLLERS = '.acct-nav, .set-index, .tabs, .chips';

  let splashDone = false;
  function hideSplash() {
    if (splashDone) return;
    splashDone = true;
    const el = document.getElementById('splash');
    if (!el) return;
    el.classList.add('splash-out');
    setTimeout(function () { el.remove(); }, 450);
  }

  S.app = {
    render: function (path, opts) {
      const fresh = !shellMounted;
      mountShell();
      if (fresh) S.player.render();
      const match = S.router.resolve(path) || { view: S.views.notFound, params: {} };
      const view = S.$('#view');
      // sideways-scrolling tab rows keep their position across re-renders
      const rowScroll = {};
      S.$$(SIDE_SCROLLERS, view).forEach(function (r) { rowScroll[r.className] = r.scrollLeft; });
      view.innerHTML = '';
      view.onclick = view.onchange = view.oninput = view.onsubmit = view.ondragstart = view.ondragover = view.ondrop = view.ondragend = view.onpointerdown = null;
      S.trackList.lists = {};
      match.view.render(view, match.params);
      S.$$(SIDE_SCROLLERS, view).forEach(function (r) {
        if (rowScroll[r.className] != null) r.scrollLeft = rowScroll[r.className];
        const on = r.querySelector('.on, .active, [aria-current="page"], [aria-selected="true"]');
        if (!on || r.scrollWidth <= r.clientWidth) return;
        const left = on.offsetLeft - r.offsetLeft, right = left + on.offsetWidth;
        if (left < r.scrollLeft + 8) r.scrollLeft = Math.max(0, left - 16);
        else if (right > r.scrollLeft + r.clientWidth - 8) r.scrollLeft = right - r.clientWidth + 16;
      });
      hideSplash();
      S.notify.updateBell();
      setActive(path);
      if (!opts || !opts.keepScroll) S.$('#main').scrollTop = 0;
      track(path);
      const h1 = view.querySelector('h1');
      document.title = (h1 ? h1.textContent + ' · ' : '') + 'Volvix';
    },
    /** Re-render the current screen in place (after data changes), keeping scroll position. */
    refresh: function () {
      if (!shellMounted) return;
      const main = S.$('#main');
      const top = main.scrollTop;
      S.app.render(S.router.current(), { keepScroll: true });
      main.scrollTop = top;
    },
    refreshSidebar: renderSidebarPlaylists,
    remount: function () {
      if (!shellMounted) return;
      shellMounted = false;
      S.app.render(S.router.current(), { keepScroll: true });
    }
  };

  /* ---------- routes ---------- */
  S.views.notFound = { render: function (root) {
    root.innerHTML = S.emptyState('search', 'Page not found', 'That page doesn\'t exist.', '<a class="btn btn-secondary" href="#/home">Go home</a>');
  } };

  S.router.add('/home', S.views.home);
  S.router.add('/search', S.views.search);
  S.router.add('/search/:q', S.views.search);
  S.router.add('/library', S.views.library);
  S.router.add('/library/:tab', S.views.library);
  S.router.add('/album/:id', S.views.album);
  S.router.add('/artist/:slug', S.views.artist);
  S.router.add('/playlist/:id', S.views.playlist);
  S.router.add('/requests', S.views.requests);
  S.router.add('/household', S.views.household);
  S.router.add('/notifications', S.views.notifications);
  S.router.add('/account/:tab', S.views.account);
  S.router.add('/account', { render: function () { S.router.replace('/account/profile'); S.app.render('/account/profile'); } });
  // old sign-in link: you're already signed in
  S.router.add('/login', { render: function () { S.router.replace('/home'); S.app.render('/home'); } });

  /* ---------- start ---------- */
  bindShell();
  S.router.start();
})(window.Spin);
