import assert from 'node:assert';
import { result, read } from './main.js';

assert.equal(result, 'local');
assert.deepEqual(await read(), [
  ['LOCAL', 'local'],
  ['VALUE', 'leaf'],
]);
