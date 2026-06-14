import assert from 'node:assert';
import A from './a.js';
import Foo from './foo.js';

// Mirrors Rollup's `splitting-cycles-module-with-TLA-and-dynamic-importer`.
// Rolldown may collapse the whole statically reachable cycle into one chunk, so
// this fixture asserts both the direct static path and the rewritten dynamic
// import path execute correctly.
class Bar extends A {
  bar() {
    return new Foo().foo();
  }
}

assert.strictEqual(new Bar().bar(), 'hello-from-foo');
assert.strictEqual((await new A().foo()).foo(), 'hello-from-foo');
