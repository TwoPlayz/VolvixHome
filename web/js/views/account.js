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

  /** Profile picture: crop to a centered square, shrink to 256px, send as a JPEG. */
  function uploadPicture(file) {
    const img = new Image();
    const url = URL.createObjectURL(file);
    img.onload = function () {
      const side = Math.min(img.width, img.height), size = 256;
      const c = document.createElement('canvas');
      c.width = c.height = size;
      c.getContext('2d').drawImage(img, (img.width - side) / 2, (img.height - side) / 2, side, side, 0, 0, size, size);
      URL.revokeObjectURL(url);
      c.toBlob(function (blob) {
        fetch('/api/me/avatar', { method: 'PUT', credentials: 'same-origin', headers: { 'Content-Type': 'image/jpeg' }, body: blob })
          .then(function (r) { return r.ok ? r.json() : r.json().then(function (j) { throw new Error(j.error || 'Couldn\'t save the picture.'); }); })
          .then(function () { S.toast('Picture updated', 'check'); setTimeout(function () { location.reload(); }, 600); })
          .catch(function (x) { S.toast(x.message, 'x'); });
      }, 'image/jpeg', 0.88);
    };
    img.onerror = function () { URL.revokeObjectURL(url); S.toast('Couldn\'t read that picture.', 'x'); };
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
  /** Wait for the server to finish scanning, then reload so the new music shows. */
  function reloadWhenScanned() {
    const poll = setInterval(function () {
      S.server.json('GET', '/api/status').then(function (s) {
        if (!s.scanning) { clearInterval(poll); S.toast(s.albums + ' albums found', 'check'); setTimeout(function () { location.reload(); }, 900); }
      }).catch(function () {});
    }, 1200);
  }
  /** Pick the music folder by browsing the server's folders (so no one has to type a path). */
  function pickFolder() {
    let current = '';
    const m = S.modal({
      title: 'Choose your music folder',
      body: '<p class="modal-text">Pick the folder that holds your music (the one Lidarr saves to). Volvix looks inside it, including subfolders.</p><div class="fb" data-fb><p class="set-hint">Loading…</p></div>',
      actions: [
        { label: 'Cancel' },
        { label: 'Use this folder', kind: 'primary', onClick: function (close) {
          S.server.json('PUT', '/api/library/folder', { path: current }).then(function (r) {
            close(); folder = null; S.toast('Using ' + r.path, 'check'); S.app.refresh(); reloadWhenScanned();
          }).catch(function (x) { S.toast(x.message, 'x'); });
        } }
      ]
    });
    const host = function () { return m.el.querySelector('[data-fb]'); };
    function show(path) {
      S.server.json('GET', '/api/library/browse' + (path ? '?path=' + encodeURIComponent(path) : '')).then(function (r) {
        current = r.path;
        host().innerHTML =
          '<div class="fb-path">' + S.icon('library', 15) + '<span>' + S.esc(r.path) + '</span></div>' +
          '<div class="fb-note">' + (r.songsHere ? S.icon('check', 14) + r.songsHere + ' song' + (r.songsHere === 1 ? '' : 's') + ' directly in this folder' : 'No songs directly in this folder (subfolders count too)') + '</div>' +
          '<div class="fb-list">' + (r.parent ? '<button class="fb-row" data-dir="' + S.esc(r.parent) + '">' + S.icon('arrowUp', 15) + '<span>Up one level</span></button>' : '') +
            r.dirs.map(function (d) { return '<button class="fb-row" data-dir="' + S.esc(d.path) + '">' + S.icon('library', 15) + '<span>' + S.esc(d.name) + '</span></button>'; }).join('') +
            (!r.dirs.length ? '<p class="set-hint">No folders in here.</p>' : '') + '</div>';
      }).catch(function (x) { host().innerHTML = '<p class="auth-error">' + S.esc(x.message) + '</p>'; });
    }
    m.el.addEventListener('click', function (e) { const b = e.target.closest('[data-dir]'); if (b) show(b.dataset.dir); });
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
  function eqGroup(st) {
    const eq = st.settings.eq || { on: false, preset: 'flat', pre: 0, gains: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0] };
    const P = S.fx.PRESETS, B = S.fx.BANDS;
    const label = function (f) { return f >= 1000 ? (f / 1000) + 'k' : String(f); };
    const bands = B.map(function (f, i) {
      return '<label class="eq-band"><span class="eq-db" data-eq-db="' + i + '">' + (eq.gains[i] > 0 ? '+' : '') + eq.gains[i] + '</span>' +
        '<input type="range" class="eq-slider" min="-12" max="12" step="1" value="' + eq.gains[i] + '" data-eq-band="' + i + '" aria-label="' + label(f) + ' Hz"' + (eq.on ? '' : ' disabled') + '>' +
        '<span class="eq-hz">' + label(f) + '</span></label>';
    }).join('');
    return group('Equalizer',
      row('Equalizer', S.fx.available() ? 'Shape the sound with ten bands. It works on this device while it plays (not when casting to a TV).' : 'This browser can\'t process sound.', toggle('eqOn', !!eq.on), { icon: 'settings' }) +
      '<div class="eq-panel"' + (eq.on ? '' : ' data-off') + '>' +
        '<div class="eq-top"><label class="eq-pick"><span>Preset</span><select class="input" data-eq-preset' + (eq.on ? '' : ' disabled') + '>' +
          Object.keys(P).map(function (k) { return '<option value="' + k + '"' + (eq.preset === k ? ' selected' : '') + '>' + S.esc(P[k].label) + '</option>'; }).join('') +
          '<option value="custom"' + (eq.preset === 'custom' ? ' selected' : '') + '>Custom</option></select></label>' +
          '<label class="eq-pick"><span>Preamp <b data-eq-prev>' + (eq.pre > 0 ? '+' : '') + eq.pre + ' dB</b></span><input type="range" min="-12" max="6" step="1" value="' + eq.pre + '" data-eq-pre' + (eq.on ? '' : ' disabled') + '></label>' +
          '<button class="btn btn-ghost btn-sm" data-act="eq-reset"' + (eq.on ? '' : ' disabled') + '>Reset</button></div>' +
        '<div class="eq-bands">' + bands + '</div>' +
      '</div>', { id: 'equalizer' });
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
        row('Resume where I left off', 'Reopening Volvix shows the last song and spot, paused.', toggle('resumeLast', st.settings.resumeLast !== false), { icon: 'history' }),
        { id: 'playback' }) +
      nativeGroup() +
      shareGroup() +
      eqGroup(st) +
      group('Downloads',
        row('Only play downloads', 'Use only the songs saved on this device. Everything else is greyed out and skipped. This also happens by itself when you lose your connection.',
          toggle('offlineOnly', !!st.settings.offlineOnly), { icon: 'wifiOff' }) +
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
      // the equalizer sliders move the sound while you drag; letting go saves
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
        if (e.target.matches && e.target.matches('[data-pfp-file]')) { if (e.target.files && e.target.files[0]) uploadPicture(e.target.files[0]); return; }
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
            fetch('/api/me/avatar', { method: 'DELETE', credentials: 'same-origin' }).then(function () { S.toast('Picture removed', 'check'); setTimeout(function () { location.reload(); }, 600); });
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
          else if (act.dataset.act === 'rescan') {
            S.server.json('POST', '/api/library/rescan').then(function () {
              S.toast('Looking for new music…', 'library');
              if (status) status.scanning = true;
              S.app.refresh();
              // check back until the scan is done
              const poll = setInterval(function () {
                S.server.json('GET', '/api/status').then(function (s) {
                  status = s;
                  if (!s.scanning) { clearInterval(poll); S.linkToast('Library updated: ' + s.albums + ' albums', 'Reload', '/home'); setTimeout(function () { location.reload(); }, 1500); }
                });
              }, 1500);
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
        if (form.dataset.form === 'password') {
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
            if (first) setTimeout(function () { location.reload(); }, 900); else S.app.refresh();
          }).catch(fail);
        } else if (form.dataset.form === 'add-user') {
          S.server.json('POST', '/api/users', { name: f.get('name'), password: f.get('password') })
            .then(function (r) { users = null; S.toast(r.user.name + ' can now sign in', 'check'); S.app.refresh(); }).catch(fail);
        }
      };
    }
  };
})(window.Spin);
