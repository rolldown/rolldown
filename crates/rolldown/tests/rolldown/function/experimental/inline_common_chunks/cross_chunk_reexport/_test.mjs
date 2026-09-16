import assert from 'node:assert/strict';
import { readFile, readdir } from 'node:fs/promises';

globalThis.events = [];
await import('./dist/a.js');
await import('./dist/b.js');
await import('./dist/c.js');
assert.deepEqual(globalThis.events, ['shared:init', 'a:1:1', 'b:2:2', 'c:2']);

const files = (await readdir(new URL('./dist/', import.meta.url), { recursive: true })).filter(
  (file) => file.endsWith('.js'),
);
const sources = await Promise.all(
  files.map((file) => readFile(new URL(`./dist/${file}`, import.meta.url), 'utf8')),
);
const keys = sources.flatMap((source) =>
  [...source.matchAll(/__rd_share\("(rd:[^"]+)"/g)].map((match) => match[1]),
);
assert.equal(new Set(keys).size, 2);
assert.equal(sources.filter((source) => source.includes('shared:init')).length, 3);
