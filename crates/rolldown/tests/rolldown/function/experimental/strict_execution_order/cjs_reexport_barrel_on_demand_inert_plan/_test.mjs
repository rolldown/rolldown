import assert from 'node:assert';
import { readFile } from 'node:fs/promises';

const read = (name) => readFile(new URL(`./dist/${name}`, import.meta.url), 'utf8');

// The plain ESM entry keeps the wrap-all order plan non-empty while the on-demand
// plan stays empty (nothing is at risk and the interop-wrapped barrel is not
// order-wrap-eligible). Consumer-local carrier lowering must still run in both
// modes, or the barrel keeps its monolithic interop body against per-consumer
// leaf placement and the emitted chunks require each other in a startup cycle.
//
// Same structural pins as `cjs_reexport_barrel_probe_plan_divergence`: each pure
// CJS leaf may appear only in its consuming entry's chunk, and the shared barrel
// chunk may not retain a monolithic leaf initialization.
const entryA = await read('entry-a.js');
const entryB = await read('entry-b.js');
const entryC = await read('entry-c.js');
const barrel = await read('barrel.js');
assert.match(entryA, /exports\.a/);
assert.doesNotMatch(entryA, /exports\.b/);
assert.match(entryB, /exports\.b/);
assert.doesNotMatch(entryB, /exports\.a/);
assert.doesNotMatch(entryC, /exports\.[ab]/);
assert.doesNotMatch(barrel, /exports\.[ab]/);

const a = await import('./dist/entry-a.js');
assert.strictEqual(a.a, 'a');

const b = await import('./dist/entry-b.js');
assert.strictEqual(b.b, 'b');

const c = await import('./dist/entry-c.js');
assert.strictEqual(c.c, 'side');
