/* Volvix Home — playback: real audio streamed from your server, with the queue, shuffle, repeat,
 * resume-where-you-left-off, and lock screen / media key controls (Media Session). */
window.Spin = window.Spin || {};
(function (S) {
  'use strict';

  const TICK = 250; // ms

  const st = {
    queue: [],        // track ids in play order
    original: null,   // un-shuffled order, kept while shuffle is on
    userQ: 0,         // songs added one at a time by the person: they sit right after the current song, in the order added
    index: -1,
    playing: false,
    pos: 0,           // seconds into the current track
    shuffle: false,
    repeat: 'off',    // 'off' | 'all' | 'one'
    volume: 0.7,
    muted: false,
    context: null,    // { type, id, name }
    queueOpen: false
  };
  let timer = null;
  let xfadeDone = null;     // the song a crossfade was already started for

  /* ---------- listening party: while in one, the server decides what plays ---------- */
  let interceptor = null;   // { handle(name, args) → true when the party took that action, ended(index) }
  let remote = null;        // { state, offset, output, blocked } while following a party
  const intercept = function (name, args) { return !!(interceptor && interceptor.handle(name, args)); };

  /* ---------- the audio element ---------- */
  const audio = new Audio();
  audio.preload = 'auto';
  let loadedId = null;
  /** Point the audio element at a track (only reloads when the track changes). */
  let pendingBlob = null; // a downloaded song is being fetched from this device's storage
  function load(t, startAt) {
    if (!t) return;
    if (loadedId !== t.id) {
      loadedId = t.id; pendingBlob = null;
      if (S.dl && S.dl.has(t.id) && !(S.player.streamOnly && navigator.onLine !== false)) {
        // a song downloaded to this device plays from there (no connection needed)
        const id = t.id; pendingBlob = id;
        audio.removeAttribute('src'); audio.load();
        S.dl.blobUrl(id).then(function (url) {
          if (loadedId !== id) return; // moved on already
          pendingBlob = null;
          audio.src = url || S.server.streamUrl(id);
          if (startAt != null) setPos(startAt);
          if (st.playing || (remote && remote.output === 'own' && remote.state && remote.state.playing)) startAudio();
        });
        return;
      }
      audio.src = S.server.streamUrl(t.id);
    }
    if (startAt != null) setPos(startAt);
  }
  function setPos(sec) {
    st.pos = sec;
    try {
      if (audio.readyState >= 1) audio.currentTime = sec;
      else audio.addEventListener('loadedmetadata', function () { try { audio.currentTime = sec; } catch (e) { /* not seekable yet */ } }, { once: true });
    } catch (e) { /* ignore */ }
  }
  /** Start the audio; if the browser blocks it (no tap yet), show it as paused. */
  function startAudio() {
    if (pendingBlob) return; // starts by itself once the downloaded file is ready
    if (S.fx) S.fx.wake(); // crossfade / EQ / leveling: build the sound chain and let it make sound
    const p = audio.play();
    if (p && p.catch) p.catch(function (err) {
      if (err && err.name === 'AbortError') return; // a newer track took over
      // in a party the music IS playing for everyone: we just need a tap before this device may make sound
      if (remote) { remote.blocked = true; S.toast('Tap play to hear the party', 'volume'); render(); return; }
      st.playing = false; stopTimer(); render();
    });
  }
  const durationOf = function (t) { return (t && t.duration) || (isFinite(audio.duration) ? Math.round(audio.duration) : 0); };

  // "owned" here means playable right now: owned, and downloaded if there's no internet
  const owned = function (id) { return S.lib.isPlayable ? S.lib.isPlayable(id) : S.lib.isTrackOwned(id); };
  const isOffline = function () { return !!(S.dl && S.dl.isOffline && S.dl.isOffline()); };
  const current = function () { return st.index >= 0 ? S.data.track(st.queue[st.index]) : null; };

  function shuffleArray(arr) {
    for (let i = arr.length - 1; i > 0; i--) {
      const j = Math.floor(Math.random() * (i + 1));
      const tmp = arr[i]; arr[i] = arr[j]; arr[j] = tmp;
    }
    return arr;
  }

  function skippedToast(n) {
    if (n > 0) S.toast('Skipped ' + n + ' ' + (isOffline() ? 'unavailable' : 'locked') + ' song' + (n === 1 ? '' : 's'), isOffline() ? 'wifiOff' : 'lock');
  }

  /** Walk from `from` in direction `dir` to the next owned track. */
  function seek(from, dir) {
    const n = st.queue.length;
    let i = from, skipped = 0;
    for (let k = 0; k < n; k++) {
      i += dir;
      if (i >= n || i < 0) {
        if (st.repeat !== 'all') return { index: -1, skipped: skipped };
        i = (i + n) % n;
      }
      if (owned(st.queue[i])) return { index: i, skipped: skipped };
      if (S.data.track(st.queue[i])) skipped++; // a song that's gone from the library is passed over without a word
    }
    return { index: -1, skipped: skipped };
  }

  function startTimer() {
    stopTimer();
    timer = setInterval(tick, TICK);
  }
  function stopTimer() { if (timer) clearInterval(timer); timer = null; }

  function tick() {
    const t = current();
    if (!t) return;
    if (remote) {
      // party: the position comes from the party clock; a device making sound corrects itself if it drifts
      st.pos = remoteTarget();
      if (remote.output === 'own' && !audio.paused && Math.abs((audio.currentTime || 0) - st.pos) > 2) setPos(st.pos);
      renderProgress(); mediaPosition();
      return;
    }
    if (!st.playing) return;
    if (st.sleepAt && Date.now() >= st.sleepAt) { st.sleepAt = 0; S.toast('Sleep timer: paused', 'moon'); toggle(); return; }
    st.pos = audio.currentTime || st.pos;
    if (S.stats && !audio.paused) S.stats.tick(t, TICK / 1000);
    maybeCrossfade(t);
    renderProgress();
    saveResume(false);
    mediaPosition();
  }

  /** Near the end of a song, start the next one underneath (Settings > Playback > Crossfade). */
  function maybeCrossfade(t) {
    const s = S.store.state.settings || {};
    const fade = Number(s.crossfade) || 0;
    if (!fade || !S.fx || remote || !st.playing || xfadeDone === t.id || st.repeat === 'one') return;
    const dur = durationOf(t), left = dur - (audio.currentTime || 0);
    if (!dur || dur < fade * 2.5 || left > fade || left < 0.4) return;
    const nxt = seek(st.index, 1);
    if (nxt.index < 0) return; // the last song just ends
    const n = S.data.track(st.queue[nxt.index]);
    // songs that follow each other on an album usually flow into each other already
    if (!s.crossfadeAlbums && n && n.albumId === t.albumId && n.no === t.no + 1) return;
    xfadeDone = t.id;
    if (!S.fx.crossfade(Math.min(fade, left), function () { next(true); })) xfadeDone = null;
  }

  // the song ended / the OS or headphones paused or played it
  audio.addEventListener('ended', function () {
    if (remote) { if (interceptor && interceptor.ended) interceptor.ended(st.index); return; } // the party moves on
    if (st.sleepSong) { st.sleepSong = false; st.playing = false; stopTimer(); S.toast('Sleep timer: paused after this song', 'moon'); render(); return; }
    if (st.repeat === 'one') { setPos(0); startAudio(); return; }
    next(true);
  });
  audio.addEventListener('pause', function () { if (remote) return; if (st.playing && !audio.ended) { st.playing = false; stopTimer(); render(); } });
  audio.addEventListener('play', function () { if (remote) return; if (!st.playing) { st.playing = true; startTimer(); render(); } });
  audio.addEventListener('loadedmetadata', function () {
    const t = current();
    if (t && !t.duration && isFinite(audio.duration)) { t.duration = Math.round(audio.duration); renderProgress(); }
  });
  let errors = 0;
  audio.addEventListener('error', function () {
    const t = current();
    if (!t || !audio.src) return;
    S.toast('Couldn\'t play ' + t.title, 'x');
    if (remote) return; // the party decides what's next
    // skip ahead, but don't spin through a whole queue of broken files
    if (++errors < 4 && st.playing) setTimeout(function () { next(true); }, 800);
    else { st.playing = false; stopTimer(); render(); }
  });
  audio.addEventListener('playing', function () { errors = 0; });

  /* ---------- remember where you left off ---------- */
  let lastSaved = 0;
  function saveResume(force) {
    if (remote) return; // a party's queue isn't yours to resume
    const now = Date.now();
    if (!force && now - lastSaved < 4000) return;
    lastSaved = now;
    const t = current();
    S.store.aux.set('resume', t ? {
      queue: st.queue.length > 500 ? st.queue.slice(Math.max(0, st.index - 50), st.index + 450) : st.queue,
      index: st.queue.length > 500 ? Math.min(50, st.index) : st.index,
      pos: Math.round(st.pos), context: st.context, shuffle: st.shuffle, repeat: st.repeat,
      volume: st.volume, muted: st.muted, at: now
    } : null);
  }
  function restoreResume() {
    const r = S.store.aux.get('resume');
    if (!r || !Array.isArray(r.queue) || !r.queue.length) return;
    if ((S.store.state.settings || {}).resumeLast === false) return; // they chose to start fresh
    const queue = r.queue.filter(function (id) { return !!S.data.track(id); });
    const id = r.queue[r.index];
    const index = queue.indexOf(id);
    if (index < 0) return;
    st.queue = queue; st.index = index; st.context = r.context || null;
    st.shuffle = !!r.shuffle; st.repeat = r.repeat || 'off';
    if (typeof r.volume === 'number') st.volume = r.volume;
    st.muted = !!r.muted;
    const t = current();
    st.pos = Math.min(Math.max(0, r.pos || 0), Math.max(0, t.duration - 1));
    st.playing = false; // never auto-play on launch
    load(t, st.pos);
  }

  /* ---------- lock screen, Control Center, media keys, headphones (Media Session) ---------- */
  const ms = navigator.mediaSession;
  let msTrack = null;
  function mediaSession(t, album) {
    if (!ms) return;
    try {
      ms.playbackState = t ? (st.playing ? 'playing' : 'paused') : 'none';
      if (!t || msTrack === t.id) return;
      msTrack = t.id;
      ms.metadata = new MediaMetadata({
        title: t.title, artist: t.artist, album: album ? album.title : '',
        artwork: album && album.cover ? [{ src: S.server.coverUrl(album.id), sizes: '512x512' }] : []
      });
    } catch (e) { /* older browsers */ }
  }
  let msLast = 0;
  function mediaPosition() {
    if (!ms || !ms.setPositionState || Date.now() - msLast < 5000) return;
    msLast = Date.now();
    const t = current();
    try { if (t && durationOf(t)) ms.setPositionState({ duration: durationOf(t), position: Math.min(st.pos, durationOf(t)), playbackRate: 1 }); } catch (e) { /* ignore */ }
  }
  if (ms) {
    const on = function (action, fn) { try { ms.setActionHandler(action, fn); } catch (e) { /* unsupported action */ } };
    on('play', function () { if (!st.playing) toggle(); });
    on('pause', function () { if (st.playing) toggle(); });
    on('nexttrack', function () { next(false); });
    on('previoustrack', function () { prev(); });
    on('seekto', function (d) { if (d && d.seekTime != null) userSeek(d.seekTime); });
    on('seekbackward', function () { userSeek(Math.max(0, st.pos - 10)); });
    on('seekforward', function () { userSeek(st.pos + 10); });
  }
  window.addEventListener('pagehide', function () { saveResume(true); });
  document.addEventListener('visibilitychange', function () { if (document.hidden) saveResume(true); });

  function recordRecent() {
    if (!st.context || st.context.type !== 'album') return;
    if (S.store.state.settings && S.store.state.settings.privateSession) return; // private session: no history
    const id = st.context.id;
    S.store.update(function (s) {
      s.recent = [id].concat((s.recent || []).filter(function (x) { return x !== id; })).slice(0, 12);
    });
  }

  /* ---------- playback settings: speed, sleep timer ---------- */
  function applySettings() {
    const s = S.store.state.settings || {};
    if (audio.playbackRate !== 1) audio.playbackRate = 1; // (speed control was removed)
    audio.defaultPlaybackRate = 1;
  }
  audio.addEventListener('loadstart', applySettings);
  /** v: 0 = off, minutes, or 'song' (stop when this song ends). */
  function sleep(v) {
    st.sleepAt = 0; st.sleepSong = false;
    if (v === 'song') st.sleepSong = true;
    else if (Number(v) > 0) st.sleepAt = Date.now() + Number(v) * 60000;
  }

  /* ---------- public API ---------- */
  /** The user moved the seek bar (or a lock-screen scrubber). In a party that's everyone's position. */
  function userSeek(sec) {
    if (intercept('seek', [sec])) return;
    setPos(sec); renderProgress();
    S.player.seekHooks.forEach(function (fn) { fn(sec); });
  }

  function play(ids, startIndex, context) {
    if (!ids || !ids.length) return;
    // songs that have left the library (still listed in a playlist or Liked Songs) are not part of what plays
    if (ids.some(function (id) { return !S.data.track(id); })) {
      if (startIndex > 0) startIndex -= ids.slice(0, startIndex).filter(function (id) { return !S.data.track(id); }).length;
      ids = ids.filter(function (id) { return !!S.data.track(id); });
      if (!ids.length) { S.toast('Those songs aren\'t in your library any more', 'lock'); return; }
    }
    if (intercept('play', [ids, startIndex, context])) return;
    // -1 = "from the top": the first song, or a random one when shuffle is on
    startIndex = startIndex === -1 ? (st.shuffle ? Math.floor(Math.random() * ids.length) : 0) : (startIndex || 0);
    if (!ids.some(owned)) {
      if (isOffline() && ids.some(S.lib.isTrackOwned)) S.toast('No internet connection, and none of these are downloaded.', 'wifiOff');
      else if (ids.some(function (id) { return S.lib.isBlocked && S.lib.isBlocked(id); })) S.toast('Explicit songs are turned off for your account.', 'lock');
      else S.toast('Those songs aren\'t in your library', 'lock');
      return;
    }
    let queue = ids.slice();
    st.original = null;
    if (st.shuffle) {
      st.original = queue.slice();
      const first = queue.splice(startIndex, 1);
      queue = first.concat(shuffleArray(queue));
      startIndex = 0;
    }
    st.queue = queue;
    st.context = context || null;
    st.userQ = 0;

    let i = startIndex, skipped = 0;
    while (i < queue.length && !owned(queue[i])) { i++; skipped++; }
    if (i >= queue.length) { st.index = -1; const r = seek(-1, 1); i = r.index; skipped += r.skipped; }
    skippedToast(skipped);

    st.index = i;
    st.pos = 0;
    st.playing = true;
    load(current(), 0);
    startAudio();
    startTimer();
    recordRecent();
    render();
  }

  function toggle() {
    if (intercept('toggle', [])) return;
    if (st.index >= 0 && !current()) { heal(); return; }
    if (!current()) {
      // nothing loaded: start the most recent album
      const recent = (S.store.state.recent || [])[0];
      const album = recent && S.data.album(recent);
      if (album) play(album.trackIds, 0, { type: 'album', id: album.id, name: album.title });
      else S.toast('Pick something to play from your library');
      return;
    }
    st.playing = !st.playing;
    if (st.playing) { load(current()); startAudio(); startTimer(); } else { audio.pause(); stopTimer(); }
    render();
  }

  /** The queue played out: forget it, so there's no stale song and the mini player hides. */
  function clearPlayer() {
    st.playing = false; stopTimer(); audio.pause();
    audio.removeAttribute('src'); audio.load(); loadedId = null;
    st.queue = []; st.index = -1; st.original = null; st.userQ = 0; st.pos = 0; st.context = null; st.queueOpen = false;
    render();
  }

  /** The library changed under the player (a song was removed): drop what no longer exists, and if the song that was
   *  playing is gone, move to the next one, or clear the player, instead of getting stuck on nothing. */
  function heal() {
    if (remote || !st.queue.length) return;
    const gone = function (id) { return !S.data.track(id); };
    if (!st.queue.some(gone)) return;
    const curGone = st.index >= 0 && gone(st.queue[st.index]);
    let before = 0;
    st.queue.forEach(function (id, i) { if (i < st.index && !gone(id)) before++; });
    st.queue = st.queue.filter(function (id) { return !gone(id); });
    if (st.original) st.original = st.original.filter(function (id) { return !gone(id); });
    if (!curGone) { st.index = st.index >= 0 ? before : st.index; renderQueue(); return; }
    st.index = before - 1; // so that seeking forward lands on the song after the missing one
    const r = seek(st.index, 1);
    if (r.index < 0) { clearPlayer(); S.toast('That song is no longer in your library', 'x'); return; }
    st.index = r.index; xfadeDone = null; st.pos = 0;
    load(current(), 0);
    if (st.playing) { startAudio(); startTimer(); }
    S.toast('A song was removed from your library, so playback moved on', 'x');
    render();
  }

  function next(auto) {
    if (st.index >= 0 && !current()) { heal(); return; }
    if (!current()) return;
    if (!auto && intercept('next', [])) return;
    const r = seek(st.index, 1);
    skippedToast(r.skipped);
    if (r.index < 0) {
      // end of queue
      if (auto) { clearPlayer(); return; } // nothing left: the player goes away
      st.playing = false; stopTimer(); audio.pause(); setPos(0);
      S.toast('End of queue');
      render();
      return;
    }
    st.index = r.index;
    xfadeDone = null;
    st.userQ = Math.max(0, st.userQ - 1); // one of the person's own picks has just been reached
    load(current(), 0);
    if (st.playing) { startAudio(); startTimer(); }
    render();
  }

  function prev() {
    if (st.index >= 0 && !current()) { heal(); return; }
    if (!current()) return;
    if (intercept('prev', [])) return;
    if (st.pos > 3) { setPos(0); renderProgress(); return; }
    const r = seek(st.index, -1);
    skippedToast(r.skipped);
    if (r.index < 0) { setPos(0); renderProgress(); return; }
    st.index = r.index; st.userQ = 0;
    load(current(), 0);
    if (st.playing) startAudio();
    render();
  }

  function setShuffle(on) {
    st.shuffle = on; st.userQ = 0;
    // in a party the queue is shared, so shuffle only decides how songs you start or add are ordered
    if (remote) { S.toast(on ? 'Shuffle on' : 'Shuffle off', 'shuffle'); render(); return; }
    if (st.index >= 0) {
      const curId = st.queue[st.index];
      if (on) {
        st.original = st.queue.slice();
        const upcoming = st.queue.slice(st.index + 1);
        st.queue = st.queue.slice(0, st.index + 1).concat(shuffleArray(upcoming));
      } else if (st.original) {
        st.queue = st.original;
        st.index = st.queue.indexOf(curId);
        st.original = null;
      }
    }
    S.toast(on ? 'Shuffle on' : 'Shuffle off', 'shuffle');
    render();
  }

  function cycleRepeat() {
    if (remote) { S.toast('Repeat isn\'t available in a party'); return; }
    st.repeat = st.repeat === 'off' ? 'all' : st.repeat === 'all' ? 'one' : 'off';
    S.toast(st.repeat === 'off' ? 'Repeat off' : st.repeat === 'all' ? 'Repeat all' : 'Repeat this song', 'repeat');
    render();
  }

  function insert(ids, atNext) {
    ids = ids.filter(owned);
    if (!ids.length) return;
    if (intercept(atNext ? 'addNext' : 'addToQueue', [ids])) return;
    if (st.index < 0) { play(ids, 0, null); return; }
    // one song at a time goes to the front: right after the current song, and after the songs added before it.
    // A whole album or playlist goes to the end of the queue.
    const mine = atNext || ids.length === 1;
    const at = mine ? st.index + 1 + (atNext ? 0 : st.userQ) : st.queue.length;
    st.queue.splice.apply(st.queue, [at, 0].concat(ids));
    if (mine) st.userQ += ids.length;
    S.toast(mine ? (at === st.index + 1 ? 'Playing next' : 'Added after your other picks') : 'Added to queue', 'queue');
    renderQueue();
  }

  /** Where the party is right now, in seconds (its clock, not this device's). */
  function remoteTarget() {
    if (!remote) return st.pos;
    const s = remote.state;
    const elapsed = s.playing ? Date.now() + remote.offset - s.updatedAt : 0;
    return Math.max(0, (s.posMs + elapsed) / 1000);
  }

  function applyRemote(s, output, offset) {
    remote = { state: s, output: output, offset: offset, blocked: remote ? remote.blocked : false };
    st.queue = s.queue.slice(); st.original = null;
    st.index = s.index;
    st.context = { type: 'party', id: s.id, name: 'Party' };
    st.playing = !!s.playing && s.index >= 0;
    const t = current();
    if (!t) { audio.pause(); st.pos = 0; stopTimer(); render(); return; }
    const target = remoteTarget();
    st.pos = target;
    if (output === 'own') {
      audio.muted = st.muted;
      if (loadedId !== t.id) { audio.src = S.server.streamUrl(t.id); loadedId = t.id; setPos(target); }
      else if (Math.abs((audio.currentTime || 0) - target) > 1.5) setPos(target);
      if (st.playing) { if (audio.paused && !remote.blocked) startAudio(); } else { audio.pause(); remote.blocked = false; }
    } else {
      audio.pause(); // the sound comes out of the host's speaker; this device stays silent
    }
    startTimer();
    render();
  }

  function releaseRemote() {
    if (!remote) return;
    const silent = remote.output === 'host';
    remote = null;
    st.context = null;
    if (silent) { st.playing = false; audio.pause(); stopTimer(); }
    else if (st.playing && audio.paused) startAudio();
    render();
  }

  function playAt(queueIndex) {
    if (intercept('playAt', [queueIndex])) return;
    if (!owned(st.queue[queueIndex])) { S.toast('Locked: this song will be skipped', 'lock'); return; }
    st.userQ = queueIndex > st.index ? Math.max(0, st.userQ - (queueIndex - st.index)) : 0;
    st.index = queueIndex; st.playing = true;
    load(current(), 0); startAudio();
    startTimer(); render();
  }

  /* ---------- rendering into the player bar ---------- */
  function render() {
    const t = current();
    const album = t && S.data.album(t.albumId);
    const art = S.$('[data-p="art"]');
    // nothing loaded (first launch, or the queue was cleared): no player bar at all
    document.documentElement.classList.toggle('player-empty', !t && !remote); // the player only goes away when nothing's playing and you're not in a party
    if (!art) return;

    art.innerHTML = album ? S.cover(album) : S.icon('music', 22);
    S.$('[data-p="title"]').innerHTML = t
      ? '<a href="#/album/' + album.id + '">' + S.esc(t.title) + '</a>' + (t.explicit ? ' ' + S.explicitBadge() : '')
      : 'Nothing playing';
    S.$('[data-p="sub"]').innerHTML = t ? S.artistLink(t.artist) : '';
    const like = S.$('[data-p="like"]');
    if (like) { like.hidden = !t; like.dataset.like = t ? t.id : ''; }
    const bars = S.$('[data-p="eq"]');
    if (bars) { bars.classList.toggle('on', !!t); bars.classList.toggle('paused', !(st.playing && t)); }
    const more = S.$('[data-p="more"]');
    if (more) more.hidden = !t;
    const add = S.$('[data-p="add"]');
    if (add) add.hidden = !t;
    if (S.pl && S.pl.syncAdds) S.pl.syncAdds();

    const playing = st.playing && !!t;
    S.$$('[data-player="toggle"]').forEach(function (b) {
      b.innerHTML = S.icon(playing ? 'pause' : 'play', 18);
      b.setAttribute('aria-label', playing ? 'Pause' : 'Play');
    });
    const sh = S.$('[data-player="shuffle"]');
    sh.classList.toggle('on', st.shuffle);
    sh.setAttribute('aria-pressed', st.shuffle);
    const rp = S.$('[data-player="repeat"]');
    rp.classList.toggle('on', st.repeat !== 'off');
    rp.classList.toggle('repeat-one', st.repeat === 'one');
    rp.setAttribute('aria-label', 'Repeat: ' + st.repeat);

    const q = S.$('[data-p="quality"]');
    if (q && album) qualityButton(q, S.lib.streamQuality ? S.lib.streamQuality(album, t.id) : S.lib.quality(album));

    audio.volume = st.volume;
    audio.muted = st.muted || !!S.player.silent; // silent: a cast device is making the sound
    applySettings();
    mediaSession(t, album);
    const vol = S.$('[data-p="volume-fill"]');
    if (vol) vol.style.width = (st.muted ? 0 : st.volume * 100) + '%';
    S.$$('[data-volume]').forEach(function (b) { b.setAttribute('aria-valuenow', Math.round((st.muted ? 0 : st.volume) * 100)); b.setAttribute('aria-valuetext', Math.round((st.muted ? 0 : st.volume) * 100) + ' percent'); });
    const volBtn = S.$('[data-player="mute"]');
    if (volBtn) volBtn.classList.toggle('muted', st.muted || st.volume === 0);

    S.$$('[data-player="queue"]').forEach(function (b) { b.classList.toggle('on', st.queueOpen); });

    renderProgress();
    renderQueue();
    saveResume(true);
    if (S.pl && S.pl.syncHearts) S.pl.syncHearts();
    if (S.markPlaying) S.markPlaying();
    S.player.hooks.forEach(function (fn) { fn(); });
  }

  function renderProgress() {
    const t = current();
    const dur = durationOf(t);
    const pct = dur ? Math.min(100, (st.pos / dur) * 100) : 0;
    const fill = S.$('[data-p="progress-fill"]');
    if (!fill) return;
    fill.style.width = pct + '%';
    S.$('[data-p="mini-progress"]').style.width = pct + '%';
    S.$('[data-p="pos"]').textContent = S.fmtTime(st.pos);
    S.$('[data-p="dur"]').textContent = S.fmtTime(dur);
    S.$$('[data-seek]').forEach(function (b) { b.setAttribute('aria-valuenow', Math.round(pct)); b.setAttribute('aria-valuetext', S.fmtTime(st.pos) + ' of ' + S.fmtTime(dur)); });
    S.player.progressHooks.forEach(function (fn) { fn(); });
  }

  function renderQueue() {
    // the computer Now Playing page's "Up next" shows the same rows
    const np = S.$('[data-np-queue]');
    if (np) np.innerHTML = queueBody();
    const panel = S.$('[data-p="queue-panel"]');
    if (!panel) return;
    panel.hidden = !st.queueOpen;
    if (!st.queueOpen) return;
    panel.innerHTML =
      '<div class="q-head"><h2>Queue</h2><div class="q-head-actions">' +
        (st.queue.length - st.index - 1 > 1 ? '<button class="btn btn-secondary btn-sm" data-q-reshuffle title="Shuffle the songs coming up">' + S.icon('shuffle', 14) + 'Reshuffle</button>' : '') +
        '<button class="icon-btn" data-player="queue" aria-label="Close queue">' + S.icon('x', 18) + '</button></div></div>' +
      '<div class="q-body">' + queueBody() + '</div>';
  }

  /** The queue's rows: now playing, what you queued, then what's next from the album / playlist. */
  function queueBody() {
    const t = current();
    function row(id, i) {
      const tr = S.data.track(id);
      const al = S.data.album(tr.albumId);
      const ok = owned(id);
      const upcoming = i > st.index;
      return '<button class="q-row' + (ok ? '' : ' locked') + (i === st.index ? ' now' : '') + (upcoming ? ' movable' : '') + '" data-queue-index="' + i + '">' +
        (upcoming ? '<span class="drag-sign q-sign" aria-hidden="true" title="Drag to reorder: grab the row anywhere">' + S.icon('grip', 16) + '</span>' : '') +
        '<span class="q-cover">' + S.cover(al) + '</span>' +
        '<span class="q-text"><span class="q-name">' + S.esc(tr.title) + (tr.explicit ? ' ' + S.explicitBadge() : '') + '</span>' +
        '<span class="q-sub">' + (ok ? '' : S.icon('lock', 11) + (S.lib.isBlocked(id) ? 'Explicit · skipped · ' : 'Will be skipped · ')) + S.artistLink(tr.artist, { inert: true }) + '</span></span>' +
        '<span class="q-dur">' + S.fmtTime(tr.duration) + '</span>' +
        (upcoming ? '<span class="q-remove" role="button" tabindex="0" data-q-remove="' + i + '" aria-label="Remove from queue" title="Remove from queue">' + S.icon('x', 15) + '</span>' : '') +
        '</button>';
    }
    const upcoming = [], mineRows = [];
    for (let i = st.index + 1; i < st.queue.length; i++) (i <= st.index + st.userQ ? mineRows : upcoming).push(row(st.queue[i], i));

    return (t ? '<div class="q-label">Now playing</div>' + row(st.queue[st.index], st.index) : '<p class="q-empty">Nothing playing.</p>') +
      (mineRows.length ? '<div class="q-label">Next in queue</div>' + mineRows.join('') : '') +
      (upcoming.length ? '<div class="q-label">Next' + (st.context && st.context.name ? ' from ' + S.esc(st.context.name) : '') + '</div>' + upcoming.join('') : '') +
      (t && !upcoming.length && !mineRows.length ? '<p class="q-empty">' + (st.repeat === 'all' ? 'Repeat is on: the queue starts over.' : 'Nothing else queued.') + '</p>' : '');
  }

  /* ---------- input ---------- */
  document.addEventListener('click', function (e) {
    const rm = e.target.closest('[data-q-remove]');
    if (rm) { removeAt(parseInt(rm.dataset.qRemove, 10)); return; }
    if (e.target.closest('[data-q-reshuffle]')) { reshuffle(); return; }
    const qi = e.target.closest('[data-queue-index]');
    if (qi) { playAt(parseInt(qi.dataset.queueIndex, 10)); return; }

    const b = e.target.closest('[data-player]');
    if (!b) return;
    switch (b.dataset.player) {
      case 'toggle': toggle(); break;
      case 'next': next(false); break;
      case 'prev': prev(); break;
      case 'shuffle': setShuffle(!st.shuffle); break;
      case 'repeat': cycleRepeat(); break;
      case 'queue':
        // computers: the queue is the Now Playing page's "Up next" (pressing it again goes back); phones: the queue panel
        if (!b.closest('.np, [data-p="queue-panel"]') && window.matchMedia('(min-width: 761px)').matches) {
          if (st.queueOpen) { st.queueOpen = false; render(); }
          if (/^\/now/.test(S.router.current())) history.back(); else location.hash = '#/now/queue';
          break;
        }
        st.queueOpen = !st.queueOpen; render(); break;
      case 'mute': st.muted = !st.muted; render(); break;
      case 'quality': qualityMenu(b); break;
    }
  });

  /* ---------- queue editing ---------- */
  function moveInQueue(from, to) {
    if (from <= st.index || to <= st.index || from === to) return;
    if (intercept('move', [from, to])) return;
    const lo = st.index + 1, hi = st.index + st.userQ;
    const fromIn = from >= lo && from <= hi, toIn = to >= lo && to <= hi;
    if (fromIn && !toIn) st.userQ--; else if (!fromIn && toIn) st.userQ++;
    const id = st.queue.splice(from, 1)[0];
    st.queue.splice(to, 0, id);
    renderQueue(); saveResume(true);
  }
  function removeAt(i) {
    if (i <= st.index) return;
    if (intercept('remove', [i])) return;
    if (i >= st.index + 1 && i <= st.index + st.userQ) st.userQ--;
    st.queue.splice(i, 1);
    renderQueue(); saveResume(true);
    const first = S.$('[data-q-remove]'); if (first && document.activeElement === document.body) first.focus();
  }
  function reshuffle() {
    if (intercept('reshuffle', [])) return;
    const upcoming = st.queue.slice(st.index + 1);
    if (upcoming.length < 2) return;
    st.queue = st.queue.slice(0, st.index + 1).concat(shuffleArray(upcoming));
    st.userQ = 0;
    S.toast('Reshuffled what\'s coming up', 'shuffle');
    renderQueue(); saveResume(true);
  }
  document.addEventListener('keydown', function (e) {
    const rm = e.target.closest && e.target.closest('[data-q-remove]');
    if (rm && (e.key === 'Enter' || e.key === ' ')) { e.preventDefault(); e.stopPropagation(); removeAt(parseInt(rm.dataset.qRemove, 10)); }
  }, true);
  // drag a row anywhere to reorder what's coming up (mouse, touch, pen)
  document.addEventListener('pointerdown', function (e) {
    const body = e.target.closest('.q-body');
    if (!body || !e.target.closest('.q-row.movable')) return;
    S.dragRows(e, {
      row: '.q-row.movable',
      scope: body,
      scroller: body,
      index: function (row) { return parseInt(row.dataset.queueIndex, 10); },
      drop: function (from, to) { moveInQueue(from, to); }
    });
  });
  /** Quality as a small icon button: bars for Low → Hi-Res, accent for Hi-Res, dot for a downloaded copy. */
  function qualityButton(btn, q) {
    btn.innerHTML = S.qualityIcon(q, 18);
    btn.classList.toggle('hires', !!q.hires);
    btn.classList.toggle('q-dl', !!q.downloaded);
    const label = 'Quality: ' + q.label + ' · ' + q.detail + (q.downloaded ? ' · downloaded' : '');
    btn.title = label;
    btn.setAttribute('aria-label', label);
  }

  function qualityMenu(anchor) {
    const t = current();
    const album = t && S.data.album(t.albumId);
    const q = album ? (S.lib.streamQuality ? S.lib.streamQuality(album, t.id) : S.lib.quality(album)) : null;
    S.popover(anchor, {
      header: '<div class="q-pop"><div class="menu-name">' + (q ? q.label : 'Lossless') + '</div>' +
        '<div class="menu-sub">' + (q ? S.esc(q.detail) : '') + '</div>' +
        '<p class="menu-sub q-pop-note">Playing the original file from your server, untouched.</p></div>',
      items: []
    });
  }

  // Drag-to-seek and drag-to-set-volume
  document.addEventListener('pointerdown', function (e) {
    const bar = e.target.closest('[data-seek], [data-volume]');
    if (!bar) return;
    e.preventDefault();
    const isSeek = bar.hasAttribute('data-seek');
    function apply(ev) {
      const r = bar.getBoundingClientRect();
      const ratio = Math.min(1, Math.max(0, (ev.clientX - r.left) / r.width));
      if (isSeek) {
        const t = current();
        if (!t) return;
        userSeek(ratio * durationOf(t));
      } else {
        st.volume = ratio; st.muted = false;
        render();
      }
    }
    apply(e);
    function move(ev) { apply(ev); }
    function up() { document.removeEventListener('pointermove', move); document.removeEventListener('pointerup', up); }
    document.addEventListener('pointermove', move);
    document.addEventListener('pointerup', up);
  });

  // keyboard on the seek and volume bars: arrows move by 10 seconds / 5 percent (Home, End jump to the ends)
  document.addEventListener('keydown', function (e) {
    const bar = e.target.closest && e.target.closest('[data-seek], [data-volume]');
    if (!bar || e.ctrlKey || e.metaKey || e.altKey) return;
    const k = e.key, up = k === 'ArrowRight' || k === 'ArrowUp', down = k === 'ArrowLeft' || k === 'ArrowDown';
    if (!up && !down && k !== 'Home' && k !== 'End') return;
    e.preventDefault();
    if (bar.hasAttribute('data-seek')) {
      const t = current(); if (!t) return;
      const dur = durationOf(t);
      userSeek(k === 'Home' ? 0 : k === 'End' ? Math.max(0, dur - 1) : Math.max(0, Math.min(dur, st.pos + (up ? 10 : -10))));
    } else {
      st.volume = k === 'Home' ? 0 : k === 'End' ? 1 : Math.max(0, Math.min(1, Math.round((st.volume + (up ? 0.05 : -0.05)) * 100) / 100));
      st.muted = false; render();
    }
  });

  document.addEventListener('keydown', function (e) {
    if (e.code !== 'Space') return;
    const tag = (e.target.tagName || '').toLowerCase();
    if (tag === 'input' || tag === 'textarea' || tag === 'select' || tag === 'button' || e.target.isContentEditable) return;
    if (e.target.closest && e.target.closest('.track')) return;
    e.preventDefault();
    toggle();
  });

  restoreResume();

  S.player = {
    state: st,
    play: play,
    toggle: toggle,
    next: next,
    prev: prev,
    addNext: function (ids) { insert(ids, true); },
    addToQueue: function (ids) { insert(ids, false); },
    heal: heal,
    isCurrent: function (id) { const t = current(); return !!t && t.id === id; },
    current: current,
    render: render,
    renderQueue: renderQueue,
    /** Shuffle on/off for the whole player (it never starts or stops anything by itself). */
    toggleShuffle: function () { setShuffle(!st.shuffle); },
    audio: audio,
    seekTo: userSeek,
    applySettings: applySettings,
    /* ----- moving playback between your devices (see devices.js) ----- */
    snapshot: function () {
      const n = st.queue.length > 500;
      return { queue: n ? st.queue.slice(Math.max(0, st.index - 50), st.index + 450) : st.queue, index: n ? Math.min(50, st.index) : st.index,
        pos: st.pos, playing: st.playing, context: st.context, shuffle: st.shuffle, repeat: st.repeat };
    },
    /** Take over from another device: same queue, song and spot. */
    adopt: function (s) {
      if (remote || !s || !Array.isArray(s.queue) || !s.queue.length) return;
      const queue = s.queue.filter(function (id) { return !!S.data.track(id); });
      const id = s.queue[s.index];
      const index = queue.indexOf(id);
      if (index < 0) return;
      st.queue = queue; st.index = index; st.context = s.context || null; st.original = null;
      st.shuffle = !!s.shuffle; st.repeat = s.repeat || 'off';
      st.playing = !!s.playing;
      load(current(), Math.max(0, Number(s.pos) || 0));
      if (st.playing) { startAudio(); startTimer(); } else stopTimer();
      render();
    },
    /** Another of your devices took over: stop here (the queue stays, in case you come back). */
    pauseLocal: function () {
      if (remote || !st.playing) return false;
      st.playing = false; audio.pause(); stopTimer(); render();
      return true;
    },
    sleep: sleep,
    sleepInfo: function () { return st.sleepSong ? 'song' : (st.sleepAt && st.sleepAt > Date.now() ? Math.ceil((st.sleepAt - Date.now()) / 60000) : 0); },
    /* ----- listening party (see party.js) ----- */
    setInterceptor: function (i) { interceptor = i; },
    isRemote: function () { return !!remote; },
    remoteState: function () { return remote && remote.state; },
    /** The party's sound was blocked until a tap: a tap on play lets this device start it. */
    resumeLocalAudio: function () { if (remote) { remote.blocked = false; startAudio(); } },
    audioBlocked: function () { return !!(remote && remote.blocked && audio.paused); },
    /** Follow a party: its queue, song, position and play / pause become this player's. */
    applyRemote: applyRemote,
    /** Stop following. On your own speaker the music carries on; on the host's speaker it stops here. */
    releaseRemote: releaseRemote,
    moveInQueue: moveInQueue,
    removeFromQueue: removeAt,
    reshuffle: reshuffle,
    qualityButton: qualityButton,
    streamOnly: false,  // AirPlay can't reach a downloaded copy kept inside the app, so play from the server while it's on
    /** Load the current song again (used when the source should change, e.g. AirPlay switched on). */
    reloadSource: function () {
      const t = current();
      if (!t || remote) return;
      const pos = audio.currentTime || st.pos, was = st.playing;
      loadedId = null; load(t, pos);
      if (was && !pendingBlob) startAudio();
    },
    silent: false,      // the sound comes from a cast device: this one keeps time but stays quiet
    seekHooks: [],      // called with the new position when the person seeks
    hooks: [],          // called after every full render (used by v7 extras)
    progressHooks: []   // called on every progress tick
  };
})(window.Spin);
