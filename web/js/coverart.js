/* Volvix Home — covers for albums that have no cover art: the name is printed on a generated background. Anyone can
   pick a different background and font from a short list (the same for everyone in the household). Albums with real
   cover art are never touched. */
window.Spin = window.Spin || {};
(function (S) {
  'use strict';

  // The look of each choice is in styles.css ([data-bg=…] and [data-font=…]); these are the names the picker shows.
  S.coverBackgrounds = [
    ['sunset', 'Sunset'], ['ocean', 'Ocean'], ['midnight', 'Midnight'], ['forest', 'Forest'], ['grape', 'Grape'], ['rose', 'Rose'],
    ['ember', 'Ember'], ['mono', 'Graphite'], ['paper', 'Paper'], ['stripes', 'Stripes'], ['rings', 'Rings'], ['grid', 'Grid'],
    ['sunburst', 'Sunburst'], ['checker', 'Checker'], ['aurora', 'Aurora'], ['dusk', 'Dusk']
  ];
  S.coverFonts = [
    ['display', 'Modern'], ['serif', 'Serif'], ['mono', 'Typewriter'], ['rounded', 'Rounded'], ['condensed', 'Poster'], ['script', 'Script'], ['slab', 'Slab']
  ];

  /** The picker: live preview, background swatches, font choices. Save shares it with the household. */
  S.coverDialog = function (albumId) {
    const album = S.data.album(albumId);
    if (!album) return;
    if (album.cover) { S.toast('This album has its own cover art', 'x'); return; }
    let look = album.look ? { bg: album.look.bg, font: album.look.font } : { bg: S.coverBackgrounds[0][0], font: 'display' };
    const preview = function () { return S.cover(Object.assign({}, album, { look: look, cover: false })); };
    const body = function () {
      return '<div class="cv-dlg"><div class="cv-preview" data-cv-preview>' + preview() + '</div><div class="cv-pick">' +
        '<div class="q-label">Background</div><div class="cv-swatches" role="radiogroup" aria-label="Background">' + S.coverBackgrounds.map(function (b) {
          return '<button class="cv-swatch' + (look.bg === b[0] ? ' on' : '') + '" data-cv-bg="' + b[0] + '" role="radio" aria-checked="' + (look.bg === b[0]) + '" title="' + b[1] + '" aria-label="' + b[1] + '"><span class="cover cover--custom" data-bg="' + b[0] + '"></span></button>';
        }).join('') + '</div>' +
        '<div class="q-label">Font</div><div class="cv-fonts" role="radiogroup" aria-label="Font">' + S.coverFonts.map(function (f) {
          return '<button class="cv-font' + (look.font === f[0] ? ' on' : '') + '" data-cv-font="' + f[0] + '" role="radio" aria-checked="' + (look.font === f[0]) + '"><span data-font="' + f[0] + '">' + f[1] + '</span></button>';
        }).join('') + '</div></div></div>';
    };
    const m = S.modal({
      title: 'Customize cover',
      body: body(),
      actions: [
        { label: 'Cancel' },
        { label: 'Use the automatic look', kind: 'secondary', onClick: function (close) {
          fetch('/api/album/' + encodeURIComponent(albumId) + '/art', { method: 'DELETE', credentials: 'same-origin' }).then(function () { album.look = null; close(); S.app.refresh(); S.toast('Back to the automatic look', 'check'); });
        } },
        { label: 'Save', kind: 'primary', onClick: function (close) {
          S.server.json('PUT', '/api/album/' + encodeURIComponent(albumId) + '/art', look).then(function () { album.look = { bg: look.bg, font: look.font }; close(); S.app.refresh(); S.toast('Cover saved for everyone', 'check'); })
            .catch(function (x) { S.toast(x.message, 'x'); });
        } }
      ]
    });
    m.el.addEventListener('click', function (e) {
      const bg = e.target.closest('[data-cv-bg]'), ft = e.target.closest('[data-cv-font]');
      if (!bg && !ft) return;
      if (bg) look.bg = bg.dataset.cvBg; else look.font = ft.dataset.cvFont;
      m.el.querySelector('.modal-body, [data-cv-preview]') && (m.el.querySelector('[data-cv-preview]').innerHTML = preview());
      m.el.querySelectorAll('[data-cv-bg]').forEach(function (b) { const on = b.dataset.cvBg === look.bg; b.classList.toggle('on', on); b.setAttribute('aria-checked', on); });
      m.el.querySelectorAll('[data-cv-font]').forEach(function (b) { const on = b.dataset.cvFont === look.font; b.classList.toggle('on', on); b.setAttribute('aria-checked', on); });
    });
  };
})(window.Spin);
