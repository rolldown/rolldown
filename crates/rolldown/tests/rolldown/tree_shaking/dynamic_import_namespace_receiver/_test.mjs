import assert from 'node:assert/strict';
const { value, merged, ...loaders } = await import('./dist/main.js');
assert.equal(value, 'expected');
assert.equal(await merged(), 'expected', 'merged');
for (const [name, load] of Object.entries(loaders)) {
  assert.equal(await load(), 'expected', name);
}
