import assert from 'node:assert';
import { Foo as F } from './a.js';

// `F` is printed as `Foo`, so each class below gets its own name renamed.
export const X = class Foo {
  static get() {
    return F;
  }
};
export const Y = class Foo {
  static self() {
    return Foo;
  }
  static get() {
    return F;
  }
};
export function make() {
  class Foo {
    static get() {
      return F;
    }
  }
  return Foo;
}

assert.strictEqual(F.name, 'Foo');
assert.strictEqual(X.name, 'Foo');
assert.strictEqual(X.get(), F);
assert.strictEqual(Y.name, 'Foo');
assert.strictEqual(Y.self(), Y);
assert.strictEqual(make().name, 'Foo');
assert.strictEqual(make().get(), F);
