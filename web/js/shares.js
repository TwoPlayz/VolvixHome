/* Volvix Home — share links: a link to an album or playlist that opens a tiny player for anyone who has it,
   for 1, 7 or 30 days (see server shares.rs). Make one from an album's or playlist's ⋯ menu; cancel them in Settings. */
window.Spin = window.Spin || {};
(function (S) {
  'use strict';

  const full = function (path) { return location.origin + path; };
  const left = function (exp) {
    const d = Math.max(0, exp - Date.now() / 1000), days = Math.round(d / 86400 * 10) / 10 >= 1 ? Math.round(d / 86400) : 0;
    return days >= 1 ? days + ' day' + (days > 1 ? 's' : '') + ' left' : Math.max(1, Math.round(d / 3600)) + ' hours left';
  };

  /** Ask how long, make the link, show it with a Copy button. */
  S.shareDialog = function (kind, name, trackIds) {
    if (!S.data.can('share')) { S.toast('You don\'t have permission to make share links. Ask an admin.', 'lock'); return; }
    if (!trackIds.length) { S.toast('There\'s nothing in it to share yet', 'x'); return; }
    let days = 7;
    const m = S.modal({
      title: 'Share ' + (kind === 'album' ? 'this album' : 'this playlist'),
      body: '<div class="share-dlg"><p class="modal-text">Anyone with the link can play <strong>' + S.esc(name) + '</strong> (' + trackIds.length + ' song' + (trackIds.length === 1 ? '' : 's') +
        ') in their browser. They can\'t see anything else on your server, and you can cancel the link any time in Settings.</p>' +
        '<div class="seg-ctl" role="radiogroup" aria-label="How long">' + [[1, '1 day'], [7, '7 days'], [30, '30 days']].map(function (d) {
          return '<button class="' + (d[0] === days ? 'on' : '') + '" data-days="' + d[0] + '" role="radio" aria-checked="' + (d[0] === days) + '">' + d[1] + '</button>';
        }).join('') + '</div><div data-link class="share-link" hidden></div></div>',
      actions: [{ label: 'Close' }, { label: 'Create link', kind: 'primary', onClick: function (close) {
        const btn = m.el.querySelector('.modal-actions .btn-primary'); if (btn) btn.disabled = true;
        S.server.json('POST', '/api/shares', { kind: kind, name: name, tracks: trackIds, days: days }).then(function (r) {
          const url = full(r.share.path), box = m.el.querySelector('[data-link]');
          box.hidden = false;
          box.innerHTML = '<input class="input" readonly value="' + S.esc(url) + '" aria-label="Share link"><button class="btn btn-secondary btn-sm" data-copy-link>' + S.icon('copy', 14) + 'Copy</button>' +
            '<p class="set-hint">Works for ' + left(r.share.expires) + '. ' + (/^(localhost|127\.)/.test(location.hostname) ? 'This address only works on this computer: open Volvix Home by your server\'s network or internet address to share with others.' : '') + '</p>';
          if (btn) btn.remove();
          box.querySelector('input').select();
        }).catch(function (x) { if (btn) btn.disabled = false; S.toast(x.message, 'x'); });
        return false; // keep the dialog open to show the link
      } }]
    });
    m.el.addEventListener('click', function (e) {
      const d = e.target.closest('[data-days]');
      if (d) { days = Number(d.dataset.days); m.el.querySelectorAll('[data-days]').forEach(function (b) { b.classList.toggle('on', b === d); b.setAttribute('aria-checked', b === d); }); return; }
      if (e.target.closest('[data-copy-link]')) {
        const v = m.el.querySelector('[data-link] input').value;
        (navigator.clipboard ? navigator.clipboard.writeText(v) : Promise.reject()).then(function () { S.toast('Link copied', 'check'); }).catch(function () { m.el.querySelector('[data-link] input').select(); S.toast('Press Ctrl+C to copy', 'copy'); });
      }
    });
  };

  /** Settings: the links that are open right now. */
  S.shares = {
    load: function () { return S.server.json('GET', '/api/shares').then(function (r) { return r.shares; }); },
    row: function (s) {
      return '<div class="set-row"><div class="set-text"><strong>' + S.esc(s.name) + '</strong><span>' + S.esc(s.kind) + ' · ' + s.count + ' song' + (s.count === 1 ? '' : 's') + ' · by ' + S.esc(s.by) + ' · ' + left(s.expires) + '</span></div>' +
        '<div class="set-control"><button class="btn btn-secondary btn-sm" data-share-copy="' + S.esc(full(s.path)) + '">' + S.icon('copy', 14) + 'Copy link</button><button class="btn btn-ghost btn-sm" data-share-cancel="' + s.token + '">Cancel</button></div></div>';
    }
  };
})(window.Spin);
