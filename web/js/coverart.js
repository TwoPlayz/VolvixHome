/* Volvix Home — drawn covers: for an album that has no cover art, or a playlist you dress up yourself, the name is
   printed on a generated background. You choose the background, its two colours, the font and the text colour.
   For albums the choice is the same for everyone in the household. Albums with real cover art are never touched;
   an album without art can also be given a picture instead (Change picture). */
window.Spin = window.Spin || {};
(function (S) {
  'use strict';

  S.coverBackgrounds = [
    ['sunset', 'Sunset'], ['ocean', 'Ocean'], ['midnight', 'Midnight'], ['forest', 'Forest'], ['grape', 'Grape'], ['rose', 'Rose'],
    ['ember', 'Ember'], ['mono', 'Graphite'], ['paper', 'Paper'], ['stripes', 'Stripes'], ['rings', 'Rings'], ['grid', 'Grid'],
    ['sunburst', 'Sunburst'], ['checker', 'Checker'], ['aurora', 'Aurora'], ['dusk', 'Dusk']
  ];
  S.coverFonts = [
    ['display', 'Modern'], ['serif', 'Serif'], ['mono', 'Typewriter'], ['rounded', 'Rounded'], ['condensed', 'Poster'], ['script', 'Script'], ['slab', 'Slab']
  ];

  /* What each background is made of: its kind, its own two colours, its text colour (and an angle for the gradients).
     The same table is in the Windows app. Your own colours replace the two defaults. */
  const PRESETS = {
    sunset: ['linear', '#ff6a3d', '#ffd89b', '#3a1200', 135], ocean: ['linear', '#0f4c75', '#7fd1e8', '#ffffff', 160],
    midnight: ['linear', '#0b132b', '#3a506b', '#e8f1ff', 160], forest: ['linear', '#0f3d2e', '#8fc98a', '#f3fff0', 150],
    grape: ['linear', '#3c1053', '#c99bff', '#ffffff', 145], rose: ['linear', '#ffd1dc', '#ff6f91', '#4a0f22', 145],
    ember: ['radial', '#ffcf5a', '#8a1c0d', '#ffffff'], mono: ['linear', '#2b2b2e', '#55555c', '#f4f4f5', 160],
    paper: ['solid', '#f1e9db', '#f1e9db', '#2a241c'], stripes: ['stripes', '#e5484d', '#c23a3f', '#ffffff'],
    rings: ['rings', '#1d3557', '#274b7a', '#f1faee'], grid: ['grid', '#101820', '#3a4650', '#e7f6ff'],
    sunburst: ['sunburst', '#ffd23f', '#ff9f1c', '#2b1800'], checker: ['checker', '#222222', '#eeeeee', '#ffffff'],
    aurora: ['linear', '#0d1b2a', '#e0a1ff', '#ffffff', 200], dusk: ['linear', '#2b1055', '#ffc3a0', '#ffffff', 180]
  };
  S.lookDefaults = function (bg) { const p = PRESETS[bg] || PRESETS.sunset; return { c1: p[1], c2: p[2], ink: p[3] }; };

  const mix = function (a, b) { return 'color-mix(in srgb, ' + a + ' 50%, ' + b + ')'; };
  /** The CSS for a background in two colours. */
  function bgCss(bg, c1, c2) {
    const p = PRESETS[bg] || PRESETS.sunset;
    switch (p[0]) {
      case 'linear': return 'background:linear-gradient(' + p[4] + 'deg,' + c1 + ',' + mix(c1, c2) + ' 55%,' + c2 + ')';
      case 'radial': return 'background:radial-gradient(circle at 30% 20%,' + c1 + ',' + mix(c1, c2) + ' 45%,' + c2 + ')';
      case 'solid': return 'background:' + c1;
      case 'stripes': return 'background:repeating-linear-gradient(45deg,' + c1 + ' 0 14px,' + c2 + ' 14px 28px)';
      case 'rings': return 'background:repeating-radial-gradient(circle at 70% 30%,' + c1 + ' 0 12px,' + c2 + ' 12px 24px)';
      case 'grid': return 'background-color:' + c1 + ';background-image:linear-gradient(' + c2 + ' 1px,transparent 1px),linear-gradient(90deg,' + c2 + ' 1px,transparent 1px);background-size:22px 22px';
      case 'sunburst': return 'background:repeating-conic-gradient(from 0deg at 50% 38%,' + c1 + ' 0 10deg,' + c2 + ' 10deg 20deg)';
      default: return 'background:conic-gradient(' + c1 + ' 25%,' + c2 + ' 0 50%,' + c1 + ' 0 75%,' + c2 + ' 0) 0 0 / 40px 40px';
    }
  }
  /** Inline style for a look with colours of its own ('' when it just uses the preset). */
  function lookStyle(look) {
    if (!look || !(look.c1 || look.c2 || look.ink)) return '';
    const d = S.lookDefaults(look.bg);
    return bgCss(look.bg, look.c1 || d.c1, look.c2 || d.c2) + ';color:' + (look.ink || d.ink);
  }

  /** A drawn cover: the title (and a second line) on the chosen background. */
  S.lookTile = function (look, title, sub, extraClass) {
    const st = lookStyle(look);
    return '<div class="cover cover--custom ' + (extraClass || '') + '" data-bg="' + S.esc(look.bg) + '" data-font="' + S.esc(look.font) + '"' + (st ? ' style="' + S.esc(st) + '"' : '') + '>' +
      '<span class="cover-title">' + S.esc(title || '') + '</span>' + (sub ? '<span class="cover-artist">' + S.esc(sub) + '</span>' : '') + '</div>';
  };

  /**
   * The picker: live preview, background swatches (drawn in your two colours), colours, font, text colour.
   * opts: { name, sub, look, note, onSave(look) -> Promise, onReset() -> Promise }
   */
  S.lookDialog = function (opts) {
    let look = opts.look ? { bg: opts.look.bg, font: opts.look.font, c1: opts.look.c1 || null, c2: opts.look.c2 || null, ink: opts.look.ink || null }
      : { bg: S.coverBackgrounds[0][0], font: 'display', c1: null, c2: null, ink: null };
    const preview = function () { return S.lookTile(look, opts.name, opts.sub, ''); };
    const colourOf = function (k) { return look[k] || S.lookDefaults(look.bg)[k]; };
    const swatch = function (b) {
      return '<button class="cv-swatch' + (look.bg === b[0] ? ' on' : '') + '" data-cv-bg="' + b[0] + '" role="radio" aria-checked="' + (look.bg === b[0]) + '" title="' + b[1] + '" aria-label="' + b[1] + '">' +
        S.lookTile({ bg: b[0], font: 'display', c1: look.c1, c2: look.c2 }, '', '', '').replace('<span class="cover-title"></span>', '') + '</button>';
    };
    const body = function () {
      return '<div class="cv-dlg"><div class="cv-preview" data-cv-preview>' + preview() + '</div><div class="cv-pick">' +
        '<div class="q-label">Background</div><div class="cv-swatches" role="radiogroup" aria-label="Background" data-cv-swatches>' + S.coverBackgrounds.map(swatch).join('') + '</div>' +
        '<div class="q-label">Colours</div><div class="cv-colours">' +
          '<label class="cv-colour"><input type="color" data-cv-c="c1" value="' + colourOf('c1') + '"><span>Primary</span></label>' +
          '<label class="cv-colour"><input type="color" data-cv-c="c2" value="' + colourOf('c2') + '"><span>Secondary</span></label>' +
          '<label class="cv-colour"><input type="color" data-cv-c="ink" value="' + colourOf('ink') + '"><span>Text</span></label>' +
          '<button class="btn btn-ghost btn-sm" type="button" data-cv-reset-colours>Reset colours</button></div>' +
        '<div class="q-label">Font</div><div class="cv-fonts" role="radiogroup" aria-label="Font">' + S.coverFonts.map(function (f) {
          return '<button class="cv-font' + (look.font === f[0] ? ' on' : '') + '" data-cv-font="' + f[0] + '" role="radio" aria-checked="' + (look.font === f[0]) + '"><span data-font="' + f[0] + '">' + f[1] + '</span></button>';
        }).join('') + '</div>' + (opts.note ? '<p class="set-hint">' + opts.note + '</p>' : '') + '</div></div>';
    };
    const payload = function () { return { bg: look.bg, font: look.font, c1: look.c1, c2: look.c2, ink: look.ink }; };
    const m = S.modal({
      title: 'Customize cover',
      body: body(),
      actions: [
        { label: 'Cancel' },
        { label: 'Use the automatic look', kind: 'secondary', onClick: function (close) { Promise.resolve(opts.onReset()).then(function () { close(); }).catch(function (x) { S.toast(x.message, 'x'); }); } },
        { label: 'Save', kind: 'primary', onClick: function (close) { Promise.resolve(opts.onSave(payload())).then(function () { close(); }).catch(function (x) { S.toast(x.message, 'x'); }); } }
      ]
    });
    function redraw(full) {
      m.el.querySelector('[data-cv-preview]').innerHTML = preview();
      m.el.querySelector('[data-cv-swatches]').innerHTML = S.coverBackgrounds.map(swatch).join('');
      m.el.querySelectorAll('[data-cv-font]').forEach(function (b) { const on = b.dataset.cvFont === look.font; b.classList.toggle('on', on); b.setAttribute('aria-checked', on); });
      if (full) m.el.querySelectorAll('[data-cv-c]').forEach(function (i) { i.value = colourOf(i.dataset.cvC); });
    }
    m.el.addEventListener('click', function (e) {
      const bg = e.target.closest('[data-cv-bg]'), ft = e.target.closest('[data-cv-font]');
      if (e.target.closest('[data-cv-reset-colours]')) { look.c1 = look.c2 = look.ink = null; redraw(true); return; }
      if (!bg && !ft) return;
      if (bg) look.bg = bg.dataset.cvBg; else look.font = ft.dataset.cvFont;
      redraw(!!bg);
    });
    // picking a colour changes the preview and every background option straight away
    m.el.addEventListener('input', function (e) {
      const i = e.target.closest('[data-cv-c]');
      if (!i) return;
      look[i.dataset.cvC] = i.value;
      redraw(false);
    });
  };

  /** An album with no art: choose how it is drawn (shared by the household). */
  S.coverDialog = function (albumId) {
    const album = S.data.album(albumId);
    if (!album) return;
    if (album.cover) { S.toast('This album has its own cover art', 'x'); return; }
    S.lookDialog({
      name: album.title, sub: album.artist, look: album.look,
      onSave: function (look) {
        return S.server.json('PUT', '/api/album/' + encodeURIComponent(albumId) + '/art', look).then(function () { album.look = look; S.app.refresh(); S.toast('Cover saved for everyone', 'check'); });
      },
      onReset: function () {
        return fetch('/api/album/' + encodeURIComponent(albumId) + '/art', { method: 'DELETE', credentials: 'same-origin' }).then(function () { album.look = null; S.app.refresh(); S.toast('Back to the automatic look', 'check'); });
      }
    });
  };

  /** A picture for an album that has no art: adjust it, then it is kept on the server for everyone. */
  S.albumPicture = {
    choose: function (albumId) {
      const album = S.data.album(albumId);
      if (!album || (album.cover && !album.customCover)) { S.toast('This album has its own cover art', 'x'); return; }
      S.pickPicture({
        title: 'Adjust the picture', out: 800, saveLabel: 'Use this picture',
        onSave: function (blob) {
          return fetch('/api/album/' + encodeURIComponent(albumId) + '/picture', { method: 'PUT', credentials: 'same-origin', headers: { 'Content-Type': 'image/jpeg' }, body: blob })
            .then(function (r) { return r.json().then(function (j) { if (!r.ok) throw new Error(j.error || 'Couldn\'t save the picture'); return j; }); })
            .then(function (r) { album.cover = true; album.customCover = true; album.coverV = r.coverV; S.app.refresh(); S.toast('Picture updated for everyone', 'check'); });
        }
      });
    },
    remove: function (albumId) {
      const album = S.data.album(albumId);
      fetch('/api/album/' + encodeURIComponent(albumId) + '/picture', { method: 'DELETE', credentials: 'same-origin' }).then(function () {
        if (album) { album.cover = false; album.customCover = false; album.coverV = 0; }
        S.app.refresh(); S.toast('Picture removed', 'check');
      });
    }
  };
})(window.Spin);
