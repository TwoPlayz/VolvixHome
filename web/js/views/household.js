/* Volvix Home — Household: what everyone else is listening to, and their public playlists. */
window.Spin = window.Spin || {};
Spin.views = Spin.views || {};
(function (S) {
  'use strict';

  function songLink(n) {
    const al = S.data.album(n.albumId);
    return (al ? S.albumLink(al, n.title) : S.esc(n.title)) + '<span class="hh-by"> · ' + (S.data.artists.indexOf(n.artist) !== -1 ? S.artistLink(n.artist) : S.esc(n.artist)) + '</span>';
  }

  function nowLine(m) {
    const n = m.now;
    if (!n) return '<span class="hh-quiet">Nothing shared yet</span>';
    if (n.listening) return '<span class="eq eq-static" aria-label="Listening now"><i></i><i></i><i></i></span><span class="hh-song">' + songLink(n) + '</span>';
    return '<span class="hh-quiet">Last played ' + S.ago(n.at) + '</span><span class="hh-song">' + songLink(n) + '</span>';
  }

  function memberCard(m) {
    const live = !!(m.now && m.now.listening);
    return '<div class="hh-card' + (live ? ' live' : '') + '">' + S.avatar({ id: m.id, name: m.name }, 44) +
      '<div class="hh-text"><strong>' + S.esc(m.name) + '</strong><div class="hh-now">' + nowLine(m) + '</div></div>' +
      (m.now && S.data.track(m.now.trackId)
        ? '<button class="icon-btn" data-hh-play="' + S.esc(m.now.trackId) + '" aria-label="Play ' + S.esc(m.now.title) + '" title="Play this song">' + S.icon('play', 18) + '</button>' : '') +
      '</div>';
  }

  function playlistsFor(m) {
    const pls = m.playlists.map(function (p) { return S.household.playlist(p.id); }).filter(Boolean);
    if (!pls.length) return '';
    return S.shelf(m.name.split(' ')[0] + '\'s playlists', pls.map(S.playlistCard).join(''));
  }

  S.views.household = {
    render: function (root) {
      root.innerHTML = '<header class="page-head"><h1 class="page-title">Household</h1>' +
        '<p class="page-sub">What everyone is listening to, and the playlists they\'ve made public.</p></header>' +
        '<div data-hh></div>';
      const host = S.$('[data-hh]', root);

      function draw() {
        const members = S.household.members();
        if (!members.length) {
          host.innerHTML = S.emptyState('family', 'Just you so far', S.data.can('members')
            ? 'Add the people you share this server with under Account → People, and they show up here.'
            : 'When more people join this server, you\'ll see what they\'re listening to here.',
            S.data.can('members') ? '<a class="btn btn-secondary" href="#/account/members">Add members</a>' : '');
          return;
        }
        // listening now first, then most recently active
        const sorted = members.slice().sort(function (a, b) {
          const la = a.now && a.now.listening ? 1 : 0, lb = b.now && b.now.listening ? 1 : 0;
          return lb - la || ((b.now && b.now.at) || 0) - ((a.now && a.now.at) || 0) || a.name.localeCompare(b.name);
        });
        const withLists = sorted.filter(function (m) { return m.playlists.length; });
        host.innerHTML =
          '<section class="section"><div class="section-title"><div><h2>Listening</h2></div></div><div class="hh-grid">' + sorted.map(memberCard).join('') + '</div></section>' +
          withLists.map(playlistsFor).join('') +
          (!withLists.length ? '<p class="list-note">' + S.icon('lock', 12) + ' Nobody has made a playlist public yet. In a playlist\'s Edit details, turn on "Visible to the household".</p>' : '');
      }
      draw();
      // fresh data now, then every 10 s while this page is open
      const refresh = function () { S.household.load().then(function () { if (document.body.contains(host)) draw(); }).catch(function () {}); };
      refresh();
      const timer = setInterval(function () { if (!document.body.contains(host)) clearInterval(timer); else if (!document.hidden) refresh(); }, 10000);

      root.onclick = function (e) {
        const play = e.target.closest('[data-hh-play]');
        if (play) {
          const id = play.dataset.hhPlay, t = S.data.track(id);
          S.player.play([id], 0, { type: 'song', id: id, name: t ? t.title : 'Song' });
        }
      };
    }
  };
})(window.Spin);
