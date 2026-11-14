import assert from 'node:assert';
import { Foo as F, Named } from './a.js';
import { Baz } from './b.js';

export class Foo {}

assert.strictEqual(F.name, 'Foo');
assert.strictEqual(F.tag, 'Foo');
assert.strictEqual(F.tag2, 'Foo');
assert.strictEqual(Named.name, 'Baz');
assert.strictEqual(Named.tag, 'Baz');
assert.strictEqual(Named.base(), Baz);
