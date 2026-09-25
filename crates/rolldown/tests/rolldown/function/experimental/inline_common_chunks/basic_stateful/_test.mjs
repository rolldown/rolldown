import assert from 'node:assert/strict';
import { readFile, readdir } from 'node:fs/promises';

globalThis.events = [];
await import('./dist/a.js');
await import('./dist/b.js');

assert.deepEqual(globalThis.events, ['shared:init', 'plain-this', 'a:1', 'plain-this', 'b:2']);

const files = (await readdir(new URL('./dist/', import.meta.url), { recursive: true })).filter(
  (file) => file.endsWith('.js'),
);
const sources = await Promise.all(
  files.map((file) => readFile(new URL(`./dist/${file}`, import.meta.url), 'utf8')),
);
assert.equal(
  files.some((file) => file.includes('shared')),
  false,
);
if (globalThis.__configName === 'minified') {
  assert.equal(sources.filter((source) => source.includes('rd:')).length, 2);
} else {
  assert.equal(sources.filter((source) => source.includes('__rd_share("rd:')).length, 2);
}
