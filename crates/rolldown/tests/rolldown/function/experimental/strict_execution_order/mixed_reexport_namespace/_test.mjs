import assert from 'node:assert/strict';
import { entries } from './dist/main.js';

assert.deepEqual(entries, [
  ['LOCAL', 'local'],
  ['VALUE', 'leaf'],
]);
