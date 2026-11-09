/* Volvix Home — the catalog, from your server (boot.js loads it into window.VH before the app starts).
 * Albums, tracks and artists come from /api/library; the signed-in person comes from /api/me.
 * The rest of the app uses the same shapes as Volvix: S.data.album(id), S.data.track(id), album.trackIds… */
window.Spin = window.Spin || {};
(function (S) {
  'use strict';

  const VH = window.VH || { user: { id: 'u_me', name: 'Listener', username: 'me' }, catalog: { albums: [], tracks: [], artists: [] } };

  /* ---------- generated art (shown under real cover art, and when an album has none) ---------- */
  const PALETTES = [
    ['#1F3A4D', '#E8C07D', '#F4EFE6'], ['#E9E4DA', '#C2462F', '#1D1B19'], ['#2C1E3A', '#D9775E', '#F6EDE6'],
    ['#4A5A3A', '#E7E2D2', '#F3F0E6'], ['#B23A2E', '#1A1A1A', '#F5EDE3'], ['#D9A441', '#5A3A1E', '#2A1B0E'],
    ['#DCE3EA', '#5B7086', '#1F2A36'], ['#3B3F5C', '#C9A227', '#F2EEDF'], ['#7A2F3B', '#E9B9A6', '#FBEFEA'],
    ['#0F2A3D', '#4FA3A5', '#E6F2F2'], ['#5C4B8A', '#F0C674', '#F7F2FF'], ['#283618', '#DDA15E', '#FEFAE0']
  ];
  const VARIANTS = ['circle', 'band', 'split', 'rings', 'plain'];
  function hash(s) { let h = 0; for (let i = 0; i < s.length; i++) h = (h * 31 + s.charCodeAt(i)) >>> 0; return h; }
  function artFor(id) {
    const h = hash(id), p = PALETTES[h % PALETTES.length];
    return { c1: p[0], c2: p[1], fg: p[2], variant: VARIANTS[(h >> 4) % VARIANTS.length] };
  }

  /* ---------- catalog ---------- */
  const TRACKS = {};
  (VH.catalog.tracks || []).forEach(function (t) {
    TRACKS[t.id] = { id: t.id, title: t.title, artist: t.artist, albumId: t.albumId, no: t.no || 0, disc: t.disc || 1, duration: t.duration || 0, explicit: false, credits: t.credits || [] };
  });
  const ALBUMS = (VH.catalog.albums || []).map(function (a) {
    const q = a.quality || {};
    return {
      id: a.id, title: a.title, artist: a.artist, year: a.year || null, genre: a.genre || null,
      label: a.label || null, copyright: a.copyright || null, catalogNumber: a.catalogNumber || null,
      trackIds: (a.trackIds || []).filter(function (id) { return TRACKS[id]; }),
      cover: !!a.cover, added: a.added || 0, art: artFor(a.id),
      format: 'digital',
      // e.g. "24/96" for the quality badge; lossy files report their bitrate instead
      maxQuality: q.lossless && q.bits ? q.bits + '/' + (q.khz || 44.1) : null,
      quality: q
    };
  });
  const byId = {};
  ALBUMS.forEach(function (a) { byId[a.id] = a; });

  const ME = { id: 'u_me', serverId: VH.user.id, name: VH.user.name, username: VH.user.username, admin: !!VH.user.admin, pfp: null };
  // everyone in the household (server ids); you are also 'u_me' inside playlists and the UI
  const PEOPLE = {};
  (VH.people || []).forEach(function (p) { PEOPLE[p.id] = { id: p.id, name: p.name, admin: !!p.admin }; });

  S.data = {
    albums: ALBUMS,
    tracks: TRACKS,
    artists: (VH.catalog.artists || []).map(function (r) { return r.name; }),
    me: ME,
    people: PEOPLE,
    /** Everyone except you. */
    others: function () { return Object.keys(PEOPLE).filter(function (id) { return id !== ME.serverId; }).map(function (id) { return PEOPLE[id]; }); },
    album: function (id) { return byId[id] || null; },
    track: function (id) { return TRACKS[id] || null; },
    person: function (id) { return id === 'u_me' || id === ME.serverId ? ME : (PEOPLE[id] || { id: id, name: 'Someone' }); },
    single: function () { return null; },
    singlesOf: function () { return []; },
    albumsBy: function (artist) {
      const n = artist.toLowerCase();
      return ALBUMS.filter(function (a) { return a.artist.toLowerCase() === n; });
    },
    /** Tracks by an artist that sit on other artists' albums (features, compilations). */
    appearsOn: function (artist) {
      const n = artist.toLowerCase();
      return ALBUMS.filter(function (a) { return a.artist.toLowerCase() !== n && a.trackIds.some(function (id) { return TRACKS[id].artist.toLowerCase() === n; }); });
    },
    FORMATS: { digital: '' },
    seedPlaylists: [],
    friendPlaylists: []
  };

  /* ---------- everything on your server is yours ---------- */
  function qualityOf(album) {
    const q = (album && album.quality) || {};
    if (q.lossless) {
      const hires = (q.bits || 16) >= 24 || (q.khz || 44.1) > 48;
      return { id: hires ? 'hires' : 'lossless', label: hires ? 'Hi-Res Lossless' : 'Lossless', hires: hires, lossless: true,
        detail: (q.bits ? q.bits + '-bit / ' : '') + (q.khz ? q.khz + ' kHz ' : '') + (q.codec || '') };
    }
    const kbps = q.kbps || 0;
    return { id: kbps >= 256 ? 'high' : kbps >= 128 ? 'normal' : 'low', label: kbps >= 256 ? 'High Quality' : kbps >= 128 ? 'Normal' : 'Low',
      hires: false, lossless: false, detail: (q.codec || 'Audio') + (kbps ? ' ' + kbps + ' kbps' : '') };
  }
  S.lib = {
    status: function (id) { return byId[id] ? 'owned' : null; },
    info: function (id) { return byId[id] ? { status: 'owned' } : null; },
    isTrackOwned: function (id) { return !!TRACKS[id]; },
    isPlayable: function (id) { return !!TRACKS[id]; },
    ownedTrackCount: function (album) { return album.trackIds.length; },
    ownedAlbums: function () { return ALBUMS.slice(); },
    lockedAlbums: function () { return []; },
    quality: qualityOf,
    streamQuality: qualityOf,
    /** "2024 · Hi-Res Lossless" */
    ownershipLine: function (album) { return [album.year, qualityOf(album).label].filter(Boolean).join(' · '); },
    totalDuration: function (ids) { return ids.reduce(function (s, id) { return s + ((TRACKS[id] && TRACKS[id].duration) || 0); }, 0); }
  };
  /** 1 h 4 min / 38 min */
  S.fmtLong = function (sec) {
    const m = Math.round(sec / 60);
    return m >= 60 ? Math.floor(m / 60) + ' h ' + (m % 60) + ' min' : m + ' min';
  };

  /* ---------- talking to the server ---------- */
  S.server = {
    url: function (path) { return path; },
    streamUrl: function (trackId) { return '/api/stream/' + encodeURIComponent(trackId); },
    coverUrl: function (albumId) { return '/api/cover/' + encodeURIComponent(albumId); },
    json: function (method, path, body) {
      return fetch(path, {
        method: method, credentials: 'same-origin',
        headers: body !== undefined ? { 'Content-Type': 'application/json' } : {},
        body: body !== undefined ? JSON.stringify(body) : undefined
      }).then(function (r) {
        if (r.status === 401) { location.reload(); throw new Error('signed out'); }
        if (r.status === 204) return null;
        return r.json().then(function (j) { if (!r.ok) throw new Error((j && j.error) || 'Something went wrong'); return j; });
      });
    },
    logout: function () {
      return fetch('/api/logout', { method: 'POST', credentials: 'same-origin' }).finally(function () { location.hash = ''; location.reload(); });
    }
  };
})(window.Spin);
