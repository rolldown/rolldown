import { strict as assert } from 'node:assert';
import { createRequire } from 'node:module';

// A wrapper renamed only against `dup.cjs`'s own bindings lands on `read`'s parameter, and `read`
// then calls the parameter instead of the wrapper: `['param', 'param']`, with no error at all.
const require = createRequire(import.meta.url);
const { read } =
  globalThis.__configName === 'cjs' ? require('./dist/main.js') : await import('./dist/main.js');

assert.deepEqual(
  read(() => 'param'),
  ['dep', 'param'],
);
