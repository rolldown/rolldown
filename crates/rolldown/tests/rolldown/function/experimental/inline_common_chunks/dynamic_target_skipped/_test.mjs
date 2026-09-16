import assert from 'node:assert/strict';
import { readFile, readdir } from 'node:fs/promises';

globalThis.events = [];
globalThis.loads = [];
await import('./dist/a.js');
await import('./dist/b.js');
const [fromA, fromB] = await Promise.all(globalThis.loads);

assert.deepEqual(globalThis.events, ['shared:init']);
assert.equal(fromA, fromB);

const files = (await readdir(new URL('./dist/', import.meta.url), { recursive: true })).filter(
  (file) => file.endsWith('.js'),
);
const sources = await Promise.all(
  files.map((file) => readFile(new URL(`./dist/${file}`, import.meta.url), 'utf8')),
);
assert.equal(
  sources.some((source) => source.includes('shared:init')),
  true,
);
assert.equal(
  sources.some((source) => source.includes('__rd_share("rd:')),
  false,
);
