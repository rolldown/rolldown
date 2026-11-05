import assert from 'node:assert';
import Default, { Foo as OtherFoo, Bar as OtherBar, Baz as OtherBaz, SelfRef as OtherSelfRef, Named as OtherNamed } from './other.js';
import Default2 from './other2.js';
import Default3 from './other3.js';

export class Foo {}
export const Bar = class {}
export const Baz = class Baz {}
class SelfRef {}
class Named {}
function nested() {
  class Foo {}
  return Foo;
}

assert.strictEqual(Foo.name, 'Foo');
assert.strictEqual(OtherFoo.name, 'Foo');
assert.strictEqual(Bar.name, 'Bar');
assert.strictEqual(OtherBar.name, 'Bar');
assert.strictEqual(Baz.name, 'Baz');
assert.strictEqual(OtherBaz.name, 'Baz');
assert.strictEqual(nested().name, 'Foo');
assert.strictEqual(Default.name, 'default');
assert.strictEqual(Default2.name, 'default');
assert.strictEqual(Default3.name, 'default');
assert.strictEqual(SelfRef.name, 'SelfRef');
assert.strictEqual(OtherSelfRef.name, 'SelfRef');
assert.strictEqual(OtherSelfRef.self(), OtherSelfRef);
assert.strictEqual(Named.name, 'Named');
assert.strictEqual(OtherNamed.name, 'Named');
assert.strictEqual(OtherNamed.self(), OtherNamed);
