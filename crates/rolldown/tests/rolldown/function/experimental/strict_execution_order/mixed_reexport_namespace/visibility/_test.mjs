import assert from 'node:assert';
import { entries, read } from './main.js';

assert.deepEqual(entries, {
  shadowed: [
    ['LOCAL', 'local'],
    ['VALUE', 'own'],
  ],
  ambiguous: [
    ['LOCAL', 'local'],
    ['OTHER', 'other-local'],
  ],
  defaulted: [['LOCAL_DEFAULT', 'local-default']],
});
assert.deepEqual(await read(), [
  ['DEFAULT', 'default'],
  ['VALUE', 'leaf'],
]);
