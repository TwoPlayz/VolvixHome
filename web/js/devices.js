/* Volvix Home — your devices. Every open copy of Volvix Home (this PC, your phone…) shows up in the Output menu.
   Only one plays at a time: starting music on one pauses the others, and "play on" another device moves the
   song, queue and spot over to it. (Listening parties have their own speaker choice; see party.js.)
   Remote control: the device that is playing tells the server what it is playing (queue, song, spot); every other
   device then shows that, silently, and its play / pause / next / previous / seek / queue taps are sent to the playing
   device as commands. "This device" in the Output menu takes the music over. */
window.Spin = window.Spin || {};
(function (S) {
  'use strict';

  function id() {
    try {
      let v = localStorage.getItem('vh_device');
      if (!v) { v = 'd' + Math.random().toString(36).slice(2, 10) + Date.now().toString(36); localStorage.setItem('vh_device', v); }
      return v;
    } catch (e) { return 'tmp' + Math.random().toString(36).slice(2, 10); }
  }
  function guessName() {
    const ua = navigator.userAgent;
    const kind = /iPhone/.test(ua) ? 'iPhone' : /iPad/.test(ua) ? 'iPad' : /Android/.test(ua) ? 'Android' : /Mac OS X/.test(ua) ? 'Mac' : /Windows/.test(ua) ? 'Windows PC' : /Linux/.test(ua) ? 'Linux PC' : 'Device';
    const app = window.VOLVIX_NATIVE ? 'app' : /Edg\//.test(ua) ? 'Edge' : /Chrome\//.test(ua) ? 'Chrome' : /Firefox\//.test(ua) ? 'Firefox' : /Safari\//.test(ua) ? 'Safari' : '';
    return kind + (app ? ' · ' + app : '');
  }

  const me = id();
  const D = S.devices = { me: me, list: [], active: null, now: null, remote: false, block: 0, es: null };
  const post = function (path, body) {
    return fetch(path, { method: 'POST', credentials: 'same-origin', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body) }).catch(function () {});
  };

  /* ---------- the playing device reports what it plays ---------- */
  let lastKey = '', lastSent = 0;
  function report(force) {
    // the one that is playing reports; a device that is playing but not yet listed as active does too (the server then lists it)
    if (D.remote || inParty() || (D.active !== me && !S.player.state.playing)) return;
    const t = S.player.current();
    if (!t) return;
    const s = S.player.snapshot();
    const key = [t.id, s.index, !!s.playing, s.queue.length].join('|');
    const now = Date.now();
    if (!force && key === lastKey && now - lastSent < 5000) return;
    lastKey = key; lastSent = now;
    post('/api/devices/state', { device: me, state: { queue: s.queue.slice(0, 500), index: s.index, posMs: Math.round((s.pos || 0) * 1000), playing: !!s.playing } });
  }

  /* ---------- the others show it and control it ---------- */
  const command = function (c) { if (D.active) post('/api/devices/command', { device: me, to: D.active, cmd: c }); };
  const INTERCEPT = {
    handle: function (name, args) {
      switch (name) {
        case 'toggle': command({ op: D.now && D.now.playing ? 'pause' : 'play' }); return true;
        case 'next': command({ op: 'next' }); return true;
        case 'prev': command({ op: 'prev' }); return true;
        case 'seek': command({ op: 'seek', pos: args[0] }); return true;
        case 'playAt': command({ op: 'playAt', index: args[0] }); return true;
        case 'play': stopRemote(); return false; // you started something here: it plays here
        default: S.toast('Not available while you control another device'); return true;
      }
    },
    ended: function () {}
  };
  function stopRemote() {
    if (!D.remote) return;
    D.remote = false;
    S.player.setInterceptor(null);
    S.player.releaseRemote();
  }
  function syncRemote() {
    if (inParty() || Date.now() < D.block) return;
    const n = D.now;
    if (D.active && D.active !== me && n && n.queue && n.queue.length) {
      const o = other(D.active);
      D.remote = true;
      S.player.setInterceptor(INTERCEPT);
      S.player.applyRemote(Object.assign({ id: 'device' }, n), 'host', (n.serverNow || Date.now()) - Date.now());
      S.player.state.context = { type: 'device', name: 'Playing on ' + (o ? o.name : 'another device') };
      S.player.render();
    } else stopRemote();
  }
  /** Take the music over from the device that is playing: same queue, song and spot, now playing here. */
  D.takeOver = function () {
    const n = D.now;
    if (!D.remote || !n) return;
    const elapsed = n.playing ? Date.now() + ((n.serverNow || Date.now()) - Date.now()) - n.updatedAt : 0;
    stopRemote();
    D.block = Date.now() + 2500;
    S.player.adopt({ queue: n.queue, index: n.index, pos: Math.max(0, (n.posMs + elapsed) / 1000), playing: !!n.playing, shuffle: false, repeat: 'off' });
    post('/api/devices/active', { device: me });
  };

  /** Another device asked this one (the playing one) to do something. */
  function onCommand(m) {
    if (m.to !== me || inParty() || D.remote) return;
    const c = m.cmd || {}, st = S.player.state;
    if (c.op === 'play') { if (!st.playing) S.player.toggle(); }
    else if (c.op === 'pause') { if (st.playing) S.player.toggle(); }
    else if (c.op === 'next') S.player.next();
    else if (c.op === 'prev') S.player.prev();
    else if (c.op === 'seek') S.player.seekTo(Number(c.pos) || 0);
    else if (c.op === 'playAt') S.player.play(st.queue.slice(), Number(c.index) || 0, st.context);
    setTimeout(function () { report(true); }, 250);
  }

  function inParty() { return !!(S.party && S.party.active); }
  function other(idv) { return D.list.find(function (d) { return d.id === idv; }); }

  function onDevices(m) {
    const before = D.active;
    D.list = m.devices || []; D.active = m.active || null; D.now = m.now || null;
    // another device is the one playing now: this one stays quiet
    if (!inParty() && D.active && D.active !== me && D.active !== before && S.player.pauseLocal()) {
      const o = other(D.active);
      S.toast('Playing on ' + (o ? o.name : 'another device'), 'devices');
    }
    syncRemote();
    if (D.active === me) report(true);
  }
  function onTransfer(m) {
    if (m.to !== me || inParty()) return;
    S.player.adopt(m.state);
    S.toast(S.player.state.playing ? 'Playing here' : 'Tap play to continue here', 'devices');
  }

  function connect() {
    const es = new EventSource('/api/devices/events?device=' + encodeURIComponent(me) + '&name=' + encodeURIComponent(guessName()));
    es.onmessage = function (e) {
      let m; try { m = JSON.parse(e.data); } catch (x) { return; }
      if (m.type === 'devices') onDevices(m);
      else if (m.type === 'transfer') onTransfer(m);
      else if (m.type === 'command') onCommand(m);
      else if (m.type === 'now') { if (m.device === D.active) { D.now = m.state; syncRemote(); } }
    };
    D.es = es; // the browser reconnects by itself if the connection drops
  }

  // I just started playing: I'm the one that plays now
  S.player.audio.addEventListener('play', function () {
    if (inParty() || S.player.isRemote()) return;
    D.block = Date.now() + 2500; // right after starting here, the old player's last report must not pull this one back
    fetch('/api/devices/active', { method: 'POST', credentials: 'same-origin', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ device: me }) }).catch(function () {});
  });

  /** Move what's playing to another device. */
  D.transferTo = function (to) {
    const o = other(to);
    return S.server.json('POST', '/api/devices/transfer', { device: me, to: to, state: S.player.snapshot() }).then(function () {
      S.player.pauseLocal();
      S.toast('Playing on ' + (o ? o.name : 'that device'), 'devices');
    }).catch(function (e) { S.toast(e.message || 'Couldn\'t move it', 'x'); });
  };

  S.player.hooks.push(function () { report(false); });
  S.player.progressHooks.push(function () { report(false); });
  S.player.seekHooks.push(function () { report(true); });
  connect();
  window.addEventListener('pagehide', function () { if (D.es) D.es.close(); });
})(window.Spin);
