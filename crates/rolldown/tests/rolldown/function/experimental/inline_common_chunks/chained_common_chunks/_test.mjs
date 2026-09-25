import assert from 'node:assert/strict';
import { readFile, readdir } from 'node:fs/promises';

globalThis.events = [];
await import('./dist/a.js');
await import('./dist/b.js');
await import('./dist/c.js');

assert.deepEqual(globalThis.events, ['base:init', 'ab:1', 'ac:2', 'a', 'b', 'c']);

const files = (await readdir(new URL('./dist/', import.meta.url), { recursive: true })).filter(
  (file) => file.endsWith('.js'),
);
const sources = await Promise.all(
  files.map((file) => readFile(new URL(`./dist/${file}`, import.meta.url), 'utf8')),
);
const keys = sources.flatMap((source) =>
  [...source.matchAll(/__rd_share\("(rd:[^"]+)"/g)].map((match) => match[1]),
);
assert.equal(new Set(keys).size, 3);
