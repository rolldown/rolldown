import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';

globalThis.values = [];
await import('./dist/a.js');
await import('./dist/b.js');
assert.deepEqual(globalThis.values, ['a:42', 'b:42']);

for (const name of ['a', 'b']) {
  const map = JSON.parse(await readFile(new URL(`./dist/${name}.js.map`, import.meta.url), 'utf8'));
  const sharedIndex = map.sources.findIndex((source) => source.endsWith('/shared.js'));
  assert.notEqual(sharedIndex, -1);
  assert.equal(map.sourcesContent[sharedIndex].includes('readMappedValue'), true);
  assert.notEqual(map.mappings, '');
}
