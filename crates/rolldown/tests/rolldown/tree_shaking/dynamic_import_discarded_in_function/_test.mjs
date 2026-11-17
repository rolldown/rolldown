import assert from 'node:assert';
import { loadB, loadC } from './dist/main.js';

assert.strictEqual(await globalThis.aLoaded, 'a ran');
// No code reads the namespace of `a.js`, so tree shaking removes its exports.
assert.deepStrictEqual(Object.keys(await import('./dist/a.js')), []);
assert.deepStrictEqual(Object.keys(await loadB()), ['b1', 'b2']);
assert.deepStrictEqual(Object.keys(await loadC()), ['c1', 'c2']);
// `then` reads `d1` through `this`, so the value is correct only if `d.js` keeps `d1`.
assert.strictEqual(await globalThis.dThen, 'd1');
