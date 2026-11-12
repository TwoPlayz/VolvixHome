/* Volvix Home — adjust a picture before it is used: move it (drag) and zoom it (slider, mouse wheel, pinch or + / -)
   until the square shows what you want. What the square shows is exactly what is saved. Used for playlist pictures and
   for albums that have no art. (Your profile picture has its own round version in views/account.js.) */
window.Spin = window.Spin || {};
(function (S) {
  'use strict';

  /**
   * file: the chosen image file.  opts: { title, out (px, default 512), saveLabel, onSave(blob) -> Promise }.
   * Resolves nothing; errors from onSave are shown and the dialog stays open.
   */
  S.cropPicture = function (file, opts) {
    opts = opts || {};
    const img = new Image();
    const url = URL.createObjectURL(file);
    img.onerror = function () { URL.revokeObjectURL(url); S.toast('Couldn\'t read that picture.', 'x'); };
    img.onload = function () {
      const SIZE = 300, OUT = opts.out || 512, R = 14; // R: the corner the saved art is shown with
      const min = SIZE / Math.min(img.width, img.height); // smallest zoom: the photo still fills the square
      let scale = min, ox = (SIZE - img.width * min) / 2, oy = (SIZE - img.height * min) / 2;
      const m = S.modal({
        title: opts.title || 'Adjust the picture',
        body: '<div class="crop"><canvas class="crop-canvas crop-square" width="' + SIZE + '" height="' + SIZE + '" tabindex="0" role="img" aria-label="The picture. Drag to move it, use plus and minus to zoom."></canvas>' +
          '<label class="crop-zoom"><span>' + S.icon('minus', 14) + '</span><input type="range" min="1" max="4" step="0.01" value="1" aria-label="Zoom" data-crop-zoom><span>' + S.icon('plus', 14) + '</span></label>' +
          '<p class="set-hint">Drag to move. Scroll, pinch or use the slider to zoom.</p></div>',
        actions: [{ label: 'Cancel', onClick: function (close) { URL.revokeObjectURL(url); close(); } },
          { label: opts.saveLabel || 'Use this picture', kind: 'primary', onClick: function (close) { save(close); } }]
      });
      const cv = m.el.querySelector('canvas'), ctx = cv.getContext('2d'), zoom = m.el.querySelector('[data-crop-zoom]');
      const clamp = function () { ox = Math.min(0, Math.max(SIZE - img.width * scale, ox)); oy = Math.min(0, Math.max(SIZE - img.height * scale, oy)); };
      function roundRect(c, x, y, w, h, r) { c.moveTo(x + r, y); c.arcTo(x + w, y, x + w, y + h, r); c.arcTo(x + w, y + h, x, y + h, r); c.arcTo(x, y + h, x, y, r); c.arcTo(x, y, x + w, y, r); c.closePath(); }
      function draw() {
        clamp();
        ctx.clearRect(0, 0, SIZE, SIZE);
        ctx.drawImage(img, ox, oy, img.width * scale, img.height * scale);
        // the corners are rounded on the saved art everywhere, so show them rounded here too
        ctx.save(); ctx.fillStyle = 'rgba(0,0,0,.58)'; ctx.beginPath(); ctx.rect(0, 0, SIZE, SIZE); roundRect(ctx, 3, 3, SIZE - 6, SIZE - 6, R); ctx.fill('evenodd'); ctx.restore();
        ctx.strokeStyle = 'rgba(255,255,255,.85)'; ctx.lineWidth = 2; ctx.beginPath(); roundRect(ctx, 3, 3, SIZE - 6, SIZE - 6, R); ctx.stroke();
      }
      function zoomTo(s, px, py) {
        s = Math.min(min * 4, Math.max(min, s));
        const k = s / scale;
        ox = px - (px - ox) * k; oy = py - (py - oy) * k; scale = s;
        zoom.value = String(scale / min);
        draw();
      }
      zoom.addEventListener('input', function () { zoomTo(min * Number(zoom.value), SIZE / 2, SIZE / 2); });
      cv.addEventListener('wheel', function (e) { e.preventDefault(); const r = cv.getBoundingClientRect(); zoomTo(scale * (e.deltaY < 0 ? 1.08 : 1 / 1.08), (e.clientX - r.left) * SIZE / r.width, (e.clientY - r.top) * SIZE / r.height); }, { passive: false });
      cv.addEventListener('keydown', function (e) {
        const step = 12;
        if (e.key === 'ArrowLeft') ox += step; else if (e.key === 'ArrowRight') ox -= step; else if (e.key === 'ArrowUp') oy += step; else if (e.key === 'ArrowDown') oy -= step;
        else if (e.key === '+' || e.key === '=') zoomTo(scale * 1.1, SIZE / 2, SIZE / 2); else if (e.key === '-') zoomTo(scale / 1.1, SIZE / 2, SIZE / 2);
        else return;
        e.preventDefault(); draw();
      });
      const pts = new Map();
      const local = function (e) { const r = cv.getBoundingClientRect(); return [(e.clientX - r.left) * SIZE / r.width, (e.clientY - r.top) * SIZE / r.height]; };
      cv.addEventListener('pointerdown', function (e) { try { cv.setPointerCapture(e.pointerId); } catch (x) { /* synthetic pointer */ } pts.set(e.pointerId, local(e)); cv.classList.add('grabbing'); });
      cv.addEventListener('pointermove', function (e) {
        if (!pts.has(e.pointerId)) return;
        const p = local(e), old = pts.get(e.pointerId);
        if (pts.size === 1) { ox += p[0] - old[0]; oy += p[1] - old[1]; pts.set(e.pointerId, p); draw(); return; }
        const other = Array.from(pts.entries()).find(function (en) { return en[0] !== e.pointerId; })[1];
        const d0 = Math.hypot(old[0] - other[0], old[1] - other[1]), d1 = Math.hypot(p[0] - other[0], p[1] - other[1]);
        pts.set(e.pointerId, p);
        if (d0 > 8) zoomTo(scale * d1 / d0, (p[0] + other[0]) / 2, (p[1] + other[1]) / 2);
      });
      const up = function (e) { pts.delete(e.pointerId); if (!pts.size) cv.classList.remove('grabbing'); };
      cv.addEventListener('pointerup', up); cv.addEventListener('pointercancel', up);
      draw();

      function save(close) {
        const out = document.createElement('canvas'); out.width = out.height = OUT;
        const sw = SIZE / scale; // how much of the photo the square covers, in the photo's own pixels
        out.getContext('2d').drawImage(img, -ox / scale, -oy / scale, sw, sw, 0, 0, OUT, OUT);
        out.toBlob(function (blob) {
          Promise.resolve(opts.onSave ? opts.onSave(blob) : null).then(function () { URL.revokeObjectURL(url); close(); }).catch(function (x) { S.toast(x && x.message || 'Couldn\'t save the picture.', 'x'); });
        }, 'image/jpeg', 0.9);
      }
    };
    img.src = url;
  };

  /** Ask for a picture file, then open the adjust dialog for it. */
  S.pickPicture = function (opts) {
    const inp = document.createElement('input'); inp.type = 'file'; inp.accept = 'image/*';
    inp.onchange = function () { const f = inp.files && inp.files[0]; if (f) S.cropPicture(f, opts); };
    inp.click();
  };
})(window.Spin);
