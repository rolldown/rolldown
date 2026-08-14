import assert from 'node:assert';
import { Foo as ImportedFoo } from './a.js';

class Foo {
  static self = Foo;
}

assert.strictEqual(ImportedFoo.name, 'Foo');
assert.strictEqual(Foo.self, Foo);
