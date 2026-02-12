import * as fooNamespace from './foo.js';
import assert from 'node:assert';

assert.deepEqual(fooNamespace, {
  [Symbol.toStringTag]: 'Module',
  foo: 1,
});

import('./foo.js').then((mod) => {
  assert.deepEqual(mod, fooNamespace);
});
