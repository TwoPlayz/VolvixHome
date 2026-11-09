/* Volvix Home — Cast (Chromecast and other Google Cast TVs / speakers) and AirPlay.
 *  Cast: Chrome / Edge on a computer or Android (not inside the Windows app or on iPhone). The device fetches the song
 *  itself from this server, using a short-lived signed link, so open Volvix Home at your server's network address
 *  (not "localhost"). While casting, this player keeps the queue and controls everything, but stays silent.
 *  AirPlay: Safari and the iPhone app: the system's own speaker picker. */
window.Spin = window.Spin || {};
(function (S) {
  'use strict';

  const MIME = { FLAC: 'audio/flac', MP3: 'audio/mpeg', WAV: 'audio/wav', AAC: 'audio/mp4', ALAC: 'audio/mp4', OGG: 'audio/ogg', OPUS: 'audio/ogg' };
  const C = S.cast = { active: false, name: '', ready: null };
  let player = null, controller = null, loadedId = null, following = false, token = null;

  C.supported = function () {
    const ua = navigator.userAgent;
    return !window.VOLVIX_NATIVE && /Chrome\//.test(ua) && !/iPhone|iPad|Firefox/.test(ua);
  };
  C.airplay = function () { return !!window.WebKitPlaybackTargetAvailabilityEvent && typeof S.player.audio.webkitShowPlaybackTargetPicker === 'function'; };

  /** Load Google's sender library the first time someone asks to cast. Resolves true when it's usable. */
  function sdk() {
    if (C.ready) return C.ready;
    C.ready = new Promise(function (resolve) {
      const timer = setTimeout(function () { resolve(false); }, 8000);
      window.__onGCastApiAvailable = function (ok) {
        clearTimeout(timer);
        if (!ok) return resolve(false);
        try {
          const ctx = cast.framework.CastContext.getInstance();
          ctx.setOptions({ receiverApplicationId: chrome.cast.media.DEFAULT_MEDIA_RECEIVER_APP_ID, autoJoinPolicy: chrome.cast.AutoJoinPolicy.ORIGIN_SCOPED });
          ctx.addEventListener(cast.framework.CastContextEventType.SESSION_STATE_CHANGED, onSession);
          player = new cast.framework.RemotePlayer();
          controller = new cast.framework.RemotePlayerController(player);
          const E = cast.framework.RemotePlayerEventType;
          controller.addEventListener(E.IS_PAUSED_CHANGED, remotePaused);
          controller.addEventListener(E.CURRENT_TIME_CHANGED, remoteTime);
          resolve(true);
        } catch (e) { resolve(false); }
      };
      const s = document.createElement('script');
      s.src = 'https://www.gstatic.com/cv/js/sender/v1/cast_sender.js?loadCastFramework=1';
      s.onerror = function () { clearTimeout(timer); resolve(false); };
      document.head.appendChild(s);
    });
    return C.ready;
  }

  const session = function () { return cast.framework.CastContext.getInstance().getCurrentSession(); };

  function onSession(e) {
    const S_ = cast.framework.SessionState;
    if (e.sessionState === S_.SESSION_STARTED || e.sessionState === S_.SESSION_RESUMED) begin();
    else if (e.sessionState === S_.SESSION_ENDED) finish();
  }

  function begin() {
    const s = session();
    if (S.party && S.party.active) { S.toast('Casting isn\'t available in a party', 'x'); s && cast.framework.CastContext.getInstance().endCurrentSession(true); return; }
    if (/^(localhost|127\.|\[::1\])/.test(location.hostname)) {
      S.toast('Open Volvix Home at your server\'s network address (not localhost) so the TV can reach it.', 'x');
      cast.framework.CastContext.getInstance().endCurrentSession(true); return;
    }
    C.active = true; C.name = s && s.getCastDevice ? s.getCastDevice().friendlyName : 'a TV';
    loadedId = null; S.player.silent = true; S.player.render();
    S.toast('Casting to ' + C.name, 'devices');
  }
  function finish() {
    if (!C.active) return;
    C.active = false; loadedId = null; S.player.silent = false; S.player.render();
    S.toast('Stopped casting', 'devices');
  }

  function freshToken() {
    if (token && token.expires - 600 > Date.now() / 1000) return Promise.resolve(token.token);
    return S.server.json('POST', '/api/cast/token', {}).then(function (r) { token = r; return r.token; });
  }

  function loadTrack(t, pos) {
    const s = session();
    if (!s) return;
    loadedId = t.id;
    const al = S.data.album(t.albumId);
    freshToken().then(function (tk) {
      const base = location.origin;
      const mi = new chrome.cast.media.MediaInfo(base + '/api/stream/' + encodeURIComponent(t.id) + '?cast=' + encodeURIComponent(tk), MIME[(al && al.quality && al.quality.codec || '').toUpperCase()] || 'audio/mpeg');
      mi.streamType = chrome.cast.media.StreamType.BUFFERED;
      mi.duration = t.duration || null;
      const md = new chrome.cast.media.MusicTrackMediaMetadata();
      md.title = t.title; md.artist = t.artist; md.albumName = al ? al.title : '';
      if (al && al.cover) md.images = [new chrome.cast.Image(base + '/api/cover/' + encodeURIComponent(al.id) + '?cast=' + encodeURIComponent(tk))];
      mi.metadata = md;
      const req = new chrome.cast.media.LoadRequest(mi);
      req.autoplay = !!S.player.state.playing;
      req.currentTime = Math.max(0, pos || 0);
      return s.loadMedia(req);
    }).catch(function () { loadedId = null; S.toast('The TV couldn\'t play that song', 'x'); });
  }

  /** Keep the TV doing what this player says: song, play / pause, volume. */
  function sync() {
    if (!C.active || !player) return;
    const t = S.player.current(), st = S.player.state;
    if (!t) { if (loadedId && session()) { try { session().getSessionObj().stop(function () {}, function () {}); } catch (e) { /* ignore */ } } loadedId = null; return; }
    if (t.id !== loadedId) { loadTrack(t, st.pos); return; }
    if (player.isMediaLoaded) {
      if (st.playing === player.isPaused) controller.playOrPause();
      const v = st.muted ? 0 : st.volume;
      if (Math.abs((player.volumeLevel || 0) - v) > 0.02) { player.volumeLevel = v; controller.setVolumeLevel(); }
    }
  }
  S.player.hooks.push(sync);
  S.player.seekHooks.push(function (sec) {
    if (!C.active || following || !player || !player.isMediaLoaded) return;
    player.currentTime = sec; controller.seek();
  });

  // someone paused / resumed on the TV (its remote, the Google Home app…)
  function remotePaused() {
    if (!C.active || !player || !player.isMediaLoaded) return;
    const st = S.player.state;
    if (st.playing === player.isPaused) S.player.toggle();
  }
  // the TV is the clock: this player follows it
  function remoteTime() {
    if (!C.active || !player || !player.isMediaLoaded) return;
    const local = S.player.audio.currentTime || 0;
    if (Math.abs(local - player.currentTime) > 1) { following = true; try { S.player.seekTo(player.currentTime); } finally { following = false; } }
  }

  C.request = function () {
    if (!C.supported()) { S.toast('Casting works in Chrome or Edge on a computer or Android, not in the app or on iPhone.', 'x'); return; }
    sdk().then(function (ok) {
      if (!ok) { S.toast('Couldn\'t load Google Cast (no internet, or a blocker is in the way).', 'x'); return; }
      cast.framework.CastContext.getInstance().requestSession().catch(function () { /* cancelled */ });
    });
  };
  C.stop = function () { try { cast.framework.CastContext.getInstance().endCurrentSession(true); } catch (e) { /* ignore */ } };
  // AirPlay speakers can't fetch a downloaded copy (it lives inside the app), so while AirPlay is on, play from the server
  S.player.audio.addEventListener('webkitcurrentplaybacktargetiswirelesschanged', function () {
    const wireless = !!S.player.audio.webkitCurrentPlaybackTargetIsWireless;
    if (S.player.streamOnly === wireless) return;
    S.player.streamOnly = wireless;
    if (S.dl && S.player.current() && S.dl.has(S.player.current().id)) S.player.reloadSource();
  });
  C.airplayPicker = function () { try { S.player.audio.webkitShowPlaybackTargetPicker(); } catch (e) { S.toast('AirPlay isn\'t available here.', 'x'); } };
})(window.Spin);
