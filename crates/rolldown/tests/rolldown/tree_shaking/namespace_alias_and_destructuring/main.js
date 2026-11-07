import assert from 'node:assert';
import * as foo from './foo.js';
import * as bar from './bar.js';

// Declared before the alias it destructures.
function readB() {
  const { b } = aliasOfAlias;
  return b();
}

const alias = foo;
const aliasOfAlias = alias;
const { a, missing } = alias;

assert.strictEqual(a, 'a');
assert.strictEqual(missing, undefined);
assert.strictEqual(readB(), 'b');

function shadowed(foo) {
  const { a } = foo;
  return a;
}
assert.strictEqual(shadowed({ a: 1 }), 1);

// Defaults and rest elements keep the whole namespace.
const barAlias = bar;
const { x = 'default', ...rest } = barAlias;
assert.strictEqual(x, 'x');
assert.deepStrictEqual(rest, { y: 'y' });
assert.strictEqual(barAlias, bar);
