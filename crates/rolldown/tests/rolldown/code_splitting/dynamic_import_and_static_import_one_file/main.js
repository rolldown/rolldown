import * as fooNamespace from './foo.js';
import assert from 'node:assert';

assert.deepEqual(fooNamespace, {
  foo: 1,
});

import('./foo.js').then((mod) => {
  // workaround for the String tag `Module`
  assert.deepEqual(JSON.parse(JSON.stringify(mod)), fooNamespace);
});
