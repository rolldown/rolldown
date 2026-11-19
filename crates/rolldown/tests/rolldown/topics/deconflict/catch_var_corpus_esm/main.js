import assert from 'node:assert/strict';
import { setE } from './value.js';
import { inner, nested, loops, arrow } from './inner.js';
import { e as ea, aLog, getA } from './top_a.js';
import { e as eb, bLog, getB } from './top_b.js';
import { s, only, selfLog } from './self.js';
import { c, cycleLog } from './cycle_a.js';

// The entry's own top-level `e` competes for the name with every module above.
let e = 'main';
e += '!';

setE('set');
assert.deepStrictEqual(inner(), [2, 'set', 'set', undefined]);
assert.deepStrictEqual(nested(), [3, 'set', 1, 'set', undefined]);
assert.deepStrictEqual(loops(), [1, 2, 'k', 3, 4, 'set', undefined]);
assert.deepStrictEqual(arrow(), [5, 'set', 5]);
assert.deepStrictEqual(aLog, ['caught-a', 'a2', 'a']);
assert.deepStrictEqual([ea, getA()], ['a', 'a']);
assert.deepStrictEqual(bLog, ['b2', 'b!']);
assert.deepStrictEqual([eb, getB()], ['b!', 'b!']);
assert.deepStrictEqual(selfLog, [3, 2, 2, 4, undefined, undefined]);
assert.deepStrictEqual([s, only], [2, undefined]);
assert.deepStrictEqual(cycleLog, ['c1', 'c0', 'c2']);
assert.strictEqual(c, 'c2');
assert.strictEqual(e, 'main!');
