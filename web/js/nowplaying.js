/* Volvix — phone "Now Playing" screen: tap the mini player to open it full screen. */
window.Spin = window.Spin || {};
(function (S) {
  'use strict';

  const phone = window.matchMedia('(max-width: 760px)');
  let el = null;
  let open = false;
  let opener = null;

  function ensure() {
    if (el && document.body.contains(el)) return el;
    el = document.createElement('section');
    el.className = 'np';
    el.setAttribute('role', 'dialog');
    el.setAttribute('aria-modal', 'true');
    el.setAttribute('aria-label', 'Now playing');
    el.hidden = true;
    document.body.appendChild(el);
    bindSwipe(el);
    return el;
  }

  function ctrl(icon, label, action, extra) {
    return '<button class="ctrl' + (extra || '') + '" data-player="' + action + '" aria-label="' + label + '" title="' + label + '">' + S.icon(icon, 22) + '</button>';
  }

  /** Full rebuild: track changed, opened, or play/shuffle/repeat state changed. */
  function render() {
    if (!open) return;
    ensure();
    const t = S.player.current();
    const st = S.player.state;
    if (!t) { close(); return; }
    const album = S.data.album(t.albumId);
    const q = S.lib.streamQuality ? S.lib.streamQuality(album, t.id) : S.lib.quality(album);
    const inPl = S.pl.editable().some(function (p) { return S.pl.trackIds(p).indexOf(t.id) !== -1; });
    const from = st.context && st.context.name ? st.context.name : album.title;
    // keep keyboard focus on the same control across rebuilds
    const f = document.activeElement && el.contains(document.activeElement) ? document.activeElement : null;
    const focusSel = !f ? null
      : f.dataset.player ? '[data-player="' + f.dataset.player + '"]'
      : f.hasAttribute('data-add-pl') ? '.np-add'
      : f.hasAttribute('data-np-close') ? '[data-np-close]'
      : f.hasAttribute('data-np-info') ? '[data-np-info]' : null;

    el.innerHTML =
      '<div class="np-head" data-np-grab>' +
        '<button class="icon-btn" data-np-close aria-label="Close now playing">' + S.icon('chevronDown', 24) + '</button>' +
        '<div class="np-from"><span>Playing from</span><strong>' + S.esc(from) + '</strong></div>' +
        '<span class="np-head-spacer"></span>' +
      '</div>' +
      '<div class="np-art">' + S.cover(album) + '</div>' +
      '<div class="np-meta">' +
        '<div class="np-text"><div class="np-title">' + S.albumLink(album, t.title) + '</div>' +
          '<div class="np-artist">' + S.artistLink(t.artist) + '</div></div>' +
        S.likeButton(t.id, 'np-add np-like', 22) +
        '<button class="add-pl np-add' + (inPl ? ' on' : '') + '" data-add-pl="' + t.id + '" aria-label="Add to playlist" title="Add to playlist">' + S.icon(inPl ? 'check' : 'plus', 22) + '</button>' +
        '<button class="np-add np-more" data-np-info aria-label="More" title="More">' + S.icon('more', 22) + '</button>' +
      '</div>' +
      '<div class="np-progress">' +
        '<div class="bar" data-seek role="slider" aria-label="Seek"><div class="bar-fill" data-np-fill></div></div>' +
        '<div class="np-times"><span data-np-pos>0:00</span><span data-np-dur>' + S.fmtTime(t.duration) + '</span></div>' +
      '</div>' +
      '<div class="np-controls">' +
        ctrl('shuffle', 'Shuffle', 'shuffle', st.shuffle ? ' on' : '') +
        ctrl('prev', 'Previous', 'prev') +
        '<button class="ctrl-play np-play" data-player="toggle" aria-label="' + (st.playing ? 'Pause' : 'Play') + '">' + S.icon(st.playing ? 'pause' : 'play', 30) + '</button>' +
        ctrl('next', 'Next', 'next') +
        ctrl('repeat', 'Repeat: ' + st.repeat, 'repeat', (st.repeat !== 'off' ? ' on' : '') + (st.repeat === 'one' ? ' repeat-one' : '')) +
      '</div>' +
      '<div class="np-tools">' +
        '<button class="ctrl q-btn" data-player="quality" data-np-quality></button>' +
        ctrl('party', 'Listening party', 'party') +
        ctrl('queue', 'Queue', 'queue', st.queueOpen ? ' on' : '') +
      '</div>';
    S.player.qualityButton(el.querySelector('[data-np-quality]'), q);
    if (S.party && S.party.paint) S.party.paint(); // show whether you're in a party
    progress();
    if (focusSel) { const again = el.querySelector(focusSel); if (again) again.focus(); }
  }

  function progress() {
    if (!open || !el) return;
    const t = S.player.current();
    if (!t) return;
    const pos = S.player.state.pos;
    const fill = el.querySelector('[data-np-fill]');
    if (fill) fill.style.width = Math.min(100, (pos / t.duration) * 100) + '%';
    const p = el.querySelector('[data-np-pos]');
    if (p) p.textContent = S.fmtTime(pos);
  }

  let hideTimer = null;
  function show() {
    if (!S.player.current()) return;
    opener = document.activeElement;
    open = true;
    clearTimeout(hideTimer);
    ensure().hidden = false;
    el.style.transform = '';
    document.documentElement.classList.add('np-open');
    render();
    void el.offsetHeight; // start from the off-screen position so the slide plays
    el.classList.add('np-shown');
    const c = el.querySelector('[data-np-close]');
    if (c) c.focus({ preventScroll: true });
  }
  function close() {
    if (!open) return;
    open = false;
    if (el) {
      el.style.transform = '';
      el.classList.remove('np-shown');
      clearTimeout(hideTimer);
      hideTimer = setTimeout(function () { if (!open) el.hidden = true; }, 420);
    }
    document.documentElement.classList.remove('np-open');
    if (opener && opener.focus && document.body.contains(opener)) opener.focus({ preventScroll: true });
  }

  /** Drag the header down to close. */
  function bindSwipe(node) {
    let startY = null, startT = 0;
    node.addEventListener('pointerdown', function (e) {
      if (!e.target.closest('[data-np-grab], .np-art') || e.target.closest('button, a')) return;
      startY = e.clientY; startT = performance.now();
      node.classList.add('np-dragging');
    });
    window.addEventListener('pointermove', function (e) {
      if (startY == null) return;
      const dy = Math.max(0, e.clientY - startY);
      node.style.transform = 'translateY(' + dy + 'px)';
    });
    function end(e) {
      if (startY == null) return;
      const dy = e.clientY - startY;
      const speed = dy / Math.max(1, performance.now() - startT); // px per ms
      startY = null;
      node.classList.remove('np-dragging');
      if (dy > 120 || (dy > 30 && speed > 0.6)) close();
      else node.style.transform = ''; // springs back via the transition
    }
    window.addEventListener('pointerup', end);
    window.addEventListener('pointercancel', end);
  }

  // tap the mini player (anywhere but its play button) to open
  document.addEventListener('click', function (e) {
    if (!phone.matches) return;
    const bar = e.target.closest('.player');
    if (!bar || e.target.closest('[data-player="toggle"]')) return;
    e.preventDefault();
    e.stopPropagation();
    show();
  }, true);

  document.addEventListener('click', function (e) {
    if (!el || !open || !el.contains(e.target)) return;
    if (e.target.closest('[data-np-close]')) { close(); return; }
  });

  document.addEventListener('keydown', function (e) {
    if (open && e.key === 'Escape' && !document.querySelector('.modal-scrim, .popover')) close();
  });
  // following a link (album / artist) closes the screen
  window.addEventListener('hashchange', function () { if (open) close(); });
  // back on a wide screen: nothing to show
  phone.addEventListener && phone.addEventListener('change', function () { if (!phone.matches && open) close(); });

  S.player.hooks.push(render);
  S.player.progressHooks.push(progress);

  S.nowPlaying = { open: show, close: close, sync: render, isOpen: function () { return open; } };
})(window.Spin);
