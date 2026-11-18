import assert from 'node:assert/strict';
import * as ns from './bar';

assert.deepEqual(
  ns,
  Object.defineProperty(
    {
      __proto__: null,
      bar: 'bar',
    },
    Symbol.toStringTag,
    { value: 'Module' },
  ),
);
