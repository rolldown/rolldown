import assert from 'node:assert/strict';
import { readdir } from 'node:fs/promises';

const files = await readdir(new URL('./dist/', import.meta.url));
assert.equal(files.filter((file) => file.startsWith('app')).length, 1);

await import('./dist/first.js');
await import('./dist/second.js');
assert.equal(globalThis.__tla_static_first, 1);
assert.equal(globalThis.__tla_static_second, 4);
