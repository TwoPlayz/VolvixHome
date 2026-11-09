/* Volvix Home — the first-time intro: a short walkthrough the first time someone signs in. */
window.Spin = window.Spin || {};
(function (S) {
  'use strict';

  function slides() {
    const me = S.data.me;
    return [
      { icon: 'music', title: 'Welcome, ' + me.name.split(' ')[0], text: 'This is your household\'s own music service. Everything here plays from your family\'s library, in its original quality.' },
      { icon: 'heart', title: 'Make it yours', text: 'Tap the heart to like a song and the + to add it to a playlist. Your playlists, history and settings are yours and follow you to every device.' },
      { icon: 'users', title: 'Listen together', text: 'The Household page shows what everyone is playing. Start a listening party from the player to share one queue, in the same room or across town.' },
      { icon: 'search', title: 'Missing something?', text: 'Search for music that isn\'t here yet and ask for it. Once it\'s downloaded it appears on its own.' }
    ];
  }

  function show() {
    const list = slides();
    let i = 0, m = null;
    function paint() {
      const s = list[i], last = i === list.length - 1;
      m.el.querySelector('[data-intro]').innerHTML =
        '<div class="intro"><div class="intro-icon">' + S.icon(s.icon, 30) + '</div><h2>' + S.esc(s.title) + '</h2><p>' + S.esc(s.text) + '</p>' +
        '<div class="intro-dots">' + list.map(function (_, k) { return '<i' + (k === i ? ' class="on"' : '') + '></i>'; }).join('') + '</div>' +
        '<div class="intro-actions">' + (i ? '<button class="btn btn-ghost" data-intro-back>Back</button>' : '<span></span>') +
        '<button class="btn btn-primary" data-intro-next>' + (last ? 'Get started' : 'Next') + '</button></div></div>';
    }
    m = S.modal({ title: '', body: '<div data-intro></div>', actions: [] });
    paint();
    m.el.addEventListener('click', function (e) {
      if (e.target.closest('[data-intro-back]')) { i--; paint(); }
      else if (e.target.closest('[data-intro-next]')) { if (i < list.length - 1) { i++; paint(); } else m.close(); }
    });
    S.store.update(function (s) { s.introSeen = true; });
  }

  S.intro = { show: show };
  // first time on this account: show it once the app is up
  setTimeout(function () { if (!S.store.state.introSeen) show(); }, 900);
})(window.Spin);
