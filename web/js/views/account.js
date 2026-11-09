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
    return [['profile', 'Profile', 'user']].concat(S.data.me.admin ? [['members', 'Members', 'family']] : []).concat([['settings', 'Settings', 'settings']]);
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
        const notes = [u.admin ? 'Admin' : 'Member'].concat(u.hideExplicit ? ['Explicit off'] : []).join(' · ');
        return row(S.avatar({ id: u.id, name: u.name }, 36) + '<span class="set-who"><span>' + S.esc(u.name) + (me ? ' (you)' : '') + '</span><em>' + notes + '</em></span>', '',
          me ? '' : '<button class="btn btn-secondary btn-sm" data-manage-user="' + u.id + '">Manage</button>');
      }).join(''),
      { sub: 'Everyone shares the same music. Playlists, play history and settings are personal.' }) +
      group('Add a member',
        '<form class="set-form" data-form="add-user" novalidate>' +
          field('Name', 'name', 'text', 'They sign in with this name.') +
          field('Password', 'password', 'password', 'At least 8 characters. They can change it under Account.') +
          '<label class="check"><input type="checkbox" name="admin"> <span>Admin (can add people and rescan the library)</span></label>' +
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
        '<div class="mg-who">' + S.avatar({ id: u.id, name: u.name }, 48) + '<div><strong>' + S.esc(u.name) + '</strong><span>' + (u.admin ? 'Admin' : 'Member') + '</span></div></div>' +
        sw('hideExplicit', 'Turn off explicit music', 'Songs marked explicit stay listed but are skipped for them.', u.hideExplicit) +
        sw('admin', 'Admin', 'Can add and manage members and change the library.', u.admin) +
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
    m.el.addEventListener('change', function (e) {
      const key = e.target.dataset && e.target.dataset.mg;
      if (!key) return;
      const body = {}; body[key] = e.target.checked;
      S.server.json('PUT', '/api/users/' + id, body).then(function (r) {
        Object.assign(u, r.user);
        S.toast(key === 'hideExplicit' ? (e.target.checked ? 'Explicit music is off for ' + u.name : 'Explicit music is on for ' + u.name) : (e.target.checked ? u.name + ' is now an admin' : u.name + ' is now a member'), 'check');
        S.app.refresh();
      }).catch(function (x) { e.target.checked = !e.target.checked; err(x); });
    });
    m.el.addEventListener('submit', function (e) {
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
    if (!S.data.me.admin) return '';
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
          '<label class="check"><input type="checkbox" name="everyoneCanRequest"' + (s.everyoneCanRequest !== false ? ' checked' : '') + '> <span>Everyone in the household can request music (otherwise only admins)</span></label>' : '') +
        '<p class="auth-error" hidden></p>' +
        '<div class="set-actions"><button class="btn btn-primary" type="submit">' + (lidarr.configured ? 'Save' : 'Connect') + '</button>' +
          (lidarr.configured ? '<button class="btn btn-ghost" type="button" data-act="lidarr-off">Disconnect</button>' : '') + '</div>' +
      '</form>' +
      (lidarr.configured ? row('New music right away', 'In Lidarr → Settings → Connect, add a <strong>Webhook</strong> with this URL and turn on <em>On Release Import</em>. Otherwise new albums appear at the next automatic check.',
        '<button class="btn btn-secondary btn-sm" data-copy="' + S.esc(hook) + '">' + S.icon('copy', 14) + 'Copy URL</button>', { icon: 'download' }) +
        '<p class="hook-url"><code>' + S.esc(hook) + '</code></p>' : ''),
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

  /* ---------- Settings ---------- */
  let status = null;
  function settingsTab() {
    const st = S.store.state;
    if (!status) S.server.json('GET', '/api/status').then(function (s) { status = s; S.app.refresh(); }).catch(function () {});
    if (!folder && S.data.me.admin) S.server.json('GET', '/api/library/folder').then(function (f) { folder = f; S.app.refresh(); }).catch(function () {});
    return group('Appearance',
        row('Theme', st.theme === 'system' ? 'Matches your device\'s light or dark setting.' : 'Always ' + st.theme + '.',
          seg('theme', st.theme, [['system', 'System', 'monitor'], ['dark', 'Dark', 'moon'], ['light', 'Light', 'sun']]), { icon: 'sun' })) +
      group('Privacy',
        row('Share what I\'m listening to', 'Others in your household see your current song on the Household page.', toggle('shareActivity', st.settings.shareActivity !== false), { icon: 'users' }) +
        row('Private session', 'Stops sharing, and keeps what you play out of Recently played, until you turn it off.', toggle('privateSession', !!st.settings.privateSession), { icon: 'eyeOff' }),
        { id: 'privacy' }) +
      group('Library',
        (S.data.me.admin ? row('Music folder', folder ? S.esc(folder.path) + (folder.exists ? '' : ' · folder not found') + (folder.isDefault ? '' : ' · custom') : '…',
          '<button class="btn btn-secondary btn-sm" data-act="pick-folder">Change</button>', { icon: 'library' }) : '') +
        row('Music on this server', (status ? status.albums + ' albums · ' + status.tracks + ' songs' + (status.scanning ? ' · checking the folder…' : '') + '<br>' : '…') +
          (status && status.rescanMinutes ? 'The server checks the folder by itself every ' + status.rescanMinutes + ' minutes' : 'Automatic checks are off') +
          (lidarr && lidarr.configured ? ' and right after Lidarr imports something' : '') + '. New and removed files are both picked up, and you\'re told when to reload.',
          S.data.me.admin ? '<button class="btn btn-secondary btn-sm" data-act="rescan"' + (status && status.scanning ? ' disabled' : '') + '>Check now</button>' : '', { icon: 'library' }),
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

      root.onchange = function (e) {
        if (e.target.matches && e.target.matches('[data-pfp-file]')) { if (e.target.files && e.target.files[0]) uploadPicture(e.target.files[0]); return; }
        const key = e.target.dataset && e.target.dataset.setting;
        if (!key) return;
        S.store.update(function (s) { s.settings[key] = e.target.checked; });
        if (S.household) S.household.syncPresence();
        S.toast(key === 'privateSession' ? (e.target.checked ? 'Private session on' : 'Private session off') : (e.target.checked ? 'Sharing what you play' : 'Not sharing what you play'));
      };

      root.onclick = function (e) {
        const segBtn = e.target.closest('[data-seg]');
        if (segBtn && segBtn.dataset.seg === 'theme') { S.setTheme(segBtn.dataset.value); S.app.refresh(); return; }
        const act = e.target.closest('[data-act]');
        if (act) {
          if (act.dataset.act === 'logout') S.server.logout();
          else if (act.dataset.act === 'pfp-pick') { const fi = root.querySelector('[data-pfp-file]'); if (fi) fi.click(); }
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
            qualityProfileId: Number(f.get('qualityProfileId') || 0), metadataProfileId: Number(f.get('metadataProfileId') || 0),
            everyoneCanRequest: f.has('everyoneCanRequest') ? true : !form.querySelector('[name=everyoneCanRequest]')
          }).then(function (r) {
            const first = !lidarr || !lidarr.configured;
            lidarr = r; done();
            S.toast(first ? 'Lidarr connected' : 'Saved', 'check');
            // the Requests tab and Get buttons appear after a reload
            if (first) setTimeout(function () { location.reload(); }, 900); else S.app.refresh();
          }).catch(fail);
        } else if (form.dataset.form === 'add-user') {
          S.server.json('POST', '/api/users', { name: f.get('name'), password: f.get('password'), admin: !!f.get('admin') })
            .then(function (r) { users = null; S.toast(r.user.name + ' can now sign in', 'check'); S.app.refresh(); }).catch(fail);
        }
      };
    }
  };
})(window.Spin);
