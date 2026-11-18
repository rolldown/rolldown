import { count as a } from './a\uD800.js';
import { count as b } from './b\uD800.js';

globalThis.parentRuns = (globalThis.parentRuns ?? 0) + 1;
globalThis.saw = [a, b];

// Each lone surrogate becomes U+FFFD, so these accept `a\u{FFFD}.js` and `b\u{FFFD}.js`.
import.meta.hot.accept('./a\uD800.js', (mod) => {
  globalThis.saw[0] = mod.count;
});
import.meta.hot.accept([`./b\uD800.js`], ([mod]) => {
  globalThis.saw[1] = mod.count;
});
