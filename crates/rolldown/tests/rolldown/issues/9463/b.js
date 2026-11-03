// Entry `b` reaches `shared.js` only through a dynamic import, so executing `b`
// must not trigger entry `a`'s top-level side effect.
(globalThis.sideEffectLog ??= []).push('b');

import('./shared.js').then(({ foo }) => {
  foo();
});
