/* Volvix Home — notifications: the bell in the top bar, polling for new ones, and a toast when something
 * arrives (a playlist invite, a party invite, music you requested being ready). */
window.Spin = window.Spin || {};
(function (S) {
  'use strict';

  const VH = window.VH || {};
  let items = (VH.notifications && VH.notifications.items) || [];
  // what we already knew about at start-up, so old notifications don't pop up as toasts
  const seen = new Set(items.map(function (n) { return n.id; }));

  function unread() { return items.filter(function (n) { return !n.read; }).length; }

  /** "just now", "5 min ago", "2 h ago", "3 d ago" for a unix-seconds time. */
  S.ago = function (sec) {
    const d = Math.max(0, Date.now() / 1000 - sec);
    return d < 60 ? 'just now' : d < 3600 ? Math.round(d / 60) + ' min ago' : d < 86400 ? Math.round(d / 3600) + ' h ago' : Math.round(d / 86400) + ' d ago';
  };

  S.notify = {
    items: function () { return items; },
    unread: unread,
    bellButton: function () {
      return '<a class="bell" href="#/notifications" data-bell aria-label="Notifications" title="Notifications">' + S.icon('bell', 20) +
        '<span class="bell-badge" data-bell-badge hidden></span></a>';
    },
    updateBell: function () {
      const n = unread();
      S.$$('[data-bell-badge]').forEach(function (b) { b.hidden = !n; b.textContent = n > 99 ? '99+' : n; });
      S.$$('[data-bell]').forEach(function (b) { b.setAttribute('aria-label', n ? 'Notifications, ' + n + ' unread' : 'Notifications'); });
    },
    /** Fetch the latest; announce anything new with a toast. */
    load: function () {
      return S.server.json('GET', '/api/notifications').then(function (r) {
        const fresh = r.items.filter(function (n) { return !seen.has(n.id) && !n.read; });
        items = r.items;
        r.items.forEach(function (n) { seen.add(n.id); });
        S.notify.updateBell();
        fresh.reverse().forEach(S.notify.announce);
        // the open page may show them
        if (S.router.current() === '/notifications' && S.views.notifications.redraw) S.views.notifications.redraw();
        return r;
      });
    },
    /** A toast for a new notification. Party invites get a Join button (see party.js). */
    announce: function (n) {
      if (n.kind === 'party_invite' && S.party && S.party.joinDialog) { S.linkToast(n.text, 'Join', null, function () { S.party.joinFromNote(n); }); return; }
      S.linkToast(n.text, 'View', '/notifications');
    },
    markRead: function (ids) {
      const body = ids ? { ids: ids } : {};
      items.forEach(function (n) { if (!ids || ids.indexOf(n.id) !== -1) n.read = true; });
      S.notify.updateBell();
      return S.server.json('POST', '/api/notifications/read', body).catch(function () { /* next poll fixes the badge */ });
    },
    /** One of my invites was answered (here or elsewhere): reload to see its new state. */
    setDone: function (id, status) {
      const n = items.find(function (x) { return x.id === id; });
      if (n) { n.done = status; n.read = true; }
      S.notify.updateBell();
    }
  };

  setInterval(function () { if (!document.hidden) S.notify.load().catch(function () { /* offline */ }); }, 12000);
  document.addEventListener('visibilitychange', function () { if (!document.hidden) S.notify.load().catch(function () {}); });
})(window.Spin);
