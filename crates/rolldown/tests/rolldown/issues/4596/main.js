import { Foo as Bar } from './a.js';
import assert from 'node:assert';

class Foo {
}

assert.strictEqual(Foo.name, "Foo");
assert.strictEqual(Bar.name, "Foo");
