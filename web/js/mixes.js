/* Volvix Home — "Made for you": mixes built from what you play and what's on the server. Nothing leaves the server:
   it only looks at your own play history (stats.js) and your files' tags (genre, year, artist). Mixes are worked out
   when you open them and stay the same for the day, so a mix doesn't reshuffle under your finger. */
window.Spin = window.Spin || {};
(function (S) {
  'use strict';

  const DAY = 86400;
  const now = function () { return Math.floor(Date.now() / 1000); };

  function rng(seed) { // small seeded random: same day, same mix
    let a = seed >>> 0;
    return function () { a |= 0; a = a + 0x6D2B79F5 | 0; let t = Math.imul(a ^ a >>> 15, 1 | a); t = t + Math.imul(t ^ t >>> 7, 61 | t) ^ t; return ((t ^ t >>> 14) >>> 0) / 4294967296; };
  }
  function hash(s) { let h = 2166136261; for (let i = 0; i < s.length; i++) { h ^= s.charCodeAt(i); h = Math.imul(h, 16777619); } return h >>> 0; }
  const today = function () { return Math.floor(Date.now() / 86400000); };

  const playable = function (id) { return S.lib.isPlayable ? S.lib.isPlayable(id) : true; };
  const info = function (id) { const t = S.data.track(id); return t ? { t: t, a: S.data.album(t.albumId) } : null; };
  const decade = function (a) { return a && a.year ? Math.floor(a.year / 10) * 10 : 0; };

  /** What the person likes: weights per artist / genre / decade, with recent plays counting more (30-day half-life). */
  function profile() {
    const w = { artist: {}, genre: {}, decade: {}, plays: {}, last: {}, total: 0 };
    const t0 = now();
    S.stats.log().forEach(function (e) {
      const x = info(e[0]); if (!x) return;
      const weight = Math.pow(0.5, (t0 - e[1]) / (30 * DAY));
      w.total++;
      w.plays[e[0]] = (w.plays[e[0]] || 0) + 1;
      w.last[e[0]] = Math.max(w.last[e[0]] || 0, e[1]);
      const ar = x.t.artist.toLowerCase();
      w.artist[ar] = (w.artist[ar] || 0) + weight;
      if (x.a && x.a.genre) w.genre[x.a.genre] = (w.genre[x.a.genre] || 0) + weight;
      if (x.a && x.a.year) w.decade[decade(x.a)] = (w.decade[decade(x.a)] || 0) + weight;
    });
    // plays recorded before the log existed still count (all-time totals)
    const all = (S.store.state.stats && S.store.state.stats.tracks) || {};
    Object.keys(all).forEach(function (id) { if (!w.plays[id]) { w.plays[id] = all[id].n; w.last[id] = all[id].last || 0; } });
    return w;
  }
  const top = function (obj, n) { return Object.keys(obj).sort(function (a, b) { return obj[b] - obj[a]; }).slice(0, n); };
  const norm = function (obj) { const m = Math.max.apply(null, Object.keys(obj).map(function (k) { return obj[k]; }).concat([0.0001])); return function (k) { return (obj[k] || 0) / m; }; };

  /** Pick songs, no more than `per` from one artist, so a mix isn't one band. */
  function pick(ids, n, per) {
    const seen = {}, out = [];
    ids.forEach(function (id) {
      if (out.length >= n) return;
      const x = info(id); if (!x) return;
      const k = x.t.artist.toLowerCase();
      if ((seen[k] || 0) >= per) return;
      seen[k] = (seen[k] || 0) + 1; out.push(id);
    });
    return out;
  }

  function mixes() {
    const P = profile(), t0 = now(), all = Object.keys(S.data.tracks).filter(playable);
    const rand = rng(hash(S.data.me.serverId) + today());
    const mk = function (id, title, sub, ids, icon) { return ids.length >= 3 ? { id: id, title: title, sub: sub, ids: ids, icon: icon || 'music' } : null; };
    const list = [];
    const aff = {
      artist: norm(P.artist), genre: norm(P.genre), decade: norm(P.decade)
    };
    const score = function (id) {
      const x = info(id); if (!x) return 0;
      return 3 * aff.artist(x.t.artist.toLowerCase()) + 2 * (x.a && x.a.genre ? aff.genre(x.a.genre) : 0) + (x.a ? aff.decade(decade(x.a)) : 0);
    };
    const shuffle = function (a) { a = a.slice(); for (let i = a.length - 1; i > 0; i--) { const j = Math.floor(rand() * (i + 1)); const t = a[i]; a[i] = a[j]; a[j] = t; } return a; };

    if (P.total >= 3) {
      // your last 30 days, most played first
      const recent = {};
      S.stats.log().forEach(function (e) { if (t0 - e[1] < 30 * DAY && S.data.track(e[0])) recent[e[0]] = (recent[e[0]] || 0) + 1; });
      list.push(mk('repeat', 'On repeat', 'What you\'ve played most in the last 30 days', top(recent, 40).filter(playable).filter(function (id) { return recent[id] >= 2; }), 'repeat'));
      // the songs you keep coming back to
      const everything = {};
      Object.keys(P.plays).forEach(function (id) { if (S.data.track(id)) everything[id] = P.plays[id]; });
      list.push(mk('top', 'Your top songs', 'All-time favourites on this server', top(everything, 50).filter(playable), 'heart'));
      // loved once, forgotten since
      const forgotten = Object.keys(P.plays).filter(function (id) { return P.plays[id] >= 3 && t0 - (P.last[id] || 0) > 60 * DAY && S.data.track(id) && playable(id); });
      list.push(mk('rediscover', 'Rediscover', 'Songs you used to play a lot and haven\'t for a while', shuffle(forgotten).slice(0, 40), 'history'));
      // artist mixes: your top artists, plus music in the same genres from other artists
      top(P.artist, 3).forEach(function (ar) {
        const own = all.filter(function (id) { const x = info(id); return x.t.artist.toLowerCase() === ar; });
        const genres = {}; own.forEach(function (id) { const x = info(id); if (x.a && x.a.genre) genres[x.a.genre] = 1; });
        const like = all.filter(function (id) { const x = info(id); return x.t.artist.toLowerCase() !== ar && x.a && genres[x.a.genre]; });
        const name = info(own[0]) ? info(own[0]).t.artist : ar;
        const ids = pick(shuffle(own).slice(0, 14).concat(shuffle(like)), 30, 4);
        list.push(mk('more-' + hash(ar).toString(36), name + ' & more', 'Mixed with similar music from your library', ids, 'user'));
      });
      // new things that fit your taste
      const addedCut = t0 - 90 * DAY;
      const fresh = all.filter(function (id) { const x = info(id); return x.a && x.a.added > addedCut && !P.plays[id]; });
      list.push(mk('new', 'New for you', 'Recently added music, in the genres you play', pick(fresh.sort(function (a, b) { return score(b) - score(a); }), 30, 4), 'plus'));
      // never played, ranked by how much you'd probably like it
      const unplayed = all.filter(function (id) { return !P.plays[id]; });
      list.push(mk('discover', 'Discover', 'Songs you haven\'t played yet that fit your taste', pick(unplayed.sort(function (a, b) { return score(b) + rand() * 0.6 - score(a) - rand() * 0.6; }), 40, 3), 'search'));
    } else {
      // nothing played yet: a taste of what's here
      list.push(mk('start', 'Start here', 'A mix of what\'s on the server. Play a few songs and this gets personal.', pick(shuffle(all), 30, 3), 'music'));
    }
    return list.filter(Boolean);
  }

  S.mixes = {
    all: mixes,
    get: function (id) { return mixes().find(function (m) { return m.id === id; }) || null; },
    /** Albums by how much you've played them (for Home). */
    topAlbums: function (n) {
      const plays = {}, all = (S.store.state.stats && S.store.state.stats.tracks) || {};
      Object.keys(all).forEach(function (id) { const t = S.data.track(id); if (t) plays[t.albumId] = (plays[t.albumId] || 0) + all[id].n; });
      return Object.keys(plays).sort(function (a, b) { return plays[b] - plays[a]; }).slice(0, n).map(S.data.album).filter(Boolean);
    },
    /** Albums picked at random for today (the same picks all day). */
    randomAlbums: function (n) {
      const r = rng(hash('r' + S.data.me.serverId) + today()), a = S.data.albums.slice();
      for (let i = a.length - 1; i > 0; i--) { const j = Math.floor(r() * (i + 1)); const t = a[i]; a[i] = a[j]; a[j] = t; }
      return a.slice(0, n);
    }
  };
})(window.Spin);
