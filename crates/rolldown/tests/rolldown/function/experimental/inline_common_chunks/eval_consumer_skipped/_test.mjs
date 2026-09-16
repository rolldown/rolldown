import assert from 'node:assert/strict';
import { readFile, readdir } from 'node:fs/promises';

globalThis.values = [];
globalThis.sharedLoads = 0;
await import('./dist/a.js');
await import('./dist/b.js');
assert.deepEqual(globalThis.values, ['a:42:undefined', 'b:42']);
assert.equal(globalThis.sharedLoads, 1);

const files = (await readdir(new URL('./dist/', import.meta.url), { recursive: true })).filter(
  (file) => file.endsWith('.js'),
);
const sources = await Promise.all(
  files.map((file) => readFile(new URL(`./dist/${file}`, import.meta.url), 'utf8')),
);
assert.equal(
  files.some((file) => file.includes('shared')),
  true,
);
assert.equal(
  sources.some((source) => source.includes('__rd_share("rd:')),
  false,
);
