import assert from 'node:assert';
import { readFile } from 'node:fs/promises';

const read = (name) => readFile(new URL(`./dist/${name}`, import.meta.url), 'utf8');

// Pin the routing structurally, not only through the exported values: placement
// puts each pure CJS leaf into only its consuming entry's chunk, and lowering
// must emit the matching per-record carriers, so the sibling leaf's body may not
// appear in the other entry's chunk and the shared barrel chunk may not retain a
// monolithic leaf initialization. (Event-based assertions cannot pin this here:
// giving the leaves observable side effects makes their carriers eager by
// design, so every consumer would then correctly initialize both leaves.)
const entryA = await read('entry-a.js');
const entryB = await read('entry-b.js');
const barrel = await read('barrel.js');
assert.match(entryA, /exports\.a/);
assert.doesNotMatch(entryA, /exports\.b/);
assert.match(entryB, /exports\.b/);
assert.doesNotMatch(entryB, /exports\.a/);
assert.doesNotMatch(barrel, /exports\.[ab]/);

const a = await import('./dist/entry-a.js');
assert.strictEqual(a.a, 'a');

const b = await import('./dist/entry-b.js');
assert.strictEqual(b.b, 'b');
