/* Volvix Home — listening parties.
 * Start one from the party button in the player, invite people from your household, and everyone shares one
 * queue: anyone can add, remove, reorder, skip and pause, and it all stays in sync. When you join you choose
 * where the sound plays: on your own device, or on the host's speaker (same room, same car) in which case your
 * device shows the same player but stays silent. The server holds the truth; this follows it. */
window.Spin = window.Spin || {};
(function (S) {
  'use strict';

  const P = S.party = { active: false, id: null, state: null, es: null, output: 'own', offset: 0 };
  const me = function () { return S.data.me.serverId; };
  const post = function (path, body) { return S.server.json('POST', '/api/party' + path, body === undefined ? {} : body); };
  const isHost = function () { return !!(P.state && P.state.host === me()); };
  const first = function (id) { return S.data.person(id).name.split(' ')[0]; };
  const members = function () { return P.state ? P.state.members : []; };

  function shuffled(list) {
    const a = list.slice();
    for (let i = a.length - 1; i > 0; i--) { const j = Math.floor(Math.random() * (i + 1)); const t = a[i]; a[i] = a[j]; a[j] = t; }
    return a;
  }

  /* ---------- following the server ---------- */
  function apply(state) {
    if (P.state && state.id === P.state.id && state.version < P.state.version) return; // an older message arriving late
    const mine = state.members.find(function (m) { return m.id === me(); });
    if (!mine) { detach('You\'re no longer in the party'); return; }
    P.state = state;
    P.offset = state.serverNow - Date.now(); // the server's clock vs ours, so everyone agrees on "now"
    P.output = mine.output;
    S.player.applyRemote(state, P.output, P.offset);
    paint();
  }

  function connect(id) {
    closeStream();
    const es = new EventSource('/api/party/' + encodeURIComponent(id) + '/events');
    es.onmessage = function (e) {
      let m; try { m = JSON.parse(e.data); } catch (err) { return; }
      if (m.type === 'ended') detach('The party ended');
      else if (m.type === 'state') apply(m.state);
    };
    es.onerror = function () {
      // the browser retries by itself; if the server says it's gone for good, find out why
      if (es.readyState !== 2) return;
      S.server.json('GET', '/api/party').then(function (r) { if (!r.party) detach('The party ended'); }).catch(function () {});
    };
    P.es = es;
  }
  function closeStream() { if (P.es) { P.es.close(); P.es = null; } }

  function attach(state) {
    P.active = true; P.id = state.id; P.state = null;
    S.player.setInterceptor(P.interceptor);
    connect(state.id);
    apply(state);
  }

  function detach(message) {
    if (!P.active) return;
    closeStream();
    P.active = false; P.id = null; P.state = null;
    S.player.setInterceptor(null);
    S.player.releaseRemote();
    paint();
    if (message) S.toast(message, 'party');
    if (S.notify) S.notify.load().catch(function () {});
  }

  /* ---------- actions: in a party, the player's controls send these ---------- */
  function send(op, extra) {
    if (!P.active) return Promise.resolve();
    return post('/' + P.id + '/action', Object.assign({ op: op }, extra || {}))
      .then(function (r) { apply(r.party); })
      .catch(function (e) { S.toast(e.message, 'x'); });
  }

  const handlers = {
    toggle: function () {
      // this device was blocked from making sound until a tap: a tap starts it here and doesn't pause anyone
      if (S.player.audioBlocked()) { S.player.resumeLocalAudio(); return; }
      send(P.state.playing ? 'pause' : 'play');
    },
    next: function () { send('next'); },
    prev: function () { send('prev'); },
    seek: function (sec) { send('seek', { posMs: Math.round(sec * 1000) }); },
    play: function (ids, start, ctx) {
      const shuffle = S.player.state.shuffle;
      if (start === -1) start = shuffle ? Math.floor(Math.random() * ids.length) : 0;
      start = start || 0;
      if (isHost()) {
        // the host replaces the queue (with the chosen song first when shuffling)
        let list = ids.slice(), begin = start;
        if (shuffle) { const one = list.splice(start, 1); list = one.concat(shuffled(list)); begin = 0; }
        send('setQueue', { ids: list, start: begin, playing: true });
      } else {
        // guests add to the shared queue instead of replacing it for everyone
        const add = start > 0 ? [ids[start]] : ids;
        send('add', { ids: shuffle ? shuffled(add) : add }).then(function () { S.toast(add.length === 1 ? 'Added to the party queue' : 'Added ' + add.length + ' songs to the party queue', 'queue'); });
      }
    },
    addNext: function (ids) { send('addNext', { ids: ids }).then(function () { S.toast('Playing next in the party', 'queue'); }); },
    addToQueue: function (ids) { send('add', { ids: ids }).then(function () { S.toast('Added to the party queue', 'queue'); }); },
    playAt: function (i) { send('playAt', { index: i }); },
    move: function (from, to) { send('move', { from: from, to: to }); },
    remove: function (i) { send('remove', { index: i }); },
    reshuffle: function () { S.toast('The party queue is shared, so it can\'t be reshuffled'); }
  };
  P.interceptor = {
    handle: function (name, args) { const h = handlers[name]; if (!h) return false; h.apply(null, args); return true; },
    // a device that's making sound tells the party its song finished (the server also keeps time itself)
    ended: function (index) { if (P.output === 'own') send('ended', { index: index }); }
  };

  /* ---------- the party button ---------- */
  function paint() {
    S.$$('[data-player="party"]').forEach(function (b) {
      b.classList.toggle('on', P.active);
      b.setAttribute('aria-pressed', P.active ? 'true' : 'false');
      const label = P.active ? 'Listening party (' + members().length + ' here)' : 'Start a listening party';
      b.title = label; b.setAttribute('aria-label', label);
      b.innerHTML = S.icon(P.active ? 'partyFill' : 'party', parseInt(b.dataset.size || '18', 10)) +
        (P.active && members().length > 1 ? '<span class="party-count">' + members().length + '</span>' : '');
    });
    S.$$('[data-player="output"]').forEach(function (b) { b.classList.toggle('on', P.active); }); // the cast button lights up in a party
    const h = S.$('[data-p="queue-panel"] .q-head h2');
    if (h) h.textContent = P.active ? 'Party queue' : 'Queue';
    document.documentElement.classList.toggle('in-party', P.active);
  }
  P.paint = paint; // the phone Now Playing sheet calls this after it draws, since it builds its own buttons
  S.player.hooks.push(paint);

  /* ---------- dialogs ---------- */
  function peopleRows(skip) {
    return S.data.others().map(function (p) {
      const here = skip.indexOf(p.id) !== -1;
      return '<label class="pick-row pick-check">' + S.avatar(p, 40) +
        '<span class="pick-text"><span class="t-name">' + S.esc(p.name) + '</span>' +
        '<span class="t-sub">' + (here ? 'Already invited or in the party' : 'Household') + '</span></span>' +
        '<input type="checkbox" value="' + p.id + '"' + (here ? ' checked disabled' : '') + '></label>';
    }).join('');
  }
  const chosen = function (el) { return S.$$('input[type=checkbox]:checked:not(:disabled)', el).map(function (c) { return c.value; }); };

  function startDialog() {
    if (!S.data.can('parties')) { S.toast('You don\'t have permission to start parties. Ask an admin.', 'lock'); return; }
    const st = S.player.state;
    const has = !!S.player.current();
    const m = S.modal({
      title: 'Start a listening party',
      body: '<p class="modal-text">' + (has ? 'What\'s playing now becomes the party\'s queue. ' : 'Nothing is playing yet, so you\'ll start with an empty queue. ') +
        'Everyone you invite can add, remove and skip songs, and you all stay in sync.</p>' +
        (S.data.others().length ? '<div class="pick-list">' + peopleRows([]) + '</div>' : '<p class="set-hint">No one else is in your household yet. You can still start a party and invite people later.</p>'),
      actions: [
        { label: 'Cancel', kind: 'secondary' },
        { label: 'Start party', kind: 'primary', onClick: function (close) {
          const invite = chosen(m.el);
          close();
          post('', {
            invite: invite, queue: has ? st.queue.slice(0, 500) : [], index: has ? st.index : 0,
            posMs: Math.round((st.pos || 0) * 1000), playing: has && !!st.playing
          }).then(function (r) {
            attach(r.party);
            S.toast(invite.length ? 'Party started. Invited ' + invite.map(first).join(', ') : 'Party started', 'party');
          }).catch(function (e) { S.toast(e.message, 'x'); });
        } }
      ]
    });
  }

  function inviteDialog() {
    const skip = members().map(function (x) { return x.id; }).concat((P.state && P.state.invited) || []);
    const people = S.data.others();
    const m = S.modal({
      title: 'Invite to the party',
      body: people.length ? '<div class="pick-list">' + peopleRows(skip) + '</div>' : '<p class="modal-text">No one else is in your household yet.</p>',
      actions: [
        { label: 'Cancel', kind: 'secondary' },
        people.length ? { label: 'Send invites', kind: 'primary', onClick: function (close) {
          const ids = chosen(m.el);
          close();
          if (!ids.length) return;
          post('/' + P.id + '/invite', { userIds: ids }).then(function (r) { apply(r.party); S.toast('Invited ' + ids.map(first).join(', '), 'party'); })
            .catch(function (e) { S.toast(e.message, 'x'); });
        } } : { label: 'Done', kind: 'primary' }
      ]
    });
  }

  /** Someone invited you. Choose where the sound plays, then join. */
  function joinDialog(o) {
    const host = o.host || 'the host';
    const m = S.modal({
      title: 'Join ' + host + '\'s party?',
      body: '<p class="modal-text">Where should the music play?</p>' +
        '<label class="choice"><input type="radio" name="out" value="own" checked><span><strong>This device</strong><em>You hear it here, in sync with everyone.</em></span></label>' +
        '<label class="choice"><input type="radio" name="out" value="host"><span><strong>' + S.esc(host) + '\'s speaker</strong><em>This device shows the party but stays silent. Best when you\'re in the same room or car.</em></span></label>',
      actions: [
        { label: 'Not now', kind: 'secondary' },
        { label: 'Join', kind: 'primary', onClick: function (close) {
          const out = (S.$('input[name=out]:checked', m.el) || {}).value || 'own';
          close();
          if (P.active) { S.toast('You\'re already in a party. Leave it first.', 'x'); return; }
          post('/' + o.partyId + '/join', { output: out }).then(function (r) {
            attach(r.party);
            if (o.noteId && S.notify) S.notify.setDone(o.noteId, 'joined');
            S.toast('Joined ' + host + '\'s party' + (out === 'host' ? ' (listening on their speaker)' : ''), 'party');
          }).catch(function (e) { S.toast(e.message, 'x'); if (S.notify) S.notify.load().catch(function () {}); });
        } }
      ]
    });
  }
  P.joinDialog = joinDialog;
  P.joinFromNote = function (n) { joinDialog({ partyId: n.data.partyId, host: n.data.from, noteId: n.id }); };

  function leave() {
    const wasHost = isHost();
    post('/' + P.id + '/leave').then(function () { detach(wasHost ? 'Party ended' : 'You left the party'); })
      .catch(function (e) { S.toast(e.message, 'x'); });
  }

  // for the output-device menu (output.js)
  P.isHost = isHost;
  P.hostName = function () { return P.state ? first(P.state.host) : ''; };
  P.setOutput = function (to) {
    return post('/' + P.id + '/output', { output: to }).then(function (r) {
      apply(r.party);
      S.toast(to === 'host' ? 'This device is silent: music comes from ' + first(P.state.host) + '\'s speaker' : 'Playing on this device', 'volume');
    }).catch(function (e) { S.toast(e.message, 'x'); });
  };

  /** The party entries for the Play on menu (the player bar has no party button of its own on computers, like the app). */
  P.menuItems = function () {
    if (!P.active) return [{ sep: true }, { label: 'Start a listening party…', icon: 'party', onClick: startDialog }];
    return [{ sep: true }, { label: 'Party: invite people', icon: 'plus', onClick: inviteDialog },
      { label: isHost() ? 'End party for everyone' : 'Leave party', icon: 'x', danger: true, onClick: leave }];
  };

  function menu(anchor) {
    if (!P.active) {
      S.popover(anchor, { items: [{ label: 'Start a listening party', icon: 'party', onClick: startDialog }] });
      return;
    }
    const names = members().map(function (m) { return m.id === me() ? 'You' : first(m.id); }).join(', ');
    S.popover(anchor, {
      header: '<div class="q-pop"><div class="menu-name">Listening party</div><div class="menu-sub">' + S.esc(names) + '</div>' +
        '<div class="menu-sub">' + (isHost() ? 'You\'re the host' : first(P.state.host) + ' is the host') + ' · ' +
        (P.output === 'own' ? 'Playing on this device' : 'Playing on ' + S.esc(first(P.state.host)) + '\'s speaker') + '</div></div>',
      items: [
        { label: 'Invite people', icon: 'plus', onClick: inviteDialog },
        isHost() ? null : { label: P.output === 'own' ? 'Use ' + first(P.state.host) + '\'s speaker' : 'Play on this device', icon: 'volume', onClick: function () {
          post('/' + P.id + '/output', { output: P.output === 'own' ? 'host' : 'own' }).then(function (r) { apply(r.party); S.toast(P.output === 'host' ? 'This device is silent: music comes from ' + first(P.state.host) + '\'s speaker' : 'Playing on this device'); })
            .catch(function (e) { S.toast(e.message, 'x'); });
        } },
        { sep: true },
        { label: isHost() ? 'End party for everyone' : 'Leave party', icon: 'x', danger: true, onClick: leave }
      ]
    });
  }

  document.addEventListener('click', function (e) {
    const b = e.target.closest('[data-player="party"]');
    if (!b) return;
    e.preventDefault(); e.stopPropagation();
    menu(b);
  });

  // pick up a party we were in before a reload
  S.server.json('GET', '/api/party').then(function (r) { if (r.party) attach(r.party); }).catch(function () {});
  window.addEventListener('pagehide', closeStream);
})(window.Spin);
