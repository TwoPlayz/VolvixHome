/* Volvix Home — artist pictures: their photo (a rounded square) when there is one, else their initials. Never an album cover.
   Used wherever artists are listed (Home, Your Library). Photos load a few at a time and are remembered. */
window.Spin = window.Spin || {};
(function (S) {
  'use strict';
  const known = {}; // lower-case name -> photo url ('' when there is none)
  const waiting = [];
  let active = 0;

  const initials = function (name) {
    return String(name).split(/\s+/).filter(function (w) { return /[a-z0-9]/i.test(w); }).map(function (w) { return w[0]; }).slice(0, 2).join('').toUpperCase();
  };
  const img = function (src) { return '<img src="' + S.esc(src) + '" alt="" loading="lazy">'; };

  /** The picture for an artist, as markup: the photo if it is already known, else their initials (hydrate() fills it in). */
  S.artistPic = function (name) {
    const k = String(name).toLowerCase();
    return '<span class="artist-pic" data-artist-pic="' + S.esc(name) + '" style="--tint:' + S.tintFor(name) + '">' + (known[k] ? img(known[k]) : S.esc(initials(name))) + '</span>';
  };

  function pump() {
    while (active < 3 && waiting.length) {
      const job = waiting.shift();
      active++;
      S.server.json('GET', '/api/artist/info?name=' + encodeURIComponent(job.name)).then(function (r) { known[job.key] = (r && r.image) || ''; })
        .catch(function () { known[job.key] = ''; })
        .then(function () { active--; job.done(known[job.key]); pump(); });
    }
  }

  /** Put photos into the artist pictures under root (call after the markup is in the page). */
  S.hydrateArtistPics = function (root) {
    if (!(window.VH && window.VH.artistInfo)) return;
    S.$$('[data-artist-pic]', root).forEach(function (el) {
      const name = el.dataset.artistPic, key = name.toLowerCase();
      const fill = function (src) { if (src && document.body.contains(el) && !el.querySelector('img')) el.innerHTML = img(src); };
      if (key in known) return fill(known[key]);
      waiting.push({ name: name, key: key, done: fill });
    });
    pump();
  };
})(window.Spin);
