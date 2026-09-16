import assert from 'node:assert/strict';
import { readFile, readdir } from 'node:fs/promises';

globalThis.values = [];
await import('./dist/entries/a.js');
await import('./dist/entries/b.js');
assert.equal(globalThis.values.length, 2);
assert.equal(globalThis.values[0], globalThis.values[1]);
assert.equal(globalThis.values[0].includes('/chunks/'), true);

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
