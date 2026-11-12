/* Volvix Home — on a touch screen there is no hovering, but browsers keep a tapped button in its ":hover" look until the
   next tap somewhere else. So where the device can't hover, the hover styles are taken out of the stylesheet: a tapped
   button no longer stays lit (buttons that open something still show their open state: that is aria-expanded / :active,
   not :hover). */
(function () {
  'use strict';
  if (!window.matchMedia || !window.matchMedia('(hover: none)').matches) return;

  function strip(rules) {
    for (let i = rules.length - 1; i >= 0; i--) {
      const r = rules[i];
      if (r.cssRules && !(r instanceof CSSStyleRule)) { strip(r.cssRules); continue; } // @media, @supports…
      if (r.selectorText && r.selectorText.indexOf(':hover') !== -1) {
        // keep the other selectors of a comma list (".a:hover, .a:focus-visible")
        const keep = r.selectorText.split(',').filter(function (s) { return s.indexOf(':hover') === -1; });
        try {
          if (!keep.length) (r.parentRule || r.parentStyleSheet).deleteRule(i);
          else r.selectorText = keep.join(',');
        } catch (e) { /* a rule that can't be edited just keeps its hover look */ }
      }
    }
  }

  function run() {
    Array.prototype.forEach.call(document.styleSheets, function (sheet) {
      try { strip(sheet.cssRules); } catch (e) { /* a stylesheet from another site can't be read */ }
    });
  }
  if (document.readyState === 'complete') run(); else window.addEventListener('load', run);
})();
