import assert from 'node:assert/strict';
import { readFile, readdir } from 'node:fs/promises';

globalThis.events = [];
await import('./dist/a.js');
await import('./dist/b.js');
assert.deepEqual(globalThis.events, ['tla:start', 'tla:end', 'a:7', 'b:7']);

const files = (await readdir(new URL('./dist/', import.meta.url), { recursive: true })).filter(
  (file) => file.endsWith('.js'),
);
const sources = await Promise.all(
  files.map((file) => readFile(new URL(`./dist/${file}`, import.meta.url), 'utf8')),
);
assert.equal(
  sources.some((source) => source.includes('tla:start')),
  true,
);
assert.equal(
  sources.some((source) => source.includes('__rd_share("rd:')),
  false,
);
