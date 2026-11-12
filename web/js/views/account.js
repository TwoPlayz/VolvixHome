/* Volvix Home — Account: your profile and password, the household (admin), and settings. */
window.Spin = window.Spin || {};
Spin.views = Spin.views || {};
(function (S) {
  'use strict';

  function group(title, body, opts) {
    opts = opts || {};
    return '<section class="set-group"' + (opts.id ? ' id="' + opts.id + '"' : '') + '>' +
      '<div class="set-group-head"><h2>' + S.esc(title) + '</h2>' + (opts.sub ? '<p>' + opts.sub + '</p>' : '') + '</div>' +
      '<div class="set-body">' + body + '</div></section>';
  }
  function row(title, sub, control, opts) {
    opts = opts || {};
    return '<div class="set-row">' + (opts.icon ? '<span class="set-icon">' + S.icon(opts.icon, 18) + '</span>' : '') +
      '<div class="set-text"><strong>' + title + '</strong>' + (sub ? '<span>' + sub + '</span>' : '') + '</div>' +
      '<div class="set-control">' + control + '</div></div>';
  }
  function toggle(key, on) {
    return '<span class="switch"><input type="checkbox" data-setting="' + key + '"' + (on ? ' checked' : '') + '><span class="switch-ui"></span></span>';
  }
  function seg(key, value, options) {
    return '<div class="seg-ctl" role="radiogroup">' + options.map(function (o) {
      return '<button class="' + (o[0] === value ? 'on' : '') + '" data-seg="' + key + '" data-value="' + o[0] + '" role="radio" aria-checked="' + (o[0] === value) + '">' +
        (o[2] ? S.icon(o[2], 15) : '') + o[1] + '</button>';
    }).join('') + '</div>';
  }
  function field(label, name, type, hint) {
    return '<label class="field"><span class="label">' + label + '</span><input class="input" name="' + name + '" type="' + (type || 'text') + '"' +
      (type === 'password' ? ' autocomplete="new-password"' : ' autocomplete="off" autocapitalize="none" spellcheck="false"') + '>' +
      (hint ? '<span class="hint">' + hint + '</span>' : '') + '</label>';
  }

  const TABS = function () {
    return [['profile', 'Profile', 'user']].concat(S.data.can('members') ? [['members', 'Members', 'family']] : []).concat([['settings', 'Settings', 'settings']]);
  };

  /* ---------- Profile ---------- */
  function profileTab() {
    const me = S.data.me;
    return group('You',
      '<form class="set-form name-form" data-form="profile-name" novalidate><label class="field"><span class="label">Your name</span>' +
        '<input class="input" name="name" value="' + S.esc(me.name) + '" maxlength="60" autocomplete="name" autocapitalize="words" spellcheck="false"><span class="hint">This is also what you sign in with, on every device.</span></label>' +
        '<p class="auth-error" hidden></p><button class="btn btn-primary" type="submit">Save name</button></form>' +
      row(S.avatar(me, 56) + '<span class="set-who"><span>' + S.esc(me.name) + '</span><em>' + (me.admin ? 'Admin' : 'Member') + '</em></span>', '',
        '<input type="file" accept="image/*" data-pfp-file hidden>' +
        '<button class="btn btn-secondary btn-sm" data-act="pfp-pick">' + S.icon('user', 14) + 'Change picture</button>' +
        (me.avatar ? '<button class="btn btn-ghost btn-sm" data-act="pfp-remove">Remove</button>' : ''))) +
      group('Password',
        '<form class="set-form" data-form="password" novalidate>' +
          field('Current password', 'current', 'password') +
          field('New password', 'new', 'password', 'At least 8 characters. Your other devices will need to sign in again.') +
          '<p class="auth-error" hidden></p>' +
          '<button class="btn btn-primary" type="submit">Change password</button>' +
        '</form>') +
      group('Sign out', row('Sign out on this device', 'Your playlists and settings stay on the server.', '<button class="btn btn-secondary" data-act="logout">Sign out</button>', { icon: 'logout' }));
  }

  /* ---------- permissions ---------- */
  const PERMS = [
    ['request', 'Request music', 'Ask Lidarr to download music that isn\'t on the server yet.'],
    ['rescan', 'Check for new music', 'Look in the music folder for added or removed songs right now.'],
    ['parties', 'Start listening parties', 'Host a party and invite the household.'],
    ['members', 'Manage members', 'Add people, change what they may do, set passwords and remove them.'],
    ['settings', 'Server settings', 'Change the music folder and the Lidarr connection.'],
    ['share', 'Create share links', 'Make a link to an album or playlist that anyone with the link can play for a few days.']
  ];
  function roleOf(u) {
    const p = u.perms || [];
    if (u.admin || p.length === PERMS.length) return 'Admin';
    const basic = p.length === 2 && p.indexOf('request') !== -1 && p.indexOf('parties') !== -1;
    return basic ? 'Member' : 'Member · custom permissions';
  }
  /** You can manage someone unless they're an admin or hold more permissions than you (the server checks too). */
  function canManage(u) {
    const me = S.data.me;
    return me.admin || (S.data.can('members') && !u.admin && (u.perms || []).every(function (p) { return S.data.can(p); }));
  }

  /* ---------- Household (admin) ---------- */
  let users = null;
  function householdTab() {
    if (!users) {
      S.server.json('GET', '/api/users').then(function (r) { users = r.users; S.app.refresh(); }).catch(function (e) { S.toast(e.message, 'x'); });
      return group('Household', '<p class="set-hint">Loading…</p>');
    }
    return group('Members of this server',
      users.map(function (u) {
        const me = u.id === S.data.me.serverId;
        const notes = [roleOf(u)].concat(u.hideExplicit ? ['Explicit off'] : []).join(' · ');
        return row(S.avatar({ id: u.id, name: u.name }, 36) + '<span class="set-who"><span>' + S.esc(u.name) + (me ? ' (you)' : '') + '</span><em>' + notes + '</em></span>', '',
          me || !canManage(u) ? '' : '<button class="btn btn-secondary btn-sm" data-manage-user="' + u.id + '">Manage</button>');
      }).join(''),
      { sub: 'Everyone shares the same music. Playlists, play history and settings are personal.' }) +
      group('Add a member',
        '<form class="set-form" data-form="add-user" novalidate>' +
          field('Name', 'name', 'text', 'They sign in with this name.') +
          field('Password', 'password', 'password', 'At least 8 characters. They can change it under Account.') +
          '<p class="set-hint">New members can request music and start parties. Change that any time under Manage.</p>' +
          '<p class="auth-error" hidden></p>' +
          '<button class="btn btn-primary" type="submit">' + S.icon('plus', 16) + 'Add to household</button>' +
        '</form>');
  }

  /** Your picture or name changed: update every place it's shown, without reloading anything. */
  function meChanged() { S.app.refreshChrome(); S.app.refresh(); }

  /**
   * Profile picture: pick a photo, then move it (drag) and zoom it (slider, mouse wheel, pinch or + / -) until the circle
   * shows what you want. The round preview is exactly what's saved (a 256 px square, shown round everywhere).
   */
  function pictureDialog(file) {
    const img = new Image();
    const url = URL.createObjectURL(file);
    img.onerror = function () { URL.revokeObjectURL(url); S.toast('Couldn\'t read that picture.', 'x'); };
    img.onload = function () {
      const SIZE = 300, OUT = 256;
      const min = SIZE / Math.min(img.width, img.height); // smallest zoom: the photo still fills the square
      let scale = min, ox = (SIZE - img.width * min) / 2, oy = (SIZE - img.height * min) / 2;
      const m = S.modal({
        title: 'Adjust your picture',
        body: '<div class="crop"><canvas class="crop-canvas" width="' + SIZE + '" height="' + SIZE + '" tabindex="0" role="img" aria-label="Your picture. Drag to move it, use plus and minus to zoom."></canvas>' +
          '<label class="crop-zoom"><span>' + S.icon('minus', 14) + '</span><input type="range" min="1" max="4" step="0.01" value="1" aria-label="Zoom" data-crop-zoom><span>' + S.icon('plus', 14) + '</span></label>' +
          '<p class="set-hint">Drag to move. Scroll, pinch or use the slider to zoom.</p></div>',
        actions: [{ label: 'Cancel', onClick: function (close) { URL.revokeObjectURL(url); close(); } },
          { label: 'Save picture', kind: 'primary', onClick: function (close) { save(close); } }]
      });
      const cv = m.el.querySelector('canvas'), ctx = cv.getContext('2d'), zoom = m.el.querySelector('[data-crop-zoom]');
      const clamp = function () { ox = Math.min(0, Math.max(SIZE - img.width * scale, ox)); oy = Math.min(0, Math.max(SIZE - img.height * scale, oy)); };
      function draw() {
        clamp();
        ctx.clearRect(0, 0, SIZE, SIZE);
        ctx.drawImage(img, ox, oy, img.width * scale, img.height * scale);
        // dim everything outside the circle that will be shown
        ctx.save(); ctx.fillStyle = 'rgba(0,0,0,.58)'; ctx.beginPath(); ctx.rect(0, 0, SIZE, SIZE); ctx.arc(SIZE / 2, SIZE / 2, SIZE / 2 - 3, 0, Math.PI * 2, true); ctx.fill('evenodd'); ctx.restore();
        ctx.strokeStyle = 'rgba(255,255,255,.85)'; ctx.lineWidth = 2; ctx.beginPath(); ctx.arc(SIZE / 2, SIZE / 2, SIZE / 2 - 3, 0, Math.PI * 2); ctx.stroke();
      }
      /** Zoom, keeping the point (px, py) under the pointer where it is. */
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
      // one finger / mouse moves the photo; two fingers pinch to zoom
      const pts = new Map(); let last = null;
      const local = function (e) { const r = cv.getBoundingClientRect(); return [(e.clientX - r.left) * SIZE / r.width, (e.clientY - r.top) * SIZE / r.height]; };
      cv.addEventListener('pointerdown', function (e) { try { cv.setPointerCapture(e.pointerId); } catch (x) { /* synthetic pointer */ } pts.set(e.pointerId, local(e)); last = null; cv.classList.add('grabbing'); });
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
          fetch('/api/me/avatar', { method: 'PUT', credentials: 'same-origin', headers: { 'Content-Type': 'image/jpeg' }, body: blob })
            .then(function (r) { return r.ok ? r.json() : r.json().then(function (j) { throw new Error(j.error || 'Couldn\'t save the picture.'); }); })
            .then(function (r) { URL.revokeObjectURL(url); S.data.me.avatar = r.avatar; close(); S.toast('Picture updated', 'check'); meChanged(); })
            .catch(function (x) { S.toast(x.message, 'x'); });
        }, 'image/jpeg', 0.9);
      }
    };
    img.src = url;
  }
  /** An admin's controls for one person: explicit songs, admin, a new password, remove. */
  function manageDialog(id) {
    const u = (users || []).find(function (x) { return x.id === id; });
    if (!u) return;
    const sw = function (name, title, text, on) {
      return '<label class="switch-row"><span class="switch-text"><strong>' + title + '</strong><span>' + text + '</span></span>' +
        '<span class="switch"><input type="checkbox" data-mg="' + name + '"' + (on ? ' checked' : '') + '><span class="switch-ui"></span></span></label>';
    };
    const m = S.modal({
      title: 'Manage ' + u.name,
      body: '<div class="mg">' +
        '<div class="mg-who">' + S.avatar({ id: u.id, name: u.name }, 48) + '<div><strong>' + S.esc(u.name) + '</strong><span>' + roleOf(u) + '</span></div></div>' +
        sw('hideExplicit', 'Turn off explicit music', 'Songs marked explicit stay listed but are skipped for them.', u.hideExplicit) +
        '<div class="q-label mg-label">What ' + S.esc(u.name) + ' can do</div>' +
        PERMS.map(function (p) {
          const on = (u.perms || []).indexOf(p[0]) !== -1;
          return '<label class="switch-row' + (S.data.can(p[0]) ? '' : ' disabled') + '"><span class="switch-text"><strong>' + p[1] + '</strong><span>' + p[2] + (S.data.can(p[0]) ? '' : ' (you don\'t have this one, so you can\'t give it)') + '</span></span>' +
            '<span class="switch"><input type="checkbox" data-perm="' + p[0] + '"' + (on ? ' checked' : '') + (S.data.can(p[0]) ? '' : ' disabled') + '><span class="switch-ui"></span></span></label>';
        }).join('') +
        '<div class="mg-presets"><button class="btn btn-ghost btn-sm" data-mg-preset="all"' + (S.data.me.admin ? '' : ' disabled') + '>Everything (admin)</button><button class="btn btn-ghost btn-sm" data-mg-preset="basic">Basics only</button></div>' +
        '<form class="mg-pass" data-mg-pass novalidate><label class="field"><span class="label">New password</span>' +
          '<input class="input" name="password" type="password" autocomplete="new-password" placeholder="At least 8 characters"></label>' +
          '<p class="auth-error" hidden></p>' +
          '<button class="btn btn-secondary" type="submit">Set new password</button>' +
          '<span class="hint">They\'re signed out everywhere and use this password next time.</span></form>' +
        '<div class="mg-danger"><button class="btn btn-ghost mg-remove" data-mg-remove>' + S.icon('x', 15) + 'Remove ' + S.esc(u.name) + '</button></div>' +
      '</div>',
      actions: [{ label: 'Done', kind: 'secondary' }]
    });
    const err = function (x) { S.toast(x.message || 'That didn\'t work', 'x'); };
    /** Save what they may do, and update the dialog and the list. */
    function savePerms(list, onFail) {
      S.server.json('PUT', '/api/users/' + id, { perms: list }).then(function (r) {
        Object.assign(u, r.user);
        const who = m.el.querySelector('.mg-who span'); if (who) who.textContent = roleOf(u);
        m.el.querySelectorAll('[data-perm]').forEach(function (c) { c.checked = (u.perms || []).indexOf(c.dataset.perm) !== -1; });
        S.toast('Permissions saved', 'check');
        S.app.refresh();
      }).catch(function (x) { if (onFail) onFail(); err(x); });
    }
    m.el.addEventListener('change', function (e) {
      if (e.target.dataset && e.target.dataset.perm) {
        const list = Array.from(m.el.querySelectorAll('[data-perm]')).filter(function (c) { return c.checked; }).map(function (c) { return c.dataset.perm; });
        savePerms(list, function () { e.target.checked = !e.target.checked; });
        return;
      }
      const key = e.target.dataset && e.target.dataset.mg;
      if (!key) return;
      const body = {}; body[key] = e.target.checked;
      S.server.json('PUT', '/api/users/' + id, body).then(function (r) {
        Object.assign(u, r.user);
        S.toast(e.target.checked ? 'Explicit music is off for ' + u.name : 'Explicit music is on for ' + u.name, 'check');
        S.app.refresh();
      }).catch(function (x) { e.target.checked = !e.target.checked; err(x); });
    });
    m.el.addEventListener('click', function (e) {
      const pre = e.target.closest('[data-mg-preset]');
      if (!pre) return;
      savePerms(pre.dataset.mgPreset === 'all' ? PERMS.map(function (p) { return p[0]; }) : ['request', 'parties']);
    });    m.el.addEventListener('submit', function (e) {
      const form = e.target.closest('[data-mg-pass]');
      if (!form) return;
      e.preventDefault();
      const box = form.querySelector('.auth-error'); box.hidden = true;
      S.server.json('POST', '/api/users/' + id + '/password', { password: form.password.value }).then(function () {
        form.reset(); S.toast('New password set for ' + u.name, 'check');
      }).catch(function (x) { box.textContent = x.message; box.hidden = false; });
    });
    m.el.addEventListener('click', function (e) {
      if (!e.target.closest('[data-mg-remove]')) return;
      m.close && m.close();
      S.modal({
        title: 'Remove ' + u.name + '?',
        body: '<p class="modal-text">They won\'t be able to sign in, and their playlists and history are deleted. The music stays.</p>',
        actions: [{ label: 'Cancel' }, { label: 'Remove', kind: 'danger', onClick: function (close) {
          S.server.json('DELETE', '/api/users/' + id).then(function () {
            close(); users = null; S.toast(u.name + ' removed', 'check'); S.app.refresh();
          }).catch(function (x) { close(); err(x); });
        } }]
      });
    });
  }

  /* ---------- Lidarr (admin) ---------- */
  let lidarr = null;
  function select(name, value, options, empty) {
    return '<select class="input" name="' + name + '">' + (empty ? '<option value="">' + empty + '</option>' : '') + options.map(function (o) {
      return '<option value="' + S.esc(o[0]) + '"' + (String(o[0]) === String(value) ? ' selected' : '') + '>' + S.esc(o[1]) + '</option>';
    }).join('') + '</select>';
  }
  function lidarrGroup() {
    if (!S.data.can('settings')) return '';
    if (!lidarr) {
      S.server.json('GET', '/api/lidarr').then(function (r) { lidarr = r; S.app.refresh(); }).catch(function () {});
      return group('Lidarr', '<p class="set-hint">Loading…</p>', { id: 'lidarr' });
    }
    const s = lidarr.settings || {};
    const L = lidarr.lidarr;
    const hook = location.origin + (s.hookPath || '');
    return group('Lidarr',
      '<form class="set-form" data-form="lidarr" novalidate>' +
        (lidarr.configured
          ? (lidarr.error ? '<p class="auth-error">' + S.esc(lidarr.error) + '</p>' : '<p class="gm-ok">' + S.icon('check', 15) + 'Connected to Lidarr ' + S.esc(L && L.version || '') + '</p>')
          : '<p class="set-hint">Connect Lidarr to search for music you don\'t have and add it to this server automatically. Lidarr finds and downloads it with the sources you set up there.</p>') +
        '<label class="field"><span class="label">Lidarr address</span><input class="input" name="url" value="' + S.esc(s.url || '') + '" placeholder="http://lidarr:8686" autocomplete="off" spellcheck="false"></label>' +
        '<label class="field"><span class="label">API key</span><input class="input" name="apiKey" type="password" autocomplete="off" placeholder="' + (s.apiKeySet ? 'Saved (leave empty to keep it)' : 'Lidarr → Settings → General → API Key') + '"></label>' +
        (L ? '<label class="field"><span class="label">Add music to</span>' + select('rootFolder', s.rootFolder, L.rootFolders.map(function (r) { return [r.path, r.path]; })) +
          '<span class="hint">Lidarr saves downloads here, and Volvix reads your music folder (Settings → Library). They have to be the same place.' +
          (folder && s.rootFolder && !samePath(folder.path, s.rootFolder) ? ' <strong>Right now they differ</strong> (Volvix reads ' + S.esc(folder.path) + '). That\'s fine if one is just the other\'s name inside Docker; otherwise change one of them.' : '') + '</span></label>' +
          '<label class="field"><span class="label">Quality profile</span>' + select('qualityProfileId', s.qualityProfileId, L.qualityProfiles.map(function (p) { return [p.id, p.name]; })) + '</label>' +
          '<label class="field"><span class="label">Metadata profile</span>' + select('metadataProfileId', s.metadataProfileId, L.metadataProfiles.map(function (p) { return [p.id, p.name]; })) + '</label>' +
          '<p class="set-hint">Who may ask for music is set per member (Members → Manage → Request music).</p>' : '') +
        '<p class="auth-error" hidden></p>' +
        '<div class="set-actions"><button class="btn btn-primary" type="submit">' + (lidarr.configured ? 'Save' : 'Connect') + '</button>' +
          (lidarr.configured ? '<button class="btn btn-ghost" type="button" data-act="lidarr-off">Disconnect</button>' : '') + '</div>' +
      '</form>' +
      (lidarr.configured ? '<p class="set-hint" style="padding:12px 16px 0">New downloads appear on their own: Volvix Home watches the music folder, so nothing needs to be set up in Lidarr.</p>' : ''),
      { id: 'lidarr', sub: 'Get music you don\'t have yet.' });
  }

  /* ---------- Music folder (admin) ---------- */
  let folder = null;
  function samePath(a, b) {
    const n = function (p) { return String(p || '').split('\\').join('/').replace(/\/+$/, '').toLowerCase(); };
    return n(a) === n(b);
  }
  /** Wait for the server to finish scanning, then bring the new library in (no reload: the page just updates). */
  function syncWhenScanned() {
    const poll = setInterval(function () {
      S.server.json('GET', '/api/status').then(function (s) {
        if (s.scanning) return;
        clearInterval(poll);
        status = s;
        S.data.sync().then(function (c) { S.toast('Library updated: ' + c.albums + ' albums, ' + c.tracks + ' songs', 'check'); S.app.refresh(); }).catch(function () { S.toast('Couldn\'t load the new library', 'x'); });
      }).catch(function () {});
    }, 1200);
  }
  /** Pick the music folder by browsing the server's folders (so no one has to type a path). */
  function pickFolder() {
    let current = '';
    const m = S.modal({
      title: 'Choose your music folder',
      body: '<p class="modal-text">Pick the folder that holds your music (the one Lidarr saves to), or type its path. Volvix looks inside it, including subfolders.</p>' +
        '<form class="fb-type" data-fb-type><input class="input" name="p" placeholder="/path/to/music" autocomplete="off" spellcheck="false" aria-label="Folder path"><button class="btn btn-secondary btn-sm" type="submit">Go</button></form>' +
        '<div class="fb" data-fb><p class="set-hint">Loading…</p></div>',
      actions: [
        { label: 'Cancel' },
        { label: 'Use this folder', kind: 'primary', onClick: function (close) {
          S.server.json('PUT', '/api/library/folder', { path: current }).then(function (r) {
            close(); folder = null; S.toast('Using ' + r.path, 'check'); S.app.refresh(); syncWhenScanned();
          }).catch(function (x) { S.toast(x.message, 'x'); });
        } }
      ]
    });
    const host = function () { return m.el.querySelector('[data-fb]'); };
    function show(path) {
      S.server.json('GET', '/api/library/browse' + (path ? '?path=' + encodeURIComponent(path) : '')).then(function (r) {
        current = r.path;
        const inp = m.el.querySelector('[data-fb-type] input'); if (inp) inp.value = r.path;
        host().innerHTML =
          (r.denied ? '<p class="auth-error">Volvix isn\'t allowed to read this folder. It runs as the user in your stack\'s <code>user:</code> line, and that user needs read access (and write access to its own data folder). On TrueNAS: <code>chown -R 1000:1000 &lt;folder&gt;</code> or add that user to the dataset\'s permissions (Datasets → Permissions), then try again.</p>' : '') +
          (r.missing ? '<p class="auth-error">That folder doesn\'t exist on the server, so this is the closest one that does.</p>' : '') +
          (r.docker ? '<p class="set-hint">Volvix is running in Docker, so it only sees folders you mounted into the container. To see more, add a volume in your stack (for example <code>/mnt/pool/data/media:/media:ro</code>) and pick it here (<code>/media</code>).</p>' : '') +
          '<div class="fb-path">' + S.icon('library', 15) + '<span>' + S.esc(r.path) + '</span></div>' +
          '<div class="fb-note">' + (r.songsHere ? S.icon('check', 14) + r.songsHere + ' song' + (r.songsHere === 1 ? '' : 's') + ' directly in this folder' : 'No songs directly in this folder (subfolders count too)') + '</div>' +
          '<div class="fb-list">' + (r.parent ? '<button class="fb-row" data-dir="' + S.esc(r.parent) + '">' + S.icon('arrowUp', 15) + '<span>Up one level</span></button>' : '') +
            r.dirs.map(function (d) { return '<button class="fb-row" data-dir="' + S.esc(d.path) + '">' + S.icon('library', 15) + '<span>' + S.esc(d.name) + '</span></button>'; }).join('') +
            (!r.dirs.length ? '<p class="set-hint">No folders in here.</p>' : '') + '</div>';
      }).catch(function (x) { host().innerHTML = '<p class="auth-error">' + S.esc(x.message) + '</p>'; });
    }
    m.el.addEventListener('click', function (e) { const b = e.target.closest('[data-dir]'); if (b) show(b.dataset.dir); });
    m.el.addEventListener('submit', function (e) { const f = e.target.closest('[data-fb-type]'); if (!f) return; e.preventDefault(); show(f.p.value.trim()); });
    show('');
  }

  /* ---------- the Windows app's own settings (only inside that app) ---------- */
  function nativeSend(msg) { try { window.ipc.postMessage(JSON.stringify(msg)); } catch (e) { /* not in the app */ } }
  function nativeGroup() {
    const n = window.VOLVIX_NATIVE;
    if (!n || n.app !== 'windows') return '';
    const s = n.settings || {}, u = n.update || null;
    const nt = function (key, on) { return '<span class="switch"><input type="checkbox" data-native="' + key + '"' + (on ? ' checked' : '') + '><span class="switch-ui"></span></span>'; };
    const upd = !u ? 'Version ' + S.esc(n.version) : u.state === 'checking' ? 'Checking…' : u.state === 'downloading' ? 'Downloading ' + S.esc(u.version) + '…'
      : u.state === 'ready' ? 'Version ' + S.esc(u.version) + ' is installed. Restart to use it.' : u.state === 'failed' ? S.esc(u.message) : 'You have the latest version (' + S.esc(n.version) + ').';
    return group('Windows app',
      row('Start with Windows', 'Open Volvix Home in the background when you sign in to Windows.', nt('startWithWindows', s.startWithWindows !== false), { icon: 'monitor' }) +
      row('Close to the system tray', 'The close button hides the window and the music keeps playing. Use Quit in the tray icon to exit.', nt('closeToTray', s.closeToTray !== false), { icon: 'monitor' }) +
      row('Update automatically', upd, nt('autoUpdate', s.autoUpdate !== false), { icon: 'download' }) +
      row('Check for updates', '', u && u.state === 'ready' ? '<button class="btn btn-primary btn-sm" data-act="native-restart">Restart now</button>' : '<button class="btn btn-secondary btn-sm" data-act="native-check">Check now</button>', { icon: 'download' }),
      { id: 'native' });
  }

  /* ---------- share links ---------- */
  let myShares = null;
  function shareGroup() {
    if (!S.data.can('share') && !S.data.can('members')) return '';
    if (!myShares) { S.shares.load().then(function (l) { myShares = l; S.app.refresh(); }).catch(function () { myShares = []; }); return group('Share links', '<p class="set-hint">Loading…</p>', { id: 'shares' }); }
    return group('Share links', myShares.length ? myShares.map(S.shares.row).join('') : '<p class="set-hint" style="padding:12px 16px">No links are open. Share an album or playlist from its ⋯ menu.</p>',
      { id: 'shares', sub: 'Open links, newest first. Cancelling one stops it working straight away.' });
  }

  /* ---------- equalizer ---------- */
  // The equalizer is a graph: drag a dot up or down to boost or cut that part of the sound (32 Hz bass on the left, 16 kHz treble on the right).
  const G = { w: 640, h: 260, x0: 46, x1: 628, top: 16, bot: 230, r: 9 };
  /** On a phone the graph is drawn taller and narrower (about 1:1 with the screen) so the dots stay big enough to grab. */
  function eqLayout() {
    const phone = !!(window.matchMedia && window.matchMedia('(max-width: 760px)').matches);
    Object.assign(G, phone ? { w: 360, h: 340, x0: 36, x1: 352, top: 22, bot: 290, r: 11 } : { w: 640, h: 260, x0: 46, x1: 628, top: 16, bot: 230, r: 9 });
  }
  const gx = function (i) { return G.x0 + (G.x1 - G.x0) * (i + 0.5) / 10; };
  const gy = function (db) { return (G.top + G.bot) / 2 - db * ((G.bot - G.top) / 2 / 12); };
  function curve(gains) {
    const p = gains.map(function (g, i) { return [gx(i), gy(g)]; });
    let d = 'M' + G.x0 + ' ' + p[0][1].toFixed(1) + ' L' + p[0][0].toFixed(1) + ' ' + p[0][1].toFixed(1);
    for (let i = 0; i < p.length - 1; i++) {
      const p0 = p[i - 1] || p[i], p1 = p[i], p2 = p[i + 1], p3 = p[i + 2] || p2;
      d += ' C' + (p1[0] + (p2[0] - p0[0]) / 6).toFixed(1) + ' ' + (p1[1] + (p2[1] - p0[1]) / 6).toFixed(1) + ',' +
        (p2[0] - (p3[0] - p1[0]) / 6).toFixed(1) + ' ' + (p2[1] - (p3[1] - p1[1]) / 6).toFixed(1) + ',' + p2[0].toFixed(1) + ' ' + p2[1].toFixed(1);
    }
    return d + ' L' + G.x1 + ' ' + p[p.length - 1][1].toFixed(1);
  }
  const fillOf = function (gains) { return curve(gains) + ' L' + G.x1 + ' ' + gy(0) + ' L' + G.x0 + ' ' + gy(0) + ' Z'; };
  const hz = function (f) { return f >= 1000 ? (f / 1000) + 'k' : String(f); };
  const sgn = function (v) { return (v > 0 ? '+' : '') + v; };

  /** The whole equalizer: switch, presets, the graph, preamp. It lives inside Playback with crossfade and leveling. */
  function eqPanel(st) {
    eqLayout();
    const eq = st.settings.eq || { on: false, preset: 'flat', pre: 0, gains: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0] };
    const P = S.fx.PRESETS, B = S.fx.BANDS;
    const grid = [-12, -6, 0, 6, 12].map(function (d) {
      return '<line class="eq-grid' + (d === 0 ? ' zero' : '') + '" x1="' + G.x0 + '" x2="' + G.x1 + '" y1="' + gy(d) + '" y2="' + gy(d) + '"/><text class="eq-axis" x="' + (G.x0 - 8) + '" y="' + (gy(d) + 4) + '" text-anchor="end">' + sgn(d) + '</text>';
    }).join('');
    const cols = B.map(function (f, i) {
      return '<line class="eq-grid v" x1="' + gx(i) + '" x2="' + gx(i) + '" y1="' + G.top + '" y2="' + G.bot + '"/><text class="eq-axis" x="' + gx(i) + '" y="' + (G.bot + 22) + '" text-anchor="middle">' + hz(f) + '</text>';
    }).join('');
    const pts = eq.gains.map(function (g, i) {
      return '<circle class="eq-pt" data-pt="' + i + '" cx="' + gx(i) + '" cy="' + gy(g).toFixed(1) + '" r="' + G.r + '" tabindex="' + (eq.on ? 0 : -1) + '" role="slider" aria-label="' + hz(B[i]) + ' Hz" aria-valuemin="-12" aria-valuemax="12" aria-valuenow="' + g + '" aria-valuetext="' + sgn(g) + ' dB"/>';
    }).join('');
    return row('Equalizer', S.fx.available() ? 'Shape the sound: drag the dots up to boost and down to cut.' + ` It works on this device while it plays (not when casting to a TV).` : 'This browser can\'t process sound.', toggle('eqOn', !!eq.on), { icon: 'settings' }) +
      '<div class="eq-panel"' + (eq.on ? '' : ' data-off') + '>' +
        '<div class="eq-top"><label class="eq-pick"><span>Preset</span><select class="input" data-eq-preset' + (eq.on ? '' : ' disabled') + '>' +
          Object.keys(P).map(function (k) { return '<option value="' + k + '"' + (eq.preset === k ? ' selected' : '') + '>' + S.esc(P[k].label) + '</option>'; }).join('') +
          '<option value="custom"' + (eq.preset === 'custom' ? ' selected' : '') + '>Custom</option></select></label>' +
          '<label class="eq-pick"><span>Preamp <b data-eq-prev>' + sgn(eq.pre) + ' dB</b></span><input type="range" min="-12" max="6" step="1" value="' + eq.pre + '" data-eq-pre' + (eq.on ? '' : ' disabled') + '></label>' +
          '<button class="btn btn-ghost btn-sm" data-act="eq-reset"' + (eq.on ? '' : ' disabled') + '>Reset</button></div>' +
        ('<div class="eq-graph"><svg viewBox="0 0 ' + G.w + ' ' + G.h + '" role="group" aria-label="Equalizer graph" data-eq-svg>' + grid + cols +
          '<path class="eq-fill" d="' + fillOf(eq.gains) + '"/><path class="eq-line" d="' + curve(eq.gains) + '"/>' + pts +
          '<g class="eq-tip" data-eq-tip hidden><rect rx="6" width="92" height="24" x="-46" y="-40"/><text text-anchor="middle" y="-24"></text></g></svg></div>') +
      '</div>';
  }
  /** Redraw the curve and dots while dragging (no re-render of the page). */
  function eqPaint(svg, gains, active) {
    svg.querySelector('.eq-line').setAttribute('d', curve(gains));
    svg.querySelector('.eq-fill').setAttribute('d', fillOf(gains));
    svg.querySelectorAll('.eq-pt').forEach(function (c, i) {
      c.setAttribute('cy', gy(gains[i]).toFixed(1)); c.setAttribute('aria-valuenow', gains[i]); c.setAttribute('aria-valuetext', sgn(gains[i]) + ' dB');
    });
    const tip = svg.querySelector('[data-eq-tip]');
    if (active == null) { tip.hidden = true; return; }
    tip.hidden = false;
    const x = Math.max(G.x0 + 46, Math.min(G.x1 - 46, gx(active)));
    tip.setAttribute('transform', 'translate(' + x + ',' + gy(gains[active]).toFixed(1) + ')');
    tip.querySelector('text').textContent = sgn(gains[active]) + ' dB · ' + hz(S.fx.BANDS[active]) + ' Hz';
  }
  /** Dragging and keys on the graph. Safe to call after every render. */
  function bindEq(root) {
    eqLayout();
    const svg = root.querySelector('[data-eq-svg]');
    if (!svg) return;
    const on = function () { return !!(S.store.state.settings.eq && S.store.state.settings.eq.on); };
    const gains = function () { return (S.store.state.settings.eq.gains || []).slice(); };
    const at = function (e) {
      const r = svg.getBoundingClientRect();
      const sx = (e.clientX - r.left) * (G.w / r.width), sy = (e.clientY - r.top) * (G.h / r.height);
      const i = Math.max(0, Math.min(9, Math.round((sx - G.x0) / (G.x1 - G.x0) * 10 - 0.5)));
      const db = Math.max(-12, Math.min(12, Math.round(((G.top + G.bot) / 2 - sy) / ((G.bot - G.top) / 2 / 12) * 2) / 2));
      return { i: i, db: db };
    };
    const set = function (g, active, persist) {
      eqSet({ gains: g, preset: 'custom' }, persist);
      const sel = root.querySelector('[data-eq-preset]'); if (sel) sel.value = 'custom';
      eqPaint(svg, g, active);
    };
    let drag = null;
    svg.addEventListener('pointerdown', function (e) {
      if (!on()) return;
      e.preventDefault();
      const p = at(e); drag = p.i;
      try { svg.setPointerCapture(e.pointerId); } catch (x) { /* ignore */ }
      const g = gains(); g[p.i] = p.db; set(g, p.i, false);
    });
    svg.addEventListener('pointermove', function (e) {
      if (drag == null) return;
      const g = gains(); g[drag] = at(e).db; set(g, drag, false);
    });
    const end = function () { if (drag == null) return; drag = null; eqSet({}, true); eqPaint(svg, gains(), null); };
    svg.addEventListener('pointerup', end); svg.addEventListener('pointercancel', end);
    svg.addEventListener('dblclick', function (e) {
      if (!on()) return;
      const i = at(e).i, g = gains(); g[i] = 0; set(g, i, true); eqPaint(svg, g, null);
    });
    svg.addEventListener('keydown', function (e) {
      const pt = e.target.closest && e.target.closest('[data-pt]');
      if (!pt || !on()) return;
      const i = Number(pt.dataset.pt), g = gains();
      const step = e.shiftKey ? 3 : 1;
      if (e.key === 'ArrowUp' || e.key === 'ArrowRight') g[i] = Math.min(12, g[i] + step);
      else if (e.key === 'ArrowDown' || e.key === 'ArrowLeft') g[i] = Math.max(-12, g[i] - step);
      else if (e.key === 'Home' || e.key === 'Delete') g[i] = 0;
      else return;
      e.preventDefault();
      set(g, i, true); pt.focus();
    });
    svg.addEventListener('focusout', function () { if (drag == null) eqPaint(svg, gains(), null); });
  }
  /** Save the EQ the person set, apply it to the playing sound, and keep the panel in step. */
  function eqSet(patch, persist) {
    const cur = S.store.state.settings.eq || {};
    const eq = Object.assign({ on: false, preset: 'flat', pre: 0, gains: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0] }, cur, patch);
    eq.gains = (patch.gains || cur.gains || eq.gains).slice();
    if (persist) S.store.update(function (s) { s.settings.eq = eq; }); else S.store.state.settings.eq = eq;
    S.fx.sync();
    return eq;
  }

  /* ---------- Settings ---------- */
  let sleepChoice = '0';
  function sleepValue() { return S.player.sleepInfo() ? sleepChoice : '0'; }
  function sleepText() {
    const s = S.player.sleepInfo();
    return s === 'song' ? 'Pauses when this song ends.' : s ? 'Pauses in about ' + s + ' minute' + (s === 1 ? '' : 's') + '.' : 'Pause the music after a while, so you can fall asleep to it.';
  }
  let status = null;
  function settingsTab() {
    const st = S.store.state;
    if (!status) S.server.json('GET', '/api/status').then(function (s) { status = s; S.app.refresh(); }).catch(function () {});
    if (!folder && S.data.can('settings')) S.server.json('GET', '/api/library/folder').then(function (f) { folder = f; S.app.refresh(); }).catch(function () {});
    return group('Appearance',
        row('Theme', st.theme === 'system' ? 'Matches your device\'s light or dark setting.' : 'Always ' + st.theme + '.',
          seg('theme', st.theme, [['system', 'System', 'monitor'], ['dark', 'Dark', 'moon'], ['light', 'Light', 'sun']]), { icon: 'sun' })) +
      group('Privacy',
        row('Share what I\'m listening to', 'Others in your household see your current song on the Household page.', toggle('shareActivity', st.settings.shareActivity !== false), { icon: 'users' }) +
        row('Private session', 'Stops sharing, and keeps what you play out of Recently played, until you turn it off.', toggle('privateSession', !!st.settings.privateSession), { icon: 'eyeOff' }),
        { id: 'privacy' }) +
      group('Playback',
        row('Crossfade', 'The end of one song fades into the start of the next. Not used in listening parties, and the last song just ends.',
          seg('crossfade', String(st.settings.crossfade || 0), [['0', 'Off'], ['2', '2 s'], ['4', '4 s'], ['6', '6 s'], ['8', '8 s'], ['12', '12 s']]), { icon: 'shuffle' }) +
        row('Crossfade songs on the same album', 'Off keeps albums flowing the way they were recorded (live albums, mixes), and only crossfades between different albums.', toggle('crossfadeAlbums', !!st.settings.crossfadeAlbums), { icon: 'disc' }) +
        (window.VH && window.VH.transcoding
          ? row('Streaming quality (this device)', 'Smaller files use less data. Original sends the file as it is. Converted songs are made once on the server and kept. Downloads use this too.',
              seg('streamq', (function () { try { return localStorage.getItem('vh_streamq') || 'original'; } catch (e) { return 'original'; } })(), [['original', 'Original'], ['high', 'High'], ['normal', 'Normal'], ['low', 'Data saver']]), { icon: 'wifi' }) +
            row('Use less data on mobile data', 'Switches to Data saver whenever your browser says you\'re on a mobile connection (Android Chrome).', '<span class="switch"><input type="checkbox" data-local="vh_datasaver"' + ((function () { try { return localStorage.getItem('vh_datasaver') === '1'; } catch (e) { return false; } })() ? ' checked' : '') + '><span class="switch-ui"></span></span>', { icon: 'phone' })
          : row('Streaming quality', 'Smaller files for a phone need ffmpeg on the server (the Docker image includes it). Right now songs stream in original quality.', '', { icon: 'wifi' })) +
        row('Volume leveling', 'Uses ReplayGain tags in your files so songs play at the same loudness. Files without the tags aren\'t changed.',
          seg('leveling', st.settings.leveling || 'off', [['off', 'Off'], ['track', 'Song'], ['album', 'Album']]), { icon: 'volume' }) +
        row('Sleep timer', sleepText(), seg('sleep', sleepValue(), [['0', 'Off'], ['15', '15 min'], ['30', '30 min'], ['60', '1 hour'], ['song', 'End of song']]), { icon: 'moon' }) +
        row('Resume where I left off', 'Reopening Volvix shows the last song and spot, paused.', toggle('resumeLast', st.settings.resumeLast !== false), { icon: 'history' }) +
        eqPanel(st),
        { id: 'playback' }) +
      nativeGroup() +
      shareGroup() +
      group('Downloads',
        row('Only play downloads', 'Use only the songs saved on this device. Everything else is greyed out and skipped. This also happens by itself when you lose your connection.',
          toggle('offlineOnly', !!st.settings.offlineOnly), { icon: 'wifiOff' }) +
        row('Download on mobile data', 'Off: downloads wait for Wi-Fi, so a big album can\'t use up your data plan. Only phones that tell the browser they\'re on mobile data (Android Chrome) are held back; elsewhere it can\'t tell.',
          toggle('downloadMobile', !!st.settings.downloadMobile), { icon: 'phone' }) +
        (window.VH && window.VH.transcoding
          ? row('Download quality', 'Smaller downloads use less storage and data. "Same as streaming" follows the streaming quality above. Songs already downloaded stay as they are.',
              seg('downloadq', st.settings.downloadQuality || 'stream', [['stream', 'Same as streaming'], ['original', 'Original'], ['high', 'High'], ['normal', 'Normal'], ['low', 'Data saver']]), { icon: 'download' })
          : '') +
        row('On this device', S.dl.supported ? S.dl.count() + ' song' + (S.dl.count() === 1 ? '' : 's') + ' · ' + (S.dl.bytes() / 1048576).toFixed(0) + ' MB. Download from an album, a playlist or a song\'s ⋯ menu.' : 'This browser can\'t keep downloads.',
          S.dl.count() ? '<button class="btn btn-secondary btn-sm" data-act="dl-clear">Remove all</button>' : '', { icon: 'download' }),
        { id: 'downloads' }) +
      group('Library',
        (S.data.can('settings') ? row('Music folder', folder ? S.esc(folder.path) + (folder.exists ? '' : ' · folder not found') + (folder.isDefault ? '' : ' · custom') : '…',
          '<button class="btn btn-secondary btn-sm" data-act="pick-folder">Change</button>', { icon: 'library' }) : '') +
        row('Music on this server', (status ? status.albums + ' albums · ' + status.tracks + ' songs' + (status.scanning ? ' · checking the folder…' : '') + '<br>' : '…') +
          (status && status.watching ? 'Watching the music folder: songs added or removed (by Lidarr or by you) show up within seconds' + (status.rescanMinutes ? ', with a full check every ' + status.rescanMinutes + ' minutes as a safety net' : '') : (status && status.rescanMinutes ? 'The server checks the folder by itself every ' + status.rescanMinutes + ' minutes (this folder can\'t report changes live)' : 'Automatic checks are off')) +
          '. You\'re told when to reload.',
          S.data.can('rescan') ? '<button class="btn btn-secondary btn-sm" data-act="rescan"' + (status && status.scanning ? ' disabled' : '') + '>Check now</button>' : '', { icon: 'library' }),
        { id: 'library' }) +
      lidarrGroup() +
      group('About',
        row('Volvix Home', status ? 'Version ' + status.version : '', '', { icon: 'disc' }) +
        // only inside the Windows / iPhone apps: go back to the screen where you enter a server address
        row('Playback log', 'For finding out why sound stops or glitches: what the audio did, with times.', '<button class="btn btn-secondary btn-sm" data-act="audio-log">Show</button>', { icon: 'info' }) +
        (window.VOLVIX_NATIVE ? row('Server', location.host, '<button class="btn btn-secondary btn-sm" data-act="switch-server">Switch server</button>', { icon: 'devices' }) : ''));
  }

  S.views.account = {
    render: function (root, params) {
      const tabs = TABS();
      const want = params.tab === 'people' ? 'members' : params.tab; // the tab used to be called People
      const tab = tabs.find(function (t) { return t[0] === want; }) || tabs[0];
      root.innerHTML =
        '<header class="page-head"><h1 class="page-title">' + (tab[0] === 'settings' ? 'Settings' : 'Account') + '</h1></header>' +
        '<div class="acct-layout">' +
          '<nav class="acct-nav" aria-label="Account sections">' + tabs.map(function (t) {
            return '<a class="acct-tab' + (t[0] === tab[0] ? ' active' : '') + '" href="#/account/' + t[0] + '"' + (t[0] === tab[0] ? ' aria-current="page"' : '') + '>' +
              S.icon(t[2], 18) + '<span>' + t[1] + '</span></a>';
          }).join('') + '</nav>' +
          '<div class="acct-body">' + (tab[0] === 'members' ? householdTab() : tab[0] === 'settings' ? settingsTab() : profileTab()) + '</div>' +
        '</div>';

      if (!window.__vhNativeHook) { window.__vhNativeHook = true; window.addEventListener('vh-native-update', function () { if (/\/account\/settings/.test(location.hash)) S.app.refresh(); }); }
      bindEq(root);
      // the preamp slider moves the sound while you drag; letting go saves
      root.oninput = function (e) {
        const t = e.target;
        if (t.dataset.eqBand !== undefined) {
          const eq = S.store.state.settings.eq, g = (eq.gains || []).slice(); g[Number(t.dataset.eqBand)] = Number(t.value);
          eqSet({ gains: g, preset: 'custom' }, false);
          const lab = root.querySelector('[data-eq-db="' + t.dataset.eqBand + '"]'); if (lab) lab.textContent = (t.value > 0 ? '+' : '') + t.value;
          const sel = root.querySelector('[data-eq-preset]'); if (sel) sel.value = 'custom';
        } else if (t.hasAttribute('data-eq-pre')) {
          eqSet({ pre: Number(t.value), preset: 'custom' }, false);
          const lab = root.querySelector('[data-eq-prev]'); if (lab) lab.textContent = (t.value > 0 ? '+' : '') + t.value + ' dB';
        }
      };
      root.onchange = function (e) {
        if (e.target.dataset && e.target.dataset.local) {
          try { localStorage.setItem(e.target.dataset.local, e.target.checked ? '1' : '0'); } catch (x) { /* ignore */ }
          S.toast(e.target.checked ? 'Data saver on' : 'Data saver off', 'check'); return;
        }
        if (e.target.dataset && (e.target.dataset.eqBand !== undefined || e.target.hasAttribute('data-eq-pre'))) { eqSet({}, true); return; }
        if (e.target.hasAttribute && e.target.hasAttribute('data-eq-preset')) {
          const k = e.target.value, P = S.fx.PRESETS[k];
          if (P) { eqSet({ preset: k, pre: P.pre, gains: P.gains.slice() }, true); S.app.refresh(); }
          return;
        }
        if (e.target.dataset && e.target.dataset.setting === 'eqOn') {
          S.store.update(function (s) { s.settings.eq = Object.assign({ preset: 'flat', pre: 0, gains: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0] }, s.settings.eq, { on: e.target.checked }); });
          S.fx.wake(); S.fx.sync(); S.toast(e.target.checked ? 'Equalizer on' : 'Equalizer off', 'check'); S.app.refresh(); return;
        }
        if (e.target.matches && e.target.matches('[data-pfp-file]')) { if (e.target.files && e.target.files[0]) { pictureDialog(e.target.files[0]); e.target.value = ''; } return; }
        if (e.target.dataset && e.target.dataset.native) {
          const k = e.target.dataset.native; window.VOLVIX_NATIVE.settings[k] = e.target.checked;
          nativeSend({ type: 'setting', key: k, value: e.target.checked });
          S.toast(e.target.checked ? 'Turned on' : 'Turned off', 'check'); return;
        }
        const key = e.target.dataset && e.target.dataset.setting;
        if (!key) return;
        S.store.update(function (s) { s.settings[key] = e.target.checked; });
        if (S.household) S.household.syncPresence();
        if (key === 'offlineOnly') { S.dl.refreshOffline(); S.toast(e.target.checked ? 'Only downloaded songs play' : 'All songs play again', 'download'); return; }
        if (key === 'downloadMobile') { S.toast(e.target.checked ? 'Downloads may use mobile data' : 'Downloads wait for Wi-Fi', 'download'); return; }
        if (key === 'crossfadeAlbums') { S.toast(e.target.checked ? 'Crossfading within albums too' : 'Albums play straight through', 'check'); return; }
        if (key === 'resumeLast') { S.toast(e.target.checked ? 'Will resume where you left off' : 'Will start fresh each time'); return; }
        S.toast(key === 'privateSession' ? (e.target.checked ? 'Private session on' : 'Private session off') : (e.target.checked ? 'Sharing what you play' : 'Not sharing what you play'));
      };

      root.onclick = function (e) {
        const segBtn = e.target.closest('[data-seg]');
        if (segBtn && segBtn.dataset.seg === 'theme') { S.setTheme(segBtn.dataset.value); S.app.refresh(); return; }
        if (segBtn && segBtn.dataset.seg === 'streamq') {
          try { localStorage.setItem('vh_streamq', segBtn.dataset.value); } catch (x) { /* ignore */ }
          S.toast(segBtn.dataset.value === 'original' ? 'Streaming in original quality' : 'Streaming smaller files from the next song', 'wifi'); S.app.refresh(); return;
        }
        if (segBtn && segBtn.dataset.seg === 'downloadq') {
          S.store.update(function (s) { s.settings.downloadQuality = segBtn.dataset.value; });
          S.toast('New downloads: ' + segBtn.textContent.trim().toLowerCase(), 'download'); S.app.refresh(); return;
        }
        if (segBtn && (segBtn.dataset.seg === 'crossfade' || segBtn.dataset.seg === 'leveling')) {
          const k = segBtn.dataset.seg, v = k === 'crossfade' ? Number(segBtn.dataset.value) : segBtn.dataset.value;
          S.store.update(function (s) { s.settings[k] = v; });
          S.fx.wake(); S.fx.sync();
          S.toast(k === 'crossfade' ? (v ? 'Crossfade ' + v + ' s' : 'Crossfade off') : (v === 'off' ? 'Volume leveling off' : 'Leveling by ' + (v === 'album' ? 'album' : 'song')), 'check');
          S.app.refresh(); return;
        }
        if (e.target.closest('[data-act="eq-reset"]')) { eqSet({ preset: 'flat', pre: 0, gains: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0] }, true); S.app.refresh(); return; }
        if (segBtn && segBtn.dataset.seg === 'sleep') {
          const v = segBtn.dataset.value;
          if (v !== '0' && !S.player.current()) { S.toast('Start a song first', 'moon'); return; }
          sleepChoice = v; S.player.sleep(v === 'song' ? 'song' : Number(v));
          S.toast(v === '0' ? 'Sleep timer off' : 'Sleep timer set', 'moon'); S.app.refresh(); return;
        }
        const act = e.target.closest('[data-act]');
        if (act) {
          if (act.dataset.act === 'logout') S.server.logout();
          else if (act.dataset.act === 'pfp-pick') { const fi = root.querySelector('[data-pfp-file]'); if (fi) fi.click(); }
          else if (act.dataset.act === 'native-check') { nativeSend({ type: 'checkUpdate' }); S.toast('Checking for updates…', 'download'); }
          else if (act.dataset.act === 'native-restart') nativeSend({ type: 'restart' });
          else if (act.dataset.act === 'dl-clear') { S.dl.removeAll().then(function () { S.toast('Downloads removed', 'check'); }); }
          else if (act.dataset.act === 'pfp-remove') {
            fetch('/api/me/avatar', { method: 'DELETE', credentials: 'same-origin' }).then(function () { S.data.me.avatar = 0; S.toast('Picture removed', 'check'); meChanged(); });
          }
          else if (act.dataset.act === 'pick-folder') pickFolder();
          else if (act.dataset.act === 'switch-server') {
            // iPhone: tell the app; Windows: open the app's built-in connect page
            if (window.webkit && window.webkit.messageHandlers && window.webkit.messageHandlers.volvixhome) window.webkit.messageHandlers.volvixhome.postMessage('disconnect');
            else location.href = 'http://vh.localhost/connect.html';
          }
          else if (act.dataset.act === 'lidarr-off') {
            S.server.json('PUT', '/api/lidarr', { url: '' }).then(function (r) { lidarr = r; S.toast('Lidarr disconnected'); S.app.refresh(); });
          }
          else if (act.dataset.act === 'reload') location.reload();
          else if (act.dataset.act === 'audio-log') {
            S.modal({
              title: 'Playback log',
              body: '<p class="modal-text">What the audio did, newest last. Play a song, switch to another app, come back, then open this.</p><pre class="dj-script" style="white-space:pre-wrap;padding:10px;font-size:12px;max-height:50vh;overflow:auto">' + S.esc(S.audioLog.text()) + '</pre>',
              actions: [{ label: 'Clear', kind: 'secondary', onClick: function (close) { S.audioLog.clear(); close(); } }, { label: 'Close', kind: 'primary' }]
            });
          }
          else if (act.dataset.act === 'rescan') {
            S.server.json('POST', '/api/library/rescan').then(function () {
              S.toast('Checking the music folder…', 'library');
              if (status) status.scanning = true;
              S.app.refresh();
              syncWhenScanned(); // updates the page in place when the check is done
            }).catch(function (x) { S.toast(x.message, 'x'); });
          }
          return;
        }
        const sc = e.target.closest('[data-share-copy]');
        if (sc) { (navigator.clipboard ? navigator.clipboard.writeText(sc.dataset.shareCopy) : Promise.reject()).then(function () { S.toast('Link copied', 'check'); }).catch(function () { S.toast(sc.dataset.shareCopy, 'copy'); }); return; }
        const sx = e.target.closest('[data-share-cancel]');
        if (sx) { S.server.json('DELETE', '/api/shares/' + sx.dataset.shareCancel).then(function () { myShares = null; S.toast('Link cancelled', 'check'); S.app.refresh(); }).catch(function (x) { S.toast(x.message, 'x'); }); return; }
        const copy = e.target.closest('[data-copy]');
        if (copy) {
          (navigator.clipboard ? navigator.clipboard.writeText(copy.dataset.copy) : Promise.reject()).then(function () { S.toast('Copied', 'check'); })
            .catch(function () { S.toast('Select the URL below and copy it', 'copy'); });
          return;
        }
        const mg = e.target.closest('[data-manage-user]');
        if (mg) manageDialog(mg.dataset.manageUser);
      };

      root.onsubmit = function (e) {
        const form = e.target.closest('[data-form]');
        if (!form) return;
        e.preventDefault();
        const f = new FormData(form);
        const err = form.querySelector('.auth-error');
        const btn = form.querySelector('[type=submit]');
        err.hidden = true; btn.disabled = true;
        const done = function () { btn.disabled = false; };
        const fail = function (x) { err.textContent = x.message; err.hidden = false; done(); };
        if (form.dataset.form === 'profile-name') {
          S.server.json('PUT', '/api/me/profile', { name: f.get('name') }).then(function (r) {
            S.data.me.name = r.user.name; done(); S.toast('Name saved', 'check'); meChanged();
          }).catch(fail);
        } else if (form.dataset.form === 'password') {
          S.server.json('POST', '/api/me/password', { current: f.get('current'), new: f.get('new') })
            .then(function () { form.reset(); done(); S.toast('Password changed', 'check'); }).catch(fail);
        } else if (form.dataset.form === 'lidarr') {
          S.server.json('PUT', '/api/lidarr', {
            url: f.get('url') || '', apiKey: f.get('apiKey') || '', rootFolder: f.get('rootFolder') || '',
            qualityProfileId: Number(f.get('qualityProfileId') || 0), metadataProfileId: Number(f.get('metadataProfileId') || 0)
          }).then(function (r) {
            const first = !lidarr || !lidarr.configured;
            lidarr = r; done();
            S.toast(first ? 'Lidarr connected' : 'Saved', 'check');
            // the Requests tab and Get buttons appear after a reload
            // Requests joins the menu right now: no reload
            if (first && window.VH && window.VH.lidarr) { window.VH.lidarr.configured = true; window.VH.lidarr.canRequest = true; if (S.getMusic && S.getMusic.helpers) S.getMusic.helpers.setCan(true, true); S.app.refreshChrome(); }
            S.app.refresh();
          }).catch(fail);
        } else if (form.dataset.form === 'add-user') {
          S.server.json('POST', '/api/users', { name: f.get('name'), password: f.get('password') })
            .then(function (r) { users = null; S.toast(r.user.name + ' can now sign in', 'check'); S.app.refresh(); }).catch(fail);
        }
      };
    }
  };
})(window.Spin);
