/* Volvix — tiny hash router: #/album/al_tidewater → { view, params: { id } } */
window.Spin = window.Spin || {};
(function (S) {
  'use strict';

  const routes = [];

  S.router = {
    add: function (pattern, view) {
      const keys = [];
      const source = pattern.replace(/:([A-Za-z]+)/g, function (_, key) { keys.push(key); return '([^/]+)'; });
      routes.push({ pattern: pattern, re: new RegExp('^' + source + '/?$'), keys: keys, view: view });
    },
    resolve: function (path) {
      for (const r of routes) {
        const m = path.match(r.re);
        if (m) {
          const params = {};
          r.keys.forEach(function (k, i) { params[k] = decodeURIComponent(m[i + 1]); });
          return { view: r.view, params: params, pattern: r.pattern };
        }
      }
      return null;
    },
    current: function () {
      return location.hash.replace(/^#/, '') || '/home';
    },
    go: function (path) {
      if (location.hash === '#' + path) S.app.render(path);
      else location.hash = path;
    },
    replace: function (path) {
      history.replaceState(null, '', location.pathname + location.search + '#' + path);
    },
    start: function () {
      window.addEventListener('hashchange', function () { S.app.render(S.router.current()); });
      S.app.render(S.router.current());
    }
  };
})(window.Spin);
