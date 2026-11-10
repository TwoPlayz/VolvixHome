/* Volvix — shared UI helpers: icons, covers, avatars, toasts, modals, popovers. */
window.Spin = window.Spin || {};
(function (S) {
  'use strict';

  /* ---------- build environment (js/config.js) ---------- */
  S.env = (window.VOLVIX_CONFIG && window.VOLVIX_CONFIG.env) || 'development';
  S.isDev = S.env !== 'production';
  /** Dev-only markup (demo shortcuts, prototype notes). Empty string in production builds. */
  S.dev = function (html) { return S.isDev ? html : ''; };
  document.documentElement.setAttribute('data-env', S.env);
  /*
   * Platform: 'web' | 'windows' (native Windows app) | 'ios' (native iPhone app).
   * App stores restrict outside payments on iOS (and consoles), so those builds don't sell anything:
   * no shop, cart, gift purchases, Family Plus or payment methods. People manage payments on the website.
   */
  S.platform = (window.VOLVIX_CONFIG && window.VOLVIX_CONFIG.platform) || 'web';
  S.canBuy = false; // Volvix Home sells nothing: it plays your own library
  /** Markup that sells or takes payment. Empty in builds where purchases aren't allowed. */
  S.buy = function (html) { return S.canBuy ? html : ''; };
  /** The Volvix website, where purchases happen for builds that can't sell (the iPhone app). */
  S.WEB_URL = (window.VOLVIX_CONFIG && window.VOLVIX_CONFIG.webUrl) || 'https://volvix.app';
  /** Same screen on the website, e.g. S.webUrl('#/shop/album/al_atlas'). */
  S.webUrl = function (hash) { hash = String(hash || '#/home'); return S.WEB_URL + '/' + (hash.charAt(0) === '#' ? hash : '#' + hash); };
  /** Button that opens the website (Safari in the iPhone app). Physical records can be sold outside the App Store. */
  S.webButton = function (hash, label, cls) {
    return '<a class="btn ' + (cls || 'btn-primary btn-sm') + ' web-btn" href="' + S.webUrl(hash) + '" target="_blank" rel="noopener">' + S.esc(label) + S.icon('external', 13) + '</a>';
  };
  document.documentElement.setAttribute('data-platform', S.platform);
  // In the iPhone app the page should feel native: no pinch-zoom, and no zoom when a text field gets focus.
  // (The website keeps pinch-zoom for accessibility.)
  if (S.platform === 'ios') {
    const vp = document.querySelector('meta[name="viewport"]');
    if (vp) vp.setAttribute('content', 'width=device-width, initial-scale=1, maximum-scale=1, user-scalable=no, viewport-fit=cover');
  }

  /* ---------- basics ---------- */
  S.$ = (sel, root) => (root || document).querySelector(sel);
  S.$$ = (sel, root) => Array.from((root || document).querySelectorAll(sel));

  S.esc = function (value) {
    return String(value == null ? '' : value)
      .replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;').replace(/'/g, '&#39;');
  };

  S.fmtTime = function (sec) {
    sec = Math.max(0, Math.round(sec || 0));
    return Math.floor(sec / 60) + ':' + String(sec % 60).padStart(2, '0');
  };

  /** Path to the version-history page, works from the live site and from snapshots. */
  S.historyUrl = function () {
    return /\/versions\/v\d+\//.test(location.pathname) ? '../../history.html' : 'history.html';
  };

  /* ---------- links to artists / albums ---------- */
  S.artistSlug = function (name) { return name.toLowerCase().replace(/&/g, 'and').replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, ''); };
  S.artistUrl = function (name) { return '#/artist/' + S.artistSlug(name); };
  /**
   * A link that keeps the surrounding text color. With { inert: true } it's a span with
   * data-go, for use inside elements that are already links or buttons (cards, queue rows).
   */
  S.link = function (href, html, opts) {
    if (opts && opts.inert) return '<span class="link-plain" role="link" tabindex="0" data-go="' + href + '">' + html + '</span>';
    return '<a class="link-plain" href="' + href + '">' + html + '</a>';
  };
  S.artistLink = function (name, opts) { return S.link(S.artistUrl(name), S.esc(name), opts); };
  S.albumLink = function (album, text, opts) { return S.link('#/album/' + album.id, S.esc(text == null ? album.title : text), opts); };
  function go(e) {
    const el = e.target.closest && e.target.closest('[data-go]');
    if (!el) return;
    if (e.type === 'keydown' && e.key !== 'Enter') return;
    e.preventDefault();
    e.stopPropagation();
    location.hash = el.dataset.go.replace(/^#/, '');
  }
  // capture phase so row/card handlers (play on click) never see it
  document.addEventListener('click', go, true);
  document.addEventListener('keydown', go, true);

  /* ---------- icons (24px grid, stroke based) ---------- */
  const F = 'fill="currentColor" stroke="none"';
  const ICONS = {
    home: '<path d="M3 10.5 12 3l9 7.5V20a1 1 0 0 1-1 1h-5v-6h-6v6H4a1 1 0 0 1-1-1z"/>',
    search: '<circle cx="11" cy="11" r="7"/><path d="m20 20-3.5-3.5"/>',
    library: '<path d="M5 4v16M10 4v16"/><path d="m14.5 4.6 4.8 15"/>',
    users: '<circle cx="9" cy="8" r="3.5"/><path d="M2.5 20a6.5 6.5 0 0 1 13 0"/><path d="M16 4.6a3.5 3.5 0 0 1 0 6.8M18 14a6.5 6.5 0 0 1 3.5 6"/>',
    ticket: '<path d="M4 6h16a1 1 0 0 1 1 1v3a2 2 0 0 0 0 4v3a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1v-3a2 2 0 0 0 0-4V7a1 1 0 0 1 1-1z"/><path d="M14.5 6.5v2M14.5 11v2M14.5 15.5v2"/>',
    plus: '<path d="M12 5v14M5 12h14"/>',
    minus: '<path d="M5 12h14"/>',
    play: '<path ' + F + ' d="M7 4.6v14.8a1 1 0 0 0 1.5.86l12.3-7.4a1 1 0 0 0 0-1.72L8.5 3.74A1 1 0 0 0 7 4.6z"/>',
    pause: '<rect ' + F + ' x="6" y="4" width="4" height="16" rx="1"/><rect ' + F + ' x="14" y="4" width="4" height="16" rx="1"/>',
    prev: '<path ' + F + ' d="M19 5.2v13.6a.8.8 0 0 1-1.25.66L8 12.66a.8.8 0 0 1 0-1.32l9.75-6.8A.8.8 0 0 1 19 5.2z"/><path d="M5.5 5v14"/>',
    next: '<path ' + F + ' d="M5 5.2v13.6a.8.8 0 0 0 1.25.66L16 12.66a.8.8 0 0 0 0-1.32L6.25 4.54A.8.8 0 0 0 5 5.2z"/><path d="M18.5 5v14"/>',
    shuffle: '<path d="M16 3h5v5M4 20 21 3M21 16v5h-5M15 15l6 6M4 4l5 5"/>',
    repeat: '<path d="m17 2 4 4-4 4"/><path d="M3 11v-1a4 4 0 0 1 4-4h14"/><path d="m7 22-4-4 4-4"/><path d="M21 13v1a4 4 0 0 1-4 4H3"/>',
    volume: '<path d="M11 5 6 9H3v6h3l5 4z"/><path d="M15.5 8.5a5 5 0 0 1 0 7M18.5 5.5a9.5 9.5 0 0 1 0 13"/>',
    queue: '<path d="M3 6h14M3 12h14M3 18h8"/><path ' + F + ' d="M15 14.5v6l5-3z"/>',
    lyrics: '<path d="M21 15a2 2 0 0 1-2 2H8l-5 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/><path d="M8 8h8M8 12h5"/>',
    devices: '<rect x="2" y="4" width="14" height="10" rx="1.5"/><path d="M6 18h6"/><rect x="18" y="8" width="4" height="12" rx="1"/>',
    download: '<path d="M12 4v11M7 10l5 5 5-5M5 20h14"/>',
    lock: '<rect x="5" y="11" width="14" height="10" rx="2"/><path d="M8 11V7a4 4 0 0 1 8 0v4"/>',
    chevronLeft: '<path d="m15 18-6-6 6-6"/>',
    chevronRight: '<path d="m9 18 6-6-6-6"/>',
    chevronDown: '<path d="m6 9 6 6 6-6"/>',
    settings: '<path d="M4 21v-7M4 10V3M12 21v-9M12 8V3M20 21v-5M20 12V3M1 14h6M9 8h6M17 16h6"/>',
    history: '<path d="M3 12a9 9 0 1 0 2.6-6.4L3 8"/><path d="M3 3v5h5"/><path d="M12 7v5l3 2"/>',
    logout: '<path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4M16 17l5-5-5-5M21 12H9"/>',
    user: '<circle cx="12" cy="8" r="4"/><path d="M4 21a8 8 0 0 1 16 0"/>',
    disc: '<circle cx="12" cy="12" r="9"/><circle cx="12" cy="12" r="2.5"/>',
    music: '<path d="M9 18V5l12-2v13"/><circle cx="6" cy="18" r="3"/><circle cx="18" cy="16" r="3"/>',
    check: '<path d="M20 6 9 17l-5-5"/>',
    x: '<path d="M18 6 6 18M6 6l12 12"/>',
    more: '<circle ' + F + ' cx="5" cy="12" r="1.6"/><circle ' + F + ' cx="12" cy="12" r="1.6"/><circle ' + F + ' cx="19" cy="12" r="1.6"/>',
    eye: '<path d="M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7S2 12 2 12z"/><circle cx="12" cy="12" r="3"/>',
    eyeOff: '<path d="M10.6 5.1A10.4 10.4 0 0 1 12 5c6.5 0 10 7 10 7a17 17 0 0 1-2.2 3.2M6.6 6.6C3.9 8.4 2 12 2 12s3.5 7 10 7a9.7 9.7 0 0 0 5.4-1.6"/><path d="M9.9 9.9a3 3 0 0 0 4.2 4.2M2 2l20 20"/>',
    sun: '<circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"/>',
    moon: '<path d="M21 12.8A9 9 0 1 1 11.2 3a7 7 0 0 0 9.8 9.8z"/>',
    external: '<path d="M14 4h6v6M20 4l-9 9M18 14v5a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1h5"/>',
    compare: '<rect x="3" y="4" width="8" height="16" rx="1.5"/><rect x="13" y="4" width="8" height="16" rx="1.5"/>',
    swap: '<path d="M7 4 3 8l4 4M3 8h14M17 20l4-4-4-4M21 16H7"/>',
    phone: '<rect x="7" y="2" width="10" height="20" rx="2"/><path d="M11 18h2"/>',
    monitor: '<rect x="2" y="4" width="20" height="13" rx="1.5"/><path d="M8 21h8M12 17v4"/>',
    layers: '<path d="m12 3 9 5-9 5-9-5z"/><path d="m3 13 9 5 9-5"/>',
    vinyl: '<circle cx="12" cy="12" r="9"/><circle cx="12" cy="12" r="5.5" stroke-opacity=".45"/><circle cx="12" cy="12" r="1.5"/>',
    bookmark: '<path d="M6 3h12a1 1 0 0 1 1 1v17l-7-4-7 4V4a1 1 0 0 1 1-1z"/>',
    bookmarkOn: '<path fill="currentColor" d="M6 3h12a1 1 0 0 1 1 1v17l-7-4-7 4V4a1 1 0 0 1 1-1z"/>',
    book: '<path d="M4 4.5A1.5 1.5 0 0 1 5.5 3H11v17H5.5A1.5 1.5 0 0 1 4 18.5zM20 4.5A1.5 1.5 0 0 0 18.5 3H13v17h5.5a1.5 1.5 0 0 0 1.5-1.5z"/>',
    shelf: '<path d="M3 20h18M5 20V6M9 20V9M13 20V4M17.5 20l-2.5-12 3-.6 2.4 12"/>',
    grid: '<rect x="3" y="3" width="7" height="7" rx="1"/><rect x="14" y="3" width="7" height="7" rx="1"/><rect x="3" y="14" width="7" height="7" rx="1"/><rect x="14" y="14" width="7" height="7" rx="1"/>',
    import: '<path d="M12 3v12M7 10l5 5 5-5"/><path d="M4 15v4a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-4"/>',
    heart: '<path d="M12 20.5s-7.5-4.8-9.6-9.4C1 8 2.9 4.6 6.2 4.6c1.9 0 3.4 1 4.3 2.4L12 9l1.5-2c.9-1.4 2.4-2.4 4.3-2.4 3.3 0 5.2 3.4 3.8 6.5-2.1 4.6-9.6 9.4-9.6 9.4z"/>',
    heartFill: '<path fill="currentColor" d="M12 20.5s-7.5-4.8-9.6-9.4C1 8 2.9 4.6 6.2 4.6c1.9 0 3.4 1 4.3 2.4L12 9l1.5-2c.9-1.4 2.4-2.4 4.3-2.4 3.3 0 5.2 3.4 3.8 6.5-2.1 4.6-9.6 9.4-9.6 9.4z"/>',
    info: '<circle cx="12" cy="12" r="9"/><path d="M12 11v5.5"/><circle ' + F + ' cx="12" cy="7.6" r="1.15"/>',
    party: '<circle cx="9" cy="8" r="3.5"/><path d="M2.5 20a6.5 6.5 0 0 1 13 0"/><path d="M16 4.6a3.5 3.5 0 0 1 0 6.8M18 14a6.5 6.5 0 0 1 3.5 6"/>',
    partyFill: '<g ' + F + '><circle cx="9" cy="8" r="3.8"/><path d="M1.8 20.6a7.2 7.2 0 0 1 14.4 0z"/><path d="M15.4 4.3a3.8 3.8 0 0 1 0 7.5 4.6 4.6 0 0 0 0-7.5zM17.2 13.6a7.2 7.2 0 0 1 4.9 7h-4.2a9.6 9.6 0 0 0-2-6.2c.4-.4.9-.7 1.3-.8z"/></g>',
    mic: '<rect x="9" y="3" width="6" height="11" rx="3"/><path d="M5 11a7 7 0 0 0 14 0M12 18v3M8.5 21h7"/>',
    grip: '<g ' + F + '><circle cx="9" cy="6" r="1.6"/><circle cx="15" cy="6" r="1.6"/><circle cx="9" cy="12" r="1.6"/><circle cx="15" cy="12" r="1.6"/><circle cx="9" cy="18" r="1.6"/><circle cx="15" cy="18" r="1.6"/></g>',
    cart: '<path d="M3 4h2l2.2 10.2a1 1 0 0 0 1 .8h8.9a1 1 0 0 0 1-.8L20 8H6.2"/><circle cx="9" cy="19" r="1.4"/><circle cx="17" cy="19" r="1.4"/>',
    copy: '<rect x="8" y="8" width="12" height="12" rx="2"/><path d="M16 8V5a1 1 0 0 0-1-1H5a1 1 0 0 0-1 1v10a1 1 0 0 0 1 1h3"/>',
    send: '<path d="M21 3 10 14M21 3l-7 18-4-7-7-4z"/>',
    arrowUp: '<path d="M12 19V5M6 11l6-6 6 6"/>',
    arrowDown: '<path d="M12 5v14M6 13l6 6 6-6"/>',
    bell: '<path d="M6 16V11a6 6 0 0 1 12 0v5l1.5 2h-15z"/><path d="M10 20a2 2 0 0 0 4 0"/>',
    wifi: '<path d="M5 12.6a10 10 0 0 1 14 0M8.5 16.1a5 5 0 0 1 7 0M2 9a15 15 0 0 1 20 0"/><circle fill="currentColor" stroke="none" cx="12" cy="20" r="1"/>',
    wifiOff: '<path d="M2 2l20 20M8.5 16.5a5 5 0 0 1 7 0M5 12.9a10 10 0 0 1 5.2-2.7M19 12.9a10 10 0 0 0-2-1.5M2 8.8a15 15 0 0 1 4.2-2.6M22 8.8A15 15 0 0 0 10.7 5"/><circle ' + F + ' cx="12" cy="20" r="1"/>',
    family: '<circle cx="7" cy="7" r="3"/><circle cx="17" cy="7" r="3"/><circle cx="12" cy="14" r="2.5"/><path d="M2 20a5 5 0 0 1 8.5-3.6M21.9 20a5 5 0 0 0-8.4-3.6M8.5 21a3.5 3.5 0 0 1 7 0"/>'
  };

  S.icon = function (name, size) {
    size = size || 20;
    return '<svg class="icon" width="' + size + '" height="' + size + '" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">' + (ICONS[name] || '') + '</svg>';
  };

  /** Streaming quality as a sound wave: the fuller the wave, the better the sound (Low → Lossless; Hi-Res is
   *  tinted by CSS). Symmetric around the middle, so it reads as audio, not as a signal-strength meter. */
  S.qualityIcon = function (q, size) {
    size = size || 18;
    if (!q || q.id === 'offline') return S.icon('wifiOff', size);
    const level = q.hires || q.lossless ? 4 : ({ low: 1, normal: 2, high: 3 })[q.id] || 3;
    const shape = [0.35, 0.7, 1, 0.55, 0.85, 0.4];          // one fixed waveform…
    const amp = [0, 7, 11, 15, 19][level];                   // …scaled by quality
    let bars = '';
    shape.forEach(function (s, i) {
      const h = Math.max(2.5, s * amp);
      bars += '<rect x="' + (2.6 + i * 3.4) + '" y="' + (12 - h / 2) + '" width="2.2" height="' + h + '" rx="1.1" fill="currentColor" stroke="none"/>';
    });
    return '<svg class="icon q-icon" width="' + size + '" height="' + size + '" viewBox="0 0 24 24" aria-hidden="true">' + bars + '</svg>';
  };

  S.logoMark = function (size) {
    size = size || 28;
    return '<svg class="logo-mark" width="' + size + '" height="' + size + '" viewBox="0 0 32 32" aria-hidden="true">' +
      '<circle cx="16" cy="16" r="15" fill="var(--accent)"/>' +
      '<circle cx="16" cy="16" r="9.5" fill="none" stroke="#000" stroke-opacity=".22"/>' +
      '<circle cx="16" cy="16" r="3.5" fill="var(--logo-hole)"/></svg>';
  };

  S.brand = function (size) {
    return '<span class="brand">' + S.logoMark(size) + '<span>Volvix</span></span>';
  };

  S.appleLogo = function (size) {
    size = size || 18;
    return '<svg width="' + size + '" height="' + size + '" viewBox="0 0 24 24" aria-hidden="true"><path fill="currentColor" d="M16.37 12.6c-.02-2.3 1.88-3.4 1.97-3.46-1.07-1.57-2.74-1.78-3.33-1.8-1.42-.14-2.77.83-3.49.83-.72 0-1.83-.81-3.01-.79-1.55.02-2.98.9-3.78 2.29-1.61 2.8-.41 6.94 1.16 9.21.77 1.11 1.68 2.36 2.88 2.31 1.16-.05 1.6-.75 3-.75 1.4 0 1.79.75 3.01.73 1.25-.02 2.04-1.13 2.8-2.25.88-1.29 1.25-2.54 1.27-2.6-.03-.01-2.43-.93-2.46-3.72zM14.07 5.85c.64-.77 1.07-1.85.95-2.92-.92.04-2.03.61-2.69 1.38-.59.68-1.11 1.77-.97 2.82 1.02.08 2.07-.52 2.71-1.28z"/></svg>';
  };

  S.googleLogo = function (size) {
    size = size || 18;
    return '<svg width="' + size + '" height="' + size + '" viewBox="0 0 48 48" aria-hidden="true">' +
      '<path fill="#EA4335" d="M24 9.5c3.54 0 6.71 1.22 9.21 3.6l6.85-6.85C35.9 2.38 30.47 0 24 0 14.62 0 6.51 5.38 2.56 13.22l7.98 6.19C12.43 13.72 17.74 9.5 24 9.5z"/>' +
      '<path fill="#4285F4" d="M46.98 24.55c0-1.57-.15-3.09-.38-4.55H24v9.02h12.94c-.58 2.96-2.26 5.48-4.78 7.18l7.73 6c4.51-4.18 7.09-10.36 7.09-17.65z"/>' +
      '<path fill="#FBBC05" d="M10.53 28.59c-.48-1.45-.76-2.99-.76-4.59s.27-3.14.76-4.59l-7.98-6.19C.92 16.46 0 20.12 0 24c0 3.88.92 7.54 2.56 10.78l7.97-6.19z"/>' +
      '<path fill="#34A853" d="M24 48c6.48 0 11.93-2.13 15.89-5.81l-7.73-6c-2.15 1.45-4.92 2.3-8.16 2.3-6.26 0-11.57-4.22-13.47-9.91l-7.98 6.19C6.51 42.62 14.62 48 24 48z"/></svg>';
  };

  /* ---------- covers & avatars ---------- */
  /** Generated flat album art — no external images. */
  S.cover = function (album, extraClass) {
    if (!album) return '<div class="cover cover--empty ' + (extraClass || '') + '">' + S.icon('music', 22) + '</div>';
    const a = album.art || {};
    // no cover file: the album's name is printed on generated art (in the background and font someone chose, if they did)
    const look = !album.cover && album.look ? album.look : null;
    const text = !album.cover ? '<span class="cover-title">' + S.esc(album.title) + '</span><span class="cover-artist">' + S.esc(album.artist) + '</span>' : '';
    // has a cover file: a plain tile while it loads, so the generated art doesn't flash first; it fades in once decoded.
    // Only if the picture fails does the generated art show.
    const real = album.cover && S.server;
    return '<div class="cover cover--' + (look ? 'custom' : (a.variant || 'plain')) + (real ? ' cover--loading' : '') + ' ' + (extraClass || '') + '" style="--c1:' + a.c1 + ';--c2:' + a.c2 + ';--fg:' + a.fg + '"' +
      (look ? ' data-bg="' + S.esc(look.bg) + '" data-font="' + S.esc(look.font) + '"' : '') + '>' + text +
      (real ? '<img class="cover-img" src="' + S.server.coverUrl(album.id) + '" alt="" loading="lazy" decoding="async" onload="this.parentNode.classList.remove(\'cover--loading\')" onerror="this.parentNode.classList.remove(\'cover--loading\');this.remove()">' : '') +
      '</div>';
  };

  /** A collage of up to four album covers: 4 = a 2×2 grid, 3 = one big and two small, 2 = side by side, 1 = just that cover. */
  S.collage = function (albumIds, extraClass) {
    const albums = albumIds.map(function (id) { return S.data.album(id); }).filter(Boolean).slice(0, 4);
    if (albums.length >= 2) {
      return '<div class="mosaic mosaic-' + albums.length + ' ' + (extraClass || '') + '">' + albums.map(function (a) { return S.cover(a); }).join('') + '</div>';
    }
    if (albums.length) return S.cover(albums[0], extraClass);
    return '<div class="cover cover--empty ' + (extraClass || '') + '">' + S.icon('music', 22) + '</div>';
  };

  /** A playlist's cover: a collage of the albums its songs come from (Spotify-style). */
  S.playlistCover = function (playlist, extraClass) {
    // Liked Songs has its own cover: a heart on a red-to-purple wash
    if (playlist && playlist.liked) {
      return '<div class="cover cover--liked ' + (extraClass || '') + '" aria-hidden="true">' + S.icon('heartFill', 40) + '</div>';
    }
    const albums = [];
    (playlist.tracks || []).forEach(function (entry) {
      const t = S.data.track(entry.id || entry);
      if (t && albums.indexOf(t.albumId) === -1) albums.push(t.albumId);
    });
    return S.collage(albums, extraClass);
  };

  const AVATAR_COLORS = ['#5E548E', '#335C67', '#9E2A2B', '#BC6C25', '#3D5A80', '#6B4F3A', '#4A6C2F', '#7A4069'];
  /** Someone's uploaded picture, if they have one (me, or a household member by server id). */
  function pfpFor(user) {
    const D = S.data;
    if (!D || !D.me) return null;
    const id = user.id === 'u_me' ? D.me.serverId : user.id;
    const v = id === D.me.serverId ? D.me.avatar : (D.people[id] || {}).avatar;
    return v ? '/api/avatar/' + encodeURIComponent(id) + '?v=' + v : null;
  }
  S.avatar = function (user, size) {
    size = size || 32;
    if (!user) return '';
    const style = 'width:' + size + 'px;height:' + size + 'px;font-size:' + Math.round(size * 0.4) + 'px';
    const pic = user.pfp || pfpFor(user);
    if (pic) return '<span class="avatar" style="' + style + '"><img src="' + S.esc(pic) + '" alt=""></span>';
    let hash = 0;
    for (const ch of user.id || user.name || '') hash = (hash * 31 + ch.charCodeAt(0)) >>> 0;
    const initials = (user.name || '?').split(/\s+/).map(function (p) { return p[0]; }).slice(0, 2).join('').toUpperCase();
    return '<span class="avatar" style="' + style + ';background:' + AVATAR_COLORS[hash % AVATAR_COLORS.length] + '">' + S.esc(initials) + '</span>';
  };

  /** Seconds since 1970 -> "Mar 4, 2026" (this year drops the year). */
  S.fmtDay = function (sec) {
    if (!sec) return '';
    const d = new Date(sec * 1000), now = new Date();
    return d.toLocaleDateString(undefined, d.getFullYear() === now.getFullYear() ? { month: 'short', day: 'numeric' } : { month: 'short', day: 'numeric', year: 'numeric' });
  };

  /* ---------- toasts ---------- */
  S.toast = function (message, icon) {
    const host = document.getElementById('toasts');
    if (!host) return;
    const el = document.createElement('div');
    el.className = 'toast';
    el.innerHTML = (icon ? S.icon(icon, 16) : '') + '<span>' + S.esc(message) + '</span>';
    host.appendChild(el);
    setTimeout(function () { el.classList.add('leaving'); }, 2400);
    setTimeout(function () { el.remove(); }, 2700);
  };

  const FOCUSABLE = 'a[href], button:not([disabled]), input:not([disabled]):not([type="hidden"]), select, textarea, [tabindex]:not([tabindex="-1"])';

  /* ---------- modal ---------- */
  /**
   * S.modal({ title, body (html), actions: [{ label, kind, onClick(close) }] })
   * Returns { el, close }.
   */
  S.modal = function (opts) {
    const scrim = document.createElement('div');
    const titleId = 'm' + Math.random().toString(36).slice(2, 8);
    const opener = document.activeElement;
    scrim.className = 'modal-scrim';
    scrim.innerHTML = '<div class="modal" role="dialog" aria-modal="true"' +
        (opts.title ? ' aria-labelledby="' + titleId + '"' : '') + '>' +
      (opts.title ? '<h2 id="' + titleId + '">' + S.esc(opts.title) + '</h2>' : '') +
      '<div class="modal-body">' + (opts.body || '') + '</div>' +
      '<div class="modal-actions"></div></div>';
    const actions = scrim.querySelector('.modal-actions');
    const openedOn = location.hash;
    let closed = false;
    function close() {
      if (closed) return;
      closed = true;
      document.removeEventListener('keydown', onKey);
      window.removeEventListener('hashchange', onNav);
      scrim.remove();
      // hand focus back to whatever opened the dialog (if it's still on the page)
      const top = S.$$('.modal-scrim').pop();
      if (top) { const f = top.querySelector(FOCUSABLE); if (f) f.focus(); }
      else if (opener && opener.focus && document.body.contains(opener)) opener.focus();
    }
    function onKey(e) {
      // only the top-most dialog reacts
      if (S.$$('.modal-scrim').pop() !== scrim) return;
      if (e.key === 'Escape') { if (!document.querySelector('.popover')) close(); return; }
      if (e.key !== 'Tab') return;
      // keep Tab / Shift+Tab inside the dialog
      const items = S.$$(FOCUSABLE, scrim).filter(function (el) { return el.offsetParent !== null; });
      if (!items.length) return;
      const first = items[0], last = items[items.length - 1];
      if (e.shiftKey && (document.activeElement === first || !scrim.contains(document.activeElement))) { e.preventDefault(); last.focus(); }
      else if (!e.shiftKey && (document.activeElement === last || !scrim.contains(document.activeElement))) { e.preventDefault(); first.focus(); }
    }
    // following a link inside closes the dialog (but not one opened right after a navigation)
    function onNav() { if (location.hash !== openedOn) close(); }
    window.addEventListener('hashchange', onNav);
    (opts.actions || [{ label: 'Close', kind: 'secondary' }]).forEach(function (a) {
      const b = document.createElement('button');
      b.className = 'btn btn-' + (a.kind || 'secondary');
      b.textContent = a.label;
      b.addEventListener('click', function () { if (a.onClick) a.onClick(close); else close(); });
      actions.appendChild(b);
    });
    scrim.addEventListener('mousedown', function (e) { if (e.target === scrim) close(); });
    document.addEventListener('keydown', onKey);
    document.body.appendChild(scrim);
    const first = scrim.querySelector('input, button');
    if (first) first.focus();
    return { el: scrim, close: close };
  };

  /* ---------- popover menu ---------- */
  /**
   * S.popover(anchor, { header (html), items: [{ label, icon, onClick, sep }] })
   */
  let openPopover = null;
  S.popover = function (anchor, opts) {
    if (openPopover) { const same = openPopover.anchor === anchor; openPopover.close(); if (same) return; }
    const el = document.createElement('div');
    el.className = 'popover';
    el.setAttribute('role', 'menu');
    el.innerHTML = (opts.header ? '<div class="popover-head">' + opts.header + '</div><div class="popover-sep"></div>' : '');
    opts.items.filter(Boolean).forEach(function (item) {
      if (item.sep) { el.insertAdjacentHTML('beforeend', '<div class="popover-sep"></div>'); return; }
      const b = document.createElement('button');
      b.className = 'popover-item' + (item.danger ? ' danger' : '');
      b.setAttribute('role', 'menuitem');
      b.innerHTML = (item.icon ? S.icon(item.icon, 18) : '') + '<span>' + S.esc(item.label) + '</span>';
      b.addEventListener('click', function () { close(true); if (item.onClick) item.onClick(); });
      el.appendChild(b);
    });
    document.body.appendChild(el);
    anchor.setAttribute('aria-expanded', 'true');

    const r = anchor.getBoundingClientRect();
    const w = el.offsetWidth, h = el.offsetHeight;
    let left = Math.min(r.right - w, window.innerWidth - w - 8);
    left = Math.max(8, left);
    let top = r.bottom + 6;
    if (top + h > window.innerHeight - 8) top = Math.max(8, r.top - h - 6);
    el.style.left = left + 'px';
    el.style.top = top + 'px';

    function onDoc(e) { if (!el.contains(e.target) && !anchor.contains(e.target)) close(); }
    // menu keyboard: arrows / Home / End move, Escape closes, Tab leaves
    function onKey(e) {
      const items = S.$$('.popover-item', el);
      const i = items.indexOf(document.activeElement);
      if (e.key === 'Escape') { e.preventDefault(); close(true); }
      else if (e.key === 'ArrowDown') { e.preventDefault(); (items[i + 1] || items[0]).focus(); }
      else if (e.key === 'ArrowUp') { e.preventDefault(); (items[i - 1] || items[items.length - 1]).focus(); }
      else if (e.key === 'Home') { e.preventDefault(); items[0].focus(); }
      else if (e.key === 'End') { e.preventDefault(); items[items.length - 1].focus(); }
      else if (e.key === 'Tab') close();
    }
    let closed = false;
    function close(returnFocus) {
      if (closed) return;
      closed = true;
      anchor.setAttribute('aria-expanded', 'false');
      if (returnFocus && document.body.contains(anchor)) anchor.focus();
      el.remove();
      document.removeEventListener('mousedown', onDoc);
      document.removeEventListener('keydown', onKey);
      window.removeEventListener('hashchange', onNav);
      if (openPopover && openPopover.el === el) openPopover = null;
    }
    const openedOn = location.hash;
    function onNav() { if (location.hash !== openedOn) close(); }
    window.addEventListener('hashchange', onNav);
    setTimeout(function () { document.addEventListener('mousedown', onDoc); }, 0);
    document.addEventListener('keydown', onKey);
    const firstItem = el.querySelector('.popover-item');
    if (firstItem) firstItem.focus({ preventScroll: true });
    openPopover = { anchor: anchor, close: close, el: el };
    return openPopover;
  };
})(window.Spin);
