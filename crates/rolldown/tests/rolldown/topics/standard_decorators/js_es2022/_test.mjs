import assert from 'node:assert';
import Anon, { Foo, log } from './dist/main.js';
const foo = new Foo();
assert.strictEqual(foo.x, 2);
assert.strictEqual(foo.y, 103);
foo.y = 5;
assert.strictEqual(foo.y, 6);
assert.strictEqual(foo.greet('a'), 'hi a 2');
assert.strictEqual(Foo.reveal(), 7);
assert.strictEqual(Foo.name, 'Foo');
assert.strictEqual(Anon.name, 'default');
const metaKey = Symbol.metadata ?? Symbol.for('Symbol.metadata');
assert.strictEqual(Foo[metaKey]?.tagged, true);
assert.deepStrictEqual(log, [
  'method:reveal',
  'accessor:y',
  'method:greet',
  'field:x',
  'class:Foo',
  'init:Foo',
  'class:default',
  'init:default',
  'call:greet',
  'call:reveal',
]);
