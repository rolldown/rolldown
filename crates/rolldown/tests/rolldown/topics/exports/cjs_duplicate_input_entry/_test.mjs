import assert from 'node:assert/strict';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
for (const [sourcePath, first, second] of [
  ['./lib.cjs', 'a', 'b'],
  ['./primitive.cjs', 'primitive_a', 'primitive_b'],
  ['./null.cjs', 'null_a', 'null_b'],
  ['./empty.cjs', 'empty_a', 'empty_b'],
]) {
  const source = require(sourcePath);
  const a = require(`./dist/${first}.js`);
  const b = require(`./dist/${second}.js`);
  assert.deepStrictEqual(a, source);
  assert.deepStrictEqual(b, source);
  assert.strictEqual(a, b);
}

const a = require('./dist/a.js');
assert.equal(a.default, 'named default');
assert.equal(a.__proto__, 'own proto');
assert.ok(Object.hasOwn(a, '__proto__'));

const source = require('./callable.cjs');
const callableA = require('./dist/callable_a.js');
const callableB = require('./dist/callable_b.js');
assert.equal(typeof callableA, 'function');
assert.equal(callableA(41), source(41));
assert.strictEqual(callableA, callableB);
