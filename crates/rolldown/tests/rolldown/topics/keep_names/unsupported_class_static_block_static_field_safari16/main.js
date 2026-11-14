import assert from 'node:assert';
import Default, { Foo as F, Bar as B } from './a.js';
import { Qux as Q, Holder } from './b.js';

export class Foo {}
export const Bar = class {};
export class Qux {}

assert.strictEqual(F.name, 'Foo');
assert.strictEqual(F.tag, 'Foo');
assert.strictEqual(F.tag2, 'Foo');
assert.strictEqual(B.name, 'Bar');
assert.strictEqual(B.tag, 'Bar');
assert.strictEqual(Default.name, 'default');
assert.strictEqual(Default.tag, 'default');

assert.strictEqual(Q.name, 'Qux');
assert.strictEqual(Q.tag, 'Qux');
assert.strictEqual(Q.read(), 'qux');

const Made = Holder.make();
assert.strictEqual(Made.name, 'Baz');
assert.strictEqual(Made.tag, 'Baz');
assert.strictEqual(Made.read(), 'holder');
