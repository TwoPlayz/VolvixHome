/* Volvix Home — playing on TVs and speakers.
 *  Cast (Chromecast and other Google Cast TVs / speakers): the SERVER talks to the device (see server/src/castdev.rs),
 *  so it works from any address, in any browser, and inside the Windows and iPhone apps. The server finds the devices
 *  on the network; the device fetches each song from the server itself, with a short-lived signed link.
 *  While casting, this player keeps the queue and the clock but stays silent, and follows the TV's play / pause.
 *  AirPlay: Safari and the iPhone app: the system's own speaker picker. */
window.Spin = window.Spin || {};
(function (S) {
  'use strict';

  const C = S.cast = { active: false, name: '', device: null, devices: [] };
  // sentPlaying: playing / paused as last told to the TV (so a change on the TV's own remote can be told apart)
  let loadedId = null, following = false, poll = null, loading = false, sentPlaying = null, sentVolume = null;
  // right after a command the TV still reports its old state for a moment: don't follow it back
  let quietUntil = 0;
  const hush = function () { quietUntil = Date.now() + 3000; };

  C.supported = function () { return true; };
  C.airplay = function () { return !!window.WebKitPlaybackTargetAvailabilityEvent && typeof S.player.audio.webkitShowPlaybackTargetPicker === 'function'; };

  /** The TVs and speakers the server can see (refreshed in the background, so the menu opens instantly). */
  C.refresh = function () {
    return S.server.json('GET', '/api/cast/devices').then(function (r) { C.devices = r.devices || []; return C.devices; }).catch(function () { return C.devices; });
  };
  C.refresh();
  setInterval(function () { if (document.visibilityState === 'visible') C.refresh(); }, 30000);

  const api = function (path, body) { return S.server.json('POST', path, body); };

  /** Start casting to a device: what's playing moves there, at the same spot. */
  C.start = function (dev) {
    if (S.party && S.party.active && S.party.output === 'host') { S.toast('This device is silent in the party. Switch to "This device" first, then cast', 'x'); return; }
    const t = S.player.current();
    if (!t) { S.toast('Start a song first, then cast it', 'devices'); return; }
    C.active = true; C.device = dev.id; C.name = dev.name;
    loadedId = null;
    sentVolume = S.player.state.muted ? 0 : S.player.state.volume; // the TV keeps its own volume until you change it here
    S.player.silent = true; S.player.render();
    S.toast('Connecting to ' + dev.name + '…', 'devices');
    loadTrack(t, S.player.state.pos, true);
    clearInterval(poll);
    poll = setInterval(follow, 1500);
  };

  function finish(message) {
    if (!C.active) return;
    clearInterval(poll); poll = null;
    C.active = false; C.device = null; loadedId = null;
    S.player.silent = false; S.player.render();
    S.toast(message || 'Stopped casting', 'devices');
  }

  C.stop = function () {
    const dev = C.device;
    finish();
    if (dev) api('/api/cast/control', { device: dev, action: 'stop' }).catch(function () {});
  };

  function loadTrack(t, pos, first) {
    loadedId = t.id; loading = true; sentPlaying = !!S.player.state.playing; hush();
    api('/api/cast/play', { device: C.device, track: t.id, at: Math.max(0, pos || 0), autoplay: !!S.player.state.playing, origin: location.origin })
      .then(function () { loading = false; if (first) S.toast('Casting to ' + C.name, 'devices'); })
      .catch(function (e) {
        loading = false; loadedId = null;
        if (first) finish(e.message || 'Couldn\'t cast to ' + C.name); else S.toast(e.message || 'The TV couldn\'t play that song', 'x');
      });
  }

  /** Keep the TV doing what this player says: song, play / pause, volume. */
  function sync() {
    if (!C.active || loading) return;
    const t = S.player.current(), st = S.player.state;
    if (!t) { C.stop(); return; }
    if (t.id !== loadedId) { loadTrack(t, st.pos); return; }
    if (sentPlaying !== !!st.playing) {
      // played / paused here: tell the TV
      sentPlaying = !!st.playing; hush();
      api('/api/cast/control', { device: C.device, action: st.playing ? 'play' : 'pause' }).catch(function () {});
    }
    const v = st.muted ? 0 : st.volume;
    if (sentVolume === null || Math.abs(v - sentVolume) > 0.02) {
      sentVolume = v;
      clearTimeout(volTimer);
      volTimer = setTimeout(function () { api('/api/cast/control', { device: C.device, action: 'volume', value: v }).catch(function () {}); }, 150);
    }
  }
  let volTimer = null;
  S.player.hooks.push(sync);
  S.player.seekHooks.push(function (sec) {
    if (!C.active || following || loading) return;
    hush();
    api('/api/cast/control', { device: C.device, action: 'seek', value: sec }).catch(function () {});
  });

  /** The TV is the clock: follow its position, and its play / pause from its own remote or the Google Home app. */
  function follow() {
    if (!C.active || loading) return;
    S.server.json('GET', '/api/cast/status?device=' + encodeURIComponent(C.device)).then(function (r) {
      const s = r.status || {};
      if (!C.active || loading) return;
      if (!s.connected) { finish('Lost the connection to ' + C.name); return; }
      if (s.playerState === 'IDLE' && s.idleReason === 'CANCELLED') { finish(C.name + ' is playing something else now'); return; }
      if (s.playerState === 'IDLE' && s.idleReason === 'ERROR') { S.toast(C.name + ' couldn\'t play that song', 'x'); return; }
      const st = S.player.state;
      if (Date.now() < quietUntil) return;
      // in a party the party's clock is the clock: the TV is kept to it (not the other way round)
      if (S.party && S.party.active) {
        if ((s.playerState === 'PLAYING' || s.playerState === 'PAUSED') && Math.abs(s.currentTime - st.pos) > 2.5) {
          hush(); api('/api/cast/control', { device: C.device, action: 'seek', value: st.pos }).catch(function () {});
        }
        return;
      }
      if (s.playerState === 'PLAYING' || s.playerState === 'PAUSED') {
        const remotePlaying = s.playerState === 'PLAYING';
        // changed on the TV itself (its remote, the Google Home app): this player follows
        if (remotePlaying !== sentPlaying) { sentPlaying = remotePlaying; if (!!st.playing !== remotePlaying) S.player.toggle(); }
        if (Math.abs((S.player.audio.currentTime || 0) - s.currentTime) > 2) {
          following = true; try { S.player.seekTo(s.currentTime); } finally { following = false; }
        }
      }
    }).catch(function () { /* a missed poll is fine */ });
  }

  /** Add a TV by its address (when the server doesn't find it by itself, e.g. it runs in Docker). */
  C.addByAddress = function () {
    S.modal({
      title: 'Add a TV or speaker',
      body: '<p class="modal-text">Type its address on your network. You can find it in the Google Home app: the device → Settings → Device information.</p>' +
        '<label class="field"><span class="label">Address</span><input class="input" data-cast-host placeholder="192.168.1.40" inputmode="decimal" autocomplete="off"></label>',
      actions: [{ label: 'Cancel', kind: 'secondary' }, { label: 'Add', kind: 'primary', onClick: function (close) {
        const input = document.querySelector('[data-cast-host]');
        const host = input ? input.value.trim() : '';
        if (!host) return;
        api('/api/cast/devices', { host: host }).then(function (r) {
          close(); C.refresh();
          S.toast((r.device && r.device.name || 'That device') + ' added', 'check');
        }).catch(function (e) { S.toast(e.message || 'Couldn\'t add it', 'x'); });
      } }]
    });
  };

  // AirPlay speakers can't fetch a downloaded copy (it lives inside the app), so while AirPlay is on, play from the server
  S.player.audio.addEventListener('webkitcurrentplaybacktargetiswirelesschanged', function () {
    const wireless = !!S.player.audio.webkitCurrentPlaybackTargetIsWireless;
    if (S.player.streamOnly === wireless) return;
    S.player.streamOnly = wireless;
    if (S.dl && S.player.current() && S.dl.has(S.player.current().id)) S.player.reloadSource();
  });
  C.airplayPicker = function () { try { S.player.audio.webkitShowPlaybackTargetPicker(); } catch (e) { S.toast('AirPlay isn\'t available here.', 'x'); } };
})(window.Spin);
