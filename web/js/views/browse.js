/* Volvix Home — Browse: your library by genre, decade and label (all read from your files' tags). */
window.Spin = window.Spin || {};
Spin.views = Spin.views || {};
(function (S) {
  'use strict';

  const decadeOf = function (a) { return a.year ? Math.floor(a.year / 10) * 10 + 's' : null; };
  const KINDS = {
    genre: { label: 'Genre', of: function (a) { return a.genre; } },
    decade: { label: 'Decade', of: decadeOf },
    label: { label: 'Label', of: function (a) { return a.label; } }
  };

  /** { value: [albums] } for a kind, biggest first (decades newest first). */
  function groups(kind) {
    const map = {};
    S.data.albums.forEach(function (a) { const v = KINDS[kind].of(a); if (v) (map[v] = map[v] || []).push(a); });
    const keys = Object.keys(map);
    keys.sort(kind === 'decade' ? function (x, y) { return parseInt(y, 10) - parseInt(x, 10); } : function (x, y) { return map[y].length - map[x].length || x.localeCompare(y); });
    return keys.map(function (k) { return { value: k, albums: map[k] }; });
  }

  const PALETTE = ['#E5484D', '#5E548E', '#335C67', '#BC6C25', '#3D5A80', '#4A6C2F', '#7A4069', '#9E2A2B'];
  function tile(kind, g, i) {
    return '<a class="browse-tile" style="background:' + PALETTE[i % PALETTE.length] + '" href="#/browse/' + kind + '/' + encodeURIComponent(g.value) + '">' +
      '<strong>' + S.esc(g.value) + '</strong><span>' + g.albums.length + ' album' + (g.albums.length === 1 ? '' : 's') + '</span></a>';
  }

  /** The Browse tab of Your Library. */
  S.browseSections = function () {
    const out = ['genre', 'decade', 'label'].map(function (k) {
      const g = groups(k);
      if (!g.length) return '';
      return '<section class="section"><div class="section-title"><div><h2>' + KINDS[k].label + (k === 'label' ? 's' : k === 'genre' ? 's' : 's') + '</h2></div></div>' +
        '<div class="browse-grid">' + g.slice(0, k === 'label' ? 24 : 40).map(function (x, i) { return tile(k, x, i); }).join('') + '</div></section>';
    }).join('');
    return out || S.emptyState('library', 'Nothing to browse yet', 'Genres, years and labels come from your files\' tags.');
  };

  S.views.browse = {
    render: function (root, params) {
      const kind = KINDS[params.kind] ? params.kind : 'genre';
      const value = params.value;
      const albums = S.data.albums.filter(function (a) { return KINDS[kind].of(a) === value; }).sort(function (a, b) { return (b.year || 0) - (a.year || 0); });
      if (!albums.length) { root.innerHTML = S.emptyState('library', 'Nothing here', 'No albums with that ' + KINDS[kind].label.toLowerCase() + '.', '<a class="btn btn-secondary" href="#/library/browse">Browse</a>'); return; }
      const ids = [];
      albums.forEach(function (a) { a.trackIds.forEach(function (id) { ids.push(id); }); });
      root.innerHTML =
        '<header class="page-head"><div class="eyebrow">' + KINDS[kind].label + '</div><h1 class="page-title">' + S.esc(value) + '</h1>' +
          '<p class="page-sub">' + albums.length + ' album' + (albums.length === 1 ? '' : 's') + ' · ' + ids.length + ' songs</p></header>' +
        '<div class="profile-actions" style="margin-bottom:16px"><button class="btn btn-primary" data-br-play>' + S.icon('play', 16) + 'Play</button>' +
          '<button class="btn btn-secondary" data-br-shuffle>' + S.icon('shuffle', 16) + 'Shuffle</button></div>' +
        '<div class="grid">' + albums.map(function (a) { return S.albumCard(a, { meta: '<span>' + S.artistLink(a.artist, { inert: true }) + (a.year ? ' · ' + a.year : '') + '</span>' }); }).join('') + '</div>';
      const ctx = { type: 'browse', name: value };
      root.onclick = function (e) {
        if (e.target.closest('[data-br-play]')) S.player.play(ids, 0, ctx);
        else if (e.target.closest('[data-br-shuffle]')) { if (!S.player.state.shuffle) S.player.toggleShuffle(); S.player.play(ids, -1, ctx); }
      };
    }
  };
})(window.Spin);
