(globalThis.sideEffectLog ??= []).push('b');

import('./shared.js').then(({ foo }) => {
  foo();
});
