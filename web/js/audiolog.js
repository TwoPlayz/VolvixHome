/* Volvix Home — a short log of what the audio did (play, pause, stalls, speed changes, going to the background), kept on this
   device, for working out why sound stops or glitches. Settings → Playback log shows it. */
window.Spin = window.Spin || {};
(function (S) {
  'use strict';
  const KEY = 'vh_audiolog', MAX = 80;
  let lines = [];
  try { lines = JSON.parse(localStorage.getItem(KEY) || '[]'); } catch (e) { lines = []; }
  const t0 = Date.now();

  function add(what) {
    const d = new Date();
    const clock = d.toTimeString().slice(0, 8);
    const a = S.player && S.player.audio;
    const pos = a ? ' @' + (a.currentTime || 0).toFixed(1) + 's' : '';
    lines.push(clock + ' ' + what + pos);
    if (lines.length > MAX) lines = lines.slice(-MAX);
    try { localStorage.setItem(KEY, JSON.stringify(lines)); } catch (e) { /* full or blocked */ }
  }
  add('— page opened');

  document.addEventListener('visibilitychange', function () { add(document.hidden ? 'page hidden (background)' : 'page visible (foreground)'); });
  window.addEventListener('pagehide', function () { add('pagehide'); });
  window.addEventListener('pageshow', function (e) { add('pageshow' + (e.persisted ? ' (restored)' : '')); });

  function watch() {
    const a = S.player && S.player.audio;
    if (!a) return setTimeout(watch, 500);
    ['play', 'playing', 'pause', 'waiting', 'stalled', 'suspend', 'seeking', 'seeked', 'ended', 'error', 'emptied', 'abort'].forEach(function (ev) {
      a.addEventListener(ev, function () { add('audio ' + ev + (ev === 'error' && a.error ? ' code ' + a.error.code : '')); });
    });
    a.addEventListener('ratechange', function () { add('audio ratechange ' + a.playbackRate); });
    a.addEventListener('loadstart', function () { add('audio loadstart'); });
  }
  watch();

  S.audioLog = {
    text: function () { return lines.join('\n') || 'Nothing logged yet.'; },
    clear: function () { lines = []; try { localStorage.removeItem(KEY); } catch (e) { /* ignore */ } add('— cleared'); }
  };
})(window.Spin);
