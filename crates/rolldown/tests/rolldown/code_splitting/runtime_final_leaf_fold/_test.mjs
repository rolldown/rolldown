import assert from 'node:assert/strict';
import { readdirSync } from 'node:fs';
const { value } = await import('./dist/a.js');
assert.equal(value, 'ok');
assert.equal((await import('./dist/b.js')).value, 'b');
assert.deepEqual(
  readdirSync(new URL('./dist/', import.meta.url))
    .filter((file) => file.endsWith('.js'))
    .sort(),
  ['a.js', 'b.js'],
);
