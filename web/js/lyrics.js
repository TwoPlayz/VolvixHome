/* Volvix Home — lyrics panel. Synced lyrics follow the song (tap a line to jump there); plain lyrics just scroll.
   Word-synced lyrics (enhanced LRC) also fill in each word as it's sung (tap a word to jump to it).
   Where they come from (the server decides): a .lrc file next to the song, the file's own tags, then LRCLIB. */
window.Spin = window.Spin || {};
(function (S) {
  'use strict';

  const cache = {};            // trackId -> { lyrics, online } | { error }
  let open = false;
  let shownId = null;          // track the panel is currently showing
  let lines = [];              // synced lines of that track
  let active = -1;
  let userScrolledAt = 0;
  let wordFrame = 0;           // requestAnimationFrame id while words are being filled in
  let lastPos = -1, lastPosAt = 0;

  function panel() { return S.$('[data-p="lyrics-panel"]'); }

  function note(icon, title, text, retry) {
    return '<div class="ly-note">' + S.icon(icon, 28) + '<strong>' + S.esc(title) + '</strong><p>' + text + '</p>' +
      (retry ? '<button class="btn btn-secondary btn-sm" data-lyrics-retry>Try again</button>' : '') + '</div>';
  }

  function body(t, res) {
    if (!res) return note('lyrics', 'Looking for lyrics…', 'Checking the song\'s files, then the lyrics service.');
    if (res.error) return note('x', 'Couldn\'t load lyrics', S.esc(res.error), true);
    const ly = res.lyrics;
    if (!ly) {
      return note('lyrics', 'No lyrics found',
        res.online
          ? 'Nothing for this song in its files or on LRCLIB. Put a <b>.lrc</b> file with the same name next to the song to add your own.'
          : 'Online lyrics are turned off on this server. Put a <b>.lrc</b> file with the same name next to the song, or embed the lyrics in its tags.');
    }
    if (ly.instrumental) return note('music', 'Instrumental', 'This song has no lyrics.');
    if (ly.synced && ly.synced.length) {
      return '<div class="ly-lines" data-lyrics-synced>' + ly.synced.map(function (l, i) {
        const inner = l.words && l.words.length
          ? l.words.map(function (w, j) { return '<span class="ly-w" data-lw="' + j + '">' + S.esc(w.text) + '</span>'; }).join('')
          : (l.text ? S.esc(l.text) : '♪');
        return '<button class="ly-line' + (l.text ? '' : ' ly-gap') + (l.words && l.words.length ? ' ly-words' : '') + '" data-ly="' + i + '">' + inner + '</button>';
      }).join('') + '</div>';
    }
    return '<div class="ly-plain">' + S.esc(ly.plain).replace(/\n/g, '<br>') + '</div>';
  }

  function paint() {
    const el = panel();
    if (!el) return;
    el.hidden = !open;
    document.documentElement.classList.toggle('lyrics-open', open);
    S.$$('[data-lyrics-toggle]').forEach(function (b) { b.classList.toggle('on', open); });
    if (!open) return;
    const t = S.player.current();
    if (!t) { open = false; el.hidden = true; return; }
    const res = cache[t.id];
    lines = res && res.lyrics && res.lyrics.synced ? res.lyrics.synced : [];
    active = -1;
    shownId = t.id;
    el.innerHTML =
      '<div class="q-head"><div class="ly-title"><h2>Lyrics</h2><span>' + S.esc(t.title) + ' · ' + S.esc(t.artist) + '</span></div>' +
        '<button class="icon-btn" data-lyrics-close aria-label="Close lyrics">' + S.icon('x', 18) + '</button></div>' +
      '<div class="q-body ly-body" data-lyrics-body>' + body(t, res) + '</div>';
    follow(true);
  }

  function load(t, force) {
    if (!force && cache[t.id]) return;
    delete cache[t.id];
    S.server.json('GET', '/api/lyrics/' + encodeURIComponent(t.id)).then(function (r) {
      cache[t.id] = r;
    }).catch(function (e) {
      // a failed look-up isn't remembered as "no lyrics": try again next time
      cache[t.id] = { error: e && e.message ? e.message : 'The lyrics service didn\'t answer.' };
      setTimeout(function () { if (cache[t.id] && cache[t.id].error) delete cache[t.id]; }, 20000);
    }).then(function () {
      const cur = S.player.current();
      if (open && cur && cur.id === t.id) paint();
    });
  }

  /** Highlight the line being sung. instant = jump there (no smooth scrolling). */
  function follow(instant) {
    if (!open || !lines.length) return;
    const ms = S.player.state.pos * 1000 + 250; // show a line a touch early, as the eye is faster than the ear
    let lo = 0, hi = lines.length - 1, idx = -1;
    while (lo <= hi) { const mid = (lo + hi) >> 1; if (lines[mid].ms <= ms) { idx = mid; lo = mid + 1; } else hi = mid - 1; }
    if (idx === active && !instant) return;
    active = idx;
    const wrap = S.$('[data-lyrics-synced]');
    if (!wrap) return;
    S.$$('.ly-line.now, .ly-line.past', wrap).forEach(function (n) { n.classList.remove('now', 'past'); });
    const kids = wrap.children;
    for (let i = 0; i < kids.length; i++) kids[i].classList.toggle('past', i < idx);
    const el = idx >= 0 ? kids[idx] : null;
    if (!el) return;
    el.classList.add('now');
    words();
    if (Date.now() - userScrolledAt > 4000) {
      const box = S.$('[data-lyrics-body]');
      const target = el.offsetTop - box.clientHeight / 2 + el.offsetHeight / 2;
      if (instant) box.scrollTop = target; else box.scrollTo({ top: target, behavior: 'smooth' });
    }
  }

  /** Where the song is, in ms, smoothed between the player's progress ticks so words fill in evenly. */
  function nowMs() {
    const st = S.player.state;
    const t = performance.now();
    if (st.pos !== lastPos) { lastPos = st.pos; lastPosAt = t; }
    return st.pos * 1000 + (st.playing ? Math.min(1000, t - lastPosAt) : 0);
  }

  /** Fill in the words of the line being sung (only for word-synced lyrics). Runs every frame while playing. */
  function words() {
    if (wordFrame) { cancelAnimationFrame(wordFrame); wordFrame = 0; }
    if (!open || active < 0) return;
    const l = lines[active];
    if (!l || !l.words || !l.words.length) return;
    const wrap = S.$('[data-lyrics-synced]');
    const el = wrap && wrap.children[active];
    if (!el) return;
    const spans = el.querySelectorAll('.ly-w');
    const ms = nowMs() + 60;
    const nextLine = lines[active + 1];
    for (let i = 0; i < l.words.length && i < spans.length; i++) {
      const start = l.words[i].ms;
      const next = l.words[i + 1];
      // the last word runs until the next line, but no longer than a couple of seconds
      const end = next ? next.ms : Math.min(nextLine ? nextLine.ms : start + 1000, start + 2000);
      const p = end > start ? Math.max(0, Math.min(1, (ms - start) / (end - start))) : (ms >= start ? 1 : 0);
      spans[i].style.setProperty('--p', (p * 100).toFixed(1) + '%');
    }
    if (S.player.state.playing) wordFrame = requestAnimationFrame(words);
  }

  function openFor(trackId) {
    const t = S.player.current();
    if (!t) { S.toast('Play a song first', 'lyrics'); return; }
    const st = S.player.state;
    open = true;
    if (st.queueOpen) { st.queueOpen = false; S.player.render(); } // one side panel at a time
    load(t);
    paint();
  }
  function close() { open = false; paint(); }

  document.addEventListener('click', function (e) {
    if (e.target.closest('[data-lyrics-close]')) { close(); return; }
    if (e.target.closest('[data-lyrics-retry]')) { const t = S.player.current(); if (t) { load(t, true); paint(); } return; }
    const tog = e.target.closest('[data-lyrics-toggle]');
    if (tog) { open ? close() : openFor(); return; }
    const line = e.target.closest('.ly-line');
    if (line && line.closest('[data-p="lyrics-panel"]')) {
      const l = lines[parseInt(line.dataset.ly, 10)];
      const wEl = e.target.closest('.ly-w');
      const w = wEl && l && l.words ? l.words[parseInt(wEl.dataset.lw, 10)] : null;
      if (l) { userScrolledAt = 0; S.player.seekTo((w || l).ms / 1000); }
    }
  });
  // the reader scrolled by hand: leave them be for a few seconds
  ['wheel', 'touchmove'].forEach(function (ev) {
    document.addEventListener(ev, function (e) { if (e.target.closest && e.target.closest('[data-p="lyrics-panel"]')) userScrolledAt = Date.now(); }, { passive: true });
  });
  document.addEventListener('keydown', function (e) {
    if (open && e.key === 'Escape' && !document.querySelector('.modal-scrim, .popover')) close();
  });

  // new song, or the queue panel opened: keep the panel honest
  S.player.hooks.push(function () {
    if (!open) return;
    const t = S.player.current();
    if (!t) { close(); return; }
    if (S.player.state.queueOpen) { open = false; paint(); return; }
    if (t.id !== shownId) { load(t); paint(); }
  });
  // every progress tick also refreshes the words, so they keep up even when the browser skips animation frames
  S.player.progressHooks.push(function () { follow(false); words(); });

  S.lyrics = { open: openFor, close: close, isOpen: function () { return open; } };
})(window.Spin);
