/* Volvix Home — Your Library: albums, artists, songs and playlists on the server. */
window.Spin = window.Spin || {};
Spin.views = Spin.views || {};
(function (S) {
  'use strict';

  const ui = { tab: 'albums', sort: 'added', artists: 'following' };
  const TABS = [['albums', 'Albums'], ['artists', 'Artists'], ['songs', 'Songs'], ['playlists', 'Playlists'], ['browse', 'Browse']];
  const SORTS = [['added', 'Recently added'], ['title', 'Title'], ['artist', 'Artist'], ['year', 'Release year']];

  function sorted() {
    const list = S.data.albums.slice();
    const by = {
      added: function (a, b) { return b.added - a.added; },
      title: function (a, b) { return a.title.localeCompare(b.title); },
      artist: function (a, b) { return a.artist.localeCompare(b.artist) || (a.year || 0) - (b.year || 0); },
      year: function (a, b) { return (b.year || 0) - (a.year || 0); }
    }[ui.sort];
    return list.sort(by);
  }

  function summary() {
    const a = S.data.albums.length, t = Object.keys(S.data.tracks).length;
    return a + ' album' + (a === 1 ? '' : 's') + ' · ' + t + ' song' + (t === 1 ? '' : 's') + ' · ' + S.data.artists.length + ' artists';
  }

  function body() {
    if (ui.tab === 'playlists') {
      const pls = S.pl.all().concat(S.pl.followed());
      return pls.length ? '<div class="grid">' + pls.map(S.playlistCard).join('') + '</div>'
        : S.emptyState('music', 'No playlists yet', 'Make one with the + next to Playlists, or from any song\'s + button.',
          '<button class="btn btn-primary" data-new-pl>' + S.icon('plus', 16) + 'New playlist</button>');
    }
    if (!S.data.albums.length) return S.emptyState('music', 'No music yet', 'Music added to the server shows up here on its own.');
    if (ui.tab === 'browse') return S.browseSections();
    if (ui.tab === 'albums') return '<div class="grid">' + sorted().map(function (a) {
      return S.albumCard(a, { meta: '<span>' + S.artistLink(a.artist, { inert: true }) + (a.year ? ' · ' + a.year : '') + '</span>' });
    }).join('') + '</div>';
    if (ui.tab === 'songs') {
      const ids = [];
      sorted().forEach(function (a) { a.trackIds.forEach(function (id) { ids.push(id); }); });
      return S.trackList(ids.slice(0, 2000), { showCover: true, showAlbum: true, context: { type: 'library', name: 'Your Library' } });
    }
    // artists: the ones you follow (Follow is on each artist's page), or everyone on the server
    const followed = S.store.state.followedArtists.map(function (n) { return n.toLowerCase(); });
    const names = ui.artists === 'all' ? S.data.artists : S.data.artists.filter(function (n) { return followed.indexOf(n.toLowerCase()) !== -1; });
    const chips = '<div class="chips artist-scope">' + [['following', 'Following'], ['all', 'All artists']].map(function (c) {
      return '<button class="chip' + (ui.artists === c[0] ? ' active' : '') + '" data-scope="' + c[0] + '">' + c[1] + '</button>';
    }).join('') + '</div>';
    if (!names.length) {
      return chips + S.emptyState('user', 'You\'re not following any artists yet', 'Open an artist and press Follow. They\'ll be here.',
        '<button class="btn btn-secondary" data-scope="all">Browse all artists</button>');
    }
    return chips + '<div class="rows">' + names.map(function (name) {
      const al = S.data.albumsBy(name);
      const shown = al.length ? al : S.data.appearsOn(name);
      return '<a class="list-row" href="' + S.artistUrl(name) + '">' +
        '<span class="list-art round">' + S.cover(shown[0]) + '</span>' +
        '<span class="list-text"><span class="t-name">' + S.esc(name) + '</span>' +
        '<span class="t-sub">' + (al.length ? al.length + ' album' + (al.length > 1 ? 's' : '') : 'Appears on ' + shown.length) + '</span></span>' +
        S.icon('chevronRight', 18) + '</a>';
    }).join('') + '</div>';
  }

  S.views.library = {
    render: function (root, params) {
      if (params && params.tab && TABS.some(function (t) { return t[0] === params.tab; })) ui.tab = params.tab;
      root.innerHTML =
        '<header class="page-head"><h1 class="page-title">Your Library</h1><p class="page-sub">' + S.esc(summary()) + '</p></header>' +
        '<div class="tabs" role="tablist">' + TABS.map(function (t) {
          return '<button class="tab-btn' + (ui.tab === t[0] ? ' active' : '') + '" role="tab" aria-selected="' + (ui.tab === t[0]) + '" data-tab="' + t[0] + '">' + t[1] + '</button>';
        }).join('') + '</div>' +
        (ui.tab === 'albums' || ui.tab === 'songs' ?
          '<div class="filter-row"><div></div><div class="filter-tools"><label class="sort">Sort <select class="select-sm" data-sort>' + SORTS.map(function (s) {
            return '<option value="' + s[0] + '"' + (ui.sort === s[0] ? ' selected' : '') + '>' + s[1] + '</option>';
          }).join('') + '</select></label></div></div>' : '') +
        '<div data-body>' + body() + '</div>';

      root.onclick = function (e) {
        const tab = e.target.closest('[data-tab]');
        if (tab) { ui.tab = tab.dataset.tab; history.replaceState(null, '', '#/library/' + ui.tab); S.app.refresh(); return; }
        if (e.target.closest('[data-new-pl]')) { S.pl.createDialog(); return; }
        const scope = e.target.closest('[data-scope]');
        if (scope) { ui.artists = scope.dataset.scope; S.$('[data-body]', root).innerHTML = body(); }
      };
      root.onchange = function (e) {
        if (e.target.matches('[data-sort]')) { ui.sort = e.target.value; S.$('[data-body]', root).innerHTML = body(); }
      };
    }
  };
})(window.Spin);
