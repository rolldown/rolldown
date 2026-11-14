import assert from 'node:assert';

await import('./dist/main.js');

assert.strictEqual(
  globalThis.__result,
  'tag:PREF_OK',
  'sync capture must observe the initialized wrapped facade',
);
assert.strictEqual(
  await globalThis.__ready,
  'tag:PREF_OK',
  'async capture must observe the initialized wrapped facade',
);
