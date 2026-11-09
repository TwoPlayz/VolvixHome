/* Volvix Home — look inside music you don't have yet: an artist's releases, or an album's songs, from MusicBrainz,
   with a way to get it. Open one from "Get more music" in Search. Anything that's already on the server opens
   its normal page instead. */
window.Spin = window.Spin || {};
Spin.views = Spin.views || {};
(function (S) {
  'use strict';

  const H = function () { return S.getMusic.helpers; };
  const fmt = function (ms) { return ms ? S.fmtTime(Math.round(ms / 1000)) : ''; };
  const loading = function (root, title) { root.innerHTML = '<header class="page-head"><h1 class="page-title">' + S.esc(title) + '</h1><p class="page-sub">Looking it up on MusicBrainz…</p></header>'; };
  const failed = function (root, e) { root.innerHTML = S.emptyState('x', 'Couldn\'t load that', e.message || 'MusicBrainz didn\'t answer. Try again in a moment.', '<a class="btn btn-secondary" href="#/search">Back to search</a>'); };
  const kindOf = function (a) { return a.type === 'Single' ? 'Singles' : 'Albums & EPs'; };
  const tags = function (a) { return [a.year, a.type && a.type !== 'Album' ? a.type : '', ((a.secondary || []).join(', '))].filter(Boolean).join(' · '); };

  /** A card in an artist's release list: click for the songs; the Get button asks Lidarr for it. */
  function releaseCard(a) {
    return '<div class="gm-card"><a class="gm-link" href="' + H().albumHref(a) + '">' + H().art(a.image, false, a.title) +
      '<span class="gm-text"><span class="t-name">' + S.esc(a.title) + '</span><span class="t-sub">' + S.esc(tags(a)) + '</span></span></a>' +
      H().stateButton(a, 'album') + '</div>';
  }

  S.views.discoverArtist = {
    render: function (root, params) {
      loading(root, 'Artist');
      S.server.json('GET', '/api/discover/artist/' + encodeURIComponent(params.id)).then(function (a) {
        if (!document.body.contains(root)) return;
        H().setCan(a.canRequest, a.configured);
        const groups = {};
        a.albums.forEach(function (x) { (groups[kindOf(x)] = groups[kindOf(x)] || []).push(x); });
        const left = a.albums.filter(function (x) { return !x.onServer && !x.requested; }).length;
        const initials = String(a.name).split(/\s+/).map(function (w) { return w[0]; }).slice(0, 2).join('');
        root.innerHTML =
          '<header class="profile-head"><span class="artist-hero">' + S.esc(initials) + '</span>' +
            '<div class="profile-info"><div class="eyebrow">Not in your library yet</div><h1 class="album-title">' + S.esc(a.name) + '</h1>' +
            '<div class="album-meta">' + S.esc([a.type, a.disambiguation, a.country].filter(Boolean).join(' · ')) + (a.albums.length ? ' · ' + a.albums.length + ' releases' : '') + '</div>' +
            '<div class="profile-actions">' +
              (a.onServer ? '<a class="btn btn-secondary" href="' + S.artistUrl(a.onServer) + '">' + S.icon('check', 16) + 'Open in your library</a>' : '') +
              (a.requested ? '<span class="gm-state">' + S.icon('download', 14) + 'Everything requested</span>'
                : !a.canRequest ? H().noGet()
                : left ? '<button class="btn btn-primary" data-get="artist" data-id="' + S.esc(a.foreignId) + '">' + S.icon('download', 16) + 'Get everything (' + left + ')</button>' : '') +
            '</div></div></header>' +
          (a.albums.length ? Object.keys(groups).map(function (k) {
            return '<section class="section"><div class="section-title"><div><h2>' + k + '</h2></div></div><div class="gm-grid">' + groups[k].map(releaseCard).join('') + '</div></section>';
          }).join('') : S.emptyState('music', 'No releases listed', 'MusicBrainz doesn\'t list any albums for this artist yet.')) +
          '<p class="gm-credit">Open a release to see its songs. Lidarr gets whole releases, so a song comes with its album or single.</p>';
      }).catch(function (e) { failed(root, e); });
    }
  };

  S.views.discoverAlbum = {
    render: function (root, params) {
      loading(root, 'Album');
      S.server.json('GET', '/api/discover/album/' + encodeURIComponent(params.id)).then(function (a) {
        if (!document.body.contains(root)) return;
        H().setCan(a.canRequest, a.configured);
        const artistLink = a.onServer ? S.esc(a.artist) : (a.artistId ? '<a href="#/get/artist/' + a.artistId + '">' + S.esc(a.artist) + '</a>' : S.esc(a.artist));
        const multi = a.tracks.some(function (t) { return t.disc > 1; });
        root.innerHTML =
          '<header class="album-head"><div class="album-art gm-bigart">' + H().art(a.image, false, a.title) + '</div>' +
            '<div class="album-info"><div class="eyebrow">' + S.esc(a.type || 'Album') + ' · not in your library yet</div><h1 class="album-title">' + S.esc(a.title) + '</h1>' +
            '<div class="album-meta"><span class="album-artist">' + artistLink + '</span> · ' + S.esc([a.year, a.tracks.length ? a.tracks.length + ' songs' : '', (a.secondary || []).join(', ')].filter(Boolean).join(' · ')) + '</div>' +
            '<div class="profile-actions">' +
              (a.onServer ? '<a class="btn btn-primary" href="#/album/' + a.onServer + '">' + S.icon('check', 16) + 'In your library: open it</a>'
                : a.requested ? '<span class="gm-state">' + S.icon('download', 14) + 'Requested. It shows up when Lidarr is done.</span>'
                : !a.canRequest ? H().noGet()
                : '<button class="btn btn-primary" data-get="album" data-id="' + S.esc(a.foreignId) + '">' + S.icon('download', 16) + 'Get this album</button>') +
            '</div></div></header>' +
          (a.tracks.length
            ? '<div class="gm-tracks">' + a.tracks.map(function (t) {
                return '<div class="gm-track"><span class="gm-no">' + (multi ? t.disc + '.' : '') + S.esc(t.no == null ? '' : t.no) + '</span><span class="gm-tt">' + S.esc(t.title) + '</span><span class="gm-len">' + fmt(t.lengthMs) + '</span></div>';
              }).join('') + '</div>'
            : S.emptyState('music', 'No track list yet', 'MusicBrainz doesn\'t have the songs for this release yet. You can still get the album.')) +
          '<p class="gm-credit">Lidarr gets the whole release, so every song above comes together. Song list from MusicBrainz.</p>';
      }).catch(function (e) { failed(root, e); });
    }
  };
})(window.Spin);
