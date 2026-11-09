/* Volvix Home — sound effects: crossfade, volume leveling (ReplayGain) and the equalizer, all in the browser
   with the Web Audio API. Nothing is processed unless one of them is switched on: with everything off the music
   plays straight from the audio element, exactly as before. (Once a feature has been used, sound keeps going
   through this chain until the page is reloaded; the web can't undo that.)
   The chain:  song ─ gain ─┐
                            ├─ preamp ─ 10-band EQ ─ speakers
   outgoing song ─ gain ────┘          (the outgoing song is a second element, only alive during a crossfade) */
window.Spin = window.Spin || {};
(function (S) {
  'use strict';

  const BANDS = [32, 64, 125, 250, 500, 1000, 2000, 4000, 8000, 16000];
  const PRESETS = {
    flat: { label: 'Flat', pre: 0, gains: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0] },
    bass: { label: 'Bass boost', pre: -3, gains: [6, 5, 4, 2, 0, 0, 0, 0, 0, 0] },
    treble: { label: 'Treble boost', pre: -3, gains: [0, 0, 0, 0, 0, 1, 2, 4, 5, 6] },
    vocal: { label: 'Vocal', pre: -2, gains: [-2, -2, -1, 1, 3, 4, 3, 1, 0, -1] },
    rock: { label: 'Rock', pre: -3, gains: [5, 4, 2, -1, -2, -1, 2, 4, 5, 5] },
    pop: { label: 'Pop', pre: -2, gains: [-1, 1, 3, 4, 3, 0, -1, -1, 1, 2] },
    jazz: { label: 'Jazz', pre: -2, gains: [3, 2, 1, 2, -2, -2, 0, 1, 2, 3] },
    classical: { label: 'Classical', pre: -2, gains: [4, 3, 2, 1, -1, -1, 0, 2, 3, 4] },
    electronic: { label: 'Electronic', pre: -3, gains: [5, 4, 1, 0, -2, 2, 1, 1, 4, 5] },
    loudness: { label: 'Loudness', pre: -4, gains: [6, 4, 0, 0, -1, 0, -1, 0, 3, 5] },
    late: { label: 'Late night (quieter highs and lows)', pre: 0, gains: [-3, -2, 0, 1, 1, 1, 0, -1, -3, -4] }
  };

  let ctx = null, nodes = null;
  const settings = function () { return S.store.state.settings || {}; };
  const dbToGain = function (db) { return Math.pow(10, db / 20); };

  function needed() {
    const s = settings();
    return !!(s.eq && s.eq.on) || Number(s.crossfade) > 0 || (s.leveling && s.leveling !== 'off');
  }

  /** Build the audio chain (once). Returns false if this browser can't. */
  function ensure() {
    if (nodes) return true;
    const AC = window.AudioContext || window.webkitAudioContext;
    if (!AC || !S.player || !S.player.audio) return false;
    try {
      ctx = new AC();
      const tail = new Audio();
      tail.preload = 'auto'; tail.crossOrigin = 'anonymous';
      const mainSrc = ctx.createMediaElementSource(S.player.audio);
      const tailSrc = ctx.createMediaElementSource(tail);
      const mainGain = ctx.createGain(), tailGain = ctx.createGain(), pre = ctx.createGain();
      tailGain.gain.value = 0;
      const filters = BANDS.map(function (f) {
        const b = ctx.createBiquadFilter();
        b.type = 'peaking'; b.frequency.value = f; b.Q.value = 1.1; b.gain.value = 0;
        return b;
      });
      mainSrc.connect(mainGain); tailSrc.connect(tailGain);
      mainGain.connect(pre); tailGain.connect(pre);
      let last = pre;
      filters.forEach(function (b) { last.connect(b); last = b; });
      last.connect(ctx.destination);
      nodes = { tail: tail, mainGain: mainGain, tailGain: tailGain, pre: pre, filters: filters, level: 1 };
      sync();
      return true;
    } catch (e) { ctx = null; nodes = null; return false; }
  }

  /** Make sure sound can come out: browsers keep a new audio context paused until the person has tapped something. */
  function wake() {
    if (needed()) ensure();
    if (ctx && ctx.state === 'suspended') ctx.resume().catch(function () { /* waits for a tap */ });
  }
  ['pointerdown', 'keydown', 'touchend'].forEach(function (ev) { document.addEventListener(ev, function () { if (ctx && ctx.state === 'suspended') ctx.resume().catch(function () {}); }, { passive: true }); });

  /** Loudness correction for a song, in dB (0 when its file has no ReplayGain tags). */
  function levelDb(track) {
    const s = settings();
    if (!track || !track.replayGain || !s.leveling || s.leveling === 'off') return 0;
    const rg = track.replayGain;
    let db = s.leveling === 'album' ? (rg.album != null ? rg.album : rg.track) : (rg.track != null ? rg.track : rg.album);
    if (db == null) return 0;
    // never boost past full scale: the peak tells us how much room there is
    if (rg.peak && db > 0) db = Math.min(db, -20 * Math.log10(rg.peak));
    return Math.max(-20, Math.min(12, db));
  }

  /** Apply the EQ and this song's leveling to the chain. */
  function sync() {
    if (!nodes) { if (needed()) ensure(); return; }
    const s = settings(), eq = s.eq || {};
    const on = !!eq.on, t = S.player.current();
    const now = ctx.currentTime;
    nodes.filters.forEach(function (b, i) { b.gain.setTargetAtTime(on ? Number((eq.gains || [])[i]) || 0 : 0, now, 0.02); });
    nodes.pre.gain.setTargetAtTime(on ? dbToGain(Number(eq.pre) || 0) : 1, now, 0.02);
    nodes.level = dbToGain(levelDb(t));
    if (!nodes.fading) nodes.mainGain.gain.setTargetAtTime(nodes.level, now, 0.05);
  }

  /**
   * Crossfade: the song that's ending carries on in a second element and fades out while `startNext` brings the
   * next song in on the main one (fading in). Returns false when the chain isn't available (then nothing happens).
   */
  function crossfade(seconds, startNext) {
    if (!ensure()) return false;
    wake();
    const a = S.player.audio, n = nodes, now = ctx.currentTime;
    const fromLevel = n.level;
    n.tail.src = a.currentSrc || a.src;
    n.tail.currentTime = a.currentTime;
    n.tail.volume = a.volume;
    n.tail.play().catch(function () {});
    n.tailGain.gain.cancelScheduledValues(now);
    n.tailGain.gain.setValueAtTime(fromLevel, now);
    n.tailGain.gain.linearRampToValueAtTime(0, now + seconds);
    setTimeout(function () { try { n.tail.pause(); n.tail.removeAttribute('src'); n.tail.load(); } catch (e) { /* ignore */ } }, seconds * 1000 + 300);
    // the next song comes in from silence
    n.fading = true;
    n.mainGain.gain.cancelScheduledValues(now);
    n.mainGain.gain.setValueAtTime(0, now);
    startNext();
    const t2 = ctx.currentTime;
    n.level = dbToGain(levelDb(S.player.current()));
    n.mainGain.gain.cancelScheduledValues(t2);
    n.mainGain.gain.setValueAtTime(0, t2);
    n.mainGain.gain.linearRampToValueAtTime(n.level, t2 + seconds);
    setTimeout(function () { n.fading = false; sync(); }, seconds * 1000 + 100);
    return true;
  }

  S.fx = {
    BANDS: BANDS, PRESETS: PRESETS,
    needed: needed, wake: wake, sync: sync, crossfade: crossfade,
    available: function () { return !!(window.AudioContext || window.webkitAudioContext); },
    active: function () { return !!nodes; },
    /** For tests: the audio context and nodes. */
    debug: function () { return { ctx: ctx, nodes: nodes }; }
  };
  S.player.hooks.push(sync); // a new song: its own loudness correction
})(window.Spin);
