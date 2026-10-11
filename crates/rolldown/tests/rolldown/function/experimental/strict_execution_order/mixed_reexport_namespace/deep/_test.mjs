import assert from 'node:assert';
import { entries } from './dist/main.js';

assert.deepEqual(entries, [
  ['LOCAL', 'local'],
  ['OUTER', 'outer'],
  ['VALUE', 'leaf'],
]);
