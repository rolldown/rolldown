import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';

globalThis.retained = [];
globalThis.values = [];
await import('./dist/a.js');
await import('./dist/b.js');

assert.deepEqual(globalThis.values, [
  'a:undefined:undefined:undefined',
  'b:undefined:undefined:undefined',
]);
assert.equal(globalThis.retained.length, 2);
assert.equal(
  (await readFile(new URL('./dist/a.js', import.meta.url), 'utf8')).includes('"rd:'),
  true,
);
