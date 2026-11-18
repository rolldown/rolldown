import * as fooNamespace from './foo.js';
import assert from 'node:assert/strict';

assert.deepEqual(
  fooNamespace,
  Object.defineProperty(
    {
      __proto__: null,
      foo: 1,
    },
    Symbol.toStringTag,
    { value: 'Module' },
  ),
);

import('./foo.js').then((mod) => {
  assert.deepEqual(mod, fooNamespace);
});
