import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { load_a, load_b, load_named } from './dist/main.js';

const shared = readFileSync(new URL('./dist/shared.js', import.meta.url), 'utf8');
if (globalThis.__configName !== 'no-treeshake') {
  assert.ok(!shared.includes('DEAD'), 'dead warning must not be rooted by a chunk export');
  assert.ok(!shared.includes('USED'));
  const exports = await import('./dist/shared.js');
  assert.equal(Object.keys(exports).length, 1, 'only shared should be exported');
}
assert.equal((await load_a()).a, 'shared');
assert.equal((await load_b()).b, 'shared');
assert.equal((await load_named()).named, 'shared');
