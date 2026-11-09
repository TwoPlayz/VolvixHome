/* Volvix Home — your devices. Every open copy of Volvix Home (this PC, your phone…) shows up in the Output menu.
   Only one plays at a time: starting music on one pauses the others, and "play on" another device moves the
   song, queue and spot over to it. (Listening parties have their own speaker choice; see party.js.) */
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
  const D = S.devices = { me: me, list: [], active: null, es: null };

  function inParty() { return !!(S.party && S.party.active); }
  function other(idv) { return D.list.find(function (d) { return d.id === idv; }); }

  function onDevices(m) {
    const before = D.active;
    D.list = m.devices || []; D.active = m.active || null;
    // another device is the one playing now: this one stays quiet
    if (!inParty() && D.active && D.active !== me && D.active !== before && S.player.pauseLocal()) {
      const o = other(D.active);
      S.toast('Playing on ' + (o ? o.name : 'another device'), 'devices');
    }
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
      if (m.type === 'devices') onDevices(m); else if (m.type === 'transfer') onTransfer(m);
    };
    D.es = es; // the browser reconnects by itself if the connection drops
  }

  // I just started playing: I'm the one that plays now
  S.player.audio.addEventListener('play', function () {
    if (inParty() || S.player.isRemote()) return;
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

  connect();
  window.addEventListener('pagehide', function () { if (D.es) D.es.close(); });
})(window.Spin);
