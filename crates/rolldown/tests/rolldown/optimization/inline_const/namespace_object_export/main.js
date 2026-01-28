import assert from 'node:assert';
import * as ns from './bar';

assert.deepEqual(ns, {
  [Symbol.toStringTag]: 'Module',
  bar: 'bar',
});
