import assert from 'node:assert';
import A from './a.js';
import Foo from './foo.js';

// Mirrors Rollup's `splitting-cycles-module-with-TLA-and-dynamic-importer`.
// `foo` is in a dependency cycle (foo -> a -> b -> dynamic import foo) and is
// dynamically imported by `b`, which is a top-level-await module. `foo` must be
// isolated into its own chunk; collapsing it into the importer's chunk deadlocks.
class Bar extends A {
  bar() {
    return new Foo().foo();
  }
}

assert.strictEqual(new Bar().bar(), 'hello-from-foo');
