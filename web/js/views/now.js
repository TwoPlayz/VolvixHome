/* Volvix Home — Now Playing page (computers): the playing song big on the left, and Up next / Lyrics / Credits on
 * the right, like the Windows app. Phones use their own full-screen Now Playing sheet (nowplaying.js). */
window.Spin = window.Spin || {};
Spin.views = Spin.views || {};
(function (S) {
  'use strict';

  const TABS = [['queue', 'Up next'], ['lyrics', 'Lyrics'], ['credits', 'Credits']];
  let tab = 'queue';
  try { tab = localStorage.getItem('vh_np_tab') || 'queue'; } catch (e) { /* private mode */ }
  let shownTrack = null;

  function side(root) {
    const host = S.$('[data-np-side]', root);
    if (!host) return;
    const t = S.player.current();
    if (S.lyrics && S.lyrics.unmount) S.lyrics.unmount();
    if (tab === 'queue') {
      host.innerHTML = '<div class="q-body np-queue" data-np-queue></div>';
      S.player.renderQueue();
    } else if (tab === 'lyrics') {
      host.innerHTML = '<div class="np-lyrics" data-np-lyrics></div>';
      S.lyrics.mount(S.$('[data-np-lyrics]', host));
    } else {
      host.innerHTML = t ? '<div class="np-credits">' + S.creditsHtml(t.id) + '</div>' : '<p class="q-empty">Nothing playing.</p>';
      if (t) S.creditsFill(host, t.id);
    }
    S.$$('[data-np-tab]', root).forEach(function (b) { b.classList.toggle('active', b.dataset.npTab === tab); b.setAttribute('aria-selected', b.dataset.npTab === tab ? 'true' : 'false'); });
  }

  function render(root) {
    const t = S.player.current();
    shownTrack = t ? t.id : null;
    if (!t) {
      root.innerHTML = '<header class="page-head"><h1 class="page-title">Now playing</h1></header>' +
        S.emptyState('music', 'Nothing playing', 'Pick an album, a playlist or a song to start.', '<a class="btn btn-primary" href="#/home">Go home</a>');
      return;
    }
    const album = S.data.album(t.albumId);
    const inPl = S.pl.editable().some(function (p) { return S.pl.trackIds(p).indexOf(t.id) !== -1; });
    root.innerHTML =
      '<div class="np-page">' +
        '<section class="np-main">' +
          '<div class="np-page-art">' + S.cover(album, '', true) + '</div>' +
          '<h1 class="np-page-title">' + S.albumLink(album, t.title) + (t.explicit ? ' ' + S.explicitBadge() : '') + '</h1>' +
          '<div class="np-page-artist">' + S.artistLink(t.artist) + '</div>' +
          '<div class="np-page-album">' + S.albumLink(album) + (album.year ? ' · ' + album.year : '') + '</div>' +
          '<div class="np-page-actions">' + S.likeButton(t.id, 'np-page-btn', 20) +
            '<button class="add-pl np-page-btn' + (inPl ? ' on' : '') + '" data-add-pl="' + t.id + '" aria-label="Add to playlist" title="Add to playlist">' + S.icon(inPl ? 'check' : 'plus', 20) + '</button></div>' +
        '</section>' +
        '<section class="np-aside">' +
          '<div class="chips np-tabs" role="tablist">' + TABS.map(function (x) {
            return '<button class="chip" role="tab" data-np-tab="' + x[0] + '">' + x[1] + '</button>';
          }).join('') + '</div>' +
          '<div class="np-side" data-np-side></div>' +
        '</section>' +
      '</div>';
    side(root);
    root.onclick = function (e) {
      const b = e.target.closest('[data-np-tab]');
      if (!b) return;
      tab = b.dataset.npTab;
      try { localStorage.setItem('vh_np_tab', tab); } catch (x) { /* private mode */ }
      side(root);
    };
  }

  // a new song: redraw the page (only when it's open)
  S.player.hooks.push(function () {
    if (S.router.current() !== '/now') return;
    const t = S.player.current();
    if ((t ? t.id : null) !== shownTrack) { const v = S.$('#view'); if (v) render(v); }
  });

  S.views.now = { render: function (root, params) { if (params && params.tab) tab = params.tab; render(root); } };
})(window.Spin);
