/* Volvix Home — the Output menu: where the music plays.
 *  - On its own: this device, or any other device where you have Volvix Home open (phone, PC…). Only one plays
 *    at a time; picking another moves the song, queue and spot there.
 *  - In a party: a guest swaps between the host's speaker and their own device. */
window.Spin = window.Spin || {};
(function (S) {
  'use strict';

  function menu(anchor) {
    const P = S.party;
    const inParty = P && P.active && P.state;

    if (inParty) {
      const host = P.hostName();
      if (P.isHost()) {
        S.popover(anchor, {
          header: '<div class="q-pop"><div class="menu-name">Party sound</div><div class="menu-sub">Guests on your speaker hear this device</div></div>',
          items: [{ label: 'This device (host)', icon: 'check', onClick: function () {} }].concat(P.menuItems())
        });
        return;
      }
      const own = P.output === 'own';
      S.popover(anchor, {
        header: '<div class="q-pop"><div class="menu-name">Play on</div><div class="menu-sub">Party with ' + S.esc(host) + '</div></div>',
        items: [
          { label: 'This device', icon: own ? 'check' : 'devices', onClick: function () { if (!own) P.setOutput('own'); } },
          { label: host + '\'s speaker', icon: own ? 'devices' : 'check', onClick: function () { if (own) P.setOutput('host'); } }
        ].concat(P.menuItems())
      });
      return;
    }

    const D = S.devices;
    const items = [];
    const here = D.list.find(function (d) { return d.id === D.me; });
    const playingHere = D.active === D.me || !D.active;
    items.push({ label: (here ? here.name : 'This device') + ' (this device)', icon: playingHere ? 'check' : 'devices', onClick: function () {
      // bring it back here
      const o = D.list.find(function (d) { return d.id === D.active; });
      if (!playingHere && o) S.toast('Press play here to move it back', 'devices');
    } });
    D.list.filter(function (d) { return d.id !== D.me; }).forEach(function (d) {
      items.push({ label: d.name + (d.active ? ' · playing' : ''), icon: d.active ? 'check' : 'devices', onClick: function () {
        if (!S.player.current()) { S.toast('Start a song first, then move it', 'devices'); return; }
        D.transferTo(d.id);
      } });
    });
    // TVs and speakers (Google Cast), found by the server
    const C = S.cast;
    items.push({ sep: true });
    if (C.active) {
      items.push({ label: 'Casting to ' + C.name, icon: 'check', onClick: function () {} }, { label: 'Stop casting', icon: 'x', onClick: C.stop });
    } else {
      C.devices.forEach(function (dev) {
        items.push({ label: dev.name + (dev.model ? ' · ' + dev.model : ''), icon: 'devices', onClick: function () { C.start(dev); } });
      });
      if (!C.devices.length) items.push({ label: 'No TVs or speakers found yet', icon: 'info', onClick: function () { C.refresh(); } });
      items.push({ label: 'Add a TV or speaker by address…', icon: 'plus', onClick: C.addByAddress });
    }
    if (P && P.menuItems) P.menuItems().forEach(function (i) { items.push(i); });
    if (C.airplay()) items.push({ label: 'AirPlay…', icon: 'volume', onClick: C.airplayPicker });
    S.popover(anchor, { header: '<div class="q-pop"><div class="menu-name">Play on</div><div class="menu-sub">One device plays at a time</div></div>', items: items });
  }

  document.addEventListener('click', function (e) {
    const b = e.target.closest('[data-player="output"]');
    if (!b) return;
    e.preventDefault(); e.stopPropagation();
    menu(b);
  });

  S.output = { menu: menu };
})(window.Spin);
