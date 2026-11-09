/* Volvix Home — your listening numbers: when you first played a song, how many times, and (for the yearly
   Rewind) what you played each year. A play counts after 30 seconds, or half the song if it's shorter.
   Private sessions record nothing. Kept in your own app data, so it follows you to every device. */
window.Spin = window.Spin || {};
(function (S) {
  'use strict';

  let heardId = null, heard = 0, counted = false, pendingSec = 0, lastFlush = 0;

  function root(s) { return s.stats || (s.stats = { tracks: {}, years: {} }); }
  function yearOf(s, y) { const r = root(s); return r.years[y] || (r.years[y] = { plays: {}, sec: 0 }); }
  const now = function () { return Math.floor(Date.now() / 1000); };

  function flush() {
    if (!pendingSec) return;
    const add = pendingSec; pendingSec = 0; lastFlush = Date.now();
    S.store.update(function (s) { yearOf(s, new Date().getFullYear()).sec += add; });
  }

  function record(id) {
    flush();
    S.store.update(function (s) {
      const r = root(s), t = r.tracks[id] || (r.tracks[id] = { first: now(), n: 0 });
      t.n++; t.last = now();
      // a short play history, so recent listening can count more than old listening (mixes use this)
      const log = r.log || (r.log = []);
      log.push([id, now()]);
      if (log.length > 3000) log.splice(0, log.length - 3000);
      const y = yearOf(s, new Date().getFullYear());
      y.plays[id] = (y.plays[id] || 0) + 1;
    });
  }

  S.stats = {
    /** Called a few times a second while a song plays from this device. dt = seconds since the last call. */
    tick: function (track, dt) {
      if (!track || (S.store.state.settings || {}).privateSession) return;
      if (heardId !== track.id) { heardId = track.id; heard = 0; counted = false; }
      heard += dt; pendingSec += dt;
      const need = Math.min(30, (track.duration || 60) / 2);
      if (!counted && heard >= need) { counted = true; record(track.id); }
      else if (counted && track.duration && heard >= track.duration) { heard = 0; counted = false; } // looped: counts again
      if (Date.now() - lastFlush > 30000) flush();
    },
    flush: flush,
    /** { first, last, n } in seconds, or null if you've never played it. */
    get: function (id) { const r = S.store.state.stats; return (r && r.tracks[id]) || null; },
    year: function (y) { const r = S.store.state.stats; return (r && r.years[y]) || null; },
    /** [[trackId, seconds], …] oldest first. */
    log: function () { const r = S.store.state.stats; return (r && r.log) || []; },
    years: function () { const r = S.store.state.stats; return r ? Object.keys(r.years).map(Number).sort() : []; }
  };
  window.addEventListener('pagehide', flush);
  document.addEventListener('visibilitychange', function () { if (document.hidden) flush(); });
})(window.Spin);
