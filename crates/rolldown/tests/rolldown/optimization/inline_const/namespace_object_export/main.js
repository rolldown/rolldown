import assert from 'node:assert/strict';
import * as ns from './bar';

assert.deepEqual(
  ns,
  Object.defineProperty(
    {
      bar: 'bar',
    },
    Symbol.toStringTag,
    { value: 'Module' },
  ),
);
