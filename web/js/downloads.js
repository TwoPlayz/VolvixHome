/* Volvix Home — the website doesn't keep downloads (the phone and Windows apps do). This only clears what an older
   version left in the browser, so it doesn't sit there using up space. */
(function () {
  'use strict';
  try {
    if (typeof caches !== 'undefined') caches.delete('vh-audio').catch(function () {});
    Object.keys(localStorage).forEach(function (k) { if (k.indexOf('vh_dl_') === 0) localStorage.removeItem(k); });
  } catch (e) { /* private window or blocked storage */ }
})();
