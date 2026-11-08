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
    return [['profile', 'Profile', 'user']].concat(S.data.me.admin ? [['household', 'Household', 'family']] : []).concat([['settings', 'Settings', 'settings']]);
  };

  /* ---------- Profile ---------- */
  function profileTab() {
    const me = S.data.me;
    return group('You',
      row(S.avatar(me, 48) + '<span class="set-who"><span>' + S.esc(me.name) + '</span><em>@' + S.esc(me.username) + (me.admin ? ' · admin' : '') + '</em></span>', '', '')) +
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
    return group('People on this server',
      users.map(function (u) {
        const me = u.id === S.data.me.serverId;
        return row(S.avatar({ id: u.id, name: u.name }, 36) + '<span class="set-who"><span>' + S.esc(u.name) + (me ? ' (you)' : '') + '</span><em>@' + S.esc(u.username) + (u.admin ? ' · admin' : '') + '</em></span>', '',
          me ? '' : '<button class="btn btn-ghost btn-sm" data-remove-user="' + u.id + '" data-name="' + S.esc(u.name) + '">Remove</button>');
      }).join(''),
      { sub: 'Everyone shares the same music. Playlists, play history and settings are personal.' }) +
      group('Add someone',
        '<form class="set-form" data-form="add-user" novalidate>' +
          field('Name', 'name') + field('Username', 'username', 'text', 'They sign in with this.') +
          field('Password', 'password', 'password', 'At least 8 characters. They can change it under Account.') +
          '<label class="check"><input type="checkbox" name="admin"> <span>Admin (can add people and rescan the library)</span></label>' +
          '<p class="auth-error" hidden></p>' +
          '<button class="btn btn-primary" type="submit">' + S.icon('plus', 16) + 'Add to household</button>' +
        '</form>');
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
        (L ? '<label class="field"><span class="label">Add music to</span>' + select('rootFolder', s.rootFolder, L.rootFolders.map(function (r) { return [r.path, r.path]; })) + '<span class="hint">Your music folder in Lidarr. Volvix Home should read the same folder.</span></label>' +
          '<label class="field"><span class="label">Quality profile</span>' + select('qualityProfileId', s.qualityProfileId, L.qualityProfiles.map(function (p) { return [p.id, p.name]; })) + '</label>' +
          '<label class="field"><span class="label">Metadata profile</span>' + select('metadataProfileId', s.metadataProfileId, L.metadataProfiles.map(function (p) { return [p.id, p.name]; })) + '</label>' +
          '<label class="check"><input type="checkbox" name="everyoneCanRequest"' + (s.everyoneCanRequest !== false ? ' checked' : '') + '> <span>Everyone in the household can request music (otherwise only admins)</span></label>' : '') +
        '<p class="auth-error" hidden></p>' +
        '<div class="set-actions"><button class="btn btn-primary" type="submit">' + (lidarr.configured ? 'Save' : 'Connect') + '</button>' +
          (lidarr.configured ? '<button class="btn btn-ghost" type="button" data-act="lidarr-off">Disconnect</button>' : '') + '</div>' +
      '</form>' +
      (lidarr.configured ? row('New music right away', 'In Lidarr → Settings → Connect, add a <strong>Webhook</strong> with this URL and turn on <em>On Release Import</em>. Otherwise new albums appear within ' + 15 + ' minutes.',
        '<button class="btn btn-secondary btn-sm" data-copy="' + S.esc(hook) + '">' + S.icon('copy', 14) + 'Copy URL</button>', { icon: 'download' }) +
        '<p class="hook-url"><code>' + S.esc(hook) + '</code></p>' : ''),
      { id: 'lidarr', sub: 'Get music you don\'t have yet.' });
  }

  /* ---------- Settings ---------- */
  let status = null;
  function settingsTab() {
    const st = S.store.state;
    if (!status) S.server.json('GET', '/api/status').then(function (s) { status = s; S.app.refresh(); }).catch(function () {});
    return group('Appearance',
        row('Theme', st.theme === 'system' ? 'Matches your device\'s light or dark setting.' : 'Always ' + st.theme + '.',
          seg('theme', st.theme, [['system', 'System', 'monitor'], ['dark', 'Dark', 'moon'], ['light', 'Light', 'sun']]), { icon: 'sun' })) +
      group('Library',
        row('Music on this server', status ? status.albums + ' albums · ' + status.tracks + ' songs' + (status.scanning ? ' · looking for new music…' : '') : '…',
          S.data.me.admin ? '<button class="btn btn-secondary btn-sm" data-act="rescan"' + (status && status.scanning ? ' disabled' : '') + '>Look for new music</button>' : '', { icon: 'library' }) +
        row('New music', 'The server checks the music folder every 15 minutes. Reload to see what\'s new.', '<button class="btn btn-ghost btn-sm" data-act="reload">Reload</button>', { icon: 'download' }),
        { id: 'library' }) +
      lidarrGroup() +
      group('About',
        row('Volvix Home', status ? 'Version ' + status.version : '', '', { icon: 'disc' }) +
        row('1 + 1', 'Simple addition, worked out in the app.', '<strong>' + (1 + 1) + '</strong>', { icon: 'plus' }));
  }

  S.views.account = {
    render: function (root, params) {
      const tabs = TABS();
      const tab = tabs.find(function (t) { return t[0] === params.tab; }) || tabs[0];
      root.innerHTML =
        '<header class="page-head"><h1 class="page-title">' + (tab[0] === 'settings' ? 'Settings' : 'Account') + '</h1></header>' +
        '<div class="acct-layout">' +
          '<nav class="acct-nav" aria-label="Account sections">' + tabs.map(function (t) {
            return '<a class="acct-tab' + (t[0] === tab[0] ? ' active' : '') + '" href="#/account/' + t[0] + '"' + (t[0] === tab[0] ? ' aria-current="page"' : '') + '>' +
              S.icon(t[2], 18) + '<span>' + t[1] + '</span></a>';
          }).join('') + '</nav>' +
          '<div class="acct-body">' + (tab[0] === 'household' ? householdTab() : tab[0] === 'settings' ? settingsTab() : profileTab()) + '</div>' +
        '</div>';

      root.onclick = function (e) {
        const segBtn = e.target.closest('[data-seg]');
        if (segBtn && segBtn.dataset.seg === 'theme') { S.setTheme(segBtn.dataset.value); S.app.refresh(); return; }
        const act = e.target.closest('[data-act]');
        if (act) {
          if (act.dataset.act === 'logout') S.server.logout();
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
        const rm = e.target.closest('[data-remove-user]');
        if (rm) {
          S.modal({
            title: 'Remove ' + rm.dataset.name + '?',
            body: '<p class="modal-text">They won\'t be able to sign in, and their playlists and history are deleted. The music stays.</p>',
            actions: [{ label: 'Cancel' }, { label: 'Remove', kind: 'danger', onClick: function (close) {
              S.server.json('DELETE', '/api/users/' + rm.dataset.removeUser).then(function () {
                close(); users = null; S.toast(rm.dataset.name + ' removed', 'check'); S.app.refresh();
              }).catch(function (x) { close(); S.toast(x.message, 'x'); });
            } }]
          });
        }
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
          S.server.json('POST', '/api/users', { name: f.get('name'), username: f.get('username'), password: f.get('password'), admin: !!f.get('admin') })
            .then(function (r) { users = null; S.toast(r.user.name + ' can sign in as @' + r.user.username, 'check'); S.app.refresh(); }).catch(fail);
        }
      };
    }
  };
})(window.Spin);
