/* Volvix Home — output device: where this device's sound comes out.
 *  - On its own: pick a speaker, headphones or Bluetooth device (browsers that allow it: Chrome, Edge, Android).
 *  - In a party: a guest swaps between the host's speaker and their own device. Only on their own device do they
 *    also get the device list. The host just picks which device plays (guests on the host's speaker hear that one). */
window.Spin = window.Spin || {};
(function (S) {
  'use strict';

  const audio = S.player.audio;
  const canSink = typeof audio.setSinkId === 'function';
  const KEY = 'vh_sink';
  function saved() { try { return localStorage.getItem(KEY) || ''; } catch (e) { return ''; } }
  function remember(id) { try { if (id) localStorage.setItem(KEY, id); else localStorage.removeItem(KEY); } catch (e) { /* private mode */ } }

  // go back to the speaker picked last time
  if (canSink && saved()) audio.setSinkId(saved()).catch(function () { remember(''); });

  function choose(id, label) {
    return audio.setSinkId(id).then(function () {
      remember(id);
      S.toast(id ? 'Playing on ' + label : 'Playing on this device\'s default speaker', 'volume');
    }).catch(function () { S.toast('Couldn\'t switch to ' + label, 'x'); });
  }

  /** Output devices the browser lets us see (names appear once the browser has allowed it). */
  function listDevices() {
    if (!canSink || !navigator.mediaDevices || !navigator.mediaDevices.enumerateDevices) return Promise.resolve([]);
    return navigator.mediaDevices.enumerateDevices().then(function (all) {
      return all.filter(function (d) { return d.kind === 'audiooutput' && d.deviceId && d.deviceId !== 'default' && d.deviceId !== 'communications'; });
    }).catch(function () { return []; });
  }

  function deviceItems() {
    return listDevices().then(function (devs) {
      const current = audio.sinkId || '';
      const items = [{ label: 'Default speaker', icon: current ? 'devices' : 'check', onClick: function () { choose('', 'the default speaker'); } }];
      devs.forEach(function (d, i) {
        const name = d.label || 'Speaker ' + (i + 1);
        items.push({ label: name, icon: current === d.deviceId ? 'check' : 'devices', onClick: function () { choose(d.deviceId, name); } });
      });
      // some browsers hide names until you pick one in their own dialog
      if (navigator.mediaDevices && navigator.mediaDevices.selectAudioOutput && !devs.some(function (d) { return d.label; })) {
        items.push({ label: 'Choose another device…', icon: 'plus', onClick: function () {
          navigator.mediaDevices.selectAudioOutput().then(function (d) { return choose(d.deviceId, d.label || 'that device'); }).catch(function () { /* cancelled */ });
        } });
      }
      return items;
    });
  }

  function note(text) { return { label: text, icon: 'info', onClick: function () {} }; }

  function menu(anchor) {
    const P = S.party;
    const inParty = P && P.active && P.state;
    const guest = inParty && !P.isHost();

    // a guest on the host's speaker: this device is silent, so the only choice is who plays
    if (guest && P.output === 'host') {
      S.popover(anchor, {
        header: '<div class="q-pop"><div class="menu-name">Play on</div><div class="menu-sub">Party with ' + S.esc(P.hostName()) + '</div></div>',
        items: [
          { label: P.hostName() + '\'s speaker', icon: 'check', onClick: function () {} },
          { label: 'This device', icon: 'devices', onClick: function () { P.setOutput('own'); } }
        ]
      });
      return;
    }

    deviceItems().then(function (devices) {
      const items = [];
      let header = '<div class="q-pop"><div class="menu-name">Play on</div>';
      if (guest) {
        header += '<div class="menu-sub">Party with ' + S.esc(P.hostName()) + '</div></div>';
        items.push({ label: P.hostName() + '\'s speaker', icon: 'devices', onClick: function () { P.setOutput('host'); } }, { sep: true });
      } else if (inParty) {
        header += '<div class="menu-sub">Guests on your speaker hear this one</div></div>';
      } else {
        header += '</div>';
      }
      if (canSink) items.push.apply(items, devices);
      else items.push(note('Your browser can\'t pick a speaker. Use the system volume / AirPlay / Bluetooth menu.'));
      S.popover(anchor, { header: header, items: items });
    });
  }

  document.addEventListener('click', function (e) {
    const b = e.target.closest('[data-player="output"]');
    if (!b) return;
    e.preventDefault(); e.stopPropagation();
    menu(b);
  });

  S.output = { menu: menu, supported: canSink };
})(window.Spin);
