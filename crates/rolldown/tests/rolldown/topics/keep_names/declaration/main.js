// MULTIPLE ENTRY MODULES
import { test as s, a as b, Foo as Bar } from './a.js';
import assert from 'node:assert'


s();
b();
const test = 10;
class Foo extends Bar {

}
console.log(`test: `, test)
assert.strictEqual(Foo.name, "Foo")
assert.strictEqual(Bar.name, "Foo")

assert.strictEqual(s.name, "test")
